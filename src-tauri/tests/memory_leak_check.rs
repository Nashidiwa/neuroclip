#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_lossless
)]

//! Memory Leak Check Suite (`TASK-M4-31`, Milestone M4.9).
//!
//! Enforces `AC-31.2`:
//! - **0 byte heap leak** across repeated 500,000-message + 8-hour audio stream cycles.
//! - **0 byte net heap growth** across 5 consecutive E2E highlight clip sessions after warmup.
//! - **Idle RAM <= 100 MB** verified after the 10-second post-task settle period using both the
//!   custom lock-free `TrackingAllocator` and Win32 kernel `K32GetProcessMemoryInfo`.

use std::alloc::{GlobalAlloc, Layout, System};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use neuroclip_lib::highlight::{
    compute_chat_velocity_series_from_events, compute_emote_density_series_default,
    default_swarm_dictionary, detect_energy_peaks_default, fuse_multimodal_series,
    generate_clusters_from_fused_series, process_pcm_reader, DEFAULT_BASELINE_WINDOW_SEC,
    DEFAULT_BLOCK_DURATION_SEC, DEFAULT_BUCKET_WIDTH_SEC, DEFAULT_SAMPLE_RATE,
};
use neuroclip_lib::ingestion::ChatEvent;
use neuroclip_lib::{run_synthetic_e2e_pipeline_internal, SyntheticE2EOptions};

// ===========================================================================
// Dual Memory Telemetry: Lock-free TrackingAllocator + Win32 K32GetProcessMemoryInfo
// ===========================================================================

struct TrackingAllocator;

static LIVE_HEAP_BYTES: AtomicUsize = AtomicUsize::new(0);
static PEAK_HEAP_BYTES: AtomicUsize = AtomicUsize::new(0);

#[inline]
fn record_heap_growth(delta_bytes: usize) {
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

// SAFETY: Wraps std::alloc::System using lock-free AtomicUsize accounting (never allocates).
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
        record_heap_shrink(layout.size());
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = unsafe { System.realloc(ptr, layout, new_size) };
        if !new_ptr.is_null() {
            let old_size = layout.size();
            if new_size >= old_size {
                record_heap_growth(new_size - old_size);
            } else {
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

        // SAFETY: Valid process handle and sized ProcessMemoryCounters buffer.
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
        }
    }
}

static LEAK_LOCK: Mutex<()> = Mutex::new(());

fn acquire_leak_lock() -> std::sync::MutexGuard<'static, ()> {
    LEAK_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

// ===========================================================================
// Constants & Deterministic Workload Helpers
// ===========================================================================

const PEAK_RAM_LIMIT_MB: f64 = 500.0;
const IDLE_RAM_LIMIT_MB: f64 = 100.0;

const EIGHT_HOURS_SEC: usize = 28_800;
const EIGHT_HOURS_F64: f64 = 28_800.0;
const CHAT_EVENT_COUNT: usize = 500_000;
const EXPECTED_5S_BUCKETS: usize = 5_760;
const EXPECTED_AUDIO_BLOCKS: usize = 57_600;
const PEAK_TIMESTAMPS_SEC: [usize; 5] = [1_800, 7_200, 14_400, 21_600, 25_200];

struct TempTestDir {
    path: PathBuf,
}

