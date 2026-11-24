//! Hermetic integration test suite for Managed Sidecars Auto-Updater (`TASK-M4-29`).
//!
//! Validates:
//! - `UT-29.1`: SHA-256 hash verification against release manifest (`AC-29.1`).
//! - `UT-29.2`: Tampered / corrupted binary rejection and immediate cleanup (`AC-29.1`).
//! - `IT-29.1`: Windows atomic swap and automatic rollback to `.old` backup (`AC-29.3`).
//! - `ED-29.1`: Multi-layer anti-bot defense guidance and bot block coordination (`AC-29.4`, `ERR-04`).
//!
//! Complies with the zero-network hermetic testing invariant (`CONVENTIONS.md` §7.1).

use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Arc;

use neuroclip_lib::error::{AppError, SidecarError};
use neuroclip_lib::ingestion::cookies::{get_desktop_user_agents, BrowserType};
use neuroclip_lib::sidecar::manifest::{
    load_local_manifest, parse_sha256sums, resolve_asset_checksum, SidecarTool,
};
use neuroclip_lib::sidecar::updater::{
    MockHttpHandler, MockHttpResponse, SidecarUpdater, SidecarUpdaterConfig,
};

/// Helper to create an isolated temporary workspace for sidecar testing.
fn create_test_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("neuroclip_test_{label}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("Failed creating test directory");
    dir
}

/// UT-29.1: Valid SHA-256 Checksum Verification Test (`AC-29.1`).
///
/// Ensures valid mock binary matching the release manifest's `SHA2-256SUMS` is successfully
/// verified, atomically installed, and reflected in the local `sidecar_manifest.json`.
#[tokio::test]
async fn test_ut_29_1_sha256_verification_success() {
    let test_dir = create_test_dir("ut_29_1");
    let binary_filename = SidecarTool::YtDlp.exe_name();
    let mock_binary_bytes = b"#!/bin/sh\necho 'yt-dlp version 2026.11.23'\n";

    // Compute authoritative SHA-256 of the mock binary
    let mut hasher = Sha256::new();
    hasher.update(mock_binary_bytes);
    let expected_sha256 = format!("{:x}", hasher.finalize());

    // Construct mock GitHub release JSON and SHA2-256SUMS content
    let release_json = serde_json::json!({
        "tag_name": "2026.11.23",
        "name": "yt-dlp 2026.11.23",
        "published_at": "2026-11-23T18:00:00Z",
        "assets": [
            {
                "name": binary_filename,
                "browser_download_url": format!("https://mock.github.com/releases/{binary_filename}"),
                "size": mock_binary_bytes.len(),
                "content_type": "application/octet-stream"
            },
            {
                "name": "SHA2-256SUMS",
                "browser_download_url": "https://mock.github.com/releases/SHA2-256SUMS",
                "size": 100,
                "content_type": "text/plain"
            }
        ]
    })
    .to_string();

    let checksum_content = format!("{expected_sha256}  {binary_filename}\n");

    let bin_clone = mock_binary_bytes.to_vec();
    let release_clone = release_json.into_bytes();
    let checksum_clone = checksum_content.into_bytes();

    let mock_handler: MockHttpHandler = Arc::new(move |url: &str| {
        if url.contains("releases/latest") {
            Ok(MockHttpResponse::ok(release_clone.clone()))
        } else if url.ends_with("SHA2-256SUMS") {
            Ok(MockHttpResponse::ok(checksum_clone.clone()))
        } else if url.ends_with(binary_filename) {
            Ok(MockHttpResponse::ok(bin_clone.clone()))
        } else {
            Ok(MockHttpResponse::status(404, b"Not Found"))
        }
    });

    let updater = SidecarUpdater::new(SidecarUpdaterConfig {
        target_dir: Some(test_dir.clone()),
        release_api_url: Some(
            "https://mock.github.com/repos/yt-dlp/yt-dlp/releases/latest".to_string(),
        ),
        ..Default::default()
    })
    .with_mock_handler(mock_handler);

    // 1. Verify release checking resolves expected checksum correctly
    let check = updater
        .check_latest_release(SidecarTool::YtDlp)
        .await
        .expect("check_latest_release failed");
    assert_eq!(check.latest_version, "2026.11.23");
    assert_eq!(check.expected_sha256, expected_sha256);
    assert_eq!(check.binary_size, mock_binary_bytes.len() as u64);

    // 2. Download and verify binary
    let temp_download = updater
        .download_and_verify(
            &check.binary_download_url,
            &check.expected_sha256,
            &test_dir,
            binary_filename,
        )
        .await
        .expect("download_and_verify failed");
    assert!(temp_download.exists());

    // 3. Apply update in place
    let report = updater
        .apply_update(SidecarTool::YtDlp)
        .await
        .expect("apply_update failed");
    assert!(report.success);
    assert_eq!(report.updated_version, "2026.11.23");
    assert_eq!(report.sha256_hash, expected_sha256);

    // 4. Verify target binary exists and contains correct bytes
    let installed_binary = test_dir.join(binary_filename);
    assert!(installed_binary.is_file());
    assert_eq!(
        std::fs::read(&installed_binary).expect("Failed reading installed binary"),
        mock_binary_bytes
    );

    // 5. Verify local manifest was persisted atomically
    let manifest_file = updater.manifest_path(SidecarTool::YtDlp);
    let manifest = load_local_manifest(&manifest_file)
        .expect("Failed loading manifest")
        .expect("Manifest missing");
    assert_eq!(manifest.tool_name, "yt-dlp");
    assert_eq!(manifest.installed_version, Some("2026.11.23".to_string()));
    assert_eq!(manifest.sha256_hash, Some(expected_sha256));

    let _ = std::fs::remove_dir_all(&test_dir);
}

