//! Hardware profiling and deterministic recommendation engine for NeuroClip.
//!
//! Conforms 100% to JSON Schema 5.1 in `docs/ARCHITECTURE.md`.
//! Strictly adheres to zero-panic invariants (#![deny(clippy::unwrap_used)]).

use crate::error::HardwareError;
use chrono::Utc;
use serde::{Deserialize, Serialize};

/// GPU vendor enumeration conforming strictly to Schema 5.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum GpuVendor {
    #[serde(rename = "NVIDIA")]
    Nvidia,
    #[serde(rename = "AMD")]
    Amd,
    #[serde(rename = "Intel")]
    Intel,
    #[serde(rename = "Unknown")]
    #[default]
    Unknown,
}

/// Execution mode recommendation matching Schema 5.1 enum values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RecommendedMode {
    #[default]
    CloudByok,
    Hybrid,
    OfflineLocal,
}

/// CPU hardware specifications.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CpuInfo {
    pub model: String,
    pub cores: usize,
    pub threads: usize,
}

/// System RAM metrics in megabytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RamInfo {
    pub total_mb: u64,
    pub available_mb: u64,
}

/// GPU graphics adapter and dedicated VRAM metrics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuInfo {
    pub vendor: GpuVendor,
    pub model: String,
    pub vram_mb: u64,
    pub cuda_supported: bool,
}

/// Complete Hardware Profile conforming strictly to JSON Schema 5.1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HardwareProfile {
    pub cpu_info: CpuInfo,
    pub ram_info: RamInfo,
    pub gpu_info: GpuInfo,
    pub recommended_mode: RecommendedMode,
    pub recommendation_reason: String,
    pub timestamp: String,
}

/// Pure, deterministic recommendation heuristic engine.
///
/// Implements decision matrix matching UT-27.1, UT-27.2, UT-27.3, and TC-HW-01..10:
/// - Low RAM (< 8192 MB) -> `CloudByok`
/// - iGPU (Intel / Unknown with low VRAM) -> `CloudByok`
/// - Low Dedicated VRAM (< 3000 MB) -> `CloudByok`
/// - Non-CUDA GPU (e.g. AMD Radeon) -> `CloudByok`
/// - NVIDIA CUDA with >= 8000 MB VRAM & >= 16384 MB RAM -> `OfflineLocal`
/// - NVIDIA CUDA with >= 8000 MB VRAM but < 16384 MB RAM -> `Hybrid` (bottleneck)
/// - Mid-range GPU (3000 MB - 7999 MB VRAM) -> `Hybrid`
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn evaluate_recommended_mode(
    _cpu: &CpuInfo,
    ram: &RamInfo,
    gpu: &GpuInfo,
) -> (RecommendedMode, String) {
    // 1. Invariant: RAM < 8GB cannot support local heavy models safely
    if ram.total_mb < 8192 {
        return (
            RecommendedMode::CloudByok,
            "RAM hệ thống < 8GB không đủ đáp ứng mô hình cục bộ: Đề xuất dùng Cloud BYOK để xử lý nhanh và không quá tải máy.".to_string(),
        );
    }

    // 2. Invariant: Intel iGPU
    if gpu.vendor == GpuVendor::Intel {
        return (
            RecommendedMode::CloudByok,
            "Cấu hình iGPU Intel: Đề xuất dùng Cloud BYOK để xử lý siêu tốc và không nóng máy."
                .to_string(),
        );
    }

    // 3. Invariant: AMD GPUs (no CUDA acceleration)
    if gpu.vendor == GpuVendor::Amd {
        return (
            RecommendedMode::CloudByok,
            "GPU AMD Radeon chưa hỗ trợ tăng tốc CUDA: Đề xuất dùng Cloud BYOK (Groq API) để đạt tốc độ cao nhất.".to_string(),
        );
    }

    // 4. Invariant: Non-CUDA / Unknown GPUs
    if !gpu.cuda_supported || gpu.vendor == GpuVendor::Unknown {
        return (
            RecommendedMode::CloudByok,
            "Cấu hình văn phòng/iGPU không có CUDA: Đề xuất dùng Cloud BYOK để xử lý siêu tốc và không nóng máy.".to_string(),
        );
    }

    // 5. Invariant: Low dedicated VRAM (< 3000 MB)
    if gpu.vram_mb < 3000 {
        return (
            RecommendedMode::CloudByok,
            "VRAM card đồ họa < 3GB không đủ đáp ứng mô hình Whisper cục bộ: Đề xuất dùng Cloud BYOK (Groq API).".to_string(),
        );
    }

    // 6. Invariant: High-end NVIDIA CUDA GPU (>= 8000 MB VRAM)
    if gpu.vram_mb >= 8000 {
        if ram.total_mb >= 15000 {
            return (
                RecommendedMode::OfflineLocal,
                format!(
                    "GPU NVIDIA mạnh mẽ ({:.0}GB VRAM) hỗ trợ CUDA và RAM hệ thống >= 16GB: Đề xuất chế độ Offline Local (Faster-Whisper Large hoàn toàn cục bộ).",
                    (gpu.vram_mb as f64) / 1024.0
                ),
            );
        }
        // VRAM >= 8GB but system RAM is low (< 15000 MB / < 16GB)
        return (
            RecommendedMode::Hybrid,
            "GPU VRAM >= 8GB nhưng RAM hệ thống < 16GB: Đề xuất chế độ Hybrid để tránh nghẽn bộ nhớ.".to_string(),
        );
    }

    // 7. Invariant: Mid-range NVIDIA GPU (3000 MB - 7999 MB VRAM)
    (
        RecommendedMode::Hybrid,
        format!(
            "GPU tầm trung (VRAM {:.0}GB) hỗ trợ CUDA: Đề xuất chế độ Hybrid (Cloud Groq STT + Local Video Processing).",
            (gpu.vram_mb as f64) / 1024.0
        ),
    )
}

