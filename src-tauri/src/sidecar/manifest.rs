//! Sidecar manifest parsing, GitHub release contracts, and metadata storage.
//!
//! Complies with ADR-007, AC-29.1, UT-29.1, and `docs/CONVENTIONS.md` (zero-panic invariant).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, warn};

use crate::error::{AppError, SidecarError, StorageError};

/// Managed sidecar CLI tools supported by NeuroClip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SidecarTool {
    /// yt-dlp media extraction sidecar.
    YtDlp,
    /// FFmpeg media processing sidecar.
    Ffmpeg,
}

impl SidecarTool {
    /// Return the canonical identifier string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::YtDlp => "yt-dlp",
            Self::Ffmpeg => "ffmpeg",
        }
    }

    /// Return the platform-specific executable filename.
    #[must_use]
    pub const fn exe_name(self) -> &'static str {
        match self {
            #[cfg(windows)]
            Self::YtDlp => "yt-dlp.exe",
            #[cfg(not(windows))]
            Self::YtDlp => "yt-dlp",
            #[cfg(windows)]
            Self::Ffmpeg => "ffmpeg.exe",
            #[cfg(not(windows))]
            Self::Ffmpeg => "ffmpeg",
        }
    }

    /// Return the environment variable name used for explicit binary overrides.
    #[must_use]
    pub const fn env_var(self) -> &'static str {
        match self {
            Self::YtDlp => "NEUROCLIP_YTDLP_PATH",
            Self::Ffmpeg => "NEUROCLIP_FFMPEG_PATH",
        }
    }

    /// Return the GitHub repository owner and name `(owner, repo)`.
    #[must_use]
    pub const fn github_repo(self) -> (&'static str, &'static str) {
        match self {
            Self::YtDlp => ("yt-dlp", "yt-dlp"),
            Self::Ffmpeg => ("GyanD", "codexffmpeg"),
        }
    }

    /// Return the expected checksum asset filename in GitHub releases.
    #[must_use]
    pub const fn checksum_asset_name(self) -> &'static str {
        match self {
            Self::YtDlp => "SHA2-256SUMS",
            Self::Ffmpeg => "checksums.txt",
        }
    }

    /// Parse a tool name string loosely into [`SidecarTool`].
    #[must_use]
    pub fn from_str_loose(s: &str) -> Option<Self> {
        let normalized = s.trim().to_ascii_lowercase().replace('_', "-");
        match normalized.as_str() {
            "yt-dlp" | "ytdlp" => Some(Self::YtDlp),
            "ffmpeg" | "ffprobe" => Some(Self::Ffmpeg),
            _ => None,
        }
    }
}

/// GitHub release asset metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitHubAsset {
    /// Asset filename (e.g. `yt-dlp.exe`, `SHA2-256SUMS`).
    pub name: String,
    /// Direct download URL for the asset.
    pub browser_download_url: String,
    /// Asset size in bytes.
    #[serde(default)]
    pub size: u64,
    /// MIME content type if reported.
    #[serde(default)]
    pub content_type: Option<String>,
}

/// GitHub release payload schema returned by `/repos/{owner}/{repo}/releases/latest`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitHubRelease {
    /// Release tag (e.g. `"2026.11.23"`).
    pub tag_name: String,
    /// Release title/name if available.
    #[serde(default)]
    pub name: Option<String>,
    /// ISO 8601 publication timestamp.
    #[serde(default)]
    pub published_at: Option<String>,
    /// Web URL to release notes on GitHub.
    #[serde(default)]
    pub html_url: Option<String>,
    /// Release changelog / markdown body.
    #[serde(default)]
    pub body: Option<String>,
    /// Release assets list.
    #[serde(default)]
    pub assets: Vec<GitHubAsset>,
}

impl GitHubRelease {
    /// Find an asset by exact name.
    #[must_use]
    pub fn find_asset(&self, name: &str) -> Option<&GitHubAsset> {
        self.assets.iter().find(|a| a.name == name)
    }

    /// Locate the primary binary asset for this platform and tool.
    #[must_use]
    pub fn find_binary_asset(&self, tool: SidecarTool) -> Option<&GitHubAsset> {
        match tool {
            SidecarTool::YtDlp => {
                let target_name = tool.exe_name();
                if let Some(asset) = self.find_asset(target_name) {
                    return Some(asset);
                }
                // Fallback: look case-insensitively
                self.assets
                    .iter()
                    .find(|a| a.name.eq_ignore_ascii_case(target_name))
            }
            SidecarTool::Ffmpeg => {
                let target_name = tool.exe_name();
                self.assets
                    .iter()
                    .find(|a| a.name.to_ascii_lowercase().contains("ffmpeg"))
                    .or_else(|| self.find_asset(target_name))
            }
        }
    }

