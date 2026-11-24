//! Windows Credential Locker with In-Memory Session Fallback Provider.
//!
//! Provides a two-tier secure credential storage system:
//! - **Tier 1 (Primary)**: Windows Credential Manager (`wincred` via `keyring-rs`),
//!   storing API keys in the OS vault without persisting plaintext secrets to disk (AC-28.2).
//! - **Tier 2 (Fallback Provider)**: In-Memory Session Credential Provider
//!   (`Arc<RwLock<HashMap<String, String>>>`) for Headless CI / Windows Sandbox environments
//!   without interactive logon sessions (e.g. error `0x520 ERROR_NO_SUCH_LOGON_SESSION`
//!   or `PlatformFailure`) (AC-28.4, UT-28.2).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock, RwLock};

/// Canonical service identifier for NeuroClip in Windows Credential Manager.
pub const SERVICE_NAME: &str = "NeuroClip";

/// Global process-wide fallback storage ensuring shared in-memory credentials across handles in headless/fallback mode.
static GLOBAL_FALLBACK_STORE: OnceLock<Arc<RwLock<HashMap<String, String>>>> = OnceLock::new();

fn get_global_fallback_store() -> &'static Arc<RwLock<HashMap<String, String>>> {
    GLOBAL_FALLBACK_STORE.get_or_init(|| Arc::new(RwLock::new(HashMap::new())))
}

/// Errors occurring during keyring operations.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum KeyringError {
    #[error("Lỗi nền tảng Windows Credential: {0}")]
    PlatformFailure(String),
    #[error("Tên dịch vụ hoặc provider không hợp lệ: {0}")]
    InvalidInput(String),
    #[error("Thao tác khóa bảo mật thất bại: {0}")]
    Other(String),
}

impl From<KeyringError> for crate::error::AppError {
    fn from(err: KeyringError) -> Self {
        match err {
            KeyringError::InvalidInput(msg) => {
                crate::error::AppError::Config(crate::error::ConfigError::InvalidValue {
                    key: "api_key".to_string(),
                    message: msg,
                })
            }
            KeyringError::PlatformFailure(msg) | KeyringError::Other(msg) => {
                crate::error::AppError::Internal(format!("Lỗi bảo mật Keyring: {msg}"))
            }
        }
    }
}

/// Two-Tier Credential Manager for secure API Key storage.
#[derive(Debug, Clone)]
pub struct KeyringManager {
    service: String,
    in_memory_store: Arc<RwLock<HashMap<String, String>>>,
    tier2_active: Arc<AtomicBool>,
    simulate_failure: Arc<AtomicBool>,
}

impl KeyringManager {
    /// Create a new `KeyringManager` instance for the given service.
    #[must_use]
    pub fn new(service: impl Into<String>) -> Self {
        let env_force_fallback = std::env::var("NEUROCLIP_KEYRING_FALLBACK").as_deref() == Ok("1")
            || std::env::var("NEUROCLIP_MOCK_KEYRING").as_deref() == Ok("1");

        Self {
            service: service.into(),
            in_memory_store: Arc::clone(get_global_fallback_store()),
            tier2_active: Arc::new(AtomicBool::new(env_force_fallback)),
            simulate_failure: Arc::new(AtomicBool::new(false)),
        }
    }

    #[inline]
    fn scoped_key(&self, provider: &str) -> String {
        format!("{}:{}", self.service, provider)
    }

