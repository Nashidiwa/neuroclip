//! Twitch GraphQL API client for cursor-paginated chat replay extraction.
//!
//! Complies with docs/CONVENTIONS.md (zero-panic invariant, Result<T, AppError>)
//! and docs/ARCHITECTURE.md §4.2, §5.3, §6 (ERR-05).

use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, IngestError, StorageError};
use crate::ingestion::streamer::assign_child_to_job_guard;

pub const TWITCH_GQL_ENDPOINT: &str = "https://gql.twitch.tv/gql";
/// Client-ID that permits unauthenticated cursor pagination for Twitch VOD comments.
pub const TWITCH_CLIENT_ID: &str = "kd1unb4b3q4t58fwlpcbzcbnm76a8fp";
pub const TWITCH_COMMENTS_SHA256: &str =
    "b70a3591ff0f4e0313d126c6a1502d79a1c02baebb288227c582044aa76adf6a";
pub const TWITCH_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

const MAX_SEEN_COMMENT_IDS: usize = 2_048;
const MAX_OFFSET_STEP_EMPTY_PAGES: u8 = 2;

/// Progress payload emitted during chat extraction stages.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatProgressPayload {
    pub task_id: String,
    pub stage: String,
    pub percent: f64,
    pub messages_extracted: u64,
    pub current_offset_sec: f64,
}

#[derive(Serialize)]
struct GqlVariables<'a> {
    #[serde(rename = "videoID")]
    video_id: &'a str,
    #[serde(
        skip_serializing_if = "Option::is_none",
        rename = "contentOffsetSeconds"
    )]
    content_offset_seconds: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<&'a str>,
}

#[derive(Serialize)]
struct GqlPersistedQuery<'a> {
    version: u32,
    #[serde(rename = "sha256Hash")]
    sha256_hash: &'a str,
}

#[derive(Serialize)]
struct GqlExtensions<'a> {
    #[serde(rename = "persistedQuery")]
    persisted_query: GqlPersistedQuery<'a>,
}

#[derive(Serialize)]
struct GqlRequestPayload<'a> {
    #[serde(rename = "operationName")]
    operation_name: &'a str,
    variables: GqlVariables<'a>,
    extensions: GqlExtensions<'a>,
}

/// Extract the clean Twitch VOD ID from a URL, `vod_<digits>`, or `v<digits>` without
/// truncating synthetic `vod_*` prefixes into `od_*`.
#[must_use]
pub fn extract_twitch_vod_id(vod_id: &str) -> &str {
    let trimmed = vod_id.trim();

    let after_scheme = trimmed
        .get(..8)
        .filter(|head| head.eq_ignore_ascii_case("https://"))
        .map(|_| &trimmed[8..])
        .or_else(|| {
            trimmed
                .get(..7)
                .filter(|head| head.eq_ignore_ascii_case("http://"))
                .map(|_| &trimmed[7..])
        });

    let without_url = if let Some(host_and_rest) = after_scheme {
        if let Some((host, rest)) = host_and_rest.split_once('/') {
            let is_twitch_host = host.is_ascii()
                && (host.eq_ignore_ascii_case("twitch.tv")
                    || host
                        .get(host.len().saturating_sub(".twitch.tv".len())..)
                        .is_some_and(|tail| tail.eq_ignore_ascii_case(".twitch.tv")));
            if is_twitch_host {
                let path_only = rest.split(['?', '#']).next().unwrap_or(rest);
                let mut segments = path_only.split('/');
                let mut extracted = None;
                while let Some(seg) = segments.next() {
                    if seg.eq_ignore_ascii_case("videos") || seg.eq_ignore_ascii_case("video") {
                        extracted = Some(segments.next().unwrap_or(""));
                        break;
                    }
                }
                extracted.unwrap_or("")
            } else {
                trimmed
            }
        } else {
            trimmed
        }
    } else {
        trimmed
    };

    let before_query = without_url
        .split(['?', '#', '/'])
        .next()
        .unwrap_or(without_url);

    if let Some(rest) = before_query.strip_prefix("vod_") {
        if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()) {
            return rest;
        }
        return before_query;
    }

    if let Some(rest) = before_query.strip_prefix('v') {
        if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()) {
            return rest;
        }
    }

    before_query
}