    /// Locate the checksum asset for the tool (e.g. `SHA2-256SUMS`).
    #[must_use]
    pub fn find_checksum_asset(&self, tool: SidecarTool) -> Option<&GitHubAsset> {
        let expected_name = tool.checksum_asset_name();
        self.find_asset(expected_name).or_else(|| {
            self.assets.iter().find(|a| {
                a.name.eq_ignore_ascii_case(expected_name)
                    || a.name.to_ascii_uppercase().contains("SHA2-256SUMS")
                    || a.name.to_ascii_uppercase().contains("SHA256SUMS")
            })
        })
    }
}

/// Parse the raw text of a `SHA2-256SUMS` file into a map of filename to lowercase hex SHA-256 digest.
///
/// Supported line formats:
/// - `<64_hex_chars>  <filename>` (2 spaces, text mode)
/// - `<64_hex_chars> *<filename>` (1 space + asterisk, binary mode)
/// - `<64_hex_chars> <filename>` (1 space)
///
/// Blank lines and lines starting with `#` are ignored.
#[must_use]
pub fn parse_sha256sums(content: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Split by whitespace
        let mut parts = trimmed.split_whitespace();
        let Some(hash) = parts.next() else {
            continue;
        };
        let Some(file_token) = parts.next() else {
            continue;
        };

        // SHA-256 hash must be exactly 64 hexadecimal characters
        if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }

        // Strip leading binary indicator asterisk `*`
        let clean_filename = file_token.strip_prefix('*').unwrap_or(file_token).trim();
        if !clean_filename.is_empty() {
            let lower_hash = hash.to_ascii_lowercase();
            map.insert(clean_filename.to_string(), lower_hash.clone());
            // Also store lowercased key for case-insensitive lookup
            map.insert(clean_filename.to_ascii_lowercase(), lower_hash);
        }
    }

    map
}

/// Resolve expected SHA-256 hex digest for an asset from `SHA2-256SUMS` content.
///
/// # Errors
/// Returns [`AppError::Sidecar`] with [`SidecarError::UpdateFailed`] if the asset
/// is not found or has an invalid checksum in the manifest.
pub fn resolve_asset_checksum(
    checksum_content: &str,
    asset_name: &str,
) -> Result<String, AppError> {
    let table = parse_sha256sums(checksum_content);

    if let Some(hash) = table.get(asset_name) {
        return Ok(hash.clone());
    }

    let lower_name = asset_name.to_ascii_lowercase();
    if let Some(hash) = table.get(&lower_name) {
        return Ok(hash.clone());
    }

    Err(AppError::Sidecar(SidecarError::UpdateFailed {
        binary: asset_name.to_string(),
        reason: format!(
            "Không tìm thấy mã băm SHA-256 cho '{asset_name}' trong tập tin kiểm tra tính toàn vẹn (SHA2-256SUMS)"
        ),
    }))
}

/// Local manifest tracking managed sidecars installed on the machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SidecarLocalManifest {
    /// Canonical tool name (`"yt-dlp"`, `"ffmpeg"`).
    pub tool_name: String,
    /// Currently installed version string (e.g. `"2026.11.23"`).
    pub installed_version: Option<String>,
    /// Full path to the installed binary.
    pub installed_path: Option<String>,
    /// Computed SHA-256 hex digest of the installed binary.
    pub sha256_hash: Option<String>,
    /// ISO 8601 timestamp of last update check.
    pub last_checked_at: Option<String>,
    /// ISO 8601 timestamp of last successful update.
    pub last_updated_at: Option<String>,
}

/// Load the local sidecar manifest from disk if it exists.
///
/// # Errors
/// Returns [`AppError::Storage`] if file reading fails.
pub fn load_local_manifest(manifest_path: &Path) -> Result<Option<SidecarLocalManifest>, AppError> {
    if !manifest_path.is_file() {
        return Ok(None);
    }

    let raw = std::fs::read_to_string(manifest_path).map_err(|e| {
        AppError::Storage(StorageError::CacheCleanupFailed(format!(
            "Không thể đọc manifest công cụ phụ trợ tại {}: {e}",
            manifest_path.display()
        )))
    })?;

    let parsed: SidecarLocalManifest = serde_json::from_str(&raw).map_err(|e| {
        warn!(path = %manifest_path.display(), error = %e, "Corrupted sidecar manifest, ignoring");
        AppError::Storage(StorageError::CacheCleanupFailed(format!(
            "Dữ liệu manifest không hợp lệ: {e}"
        )))
    })?;

    Ok(Some(parsed))
}

