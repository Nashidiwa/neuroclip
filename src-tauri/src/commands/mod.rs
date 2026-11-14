//! Tauri IPC command modules.

pub mod chat_cmd;
pub mod config_cmd;
pub mod ingest_cmd;
pub mod log_cmd;
pub mod storage_cmd;

pub use chat_cmd::{
    __cmd__get_chat_events, __cmd__get_chat_velocity_heatmap, __cmd__start_chat_extraction,
    __tauri_command_name_get_chat_events, __tauri_command_name_get_chat_velocity_heatmap,
    __tauri_command_name_start_chat_extraction, get_chat_events, get_chat_velocity_heatmap,
    start_chat_extraction,
};
pub use config_cmd::{
    __cmd__detect_hardware, __cmd__get_app_config, __cmd__get_hardware_profile, __cmd__ping,
    __cmd__save_app_config, __tauri_command_name_detect_hardware,
    __tauri_command_name_get_app_config, __tauri_command_name_get_hardware_profile,
    __tauri_command_name_ping, __tauri_command_name_save_app_config, detect_hardware,
    get_app_config, get_hardware_profile, ping, save_app_config, AppState,
};
pub use ingest_cmd::{
    __cmd__cancel_vod_ingestion, __cmd__extract_pcm_stream, __cmd__get_vod_manifest,
    __cmd__probe_vod, __cmd__start_vod_ingestion, __tauri_command_name_cancel_vod_ingestion,
    __tauri_command_name_extract_pcm_stream, __tauri_command_name_get_vod_manifest,
    __tauri_command_name_probe_vod, __tauri_command_name_start_vod_ingestion,
    cancel_vod_ingestion, extract_pcm_stream, get_vod_manifest, probe_vod, start_vod_ingestion,
};
pub use log_cmd::{
    __cmd__clear_logs, __cmd__get_recent_logs, __cmd__log_frontend_event,
    __tauri_command_name_clear_logs, __tauri_command_name_get_recent_logs,
    __tauri_command_name_log_frontend_event, clear_logs, get_recent_logs, log_frontend_event,
};
pub use storage_cmd::{
    __cmd__check_preflight_storage, __cmd__clean_expired_cache, __cmd__initialize_project,
    __cmd__initialize_storage, __tauri_command_name_check_preflight_storage,
    __tauri_command_name_clean_expired_cache, __tauri_command_name_initialize_project,
    __tauri_command_name_initialize_storage, check_preflight_storage, clean_expired_cache,
    initialize_project, initialize_storage,
};
