//! Custom `asset://` URI scheme protocol handler with HTTP 206 Partial Content Range streaming.
//!
//! Enables zero-network local media streaming (`TASK-M4-07`, `AC-07.2`, `ED-07.2`)
//! for arbitrary user-dropped video files without static path-scope rejections and
//! with bounded chunk reads (2 MiB max per request) for sub-100ms 1080p60 seeking.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf, Prefix};

use percent_encoding::percent_decode_str;
use tauri::http::{header, Method, Request, Response, StatusCode};

/// Maximum bytes read into memory for a single HTTP 206 Range chunk (2 MiB).
pub const MAX_STREAM_CHUNK_BYTES: usize = 2 * 1024 * 1024;

/// Maximum bytes read into memory for an un-ranged full `GET` response (16 MiB).
const MAX_FULL_GET_BYTES: u64 = 16 * 1024 * 1024;

const ALLOWED_ASSET_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "webm", "mov", "ts", "flv", "m4v", "avi", "wav", "m4a", "aac", "mp3", "opus",
    "ogg", "png", "jpg", "jpeg", "webp", "ass", "srt", "vtt",
];

/// Parsed inclusive byte range `(start, end)` for a file of `total_len` bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HttpByteRange {
    pub start: u64,
    pub end: u64,
}

/// Errors encountered while parsing an HTTP `Range` header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeParseError {
    Malformed,
    Unsatisfiable,
}

/// Errors encountered while decoding or validating an `asset://` request path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetPathError {
    Forbidden(String),
    InvalidEncoding(String),
}

/// Detect MIME type from a local media file extension.
#[must_use]
pub fn detect_media_mime_type(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);

    match ext.as_deref() {
        Some("mp4" | "m4v") => "video/mp4",
        Some("mkv") => "video/x-matroska",
        Some("webm") => "video/webm",
        Some("mov") => "video/quicktime",
        Some("ts") => "video/mp2t",
        Some("flv") => "video/x-flv",
        Some("avi") => "video/x-msvideo",
        Some("wav") => "audio/wav",
        Some("m4a" | "aac") => "audio/mp4",
        Some("mp3") => "audio/mpeg",
        Some("opus" | "ogg") => "audio/ogg",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("vtt") => "text/vtt; charset=utf-8",
        Some("ass" | "srt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// Return true if the file has an allowed media/asset extension.
#[must_use]
pub fn is_allowed_asset_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .is_some_and(|ext| ALLOWED_ASSET_EXTENSIONS.contains(&ext.as_str()))
}

fn has_unc_or_network_prefix(raw: &str) -> bool {
    raw.starts_with("//")
        || raw.starts_with("\\\\")
        || raw.starts_with("/\\")
        || raw.starts_with("\\/")
}

/// Decode and validate a URI path from an `asset://` / `http://asset.localhost/...` request.
///
/// Rejects null bytes, `..` path traversal segments, UNC/network/device paths,
/// relative paths, and Windows drive-relative paths (`C:relative.mp4`).
///
/// # Errors
/// Returns [`AssetPathError`] when encoding is invalid or path validation fails.
pub fn decode_asset_request_path(uri_path: &str) -> Result<PathBuf, AssetPathError> {
    let decoded = percent_decode_str(uri_path)
        .decode_utf8()
        .map_err(|e| AssetPathError::InvalidEncoding(e.to_string()))?
        .into_owned();

    if decoded.contains('\0') {
        return Err(AssetPathError::Forbidden(
            "Null byte in asset path".to_string(),
        ));
    }

    // Reject explicit parent-dir segments regardless of slash style across platforms.
    for seg in decoded.split(['/', '\\']) {
        if seg == ".." {
            return Err(AssetPathError::Forbidden(
                "Path traversal ('..') is forbidden".to_string(),
            ));
        }
    }

    if has_unc_or_network_prefix(&decoded) {
        return Err(AssetPathError::Forbidden(
            "UNC and network asset paths are forbidden".to_string(),
        ));
    }

    let mut normalized = decoded.as_str();
    // WebView2 / Tauri convertFileSrc sends `/C:/path/to/file.mp4` or `/C%3A%5C...`
    if let Some(stripped) = normalized.strip_prefix('/') {
        let bytes = stripped.as_bytes();
        if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
            normalized = stripped;
        }
    }

    if normalized.trim().is_empty() {
        return Err(AssetPathError::Forbidden("Empty asset path".to_string()));
    }

    if has_unc_or_network_prefix(normalized) {
        return Err(AssetPathError::Forbidden(
            "UNC and network asset paths are forbidden".to_string(),
        ));
    }

    let path = PathBuf::from(normalized);
    let mut has_root_dir = false;
    let mut has_local_disk_prefix = false;

    for comp in path.components() {
        match comp {
            Component::ParentDir => {
                return Err(AssetPathError::Forbidden(
                    "Parent directory traversal is forbidden".to_string(),
                ));
            }
            Component::Prefix(prefix) => match prefix.kind() {
                Prefix::Disk(_) | Prefix::VerbatimDisk(_) => {
                    has_local_disk_prefix = true;
                }
                Prefix::UNC(_, _) | Prefix::VerbatimUNC(_, _) | Prefix::DeviceNS(_) => {
                    return Err(AssetPathError::Forbidden(
                        "UNC and device namespace paths are forbidden".to_string(),
                    ));
                }
                Prefix::Verbatim(_) => {
                    return Err(AssetPathError::Forbidden(
                        "Verbatim non-disk paths are forbidden".to_string(),
                    ));
                }
            },
            Component::RootDir => {
                has_root_dir = true;
            }
            Component::CurDir | Component::Normal(_) => {}
        }
    }

    if !path.is_absolute() || !has_root_dir {
        return Err(AssetPathError::Forbidden(
            "Asset path must be an absolute local disk path".to_string(),
        ));
    }

    if cfg!(windows) && !has_local_disk_prefix {
        return Err(AssetPathError::Forbidden(
            "Windows asset path must start with a local disk drive root".to_string(),
        ));
    }

    Ok(path)
}

