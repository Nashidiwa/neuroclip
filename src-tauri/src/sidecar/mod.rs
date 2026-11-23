//! Managed sidecar subprocess supervision and OS kill-switch module (`TASK-M4-03`).

pub mod kill_switch;
pub mod supervisor;

use std::path::PathBuf;

#[cfg(windows)]
pub use kill_switch::get_global_job_guard;
pub use kill_switch::{assign_child_to_job_guard, ChildProcessGuard, WindowsJobObjectGuard};
pub use supervisor::{
    compute_sidecar_progress, create_bounded_log_channel, drain_pipe_lossy,
    parse_ffmpeg_progress_line, parse_ytdlp_error_line, FfmpegProgress, LossyLineReader,
    SidecarProgressCallback, SidecarProgressPayload, SubprocessConfig, SubprocessOutput,
    SubprocessSupervisor, BOUNDED_LOG_CAPACITY, MAX_LOG_LINE_BYTES, SIDECAR_PROGRESS_EVENT,
};

/// Resolve a sidecar binary (`yt-dlp`, `ffmpeg`, `ffprobe`) adhering to `ADR-007` tier precedence:
/// 1. `NEUROCLIP_<TOOL>_PATH` environment variable override
/// 2. `%APPDATA%/NeuroClip/bin/<tool>` and `%LOCALAPPDATA%/NeuroClip/bin/<tool>`
/// 3. `<exe_dir>/resources/sidecars/<tool>` (and relative `resources/sidecars/<tool>` in dev)
/// 4. Bare `<tool>` name resolved via system `PATH`
#[must_use]
pub fn resolve_sidecar_binary(tool_name: &str, env_var: &str) -> String {
    if let Ok(p) = std::env::var(env_var) {
        let trimmed = p.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    #[cfg(windows)]
    let exe_name = format!("{tool_name}.exe");
    #[cfg(not(windows))]
    let exe_name = tool_name.to_string();

    if let Ok(app_data) = std::env::var("APPDATA") {
        let trimmed = app_data.trim();
        if !trimmed.is_empty() {
            let candidate = PathBuf::from(trimmed)
                .join("NeuroClip")
                .join("bin")
                .join(&exe_name);
            if candidate.is_file() {
                return candidate.to_string_lossy().into_owned();
            }
        }
    }

    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let trimmed = local_app_data.trim();
        if !trimmed.is_empty() {
            let candidate = PathBuf::from(trimmed)
                .join("NeuroClip")
                .join("bin")
                .join(&exe_name);
            if candidate.is_file() {
                return candidate.to_string_lossy().into_owned();
            }
        }
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let candidate = exe_dir.join("resources").join("sidecars").join(&exe_name);
            if candidate.is_file() {
                return candidate.to_string_lossy().into_owned();
            }
        }
    }

    let dev_resource = PathBuf::from("resources").join("sidecars").join(&exe_name);
    if dev_resource.is_file() {
        return dev_resource.to_string_lossy().into_owned();
    }

    tool_name.to_string()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::core::storage::test_env::{ScopedEnvVar, ENV_LOCK};

    #[test]
    fn test_resolve_sidecar_binary_adr007_precedence() {
        let _env_guard = ENV_LOCK.blocking_lock();
        let temp_root =
            std::env::temp_dir().join(format!("neuroclip_sidecar_resolver_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_root);

        let appdata_dir = temp_root.join("appdata");
        let local_appdata_dir = temp_root.join("localappdata");
        let appdata_bin = appdata_dir.join("NeuroClip").join("bin");
        let local_bin = local_appdata_dir.join("NeuroClip").join("bin");
        std::fs::create_dir_all(&appdata_bin).unwrap();
        std::fs::create_dir_all(&local_bin).unwrap();

        #[cfg(windows)]
        let exe_name = "ffmpeg.exe";
        #[cfg(not(windows))]
        let exe_name = "ffmpeg";

        let appdata_exe = appdata_bin.join(exe_name);
        let local_exe = local_bin.join(exe_name);
        std::fs::write(&local_exe, b"local").unwrap();

        let _env_override = ScopedEnvVar::remove("NEUROCLIP_FFMPEG_PATH");
        let _env_appdata = ScopedEnvVar::set("APPDATA", &appdata_dir.to_string_lossy());
        let _env_local = ScopedEnvVar::set("LOCALAPPDATA", &local_appdata_dir.to_string_lossy());

        // 1. Falls back to %LOCALAPPDATA%/NeuroClip/bin when %APPDATA%/NeuroClip/bin lacks binary
        assert_eq!(
            resolve_sidecar_binary("ffmpeg", "NEUROCLIP_FFMPEG_PATH"),
            local_exe.to_string_lossy()
        );

        // 2. %APPDATA%/NeuroClip/bin takes precedence over %LOCALAPPDATA%/NeuroClip/bin
        std::fs::write(&appdata_exe, b"appdata").unwrap();
        assert_eq!(
            resolve_sidecar_binary("ffmpeg", "NEUROCLIP_FFMPEG_PATH"),
            appdata_exe.to_string_lossy()
        );

        // 3. Explicit NEUROCLIP_FFMPEG_PATH env var overrides everything
        let env_override_set =
            ScopedEnvVar::set("NEUROCLIP_FFMPEG_PATH", "C:\\custom\\ffmpeg_custom.exe");
        assert_eq!(
            resolve_sidecar_binary("ffmpeg", "NEUROCLIP_FFMPEG_PATH"),
            "C:\\custom\\ffmpeg_custom.exe"
        );
        drop(env_override_set);

        // 4. Falls back to bare tool name when no tier exists
        let _ = std::fs::remove_file(&appdata_exe);
        let _ = std::fs::remove_file(&local_exe);
        assert_eq!(
            resolve_sidecar_binary("ffmpeg", "NEUROCLIP_FFMPEG_PATH"),
            "ffmpeg"
        );

        let _ = std::fs::remove_dir_all(&temp_root);
    }
}
