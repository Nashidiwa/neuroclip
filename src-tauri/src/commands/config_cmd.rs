//! IPC Command handlers for Application Configuration and Hardware Profiling.

use crate::config::app_config::{resolve_config_file_path, AppConfig};
use crate::config::atomic::atomic_save_json;
use crate::config::hardware::{probe_system_hardware, HardwareProfile};
use crate::error::AppError;
use std::sync::RwLock;
use tauri::State;

/// Shared application state managed across Tauri IPC commands.
pub struct AppState {
    pub config: RwLock<AppConfig>,
    pub hardware_profile: RwLock<Option<HardwareProfile>>,
    config_save_lock: tokio::sync::Mutex<()>,
}

impl Default for AppState {
    fn default() -> Self {
        let path = resolve_config_file_path();
        let config =
            crate::config::atomic::atomic_load_json::<AppConfig>(&path).unwrap_or_default();

        Self {
            config: RwLock::new(config),
            hardware_profile: RwLock::new(None),
            config_save_lock: tokio::sync::Mutex::new(()),
        }
    }
}

/// Retrieve the active application configuration.
#[tauri::command]
pub async fn get_app_config(state: State<'_, AppState>) -> Result<AppConfig, AppError> {
    let guard = match state.config.read() {
        Ok(g) => g,
        Err(e) => {
            return Err(AppError::Internal(format!(
                "Failed to acquire config read lock: {e}"
            )))
        }
    };
    Ok(guard.clone())
}

/// Validate and atomically persist application configuration.
#[tauri::command]
pub async fn save_app_config(
    config: AppConfig,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    // 1. Validate constraints
    config.validate()?;

    // 2. Serialize concurrent saves without holding std::sync::RwLock across blocking disk I/O
    let _save_guard = state.config_save_lock.lock().await;

    // 3. Persist to disk atomically on blocking threadpool
    let config_for_disk = config.clone();
    tokio::task::spawn_blocking(move || {
        let path = resolve_config_file_path();
        atomic_save_json(&path, &config_for_disk)
    })
    .await
    .map_err(|e| AppError::Internal(format!("Config save task panicked: {e}")))??;

    // 4. Update memory state
    let mut guard = match state.config.write() {
        Ok(g) => g,
        Err(e) => {
            return Err(AppError::Internal(format!(
                "Failed to acquire config write lock: {e}"
            )))
        }
    };
    *guard = config;

    Ok(())
}

/// Retrieve the cached hardware profile if previously detected.
#[tauri::command]
pub async fn get_hardware_profile(
    state: State<'_, AppState>,
) -> Result<Option<HardwareProfile>, AppError> {
    let guard = match state.hardware_profile.read() {
        Ok(g) => g,
        Err(e) => {
            return Err(AppError::Internal(format!(
                "Failed to acquire hardware profile read lock: {e}"
            )))
        }
    };
    Ok(guard.clone())
}

/// Probe system hardware capabilities, evaluate recommendation, and cache the profile.
#[tauri::command]
pub async fn detect_hardware(state: State<'_, AppState>) -> Result<HardwareProfile, AppError> {
    let profile = tokio::task::spawn_blocking(probe_system_hardware)
        .await
        .map_err(|e| AppError::Internal(format!("Hardware probe task panicked: {e}")))??;

    let mut guard = match state.hardware_profile.write() {
        Ok(g) => g,
        Err(e) => {
            return Err(AppError::Internal(format!(
                "Failed to acquire hardware profile write lock: {e}"
            )))
        }
    };
    *guard = Some(profile.clone());

    Ok(profile)
}

/// Health check ping/pong command adhering to Result<T, AppError> contract
#[tauri::command]
pub fn ping() -> Result<String, AppError> {
    Ok("pong".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_default_initialization() {
        let state = AppState::default();
        let config_res = state.config.read();
        assert!(config_res.is_ok());
        if let Ok(cfg) = config_res {
            assert_eq!(cfg.theme, "dark");
            assert_eq!(cfg.audio_buffer_size_kb, 64);
        }
        let profile_res = state.hardware_profile.read();
        assert!(profile_res.is_ok());
        if let Ok(prof) = profile_res {
            assert!(prof.is_none());
        }
    }
}
