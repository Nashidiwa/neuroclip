//! Managed sidecar binary auto-updater and anti-bot defense coordinator.
//!
//! Complies with ADR-007, ERR-04, AC-29.1 to AC-29.4, and `docs/CONVENTIONS.md`.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::error::{AppError, SidecarError, StorageError};
use crate::ingestion::cookies::{get_rotated_desktop_user_agent, BrowserType};
use crate::sidecar::manifest::{
    get_default_sidecar_dir, load_local_manifest, resolve_asset_checksum, save_local_manifest,
    GitHubRelease, SidecarLocalManifest, SidecarTool,
};
use crate::sidecar::resolve_sidecar_binary;

static NONCE_COUNTER: AtomicU64 = AtomicU64::new(1);

fn next_nonce() -> u64 {
    NONCE_COUNTER.fetch_add(1, Ordering::Relaxed)
}

/// Simulated HTTP response from mock transport (`CONVENTIONS.md` §7.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockHttpResponse {
    /// HTTP status code.
    pub status_code: u16,
    /// Response payload bytes.
    pub body: Vec<u8>,
}

impl MockHttpResponse {
    /// Create a 200 OK mock response with the given body.
    #[must_use]
    pub fn ok(body: impl Into<Vec<u8>>) -> Self {
        Self {
            status_code: 200,
            body: body.into(),
        }
    }

    /// Create a mock response with a specific status code and body.
    #[must_use]
    pub fn status(status_code: u16, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status_code,
            body: body.into(),
        }
    }
}

/// Closure type for hermetic mock transport testing without live network calls.
pub type MockHttpHandler = Arc<dyn Fn(&str) -> Result<MockHttpResponse, AppError> + Send + Sync>;

/// Configuration options for [`SidecarUpdater`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SidecarUpdaterConfig {
    /// Target installation directory (e.g. `%LOCALAPPDATA%/NeuroClip/bin`).
    pub target_dir: Option<PathBuf>,
    /// Custom release metadata endpoint URL (overriding GitHub API).
    pub release_api_url: Option<String>,
    /// Custom User-Agent header for HTTP requests.
    pub user_agent: String,
    /// Cache duration for weekly background update checks in hours (default: 168).
    pub weekly_check_interval_hours: u64,
}

impl Default for SidecarUpdaterConfig {
    fn default() -> Self {
        Self {
            target_dir: None,
            release_api_url: None,
            user_agent: "NeuroClip-AutoUpdater/1.0".to_string(),
            weekly_check_interval_hours: 168,
        }
    }
}

/// Structured summary returned when querying for sidecar updates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseCheckInfo {
    /// Tool queried (`yt-dlp` or `ffmpeg`).
    pub tool: SidecarTool,
    /// Currently installed version string, if detected.
    pub current_version: Option<String>,
    /// Latest release version tag from GitHub (e.g. `"2026.11.23"`).
    pub latest_version: String,
    /// Whether an update is available (`current_version != latest_version`).
    pub update_available: bool,
    /// Download URL for the executable binary.
    pub binary_download_url: String,
    /// Download URL for the checksum manifest.
    pub checksum_download_url: String,
    /// Authoritative SHA-256 hex digest extracted from release checksums.
    pub expected_sha256: String,
    /// Asset size in bytes.
    pub binary_size: u64,
    /// ISO 8601 release publication timestamp.
    pub published_at: Option<String>,
    /// Release notes body.
    pub release_notes: Option<String>,
}

/// Detailed report returned after downloading, verifying, and applying an update.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SidecarUpdateReport {
    /// Tool updated.
    pub tool: SidecarTool,
    /// Previous version installed before update.
    pub previous_version: Option<String>,
    /// New version installed.
    pub updated_version: String,
    /// Full path to the installed executable.
    pub installed_path: String,
    /// Verified SHA-256 hex digest of the installed binary.
    pub sha256_hash: String,
    /// Whether the update succeeded.
    pub success: bool,
    /// User-facing descriptive message.
    pub message: String,
}

