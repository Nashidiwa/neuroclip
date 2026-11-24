#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_lossless
)]

//! Low-End PC Stress Benchmark & Hardware Guardrail Suite (`TASK-M4-31`, Milestone M4.9).
//!
//! Simulates a minimum-spec Windows PC environment (8GB RAM, Intel iGPU / 2GB VRAM, 4-core CPU):
//! 1. **AC-31.1 (Peak RAM <= 500 MB):** Streams 8 hours (28,800s) of 16kHz PCM audio (`TASK-M4-11`)
//!    and processes 500,000 synthetic chat messages through Z-Score (`TASK-M4-09`), Swarm Emote
//!    Filter (`TASK-M4-10`), and Multimodal Fusion + Clustering (`TASK-M4-12`).
//! 2. **AC-31.2 (Sustained Session & Cleanup <= 100 MB):** Executes 5 consecutive E2E highlight clip
//!    pipelines (`TASK-M4-30`) on a constrained runtime and verifies full memory release.
//! 3. **AC-31.3 (UI Responsiveness <= 100 ms):** Runs a 10ms Tokio event-loop heartbeat alongside
//!    heavy 500k chat + 8h audio RMS work dispatched via `tokio::task::spawn_blocking`, ensuring the
//!    async event loop never stalls for > 100 ms.
//! 4. **2GB VRAM CUDA OOM Guardrail:** Verifies `2048 MB < 2500 MB` triggers `ERR_STT_VRAM_INSUFFICIENT`
//!    (`StrictReject`) and clamps to INT8 Medium <= 1800 MB (`AutoDowngrade`) (`TASK-M4-16`).

use std::alloc::{GlobalAlloc, Layout, System};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use neuroclip_lib::config::hardware::evaluate_recommended_mode;
use neuroclip_lib::error::codes;
use neuroclip_lib::highlight::{
    compute_chat_velocity_series_from_events, compute_emote_density_series_default,
    default_swarm_dictionary, detect_energy_peaks_default, fuse_multimodal_series,
    generate_clusters_from_fused_series, process_pcm_reader, DEFAULT_BASELINE_WINDOW_SEC,
    DEFAULT_BLOCK_DURATION_SEC, DEFAULT_BUCKET_WIDTH_SEC, DEFAULT_SAMPLE_RATE,
};
use neuroclip_lib::ingestion::ChatEvent;
use neuroclip_lib::stt::faster_whisper_worker::{
    check_hardware_guardrail_from_profile, estimate_vram_usage_mb,
    evaluate_hardware_guardrail_from_profile, GuardrailPolicy, MAX_INT8_VRAM_CAP_MB,
    MIN_LARGE_VRAM_MB,
};
use neuroclip_lib::{
    run_synthetic_e2e_pipeline_internal, validate_schema_5_1_hardware_profile, AppError, CpuInfo,
    GpuInfo, GpuVendor, HardwareProfile, RamInfo, RecommendedMode, SttError, SyntheticE2EOptions,
};

// ===========================================================================
// Dual Memory Telemetry: Lock-free TrackingAllocator + Win32 K32GetProcessMemoryInfo
// ===========================================================================

struct TrackingAllocator;

static LIVE_HEAP_BYTES: AtomicUsize = AtomicUsize::new(0);
static PEAK_HEAP_BYTES: AtomicUsize = AtomicUsize::new(0);
static ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);
static DEALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);

#[inline]
fn record_heap_growth(delta_bytes: usize) {
    ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
    let prev = LIVE_HEAP_BYTES.fetch_add(delta_bytes, Ordering::Relaxed);
    let new_live = prev.saturating_add(delta_bytes);
    PEAK_HEAP_BYTES.fetch_max(new_live, Ordering::Relaxed);
}

#[inline]
fn record_heap_shrink(delta_bytes: usize) {
    let _ = LIVE_HEAP_BYTES.try_update(Ordering::Relaxed, Ordering::Relaxed, |cur| {
        Some(cur.saturating_sub(delta_bytes))
    });
}

// SAFETY: Wraps std::alloc::System with strictly lock-free AtomicUsize accounting (never allocates).
unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            record_heap_growth(layout.size());
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            record_heap_growth(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
        DEALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
        record_heap_shrink(layout.size());
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = unsafe { System.realloc(ptr, layout, new_size) };
        if !new_ptr.is_null() {
            let old_size = layout.size();
            if new_size >= old_size {
                record_heap_growth(new_size - old_size);
            } else {
                DEALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
                record_heap_shrink(old_size - new_size);
            }
        }
        new_ptr
    }
}