fn build_twitch_http_client() -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent(TWITCH_USER_AGENT)
        .build()
}

async fn execute_twitch_gql_reqwest(
    client: &reqwest::Client,
    json_body: &str,
    cancel_token: &CancellationToken,
) -> Result<Result<Option<Vec<u8>>, String>, AppError> {
    const RETRY_BACKOFF_MS: [u64; 2] = [300, 800];

    for attempt in 0..=RETRY_BACKOFF_MS.len() {
        if cancel_token.is_cancelled() {
            return Err(AppError::TaskCancelled);
        }

        let req = client
            .post(TWITCH_GQL_ENDPOINT)
            .header("Client-ID", TWITCH_CLIENT_ID)
            .header("Content-Type", "application/json")
            .body(json_body.to_string());

        let send_res = tokio::select! {
            biased;
            () = cancel_token.cancelled() => {
                return Err(AppError::TaskCancelled);
            }
            res = req.send() => res,
        };

        match send_res {
            Ok(resp) => {
                let status = resp.status();
                let status_code = status.as_u16();
                let is_transient_status = status_code == 429 || status.is_server_error();

                let body_res = tokio::select! {
                    biased;
                    () = cancel_token.cancelled() => {
                        return Err(AppError::TaskCancelled);
                    }
                    res = resp.bytes() => res,
                };

                match body_res {
                    Ok(bytes) => {
                        if status.is_success() {
                            return Ok(Ok(Some(bytes.to_vec())));
                        }
                        let snippet = String::from_utf8_lossy(&bytes);
                        let preview: String = snippet.chars().take(200).collect();
                        tracing::warn!(
                            status = status_code,
                            attempt = attempt + 1,
                            body = %preview,
                            "Twitch GQL HTTP error response"
                        );
                        if is_transient_status {
                            if let Some(&delay_ms) = RETRY_BACKOFF_MS.get(attempt) {
                                tokio::select! {
                                    biased;
                                    () = cancel_token.cancelled() => {
                                        return Err(AppError::TaskCancelled);
                                    }
                                    () = tokio::time::sleep(Duration::from_millis(delay_ms)) => {}
                                }
                                continue;
                            }
                        }
                        return Ok(Ok(None));
                    }
                    Err(e) => {
                        tracing::warn!(
                            attempt = attempt + 1,
                            error = %e,
                            "Twitch GQL response body read failed"
                        );
                        if let Some(&delay_ms) = RETRY_BACKOFF_MS.get(attempt) {
                            tokio::select! {
                                biased;
                                () = cancel_token.cancelled() => {
                                    return Err(AppError::TaskCancelled);
                                }
                                () = tokio::time::sleep(Duration::from_millis(delay_ms)) => {}
                            }
                            continue;
                        }
                        return Ok(Err(e.to_string()));
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    attempt = attempt + 1,
                    error = %e,
                    "Twitch GQL HTTP request failed"
                );
                if let Some(&delay_ms) = RETRY_BACKOFF_MS.get(attempt) {
                    tokio::select! {
                        biased;
                        () = cancel_token.cancelled() => {
                            return Err(AppError::TaskCancelled);
                        }
                        () = tokio::time::sleep(Duration::from_millis(delay_ms)) => {}
                    }
                    continue;
                }
                return Ok(Err(e.to_string()));
            }
        }
    }

    Ok(Ok(None))
}

async fn execute_twitch_gql_curl(
    json_body: String,
    cancel_token: &CancellationToken,
) -> Result<Option<Vec<u8>>, AppError> {
    // Execute request via Windows-native curl.exe subprocess
    let mut cmd = Command::new("curl.exe");
    cmd.args([
        "-s",
        "--max-time",
        "15",
        "-X",
        "POST",
        "-H",
        &format!("Client-ID: {TWITCH_CLIENT_ID}"),
        "-H",
        "Content-Type: application/json",
        "-H",
        &format!("User-Agent: {TWITCH_USER_AGENT}"),
        "-d",
        &json_body,
        TWITCH_GQL_ENDPOINT,
    ]);
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let child = cmd
        .spawn()
        .map_err(|e| AppError::Internal(format!("Failed to spawn curl for Twitch GQL: {e}")))?;
    assign_child_to_job_guard(&child);

    let output = tokio::select! {
        biased;
        () = cancel_token.cancelled() => {
            return Err(AppError::TaskCancelled);
        }
        res = child.wait_with_output() => {
            res.map_err(|e| AppError::Internal(format!("Curl process execution failed: {e}")))?
        }
    };

    if !output.status.success() {
        let stderr_preview: String = String::from_utf8_lossy(&output.stderr)
            .chars()
            .take(200)
            .collect();
        tracing::warn!(
            exit_code = output.status.code(),
            stderr = %stderr_preview,
            "curl.exe fallback for Twitch GQL exited with non-zero status"
        );
        return Ok(None);
    }

    Ok(Some(output.stdout))
}

async fn execute_twitch_gql_request(
    client: Option<&reqwest::Client>,
    json_body: String,
    cancel_token: &CancellationToken,
) -> Result<Option<Vec<u8>>, AppError> {
    if let Some(http_client) = client {
        match execute_twitch_gql_reqwest(http_client, &json_body, cancel_token).await? {
            Ok(Some(bytes)) => return Ok(Some(bytes)),
            Ok(None) => {
                tracing::warn!("Twitch GQL reqwest returned non-2xx; falling back to curl.exe");
            }
            Err(transport_err) => {
                tracing::warn!(
                    error = %transport_err,
                    "Twitch GQL reqwest transport failed; falling back to curl.exe"
                );
            }
        }
    }

    execute_twitch_gql_curl(json_body, cancel_token).await
}

/// Download Twitch VOD chat replay using Twitch GQL cursor pagination.
///
/// If chat is missing, deleted, or expired, returns `ERR_INGEST_CHAT_UNAVAILABLE`
/// within <= 200ms to allow soft fallback (ERR-05).
pub async fn fetch_twitch_vod_chat_gql<F>(
    vod_id: &str,
    output_ndjson_path: &Path,
    cancel_token: CancellationToken,
    progress_callback: F,
) -> Result<u64, AppError>
where
    F: Fn(ChatProgressPayload) + Send + Sync + 'static,
{
    fetch_twitch_vod_chat_gql_with_task_id(
        vod_id,
        vod_id,
        output_ndjson_path,
        cancel_token,
        progress_callback,
    )
    .await
}

/// Download Twitch VOD chat replay querying GQL with `vod_id` while emitting progress events
/// keyed by `task_id` (so callers with synthetic local `vod_id`s keep correlation).
///
/// # Errors
/// Returns `AppError::Ingestion(IngestError::ChatReplayUnavailable)` if replay comments are missing,
/// `AppError::TaskCancelled` if cancelled, or `AppError::Storage` / `AppError::Internal` on I/O errors.
pub async fn fetch_twitch_vod_chat_gql_with_task_id<F>(
    task_id: &str,
    vod_id: &str,
    output_ndjson_path: &Path,
    cancel_token: CancellationToken,
    progress_callback: F,
) -> Result<u64, AppError>
where
    F: Fn(ChatProgressPayload) + Send + Sync + 'static,
{
    let http_client = match build_twitch_http_client() {
        Ok(client) => Some(client),
        Err(e) => {
            tracing::warn!(error = %e, "Failed to build Twitch HTTP client; falling back to curl.exe");
            None
        }
    };
    let token_for_fetch = cancel_token.clone();

    fetch_twitch_vod_chat_gql_for_task_with_fetcher(
        task_id,
        vod_id,
        output_ndjson_path,
        &cancel_token,
        progress_callback,
        move |json_body| {
            let client = http_client.clone();
            let token = token_for_fetch.clone();
            async move { execute_twitch_gql_request(client.as_ref(), json_body, &token).await }
        },
    )
    .await
}

fn log_gql_errors(first_elem: &serde_json::Value, page_index: u64, offset_stepping: bool) {
    if let Some(errors) = first_elem
        .get("errors")
        .and_then(serde_json::Value::as_array)
    {
        if !errors.is_empty() {
            tracing::warn!(
                page_index,
                offset_stepping,
                errors = %serde_json::Value::Array(errors.clone()),
                "Twitch GQL returned errors array"
            );
        }
    }
}

fn is_integrity_or_cursor_error(first_elem: &serde_json::Value) -> bool {
    let Some(errors) = first_elem
        .get("errors")
        .and_then(serde_json::Value::as_array)
    else {
        return false;
    };
    if errors.is_empty() {
        return false;
    }

    errors.iter().any(|err| {
        let code_matches = err
            .pointer("/extensions/code")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|code| code.eq_ignore_ascii_case("IntegrityCheckFailed"));
        let msg_matches = err
            .get("message")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|msg| msg.to_ascii_lowercase().contains("integrity"));
        code_matches || msg_matches
    }) || first_elem
        .pointer("/data/video/comments")
        .is_none_or(serde_json::Value::is_null)
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn next_offset_seconds(current_offset: f64, last_requested_offset: u64) -> u64 {
    let floored = if current_offset.is_finite() && current_offset > 0.0 {
        let capped = current_offset.floor().min(f64::from(u32::MAX) * 100.0);
        capped as u64
    } else {
        0
    };
    floored
        .saturating_add(1)
        .max(last_requested_offset.saturating_add(1))
}