/// Status summary for an installed sidecar binary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SidecarStatusReport {
    /// Tool identifier (`yt-dlp` or `ffmpeg`).
    pub tool: SidecarTool,
    /// Canonical name string.
    pub tool_name: String,
    /// Resolved binary path on the filesystem.
    pub binary_path: String,
    /// Whether the binary file exists and is executable.
    pub exists: bool,
    /// Current detected version string.
    pub version: Option<String>,
    /// Stored SHA-256 hash if tracked in local manifest.
    pub sha256_hash: Option<String>,
    /// ISO 8601 timestamp of last update check.
    pub last_checked_at: Option<String>,
}

/// Comprehensive guidance payload for resolving YouTube bot blocks (`ERR_INGEST_BOT_BLOCKED`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AntiBotGuidance {
    /// Sidecar tool (`yt-dlp`).
    pub tool: SidecarTool,
    /// Current installed version.
    pub current_version: Option<String>,
    /// Whether the installed version is already the latest available release.
    pub is_latest: bool,
    /// Rotated desktop User-Agent string.
    pub rotated_user_agent: String,
    /// Supported desktop browsers for cookie extraction.
    pub supported_browsers: Vec<String>,
    /// Recommended cookie extraction command options.
    pub recommended_cookie_sources: Vec<String>,
    /// Example CLI arguments for yt-dlp anti-bot defense.
    pub cli_flags_example: Vec<String>,
    /// Localized Vietnamese instructions for the user.
    pub instructions_vi: String,
    /// Raw error details that triggered the bot block.
    pub bot_blocked_details: Option<String>,
}

/// Engine for checking, downloading, verifying, and applying sidecar binary updates.
#[derive(Clone)]
pub struct SidecarUpdater {
    config: SidecarUpdaterConfig,
    client: reqwest::Client,
    mock_handler: Option<MockHttpHandler>,
}