fn parse_ascii_digits_u64(s: &str) -> Result<u64, RangeParseError> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(RangeParseError::Malformed);
    }
    s.parse::<u64>().map_err(|_| RangeParseError::Malformed)
}

/// Parse an HTTP `Range: bytes=...` header against a file of length `total_len`.
///
/// Open-ended or oversized ranges are capped at [`MAX_STREAM_CHUNK_BYTES`] (2 MiB)
/// so seeking stays bounded in memory and fast on large 1080p60 recordings.
///
/// # Errors
/// Returns [`RangeParseError::Malformed`] or [`RangeParseError::Unsatisfiable`].
pub fn parse_http_range(
    range_header: &str,
    total_len: u64,
) -> Result<HttpByteRange, RangeParseError> {
    if total_len == 0 {
        return Err(RangeParseError::Unsatisfiable);
    }

    let trimmed = range_header.trim();
    let spec = trimmed
        .strip_prefix("bytes=")
        .or_else(|| trimmed.strip_prefix("BYTES="))
        .ok_or(RangeParseError::Malformed)?
        .trim();

    if spec.is_empty() || spec.contains(',') {
        // Multi-range requests are rejected cleanly; media players issue single ranges.
        return Err(RangeParseError::Malformed);
    }

    let (start_str, end_str) = spec.split_once('-').ok_or(RangeParseError::Malformed)?;
    let start_str = start_str.trim();
    let end_str = end_str.trim();

    let max_chunk = MAX_STREAM_CHUNK_BYTES as u64;

    if start_str.is_empty() {
        // Suffix range: bytes=-<suffix>
        let suffix = parse_ascii_digits_u64(end_str)?;
        if suffix == 0 {
            return Err(RangeParseError::Unsatisfiable);
        }
        let take = suffix.min(total_len).min(max_chunk);
        let start = total_len - take;
        let end = total_len - 1;
        return Ok(HttpByteRange { start, end });
    }

    let start = parse_ascii_digits_u64(start_str)?;
    if start >= total_len {
        return Err(RangeParseError::Unsatisfiable);
    }

    let requested_end = if end_str.is_empty() {
        total_len - 1
    } else {
        let parsed_end = parse_ascii_digits_u64(end_str)?;
        if parsed_end < start {
            return Err(RangeParseError::Malformed);
        }
        parsed_end.min(total_len - 1)
    };

    let capped_end = start
        .saturating_add(max_chunk.saturating_sub(1))
        .min(requested_end);

    Ok(HttpByteRange {
        start,
        end: capped_end,
    })
}

