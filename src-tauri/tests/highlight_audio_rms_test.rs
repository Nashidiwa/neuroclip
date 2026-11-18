#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Integration and Benchmark Test Suite for TASK-M4-11:
//! Audio RMS Peak Detector & Streaming Energy Analyzer.
//!
//! Complies with `docs/CONVENTIONS.md` (§7 Testing & QA), `docs/ARCHITECTURE.md` §4.3 & §5.4,
//! and `docs/TASK_BOARD.md` §TASK-M4-11.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use neuroclip_lib::{
    analyze_audio_samples_default, analyze_wav_file, apply_silence_clamp, calculate_raw_rms,
    calculate_sine_normalized_rms, clamp_silence_floor, dbfs_to_normalized_scale, dbfs_to_rms,
    detect_energy_peaks_default, rms_to_dbfs, AudioBlockMetrics, AudioRmsPoint,
    DEFAULT_BLOCK_SAMPLES, MIN_RMS_THRESHOLD, SILENCE_FLOOR_DBFS, SINE_1KHZ_EXPECTED_RMS,
    SINE_1KHZ_RMS_TOLERANCE,
};

/// Helper directory for creating temporary test files with auto-cleanup.
struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "neuroclip_test_audio_rms_{name}_{}_{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        let _ = std::fs::create_dir_all(&path);
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Helper to generate a synthetic sine wave of given frequency, duration, sample rate, and amplitude.
fn generate_sine_wave(
    freq_hz: f64,
    duration_sec: f64,
    sample_rate: usize,
    amplitude: f64,
) -> Vec<i16> {
    let total_samples = ((sample_rate as f64) * duration_sec).round() as usize;
    let mut samples = Vec::with_capacity(total_samples);
    let amp = amplitude.clamp(0.0, 32767.0);

    for i in 0..total_samples {
        let theta = 2.0 * std::f64::consts::PI * freq_hz * (i as f64) / (sample_rate as f64);
        let val = (amp * theta.sin()).round() as i16;
        samples.push(val);
    }
    samples
}

/// Helper to generate silence of given duration in seconds.
fn generate_silence(duration_sec: f64, sample_rate: usize) -> Vec<i16> {
    let total_samples = ((sample_rate as f64) * duration_sec).round() as usize;
    vec![0i16; total_samples]
}

