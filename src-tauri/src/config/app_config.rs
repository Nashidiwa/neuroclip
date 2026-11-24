//! Global application configuration and default settings for NeuroClip.
//!
//! Controls user preferences, AI service models, sidecar binaries,
//! and buffer size constraints.

use crate::error::ConfigError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Paths to external managed sidecar executables.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct SidecarPaths {
    pub ffmpeg_path: Option<String>,
    pub ytdlp_path: Option<String>,
    pub whisper_worker_path: Option<String>,
}

/// Global Application Configuration for NeuroClip.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct AppConfig {
    /// UI Theme mode (e.g. "dark", "light", "acrylic").
    pub theme: String,
    /// UI display language (ISO 639-1 code, e.g. "vi", "en").
    pub language: String,
    /// Opt-in anonymous telemetry (default false per Core Invariant #5).
    pub telemetry_enabled: bool,
    /// Whether managed sidecar binaries auto-check for updates.
    pub auto_update_sidecars: bool,
    /// Export directory for rendered clips and subtitle projects.
    pub output_directory: String,
    /// Temporary cache directory for audio chunks and intermediate files.
    pub cache_directory: String,
    /// Streaming audio buffer size in kilobytes (default 64KB per ADR-002 & CONVENTIONS §2.3).
    pub audio_buffer_size_kb: usize,
    /// Active Speech-to-Text provider ("groq", "local_whisper", "cloud_byok").
    pub active_stt_provider: String,
    /// Active STT model name ("whisper-large-v3", "medium-int8").
    pub active_stt_model: String,
    /// Active Translation LLM model ("gemini-1.5-flash").
    pub active_translation_model: String,
    /// Optional custom sidecar paths.
    pub sidecar_paths: SidecarPaths,
    /// Whether this is the first run of the application (triggers onboarding wizard).
    #[serde(default = "default_first_run")]
    pub is_first_run: bool,
    /// User BYOK Groq API Key (not serialized to disk per AC-28.2 / ED-28.1).
    #[serde(default, skip_serializing)]
    pub groq_api_key: String,
    /// User BYOK Google Gemini API Key (not serialized to disk per AC-28.2 / ED-28.1).
    #[serde(default, skip_serializing)]
    pub gemini_api_key: String,
    /// Days to retain rotated log files and temporary cache items.
    pub max_history_days: u32,
}

fn default_first_run() -> bool {
    true
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            language: "vi".to_string(),
            telemetry_enabled: false,
            auto_update_sidecars: true,
            output_directory: default_output_dir(),
            cache_directory: default_cache_dir(),
            audio_buffer_size_kb: 64,
            active_stt_provider: "groq".to_string(),
            active_stt_model: "whisper-large-v3".to_string(),
            active_translation_model: "gemini-1.5-flash".to_string(),
            sidecar_paths: SidecarPaths::default(),
            is_first_run: true,
            groq_api_key: String::new(),
            gemini_api_key: String::new(),
            max_history_days: 14,
        }
    }
}

impl AppConfig {
    /// Validate configuration constraints.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.audio_buffer_size_kb == 0 {
            return Err(ConfigError::InvalidValue {
                key: "audio_buffer_size_kb".to_string(),
                message: "Dung lượng đệm âm thanh phải lớn hơn 0 KB".to_string(),
            });
        }
        if self.theme.trim().is_empty() {
            return Err(ConfigError::InvalidValue {
                key: "theme".to_string(),
                message: "Chủ đề giao diện không được để trống".to_string(),
            });
        }
        if self.language.trim().is_empty() {
            return Err(ConfigError::InvalidValue {
                key: "language".to_string(),
                message: "Ngôn ngữ giao diện không được để trống".to_string(),
            });
        }
        Ok(())
    }
}

