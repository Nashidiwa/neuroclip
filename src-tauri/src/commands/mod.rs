//! Tauri IPC command modules.

pub mod chat_cmd;
pub mod config_cmd;
pub mod highlight_cmd;
pub mod ingest_cmd;
pub mod log_cmd;
pub mod slice_cmd;
pub mod storage_cmd;
pub mod stt_cmd;
pub mod translation_cmd;

pub use highlight_cmd::{
    __cmd__detect_highlights, __cmd__get_highlight_clusters, __cmd__toggle_cluster_bookmark,
    __cmd__toggle_highlight_bookmark, __tauri_command_name_detect_highlights,
    __tauri_command_name_get_highlight_clusters, __tauri_command_name_toggle_cluster_bookmark,
    __tauri_command_name_toggle_highlight_bookmark, detect_highlights, get_highlight_clusters,
    toggle_cluster_bookmark, toggle_highlight_bookmark,
};

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
pub use stt_cmd::{
    __cmd__export_subtitles_ass, __cmd__export_subtitles_srt,
    __cmd__start_whisper_transcription, __cmd__transcribe_highlight_clip,
    __cmd__verify_stt_model, __tauri_command_name_export_subtitles_ass,
    __tauri_command_name_export_subtitles_srt,
    __tauri_command_name_start_whisper_transcription,
    __tauri_command_name_transcribe_highlight_clip, __tauri_command_name_verify_stt_model,
    export_subtitles_ass, export_subtitles_srt, start_whisper_transcription,
    transcribe_highlight_clip, verify_stt_model,
};
pub use slice_cmd::{
    __cmd__cancel_slice_video_segment, __cmd__slice_video_segment,
    __tauri_command_name_cancel_slice_video_segment, __tauri_command_name_slice_video_segment,
    cancel_slice_task_token, cancel_slice_video_segment, register_slice_task_token,
    resolve_vod_project_dir, slice_video_segment, slice_video_segment_internal,
    unregister_slice_task_token,
};
pub use translation_cmd::{
    __cmd__test_gemini_connection, __cmd__translate_vietsub, __cmd__update_speaker_role,
    __tauri_command_name_test_gemini_connection, __tauri_command_name_translate_vietsub,
    __tauri_command_name_update_speaker_role, test_gemini_connection, translate_vietsub,
    update_speaker_role,
};