#[global_allocator]
static GLOBAL_ALLOCATOR: TrackingAllocator = TrackingAllocator;

#[inline]
fn current_heap_bytes() -> usize {
    LIVE_HEAP_BYTES.load(Ordering::Relaxed)
}

#[inline]
fn peak_heap_bytes() -> usize {
    PEAK_HEAP_BYTES.load(Ordering::Relaxed)
}

#[inline]
fn reset_peak_heap() -> usize {
    let live = current_heap_bytes();
    PEAK_HEAP_BYTES.store(live, Ordering::Relaxed);
    live
}

#[inline]
fn bytes_to_mb(bytes: usize) -> f64 {
    (bytes as f64) / (1024.0 * 1024.0)
}

#[derive(Debug, Clone, Copy, Default)]
struct OsMemorySnapshot {
    working_set_bytes: usize,
    peak_working_set_bytes: usize,
    private_commit_bytes: usize,
    peak_private_commit_bytes: usize,
}

impl OsMemorySnapshot {
    fn working_set_mb(self) -> f64 {
        bytes_to_mb(self.working_set_bytes)
    }

    fn peak_working_set_mb(self) -> f64 {
        bytes_to_mb(self.peak_working_set_bytes)
    }

    fn private_commit_mb(self) -> f64 {
        bytes_to_mb(self.private_commit_bytes)
    }

    fn peak_private_commit_mb(self) -> f64 {
        bytes_to_mb(self.peak_private_commit_bytes)
    }
}

#[cfg(windows)]
#[repr(C)]
struct ProcessMemoryCounters {
    cb: u32,
    page_fault_count: u32,
    peak_working_set_size: usize,
    working_set_size: usize,
    quota_peak_paged_pool_usage: usize,
    quota_paged_pool_usage: usize,
    quota_peak_non_paged_pool_usage: usize,
    quota_non_paged_pool_usage: usize,
    pagefile_usage: usize,
    peak_pagefile_usage: usize,
}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn GetCurrentProcess() -> isize;
    fn K32GetProcessMemoryInfo(
        process: isize,
        ppsmemcounters: *mut ProcessMemoryCounters,
        cb: u32,
    ) -> i32;
}

fn sample_os_memory() -> OsMemorySnapshot {
    #[cfg(windows)]
    {
        let mut counters = ProcessMemoryCounters {
            cb: std::mem::size_of::<ProcessMemoryCounters>() as u32,
            page_fault_count: 0,
            peak_working_set_size: 0,
            working_set_size: 0,
            quota_peak_paged_pool_usage: 0,
            quota_paged_pool_usage: 0,
            quota_peak_non_paged_pool_usage: 0,
            quota_non_paged_pool_usage: 0,
            pagefile_usage: 0,
            peak_pagefile_usage: 0,
        };

        // SAFETY: Valid current process handle and sized ProcessMemoryCounters buffer.
        let ok = unsafe {
            let proc_handle = GetCurrentProcess();
            K32GetProcessMemoryInfo(proc_handle, &raw mut counters, counters.cb)
        };
        assert_ne!(ok, 0, "K32GetProcessMemoryInfo must succeed on Windows");
        assert!(
            counters.working_set_size > 0,
            "OS WorkingSetSize must be strictly > 0"
        );

        OsMemorySnapshot {
            working_set_bytes: counters.working_set_size,
            peak_working_set_bytes: counters.peak_working_set_size,
            private_commit_bytes: counters.pagefile_usage,
            peak_private_commit_bytes: counters.peak_pagefile_usage,
        }
    }

    #[cfg(not(windows))]
    {
        let live = current_heap_bytes();
        let peak = peak_heap_bytes();
        OsMemorySnapshot {
            working_set_bytes: live,
            peak_working_set_bytes: peak,
            private_commit_bytes: live,
            peak_private_commit_bytes: peak,
        }
    }
}

/// Serialize benchmark tests within this binary so peak RAM and latency metrics stay isolated.
static BENCH_LOCK: Mutex<()> = Mutex::new(());

fn acquire_bench_lock() -> std::sync::MutexGuard<'static, ()> {
    BENCH_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

// ===========================================================================
// Constants & Synthetic Workload Generators (8h Audio Stream + 500k Chat)
// ===========================================================================

const PEAK_RAM_LIMIT_MB: f64 = 500.0;
const IDLE_RAM_LIMIT_MB: f64 = 100.0;
const MAX_UI_STALL_MS: f64 = 100.0;

