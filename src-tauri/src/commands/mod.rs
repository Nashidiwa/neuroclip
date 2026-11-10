//! Tauri IPC command modules.

pub mod config_cmd;

pub use config_cmd::{
    detect_hardware, get_app_config, get_hardware_profile, save_app_config, AppState,
};