fn mark_comment_id_new(
    node: &serde_json::Value,
    seen_ids: &mut HashSet<String>,
    seen_order: &mut VecDeque<String>,
) -> bool {
    let Some(comment_id) = node.get("id").and_then(serde_json::Value::as_str) else {
        return true;
    };
    if seen_ids.contains(comment_id) {
        return false;
    }
    let id_owned = comment_id.to_string();
    if seen_order.len() >= MAX_SEEN_COMMENT_IDS {
        if let Some(evicted) = seen_order.pop_front() {
            seen_ids.remove(&evicted);
        }
    }
    seen_order.push_back(id_owned.clone());
    seen_ids.insert(id_owned);
    true
}

#[allow(dead_code)]
async fn fetch_twitch_vod_chat_gql_with_fetcher<F, Fetch, FetchFut>(
    vod_id: &str,
    output_ndjson_path: &Path,
    cancel_token: &CancellationToken,
    progress_callback: F,
    fetch_page: Fetch,
) -> Result<u64, AppError>
where
    F: Fn(ChatProgressPayload) + Send + Sync,
    Fetch: FnMut(String) -> FetchFut,
    FetchFut: std::future::Future<Output = Result<Option<Vec<u8>>, AppError>>,
{
    fetch_twitch_vod_chat_gql_for_task_with_fetcher(
        vod_id,
        vod_id,
        output_ndjson_path,
        cancel_token,
        progress_callback,
        fetch_page,
    )
    .await
}