    /// Retrieve an API key for the specified provider (e.g. "groq", "gemini").
    ///
    /// Checks Windows Credential Locker (Tier 1), falling back to In-Memory Session Provider (Tier 2)
    /// if unavailable or on simulated platform failure.
    pub fn get_api_key(&self, provider: &str) -> Result<Option<String>, KeyringError> {
        let clean_provider = provider.trim();
        if clean_provider.is_empty() {
            return Err(KeyringError::InvalidInput(
                "Provider name cannot be empty".to_string(),
            ));
        }

        // If failure simulation is requested or Tier 2 is already active, read from in-memory store
        if self.simulate_failure.load(Ordering::SeqCst) || self.tier2_active.load(Ordering::SeqCst)
        {
            return Ok(self.get_from_tier2(clean_provider));
        }

        #[cfg(windows)]
        {
            match keyring::Entry::new(&self.service, clean_provider) {
                Ok(entry) => match entry.get_password() {
                    Ok(secret) => Ok(Some(secret)),
                    Err(keyring::Error::NoEntry) => {
                        // Key not found in Windows Credential Locker. Check if present in session store
                        Ok(self.get_from_tier2(clean_provider))
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Keyring Tier 1 read failed for provider '{clean_provider}', falling back to Tier 2: {e}"
                        );
                        self.tier2_active.store(true, Ordering::SeqCst);
                        Ok(self.get_from_tier2(clean_provider))
                    }
                },
                Err(e) => {
                    tracing::warn!(
                        "Keyring Tier 1 entry creation failed for provider '{clean_provider}', falling back to Tier 2: {e}"
                    );
                    self.tier2_active.store(true, Ordering::SeqCst);
                    Ok(self.get_from_tier2(clean_provider))
                }
            }
        }