const EIGHT_HOURS_SEC: usize = 28_800; // 8 * 3600s
const EIGHT_HOURS_F64: f64 = 28_800.0;
const CHAT_EVENT_COUNT: usize = 500_000;
const EXPECTED_5S_BUCKETS: usize = 5_760; // 28,800s / 5s
const EXPECTED_AUDIO_BLOCKS: usize = 57_600; // 28,800s / 0.5s
const PEAK_TIMESTAMPS_SEC: [usize; 5] = [1_800, 7_200, 14_400, 21_600, 25_200];

struct TempTestDir {
    path: PathBuf,
}

impl TempTestDir {
    fn new(label: &str) -> Self {
        let nanos = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let path = std::env::temp_dir().join(format!(
            "neuroclip_bench_{label}_{}_{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempTestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn fixture_paths() -> (PathBuf, PathBuf) {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixtures_dir = manifest_dir.join("tests").join("fixtures");
    (
        fixtures_dir.join("synthetic_audio.wav"),
        fixtures_dir.join("synthetic_chat.json"),
    )
}

/// Deterministic O(1)-memory streaming PCM generator producing 8 hours of 16kHz 16-bit mono audio
/// (460,800,000 samples = ~878.9 MiB virtual stream) without allocating the audio track in RAM.
struct SyntheticPcm8HourReader {
    total_samples: usize,
    emitted_samples: usize,
    samples_per_half_sec: usize,
}

impl SyntheticPcm8HourReader {
    fn new_8h() -> Self {
        Self {
            total_samples: EIGHT_HOURS_SEC * DEFAULT_SAMPLE_RATE,
            emitted_samples: 0,
            samples_per_half_sec: DEFAULT_SAMPLE_RATE / 2,
        }
    }

    #[inline]
    fn sample_amplitude_for_block(block_idx: usize) -> i16 {
        // Each block = 0.5s => sec = block_idx / 2
        let sec = block_idx / 2;
        let is_peak = PEAK_TIMESTAMPS_SEC.iter().any(|&p| sec >= p && sec < p + 2);
        if is_peak {
            // +24 dB scream spike over ambient (~0.85 normalized PCM)
            28_000
        } else {
            // Ambient background stream level (~ -36 dBFS)
            520
        }
    }
}

impl Read for SyntheticPcm8HourReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.emitted_samples >= self.total_samples || buf.len() < 2 {
            return Ok(0);
        }

        let max_samples_in_buf = buf.len() / 2;
        let remaining_samples = self.total_samples - self.emitted_samples;
        let mut remaining_to_write = max_samples_in_buf.min(remaining_samples);
        let mut buf_offset = 0;

        while remaining_to_write > 0 {
            let block_idx = self.emitted_samples / self.samples_per_half_sec;
            let block_end = (block_idx + 1) * self.samples_per_half_sec;
            let run_samples = remaining_to_write.min(block_end - self.emitted_samples);
            let amp = Self::sample_amplitude_for_block(block_idx);
            let pos_bytes = amp.to_le_bytes();
            let neg_bytes = (-amp).to_le_bytes();

            let mut written_in_run = 0;
            if !self.emitted_samples.is_multiple_of(2) {
                buf[buf_offset] = neg_bytes[0];
                buf[buf_offset + 1] = neg_bytes[1];
                buf_offset += 2;
                written_in_run = 1;
            }

            let pair_samples = (run_samples - written_in_run) & !1;
            let pair_bytes = pair_samples * 2;
            let pattern = [pos_bytes[0], pos_bytes[1], neg_bytes[0], neg_bytes[1]];
            let (chunks, _) = buf[buf_offset..buf_offset + pair_bytes].as_chunks_mut::<4>();
            for chunk in chunks {
                *chunk = pattern;
            }
            buf_offset += pair_bytes;
            written_in_run += pair_samples;

            if written_in_run < run_samples {
                buf[buf_offset] = pos_bytes[0];
                buf[buf_offset + 1] = pos_bytes[1];
                buf_offset += 2;
            }

            self.emitted_samples += run_samples;
            remaining_to_write -= run_samples;
        }

        Ok(buf_offset)
    }
}

/// Generate 500,000 chat messages across an 8-hour VOD with 5 hype bursts and realistic spam-bot traffic.
fn generate_500k_chat_events() -> Vec<ChatEvent> {
    let mut events = Vec::with_capacity(CHAT_EVENT_COUNT);
    let background_msgs = [
        "Neuro is so smart",
        "Tutel doing math again",
        "KEKW good stream",
        "Wait what did she just say ICANT",
        "Classic Vedal moment",
    ];
    let hype_msgs = [
        "KEKW KEKW KEKW 💀",
        "Tutel HEART neuroHeart",
        "Aware OMEGALUL WutFace",
        "PogBones GIGACHAD KEKW",
    ];

    for i in 0..CHAT_EVENT_COUNT {
        // Reserve ~10% (50k messages) into concentrated bursts around PEAK_TIMESTAMPS_SEC,
        // and spread remaining ~90% evenly across 8 hours (5,760 buckets of 5s).
        let (ts, author, msg, is_swarm) = if i % 10 == 0 {
            let peak_sec = PEAK_TIMESTAMPS_SEC[(i / 10) % PEAK_TIMESTAMPS_SEC.len()];
            let sub_offset = ((i % 40) as f64) * 0.1;
            let timestamp = (peak_sec as f64) + sub_offset;
            // Include periodic spam-bot flooding from one author in burst buckets
            let bot_or_user = if i % 50 == 0 {
                "spambot_raid_01".to_string()
            } else {
                format!("hype_u{}", i % 800)
            };
            (
                timestamp,
                bot_or_user,
                hype_msgs[i % hype_msgs.len()].to_string(),
                true,
            )
        } else {
            let bucket = i % EXPECTED_5S_BUCKETS;
            let timestamp = (bucket as f64) * 5.0 + ((i % 49) as f64) * 0.1;
            (
                timestamp,
                format!("u{}", i % 2_000),
                background_msgs[i % background_msgs.len()].to_string(),
                i % 3 == 0,
            )
        };

        events.push(ChatEvent {
            timestamp_sec: ts,
            author,
            message: msg,
            badges: Vec::new(),
            emotes_detected: Vec::new(),
            is_swarm_emote: is_swarm,
        });
    }

    events
}

fn run_heavy_highlight_workload() -> (usize, usize, usize, f64, OsMemorySnapshot, usize) {
    // 1. Stream 8-hour audio track in O(1) chunk memory (921.6 MB PCM stream)
    let reader = SyntheticPcm8HourReader::new_8h();
    let audio_blocks =
        process_pcm_reader(reader, DEFAULT_SAMPLE_RATE, DEFAULT_BLOCK_DURATION_SEC).unwrap();
    let block_count = audio_blocks.len();
    let audio_points = detect_energy_peaks_default(&audio_blocks).unwrap();
    drop(audio_blocks);

    // 2. Pump 500,000 chat messages through Z-Score + Swarm Emote Density
    let events = generate_500k_chat_events();
    let chat_series = compute_chat_velocity_series_from_events(
        &events,
        DEFAULT_BUCKET_WIDTH_SEC,
        DEFAULT_BASELINE_WINDOW_SEC,
        Some(EIGHT_HOURS_F64),
    )
    .unwrap();
    let emote_series =
        compute_emote_density_series_default(&events, Some(EIGHT_HOURS_F64)).unwrap();
    let sample_peak_heap_mb = bytes_to_mb(peak_heap_bytes());
    let sample_os_during = sample_os_memory();
    drop(events);

    // 3. Multimodal fusion & clustering across the 8h stream
    let fused = fuse_multimodal_series(&chat_series, &emote_series, &audio_points, EIGHT_HOURS_F64)
        .unwrap();
    let clusters =
        generate_clusters_from_fused_series("vod_low_end_8h_500k", &fused, EIGHT_HOURS_F64)
            .unwrap();

    (
        block_count,
        chat_series.len(),
        emote_series.len(),
        sample_peak_heap_mb,
        sample_os_during,
        clusters.clusters.len(),
    )
}

// ===========================================================================
// AC-31.1: Peak RAM <= 500 MB (500k Chat + 8h Audio Stream)
// ===========================================================================

#[test]
fn test_ac_31_1_500k_chat_and_8h_audio_stress_peak_ram_under_500mb() {
    let _guard = acquire_bench_lock();

    // Warm up one-time lazy statics (SwarmDictionary) before profiling
    let _ = default_swarm_dictionary();
    println!("[AC-31.1 BENCHMARK] Starting 8h Audio + 500,000 Chat Stress Benchmark...");

    let baseline_heap = reset_peak_heap();
    let baseline_os = sample_os_memory();
    let start = Instant::now();

    let (block_count, chat_buckets, emote_buckets, workload_peak_heap_mb, os_during, cluster_count) =
        run_heavy_highlight_workload();

    let elapsed = start.elapsed();
    let post_heap = current_heap_bytes();
    let post_os = sample_os_memory();
    let peak_heap_mb = bytes_to_mb(peak_heap_bytes()).max(workload_peak_heap_mb);
    let post_heap_mb = bytes_to_mb(post_heap);

    println!(
        "[AC-31.1 BENCHMARK] Completed in {:.2}s | blocks={block_count} buckets={chat_buckets}/{emote_buckets} clusters={cluster_count}",
        elapsed.as_secs_f64()
    );
    println!(
        "[AC-31.1 MEMORY] Baseline Heap={:.2} MB | Peak Heap={peak_heap_mb:.2} MB (cap <= {PEAK_RAM_LIMIT_MB:.0} MB) | Post-Drop Heap={post_heap_mb:.2} MB",
        bytes_to_mb(baseline_heap)
    );
    println!(
        "[AC-31.1 OS MEM] Baseline WS={:.2} MB | Peak WS={:.2} MB | During WS={:.2} MB | During Commit={:.2} MB (Peak Commit={:.2} MB) | Post WS={:.2} MB | Post Commit={:.2} MB",
        baseline_os.working_set_mb(),
        post_os.peak_working_set_mb(),
        os_during.working_set_mb(),
        os_during.private_commit_mb(),
        post_os.peak_private_commit_mb(),
        post_os.working_set_mb(),
        post_os.private_commit_mb(),
    );

    assert_eq!(block_count, EXPECTED_AUDIO_BLOCKS);
    assert_eq!(chat_buckets, EXPECTED_5S_BUCKETS);
    assert_eq!(emote_buckets, EXPECTED_5S_BUCKETS);
    assert!(
        cluster_count >= 5,
        "Expected >= 5 detected highlight clusters across 8h VOD bursts, got {cluster_count}"
    );

    assert!(
        peak_heap_mb <= PEAK_RAM_LIMIT_MB,
        "AC-31.1 FAILED: Peak tracked heap {peak_heap_mb:.2} MB > {PEAK_RAM_LIMIT_MB:.0} MB limit"
    );
    assert!(
        os_during.peak_working_set_mb() <= PEAK_RAM_LIMIT_MB,
        "AC-31.1 FAILED: OS Peak WorkingSet {:.2} MB > {PEAK_RAM_LIMIT_MB:.0} MB limit",
        os_during.peak_working_set_mb()
    );
    assert!(
        os_during.peak_private_commit_mb() <= PEAK_RAM_LIMIT_MB,
        "AC-31.1 FAILED: OS Peak PrivateCommit {:.2} MB > {PEAK_RAM_LIMIT_MB:.0} MB limit",
        os_during.peak_private_commit_mb()
    );
    assert!(
        post_heap_mb <= IDLE_RAM_LIMIT_MB,
        "AC-31.2 FAILED: Post-workload live heap {post_heap_mb:.2} MB > {IDLE_RAM_LIMIT_MB:.0} MB"
    );
    assert!(
        post_os.working_set_mb() <= IDLE_RAM_LIMIT_MB,
        "AC-31.2 FAILED: Post-workload OS WorkingSet {:.2} MB > {IDLE_RAM_LIMIT_MB:.0} MB",
        post_os.working_set_mb()
    );
}

// ===========================================================================
// AC-31.2: 5 Consecutive Highlight Clips in Sustained Session
// ===========================================================================

#[test]
fn test_ac_31_2_five_consecutive_highlight_clips_sustained_session() {
    let _guard = acquire_bench_lock();
    let (audio_fixture, chat_fixture) = fixture_paths();
    assert!(
        audio_fixture.exists(),
        "Missing synthetic_audio.wav fixture"
    );
    assert!(chat_fixture.exists(), "Missing synthetic_chat.json fixture");

    // Simulate 4-core low-end PC concurrency constraints
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(2)
        .enable_all()
        .build()
        .unwrap();

    println!("[AC-31.2 SESSION] Running 5 consecutive E2E highlight clips...");
    let _ = reset_peak_heap();

    let mut post_clip_heap_bytes = [0usize; 5];
    let mut post_clip_os_ws_mb = [0.0f64; 5];
    let mut post_clip_durations_ms = [0u64; 5];

    for idx in 0..5 {
        let clip_num = idx + 1;
        let layout = if idx % 2 == 0 {
            "original_16_9"
        } else {
            "preset_center_crop_9_16"
        };

        {
            let temp_dir = TempTestDir::new(&format!("clip_{clip_num}"));
            let options = SyntheticE2EOptions {
                audio_fixture_path: audio_fixture.to_string_lossy().to_string(),
                chat_fixture_path: chat_fixture.to_string_lossy().to_string(),
                output_dir: Some(temp_dir.path().to_string_lossy().to_string()),
                vod_id: Some(format!("vod_stress_clip_{clip_num}")),
                target_layout: Some(layout.to_string()),
            };

            let report = runtime
                .block_on(run_synthetic_e2e_pipeline_internal(options))
                .expect("Consecutive E2E clip must succeed");

            assert!(
                report.passed && report.schemas.all_passed(),
                "Clip {clip_num} failed validation"
            );
            post_clip_durations_ms[idx] = report.total_duration_ms;
        }

        // Capture memory cleanly into stack arrays before printing
        post_clip_heap_bytes[idx] = current_heap_bytes();
        let os_snap = sample_os_memory();
        post_clip_os_ws_mb[idx] = os_snap.working_set_mb();
    }

    let final_os = sample_os_memory();
    let peak_heap_mb = bytes_to_mb(peak_heap_bytes());
    drop(runtime);
    let after_rt_drop_heap_mb = bytes_to_mb(current_heap_bytes());

    for idx in 0..5 {
        println!(
            "[AC-31.2 SESSION] Clip {}: duration={}ms | Post-Clip Live Heap={:.2} MB | OS WorkingSet={:.2} MB",
            idx + 1,
            post_clip_durations_ms[idx],
            bytes_to_mb(post_clip_heap_bytes[idx]),
            post_clip_os_ws_mb[idx]
        );
    }
    println!(
        "[AC-31.2 SUMMARY] Peak Heap={peak_heap_mb:.2} MB | OS Peak WS={:.2} MB | Post-Runtime Drop Heap={after_rt_drop_heap_mb:.2} MB | Final OS WS={:.2} MB",
        final_os.peak_working_set_mb(),
        final_os.working_set_mb()
    );

    assert!(
        peak_heap_mb <= PEAK_RAM_LIMIT_MB,
        "AC-31.1 FAILED during 5-clip run: Peak Heap {peak_heap_mb:.2} MB > {PEAK_RAM_LIMIT_MB:.0} MB"
    );
    assert!(
        final_os.peak_working_set_mb() <= PEAK_RAM_LIMIT_MB,
        "AC-31.1 FAILED during 5-clip run: OS Peak WS {:.2} MB > {PEAK_RAM_LIMIT_MB:.0} MB",
        final_os.peak_working_set_mb()
    );
    assert!(
        after_rt_drop_heap_mb <= IDLE_RAM_LIMIT_MB && final_os.working_set_mb() <= IDLE_RAM_LIMIT_MB,
        "AC-31.2 FAILED: Memory failed to stay <= {IDLE_RAM_LIMIT_MB:.0} MB (heap={after_rt_drop_heap_mb:.2} MB, ws={:.2} MB)",
        final_os.working_set_mb()
    );

    // Ensure no monotonic leak across sustained clips 2..=5 (1 MiB jitter tolerance here;
    // strict 0-byte leak check is asserted in `memory_leak_check.rs`).
    let drift_bytes = post_clip_heap_bytes[4].saturating_sub(post_clip_heap_bytes[1]);
    assert!(
        drift_bytes <= 1_048_576,
        "Sustained session growth between clip 2 and clip 5 was {drift_bytes} bytes (> 1 MiB)"
    );
}

// ===========================================================================
// AC-31.3: UI / Event-Loop Responsiveness <= 100 ms Under Heavy Load
// ===========================================================================

#[test]
fn test_ac_31_3_ui_event_loop_responsiveness_under_100ms_during_heavy_load() {
    let _guard = acquire_bench_lock();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(2)
        .enable_all()
        .build()
        .unwrap();

    println!(
        "[AC-31.3 UI LATENCY] Monitoring 10ms Tokio event-loop ticks during heavy 500k+8h load..."
    );

    let (tick_count, max_interval_ms, max_lag_ms, total_workload_ms) = runtime.block_on(async {
        let stop_flag = std::sync::Arc::new(AtomicBool::new(false));
        let stop_heartbeat = std::sync::Arc::clone(&stop_flag);

        let heartbeat = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(10));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            interval.tick().await; // consume immediate first tick

            let mut last_tick = Instant::now();
            let mut ticks: usize = 0;
            let mut worst_interval_ms: f64 = 0.0;
            let mut worst_lag_ms: f64 = 0.0;

            while !stop_heartbeat.load(Ordering::Relaxed) {
                interval.tick().await;
                let now = Instant::now();
                let dt_ms = now.duration_since(last_tick).as_secs_f64() * 1000.0;
                last_tick = now;
                ticks += 1;
                if dt_ms > worst_interval_ms {
                    worst_interval_ms = dt_ms;
                }
                let lag_ms = (dt_ms - 10.0).max(0.0);
                if lag_ms > worst_lag_ms {
                    worst_lag_ms = lag_ms;
                }
            }

            (ticks, worst_interval_ms, worst_lag_ms)
        });