#[allow(clippy::too_many_lines)]
async fn fetch_twitch_vod_chat_gql_for_task_with_fetcher<F, Fetch, FetchFut>(
    task_id: &str,
    vod_id: &str,
    output_ndjson_path: &Path,
    cancel_token: &CancellationToken,
    progress_callback: F,
    mut fetch_page: Fetch,
) -> Result<u64, AppError>
where
    F: Fn(ChatProgressPayload) + Send + Sync,
    Fetch: FnMut(String) -> FetchFut,
    FetchFut: std::future::Future<Output = Result<Option<Vec<u8>>, AppError>>,
{
    // Fast mock check for unit / integration tests
    if std::env::var("NEUROCLIP_MOCK_CHAT_UNAVAILABLE").is_ok() {
        return Err(AppError::Ingestion(IngestError::ChatReplayUnavailable));
    }

    if let Ok(mock_file_path) = std::env::var("NEUROCLIP_MOCK_TWITCH_CHAT_FILE") {
        if let Ok(content) = std::fs::read_to_string(&mock_file_path) {
            std::fs::write(output_ndjson_path, content.as_bytes()).map_err(|e| {
                AppError::Storage(StorageError::DirectoryCreationFailed {
                    path: output_ndjson_path.display().to_string(),
                    details: e.to_string(),
                })
            })?;
            let line_count = content.lines().filter(|l| !l.trim().is_empty()).count() as u64;
            progress_callback(ChatProgressPayload {
                task_id: task_id.to_string(),
                stage: "completed".to_string(),
                percent: 100.0,
                messages_extracted: line_count,
                current_offset_sec: 0.0,
            });
            return Ok(line_count);
        }
    }

    // Clean vod_id (strip prefix if user entered full URL, vod_<digits>, or v<digits>)
    let clean_vod_id = extract_twitch_vod_id(vod_id);
    if clean_vod_id.is_empty() {
        return Err(AppError::Ingestion(IngestError::InvalidUrl(
            "Twitch VOD ID cannot be empty".to_string(),
        )));
    }

    let out_file = File::create(output_ndjson_path).map_err(|e| {
        AppError::Storage(StorageError::DirectoryCreationFailed {
            path: output_ndjson_path.display().to_string(),
            details: e.to_string(),
        })
    })?;
    let mut writer = BufWriter::with_capacity(64 * 1024, out_file);

    let mut cursor: Option<String> = None;
    let mut use_offset_stepping = false;
    let mut next_offset: u64 = 0;
    let mut last_requested_offset: u64 = 0;
    let mut empty_offset_pages: u8 = 0;
    let mut seen_ids: HashSet<String> = HashSet::with_capacity(MAX_SEEN_COMMENT_IDS);
    let mut seen_order: VecDeque<String> = VecDeque::with_capacity(MAX_SEEN_COMMENT_IDS);

    let mut messages_count: u64 = 0;
    let mut current_offset: f64 = 0.0;
    let mut has_next_page = true;
    let mut page_index: u64 = 0;

    while has_next_page {
        if cancel_token.is_cancelled() {
            return Err(AppError::TaskCancelled);
        }

        let content_offset_arg = if use_offset_stepping {
            last_requested_offset = next_offset;
            Some(next_offset)
        } else if cursor.is_none() {
            last_requested_offset = 0;
            Some(0)
        } else {
            None
        };

        let payload = vec![GqlRequestPayload {
            operation_name: "VideoCommentsByOffsetOrCursor",
            variables: GqlVariables {
                video_id: clean_vod_id,
                content_offset_seconds: content_offset_arg,
                cursor: if use_offset_stepping {
                    None
                } else {
                    cursor.as_deref()
                },
            },
            extensions: GqlExtensions {
                persisted_query: GqlPersistedQuery {
                    version: 1,
                    sha256_hash: TWITCH_COMMENTS_SHA256,
                },
            },
        }];

        let json_body = serde_json::to_string(&payload)
            .map_err(|e| AppError::Internal(format!("Failed to serialize GQL payload: {e}")))?;

        let Some(stdout_bytes) = fetch_page(json_body).await? else {
            if page_index == 0 {
                return Err(AppError::Ingestion(IngestError::ChatReplayUnavailable));
            }
            break;
        };

        let response_str = String::from_utf8_lossy(&stdout_bytes);
        if response_str.trim().is_empty() {
            if page_index == 0 {
                return Err(AppError::Ingestion(IngestError::ChatReplayUnavailable));
            }
            break;
        }

        let Ok(resp_json) = serde_json::from_str::<serde_json::Value>(&response_str) else {
            tracing::warn!(page_index, "Twitch GQL returned invalid JSON payload");
            if page_index == 0 {
                return Err(AppError::Ingestion(IngestError::ChatReplayUnavailable));
            }
            break;
        };

        // Check if root array element has video comments
        let first_elem = resp_json
            .as_array()
            .and_then(|arr| arr.first())
            .unwrap_or(&resp_json);

        log_gql_errors(first_elem, page_index, use_offset_stepping);

        let video = first_elem.pointer("/data/video");
        let comments = first_elem.pointer("/data/video/comments");
        let comments_missing = video.is_none()
            || video == Some(&serde_json::Value::Null)
            || comments.is_none()
            || comments == Some(&serde_json::Value::Null);

        if comments_missing {
            if !use_offset_stepping && page_index > 0 && is_integrity_or_cursor_error(first_elem) {
                next_offset = next_offset_seconds(current_offset, last_requested_offset);
                tracing::warn!(
                    page_index,
                    next_offset,
                    "Twitch GQL cursor blocked by integrity check; switching to contentOffsetSeconds stepping fallback"
                );
                use_offset_stepping = true;
                cursor = None;
                continue;
            }
            if page_index == 0 {
                return Err(AppError::Ingestion(IngestError::ChatReplayUnavailable));
            }
            break;
        }

        let edges = comments
            .and_then(|c| c.get("edges"))
            .and_then(|e| e.as_array());

        let Some(edge_array) = edges else {
            if page_index == 0 {
                return Err(AppError::Ingestion(IngestError::ChatReplayUnavailable));
            }
            break;
        };

        if edge_array.is_empty() {
            break;
        }

        let mut written_this_page: u64 = 0;
        for edge in edge_array {
            if let Some(node) = edge.get("node") {
                if !mark_comment_id_new(node, &mut seen_ids, &mut seen_order) {
                    continue;
                }
                if let Ok(line) = serde_json::to_string(node) {
                    let _ = writer.write_all(line.as_bytes());
                    let _ = writer.write_all(b"\n");
                    messages_count = messages_count.saturating_add(1);
                    written_this_page = written_this_page.saturating_add(1);

                    if let Some(offset) = node
                        .get("contentOffsetSeconds")
                        .and_then(serde_json::Value::as_f64)
                    {
                        current_offset = offset;
                    }
                }
            }
        }

        // Check pagination cursor
        let page_info = comments.and_then(|c| c.get("pageInfo"));
        has_next_page = page_info
            .and_then(|p| p.get("hasNextPage"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);

        cursor = if use_offset_stepping {
            next_offset = next_offset_seconds(current_offset, last_requested_offset);
            if written_this_page == 0 {
                empty_offset_pages = empty_offset_pages.saturating_add(1);
                if empty_offset_pages >= MAX_OFFSET_STEP_EMPTY_PAGES {
                    break;
                }
            } else {
                empty_offset_pages = 0;
            }
            None
        } else {
            edge_array
                .last()
                .and_then(|last_edge| last_edge.get("cursor"))
                .and_then(|c| c.as_str())
                .map(str::to_string)
        };

        page_index = page_index.saturating_add(1);

        progress_callback(ChatProgressPayload {
            task_id: task_id.to_string(),
            stage: "extracting_twitch_chat".to_string(),
            percent: 0.0, // unknown total length during stream
            messages_extracted: messages_count,
            current_offset_sec: current_offset,
        });

        if !use_offset_stepping && cursor.is_none() {
            break;
        }
    }

    writer
        .flush()
        .map_err(|e| AppError::Internal(format!("Failed to flush output NDJSON: {e}")))?;

    if messages_count == 0 {
        return Err(AppError::Ingestion(IngestError::ChatReplayUnavailable));
    }

    progress_callback(ChatProgressPayload {
        task_id: task_id.to_string(),
        stage: "completed".to_string(),
        percent: 100.0,
        messages_extracted: messages_count,
        current_offset_sec: current_offset,
    });

    Ok(messages_count)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_twitch_vod_id_preserves_vod_prefix_and_strips_digits() {
        assert_eq!(extract_twitch_vod_id("vod_123456"), "123456");
        assert_eq!(extract_twitch_vod_id("vod_test_123"), "vod_test_123");
        assert_eq!(extract_twitch_vod_id("v987654"), "987654");
        assert_eq!(extract_twitch_vod_id("123456"), "123456");
        assert_eq!(
            extract_twitch_vod_id("https://www.twitch.tv/videos/2198765432?t=1h2m"),
            "2198765432"
        );
        assert_eq!(
            extract_twitch_vod_id("https://twitch.tv/videos/v555666"),
            "555666"
        );
        assert_eq!(
            extract_twitch_vod_id("https://m.twitch.tv/videos/123456"),
            "123456"
        );
        assert_eq!(
            extract_twitch_vod_id("http://m.twitch.tv/videos/v777888?t=5m"),
            "777888"
        );
        assert_eq!(
            extract_twitch_vod_id("HTTP://WWW.TWITCH.TV/videos/123456"),
            "123456"
        );
        assert_eq!(
            extract_twitch_vod_id("HTTPS://M.TWITCH.TV/VIDEOS/987654#chat"),
            "987654"
        );
        assert_eq!(extract_twitch_vod_id("https://www.twitch.tv/videos/"), "");
        assert_eq!(extract_twitch_vod_id("https://m.twitch.tv/videos/"), "");
        assert_eq!(
            extract_twitch_vod_id("https://www.twitch.tv/vedal987/video/2939581544?t=10s"),
            "2939581544"
        );
        assert_eq!(
            extract_twitch_vod_id("https://twitch.tv/vedal987/videos/2939581544"),
            "2939581544"
        );
        assert_eq!(extract_twitch_vod_id("video_custom"), "video_custom");
    }

    #[tokio::test]
    async fn test_twitch_gql_cursor_integrity_error_falls_back_to_offset_stepping_and_deduplicates()
    {
        use crate::core::storage::test_env::{ScopedEnvVar, ENV_LOCK};
        use std::sync::{Arc, Mutex};

        let _env_guard = ENV_LOCK.lock().await;
        let _unavail_clear = ScopedEnvVar::remove("NEUROCLIP_MOCK_CHAT_UNAVAILABLE");
        let _file_clear = ScopedEnvVar::remove("NEUROCLIP_MOCK_TWITCH_CHAT_FILE");

        let temp_out = std::env::temp_dir().join(format!(
            "neuroclip_twitch_offset_fallback_{}.ndjson",
            std::process::id()
        ));
        let cancel = CancellationToken::new();
        let captured_requests: Arc<Mutex<Vec<serde_json::Value>>> =
            Arc::new(Mutex::new(Vec::new()));
        let reqs_clone = Arc::clone(&captured_requests);
        let call_counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let count = fetch_twitch_vod_chat_gql_for_task_with_fetcher(
            "vod_local_alias",
            "2939581544",
            &temp_out,
            &cancel,
            |_| {},
            move |body| {
                let reqs = Arc::clone(&reqs_clone);
                let counter = Arc::clone(&call_counter);
                async move {
                    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
                    reqs.lock().unwrap().push(parsed);
                    let idx = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let response = match idx {
                        0 => r#"[{"data":{"video":{"comments":{"edges":[{"cursor":"cur_1","node":{"id":"m1","contentOffsetSeconds":5.0}},{"cursor":"cur_1b","node":{"id":"m2","contentOffsetSeconds":20.0}}],"pageInfo":{"hasNextPage":true}}}}}]"#,
                        1 => r#"[{"errors":[{"message":"failed integrity check","extensions":{"code":"IntegrityCheckFailed"}}],"data":{"video":{"comments":null}}}]"#,
                        _ => r#"[{"data":{"video":{"comments":{"edges":[{"cursor":"cur_2","node":{"id":"m2","contentOffsetSeconds":20.0}},{"cursor":"cur_3","node":{"id":"m3","contentOffsetSeconds":35.0}}],"pageInfo":{"hasNextPage":false}}}}}]"#,
                    };
                    Ok(Some(response.as_bytes().to_vec()))
                }
            },
        )
        .await
        .expect("Offset stepping fallback after IntegrityCheckFailed must succeed");

        assert_eq!(
            count, 3,
            "Duplicate m2 across overlap boundary must be deduplicated"
        );

        let reqs = captured_requests.lock().unwrap();
        assert_eq!(reqs.len(), 3);
        assert_eq!(reqs[0][0]["variables"]["contentOffsetSeconds"], 0);
        assert_eq!(reqs[1][0]["variables"]["cursor"], "cur_1b");
        assert_eq!(reqs[2][0]["variables"]["contentOffsetSeconds"], 21);
        assert!(reqs[2][0]["variables"].get("cursor").is_none());

        let written = std::fs::read_to_string(&temp_out).unwrap();
        let ids: Vec<String> = written
            .lines()
            .map(|line| {
                serde_json::from_str::<serde_json::Value>(line).unwrap()["id"]
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect();
        assert_eq!(ids, vec!["m1", "m2", "m3"]);
        let _ = std::fs::remove_file(&temp_out);
    }

    #[tokio::test]
    async fn test_fetch_twitch_vod_chat_gql_cancelled_returns_task_cancelled() {
        let temp_out = std::env::temp_dir().join(format!(
            "neuroclip_twitch_cancel_{}.ndjson",
            std::process::id()
        ));
        let token = CancellationToken::new();
        token.cancel();

        let start = std::time::Instant::now();
        let res = fetch_twitch_vod_chat_gql("vod_123456", &temp_out, token, |_| {}).await;
        let _ = std::fs::remove_file(&temp_out);

        assert!(matches!(res, Err(AppError::TaskCancelled)));
        assert!(start.elapsed().as_millis() < 200);
    }

    #[tokio::test]
    async fn test_live_gql_pagination_emits_caller_vod_id_while_querying_clean_id() {
        use crate::core::storage::test_env::{ScopedEnvVar, ENV_LOCK};
        use std::sync::{Arc, Mutex};

        let _env_guard = ENV_LOCK.lock().await;
        let _unavail_clear = ScopedEnvVar::remove("NEUROCLIP_MOCK_CHAT_UNAVAILABLE");
        let _file_clear = ScopedEnvVar::remove("NEUROCLIP_MOCK_TWITCH_CHAT_FILE");

        for (caller_vod_id, expected_clean_id) in [("vod_123456", "123456"), ("v987654", "987654")]
        {
            let temp_out = std::env::temp_dir().join(format!(
                "neuroclip_twitch_live_taskid_{caller_vod_id}_{}.ndjson",
                std::process::id()
            ));
            let cancel = CancellationToken::new();
            let payloads: Arc<Mutex<Vec<ChatProgressPayload>>> = Arc::new(Mutex::new(Vec::new()));
            let payloads_clone = Arc::clone(&payloads);
            let captured_requests: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
            let captured_requests_clone = Arc::clone(&captured_requests);
            let page_counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));

            let count = fetch_twitch_vod_chat_gql_with_fetcher(
                caller_vod_id,
                &temp_out,
                &cancel,
                move |p| {
                    payloads_clone.lock().unwrap().push(p);
                },
                move |body| {
                    let reqs = Arc::clone(&captured_requests_clone);
                    let counter = Arc::clone(&page_counter);
                    async move {
                        reqs.lock().unwrap().push(body);
                        let idx = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        let response = if idx == 0 {
                            r#"[{"data":{"video":{"comments":{"edges":[{"cursor":"cur_page_1","node":{"id":"m1","contentOffsetSeconds":12.5}}],"pageInfo":{"hasNextPage":true}}}}}]"#
                        } else {
                            r#"[{"data":{"video":{"comments":{"edges":[{"cursor":"cur_page_2","node":{"id":"m2","contentOffsetSeconds":45.0}}],"pageInfo":{"hasNextPage":false}}}}}]"#
                        };
                        Ok(Some(response.as_bytes().to_vec()))
                    }
                },
            )
            .await
            .expect("Live GQL pagination mock must succeed");

            assert_eq!(count, 2);

            let requests = captured_requests.lock().unwrap();
            assert_eq!(requests.len(), 2);
            for req_body in requests.iter() {
                let parsed: serde_json::Value = serde_json::from_str(req_body).unwrap();
                assert_eq!(
                    parsed[0]["variables"]["videoID"],
                    serde_json::Value::String(expected_clean_id.to_string()),
                    "Twitch GQL API request must continue sending clean_vod_id"
                );
            }

            let emitted = payloads.lock().unwrap();
            assert_eq!(emitted.len(), 3);
            assert!(
                emitted.iter().all(|p| p.task_id == caller_vod_id),
                "All live chat:progress payloads must carry caller vod_id ({caller_vod_id}), got: {emitted:?}"
            );
            assert_eq!(emitted[0].stage, "extracting_twitch_chat");
            assert_eq!(emitted[1].stage, "extracting_twitch_chat");
            assert_eq!(emitted[2].stage, "completed");
            assert!((emitted[2].percent - 100.0).abs() < 1e-4);

            let written = std::fs::read_to_string(&temp_out).unwrap();
            assert_eq!(written.lines().count(), 2);
            let _ = std::fs::remove_file(&temp_out);
        }
    }
}