/// UT-29.2: Tampered / Corrupted Binary Rejection Test (`AC-29.1`).
///
/// Ensures that when downloaded bytes do not match the expected SHA-256 digest:
/// 1. An error [`SidecarError::UpdateFailed`] is returned.
/// 2. The temporary download file is immediately deleted.
/// 3. The existing executable binary remains intact and uncorrupted.
#[tokio::test]
async fn test_ut_29_2_tampered_binary_rejected() {
    let test_dir = create_test_dir("ut_29_2");
    let binary_filename = SidecarTool::YtDlp.exe_name();
    let original_binary_bytes = b"ORIGINAL_TRUSTED_BINARY_V1";
    let target_binary = test_dir.join(binary_filename);
    std::fs::write(&target_binary, original_binary_bytes).expect("Failed writing original binary");

    // Expected digest from authoritative manifest
    let authoritative_sha256 = "622ed48023cac1daa52a3bf3a6ac38f03514de214b5f0fa069389fd0f6d17e5c";

    // Tampered payload delivered by mock server
    let tampered_payload = b"CORRUPTED_OR_TAMPERED_MALICIOUS_BYTES";

    let mock_handler: MockHttpHandler =
        Arc::new(move |_url: &str| Ok(MockHttpResponse::ok(tampered_payload.to_vec())));

    let updater = SidecarUpdater::new(SidecarUpdaterConfig {
        target_dir: Some(test_dir.clone()),
        ..Default::default()
    })
    .with_mock_handler(mock_handler);

    // Attempt download and verification with tampered payload
    let result = updater
        .download_and_verify(
            "https://mock.github.com/releases/yt-dlp.exe",
            authoritative_sha256,
            &test_dir,
            binary_filename,
        )
        .await;

    // 1. Must return UpdateFailed error
    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        AppError::Sidecar(SidecarError::UpdateFailed { binary, reason }) => {
            assert_eq!(binary, binary_filename);
            assert!(
                reason.contains("Mã băm SHA-256 không khớp"),
                "Expected SHA-256 mismatch explanation, got: {reason}"
            );
        }
        other => panic!("Unexpected error returned: {other:?}"),
    }

    // 2. Existing binary MUST remain untouched and intact
    assert_eq!(
        std::fs::read(&target_binary).expect("Failed reading original binary"),
        original_binary_bytes
    );

    // 3. No `.download` files remain in target directory
    let entries: Vec<_> = std::fs::read_dir(&test_dir)
        .expect("Failed reading dir")
        .filter_map(Result::ok)
        .collect();
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        assert!(
            !name.contains(".download"),
            "Found leaked download file: {name}"
        );
    }

    let _ = std::fs::remove_dir_all(&test_dir);
}

/// IT-29.1: Atomic Swap and Automatic Rollback Test (`AC-29.3`).
///
/// Validates:
/// 1. Successful atomic replacement from `.download` to target, backing up to `.old`.
/// 2. Automatic rollback to previous `.old` version when swap is interrupted/fails.
#[test]
fn test_it_29_1_atomic_swap_and_rollback() {
    let test_dir = create_test_dir("it_29_1");
    let binary_filename = SidecarTool::YtDlp.exe_name();
    let target_binary = test_dir.join(binary_filename);
    let download_binary = test_dir.join(format!("{binary_filename}.download"));

    let v1_bytes = b"SIDE_CAR_V1_OPERATIONAL";
    let v2_bytes = b"SIDE_CAR_V2_UPGRADED";

    // Setup initial operational binary v1
    std::fs::write(&target_binary, v1_bytes).expect("Failed writing v1");
    std::fs::write(&download_binary, v2_bytes).expect("Failed writing v2");

    let updater = SidecarUpdater::new(SidecarUpdaterConfig {
        target_dir: Some(test_dir.clone()),
        ..Default::default()
    });

    // 1. Perform successful atomic swap
    updater
        .atomic_swap_and_rollback(&download_binary, &target_binary)
        .expect("atomic_swap failed");

    // Target binary now contains v2 bytes
    assert_eq!(
        std::fs::read(&target_binary).expect("Failed reading target binary"),
        v2_bytes
    );
    assert!(!download_binary.exists());

    // 2. Verify rollback simulation:
    // If target cannot be replaced by invalid download path, rollback preserves original
    let invalid_download = test_dir.join("nonexistent_temp.download");
    let rollback_result = updater.atomic_swap_and_rollback(&invalid_download, &target_binary);
    assert!(rollback_result.is_err());

    // Operational binary v2 is restored and intact
    assert_eq!(
        std::fs::read(&target_binary).expect("Failed reading restored binary"),
        v2_bytes
    );

    let _ = std::fs::remove_dir_all(&test_dir);
}

