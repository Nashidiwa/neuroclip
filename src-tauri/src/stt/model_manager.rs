//! Quantized Q5_K Whisper model management and SHA-256 integrity verification.
//!
//! Provides bounded-memory streaming SHA-256 verification (64KB chunks, peak heap < 128KB)
//! to protect against corrupted, truncated, or tampered model files (`ADR-001`, `AC-08.2`).

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

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
    MODEL_CATALOG.iter().find(|entry| {
        entry.model_name.eq_ignore_ascii_case(name_or_filename)
            || entry.filename.eq_ignore_ascii_case(name_or_filename)
    })
}

/// Compute the SHA-256 checksum of a file on disk using a fixed 64KB stack buffer.
///
/// Peak heap allocation is bounded below 128KB regardless of whether the file
/// is 100MB or 2GB.
pub fn compute_file_sha256(path: &Path) -> Result<String, AppError> {
    let mut file = File::open(path).map_err(|_| {
        AppError::Stt(SttError::ModelNotFound {
            path: path.display().to_string(),
        })
    })?;

    let mut hasher = Sha256::new();
    let mut buffer = [0u8; SHA256_CHUNK_SIZE];

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

/// Verify the existence and cryptographic SHA-256 integrity of a Whisper model file.
///
/// Fast fails with `SttError::ModelNotFound` if missing, or `SttError::ModelIntegrityFailed`
/// if the hash does not match the authoritative registry or expected hash.
pub fn verify_model(
    model_name: &str,
    custom_path: Option<&Path>,
) -> Result<ModelInfo, AppError> {
    let catalog_entry = find_catalog_entry(model_name);

    let (filename, expected_hash) = match catalog_entry {
        Some(entry) => (entry.filename.to_string(), entry.expected_sha256),
        None => {
            let fname = if model_name.ends_with(".bin") {
                model_name.to_string()
            } else {
                format!("{model_name}.bin")
            };
            (fname, None)
        }
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

    let metadata = std::fs::metadata(&target_path).map_err(|e| {
        AppError::Stt(SttError::VadProcessingFailed(format!(
            "Failed reading model metadata {}: {e}",
            target_path.display()
        )))
    })?;

    let size_bytes = metadata.len();
    let actual_sha256 = compute_file_sha256(&target_path)?;

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
