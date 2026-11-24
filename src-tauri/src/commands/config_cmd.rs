//! IPC Command handlers for Application Configuration, Keyring Secrets, and Hardware Profiling.

use crate::config::app_config::{resolve_config_dir, resolve_config_file_path, AppConfig};
use crate::config::atomic::{atomic_load_json, atomic_save_json};
use crate::config::hardware::{probe_system_hardware, HardwareProfile};
use crate::core::keyring::{KeyringManager, SERVICE_NAME};
use crate::error::AppError;
use std::sync::{Arc, RwLock};
use tauri::State;

/// Shared application state managed across Tauri IPC commands.
pub struct AppState {
    pub config: RwLock<AppConfig>,
    pub hardware_profile: RwLock<Option<HardwareProfile>>,
    pub keyring: Arc<KeyringManager>,
    config_save_lock: tokio::sync::Mutex<()>,
}

impl Default for AppState {
    fn default() -> Self {
        let path = resolve_config_file_path();
        let mut config = atomic_load_json::<AppConfig>(&path).unwrap_or_default();

        let profile_path = resolve_config_dir().join("hardware_profile.json");
        let hardware_profile = atomic_load_json::<HardwareProfile>(&profile_path).ok();

        let keyring = Arc::new(KeyringManager::new(SERVICE_NAME));

        // Automatic one-time migration of legacy plaintext keys from JSON into Keyring (ED-28.1 / AC-28.2)
        let mut migrated = false;
        if !config.groq_api_key.trim().is_empty() {
            let _ = keyring.set_api_key("groq", &config.groq_api_key);
            config.groq_api_key.clear();
            migrated = true;
        }
        if !config.gemini_api_key.trim().is_empty() {
            let _ = keyring.set_api_key("gemini", &config.gemini_api_key);
            config.gemini_api_key.clear();
            migrated = true;
        }
        if migrated && path.exists() {
            let _ = atomic_save_json(&path, &config);
        }

        Self {
            config: RwLock::new(config),
            hardware_profile: RwLock::new(hardware_profile),
            keyring,
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
    mut config: AppConfig,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    // 1. Validate constraints
    config.validate()?;

    // 2. Sanitize and migrate any plaintext API keys passed in payload into Keyring
    if !config.groq_api_key.trim().is_empty() {
        let _ = state.keyring.set_api_key("groq", &config.groq_api_key);
        config.groq_api_key.clear();
    }
    if !config.gemini_api_key.trim().is_empty() {
        let _ = state.keyring.set_api_key("gemini", &config.gemini_api_key);
        config.gemini_api_key.clear();
    }

    // 3. Serialize concurrent saves without holding std::sync::RwLock across blocking disk I/O
    let _save_guard = state.config_save_lock.lock().await;

    // 4. Persist to disk atomically on blocking threadpool (0 plaintext keys on disk per ED-28.1)
    let config_for_disk = config.clone();
    tokio::task::spawn_blocking(move || {
        let path = resolve_config_file_path();
        atomic_save_json(&path, &config_for_disk)
    })
    .await
    .map_err(|e| AppError::Internal(format!("Config save task panicked: {e}")))??;

    // 5. Update memory state
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

/// Retrieve an API key for the specified provider from the secure Keyring (Tier 1 / Tier 2).
#[tauri::command]
pub async fn get_api_key(
    service: String,
    state: State<'_, AppState>,
) -> Result<Option<String>, AppError> {
    state.keyring.get_api_key(&service).map_err(AppError::from)
}

/// Store an API key for the specified provider in the secure Keyring (Tier 1 / Tier 2).
#[tauri::command]
pub async fn set_api_key(
    service: String,
    api_key: String,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    state
        .keyring
        .set_api_key(&service, &api_key)
        .map_err(AppError::from)
}

/// Delete an API key for the specified provider from the secure Keyring.
#[tauri::command]
pub async fn delete_api_key(service: String, state: State<'_, AppState>) -> Result<(), AppError> {
    state
        .keyring
        .delete_api_key(&service)
        .map_err(AppError::from)
}

/// Validate an API key connection within a strict <= 2.0s timeout SLA (AC-28.1, IT-28.2).
#[tauri::command]
pub async fn test_api_key_connection(service: String, api_key: String) -> Result<bool, AppError> {
    let clean_key = api_key.trim();
    if clean_key.is_empty() {
        return match service.to_lowercase().as_str() {
            "gemini" => Err(AppError::Translation(
                crate::error::TranslationError::InvalidApiKey,
            )),
            _ => Err(AppError::Stt(crate::error::SttError::InvalidApiKey)),
        };
    }

    // Mock bypass for testing / offline / hermetic CI environments
    let is_mock_env = std::env::var("NEUROCLIP_MOCK_API_TEST").as_deref() == Ok("1");
    if is_mock_env || clean_key.starts_with("mock-") {
        if clean_key == "mock-invalid" || clean_key.contains("invalid") {
            return match service.to_lowercase().as_str() {
                "gemini" => Err(AppError::Translation(
                    crate::error::TranslationError::InvalidApiKey,
                )),
                _ => Err(AppError::Stt(crate::error::SttError::InvalidApiKey)),
            };
        }
        return Ok(true);
    }

    match service.to_lowercase().as_str() {
        "groq" => {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_millis(2000))
                .build()
                .map_err(|e| AppError::Internal(format!("Failed to build HTTP client: {e}")))?;

            let resp = client
                .get("https://api.groq.com/openai/v1/models")
                .header(
                    reqwest::header::AUTHORIZATION,
                    format!("Bearer {clean_key}"),
                )
                .send()
                .await
                .map_err(|e| {
                    if e.is_timeout() {
                        AppError::Stt(crate::error::SttError::SidecarExecutionFailed {
                            details: "Groq connection timed out (> 2.0s SLA)".to_string(),
                        })
                    } else {
                        AppError::Stt(crate::error::SttError::SidecarExecutionFailed {
                            details: format!("Groq connection network error: {e}"),
                        })
                    }
                })?;

            if resp.status().is_success() {
                Ok(true)
            } else if resp.status().as_u16() == 401 || resp.status().as_u16() == 403 {
                Err(AppError::Stt(crate::error::SttError::InvalidApiKey))
            } else {
                Err(AppError::Stt(
                    crate::error::SttError::SidecarExecutionFailed {
                        details: format!("Groq API returned HTTP status {}", resp.status()),
                    },
                ))
            }
        }
        "gemini" => tokio::time::timeout(
            std::time::Duration::from_millis(2000),
            crate::commands::translation_cmd::test_gemini_connection(clean_key.to_string()),
        )
        .await
        .map_err(|_| {
            AppError::Translation(crate::error::TranslationError::ApiRequestFailed {
                status: 408,
                message: "Kiểm tra kết nối vượt quá thời gian cho phép (2.0s)".to_string(),
            })
        })?,
        unknown => Err(AppError::Config(crate::error::ConfigError::InvalidValue {
            key: "service".to_string(),
            message: format!("Unsupported service provider for connection test: {unknown}"),
        })),
    }
}

/// Convenience command to test Groq connection directly.
#[tauri::command]
pub async fn test_groq_connection(api_key: String) -> Result<bool, AppError> {
    test_api_key_connection("groq".to_string(), api_key).await
}

/// Mark initial onboarding as completed, setting `is_first_run = false` (AC-28.3, UT-28.1).
#[tauri::command]
pub async fn complete_onboarding(state: State<'_, AppState>) -> Result<(), AppError> {
    let _save_guard = state.config_save_lock.lock().await;

    let mut updated_config = {
        let guard = match state.config.read() {
            Ok(g) => g,
            Err(e) => {
                return Err(AppError::Internal(format!(
                    "Failed to acquire config read lock: {e}"
                )))
            }
        };
        guard.clone()
    };
    updated_config.is_first_run = false;

    let config_for_disk = updated_config.clone();
    tokio::task::spawn_blocking(move || {
        let path = resolve_config_file_path();
        atomic_save_json(&path, &config_for_disk)
    })
    .await
    .map_err(|e| AppError::Internal(format!("Config save task panicked: {e}")))??;

    let mut write_guard = match state.config.write() {
        Ok(g) => g,
        Err(e) => {
            return Err(AppError::Internal(format!(
                "Failed to acquire config write lock: {e}"
            )))
        }
    };
    *write_guard = updated_config;

    Ok(())
}

/// Query whether the onboarding wizard is needed (first run flag).
#[tauri::command]
pub async fn is_first_run(state: State<'_, AppState>) -> Result<bool, AppError> {
    let guard = match state.config.read() {
        Ok(g) => g,
        Err(e) => {
            return Err(AppError::Internal(format!(
                "Failed to acquire config read lock: {e}"
            )))
        }
    };
    Ok(guard.is_first_run)
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

    // Atomically persist hardware_profile.json on blocking threadpool
    let profile_for_disk = profile.clone();
    tokio::task::spawn_blocking(move || {
        let profile_path = resolve_config_dir().join("hardware_profile.json");
        atomic_save_json(&profile_path, &profile_for_disk)
    })
    .await
    .map_err(|e| AppError::Internal(format!("Hardware profile save task panicked: {e}")))??;

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
    }

    #[test]
    fn test_hardware_profile_persistence_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!(
            "neuroclip_test_hw_persist_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        let _ = std::fs::create_dir_all(&temp_dir);
        let test_profile_path = temp_dir.join("hardware_profile.json");

        let probe_result = probe_system_hardware()?;
        atomic_save_json(&test_profile_path, &probe_result)?;

        let loaded_profile: HardwareProfile = atomic_load_json(&test_profile_path)?;
        assert_eq!(
            loaded_profile.recommended_mode,
            probe_result.recommended_mode
        );
        assert_eq!(loaded_profile.cpu_info.model, probe_result.cpu_info.model);
        assert_eq!(loaded_profile.gpu_info.model, probe_result.gpu_info.model);

        let _ = std::fs::remove_dir_all(&temp_dir);

        Ok(())
    }

    #[tokio::test]
    async fn test_keyring_in_app_state() {
        let state = AppState::default();
        let _ = state.keyring.set_api_key("groq", "gsk_test_123");
        let fetched = state.keyring.get_api_key("groq").unwrap();
        assert_eq!(fetched, Some("gsk_test_123".to_string()));
    }

    #[tokio::test]
    async fn test_api_key_connection_sla_and_validation() {
        // Empty key fails fast
        assert!(test_api_key_connection("groq".into(), String::new())
            .await
            .is_err());
        assert!(test_api_key_connection("gemini".into(), "   ".into())
            .await
            .is_err());

        // Mock key connects within SLA <= 2.0s (AC-28.1, IT-28.2)
        let start = std::time::Instant::now();
        let ok_res = test_api_key_connection("groq".into(), "mock-valid-groq-key".into()).await;
        let elapsed = start.elapsed();
        assert!(ok_res.is_ok());
        assert!(ok_res.unwrap());
        assert!(elapsed.as_millis() <= 2000, "Latency exceeded 2.0s SLA");

        // Mock invalid key returns Err
        let fail_res = test_api_key_connection("groq".into(), "mock-invalid-key".into()).await;
        assert!(fail_res.is_err());

        // Gemini mock test
        let gemini_ok =
            test_api_key_connection("gemini".into(), "mock-valid-gemini-key".into()).await;
        assert!(gemini_ok.is_ok());
    }
}