        #[cfg(not(windows))]
        {
            Ok(self.get_from_tier2(clean_provider))
        }
    }

    /// Store an API key for the specified provider.
    ///
    /// Saves into Windows Credential Locker (Tier 1) and caches in memory.
    /// If Tier 1 encounters platform failure, switches seamlessly to Tier 2 (AC-28.4).
    pub fn set_api_key(&self, provider: &str, secret: &str) -> Result<(), KeyringError> {
        let clean_provider = provider.trim();
        if clean_provider.is_empty() {
            return Err(KeyringError::InvalidInput(
                "Provider name cannot be empty".to_string(),
            ));
        }

        let clean_secret = secret.trim();

        // If simulating failure or Tier 2 active, write directly to in-memory store
        if self.simulate_failure.load(Ordering::SeqCst) || self.tier2_active.load(Ordering::SeqCst)
        {
            self.set_in_tier2(clean_provider, clean_secret);
            return Ok(());
        }

        #[cfg(windows)]
        {
            match keyring::Entry::new(&self.service, clean_provider) {
                Ok(entry) => match entry.set_password(clean_secret) {
                    Ok(()) => {
                        // Also update in-memory cache for fast session access
                        self.set_in_tier2(clean_provider, clean_secret);
                        Ok(())
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Keyring Tier 1 write failed for provider '{clean_provider}', activating Tier 2: {e}"
                        );
                        self.tier2_active.store(true, Ordering::SeqCst);
                        self.set_in_tier2(clean_provider, clean_secret);
                        Ok(())
                    }
                },
                Err(e) => {
                    tracing::warn!(
                        "Keyring Tier 1 entry creation failed for provider '{clean_provider}', activating Tier 2: {e}"
                    );
                    self.tier2_active.store(true, Ordering::SeqCst);
                    self.set_in_tier2(clean_provider, clean_secret);
                    Ok(())
                }
            }
        }

        #[cfg(not(windows))]
        {
            self.set_in_tier2(clean_provider, clean_secret);
            Ok(())
        }
    }

    /// Delete an API key for the specified provider.
    pub fn delete_api_key(&self, provider: &str) -> Result<(), KeyringError> {
        let clean_provider = provider.trim();
        if clean_provider.is_empty() {
            return Err(KeyringError::InvalidInput(
                "Provider name cannot be empty".to_string(),
            ));
        }

        // Always remove from in-memory fallback
        let key = self.scoped_key(clean_provider);
        let mut store = match self.in_memory_store.write() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        store.remove(&key);

        if self.simulate_failure.load(Ordering::SeqCst) || self.tier2_active.load(Ordering::SeqCst)
        {
            return Ok(());
        }

        #[cfg(windows)]
        {
            if let Ok(entry) = keyring::Entry::new(&self.service, clean_provider) {
                match entry.delete_credential() {
                    Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                    Err(e) => {
                        tracing::warn!(
                            "Keyring Tier 1 delete failed for provider '{clean_provider}': {e}"
                        );
                        Ok(())
                    }
                }
            } else {
                Ok(())
            }
        }

        #[cfg(not(windows))]
        {
            Ok(())
        }
    }

    /// Check whether Tier 2 (In-Memory Session Provider) is currently active.
    #[must_use]
    pub fn is_tier2_active(&self) -> bool {
        self.simulate_failure.load(Ordering::SeqCst) || self.tier2_active.load(Ordering::SeqCst)
    }

    /// Simulate platform failure for testing (UT-28.2).
    pub fn simulate_platform_failure(&self, enable: bool) {
        self.simulate_failure.store(enable, Ordering::SeqCst);
        if enable {
            self.tier2_active.store(true, Ordering::SeqCst);
        }
    }

    /// Clear in-memory session store for this service namespace (primarily for unit/integration testing).
    pub fn clear_session_store(&self) {
        let prefix = format!("{}:", self.service);
        let mut store = match self.in_memory_store.write() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        store.retain(|k, _| !k.starts_with(&prefix));
    }

    fn get_from_tier2(&self, provider: &str) -> Option<String> {
        let key = self.scoped_key(provider);
        let store = match self.in_memory_store.read() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        store.get(&key).cloned()
    }

    fn set_in_tier2(&self, provider: &str, secret: &str) {
        let key = self.scoped_key(provider);
        let mut store = match self.in_memory_store.write() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        store.insert(key, secret.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier2_in_memory_session_fallback_ut_28_2() {
        let manager = KeyringManager::new("NeuroClipTest");
        manager.clear_session_store();

        // 1. Simulate platform failure (UT-28.2)
        manager.simulate_platform_failure(true);
        assert!(manager.is_tier2_active());

        // 2. Set API keys under simulated failure
        assert!(manager.set_api_key("groq", "gsk_test_key_12345").is_ok());
        assert!(manager.set_api_key("gemini", "AIzaSyTest_67890").is_ok());

        // 3. Read back API keys
        let groq_key = manager.get_api_key("groq");
        assert_eq!(groq_key, Ok(Some("gsk_test_key_12345".to_string())));

        let gemini_key = manager.get_api_key("gemini");
        assert_eq!(gemini_key, Ok(Some("AIzaSyTest_67890".to_string())));

        // 4. Delete key
        assert!(manager.delete_api_key("groq").is_ok());
        assert_eq!(manager.get_api_key("groq"), Ok(None));
        assert_eq!(
            manager.get_api_key("gemini"),
            Ok(Some("AIzaSyTest_67890".to_string()))
        );

        // Cleanup
        manager.clear_session_store();
        manager.simulate_platform_failure(false);
    }

    #[test]
    fn test_empty_provider_validation() {
        let manager = KeyringManager::new("NeuroClipTest");
        let res = manager.set_api_key("   ", "key");
        assert!(matches!(res, Err(KeyringError::InvalidInput(_))));

        let res_get = manager.get_api_key("");
        assert!(matches!(res_get, Err(KeyringError::InvalidInput(_))));
    }

    #[test]
    fn test_cross_service_isolation_in_tier2() {
        let mgr_a = KeyringManager::new("IsolationTestServiceA");
        let mgr_b = KeyringManager::new("IsolationTestServiceB");

        mgr_a.simulate_platform_failure(true);
        mgr_b.simulate_platform_failure(true);

        assert!(mgr_a.set_api_key("groq", "key_a").is_ok());
        assert!(mgr_b.set_api_key("groq", "key_b").is_ok());

        assert_eq!(mgr_a.get_api_key("groq"), Ok(Some("key_a".to_string())));
        assert_eq!(mgr_b.get_api_key("groq"), Ok(Some("key_b".to_string())));

        // Clear only service A
        mgr_a.clear_session_store();
        assert_eq!(mgr_a.get_api_key("groq"), Ok(None));
        assert_eq!(mgr_b.get_api_key("groq"), Ok(Some("key_b".to_string())));

        // Delete on service B
        assert!(mgr_b.delete_api_key("groq").is_ok());
        assert_eq!(mgr_b.get_api_key("groq"), Ok(None));

        mgr_a.simulate_platform_failure(false);
        mgr_b.simulate_platform_failure(false);
    }
}
