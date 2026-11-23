//! Quantized Q5_K Whisper model management and SHA-256 integrity verification.
//!
//! Provides bounded-memory streaming SHA-256 verification (64KB chunks, peak heap < 128KB)
//! to protect against corrupted, truncated, or tampered model files (`ADR-001`, `AC-08.2`).

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::core::storage::validate_vod_id;
use crate::error::{AppError, SttError};

/// 64KB chunk buffer size for streaming SHA-256 computation (< 128KB heap overhead).
pub const SHA256_CHUNK_SIZE: usize = 65536;

/// Metadata information about a verified or queried Whisper model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    /// Human-readable model identifier (e.g. "base.en-q5_k").
    pub model_name: String,
    /// Model binary file name (e.g. "ggml-base.en-q5_k.bin").
    pub filename: String,
    /// Absolute file path on disk.
    pub path: String,
    /// File size in bytes.
    pub size_bytes: u64,
    /// Lowercase hex SHA-256 checksum computed in streaming mode.
    pub sha256: String,
    /// Whether the model passed authoritative integrity validation.
    pub is_verified: bool,
}

/// Catalog entry for an authoritative Whisper quantized model.
#[derive(Debug, Clone, Copy)]
pub struct ModelCatalogEntry {
    pub model_name: &'static str,
    pub filename: &'static str,
    pub expected_sha256: Option<&'static str>,
    pub approx_size_mb: u64,
}

/// Authoritative registry of supported Q5_K / INT8 Whisper models.
pub const MODEL_CATALOG: &[ModelCatalogEntry] = &[
    ModelCatalogEntry {
        model_name: "base.en-q5_k",
        filename: "ggml-base.en-q5_k.bin",
        expected_sha256: Some("587e9140a5a544c688319f07742d4a6c8e3ca638977ca310ab285ccb6b3e7bc2"),
        approx_size_mb: 110,
    },
    ModelCatalogEntry {
        model_name: "small-q5_k",
        filename: "ggml-small-q5_k.bin",
        expected_sha256: Some("c49392bb3fe8a52e1f9a89667e6c3be3dfcb09d2a3c7499691b0f516d2b5ff88"),
        approx_size_mb: 350,
    },
    ModelCatalogEntry {
        model_name: "medium-q5_k",
        filename: "ggml-medium-q5_k.bin",
        expected_sha256: Some("a57c5a6d5ee3df38f2cb4244ab92ec694e9f740e53a3eb2d32782e3c015b6b15"),
        approx_size_mb: 1100,
    },
    // Note: CTranslate2 (`faster-whisper`) models vary across upstream conversions and runtime
    // quantization (`compute_type="int8"` on float16 weights vs third-party pre-quantized artifacts)
    // and NeuroClip does not yet ship a pinned model downloader/manifest, so `expected_sha256` is
    // `None` here while `verify_model` still enforces non-empty (`size_bytes > 0`) integrity.
    ModelCatalogEntry {
        model_name: "faster-whisper-medium-int8",
        filename: "faster-whisper-medium-int8",
        expected_sha256: None,
        approx_size_mb: 1500,
    },
    ModelCatalogEntry {
        model_name: "faster-whisper-small-int8",
        filename: "faster-whisper-small-int8",
        expected_sha256: None,
        approx_size_mb: 500,
    },
];

/// Resolve the directory where local STT models are stored.
///
/// Precedence:
/// 1. `NEUROCLIP_MODELS_DIR` environment variable
/// 2. `%LOCALAPPDATA%/NeuroClip/models`
/// 3. `%USERPROFILE%/AppData/Local/NeuroClip/models`
/// 4. System temp directory / `neuroclip_models`
pub fn resolve_models_dir() -> PathBuf {
    if let Ok(override_dir) = std::env::var("NEUROCLIP_MODELS_DIR") {
        let trimmed = override_dir.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let trimmed = local_app_data.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed).join("NeuroClip").join("models");
        }
    }

    if let Ok(user_profile) = std::env::var("USERPROFILE") {
        let trimmed = user_profile.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed)
                .join("AppData")
                .join("Local")
                .join("NeuroClip")
                .join("models");
        }
    }

    std::env::temp_dir().join("neuroclip_models")
}

/// Look up a model catalog entry by model name or filename.
pub fn find_catalog_entry(name_or_filename: &str) -> Option<&'static ModelCatalogEntry> {
    let lower = name_or_filename.to_ascii_lowercase();
    let normalized = match lower.as_str() {
        "medium-int8" | "medium_int8" => "faster-whisper-medium-int8",
        "small-int8" | "small_int8" => "faster-whisper-small-int8",
        other => other,
    };
    MODEL_CATALOG.iter().find(|entry| {
        entry.model_name.eq_ignore_ascii_case(normalized)
            || entry.filename.eq_ignore_ascii_case(normalized)
            || entry.model_name.eq_ignore_ascii_case(name_or_filename)
            || entry.filename.eq_ignore_ascii_case(name_or_filename)
    })
}

/// Check if a given model identifier represents a Whisper Large variant.
pub fn is_whisper_large(model: &str) -> bool {
    let lower = model.to_ascii_lowercase();
    lower.contains("large")
}

