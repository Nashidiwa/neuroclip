//! Configuration management, hardware profiling, and atomic file persistence.

pub mod app_config;
pub mod atomic;
pub mod hardware;

pub use app_config::{
    resolve_config_dir, resolve_config_file_path, AppConfig, SidecarPaths,
};
pub use atomic::{atomic_load_json, atomic_save_json};
pub use hardware::{
    evaluate_recommended_mode, probe_system_hardware, CpuInfo, GpuInfo, GpuVendor,
    HardwareProfile, RamInfo, RecommendedMode,
};