/// Probe system hardware capabilities (CPU, RAM, GPU) and generate a HardwareProfile.
pub fn probe_system_hardware() -> Result<HardwareProfile, HardwareError> {
    let cpu = probe_cpu_info();
    let ram = probe_ram_info();
    let gpu = probe_gpu_info();

    let (recommended_mode, recommendation_reason) = evaluate_recommended_mode(&cpu, &ram, &gpu);

    Ok(HardwareProfile {
        cpu_info: cpu,
        ram_info: ram,
        gpu_info: gpu,
        recommended_mode,
        recommendation_reason,
        timestamp: Utc::now().to_rfc3339(),
    })
}

fn probe_cpu_info() -> CpuInfo {
    let threads = std::thread::available_parallelism().map_or_else(
        |_| {
            std::env::var("NUMBER_OF_PROCESSORS")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(4)
        },
        std::num::NonZeroUsize::get,
    );

    let cores = if threads > 1 {
        // Safe heuristic: physical cores is usually threads / 2 for hyperthreaded CPUs
        std::cmp::max(threads / 2, 1)
    } else {
        1
    };

    let model = {
        #[cfg(windows)]
        {
            probe_windows_registry_cpu().unwrap_or_else(|| {
                std::env::var("PROCESSOR_IDENTIFIER")
                    .unwrap_or_else(|_| "Generic x86_64 Processor".to_string())
            })
        }
        #[cfg(not(windows))]
        {
            std::env::var("PROCESSOR_IDENTIFIER")
                .unwrap_or_else(|_| "Generic x86_64 Processor".to_string())
        }
    };

    CpuInfo {
        model,
        cores,
        threads,
    }
}

fn probe_ram_info() -> RamInfo {
    #[cfg(windows)]
    {
        use std::mem::size_of;
        #[repr(C)]
        struct MemoryStatusEx {
            dw_length: u32,
            dw_memory_load: u32,
            ull_total_phys: u64,
            ull_avail_phys: u64,
            ull_total_page_file: u64,
            ull_avail_page_file: u64,
            ull_total_virtual: u64,
            ull_avail_virtual: u64,
            ull_avail_extended_virtual: u64,
        }

        extern "system" {
            fn GlobalMemoryStatusEx(lp_buffer: *mut MemoryStatusEx) -> i32;
        }

        let mut status = MemoryStatusEx {
            dw_length: u32::try_from(size_of::<MemoryStatusEx>()).unwrap_or(0),
            dw_memory_load: 0,
            ull_total_phys: 0,
            ull_avail_phys: 0,
            ull_total_page_file: 0,
            ull_avail_page_file: 0,
            ull_total_virtual: 0,
            ull_avail_virtual: 0,
            ull_avail_extended_virtual: 0,
        };

        // SAFETY: Calling Win32 GlobalMemoryStatusEx with valid sized struct pointer
        let success = unsafe { GlobalMemoryStatusEx(&raw mut status) };
        if success != 0 && status.ull_total_phys > 0 {
            let total_mb = status.ull_total_phys / (1024 * 1024);
            let available_mb = status.ull_avail_phys / (1024 * 1024);
            return RamInfo {
                total_mb,
                available_mb,
            };
        }
    }

    // Fallback if not Windows or API call fails
    RamInfo {
        total_mb: 16384,
        available_mb: 8192,
    }
}