fn build_error_response(
    status: StatusCode,
    message: &str,
    content_range: Option<&str>,
) -> Response<Vec<u8>> {
    let mut builder = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(
            header::ACCESS_CONTROL_EXPOSE_HEADERS,
            "Content-Range, Accept-Ranges, Content-Length",
        );

    if let Some(cr) = content_range {
        builder = builder.header(header::CONTENT_RANGE, cr);
    }

    builder
        .body(message.as_bytes().to_vec())
        .unwrap_or_else(|_| Response::new(message.as_bytes().to_vec()))
}

fn read_file_slice(file: &mut File, start: u64, len: usize) -> Result<Vec<u8>, std::io::Error> {
    file.seek(SeekFrom::Start(start))?;
    let mut buf = vec![0u8; len];
    file.read_exact(&mut buf)?;
    Ok(buf)
}

fn handle_ranged_request(
    file: &mut File,
    range_str: &str,
    total_len: u64,
    mime_type: &'static str,
    is_head: bool,
) -> Response<Vec<u8>> {
    let byte_range = match parse_http_range(range_str, total_len) {
        Ok(r) => r,
        Err(RangeParseError::Unsatisfiable) => {
            let cr = format!("bytes */{total_len}");
            return build_error_response(
                StatusCode::RANGE_NOT_SATISFIABLE,
                "Range Not Satisfiable",
                Some(&cr),
            );
        }
        Err(RangeParseError::Malformed) => {
            return build_error_response(
                StatusCode::RANGE_NOT_SATISFIABLE,
                "Malformed Range header",
                None,
            );
        }
    };

    let chunk_len_u64 = byte_range.end - byte_range.start + 1;
    let Ok(chunk_len) = usize::try_from(chunk_len_u64) else {
        return build_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Chunk length exceeds addressable memory",
            None,
        );
    };

    let body = if is_head {
        Vec::new()
    } else {
        match read_file_slice(file, byte_range.start, chunk_len) {
            Ok(b) => b,
            Err(e) => {
                return build_error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    &format!("Failed to read range slice: {e}"),
                    None,
                );
            }
        }
    };

    let content_range = format!("bytes {}-{}/{total_len}", byte_range.start, byte_range.end);
    Response::builder()
        .status(StatusCode::PARTIAL_CONTENT)
        .header(header::CONTENT_TYPE, mime_type)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_RANGE, content_range)
        .header(header::CONTENT_LENGTH, chunk_len_u64.to_string())
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(
            header::ACCESS_CONTROL_EXPOSE_HEADERS,
            "Content-Range, Accept-Ranges, Content-Length",
        )
        .body(body)
        .unwrap_or_else(|_| Response::new(Vec::new()))
}

