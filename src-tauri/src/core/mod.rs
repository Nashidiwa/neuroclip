//! Core domain modules for NeuroClip desktop application.

pub mod custom_protocol;
pub mod storage;

/// Backward-compatibility and architectural alias for core logging subsystem.
pub use crate::logging;