fn probe_gpu_info() -> GpuInfo {
    #[cfg(windows)]
    {
        // Try reading video adapter info from Windows Registry
        if let Some(gpu) = probe_windows_registry_gpu() {
            return gpu;
        }
    }

    GpuInfo {
        vendor: GpuVendor::Unknown,
        model: "Generic Display Adapter".to_string(),
        vram_mb: 0,
        cuda_supported: false,
    }
}

/// Select the optimal GPU candidate according to hardware priority rules:
/// - Priority 1: Dedicated GPU with CUDA support or NVIDIA vendor.
/// - Priority 2: Highest dedicated VRAM (megabytes).
/// - Tie-breaker: Known vendor over Unknown.
#[must_use]
pub fn select_optimal_gpu(candidates: Vec<GpuInfo>) -> Option<GpuInfo> {
    candidates.into_iter().max_by(|a, b| {
        let a_priority = (
            a.cuda_supported || a.vendor == GpuVendor::Nvidia,
            a.vram_mb,
            a.vendor != GpuVendor::Unknown,
        );
        let b_priority = (
            b.cuda_supported || b.vendor == GpuVendor::Nvidia,
            b.vram_mb,
            b.vendor != GpuVendor::Unknown,
        );
        a_priority.cmp(&b_priority)
    })
}

#[cfg(windows)]
mod win32 {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    pub const HKEY_LOCAL_MACHINE: usize = 0x8000_0002;
    pub const KEY_READ: u32 = 0x20019;
    pub const ERROR_SUCCESS: u32 = 0;

    extern "system" {
        pub fn RegOpenKeyExW(
            h_key: usize,
            lp_sub_key: *const u16,
            ul_options: u32,
            sam_desired: u32,
            phk_result: *mut usize,
        ) -> u32;

        pub fn RegQueryValueExW(
            h_key: usize,
            lp_value_name: *const u16,
            lp_reserved: *mut u32,
            lp_type: *mut u32,
            lp_data: *mut u8,
            lpcb_data: *mut u32,
        ) -> u32;

        pub fn RegCloseKey(h_key: usize) -> u32;
    }

    pub fn to_wide_null(s: &str) -> Vec<u16> {
        let mut wide: Vec<u16> = OsStr::new(s).encode_wide().collect();
        wide.push(0);
        wide
    }

    pub fn decode_utf16_bytes(bytes: &[u8]) -> String {
        let u16_slice: Vec<u16> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&c| u16::from_ne_bytes(c))
            .take_while(|&c| c != 0)
            .collect();
        String::from_utf16_lossy(&u16_slice).trim().to_string()
    }
}

#[cfg(windows)]
fn probe_windows_registry_cpu() -> Option<String> {
    use win32::{
        decode_utf16_bytes, to_wide_null, RegCloseKey, RegOpenKeyExW, RegQueryValueExW,
        ERROR_SUCCESS, HKEY_LOCAL_MACHINE, KEY_READ,
    };

    let subkey_wide = to_wide_null("HARDWARE\\DESCRIPTION\\System\\CentralProcessor\\0");
    let mut h_key: usize = 0;
    let open_res = unsafe {
        RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            subkey_wide.as_ptr(),
            0,
            KEY_READ,
            &raw mut h_key,
        )
    };

    if open_res != ERROR_SUCCESS {
        return None;
    }

    let val_name_wide = to_wide_null("ProcessorNameString");
    let mut data_buf = [0u8; 512];
    let mut data_len = u32::try_from(data_buf.len()).unwrap_or(u32::MAX);
    let mut val_type = 0u32;

    let query_res = unsafe {
        RegQueryValueExW(
            h_key,
            val_name_wide.as_ptr(),
            std::ptr::null_mut(),
            &raw mut val_type,
            data_buf.as_mut_ptr(),
            &raw mut data_len,
        )
    };

    let _ = unsafe { RegCloseKey(h_key) };

    if query_res == ERROR_SUCCESS && data_len > 1 {
        let safe_len = (data_len as usize).min(data_buf.len());
        if let Some(slice) = data_buf.get(..safe_len) {
            let model = decode_utf16_bytes(slice);
            if !model.is_empty() {
                return Some(model);
            }
        }
    }

    None
}