fn handle_unranged_request(
    file: &mut File,
    total_len: u64,
    mime_type: &'static str,
    is_head: bool,
) -> Response<Vec<u8>> {
    if is_head || total_len == 0 {
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime_type)
            .header(header::ACCEPT_RANGES, "bytes")
            .header(header::CONTENT_LENGTH, total_len.to_string())
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .header(
                header::ACCESS_CONTROL_EXPOSE_HEADERS,
                "Content-Range, Accept-Ranges, Content-Length",
            )
            .body(Vec::new())
            .unwrap_or_else(|_| Response::new(Vec::new()));
    }

    if total_len <= MAX_FULL_GET_BYTES {
        let Ok(full_len) = usize::try_from(total_len) else {
            return build_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "File length exceeds addressable memory",
                None,
            );
        };
        let body = match read_file_slice(file, 0, full_len) {
            Ok(b) => b,
            Err(e) => {
                return build_error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    &format!("Failed to read asset: {e}"),
                    None,
                );
            }
        };
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime_type)
            .header(header::ACCEPT_RANGES, "bytes")
            .header(header::CONTENT_LENGTH, total_len.to_string())
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .header(
                header::ACCESS_CONTROL_EXPOSE_HEADERS,
                "Content-Range, Accept-Ranges, Content-Length",
            )
            .body(body)
            .unwrap_or_else(|_| Response::new(Vec::new()));
    }

    let chunk_len = MAX_STREAM_CHUNK_BYTES;
    let end = (chunk_len as u64) - 1;
    let body = match read_file_slice(file, 0, chunk_len) {
        Ok(b) => b,
        Err(e) => {
            return build_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Failed to read initial chunk: {e}"),
                None,
            );
        }
    };
    let content_range = format!("bytes 0-{end}/{total_len}");
    Response::builder()
        .status(StatusCode::PARTIAL_CONTENT)
        .header(header::CONTENT_TYPE, mime_type)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_RANGE, content_range)
        .header(header::CONTENT_LENGTH, chunk_len.to_string())
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(
            header::ACCESS_CONTROL_EXPOSE_HEADERS,
            "Content-Range, Accept-Ranges, Content-Length",
        )
        .body(body)
        .unwrap_or_else(|_| Response::new(Vec::new()))
}

/// Pure synchronous HTTP request handler for the custom `asset://` URI scheme.
///
/// Supports:
/// - `OPTIONS` CORS preflight
/// - `GET` and `HEAD`
/// - `206 Partial Content` HTTP Range streaming (`Content-Range`, `Accept-Ranges: bytes`)
/// - `416 Range Not Satisfiable`
/// - `403 Forbidden` for traversal (`..`) or disallowed extensions
/// - `404 Not Found` for missing files
#[must_use]
pub fn handle_asset_request(request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    if request.method() == Method::OPTIONS {
        return Response::builder()
            .status(StatusCode::NO_CONTENT)
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .header(header::ACCESS_CONTROL_ALLOW_METHODS, "GET, HEAD, OPTIONS")
            .header(header::ACCESS_CONTROL_ALLOW_HEADERS, "Range, Content-Type")
            .header(
                header::ACCESS_CONTROL_EXPOSE_HEADERS,
                "Content-Range, Accept-Ranges, Content-Length",
            )
            .header(header::ACCEPT_RANGES, "bytes")
            .body(Vec::new())
            .unwrap_or_else(|_| Response::new(Vec::new()));
    }

    let is_head = request.method() == Method::HEAD;
    if request.method() != Method::GET && !is_head {
        return build_error_response(StatusCode::METHOD_NOT_ALLOWED, "Method Not Allowed", None);
    }

    let file_path = match decode_asset_request_path(request.uri().path()) {
        Ok(p) => p,
        Err(AssetPathError::Forbidden(msg)) => {
            return build_error_response(StatusCode::FORBIDDEN, &msg, None);
        }
        Err(AssetPathError::InvalidEncoding(msg)) => {
            return build_error_response(StatusCode::BAD_REQUEST, &msg, None);
        }
    };

    if !is_allowed_asset_extension(&file_path) {
        return build_error_response(
            StatusCode::FORBIDDEN,
            "Unsupported or disallowed asset extension",
            None,
        );
    }

    if !file_path.is_file() {
        return build_error_response(StatusCode::NOT_FOUND, "Asset file not found", None);
    }

    let mut file = match File::open(&file_path) {
        Ok(f) => f,
        Err(e) => {
            return build_error_response(
                StatusCode::NOT_FOUND,
                &format!("Failed to open asset: {e}"),
                None,
            );
        }
    };

    let total_len = match file.metadata() {
        Ok(m) => m.len(),
        Err(e) => {
            return build_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Failed to stat asset: {e}"),
                None,
            );
        }
    };

    let mime_type = detect_media_mime_type(&file_path);

    if let Some(range_val) = request.headers().get(header::RANGE) {
        let Ok(range_str) = range_val.to_str() else {
            return build_error_response(
                StatusCode::RANGE_NOT_SATISFIABLE,
                "Invalid Range header",
                None,
            );
        };
        return handle_ranged_request(&mut file, range_str, total_len, mime_type, is_head);
    }

    handle_unranged_request(&mut file, total_len, mime_type, is_head)
}

