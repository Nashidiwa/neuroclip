//! Managed sidecar subprocess supervision and OS kill-switch module (`TASK-M4-03`).

pub mod kill_switch;
pub mod supervisor;

#[cfg(windows)]
pub use kill_switch::get_global_job_guard;
pub use kill_switch::{assign_child_to_job_guard, ChildProcessGuard, WindowsJobObjectGuard};
pub use supervisor::{
    compute_sidecar_progress, create_bounded_log_channel, parse_ffmpeg_progress_line,
    parse_ytdlp_error_line, FfmpegProgress, SidecarProgressCallback, SidecarProgressPayload,
    SubprocessConfig, SubprocessOutput, SubprocessSupervisor, BOUNDED_LOG_CAPACITY,
    SIDECAR_PROGRESS_EVENT,
};