/// Atomically persist the local sidecar manifest to disk.
///
/// # Errors
/// Returns [`AppError::Storage`] if writing or directory creation fails.
pub fn save_local_manifest(
    manifest_path: &Path,
    manifest: &SidecarLocalManifest,
) -> Result<(), AppError> {
    let parent = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    if !parent.exists() {
        std::fs::create_dir_all(parent).map_err(|e| {
            AppError::Storage(StorageError::DirectoryCreationFailed {
                path: parent.to_string_lossy().into_owned(),
                details: e.to_string(),
            })
        })?;
    }

    let json_bytes = serde_json::to_vec_pretty(manifest).map_err(|e| {
        AppError::Storage(StorageError::CacheCleanupFailed(format!(
            "Không thể tuần tự hóa sidecar manifest: {e}"
        )))
    })?;

    let tmp_path = manifest_path.with_extension(format!("tmp.{}", std::process::id()));
    std::fs::write(&tmp_path, json_bytes).map_err(|e| {
        AppError::Storage(StorageError::CacheCleanupFailed(format!(
            "Không thể ghi tập tin tạm manifest tại {}: {e}",
            tmp_path.display()
        )))
    })?;

    if let Err(e) = std::fs::rename(&tmp_path, manifest_path) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(AppError::Storage(StorageError::CacheCleanupFailed(
            format!("Không thể hoán đổi tập tin manifest: {e}"),
        )));
    }

    debug!(path = %manifest_path.display(), "Sidecar manifest saved successfully");
    Ok(())
}

/// Compute the default managed sidecar binary directory:
/// `%LOCALAPPDATA%/NeuroClip/bin` (or `%APPDATA%/NeuroClip/bin` fallback).
#[must_use]
pub fn get_default_sidecar_dir() -> PathBuf {
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let trimmed = local_app_data.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed).join("NeuroClip").join("bin");
        }
    }

    if let Ok(app_data) = std::env::var("APPDATA") {
        let trimmed = app_data.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed).join("NeuroClip").join("bin");
        }
    }

    std::env::temp_dir().join("NeuroClip").join("bin")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_sidecar_tool_properties() {
        assert_eq!(SidecarTool::YtDlp.as_str(), "yt-dlp");
        assert_eq!(SidecarTool::Ffmpeg.as_str(), "ffmpeg");
        assert_eq!(SidecarTool::YtDlp.env_var(), "NEUROCLIP_YTDLP_PATH");
        assert_eq!(SidecarTool::Ffmpeg.env_var(), "NEUROCLIP_FFMPEG_PATH");

        assert_eq!(
            SidecarTool::from_str_loose("yt-dlp"),
            Some(SidecarTool::YtDlp)
        );
        assert_eq!(
            SidecarTool::from_str_loose("ytdlp"),
            Some(SidecarTool::YtDlp)
        );
        assert_eq!(
            SidecarTool::from_str_loose("ffmpeg"),
            Some(SidecarTool::Ffmpeg)
        );
        assert_eq!(SidecarTool::from_str_loose("unknown"), None);
    }

    #[test]
    fn test_parse_sha256sums_valid() {
        let content = r"
# Release SHA2-256 checksums
622ed48023cac1daa52a3bf3a6ac38f03514de214b5f0fa069389fd0f6d17e5c  yt-dlp.exe
a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90 *yt-dlp_linux
badhash_too_short filename.txt
";
        let map = parse_sha256sums(content);
        assert_eq!(
            map.get("yt-dlp.exe"),
            Some(&"622ed48023cac1daa52a3bf3a6ac38f03514de214b5f0fa069389fd0f6d17e5c".to_string())
        );
        assert_eq!(
            map.get("yt-dlp_linux"),
            Some(&"a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90".to_string())
        );
        assert_eq!(map.get("filename.txt"), None);
    }

    #[test]
    fn test_resolve_asset_checksum() {
        let content =
            "622ed48023cac1daa52a3bf3a6ac38f03514de214b5f0fa069389fd0f6d17e5c  yt-dlp.exe\n";
        let res = resolve_asset_checksum(content, "yt-dlp.exe").unwrap();
        assert_eq!(
            res,
            "622ed48023cac1daa52a3bf3a6ac38f03514de214b5f0fa069389fd0f6d17e5c"
        );

        let err = resolve_asset_checksum(content, "nonexistent.exe");
        assert!(err.is_err());
    }

    #[test]
    fn test_manifest_load_and_save_atomic() {
        let temp_dir = std::env::temp_dir().join(format!("test_manifest_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        let manifest_path = temp_dir.join("sidecar_manifest.json");

        assert_eq!(load_local_manifest(&manifest_path).unwrap(), None);

        let manifest = SidecarLocalManifest {
            tool_name: "yt-dlp".to_string(),
            installed_version: Some("2026.11.23".to_string()),
            installed_path: Some(temp_dir.join("yt-dlp.exe").to_string_lossy().into_owned()),
            sha256_hash: Some(
                "622ed48023cac1daa52a3bf3a6ac38f03514de214b5f0fa069389fd0f6d17e5c".to_string(),
            ),
            last_checked_at: Some("2026-11-24T05:00:00Z".to_string()),
            last_updated_at: Some("2026-11-24T05:00:00Z".to_string()),
        };

        save_local_manifest(&manifest_path, &manifest).unwrap();
        let loaded = load_local_manifest(&manifest_path).unwrap().unwrap();
        assert_eq!(loaded, manifest);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