        // Give the heartbeat a brief spin-up tick before starting heavy blocking offload
        tokio::time::sleep(Duration::from_millis(25)).await;

        let work_start = Instant::now();
        let worker_result = tokio::task::spawn_blocking(run_heavy_highlight_workload)
            .await
            .expect("Heavy highlight worker must not panic");
        let work_ms = work_start.elapsed().as_secs_f64() * 1000.0;

        stop_flag.store(true, Ordering::Relaxed);
        let (ticks, worst_interval_ms, worst_lag_ms) = heartbeat
            .await
            .expect("Heartbeat monitor must join cleanly");

        assert_eq!(worker_result.0, EXPECTED_AUDIO_BLOCKS);
        assert_eq!(worker_result.1, EXPECTED_5S_BUCKETS);
        assert_eq!(worker_result.2, EXPECTED_5S_BUCKETS);

        (ticks, worst_interval_ms, worst_lag_ms, work_ms)
    });

    println!(
        "[AC-31.3 UI LATENCY] workload={total_workload_ms:.1}ms | heartbeat_ticks={tick_count} | max_tick_interval={max_interval_ms:.2}ms (cap <= {MAX_UI_STALL_MS:.0}ms) | max_lag={max_lag_ms:.2}ms"
    );

    assert!(
        tick_count >= 10,
        "Expected >= 10 heartbeat ticks during workload, observed {tick_count}"
    );
    assert!(
        max_interval_ms <= MAX_UI_STALL_MS,
        "AC-31.3 FAILED: Async event loop stalled {max_interval_ms:.2}ms (> {MAX_UI_STALL_MS:.0}ms limit)"
    );
}