/// Resolve canonical `%APPDATA%/NeuroClip/app_config.json` without panicking.
#[must_use]
pub fn resolve_config_file_path() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        return PathBuf::from(appdata)
            .join("NeuroClip")
            .join("app_config.json");
    }
    if let Ok(userprofile) = std::env::var("USERPROFILE") {
        return PathBuf::from(userprofile)
            .join("AppData")
            .join("Roaming")
            .join("NeuroClip")
            .join("app_config.json");
    }
    PathBuf::from(".neuroclip").join("app_config.json")
}

/// Resolve canonical `%APPDATA%/NeuroClip/` directory.
#[must_use]
pub fn resolve_config_dir() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        return PathBuf::from(appdata).join("NeuroClip");
    }
    if let Ok(userprofile) = std::env::var("USERPROFILE") {
        return PathBuf::from(userprofile)
            .join("AppData")
            .join("Roaming")
            .join("NeuroClip");
    }
    PathBuf::from(".neuroclip")
}

fn default_output_dir() -> String {
    if let Ok(userprofile) = std::env::var("USERPROFILE") {
        return PathBuf::from(userprofile)
            .join("Videos")
            .join("NeuroClip")
            .to_string_lossy()
            .to_string();
    }
    "./output".to_string()
}

fn default_cache_dir() -> String {
    if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
        return PathBuf::from(localappdata)
            .join("NeuroClip")
            .join("cache")
            .to_string_lossy()
            .to_string();
    }
    "./cache".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_invariants() {
        let config = AppConfig::default();
        assert_eq!(config.theme, "dark");
        assert_eq!(config.language, "vi");
        assert!(!config.telemetry_enabled);
        assert!(config.auto_update_sidecars);
        assert_eq!(config.audio_buffer_size_kb, 64);
        assert_eq!(config.active_stt_provider, "groq");
        assert_eq!(config.max_history_days, 14);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validation_fails_on_zero_buffer_size() {
        let config = AppConfig {
            audio_buffer_size_kb: 0,
            ..Default::default()
        };
        let res = config.validate();
        assert!(matches!(res, Err(ConfigError::InvalidValue { .. })));
    }

    #[test]
    fn test_resolve_config_file_path_does_not_panic() {
        let path = resolve_config_file_path();
        assert!(path.to_string_lossy().contains("app_config.json"));
    }

    #[test]
    fn test_is_first_run_default_and_serialization() {
        let config = AppConfig::default();
        assert!(config.is_first_run);

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"is_first_run\":true"));

        // When deserializing JSON without is_first_run, defaults to true
        let without_flag: AppConfig = serde_json::from_str("{}").unwrap_or_default();
        assert!(without_flag.is_first_run);
    }

    #[test]
    fn test_zero_plaintext_keys_serialized_ed_28_1() {
        let config = AppConfig {
            groq_api_key: "gsk_super_secret_groq_key_999".to_string(),
            gemini_api_key: "AIzaSySuperSecretGeminiKey_888".to_string(),
            ..Default::default()
        };

        let json_str = serde_json::to_string_pretty(&config).unwrap();

        // ED-28.1: 0 plaintext API keys serialized
        assert!(!json_str.contains("groq_api_key"));
        assert!(!json_str.contains("gemini_api_key"));
        assert!(!json_str.contains("gsk_super_secret_groq_key_999"));
        assert!(!json_str.contains("AIzaSySuperSecretGeminiKey_888"));
    }

    #[test]
    fn test_legacy_config_deserialization_for_migration() {
        let legacy_json = r#"{
            "theme": "dark",
            "language": "vi",
            "groq_api_key": "gsk_legacy_groq_123",
            "gemini_api_key": "AIzaSyLegacyGemini_456"
        }"#;

        let parsed: AppConfig = serde_json::from_str(legacy_json).unwrap();
        assert_eq!(parsed.groq_api_key, "gsk_legacy_groq_123");
        assert_eq!(parsed.gemini_api_key, "AIzaSyLegacyGemini_456");
        assert!(parsed.is_first_run);
    }
}