/// Helper to write a valid 16-bit PCM WAV file.
fn write_wav_file(
    path: &Path,
    samples: &[i16],
    sample_rate: u32,
    channels: u16,
) -> std::io::Result<()> {
    let mut file = File::create(path)?;
    let subchunk2_size = (samples.len() * 2) as u32;
    let chunk_size = 36 + subchunk2_size;
    let byte_rate = sample_rate * (u32::from(channels)) * 2;
    let block_align = channels * 2;

    // RIFF chunk descriptor
    file.write_all(b"RIFF")?;
    file.write_all(&chunk_size.to_le_bytes())?;
    file.write_all(b"WAVE")?;

    // fmt subchunk
    file.write_all(b"fmt ")?;
    file.write_all(&16u32.to_le_bytes())?; // Subchunk1Size (16 for PCM)
    file.write_all(&1u16.to_le_bytes())?; // AudioFormat (1 for PCM)
    file.write_all(&channels.to_le_bytes())?;
    file.write_all(&sample_rate.to_le_bytes())?;
    file.write_all(&byte_rate.to_le_bytes())?;
    file.write_all(&block_align.to_le_bytes())?;
    file.write_all(&16u16.to_le_bytes())?; // BitsPerSample (16-bit)

    // data subchunk
    file.write_all(b"data")?;
    file.write_all(&subchunk2_size.to_le_bytes())?;
    for &sample in samples {
        file.write_all(&sample.to_le_bytes())?;
    }

    file.flush()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// UT-11.1: Sóng sin 1kHz RMS ~ 0.7071 (±0.005) & Silence Floor Clamp -96dBFS
// ---------------------------------------------------------------------------

#[test]
fn test_ut_11_1_sine_1khz_rms_and_silence_floor_clamp() {
    let sample_rate = 16_000;
    let duration_sec = 0.5; // Exactly 1 block of 500ms = 8000 samples

    // 1. Full-scale 1kHz sine wave (amplitude 32767)
    let sine_samples = generate_sine_wave(1000.0, duration_sec, sample_rate, 32767.0);
    assert_eq!(sine_samples.len(), DEFAULT_BLOCK_SAMPLES);

    let raw_rms = calculate_raw_rms(&sine_samples);
    let delta = (raw_rms - SINE_1KHZ_EXPECTED_RMS).abs();
    assert!(
        delta <= SINE_1KHZ_RMS_TOLERANCE,
        "Raw RMS {raw_rms} deviates by {delta} from expected {SINE_1KHZ_EXPECTED_RMS} (tolerance {SINE_1KHZ_RMS_TOLERANCE})"
    );

    // Sine-peak normalized RMS should evaluate to 1.0
    let norm_rms = calculate_sine_normalized_rms(&sine_samples);
    assert!(
        (0.999..=1.0).contains(&norm_rms),
        "Sine normalized RMS should be ~1.0, got {norm_rms}"
    );

    // 2. Silence floor clamping at -96dBFS -> RMS = 0.0
    let silence = generate_silence(0.5, sample_rate);
    let silence_rms = calculate_raw_rms(&silence);
    assert_eq!(silence_rms, 0.0);
    assert_eq!(calculate_sine_normalized_rms(&silence), 0.0);

    let silence_dbfs = rms_to_dbfs(silence_rms);
    assert_eq!(silence_dbfs, SILENCE_FLOOR_DBFS);

    let (clamped_rms, clamped_db) = clamp_silence_floor(silence_rms);
    assert_eq!(clamped_rms, 0.0);
    assert_eq!(clamped_db, SILENCE_FLOOR_DBFS);

    // Signals below or equal to MIN_RMS_THRESHOLD (-96dBFS linear) clamp to 0.0
    assert_eq!(apply_silence_clamp(MIN_RMS_THRESHOLD), 0.0);
    assert_eq!(apply_silence_clamp(1e-6), 0.0);
    assert_eq!(rms_to_dbfs(MIN_RMS_THRESHOLD), -96.0);
    assert_eq!(dbfs_to_rms(-96.0), 0.0);
    assert_eq!(dbfs_to_normalized_scale(-96.0), 0.0);
}

// ---------------------------------------------------------------------------
// UT-11.2: 30s Baseline Window & Peak Threshold >= 2.5 Sigma
// ---------------------------------------------------------------------------

#[test]
fn test_ut_11_2_peak_threshold_sigma_over_30s_baseline() {
    let sample_rate = 16_000;
    // 30 seconds of baseline = 60 blocks of 500ms
    let baseline_blocks_count = 60;
    let mut all_samples = Vec::new();

    // Generate baseline audio with predictable variance around 0.10 (std dev ~ 0.02)
    for i in 0..baseline_blocks_count {
        let amp = if i % 2 == 0 { 3276.0 } else { 4000.0 };
        let block_pcm = generate_sine_wave(440.0, 0.5, sample_rate, amp);
        all_samples.extend_from_slice(&block_pcm);
    }

    // Block 60: inject a statistical outlier with large amplitude (amplitude 10,000, > 3.0 sigma)
    let outlier_pcm = generate_sine_wave(440.0, 0.5, sample_rate, 10_000.0);
    all_samples.extend_from_slice(&outlier_pcm);

    // Block 61: inject a moderate amplitude (amplitude 3600, within normal baseline variance)
    let normal_pcm = generate_sine_wave(440.0, 0.5, sample_rate, 3600.0);
    all_samples.extend_from_slice(&normal_pcm);

    let points = analyze_audio_samples_default(&all_samples, sample_rate).unwrap();
    assert_eq!(points.len(), 62);

    // Baseline blocks (0..59) should not be peaks
    for (i, pt) in points.iter().take(baseline_blocks_count).enumerate() {
        assert!(
            !pt.is_peak,
            "Baseline block {i} should not be flagged as peak"
        );
    }

    // Block 60 is an outlier >= 2.5 sigma and must be flagged as peak
    assert!(
        points[60].is_peak,
        "Block 60 outlier must be flagged is_peak = true (Z-score: {:?})",
        points[60].z_score
    );
    assert_eq!(points[60].timestamp_sec, 30.0);
    if let Some(z) = points[60].z_score {
        assert!(z >= 2.5, "Z-score should be >= 2.5, got {z}");
    }

    // Block 61 returns to normal level and must NOT be flagged as peak
    assert!(
        !points[61].is_peak,
        "Block 61 returning to baseline must NOT be flagged as peak"
    );
}

// ---------------------------------------------------------------------------
// UT-11.3: Sudden Volume Spike (+21dB) Sets is_peak = true at 500ms Block
// ---------------------------------------------------------------------------

#[test]
fn test_ut_11_3_sudden_volume_spike_21db_sets_is_peak() {
    let sample_rate = 16_000;
    // 30 seconds of quiet conversational level at -30 dBFS
    // -30 dBFS linear RMS is 10^(-30/20) ≈ 0.0316227
    // For sine wave, peak amplitude = RMS * sqrt(2) * 32768 ≈ 0.0316227 * 1.4142 * 32768 ≈ 1466
    let baseline_amp = 1466.0;
    let mut stream_samples = Vec::new();

    for _ in 0..60 {
        let block_pcm = generate_sine_wave(500.0, 0.5, sample_rate, baseline_amp);
        stream_samples.extend_from_slice(&block_pcm);
    }

    // At 30.0s (block 60), sudden shout occurs at -9.0 dBFS (+21dB relative surge)
    // -9 dBFS linear RMS is 10^(-9/20) ≈ 0.354813
    // For sine wave, peak amplitude = 0.354813 * 1.4142 * 32768 ≈ 16443
    let spike_amp = 16443.0;
    let spike_block_pcm = generate_sine_wave(1000.0, 0.5, sample_rate, spike_amp);
    stream_samples.extend_from_slice(&spike_block_pcm);

    // Followed by 1 block of return to baseline
    let after_block_pcm = generate_sine_wave(500.0, 0.5, sample_rate, baseline_amp);
    stream_samples.extend_from_slice(&after_block_pcm);

    let points = analyze_audio_samples_default(&stream_samples, sample_rate).unwrap();
    assert_eq!(points.len(), 62);

    // Steady baseline before spike
    for (i, pt) in points.iter().take(60).enumerate() {
        assert!(
            !pt.is_peak,
            "Baseline block {i} at -30dBFS should not be a peak"
        );
    }

    // Block 60: The +21dB spike
    let peak_point = &points[60];
    assert!(
        peak_point.is_peak,
        "Sudden +21dB volume spike at block 60 MUST be flagged is_peak = true"
    );
    assert_eq!(peak_point.timestamp_sec, 30.0);
    if let Some(delta) = peak_point.delta_db {
        assert!(
            (delta - 21.0).abs() < 1.0,
            "Expected ~+21dB volume delta over baseline, got {delta} dB"
        );
    }
}

// ---------------------------------------------------------------------------
// IT-11.1: 500ms Block Resolution Output Array audio_rms_series
// ---------------------------------------------------------------------------

#[test]
fn test_it_11_1_block_resolution_500ms_series_contract() {
    let sample_rate = 16_000;
    let duration_sec = 10.0; // 10 seconds = exactly 20 blocks of 500ms
    let samples = generate_sine_wave(440.0, duration_sec, sample_rate, 5000.0);

    let points = analyze_audio_samples_default(&samples, sample_rate).unwrap();
    assert_eq!(points.len(), 20, "10 seconds must yield exactly 20 blocks");

    for (idx, point) in points.iter().enumerate() {
        let expected_ts = (idx as f64) * 0.5;
        assert!(
            (point.timestamp_sec - expected_ts).abs() < 1e-6,
            "Block {idx} timestamp {} mismatch expected {expected_ts}",
            point.timestamp_sec
        );
        assert!(
            point.rms_energy >= 0.0 && point.rms_energy <= 1.0,
            "Block {idx} rms_energy {} must be clamped in [0.0, 1.0]",
            point.rms_energy
        );
    }

    // Verify JSON serialization conforms to Schema 5.4 contract
    let json_val = serde_json::to_value(&points[0]).unwrap();
    assert!(json_val.get("timestamp_sec").is_some());
    assert!(json_val.get("rms_energy").is_some());
    assert!(json_val.get("is_peak").is_some());
}

// ---------------------------------------------------------------------------
// ED-11.1: 8-Hour Audio Streaming Processing Benchmark (<= 45s, RAM <= 60MB)
// ---------------------------------------------------------------------------

#[test]
fn test_ed_11_1_8h_audio_streaming_benchmark() {
    // 8 hours of audio: 28,800 seconds = 57,600 blocks of 500ms
    let total_blocks = 57_600;
    let block_samples = 8_000;

    println!("Starting ED-11.1 8-hour audio streaming simulation ({total_blocks} blocks)...");
    let start_time = Instant::now();

    // Stream 57,600 blocks in chunks of 500ms through AudioBlockMetrics
    let mut blocks = Vec::with_capacity(total_blocks);
    let sample_chunk = vec![1200i16; block_samples];

    for i in 0..total_blocks {
        let raw_rms = calculate_raw_rms(&sample_chunk);
        let dbfs = rms_to_dbfs(raw_rms);
        let normalized_rms = apply_silence_clamp(raw_rms);
        let timestamp_sec = (i as f64) * 0.5;

        blocks.push(AudioBlockMetrics {
            index: i,
            timestamp_sec,
            sample_count: block_samples,
            raw_rms,
            dbfs,
            normalized_rms,
        });
    }

    // Run O(N) sliding baseline peak detector over all 57,600 blocks
    let points = detect_energy_peaks_default(&blocks).unwrap();
    let elapsed = start_time.elapsed();

    println!(
        "ED-11.1 Benchmark completed in {:.3} seconds ({:.1}x realtime)",
        elapsed.as_secs_f64(),
        28_800.0 / elapsed.as_secs_f64()
    );

    assert_eq!(points.len(), total_blocks);
    assert!(
        elapsed.as_secs_f64() <= 45.0,
        "8-hour audio streaming processing took {:.2}s, exceeding SLA limit of 45s!",
        elapsed.as_secs_f64()
    );

    // Verify bounded memory footprint:
    // 57,600 points * ~48 bytes/point ≈ 2.7 MB, which is << 60MB limit
    let estimated_points_mem_bytes = points.len() * std::mem::size_of::<AudioRmsPoint>();
    let estimated_points_mem_mb = (estimated_points_mem_bytes as f64) / (1024.0 * 1024.0);
    println!(
        "Estimated output series memory: {:.2} MB (SLA <= 60 MB)",
        estimated_points_mem_mb
    );
    assert!(
        estimated_points_mem_mb < 60.0,
        "Output memory {:.2}MB exceeds 60MB constraint",
        estimated_points_mem_mb
    );
}

// ---------------------------------------------------------------------------
// ED-11.2: Screaming Loudness (> 18dB) Flagged 100%
// ---------------------------------------------------------------------------

#[test]
fn test_ed_11_2_screaming_loudness_flagged_100_percent() {
    let sample_rate = 16_000;
    let test_scenarios = [
        // (baseline_dbfs, scream_spike_db)
        (-45.0, 20.0), // Whisper baseline (-45dB) to Neuro scream (-25dB, +20dB jump)
        (-35.0, 19.0), // Quiet room (-35dB) to Vedal shout (-16dB, +19dB jump)
        (-28.0, 22.0), // Normal chatter (-28dB) to jump-scare screech (-6dB, +22dB jump)
        (-24.0, 18.5), // Game audio (-24dB) to high-pitched laugh (-5.5dB, +18.5dB jump)
        (-25.0, 20.0), // Active stream (-25dB) to loud shout (-5.0dB, +20dB jump)
    ];

    for (case_idx, &(baseline_db, spike_db)) in test_scenarios.iter().enumerate() {
        let baseline_rms = 10.0_f64.powf(baseline_db / 20.0);
        let spike_dbfs = baseline_db + spike_db;
        let spike_rms = 10.0_f64.powf(spike_dbfs / 20.0);

        let baseline_amp = (baseline_rms * std::f64::consts::SQRT_2 * 32768.0).min(32767.0);
        let spike_amp = (spike_rms * std::f64::consts::SQRT_2 * 32768.0).min(32767.0);

        let mut stream = Vec::new();
        // 30s baseline (60 blocks)
        for _ in 0..60 {
            let pcm = generate_sine_wave(440.0, 0.5, sample_rate, baseline_amp);
            stream.extend_from_slice(&pcm);
        }

        // Screaming spike at block 60
        let scream_pcm = generate_sine_wave(1200.0, 0.5, sample_rate, spike_amp);
        stream.extend_from_slice(&scream_pcm);

        let points = analyze_audio_samples_default(&stream, sample_rate).unwrap();
        assert_eq!(points.len(), 61);

        assert!(
            points[60].is_peak,
            "Scenario #{case_idx} (baseline {baseline_db}dB, +{spike_db}dB scream) MUST be flagged is_peak = true!"
        );
        assert_eq!(points[60].timestamp_sec, 30.0);
    }
}

// ---------------------------------------------------------------------------
// Integration Test: WAV File Streaming & Stereo Downmixing
// ---------------------------------------------------------------------------

#[test]
fn test_it_wav_file_streaming_and_stereo_downmix() {
    let test_dir = TestDir::new("wav_test");
    let mono_path = test_dir.path().join("test_mono.wav");
    let stereo_path = test_dir.path().join("test_stereo.wav");

    let sample_rate = 16_000;
    let duration_sec = 2.0; // 4 blocks of 500ms

    // 1. Mono WAV file
    let mono_samples = generate_sine_wave(800.0, duration_sec, sample_rate, 10_000.0);
    write_wav_file(&mono_path, &mono_samples, sample_rate as u32, 1).unwrap();

    let mono_points = analyze_wav_file(&mono_path).unwrap();
    assert_eq!(mono_points.len(), 4);
    assert!((mono_points[0].timestamp_sec - 0.0).abs() < 1e-6);
    assert!((mono_points[1].timestamp_sec - 0.5).abs() < 1e-6);
    assert!((mono_points[2].timestamp_sec - 1.0).abs() < 1e-6);
    assert!((mono_points[3].timestamp_sec - 1.5).abs() < 1e-6);

    // 2. Stereo WAV file (interleaved left/right)
    let mut stereo_samples = Vec::with_capacity(mono_samples.len() * 2);
    for &sample in &mono_samples {
        stereo_samples.push(sample); // Left channel
        stereo_samples.push(sample); // Right channel
    }
    write_wav_file(&stereo_path, &stereo_samples, sample_rate as u32, 2).unwrap();

    let stereo_points = analyze_wav_file(&stereo_path).unwrap();
    assert_eq!(stereo_points.len(), 4);
    for i in 0..4 {
        assert!(
            (stereo_points[i].rms_energy - mono_points[i].rms_energy).abs() < 0.01,
            "Stereo downmixed RMS at block {i} should closely match mono RMS"
        );
    }
}

// ---------------------------------------------------------------------------
// Adversarial / Edge Cases: Corrupted Files, Empty Streams, Silence
// ---------------------------------------------------------------------------

#[test]
fn test_adversarial_corrupted_wav_returns_safe_error() {
    let test_dir = TestDir::new("corrupt_wav");
    let invalid_path = test_dir.path().join("corrupted.wav");

    // Write incomplete header (< 44 bytes)
    let mut f = File::create(&invalid_path).unwrap();
    f.write_all(b"RIFF\x10\x00\x00\x00").unwrap();
    drop(f);

    let res = analyze_wav_file(&invalid_path);
    assert!(res.is_err(), "Corrupted WAV file must safely return Err");
}

#[test]
fn test_adversarial_pure_digital_silence_stream() {
    let sample_rate = 16_000;
    let silence = generate_silence(35.0, sample_rate); // 35 seconds of zeros (70 blocks)

    let points = analyze_audio_samples_default(&silence, sample_rate).unwrap();
    assert_eq!(points.len(), 70);

    for p in &points {
        assert_eq!(p.rms_energy, 0.0, "Pure silence must have rms_energy = 0.0");
        assert!(!p.is_peak, "Pure silence block must never be a peak");
        if let Some(db) = p.dbfs {
            assert_eq!(db, -96.0, "Silence dBFS must clamp to -96.0 dBFS");
        }
    }
}
