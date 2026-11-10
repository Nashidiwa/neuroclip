#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::todo)]
#![deny(clippy::unimplemented)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]

pub mod commands;
pub mod config;
pub mod error;
pub mod logging;

#[path = "core/storage.rs"]
pub mod storage;

/// Backward-compatibility and architectural alias for core logging subsystem.
pub mod core {
    pub use crate::logging;
    pub use crate::storage;
}

pub use commands::AppState;
pub use config::{
    AppConfig, CpuInfo, GpuInfo, GpuVendor, HardwareProfile, RamInfo, RecommendedMode, SidecarPaths,
};
pub use core::storage;
pub use error::{
    codes, AppError, ConfigError, ExportError, HardwareError, HighlightError, IngestError,
    IpcErrorResponse, SidecarError, StorageError, SttError, TranslationError,
};
pub use logging::{
    is_neuroclip_log_file, prune_logs, redact_secret, redact_secrets_in_text, resolve_log_dir,
    LogEntry, PruneReport, RedactingWriter, RetentionPolicy,
};

pub const DEFAULT_WINDOW_WIDTH: f64 = 1280.0;
pub const DEFAULT_WINDOW_HEIGHT: f64 = 800.0;
pub const MIN_WINDOW_WIDTH: f64 = 1024.0;
pub const MIN_WINDOW_HEIGHT: f64 = 640.0;
pub const SOLID_BACKGROUND_FALLBACK: &str = "#12131C";

/// Health check ping/pong command adhering to Result<T, AppError> contract
#[tauri::command]
pub fn ping() -> Result<String, AppError> {
    Ok("pong".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if let Err(err) = logging::init_logging() {
        eprintln!("Failed to initialize logging: {err}");
    }

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            ping,
            commands::get_app_config,
            commands::save_app_config,
            commands::get_hardware_profile,
            commands::detect_hardware,
            commands::get_recent_logs,
            commands::clear_logs,
            commands::log_frontend_event,
            commands::check_preflight_storage,
            commands::initialize_storage,
            commands::initialize_project,
            commands::clean_expired_cache
        ]);

    if let Err(err) = builder.run(tauri::generate_context!()) {
        eprintln!("error while running tauri application: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ping_pong_response() {
        let result = ping();
        assert!(result.is_ok());
        if let Ok(value) = result {
            assert_eq!(value, "pong");
        }
    }

    #[test]
    fn test_window_geometry_constants() {
        assert!((DEFAULT_WINDOW_WIDTH - 1280.0).abs() < f64::EPSILON);
        assert!((DEFAULT_WINDOW_HEIGHT - 800.0).abs() < f64::EPSILON);
        assert!((MIN_WINDOW_WIDTH - 1024.0).abs() < f64::EPSILON);
        assert!((MIN_WINDOW_HEIGHT - 640.0).abs() < f64::EPSILON);
        assert_eq!(SOLID_BACKGROUND_FALLBACK, "#12131C");
    }
}
