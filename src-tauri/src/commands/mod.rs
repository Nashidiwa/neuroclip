//! Tauri IPC command modules.

pub mod config_cmd;
pub mod log_cmd;
pub mod storage_cmd;

pub use config_cmd::{
    detect_hardware, get_app_config, get_hardware_profile, save_app_config, AppState,
};
pub use log_cmd::{clear_logs, get_recent_logs, log_frontend_event};
pub use storage_cmd::{
    check_preflight_storage, clean_expired_cache, initialize_project, initialize_storage,
};