impl TempTestDir {
    fn new(label: &str) -> Self {
        let nanos = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let path = std::env::temp_dir().join(format!(
            "neuroclip_leak_{label}_{}_{nanos}",
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

struct SyntheticPcm8HourReader {
    total_samples: usize,
    emitted_samples: usize,
    samples_per_half_sec: usize,
}

impl SyntheticPcm8HourReader {
    fn new_8h() -> Self {
        Self::new_with_seconds(EIGHT_HOURS_SEC)
    }

    fn new_with_seconds(duration_sec: usize) -> Self {
        Self {
            total_samples: duration_sec * DEFAULT_SAMPLE_RATE,
            emitted_samples: 0,
            samples_per_half_sec: DEFAULT_SAMPLE_RATE / 2,
        }
    }

    #[inline]
    fn sample_amplitude_for_block(block_idx: usize) -> i16 {
        let sec = block_idx / 2;
        if PEAK_TIMESTAMPS_SEC.iter().any(|&p| sec >= p && sec < p + 2) {
            28_000
        } else {
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

fn generate_500k_chat_events() -> Vec<ChatEvent> {
    let mut events = Vec::with_capacity(CHAT_EVENT_COUNT);
    for i in 0..CHAT_EVENT_COUNT {
        let (ts, author, msg, is_swarm) = if i % 10 == 0 {
            let peak_sec = PEAK_TIMESTAMPS_SEC[(i / 10) % PEAK_TIMESTAMPS_SEC.len()];
            (
                (peak_sec as f64) + ((i % 40) as f64) * 0.1,
                format!("hype_{}", i % 500),
                "KEKW Tutel Pog 💀".to_string(),
                true,
            )
        } else {
            let bucket = i % EXPECTED_5S_BUCKETS;
            (
                (bucket as f64) * 5.0 + ((i % 49) as f64) * 0.1,
                format!("u{}", i % 2_000),
                "Neuro is doing math".to_string(),
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

fn execute_one_heavy_500k_cycle() -> (usize, usize, usize, usize) {
    let blocks = process_pcm_reader(
        SyntheticPcm8HourReader::new_8h(),
        DEFAULT_SAMPLE_RATE,
        DEFAULT_BLOCK_DURATION_SEC,
    )
    .unwrap();
    let block_len = blocks.len();
    let audio_pts = detect_energy_peaks_default(&blocks).unwrap();
    drop(blocks);

    let events = generate_500k_chat_events();
    let chat_pts = compute_chat_velocity_series_from_events(
        &events,
        DEFAULT_BUCKET_WIDTH_SEC,
        DEFAULT_BASELINE_WINDOW_SEC,
        Some(EIGHT_HOURS_F64),
    )
    .unwrap();
    let emote_pts = compute_emote_density_series_default(&events, Some(EIGHT_HOURS_F64)).unwrap();
    drop(events);

    let fused = fuse_multimodal_series(&chat_pts, &emote_pts, &audio_pts, EIGHT_HOURS_F64).unwrap();
    let clusters =
        generate_clusters_from_fused_series("vod_leak_check", &fused, EIGHT_HOURS_F64).unwrap();

    (
        block_len,
        chat_pts.len(),
        emote_pts.len(),
        clusters.clusters.len(),
    )
}

fn warmup_highlight_pipeline_caches() {
    let blocks = process_pcm_reader(
        SyntheticPcm8HourReader::new_with_seconds(10),
        DEFAULT_SAMPLE_RATE,
        DEFAULT_BLOCK_DURATION_SEC,
    )
    .unwrap();
    let audio_pts = detect_energy_peaks_default(&blocks).unwrap();
    drop(blocks);

    let mut events = Vec::with_capacity(200);
    for i in 0..200 {
        events.push(ChatEvent {
            timestamp_sec: (i as f64) * 0.05,
            author: format!("warmup_u{}", i % 20),
            message: "KEKW Tutel Pog 💀".to_string(),
            badges: Vec::new(),
            emotes_detected: Vec::new(),
            is_swarm_emote: true,
        });
    }
    let chat_pts = compute_chat_velocity_series_from_events(
        &events,
        DEFAULT_BUCKET_WIDTH_SEC,
        DEFAULT_BASELINE_WINDOW_SEC,
        Some(10.0),
    )
    .unwrap();
    let emote_pts = compute_emote_density_series_default(&events, Some(10.0)).unwrap();
    drop(events);

    let fused = fuse_multimodal_series(&chat_pts, &emote_pts, &audio_pts, 10.0).unwrap();
    let _clusters = generate_clusters_from_fused_series("vod_warmup", &fused, 10.0).unwrap();
}

// ===========================================================================
// Test 1: Exact 0-Byte Heap Leak Across 3x (500k Chat + 8h Audio) Cycles
// ===========================================================================

#[test]
fn test_zero_byte_heap_leak_500k_chat_and_8h_audio_repeated_cycles() {
    let _guard = acquire_leak_lock();

    // Initialize lazy statics (SwarmDictionary), thread-local state, and stdout buffer
    // BEFORE recording baseline heap bytes so only true per-cycle retention is measured.
    let _ = default_swarm_dictionary();
    warmup_highlight_pipeline_caches();
    println!("[LEAK CHECK 1] Running 3 consecutive 500k-chat + 8h-audio cycles...");

    let baseline_bytes = reset_peak_heap();
    let mut after_cycle_bytes = [0usize; 3];
    let mut cycle_peaks_bytes = [0usize; 3];

    for idx in 0..3 {
        let _ = reset_peak_heap();
        let (blocks, chat_len, emote_len, clusters_len) = execute_one_heavy_500k_cycle();
        assert_eq!(blocks, EXPECTED_AUDIO_BLOCKS);
        assert_eq!(chat_len, EXPECTED_5S_BUCKETS);
        assert_eq!(emote_len, EXPECTED_5S_BUCKETS);
        assert!(clusters_len >= 5);

        // Capture heap metrics into stack arrays BEFORE printing
        cycle_peaks_bytes[idx] = peak_heap_bytes();
        after_cycle_bytes[idx] = current_heap_bytes();
    }

    let os_after = sample_os_memory();

    for idx in 0..3 {
        let leaked = after_cycle_bytes[idx] as isize - baseline_bytes as isize;
        println!(
            "[LEAK CHECK 1] Cycle {}: Peak Heap={:.2} MB | Post-Drop Heap={} bytes (delta={leaked:+} bytes)",
            idx + 1,
            bytes_to_mb(cycle_peaks_bytes[idx]),
            after_cycle_bytes[idx]
        );
    }
    println!(
        "[LEAK CHECK 1] Baseline={} bytes | OS WorkingSet={:.2} MB | OS Peak WS={:.2} MB | OS Commit={:.2} MB",
        baseline_bytes,
        os_after.working_set_mb(),
        os_after.peak_working_set_mb(),
        os_after.private_commit_mb()
    );

    for idx in 0..3 {
        assert_eq!(
            after_cycle_bytes[idx],
            baseline_bytes,
            "Memory leak detected in cycle {}: post={} bytes != baseline={} bytes",
            idx + 1,
            after_cycle_bytes[idx],
            baseline_bytes
        );
        assert!(
            bytes_to_mb(cycle_peaks_bytes[idx]) <= PEAK_RAM_LIMIT_MB,
            "Cycle {} exceeded Peak RAM cap: {:.2} MB > {PEAK_RAM_LIMIT_MB:.0} MB",
            idx + 1,
            bytes_to_mb(cycle_peaks_bytes[idx])
        );
    }

    assert!(
        os_after.peak_working_set_mb() <= PEAK_RAM_LIMIT_MB,
        "OS Peak WorkingSet {:.2} MB > {PEAK_RAM_LIMIT_MB:.0} MB",
        os_after.peak_working_set_mb()
    );
    assert!(
        os_after.working_set_mb() <= IDLE_RAM_LIMIT_MB,
        "OS WorkingSet {:.2} MB > {IDLE_RAM_LIMIT_MB:.0} MB after cleanup",
        os_after.working_set_mb()
    );
}

// ===========================================================================
// Test 2: 5 Consecutive Highlight Clips — 0-Byte Growth & 10s Settle <= 100MB
// ===========================================================================

#[test]
fn test_ac_31_2_five_consecutive_clips_zero_leak_and_10s_settle() {
    let _guard = acquire_leak_lock();
    let (audio_fixture, chat_fixture) = fixture_paths();
    assert!(
        audio_fixture.exists(),
        "Missing synthetic_audio.wav fixture"
    );
    assert!(chat_fixture.exists(), "Missing synthetic_chat.json fixture");

    // Deterministic single-worker current-thread runtime with long blocking keep-alive
    // so the warmup clip settles thread-local storage cleanly prior to measuring clips 1..=5.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .thread_keep_alive(Duration::from_secs(120))
        .build()
        .unwrap();

    // Warm up stdio formatting & one-time pipeline initialization (Silero VAD, SwarmDict, blocking worker TLS)
    println!("[LEAK CHECK 2] Warming up pipeline static caches before 5-clip leak audit...");
    {
        let warmup_dir = TempTestDir::new("warmup");
        let opts = SyntheticE2EOptions {
            audio_fixture_path: audio_fixture.to_string_lossy().to_string(),
            chat_fixture_path: chat_fixture.to_string_lossy().to_string(),
            output_dir: Some(warmup_dir.path().to_string_lossy().to_string()),
            vod_id: Some("vod_leak_warmup".to_string()),
            target_layout: Some("original_16_9".to_string()),
        };
        let warmup_report = runtime
            .block_on(run_synthetic_e2e_pipeline_internal(opts))
            .unwrap();
        assert!(warmup_report.passed && warmup_report.schemas.all_passed());
    }

    // Record exact post-warmup baseline into stack variable before clip loop
    let baseline_bytes = reset_peak_heap();
    let mut clip_live_bytes = [0usize; 5];
    let mut clip_peak_bytes = [0usize; 5];

    for idx in 0..5 {
        let clip_num = idx + 1;
        let layout = if idx % 2 == 0 {
            "original_16_9"
        } else {
            "preset_center_crop_9_16"
        };

        let _ = reset_peak_heap();
        {
            let temp_dir = TempTestDir::new(&format!("clip_{clip_num}"));
            let options = SyntheticE2EOptions {
                audio_fixture_path: audio_fixture.to_string_lossy().to_string(),
                chat_fixture_path: chat_fixture.to_string_lossy().to_string(),
                output_dir: Some(temp_dir.path().to_string_lossy().to_string()),
                vod_id: Some(format!("vod_leak_clip_{clip_num}")),
                target_layout: Some(layout.to_string()),
            };

            let report = runtime
                .block_on(run_synthetic_e2e_pipeline_internal(options))
                .expect("Clip run must pass");
            assert!(
                report.passed && report.schemas.all_passed(),
                "Clip {clip_num} schema/stage check failed"
            );
        }

        // Measure immediately after workspace drop BEFORE printing
        clip_peak_bytes[idx] = peak_heap_bytes();
        clip_live_bytes[idx] = current_heap_bytes();
    }

    // Observe 10-second post-task settle period per AC-31.2 QA Checklist
    let mut settle_heap_bytes = [0usize; 10];
    let mut settle_os_snaps = [OsMemorySnapshot::default(); 10];
    for sec in 0..10 {
        std::thread::sleep(Duration::from_secs(1));
        settle_heap_bytes[sec] = current_heap_bytes();
        settle_os_snaps[sec] = sample_os_memory();
    }

    // Drop the runtime after recording 10s settle metrics to verify final process idle footprint
    drop(runtime);
    let post_runtime_drop_bytes = current_heap_bytes();
    let final_os = sample_os_memory();

    println!(
        "[LEAK CHECK 2] Baseline post-warmup live heap: {baseline_bytes} bytes ({:.2} MB)",
        bytes_to_mb(baseline_bytes)
    );
    for idx in 0..5 {
        let delta = clip_live_bytes[idx] as isize - baseline_bytes as isize;
        println!(
            "[LEAK CHECK 2] Clip {}: Peak={:.2} MB | Post-Clip Live={} bytes (delta={delta:+} bytes)",
            idx + 1,
            bytes_to_mb(clip_peak_bytes[idx]),
            clip_live_bytes[idx]
        );
    }
    for sec in 0..10 {
        println!(
            "[LEAK CHECK 2] Settle T+{}s: Heap={:.2} MB ({} bytes) | OS WS={:.2} MB | OS Commit={:.2} MB",
            sec + 1,
            bytes_to_mb(settle_heap_bytes[sec]),
            settle_heap_bytes[sec],
            settle_os_snaps[sec].working_set_mb(),
            settle_os_snaps[sec].private_commit_mb()
        );
    }
    println!(
        "[LEAK CHECK 2] Final Post-Runtime Drop: Heap={:.2} MB ({post_runtime_drop_bytes} bytes) | OS WS={:.2} MB | OS Peak WS={:.2} MB | OS Commit={:.2} MB",
        bytes_to_mb(post_runtime_drop_bytes),
        final_os.working_set_mb(),
        final_os.peak_working_set_mb(),
        final_os.private_commit_mb()
    );

    // 1. Assert Peak RAM <= 500 MB across every clip
    for (idx, &peak_bytes) in clip_peak_bytes.iter().enumerate() {
        let peak_mb = bytes_to_mb(peak_bytes);
        assert!(
            peak_mb <= PEAK_RAM_LIMIT_MB,
            "AC-31.1 FAILED on clip {}: {peak_mb:.2} MB > {PEAK_RAM_LIMIT_MB:.0} MB",
            idx + 1,
        );
    }
    assert!(
        final_os.peak_working_set_mb() <= PEAK_RAM_LIMIT_MB,
        "AC-31.1 FAILED: OS Peak WorkingSet {:.2} MB > {PEAK_RAM_LIMIT_MB:.0} MB",
        final_os.peak_working_set_mb()
    );

    // 2. Assert 0-byte net retained growth across 5 consecutive clips after settle
    let settled_bytes = settle_heap_bytes[9];
    assert!(
        settled_bytes <= baseline_bytes,
        "AC-31.2 LEAK DETECTED: Post-10s settled live heap ({settled_bytes} bytes) exceeds post-warmup baseline ({baseline_bytes} bytes) by {} bytes",
        settled_bytes - baseline_bytes
    );
    assert!(
        clip_live_bytes[4] <= clip_live_bytes[0],
        "AC-31.2 LEAK DETECTED: Clip 5 live heap ({} bytes) grew above Clip 1 ({} bytes)",
        clip_live_bytes[4],
        clip_live_bytes[0]
    );

    // 3. Assert Idle RAM <= 100 MB across heap, OS WorkingSet, and OS PrivateCommit
    assert!(
        bytes_to_mb(settled_bytes) <= IDLE_RAM_LIMIT_MB,
        "AC-31.2 FAILED: Settled heap {:.2} MB > {IDLE_RAM_LIMIT_MB:.0} MB",
        bytes_to_mb(settled_bytes)
    );
    assert!(
        settle_os_snaps[9].working_set_mb() <= IDLE_RAM_LIMIT_MB,
        "AC-31.2 FAILED: Settled OS WorkingSet {:.2} MB > {IDLE_RAM_LIMIT_MB:.0} MB",
        settle_os_snaps[9].working_set_mb()
    );
    assert!(
        final_os.working_set_mb() <= IDLE_RAM_LIMIT_MB,
        "AC-31.2 FAILED: Final OS WorkingSet {:.2} MB > {IDLE_RAM_LIMIT_MB:.0} MB",
        final_os.working_set_mb()
    );
    assert!(
        final_os.private_commit_mb() <= IDLE_RAM_LIMIT_MB,
        "AC-31.2 FAILED: Final OS PrivateCommit {:.2} MB > {IDLE_RAM_LIMIT_MB:.0} MB",
        final_os.private_commit_mb()
    );
}