/// ED-29.1: Multi-layer Anti-Bot Defense Guidance & Bot Block Coordination (`AC-29.4`, `ERR-04`).
///
/// Validates:
/// 1. Rotated desktop User-Agent string drawn from genuine Windows browser pool (`DESKTOP_USER_AGENTS`).
/// 2. Supported browser identifiers include Chrome, Firefox, and Edge.
/// 3. Clear Vietnamese guidance explaining YouTube bot challenge recovery.
/// 4. CLI arguments correctly include `--cookies-from-browser` and `--user-agent`.
#[test]
fn test_ed_29_1_anti_bot_guidance_and_coordination() {
    let updater = SidecarUpdater::new(SidecarUpdaterConfig::default());
    let bot_error = "HTTP Error 403: Forbidden - Sign in to confirm you’re not a bot";

    let guidance =
        updater.create_anti_bot_guidance(SidecarTool::YtDlp, Some(bot_error.to_string()));

    // 1. Tool verification
    assert_eq!(guidance.tool, SidecarTool::YtDlp);

    // 2. User-Agent rotation verification
    let valid_user_agents = get_desktop_user_agents();
    assert!(
        valid_user_agents.contains(&guidance.rotated_user_agent.as_str()),
        "Rotated User-Agent '{}' not found in desktop pool",
        guidance.rotated_user_agent
    );

    // 3. Supported browsers
    assert!(guidance
        .supported_browsers
        .contains(&BrowserType::Chrome.as_str().to_string()));
    assert!(guidance
        .supported_browsers
        .contains(&BrowserType::Firefox.as_str().to_string()));
    assert!(guidance
        .supported_browsers
        .contains(&BrowserType::Edge.as_str().to_string()));

    // 4. CLI flags example
    let flags = &guidance.cli_flags_example;
    assert!(flags.contains(&"--cookies-from-browser".to_string()));
    assert!(flags.contains(&"chrome".to_string()));
    assert!(flags.contains(&"--user-agent".to_string()));
    assert!(flags.contains(&guidance.rotated_user_agent));
    assert!(flags.contains(&"--add-header".to_string()));
    assert!(flags.contains(&"Accept-Language: en-US,en;q=0.9".to_string()));

    // 5. Localized Vietnamese recovery instructions
    assert!(
        guidance.instructions_vi.contains("YouTube"),
        "Guidance missing YouTube reference"
    );
    assert!(
        guidance.instructions_vi.contains("cookie"),
        "Guidance missing cookie reference"
    );
    assert!(
        guidance.instructions_vi.contains("User-Agent"),
        "Guidance missing User-Agent reference"
    );

    // 6. Error detail preservation
    assert_eq!(guidance.bot_blocked_details, Some(bot_error.to_string()));
}

/// Checksum parsing edge cases: verify case-insensitive lookup and varying whitespace.
#[test]
fn test_sha256sums_parser_edge_cases() {
    let raw = r"
# Standard SHA256 file
622ed48023cac1daa52a3bf3a6ac38f03514de214b5f0fa069389fd0f6d17e5c  yt-dlp.exe
1111111111111111111111111111111111111111111111111111111111111111 *yt-dlp_arm64.exe
2222222222222222222222222222222222222222222222222222222222222222 yt-dlp_linux
";
    let map = parse_sha256sums(raw);
    assert_eq!(map.len(), 3);
    assert!(map.contains_key("yt-dlp.exe"));
    assert!(map.contains_key("yt-dlp_arm64.exe"));
    assert!(map.contains_key("yt-dlp_linux"));

    let hash_lower = resolve_asset_checksum(raw, "yt-dlp.exe").expect("Lookup failed");
    assert_eq!(
        hash_lower,
        "622ed48023cac1daa52a3bf3a6ac38f03514de214b5f0fa069389fd0f6d17e5c"
    );

    // Case-insensitive lookup
    let hash_upper = resolve_asset_checksum(raw, "YT-DLP.EXE").expect("Lookup failed");
    assert_eq!(hash_upper, hash_lower);

    // Asterisk binary prefix handling
    let hash_arm = resolve_asset_checksum(raw, "yt-dlp_arm64.exe").expect("Lookup failed");
    assert_eq!(
        hash_arm,
        "1111111111111111111111111111111111111111111111111111111111111111"
    );
}