/// Compute the SHA-256 checksum of a file on disk using a bounded 64KB chunk buffer.
///
/// Peak heap allocation is bounded below 128KB regardless of whether the file
/// is 100MB or 2GB.
///
/// # Errors
/// Returns [`AppError::Stt`] if opening or reading the model file fails.
pub fn compute_file_sha256(path: &Path) -> Result<String, AppError> {
    let mut file = File::open(path).map_err(|_| {
        AppError::Stt(SttError::ModelNotFound {
            path: path.display().to_string(),
        })
    })?;

    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; SHA256_CHUNK_SIZE];

    loop {
        let bytes_read = file.read(&mut buffer).map_err(|e| {
            AppError::Stt(SttError::VadProcessingFailed(format!(
                "Failed reading model file {}: {e}",
                path.display()
            )))
        })?;

        if bytes_read == 0 {
            break;
        }

        hasher.update(&buffer[..bytes_read]);
    }

    let result = hasher.finalize();
    Ok(format!("{result:x}"))
}

fn resolve_uncatalogued_model_filename(
    model_name: &str,
    enforce_single_component: bool,
) -> Result<String, AppError> {
    if enforce_single_component {
        let safe_name = validate_vod_id(model_name).map_err(|_| {
            AppError::Stt(SttError::ModelNotFound {
                path: model_name.to_string(),
            })
        })?;
        let mut comps = Path::new(safe_name).components();
        let is_single_normal =
            matches!(comps.next(), Some(std::path::Component::Normal(_))) && comps.next().is_none();
        if !is_single_normal {
            return Err(AppError::Stt(SttError::ModelNotFound {
                path: model_name.to_string(),
            }));
        }
    }
    let clean_name = model_name.trim();
    let is_bin = Path::new(clean_name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("bin"));
    Ok(if is_bin {
        clean_name.to_string()
    } else {
        format!("{clean_name}.bin")
    })
}

/// Verify the existence and cryptographic SHA-256 integrity of a Whisper model file.
///
/// Fast fails with `SttError::ModelNotFound` if missing, or `SttError::ModelIntegrityFailed`
/// if the file is 0 bytes or the hash does not match the authoritative registry.
///
/// # Errors
/// Returns [`AppError::Stt`] if the model is missing, empty, unreadable, or fails hash verification.
pub fn verify_model(model_name: &str, custom_path: Option<&Path>) -> Result<ModelInfo, AppError> {
    let catalog_entry = find_catalog_entry(model_name);

    let (filename, expected_hash) = if let Some(entry) = catalog_entry {
        (entry.filename.to_string(), entry.expected_sha256)
    } else {
        (
            resolve_uncatalogued_model_filename(model_name, custom_path.is_none())?,
            None,
        )
    };

    let target_path = match custom_path {
        Some(path) => path.to_path_buf(),
        None => resolve_models_dir().join(&filename),
    };

    if !target_path.exists() {
        return Err(AppError::Stt(SttError::ModelNotFound {
            path: target_path.display().to_string(),
        }));
    }

    if target_path.is_dir() {
        // CTranslate2 / Faster-Whisper directory layout containing model.bin
        let model_bin_path = target_path.join("model.bin");
        if !model_bin_path.exists() {
            return Err(AppError::Stt(SttError::ModelNotFound {
                path: model_bin_path.display().to_string(),
            }));
        }

        let metadata = std::fs::metadata(&model_bin_path).map_err(|e| {
            AppError::Stt(SttError::VadProcessingFailed(format!(
                "Failed reading model metadata {}: {e}",
                model_bin_path.display()
            )))
        })?;

        let size_bytes = metadata.len();
        let actual_sha256 = compute_file_sha256(&model_bin_path)?;

        if size_bytes == 0 {
            return Err(AppError::Stt(SttError::ModelIntegrityFailed {
                path: model_bin_path.display().to_string(),
                expected_sha256: expected_hash.unwrap_or("non-empty-binary").to_string(),
                actual_sha256,
            }));
        }

        if let Some(expected) = expected_hash {
            if !actual_sha256.eq_ignore_ascii_case(expected) {
                return Err(AppError::Stt(SttError::ModelIntegrityFailed {
                    path: model_bin_path.display().to_string(),
                    expected_sha256: expected.to_string(),
                    actual_sha256,
                }));
            }
        }

        return Ok(ModelInfo {
            model_name: model_name.to_string(),
            filename,
            path: target_path.display().to_string(),
            size_bytes,
            sha256: actual_sha256,
            is_verified: true,
        });
    }

    let metadata = std::fs::metadata(&target_path).map_err(|e| {
        AppError::Stt(SttError::VadProcessingFailed(format!(
            "Failed reading model metadata {}: {e}",
            target_path.display()
        )))
    })?;

    let size_bytes = metadata.len();
    let actual_sha256 = compute_file_sha256(&target_path)?;

    if size_bytes == 0 {
        return Err(AppError::Stt(SttError::ModelIntegrityFailed {
            path: target_path.display().to_string(),
            expected_sha256: expected_hash.unwrap_or("non-empty-binary").to_string(),
            actual_sha256,
        }));
    }

    if let Some(expected) = expected_hash {
        if !actual_sha256.eq_ignore_ascii_case(expected) {
            return Err(AppError::Stt(SttError::ModelIntegrityFailed {
                path: target_path.display().to_string(),
                expected_sha256: expected.to_string(),
                actual_sha256,
            }));
        }
    }

    Ok(ModelInfo {
        model_name: model_name.to_string(),
        filename,
        path: target_path.display().to_string(),
        size_bytes,
        sha256: actual_sha256,
        is_verified: true,
    })
}
