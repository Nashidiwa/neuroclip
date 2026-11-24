//! Integration tests for Windows Credential Keyring and Zero-Plaintext Security Invariants (`TASK-M4-28`).
//!
//! Covers:
//! - UT-28.2: Keyring failure fallback to in-memory session provider on CI/Sandbox/PlatformFailure.
//! - IT-28.1: Keyring store, retrieve, and delete roundtrip.
//! - ED-28.1: Plaintext audit verifying 0 plaintext API keys exist in config JSON on disk.
//! - AC-28.1 / IT-28.2: API Key connection test SLA <= 2.0s.

use neuroclip_lib::commands::config_cmd::{test_api_key_connection, test_groq_connection};
use neuroclip_lib::config::app_config::AppConfig;
use neuroclip_lib::config::atomic::{atomic_load_json, atomic_save_json};
use neuroclip_lib::core::keyring::KeyringManager;

#[test]
fn test_it_28_1_keyring_store_and_retrieve_roundtrip() {
    let service = format!("NeuroClipTest_{}", std::process::id());
    let manager = KeyringManager::new(&service);
    manager.clear_session_store();

    // Store Groq and Gemini keys
    let groq_secret = "gsk_test_groq_integration_key_111";
    let gemini_secret = "AIzaSyTest_Gemini_Integration_Key_222";

    assert!(manager.set_api_key("groq", groq_secret).is_ok());
    assert!(manager.set_api_key("gemini", gemini_secret).is_ok());

    // Read them back
    let fetched_groq = manager.get_api_key("groq").unwrap();
    assert_eq!(fetched_groq, Some(groq_secret.to_string()));

    let fetched_gemini = manager.get_api_key("gemini").unwrap();
    assert_eq!(fetched_gemini, Some(gemini_secret.to_string()));

    // Delete keys
    assert!(manager.delete_api_key("groq").is_ok());
    assert_eq!(manager.get_api_key("groq").unwrap(), None);

    // Gemini key remains intact
    assert_eq!(
        manager.get_api_key("gemini").unwrap(),
        Some(gemini_secret.to_string())
    );

    // Cleanup
    assert!(manager.delete_api_key("gemini").is_ok());
    manager.clear_session_store();
}

#[test]
fn test_ut_28_2_keyring_platform_failure_fallback() {
    let manager = KeyringManager::new("NeuroClipFallbackTest");
    manager.clear_session_store();

    // 1. Simulate OS / Platform Failure (e.g. 0x520 or Headless CI)
    manager.simulate_platform_failure(true);
    assert!(manager.is_tier2_active());

    // 2. Perform operations - assert zero errors or panics
    let groq_key = "gsk_fallback_session_key_999";
    assert!(manager.set_api_key("groq", groq_key).is_ok());

    // 3. Verify retrieval from in-memory session provider
    let read_back = manager.get_api_key("groq").unwrap();
    assert_eq!(read_back, Some(groq_key.to_string()));

    // 4. Overwrite key in fallback mode
    let updated_key = "gsk_fallback_session_key_updated";
    assert!(manager.set_api_key("groq", updated_key).is_ok());
    assert_eq!(
        manager.get_api_key("groq").unwrap(),
        Some(updated_key.to_string())
    );

    // 5. Delete key in fallback mode
    assert!(manager.delete_api_key("groq").is_ok());
    assert_eq!(manager.get_api_key("groq").unwrap(), None);

    // Reset simulation
    manager.simulate_platform_failure(false);
    manager.clear_session_store();
}

#[test]
fn test_ed_28_1_plaintext_audit_zero_secrets_on_disk() {
    let temp_dir = std::env::temp_dir().join(format!(
        "neuroclip_plaintext_audit_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let config_path = temp_dir.join("app_config.json");

    let groq_plaintext = "gsk_super_confidential_groq_production_key_12345678";
    let gemini_plaintext = "AIzaSySuperConfidentialGeminiProductionKey_87654321";

    let config = AppConfig {
        groq_api_key: groq_plaintext.to_string(),
        gemini_api_key: gemini_plaintext.to_string(),
        is_first_run: true,
        ..Default::default()
    };

    // Save config atomically to disk
    atomic_save_json(&config_path, &config).unwrap();

    // 1. Read raw disk bytes and convert to string
    let raw_disk_content = std::fs::read_to_string(&config_path).unwrap();

    // 2. ED-28.1 ASSERTION: Raw disk content MUST NOT contain plaintext API keys or field names
    assert!(
        !raw_disk_content.contains(groq_plaintext),
        "VIOLATION: Plaintext Groq API key found in saved config file!"
    );
    assert!(
        !raw_disk_content.contains(gemini_plaintext),
        "VIOLATION: Plaintext Gemini API key found in saved config file!"
    );
    assert!(
        !raw_disk_content.contains("groq_api_key"),
        "VIOLATION: groq_api_key field serialized to disk!"
    );
    assert!(
        !raw_disk_content.contains("gemini_api_key"),
        "VIOLATION: gemini_api_key field serialized to disk!"
    );

    // 3. Verify that non-sensitive fields and is_first_run ARE persisted
    assert!(raw_disk_content.contains("\"is_first_run\": true"));
    assert!(raw_disk_content.contains("\"theme\": \"dark\""));

    // 4. Verify deserialization from disk recovers default empty keys (requiring Keyring resolution)
    let reloaded: AppConfig = atomic_load_json(&config_path).unwrap();
    assert_eq!(reloaded.groq_api_key, "");
    assert_eq!(reloaded.gemini_api_key, "");
    assert!(reloaded.is_first_run);

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_ac_28_1_it_28_2_connection_test_sla_under_2_seconds() {
    let start = std::time::Instant::now();

    // Test with mock valid key
    let result =
        test_api_key_connection("groq".to_string(), "mock-valid-groq-key".to_string()).await;
    let elapsed = start.elapsed();

    assert!(result.is_ok(), "Mock connection test should succeed");
    assert!(result.unwrap(), "Connection test returned false");
    assert!(
        elapsed.as_millis() <= 2000,
        "AC-28.1 SLA VIOLATION: Connection test took {} ms, exceeding 2.0s SLA",
        elapsed.as_millis()
    );

    // Convenience wrapper test
    let groq_direct = test_groq_connection("mock-valid-groq-key".to_string()).await;
    assert!(groq_direct.is_ok());

    // Invalid key must return Err
    let invalid_res =
        test_api_key_connection("groq".to_string(), "mock-invalid-key".to_string()).await;
    assert!(invalid_res.is_err(), "Invalid key must return error");

    // Test Gemini connection with mock key under 2.0s SLA
    let gemini_start = std::time::Instant::now();
    let gemini_result =
        test_api_key_connection("gemini".to_string(), "mock-valid-gemini-key".to_string()).await;
    let gemini_elapsed = gemini_start.elapsed();

    assert!(
        gemini_result.is_ok(),
        "Mock Gemini connection test should succeed"
    );
    assert!(
        gemini_result.unwrap(),
        "Gemini connection test returned false"
    );
    assert!(
        gemini_elapsed.as_millis() <= 2000,
        "AC-28.1 SLA VIOLATION: Gemini connection test took {} ms, exceeding 2.0s SLA",
        gemini_elapsed.as_millis()
    );

    // Invalid Gemini key must return Err
    let invalid_gemini =
        test_api_key_connection("gemini".to_string(), "mock-invalid-gemini-key".to_string()).await;
    assert!(
        invalid_gemini.is_err(),
        "Invalid Gemini key must return error"
    );
}
