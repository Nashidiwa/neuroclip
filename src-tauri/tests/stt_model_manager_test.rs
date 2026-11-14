#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Unit tests for quantized model management and SHA-256 integrity verification (TASK-M4-08).

use std::fs::File;
use std::io::Write;
use neuroclip_lib::stt::{
    compute_file_sha256, find_catalog_entry, resolve_models_dir, verify_model,
    SHA256_CHUNK_SIZE,
};

#[test]
fn test_ut_08_6_missing_model_triggers_err_stt_model_not_found() {
    let non_existent_path = std::env::temp_dir().join("definitely_missing_whisper_model.bin");
    if non_existent_path.exists() {
        let _ = std::fs::remove_file(&non_existent_path);
    }

    let res = verify_model("base.en-q5_k", Some(&non_existent_path));
    assert!(res.is_err(), "Expected error for non-existent model");

    let err = res.unwrap_err();
    assert_eq!(err.code(), "ERR_STT_MODEL_NOT_FOUND");
}

#[test]
fn test_ut_08_7_corrupted_model_triggers_err_stt_model_integrity_failed() {
    let temp_file = std::env::temp_dir().join("corrupted_ggml_model.bin");
    {
        let mut f = File::create(&temp_file).unwrap();
        f.write_all(b"corrupted or truncated binary data").unwrap();
    }

    // Verify against a known model in the catalog
    let res = verify_model("base.en-q5_k", Some(&temp_file));
    assert!(res.is_err(), "Expected integrity check failure");

    let err = res.unwrap_err();
    assert_eq!(err.code(), "ERR_STT_MODEL_INTEGRITY_FAILED");

    // Clean up
    let _ = std::fs::remove_file(&temp_file);
}

#[test]
fn test_ut_08_8_valid_sha256_matches_synthetic_file() {
    let temp_file = std::env::temp_dir().join("valid_synthetic_model.bin");
    let test_data = b"neuroclip quantized whisper model test data 12345";
    {
        let mut f = File::create(&temp_file).unwrap();
        f.write_all(test_data).unwrap();
    }

    let actual_sha256 = compute_file_sha256(&temp_file).unwrap();

    // Independent sha256 check
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(test_data);
    let expected_sha256 = format!("{:x}", hasher.finalize());

    assert_eq!(actual_sha256, expected_sha256);

    let _ = std::fs::remove_file(&temp_file);
}

#[test]
fn test_ut_08_9_bounded_streaming_verification_multi_megabyte() {
    // Generate file larger than SHA256_CHUNK_SIZE (64KB)
    let temp_file = std::env::temp_dir().join("large_streaming_model.bin");
    let _total_size = SHA256_CHUNK_SIZE * 3 + 1024; // ~193KB
    {
        let mut f = File::create(&temp_file).unwrap();
        let chunk = vec![0x42u8; SHA256_CHUNK_SIZE];
        for _ in 0..3 {
            f.write_all(&chunk).unwrap();
        }
        f.write_all(&vec![0x42u8; 1024]).unwrap();
    }

    let computed_hash = compute_file_sha256(&temp_file).unwrap();
    assert!(!computed_hash.is_empty());
    assert_eq!(computed_hash.len(), 64); // standard 32-byte hex

    let _ = std::fs::remove_file(&temp_file);
}

#[test]
fn test_catalog_and_path_resolution() {
    let entry = find_catalog_entry("base.en-q5_k");
    assert!(entry.is_some());
    let entry = entry.unwrap();
    assert_eq!(entry.filename, "ggml-base.en-q5_k.bin");
    assert!(entry.expected_sha256.is_some());

    // By filename lookup
    assert!(find_catalog_entry("ggml-small-q5_k.bin").is_some());

    // Unknown returns None
    assert!(find_catalog_entry("nonexistent-model").is_none());

    // Resolve models dir
    let dir = resolve_models_dir();
    assert!(!dir.to_string_lossy().is_empty());
}
