//! Core domain modules for NeuroClip desktop application.

pub mod custom_protocol;
pub mod keyring;
pub mod storage;

pub use keyring::{KeyringError, KeyringManager, SERVICE_NAME};

/// Backward-compatibility and architectural alias for core logging subsystem.
pub use crate::logging;