impl SidecarUpdater {
    /// Create a new updater instance with the specified configuration.
    #[must_use]
    pub fn new(config: SidecarUpdaterConfig) -> Self {
        let client = reqwest::Client::builder()
            .user_agent(&config.user_agent)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            config,
            client,
            mock_handler: None,
        }
    }

    /// Attach an in-memory mock HTTP transport handler for hermetic testing (`CONVENTIONS.md` §7.1).
    #[must_use]
    pub fn with_mock_handler(mut self, handler: MockHttpHandler) -> Self {
        self.mock_handler = Some(handler);
        self
    }

    /// Reference to updater configuration.
    #[must_use]
    pub fn config(&self) -> &SidecarUpdaterConfig {
        &self.config
    }

    /// Target installation directory for managed binaries.
    #[must_use]
    pub fn target_dir(&self) -> PathBuf {
        self.config
            .target_dir
            .clone()
            .unwrap_or_else(get_default_sidecar_dir)
    }

    /// Target local manifest path (`sidecar_manifest.json`).
    #[must_use]
    pub fn manifest_path(&self, tool: SidecarTool) -> PathBuf {
        self.target_dir()
            .join(format!("{}_manifest.json", tool.as_str()))
    }

    /// Resolve effective target binary path in the managed directory.
    #[must_use]
    pub fn target_binary_path(&self, tool: SidecarTool) -> PathBuf {
        self.target_dir().join(tool.exe_name())
    }

    /// Resolve the installed binary path using the multi-tier discovery hierarchy (`resolve_sidecar_binary`).
    #[must_use]
    pub fn resolve_installed_path(&self, tool: SidecarTool) -> String {
        resolve_sidecar_binary(tool.as_str(), tool.env_var())
    }

    /// Probe the currently installed version of a sidecar binary by executing `<binary> --version`.
    pub fn probe_installed_version(&self, tool: SidecarTool) -> Option<String> {
        let bin_path = self.resolve_installed_path(tool);
        let path_obj = Path::new(&bin_path);

        // If file exists or is bare command name, attempt to run `--version`
        if path_obj.is_file() || !bin_path.contains(std::path::MAIN_SEPARATOR) {
            if let Ok(output) = Command::new(&bin_path).arg("--version").output() {
                if output.status.success() {
                    let text = String::from_utf8_lossy(&output.stdout);
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        // Take the first line of output
                        if let Some(first_line) = trimmed.lines().next() {
                            return Some(first_line.trim().to_string());
                        }
                    }
                }
            }
        }

        // Fallback: check stored version in local manifest
        let manifest_file = self.manifest_path(tool);
        if let Ok(Some(manifest)) = load_local_manifest(&manifest_file) {
            if let Some(v) = manifest.installed_version {
                return Some(v);
            }
        }

        None
    }

    /// Query the status of a managed sidecar.
    pub fn get_status(&self, tool: SidecarTool) -> SidecarStatusReport {
        let bin_path = self.resolve_installed_path(tool);
        let exists = Path::new(&bin_path).is_file();
        let version = self.probe_installed_version(tool);

        let manifest_file = self.manifest_path(tool);
        let (sha256_hash, last_checked_at) =
            if let Ok(Some(manifest)) = load_local_manifest(&manifest_file) {
                (manifest.sha256_hash, manifest.last_checked_at)
            } else {
                (None, None)
            };

        SidecarStatusReport {
            tool,
            tool_name: tool.as_str().to_string(),
            binary_path: bin_path,
            exists,
            version,
            sha256_hash,
            last_checked_at,
        }
    }

    /// Query GitHub Releases API to check for updates.
    ///
    /// # Errors
    /// Returns [`AppError::Sidecar`] if network requests, JSON parsing, or asset resolution fails.
    pub async fn check_latest_release(
        &self,
        tool: SidecarTool,
    ) -> Result<ReleaseCheckInfo, AppError> {
        let endpoint = if let Some(ref custom_url) = self.config.release_api_url {
            custom_url.clone()
        } else if let Ok(env_url) = std::env::var(format!("{}_RELEASE_URL", tool.env_var())) {
            env_url
        } else {
            let (owner, repo) = tool.github_repo();
            format!("https://api.github.com/repos/{owner}/{repo}/releases/latest")
        };

        debug!(tool = tool.as_str(), endpoint = %endpoint, "Fetching latest release metadata");

        let release_bytes = self.http_get(&endpoint).await?;
        let release: GitHubRelease = serde_json::from_slice(&release_bytes).map_err(|e| {
            AppError::Sidecar(SidecarError::UpdateFailed {
                binary: tool.as_str().to_string(),
                reason: format!("Không thể phân tích phản hồi từ GitHub Releases API: {e}"),
            })
        })?;

        // Locate binary asset (e.g. yt-dlp.exe)
        let binary_asset = release.find_binary_asset(tool).ok_or_else(|| {
            AppError::Sidecar(SidecarError::UpdateFailed {
                binary: tool.as_str().to_string(),
                reason: format!(
                    "Không tìm thấy tập tin nhị phân '{}' trong bản phát hành {}",
                    tool.exe_name(),
                    release.tag_name
                ),
            })
        })?;

        // Locate checksum asset (e.g. SHA2-256SUMS)
        let checksum_asset = release.find_checksum_asset(tool).ok_or_else(|| {
            AppError::Sidecar(SidecarError::UpdateFailed {
                binary: tool.as_str().to_string(),
                reason: format!(
                    "Không tìm thấy tập tin mã băm kiểm tra '{}' trong bản phát hành {}",
                    tool.checksum_asset_name(),
                    release.tag_name
                ),
            })
        })?;

        // Download checksum asset content
        debug!(
            tool = tool.as_str(),
            url = %checksum_asset.browser_download_url,
            "Downloading release checksum manifest"
        );
        let checksum_bytes = self.http_get(&checksum_asset.browser_download_url).await?;
        let checksum_text = String::from_utf8_lossy(&checksum_bytes);

        // Resolve expected SHA-256 for the binary
        let expected_sha256 = resolve_asset_checksum(&checksum_text, &binary_asset.name)?;

        let current_version = self.probe_installed_version(tool);
        let latest_version = release.tag_name.clone();

        let update_available = match current_version.as_deref() {
            Some(curr) => {
                let clean_curr = curr.trim_start_matches('v');
                let clean_latest = latest_version.trim_start_matches('v');
                clean_curr != clean_latest
            }
            None => true,
        };

        // Record check timestamp in local manifest
        let manifest_file = self.manifest_path(tool);
        let mut manifest = load_local_manifest(&manifest_file)
            .unwrap_or(None)
            .unwrap_or_else(|| SidecarLocalManifest {
                tool_name: tool.as_str().to_string(),
                ..Default::default()
            });
        manifest.last_checked_at = Some(chrono::Utc::now().to_rfc3339());
        let _ = save_local_manifest(&manifest_file, &manifest);

        Ok(ReleaseCheckInfo {
            tool,
            current_version,
            latest_version,
            update_available,
            binary_download_url: binary_asset.browser_download_url.clone(),
            checksum_download_url: checksum_asset.browser_download_url.clone(),
            expected_sha256,
            binary_size: binary_asset.size,
            published_at: release.published_at,
            release_notes: release.body,
        })
    }

    /// Download the binary into a temporary file on the target directory and verify its SHA-256 digest (`AC-29.1`, `UT-29.1`, `UT-29.2`).
    ///
    /// # Errors
    /// Returns [`AppError::Sidecar`] with [`SidecarError::UpdateFailed`] if the download fails or
    /// if the SHA-256 digest does not match the expected digest. The temporary file is guaranteed to be deleted on failure.
    pub async fn download_and_verify(
        &self,
        download_url: &str,
        expected_sha256: &str,
        target_dir: &Path,
        exe_name: &str,
    ) -> Result<PathBuf, AppError> {
        if !target_dir.exists() {
            std::fs::create_dir_all(target_dir).map_err(|e| {
                AppError::Storage(StorageError::DirectoryCreationFailed {
                    path: target_dir.to_string_lossy().into_owned(),
                    details: e.to_string(),
                })
            })?;
        }

        // Temporary file placed in target directory ensures atomic rename on same NTFS volume
        let nonce = next_nonce();
        let pid = std::process::id();
        let temp_filename = format!("{exe_name}.download_{pid}_{nonce}");
        let temp_path = target_dir.join(temp_filename);

        info!(
            exe = exe_name,
            temp_path = %temp_path.display(),
            "Downloading sidecar binary"
        );

        let binary_bytes = match self.http_get(download_url).await {
            Ok(bytes) => bytes,
            Err(e) => {
                let _ = std::fs::remove_file(&temp_path);
                return Err(e);
            }
        };

        if let Err(e) = std::fs::write(&temp_path, &binary_bytes) {
            let _ = std::fs::remove_file(&temp_path);
            return Err(AppError::Sidecar(SidecarError::UpdateFailed {
                binary: exe_name.to_string(),
                reason: format!("Không thể ghi tập tin tải về tạm thời: {e}"),
            }));
        }

        // Compute SHA-256 digest of the downloaded payload
        let mut hasher = Sha256::new();
        hasher.update(&binary_bytes);
        let digest_result = hasher.finalize();
        let computed_sha256 = format!("{digest_result:x}");

        let expected_clean = expected_sha256.trim().to_ascii_lowercase();
        let computed_clean = computed_sha256.trim().to_ascii_lowercase();

        if computed_clean != expected_clean {
            warn!(
                expected = %expected_clean,
                computed = %computed_clean,
                path = %temp_path.display(),
                "SHA-256 checksum mismatch! Immediately deleting corrupted/tampered download"
            );
            // Mandatory invariant: delete downloaded file immediately on hash mismatch
            let _ = std::fs::remove_file(&temp_path);

            return Err(AppError::Sidecar(SidecarError::UpdateFailed {
                binary: exe_name.to_string(),
                reason: format!(
                    "Mã băm SHA-256 không khớp! Dự kiến: '{expected_clean}', Thực tế: '{computed_clean}'. Tập tin đã bị hủy bỏ."
                ),
            }));
        }

        info!(
            exe = exe_name,
            sha256 = %computed_clean,
            "SHA-256 checksum verified successfully"
        );

        Ok(temp_path)
    }

    /// Perform a safe atomic swap and automatic rollback on Windows (`AC-29.3`, `IT-29.1`).
    ///
    /// 1. Rename existing binary to `.old`.
    /// 2. Move verified `.download` into target position.
    /// 3. If step 2 fails, roll back `.old` back to target position.
    /// 4. Best-effort cleanup of `.old`.
    ///
    /// # Errors
    /// Returns [`AppError::Sidecar`] if file replacement fails.
    pub fn atomic_swap_and_rollback(
        &self,
        download_path: &Path,
        target_path: &Path,
    ) -> Result<(), AppError> {
        let target_str = target_path.to_string_lossy().into_owned();
        let exe_name = target_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("sidecar");

        let old_path = target_path.with_extension("exe.old");

        // Clean up previous .old backup if present
        if old_path.exists() {
            let _ = std::fs::remove_file(&old_path);
        }

        let mut swapped_to_old = false;

        // Step 1: Backup active binary by renaming to .old
        if target_path.exists() {
            if let Err(e) = std::fs::rename(target_path, &old_path) {
                let _ = std::fs::remove_file(download_path);
                return Err(AppError::Sidecar(SidecarError::UpdateFailed {
                    binary: exe_name.to_string(),
                    reason: format!(
                        "Không thể đổi tên tập tin hiện tại sang bản sao lưu .old ({target_str}): {e}"
                    ),
                }));
            }
            swapped_to_old = true;
        }

        // Step 2: Move new verified binary into place
        if let Err(e) = std::fs::rename(download_path, target_path) {
            warn!(
                target = %target_str,
                error = %e,
                "Atomic replacement failed! Initiating automatic rollback"
            );

            // Rollback: restore .old back to target binary
            if swapped_to_old && old_path.exists() {
                if let Err(rollback_err) = std::fs::rename(&old_path, target_path) {
                    warn!(
                        target = %target_str,
                        error = %rollback_err,
                        "Critical rollback failure: could not restore .old backup"
                    );
                } else {
                    info!(target = %target_str, "Rollback to previous version succeeded");
                }
            }

            let _ = std::fs::remove_file(download_path);

            return Err(AppError::Sidecar(SidecarError::UpdateFailed {
                binary: exe_name.to_string(),
                reason: format!(
                    "Hoán đổi tập tin thất bại ({e}). Đã khôi phục về phiên bản trước đó thành công."
                ),
            }));
        }

        // Step 3: Best-effort removal of .old backup
        if old_path.exists() {
            if let Err(e) = std::fs::remove_file(&old_path) {
                debug!(
                    old_path = %old_path.display(),
                    error = %e,
                    "Deferred cleanup of .old backup (likely file lock; will be pruned later)"
                );
            }
        }

        info!(
            target = %target_str,
            "Sidecar binary atomically updated in place"
        );
        Ok(())
    }

    /// Download, verify, and apply the latest release for a sidecar tool.
    ///
    /// # Errors
    /// Returns [`AppError::Sidecar`] if update check, verification, or replacement fails.
    pub async fn apply_update(&self, tool: SidecarTool) -> Result<SidecarUpdateReport, AppError> {
        let check = self.check_latest_release(tool).await?;
        let target_dir = self.target_dir();
        let target_path = target_dir.join(tool.exe_name());

        info!(
            tool = tool.as_str(),
            latest_version = %check.latest_version,
            "Applying sidecar update"
        );

        let verified_temp = self
            .download_and_verify(
                &check.binary_download_url,
                &check.expected_sha256,
                &target_dir,
                tool.exe_name(),
            )
            .await?;

        // Perform atomic swap
        self.atomic_swap_and_rollback(&verified_temp, &target_path)?;

        // Update local manifest
        let manifest_file = self.manifest_path(tool);
        let now_str = chrono::Utc::now().to_rfc3339();
        let manifest = SidecarLocalManifest {
            tool_name: tool.as_str().to_string(),
            installed_version: Some(check.latest_version.clone()),
            installed_path: Some(target_path.to_string_lossy().into_owned()),
            sha256_hash: Some(check.expected_sha256.clone()),
            last_checked_at: Some(now_str.clone()),
            last_updated_at: Some(now_str),
        };
        let _ = save_local_manifest(&manifest_file, &manifest);

        Ok(SidecarUpdateReport {
            tool,
            previous_version: check.current_version,
            updated_version: check.latest_version,
            installed_path: target_path.to_string_lossy().into_owned(),
            sha256_hash: check.expected_sha256,
            success: true,
            message: format!(
                "Đã cập nhật '{}' lên phiên bản mới nhất thành công.",
                tool.as_str()
            ),
        })
    }

    /// Generate multi-layer anti-bot defense guidance (`ERR-04`, `AC-29.4`, `ED-29.1`).
    pub fn create_anti_bot_guidance(
        &self,
        tool: SidecarTool,
        bot_error_details: Option<String>,
    ) -> AntiBotGuidance {
        let current_version = self.probe_installed_version(tool);
        let rotated_ua = get_rotated_desktop_user_agent().to_string();

        let supported_browsers = vec![
            BrowserType::Chrome.as_str().to_string(),
            BrowserType::Firefox.as_str().to_string(),
            BrowserType::Edge.as_str().to_string(),
        ];

        let recommended_cookie_sources = vec![
            "--cookies-from-browser chrome".to_string(),
            "--cookies-from-browser firefox".to_string(),
            "--cookies-from-browser edge".to_string(),
            "--cookies <path_to_cookies.txt>".to_string(),
        ];

        let cli_flags_example = vec![
            "--cookies-from-browser".to_string(),
            "chrome".to_string(),
            "--user-agent".to_string(),
            rotated_ua.clone(),
            "--add-header".to_string(),
            "Accept-Language: en-US,en;q=0.9".to_string(),
        ];

        let instructions_vi = [
            "Hệ thống phát hiện YouTube yêu cầu xác minh bot (HTTP 403 / Sign in to confirm you're not a bot).",
            "NeuroClip kích hoạt hệ thống phòng thủ đa tầng:",
            "1. Tự động kiểm tra và nâng cấp yt-dlp lên bản phát hành mới nhất.",
            "2. Sử dụng cookie từ trình duyệt (Chrome, Firefox hoặc Edge) để xác thực phiên người dùng hợp lệ.",
            "3. Luân phiên chuỗi User-Agent của máy tính để bàn để vượt qua kiểm tra tự động.",
            "Nếu sự cố vẫn tiếp diễn, vui lòng đăng nhập YouTube trên Chrome/Firefox hoặc xuất file cookies.txt và nạp vào Cài đặt -> Công cụ phụ trợ.",
        ]
        .join("\n");

        AntiBotGuidance {
            tool,
            current_version,
            is_latest: false, // UI / caller can update with check_latest_release
            rotated_user_agent: rotated_ua,
            supported_browsers,
            recommended_cookie_sources,
            cli_flags_example,
            instructions_vi,
            bot_blocked_details: bot_error_details,
        }
    }

    /// Internal HTTP GET implementation supporting mock handler and curl.exe fallback.
    async fn http_get(&self, url: &str) -> Result<Vec<u8>, AppError> {
        if let Some(ref mock) = self.mock_handler {
            let resp = mock(url)?;
            if resp.status_code == 200 {
                return Ok(resp.body);
            }
            return Err(AppError::Sidecar(SidecarError::UpdateFailed {
                binary: "sidecar".to_string(),
                reason: format!("Máy chủ phản hồi mã lỗi HTTP {}", resp.status_code),
            }));
        }

        // Production path: try reqwest first
        match self.client.get(url).send().await {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    let bytes = response.bytes().await.map_err(|e| {
                        AppError::Sidecar(SidecarError::UpdateFailed {
                            binary: "sidecar".to_string(),
                            reason: format!("Không thể đọc nội dung phản hồi: {e}"),
                        })
                    })?;
                    return Ok(bytes.to_vec());
                }
                warn!(status = %status, url = %url, "Reqwest HTTP request returned non-success");
            }
            Err(e) => {
                debug!(url = %url, error = %e, "Reqwest request failed, trying curl.exe fallback");
            }
        }

        // Subprocess curl.exe fallback on Windows
        #[cfg(windows)]
        {
            let mut cmd = Command::new("curl.exe");
            cmd.args([
                "-s",
                "-L",
                "--max-time",
                "30",
                "-A",
                &self.config.user_agent,
                url,
            ]);
            if let Ok(output) = cmd.output() {
                if output.status.success() && !output.stdout.is_empty() {
                    return Ok(output.stdout);
                }
            }
        }

        Err(AppError::Sidecar(SidecarError::UpdateFailed {
            binary: "sidecar".to_string(),
            reason: format!("Không thể tải dữ liệu từ URL: {url}"),
        }))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
