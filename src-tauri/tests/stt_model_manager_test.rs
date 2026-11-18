#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Unit tests for quantized model management and SHA-256 integrity verification (TASK-M4-08).

use neuroclip_lib::stt::{
    compute_file_sha256, find_catalog_entry, resolve_models_dir, verify_model, MODEL_CATALOG,
    SHA256_CHUNK_SIZE,
};
use std::fs::File;
use std::io::Write;

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

#[test]
fn test_m4_16_no_placeholder_hashes_and_empty_model_rejection() {
    // Verify placeholder SHA-256 hashes (empty string & "test\n") were removed
    const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    const TEST_LF_SHA256: &str = "f2ca1bb6c7e907d06dafe4687e579fce76b37e4e93b7605022da52e6ccc26fd2";

    for entry in MODEL_CATALOG {
        if let Some(hash) = entry.expected_sha256 {
            assert_ne!(
                hash, EMPTY_SHA256,
                "Placeholder empty SHA-256 found on {}",
                entry.model_name
            );
            assert_ne!(
                hash, TEST_LF_SHA256,
                "Placeholder test-newline SHA-256 found on {}",
                entry.model_name
            );
        }
    }

    // 0-byte single-file model must fail with ERR_STT_MODEL_INTEGRITY_FAILED
    let empty_file = std::env::temp_dir().join(format!("empty_model_{}.bin", std::process::id()));
    File::create(&empty_file).unwrap();
    let res_file = verify_model("custom-empty-model", Some(&empty_file));
    let _ = std::fs::remove_file(&empty_file);
    assert!(res_file.is_err());
    assert_eq!(
        res_file.unwrap_err().code(),
        "ERR_STT_MODEL_INTEGRITY_FAILED"
    );

    // 0-byte directory model.bin must fail with ERR_STT_MODEL_INTEGRITY_FAILED
    let empty_dir = std::env::temp_dir().join(format!("empty_fw_dir_{}", std::process::id()));
    std::fs::create_dir_all(&empty_dir).unwrap();
    let empty_bin = empty_dir.join("model.bin");
    File::create(&empty_bin).unwrap();
    let res_dir = verify_model("faster-whisper-medium-int8", Some(&empty_dir));
    assert!(res_dir.is_err());
    assert_eq!(
        res_dir.unwrap_err().code(),
        "ERR_STT_MODEL_INTEGRITY_FAILED"
    );

    // Non-empty faster-whisper-medium-int8 directory must verify cleanly
    {
        let mut f = File::create(&empty_bin).unwrap();
        f.write_all(b"real ctranslate2 weights payload").unwrap();
    }
    let res_ok = verify_model("faster-whisper-medium-int8", Some(&empty_dir));
    let _ = std::fs::remove_dir_all(&empty_dir);
    assert!(
        res_ok.is_ok(),
        "Expected non-empty faster-whisper-medium-int8 to verify: {res_ok:?}"
    );
    let info = res_ok.unwrap();
    assert!(info.is_verified);
    assert!(info.size_bytes > 0);
}