// ===========================================================================
// Low-End PC Hardware Profile + Simulated 2GB VRAM CUDA OOM Guardrail (TASK-M4-16)
// ===========================================================================

#[test]
fn test_low_end_pc_2gb_vram_cuda_oom_guardrail_and_hardware_profile() {
    let _guard = acquire_bench_lock();

    // Profile A: 4-core CPU, 8GB RAM, Intel iGPU (0MB dedicated VRAM, no CUDA)
    let profile_igpu = HardwareProfile {
        cpu_info: CpuInfo {
            model: "Intel Core i3-12100".to_string(),
            cores: 4,
            threads: 4,
        },
        ram_info: RamInfo {
            total_mb: 8192,
            available_mb: 4096,
        },
        gpu_info: GpuInfo {
            vendor: GpuVendor::Intel,
            model: "Intel UHD Graphics 730".to_string(),
            vram_mb: 0,
            cuda_supported: false,
        },
        recommended_mode: RecommendedMode::CloudByok,
        recommendation_reason: "Cấu hình iGPU Intel: Đề xuất dùng Cloud BYOK.".to_string(),
        timestamp: "2026-11-24T23:00:00Z".to_string(),
    };

    let (mode_a, _) = evaluate_recommended_mode(
        &profile_igpu.cpu_info,
        &profile_igpu.ram_info,
        &profile_igpu.gpu_info,
    );
    assert_eq!(mode_a, RecommendedMode::CloudByok);
    validate_schema_5_1_hardware_profile(&serde_json::to_value(&profile_igpu).unwrap()).unwrap();

    let strict_igpu_err = evaluate_hardware_guardrail_from_profile(
        &profile_igpu,
        "large-v3",
        "float16",
        "cuda",
        GuardrailPolicy::StrictReject,
    )
    .expect_err("iGPU 0MB VRAM requesting Whisper Large CUDA must be strictly rejected");
    assert_eq!(strict_igpu_err.code(), codes::ERR_STT_VRAM_INSUFFICIENT);

    let downgrade_igpu = evaluate_hardware_guardrail_from_profile(
        &profile_igpu,
        "large-v3",
        "float16",
        "cuda",
        GuardrailPolicy::AutoDowngrade,
    )
    .unwrap();
    assert!(downgrade_igpu.was_downgraded);
    assert_eq!(downgrade_igpu.effective_device, "cpu");
    assert_eq!(downgrade_igpu.effective_model, "medium");
    assert_eq!(downgrade_igpu.effective_quantization, "int8");

    // Profile B: 4-core CPU, 8GB RAM, Simulated 2GB VRAM CUDA GPU (2048 MB < MIN_LARGE_VRAM_MB 2500 MB)
    let profile_2gb_vram = HardwareProfile {
        cpu_info: CpuInfo {
            model: "AMD Ryzen 3 4100".to_string(),
            cores: 4,
            threads: 4,
        },
        ram_info: RamInfo {
            total_mb: 8192,
            available_mb: 4200,
        },
        gpu_info: GpuInfo {
            vendor: GpuVendor::Nvidia,
            model: "NVIDIA GeForce MX550 (2GB VRAM Simulated)".to_string(),
            vram_mb: 2048,
            cuda_supported: true,
        },
        recommended_mode: RecommendedMode::CloudByok,
        recommendation_reason: "VRAM < 3GB: Đề xuất dùng Cloud BYOK.".to_string(),
        timestamp: "2026-11-24T23:00:00Z".to_string(),
    };

    let (mode_b, _) = evaluate_recommended_mode(
        &profile_2gb_vram.cpu_info,
        &profile_2gb_vram.ram_info,
        &profile_2gb_vram.gpu_info,
    );
    assert_eq!(mode_b, RecommendedMode::CloudByok);
    validate_schema_5_1_hardware_profile(&serde_json::to_value(&profile_2gb_vram).unwrap())
        .unwrap();

    // 1. StrictReject on Large model under 2048 MB VRAM (< 2500 MB) -> ERR_STT_VRAM_INSUFFICIENT
    let strict_err = check_hardware_guardrail_from_profile(&profile_2gb_vram, "large-v3")
        .expect_err("2GB VRAM (2048 MB < 2500 MB) must reject Whisper Large");
    assert!(matches!(
        strict_err,
        AppError::Stt(SttError::InsufficientVram { available_mb: 2048 })
    ));
    assert_eq!(strict_err.code(), codes::ERR_STT_VRAM_INSUFFICIENT);
    assert!(profile_2gb_vram.gpu_info.vram_mb < MIN_LARGE_VRAM_MB);

    // 2. StrictReject on float32 precision exceeding 1.8GB safe ceiling under <= 4GB GPU
    let strict_fp32_err = evaluate_hardware_guardrail_from_profile(
        &profile_2gb_vram,
        "medium",
        "float32",
        "cuda",
        GuardrailPolicy::StrictReject,
    )
    .expect_err("2GB VRAM running medium float32 (3300 MB) must fail StrictReject");
    assert_eq!(strict_fp32_err.code(), codes::ERR_STT_VRAM_INSUFFICIENT);

    // 3. AutoDowngrade clamps 2GB VRAM request from Large+FP16 to Medium+INT8 (<= 1800 MB < 2048 MB)
    let downgrade_2gb = evaluate_hardware_guardrail_from_profile(
        &profile_2gb_vram,
        "large-v3",
        "float16",
        "cuda",
        GuardrailPolicy::AutoDowngrade,
    )
    .unwrap();
    assert!(downgrade_2gb.was_downgraded);
    assert_eq!(downgrade_2gb.effective_model, "medium");
    assert_eq!(downgrade_2gb.effective_quantization, "int8");
    let estimated_vram = estimate_vram_usage_mb(
        &downgrade_2gb.effective_model,
        &downgrade_2gb.effective_quantization,
    );
    assert!(
        estimated_vram <= MAX_INT8_VRAM_CAP_MB,
        "Downgraded VRAM estimate ({estimated_vram} MB) must fit inside {MAX_INT8_VRAM_CAP_MB} MB cap"
    );
    assert!(
        estimated_vram <= profile_2gb_vram.gpu_info.vram_mb,
        "Downgraded VRAM estimate ({estimated_vram} MB) must fit inside simulated 2GB GPU"
    );

    // 4. Tighter 1GB VRAM tier (1024 MB) auto-downgrades all the way down to Small+INT8 (850 MB)
    let mut profile_1gb = profile_2gb_vram;
    profile_1gb.gpu_info.vram_mb = 1024;
    let downgrade_1gb = evaluate_hardware_guardrail_from_profile(
        &profile_1gb,
        "large-v3",
        "float16",
        "cuda",
        GuardrailPolicy::AutoDowngrade,
    )
    .unwrap();
    assert_eq!(downgrade_1gb.effective_model, "small");
    assert_eq!(downgrade_1gb.effective_quantization, "int8");
    assert!(estimate_vram_usage_mb("small", "int8") < profile_1gb.gpu_info.vram_mb);

    println!(
        "[2GB VRAM GUARDRAIL] 2048MB StrictReject -> {} | AutoDowngrade -> {} {} (est {}MB <= {}MB cap)",
        strict_err.code(),
        downgrade_2gb.effective_model,
        downgrade_2gb.effective_quantization,
        estimated_vram,
        MAX_INT8_VRAM_CAP_MB
    );
}