pub mod tests {
    use super::*;

    #[test]
    fn test_antibot_guidance_generation() {
        let updater = SidecarUpdater::new(SidecarUpdaterConfig::default());
        let guidance = updater.create_anti_bot_guidance(
            SidecarTool::YtDlp,
            Some("HTTP Error 403: Forbidden".to_string()),
        );

        assert_eq!(guidance.tool, SidecarTool::YtDlp);
        assert_ne!(guidance.rotated_user_agent, "");
        assert!(guidance.supported_browsers.contains(&"chrome".to_string()));
        assert!(guidance
            .cli_flags_example
            .contains(&"--cookies-from-browser".to_string()));
        assert!(guidance.instructions_vi.contains("YouTube"));
        assert_eq!(
            guidance.bot_blocked_details,
            Some("HTTP Error 403: Forbidden".to_string())
        );
    }

    #[tokio::test]
    async fn test_download_and_verify_success() {
        let temp_dir =
            std::env::temp_dir().join(format!("test_updater_ut29_1_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);

        let binary_content = b"#!/bin/sh\necho yt-dlp mock binary\n";
        let mut hasher = Sha256::new();
        hasher.update(binary_content);
        let expected_sha256 = format!("{:x}", hasher.finalize());

        let bin_bytes = binary_content.to_vec();
        let mock_handler: MockHttpHandler = Arc::new(move |url: &str| {
            if url.ends_with("yt-dlp.exe") {
                Ok(MockHttpResponse::ok(bin_bytes.clone()))
            } else {
                Ok(MockHttpResponse::status(404, b"Not found"))
            }
        });

        let updater = SidecarUpdater::new(SidecarUpdaterConfig {
            target_dir: Some(temp_dir.clone()),
            ..Default::default()
        })
        .with_mock_handler(mock_handler);

        let result = updater
            .download_and_verify(
                "https://mock.github.com/yt-dlp.exe",
                &expected_sha256,
                &temp_dir,
                "yt-dlp.exe",
            )
            .await;

        assert!(result.is_ok());
        let temp_path = result.unwrap();
        assert!(temp_path.exists());
        assert_eq!(std::fs::read(&temp_path).unwrap(), binary_content);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_download_and_verify_tampered_rejected() {
        let temp_dir =
            std::env::temp_dir().join(format!("test_updater_ut29_2_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);

        let tampered_content = b"MALICIOUS_OR_CORRUPTED_PAYLOAD";
        let mock_handler: MockHttpHandler =
            Arc::new(move |_url: &str| Ok(MockHttpResponse::ok(tampered_content.to_vec())));

        let expected_sha256 = "622ed48023cac1daa52a3bf3a6ac38f03514de214b5f0fa069389fd0f6d17e5c";

        let updater = SidecarUpdater::new(SidecarUpdaterConfig {
            target_dir: Some(temp_dir.clone()),
            ..Default::default()
        })
        .with_mock_handler(mock_handler);

        let result = updater
            .download_and_verify(
                "https://mock.github.com/yt-dlp.exe",
                expected_sha256,
                &temp_dir,
                "yt-dlp.exe",
            )
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            AppError::Sidecar(SidecarError::UpdateFailed { binary, reason }) => {
                assert_eq!(binary, "yt-dlp.exe");
                assert!(reason.contains("Mã băm SHA-256 không khớp"));
            }
            other => panic!("Unexpected error variant: {other:?}"),
        }

        // Verify no leftover .download files exist in target_dir
        if temp_dir.exists() {
            let entries: Vec<_> = std::fs::read_dir(&temp_dir).unwrap().collect();
            assert!(
                entries.is_empty(),
                "Temp download was not deleted on hash mismatch!"
            );
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_atomic_swap_and_rollback_success() {
        let temp_dir =
            std::env::temp_dir().join(format!("test_updater_it29_1_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let target_path = temp_dir.join("yt-dlp.exe");
        let download_path = temp_dir.join("yt-dlp.exe.download");

        std::fs::write(&target_path, b"ORIGINAL_V1").unwrap();
        std::fs::write(&download_path, b"NEW_V2").unwrap();

        let updater = SidecarUpdater::new(SidecarUpdaterConfig {
            target_dir: Some(temp_dir.clone()),
            ..Default::default()
        });

        updater
            .atomic_swap_and_rollback(&download_path, &target_path)
            .unwrap();

        assert_eq!(std::fs::read(&target_path).unwrap(), b"NEW_V2");
        assert!(!download_path.exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