#[cfg(windows)]
#[allow(clippy::too_many_lines)]
fn probe_windows_registry_gpu() -> Option<GpuInfo> {
    use win32::{
        decode_utf16_bytes, to_wide_null, RegCloseKey, RegOpenKeyExW, RegQueryValueExW,
        ERROR_SUCCESS, HKEY_LOCAL_MACHINE, KEY_READ,
    };

    let class_path =
        "SYSTEM\\CurrentControlSet\\Control\\Class\\{4d36e968-e325-11ce-bfc1-08002be10318}";
    let mut candidates: Vec<GpuInfo> = Vec::new();

    // Scan display adapter subkeys 0000 through 0008 (covers multi-GPU setups such as Optimus laptops)
    for i in 0..=8 {
        let subkey = format!("{class_path}\\{i:04}");
        let wide_subkey = to_wide_null(&subkey);

        let mut h_key: usize = 0;
        let open_res = unsafe {
            RegOpenKeyExW(
                HKEY_LOCAL_MACHINE,
                wide_subkey.as_ptr(),
                0,
                KEY_READ,
                &raw mut h_key,
            )
        };

        if open_res == ERROR_SUCCESS {
            // 1. Read DriverDesc
            let desc_name = to_wide_null("DriverDesc");
            let mut data_buf = [0u8; 512];
            let mut data_len = u32::try_from(data_buf.len()).unwrap_or(u32::MAX);
            let mut val_type = 0u32;

            let query_res = unsafe {
                RegQueryValueExW(
                    h_key,
                    desc_name.as_ptr(),
                    std::ptr::null_mut(),
                    &raw mut val_type,
                    data_buf.as_mut_ptr(),
                    &raw mut data_len,
                )
            };

            let mut model_str = String::new();
            if query_res == ERROR_SUCCESS && data_len > 1 {
                let safe_len = (data_len as usize).min(data_buf.len());
                if let Some(slice) = data_buf.get(..safe_len) {
                    model_str = decode_utf16_bytes(slice);
                }
            }

            // 2. Read VRAM:
            // Modern WDDM drivers cap 32-bit memory metrics at 4095 MB for GPUs > 4GB.
            // The 64-bit QWORD qwMemorySize contains the true physical VRAM.
            // Tier 1: Try reading 64-bit HardwareInformation.qwMemorySize first.
            let qw_mem_name = to_wide_null("HardwareInformation.qwMemorySize");
            let mut qw_buf = [0u8; 8];
            let mut qw_len = u32::try_from(qw_buf.len()).unwrap_or(u32::MAX);
            let mut qw_type = 0u32;

            let qw_res = unsafe {
                RegQueryValueExW(
                    h_key,
                    qw_mem_name.as_ptr(),
                    std::ptr::null_mut(),
                    &raw mut qw_type,
                    qw_buf.as_mut_ptr(),
                    &raw mut qw_len,
                )
            };

            let mut vram_mb = if qw_res == ERROR_SUCCESS && qw_len == 8 {
                let val = u64::from_ne_bytes(qw_buf);
                val / (1024 * 1024)
            } else {
                0
            };

            // Tier 2: If qwMemorySize was absent or 0, fallback to HardwareInformation.MemorySize (4 or 8 bytes).
            if vram_mb == 0 {
                let mem_name = to_wide_null("HardwareInformation.MemorySize");
                let mut mem_buf = [0u8; 8];
                let mut mem_len = u32::try_from(mem_buf.len()).unwrap_or(u32::MAX);
                let mut mem_type = 0u32;

                let mem_res = unsafe {
                    RegQueryValueExW(
                        h_key,
                        mem_name.as_ptr(),
                        std::ptr::null_mut(),
                        &raw mut mem_type,
                        mem_buf.as_mut_ptr(),
                        &raw mut mem_len,
                    )
                };

                if mem_res == ERROR_SUCCESS {
                    let vram_bytes = if mem_len == 4 {
                        let bytes: [u8; 4] = [mem_buf[0], mem_buf[1], mem_buf[2], mem_buf[3]];
                        u64::from(u32::from_ne_bytes(bytes))
                    } else if mem_len == 8 {
                        u64::from_ne_bytes(mem_buf)
                    } else {
                        0
                    };
                    vram_mb = vram_bytes / (1024 * 1024);
                }
            }

            let _ = unsafe { RegCloseKey(h_key) };

            if !model_str.is_empty() {
                let upper = model_str.to_uppercase();
                let (vendor, cuda_supported) =
                    if upper.contains("NVIDIA") || upper.contains("GEFORCE") {
                        (GpuVendor::Nvidia, true)
                    } else if upper.contains("AMD") || upper.contains("RADEON") {
                        (GpuVendor::Amd, false)
                    } else if upper.contains("INTEL") {
                        (GpuVendor::Intel, false)
                    } else {
                        (GpuVendor::Unknown, false)
                    };

                candidates.push(GpuInfo {
                    vendor,
                    model: model_str,
                    vram_mb,
                    cuda_supported,
                });
            }
        }
    }

    select_optimal_gpu(candidates)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_5_1_serialization_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
        let profile = HardwareProfile {
            cpu_info: CpuInfo {
                model: "AMD Ryzen 9 7950X 16-Core Processor".to_string(),
                cores: 16,
                threads: 32,
            },
            ram_info: RamInfo {
                total_mb: 32768,
                available_mb: 24576,
            },
            gpu_info: GpuInfo {
                vendor: GpuVendor::Nvidia,
                model: "NVIDIA GeForce RTX 4090".to_string(),
                vram_mb: 24576,
                cuda_supported: true,
            },
            recommended_mode: RecommendedMode::OfflineLocal,
            recommendation_reason: "Card đồ họa 24GB VRAM hỗ trợ CUDA".to_string(),
            timestamp: Utc::now().to_rfc3339(),
        };

        let json_str = serde_json::to_string(&profile)?;
        let val: serde_json::Value = serde_json::from_str(&json_str)?;

        assert!(val.get("cpu_info").is_some());
        assert!(val.get("ram_info").is_some());
        assert!(val.get("gpu_info").is_some());
        assert_eq!(val["recommended_mode"], "offline_local");
        assert_eq!(val["gpu_info"]["vendor"], "NVIDIA");
        assert!(val.get("timestamp").is_some());

        let roundtrip: HardwareProfile = serde_json::from_str(&json_str)?;
        assert_eq!(roundtrip.cpu_info.cores, 16);
        assert_eq!(roundtrip.gpu_info.vendor, GpuVendor::Nvidia);
        assert_eq!(roundtrip.recommended_mode, RecommendedMode::OfflineLocal);

        Ok(())
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_heuristic_recommendations_matrix() {
        let dummy_cpu = CpuInfo {
            model: "Test CPU".to_string(),
            cores: 8,
            threads: 16,
        };

        // TC-HW-01: RTX 4090 24GB VRAM + 32GB RAM -> offline_local
        let (mode, reason) = evaluate_recommended_mode(
            &dummy_cpu,
            &RamInfo {
                total_mb: 32768,
                available_mb: 24000,
            },
            &GpuInfo {
                vendor: GpuVendor::Nvidia,
                model: "RTX 4090".to_string(),
                vram_mb: 24576,
                cuda_supported: true,
            },
        );
        assert_eq!(mode, RecommendedMode::OfflineLocal);
        assert!(reason.contains("NVIDIA"));

        // TC-HW-03: RTX 3060 Ti 8GB VRAM + 16GB RAM -> offline_local
        let (mode, _) = evaluate_recommended_mode(
            &dummy_cpu,
            &RamInfo {
                total_mb: 16384,
                available_mb: 10000,
            },
            &GpuInfo {
                vendor: GpuVendor::Nvidia,
                model: "RTX 3060 Ti".to_string(),
                vram_mb: 8192,
                cuda_supported: true,
            },
        );
        assert_eq!(mode, RecommendedMode::OfflineLocal);

        // TC-HW-04: RTX 3050 4GB VRAM + 16GB RAM -> hybrid
        let (mode, reason) = evaluate_recommended_mode(
            &dummy_cpu,
            &RamInfo {
                total_mb: 16384,
                available_mb: 11000,
            },
            &GpuInfo {
                vendor: GpuVendor::Nvidia,
                model: "RTX 3050".to_string(),
                vram_mb: 4096,
                cuda_supported: true,
            },
        );
        assert_eq!(mode, RecommendedMode::Hybrid);
        assert!(reason.contains("Hybrid"));

        // TC-HW-06: GTX 1050 2GB VRAM -> cloud_byok (VRAM cliff < 3GB)
        let (mode, reason) = evaluate_recommended_mode(
            &dummy_cpu,
            &RamInfo {
                total_mb: 16384,
                available_mb: 10000,
            },
            &GpuInfo {
                vendor: GpuVendor::Nvidia,
                model: "GTX 1050".to_string(),
                vram_mb: 2048,
                cuda_supported: true,
            },
        );
        assert_eq!(mode, RecommendedMode::CloudByok);
        assert!(reason.contains("< 3GB"));

        // TC-HW-07: Intel Iris Xe -> cloud_byok
        let (mode, reason) = evaluate_recommended_mode(
            &dummy_cpu,
            &RamInfo {
                total_mb: 8192,
                available_mb: 4000,
            },
            &GpuInfo {
                vendor: GpuVendor::Intel,
                model: "Iris Xe".to_string(),
                vram_mb: 0,
                cuda_supported: false,
            },
        );
        assert_eq!(mode, RecommendedMode::CloudByok);
        assert!(reason.contains("Cloud"));

        // TC-HW-08: AMD Radeon RX 7900 XTX 24GB -> cloud_byok (non-CUDA)
        let (mode, reason) = evaluate_recommended_mode(
            &dummy_cpu,
            &RamInfo {
                total_mb: 32768,
                available_mb: 22000,
            },
            &GpuInfo {
                vendor: GpuVendor::Amd,
                model: "RX 7900 XTX".to_string(),
                vram_mb: 24576,
                cuda_supported: false,
            },
        );
        assert_eq!(mode, RecommendedMode::CloudByok);
        assert!(reason.contains("AMD"));

        // TC-HW-09: RTX 4090 with low system RAM (8GB) -> hybrid
        let (mode, reason) = evaluate_recommended_mode(
            &dummy_cpu,
            &RamInfo {
                total_mb: 8192,
                available_mb: 3000,
            },
            &GpuInfo {
                vendor: GpuVendor::Nvidia,
                model: "RTX 4090".to_string(),
                vram_mb: 24576,
                cuda_supported: true,
            },
        );
        assert_eq!(mode, RecommendedMode::Hybrid);
        assert!(reason.contains("< 16GB"));
    }

    #[test]
    fn test_probe_system_hardware_succeeds() {
        let profile_res = probe_system_hardware();
        assert!(profile_res.is_ok());
        if let Ok(profile) = profile_res {
            assert_ne!(profile.cpu_info.model, "");
            assert!(profile.cpu_info.cores > 0);
            assert!(profile.cpu_info.threads > 0);
            assert!(profile.ram_info.total_mb > 0);
            assert_ne!(profile.timestamp, "");
        }
    }

    #[test]
    fn test_select_optimal_gpu_dual_gpu_laptop_prefers_nvidia() {
        let igpu = GpuInfo {
            vendor: GpuVendor::Intel,
            model: "Intel Iris Xe Graphics".to_string(),
            vram_mb: 128,
            cuda_supported: false,
        };
        let dgpu = GpuInfo {
            vendor: GpuVendor::Nvidia,
            model: "NVIDIA GeForce RTX 4060 Laptop GPU".to_string(),
            vram_mb: 8192,
            cuda_supported: true,
        };

        let candidates = vec![igpu, dgpu];
        let selected = select_optimal_gpu(candidates);
        assert!(selected.is_some());
        if let Some(best) = selected {
            assert_eq!(best.vendor, GpuVendor::Nvidia);
            assert_eq!(best.vram_mb, 8192);
            assert!(best.cuda_supported);
        }
    }

    #[test]
    fn test_select_optimal_gpu_dual_nvidia_picks_highest_vram() {
        let gpu1 = GpuInfo {
            vendor: GpuVendor::Nvidia,
            model: "NVIDIA GeForce GTX 1660 SUPER".to_string(),
            vram_mb: 6144,
            cuda_supported: true,
        };
        let gpu2 = GpuInfo {
            vendor: GpuVendor::Nvidia,
            model: "NVIDIA GeForce RTX 4090".to_string(),
            vram_mb: 24576,
            cuda_supported: true,
        };

        let candidates = vec![gpu1, gpu2];
        let selected = select_optimal_gpu(candidates);
        assert!(selected.is_some());
        if let Some(best) = selected {
            assert_eq!(best.model, "NVIDIA GeForce RTX 4090");
            assert_eq!(best.vram_mb, 24576);
        }
    }

    #[test]
    fn test_select_optimal_gpu_amd_over_igpu_by_vram() {
        let igpu = GpuInfo {
            vendor: GpuVendor::Intel,
            model: "Intel UHD Graphics 770".to_string(),
            vram_mb: 0,
            cuda_supported: false,
        };
        let amd_dgpu = GpuInfo {
            vendor: GpuVendor::Amd,
            model: "AMD Radeon RX 7900 XTX".to_string(),
            vram_mb: 24576,
            cuda_supported: false,
        };

        let candidates = vec![igpu, amd_dgpu];
        let selected = select_optimal_gpu(candidates);
        assert!(selected.is_some());
        if let Some(best) = selected {
            assert_eq!(best.vendor, GpuVendor::Amd);
            assert_eq!(best.vram_mb, 24576);
        }
    }

    #[test]
    fn test_select_optimal_gpu_filters_virtual_monitors() {
        let virtual_monitor = GpuInfo {
            vendor: GpuVendor::Unknown,
            model: "Meta Virtual Monitor".to_string(),
            vram_mb: 0,
            cuda_supported: false,
        };
        let nvidia_gpu = GpuInfo {
            vendor: GpuVendor::Nvidia,
            model: "NVIDIA GeForce GTX 1660 SUPER".to_string(),
            vram_mb: 6144,
            cuda_supported: true,
        };

        let candidates = vec![virtual_monitor, nvidia_gpu];
        let selected = select_optimal_gpu(candidates);
        assert!(selected.is_some());
        if let Some(best) = selected {
            assert_eq!(best.vendor, GpuVendor::Nvidia);
        }
    }

    #[test]
    fn test_select_optimal_gpu_empty() {
        let candidates: Vec<GpuInfo> = Vec::new();
        let selected = select_optimal_gpu(candidates);
        assert!(selected.is_none());
    }

    #[test]
    fn ut_27_1_igpu_low_ram() {
        let cpu = CpuInfo {
            model: "11th Gen Intel(R) Core(TM) i5-1135G7 @ 2.40GHz".to_string(),
            cores: 4,
            threads: 8,
        };
        let ram = RamInfo {
            total_mb: 8192,
            available_mb: 4096,
        };
        let gpu = GpuInfo {
            vendor: GpuVendor::Intel,
            model: "Intel(R) Iris(R) Xe Graphics".to_string(),
            vram_mb: 128,
            cuda_supported: false,
        };

        let (mode, reason) = evaluate_recommended_mode(&cpu, &ram, &gpu);
        assert_eq!(mode, RecommendedMode::CloudByok);
        assert!(reason.contains("iGPU Intel") || reason.contains("Cloud BYOK"));
    }

    #[test]
    fn ut_27_2_mid_range_gpu() {
        let cpu = CpuInfo {
            model: "AMD Ryzen 5 5600H with Radeon Graphics".to_string(),
            cores: 6,
            threads: 12,
        };
        let ram = RamInfo {
            total_mb: 16384,
            available_mb: 10240,
        };
        let gpu = GpuInfo {
            vendor: GpuVendor::Nvidia,
            model: "NVIDIA GeForce GTX 1650".to_string(),
            vram_mb: 4096,
            cuda_supported: true,
        };

        let (mode, reason) = evaluate_recommended_mode(&cpu, &ram, &gpu);
        assert_eq!(mode, RecommendedMode::Hybrid);
        assert!(reason.contains("Hybrid"));
    }

    #[test]
    fn ut_27_3_high_end_gpu() {
        let cpu = CpuInfo {
            model: "13th Gen Intel(R) Core(TM) i7-13700K".to_string(),
            cores: 16,
            threads: 24,
        };
        let ram = RamInfo {
            total_mb: 32768,
            available_mb: 24576,
        };
        let gpu = GpuInfo {
            vendor: GpuVendor::Nvidia,
            model: "NVIDIA GeForce RTX 3080".to_string(),
            vram_mb: 10240,
            cuda_supported: true,
        };

        let (mode, reason) = evaluate_recommended_mode(&cpu, &ram, &gpu);
        assert_eq!(mode, RecommendedMode::OfflineLocal);
        assert!(reason.contains("Offline Local"));
    }

    #[test]
    fn it_27_1_schema_5_1_compliance() -> Result<(), Box<dyn std::error::Error>> {
        let profile = probe_system_hardware()?;
        let json_val = serde_json::to_value(&profile)?;

        // Top level required keys per Schema 5.1
        assert!(json_val.get("cpu_info").is_some());
        assert!(json_val.get("ram_info").is_some());
        assert!(json_val.get("gpu_info").is_some());
        assert!(json_val.get("recommended_mode").is_some());
        assert!(json_val.get("recommendation_reason").is_some());
        assert!(json_val.get("timestamp").is_some());

        // cpu_info required
        let cpu = &json_val["cpu_info"];
        assert!(cpu.get("model").is_some() && cpu["model"].is_string());
        assert!(cpu.get("cores").is_some() && cpu["cores"].is_number());
        assert!(cpu.get("threads").is_some() && cpu["threads"].is_number());

        // ram_info required
        let ram = &json_val["ram_info"];
        assert!(ram.get("total_mb").is_some() && ram["total_mb"].is_number());
        assert!(ram.get("available_mb").is_some() && ram["available_mb"].is_number());

        // gpu_info required
        let gpu = &json_val["gpu_info"];
        assert!(gpu.get("vendor").is_some() && gpu["vendor"].is_string());
        assert!(gpu.get("model").is_some() && gpu["model"].is_string());
        assert!(gpu.get("vram_mb").is_some() && gpu["vram_mb"].is_number());
        assert!(gpu.get("cuda_supported").is_some() && gpu["cuda_supported"].is_boolean());

        // recommended_mode enum validation
        let mode_str = json_val["recommended_mode"].as_str().unwrap_or("");
        assert!(["cloud_byok", "hybrid", "offline_local"].contains(&mode_str));

        // timestamp format check (RFC3339)
        let ts_str = json_val["timestamp"].as_str().unwrap_or("");
        assert!(chrono::DateTime::parse_from_rfc3339(ts_str).is_ok());

        Ok(())
    }

    #[test]
    fn it_27_2_probe_timing_under_200ms() {
        use std::time::Instant;
        let start = Instant::now();
        let profile_res = probe_system_hardware();
        let duration = start.elapsed();

        assert!(profile_res.is_ok());
        assert!(
            duration < std::time::Duration::from_millis(200),
            "Hardware probe took {duration:?}, exceeding 200ms SLA"
        );
    }

    #[test]
    fn ed_27_1_dedicated_vram_precision() {
        // Mock a 6GB card where 32-bit MemorySize is capped at 4095 MB, but qwMemorySize is 6144 MB
        let candidates = vec![
            GpuInfo {
                vendor: GpuVendor::Nvidia,
                model: "NVIDIA GeForce GTX 1660 SUPER".to_string(),
                vram_mb: 6144, // Derived from qwMemorySize
                cuda_supported: true,
            },
            GpuInfo {
                vendor: GpuVendor::Nvidia,
                model: "NVIDIA GeForce GTX 1660 SUPER (Legacy 32-bit cap)".to_string(),
                vram_mb: 4095, // Capped at 4095MB
                cuda_supported: true,
            },
        ];

        let selected = select_optimal_gpu(candidates);
        assert!(selected.is_some());
        if let Some(gpu) = selected {
            assert_eq!(
                gpu.vram_mb, 6144,
                "Must select 6144 MB rather than capped 4095 MB"
            );
            assert_eq!(gpu.model, "NVIDIA GeForce GTX 1660 SUPER");
        }
    }

    #[test]
    fn test_ram_tolerance_16gb_kit_hardware_reserve() {
        // A 16GB dual-channel kit often reports ~15360 MB due to BIOS/hardware reserved memory
        let cpu = CpuInfo {
            model: "AMD Ryzen 7 5800X".to_string(),
            cores: 8,
            threads: 16,
        };
        let ram = RamInfo {
            total_mb: 15360, // 15GB reported
            available_mb: 11000,
        };
        let gpu = GpuInfo {
            vendor: GpuVendor::Nvidia,
            model: "NVIDIA GeForce RTX 3070".to_string(),
            vram_mb: 8192,
            cuda_supported: true,
        };

        let (mode, reason) = evaluate_recommended_mode(&cpu, &ram, &gpu);
        assert_eq!(
            mode,
            RecommendedMode::OfflineLocal,
            "16GB physical RAM with BIOS reserve (15360 MB) must qualify for OfflineLocal"
        );
        assert!(reason.contains("Offline Local"));
    }
}