/// Asynchronous Tauri URI scheme protocol handler for `"asset"`.
///
/// Offloads blocking file I/O to Tauri's blocking thread pool so the `WebView` UI thread
/// never stalls during 1080p60 scrub/seek operations.
pub fn handle_asset_protocol<R: tauri::Runtime>(
    _ctx: tauri::UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
    responder: tauri::UriSchemeResponder,
) {
    tauri::async_runtime::spawn_blocking(move || {
        let response = handle_asset_request(&request);
        responder.respond(response);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_temp_media_file(name: &str, bytes: &[u8]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "neuroclip_protocol_test_{}_{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join(name);
        let mut f = File::create(&file_path).unwrap();
        f.write_all(bytes).unwrap();
        file_path
    }

    fn path_to_uri(path: &Path) -> String {
        let raw = path.to_string_lossy().replace('\\', "/");
        if raw.starts_with('/') {
            format!("http://asset.localhost{raw}")
        } else {
            format!("http://asset.localhost/{raw}")
        }
    }

    #[test]
    fn test_parse_http_range_variants() {
        assert_eq!(
            parse_http_range("bytes=0-1023", 10_000),
            Ok(HttpByteRange {
                start: 0,
                end: 1023
            })
        );
        assert_eq!(
            parse_http_range("bytes=500-99999", 1_000),
            Ok(HttpByteRange {
                start: 500,
                end: 999
            })
        );
        // Open-ended range on 10 MiB file caps at MAX_STREAM_CHUNK_BYTES (2 MiB)
        let ten_mb = 10 * 1024 * 1024;
        assert_eq!(
            parse_http_range("bytes=1024-", ten_mb),
            Ok(HttpByteRange {
                start: 1024,
                end: 1024 + (MAX_STREAM_CHUNK_BYTES as u64) - 1
            })
        );
        // Suffix range
        assert_eq!(
            parse_http_range("bytes=-256", 1_000),
            Ok(HttpByteRange {
                start: 744,
                end: 999
            })
        );
        // Out of bounds -> Unsatisfiable
        assert_eq!(
            parse_http_range("bytes=1000-", 1_000),
            Err(RangeParseError::Unsatisfiable)
        );
        // Malformed -> Malformed
        assert_eq!(
            parse_http_range("items=0-10", 1_000),
            Err(RangeParseError::Malformed)
        );
        // Leading '+' sign in range bounds -> Malformed (RFC 9110 requires 1*DIGIT)
        assert_eq!(
            parse_http_range("bytes=+0-5", 1_000),
            Err(RangeParseError::Malformed)
        );
        assert_eq!(
            parse_http_range("bytes=0-+5", 1_000),
            Err(RangeParseError::Malformed)
        );
        assert_eq!(
            parse_http_range("bytes=-+5", 1_000),
            Err(RangeParseError::Malformed)
        );
    }

    #[test]
    fn test_decode_asset_request_path_and_traversal_rejection() {
        let decoded = decode_asset_request_path("/D%3A/Videos/Neuro%20Clip.mp4").unwrap();
        assert_eq!(decoded, PathBuf::from("D:/Videos/Neuro Clip.mp4"));

        assert!(matches!(
            decode_asset_request_path("/D:/Videos/../secret.mp4"),
            Err(AssetPathError::Forbidden(_))
        ));
        assert!(matches!(
            decode_asset_request_path("/D:/Videos/%2e%2e/secret.mp4"),
            Err(AssetPathError::Forbidden(_))
        ));
    }

    #[test]
    fn test_decode_asset_request_path_rejects_unc_relative_and_drive_relative() {
        for rejected in [
            "//127.0.0.1/share/secret.mp4",
            r"\\127.0.0.1\share\secret.mp4",
            r"/\\127.0.0.1\share\secret.mp4",
            r"/\127.0.0.1\share\secret.mp4",
            r"\\?\UNC\127.0.0.1\share\secret.mp4",
            r"\\.\PhysicalDrive0",
            "relative.mp4",
            "/C:relative.mp4",
            "C:relative.mp4",
        ] {
            assert!(
                matches!(
                    decode_asset_request_path(rejected),
                    Err(AssetPathError::Forbidden(_))
                ),
                "Expected Forbidden for path '{rejected}'"
            );
        }
    }

    #[test]
    fn test_handle_asset_request_206_partial_content_and_headers() {
        let payload: Vec<u8> = (0u8..=255u8).cycle().take(8192).collect();
        let file_path = make_temp_media_file("sample.mp4", &payload);

        let req = Request::builder()
            .method(Method::GET)
            .uri(path_to_uri(&file_path))
            .header(header::RANGE, "bytes=100-199")
            .body(Vec::new())
            .unwrap();

        let resp = handle_asset_request(&req);
        assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(resp.headers().get(header::ACCEPT_RANGES).unwrap(), "bytes");
        assert_eq!(
            resp.headers().get(header::CONTENT_RANGE).unwrap(),
            "bytes 100-199/8192"
        );
        assert_eq!(resp.headers().get(header::CONTENT_LENGTH).unwrap(), "100");
        assert_eq!(
            resp.headers().get(header::CONTENT_TYPE).unwrap(),
            "video/mp4"
        );
        assert_eq!(resp.body(), &payload[100..200]);

        let _ = std::fs::remove_dir_all(file_path.parent().unwrap());
    }

    #[test]
    fn test_handle_asset_request_416_head_options_and_404() {
        let file_path = make_temp_media_file("clip.mkv", b"hello_mkv_data");

        // 416 Range Not Satisfiable
        let req_416 = Request::builder()
            .method(Method::GET)
            .uri(path_to_uri(&file_path))
            .header(header::RANGE, "bytes=9999-")
            .body(Vec::new())
            .unwrap();
        let resp_416 = handle_asset_request(&req_416);
        assert_eq!(resp_416.status(), StatusCode::RANGE_NOT_SATISFIABLE);
        assert_eq!(
            resp_416.headers().get(header::CONTENT_RANGE).unwrap(),
            "bytes */14"
        );

        // HEAD returns 200 with full Content-Length and empty body
        let req_head = Request::builder()
            .method(Method::HEAD)
            .uri(path_to_uri(&file_path))
            .body(Vec::new())
            .unwrap();
        let resp_head = handle_asset_request(&req_head);
        assert_eq!(resp_head.status(), StatusCode::OK);
        assert_eq!(
            resp_head.headers().get(header::CONTENT_LENGTH).unwrap(),
            "14"
        );
        assert_eq!(resp_head.body().len(), 0);

        // OPTIONS returns 204 CORS headers
        let req_opt = Request::builder()
            .method(Method::OPTIONS)
            .uri(path_to_uri(&file_path))
            .body(Vec::new())
            .unwrap();
        let resp_opt = handle_asset_request(&req_opt);
        assert_eq!(resp_opt.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            resp_opt
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .unwrap(),
            "*"
        );

        // Disallowed extension -> 403
        let req_forbidden = Request::builder()
            .method(Method::GET)
            .uri("http://asset.localhost/C:/Windows/System32/cmd.exe")
            .body(Vec::new())
            .unwrap();
        assert_eq!(
            handle_asset_request(&req_forbidden).status(),
            StatusCode::FORBIDDEN
        );

        // Missing media file -> 404
        let req_missing = Request::builder()
            .method(Method::GET)
            .uri("http://asset.localhost/C:/nonexistent_neuroclip_video_12345.mp4")
            .body(Vec::new())
            .unwrap();
        assert_eq!(
            handle_asset_request(&req_missing).status(),
            StatusCode::NOT_FOUND
        );

        let _ = std::fs::remove_dir_all(file_path.parent().unwrap());
    }
}
