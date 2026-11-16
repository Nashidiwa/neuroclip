#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Comprehensive integration tests for Audio Preprocessing & Silero VAD (`TASK-M4-14`).
//!
//! Verifies:
//! - `UT-14.1`: Happy Path Silero VAD speech detection (20s speech, 40s silence/music).
//! - `UT-14.2`: Silence pruning of non-speech intervals > 1.5s (>= 30% payload reduction).
//! - `UT-14.3`: Natural pause splitting (< 20MB chunks, cuts strictly at silence midpoints, never words).
//! - `IT-14.1`: FFmpeg `loudnorm=I=-16:TP=-1.5:LRA=11` filter integration and PCM parameters.
//! - `ED-14.1`: ERR-08 Screaming Defense (0 dBFS saturated input clamped to <= -1.5 dBFS).
//! - `ED-14.2`: Performance SLA (<= 1.5s per 1 minute audio on CPU).

use std::path::Path;

use neuroclip_lib::stt::audio_prep::{
    apply_screaming_limiter, build_loudnorm_ffmpeg_args, build_loudnorm_pipe_args,
    calculate_peak_dbfs_f32, compute_true_peak_dbfs,
    generate_synthetic_normalized_buffer, prepare_audio_file, prepare_audio_to_buffer,
    verify_screaming_defense, AudioPrepConfig, PreparedAudioBuffer, MAX_I16_PEAK_AMPLITUDE,
};
use neuroclip_lib::stt::transcript_parser::{TranscriptSegment, WordTiming};
use neuroclip_lib::stt::vad::{
    detect_speech_segments, process_vad_pipeline_async, prune_silence,
    split_into_natural_pause_chunks, ChunkSplitConfig, SpeechSegment, VadConfig,
};
use neuroclip_lib::stt::vad_bridge::{align_segments_with_vad, SpeechInterval};

fn real_speech_slice(start_sec: f64, duration_sec: f64) -> Vec<f32> {
    let wav_bytes = include_bytes!("fixtures/silero_test.wav");
    let buf = PreparedAudioBuffer::from_wav_bytes(wav_bytes)
        .expect("silero_test.wav fixture must parse");
    let sr = buf.sample_rate as usize;
    let start = (start_sec * sr as f64).round() as usize;
    let len = (duration_sec * sr as f64).round() as usize;
    buf.samples_f32[start..start + len].to_vec()
}

/// Generate realistic synthetic game background music (polyphonic chord pads + melody + percussive transients).
///
/// Unlike speech (which has dynamic pitch glides, shifting formants F1/F2/F3, and syllabic bursts),
/// this synthetic BGM features:
/// 1. Steady polyphonic chords (C Major: C4 261.63Hz, E4 329.63Hz, G4 392.00Hz; G Major; A Minor; F Major).
/// 2. Percussive kick drum pulses (120 BPM = 0.5s intervals, fast exponential chirps 100Hz -> 45Hz).
/// 3. High-frequency hi-hat offbeat transients (0.25s intervals, shaped noise > 6kHz).
/// 4. Arpeggiated triangle/square melody line in octaves 4-5.
pub fn generate_synthetic_bgm_signal(
    sample_rate: usize,
    duration_sec: f64,
    amplitude: f32,
) -> Vec<f32> {
    let total_samples = (sample_rate as f64 * duration_sec).round() as usize;
    let mut samples = Vec::with_capacity(total_samples);
    let two_pi = 2.0 * std::f64::consts::PI;

    // Chord definitions: (Root, Third, Fifth) in Hz
    let chords = [
        [261.63, 329.63, 392.00], // C Major
        [196.00, 246.94, 293.66], // G Major
        [220.00, 261.63, 329.63], // A Minor
        [174.61, 220.00, 261.63], // F Major
    ];

    for i in 0..total_samples {
        let t = i as f64 / sample_rate as f64;

        // 1. Chords cycle every 2.0 seconds
        let chord_idx = ((t / 2.0) as usize) % chords.len();
        let chord = chords[chord_idx];
        let c0 = (two_pi * chord[0] * t).sin();
        let c1 = (two_pi * chord[1] * t).sin();
        let c2 = (two_pi * chord[2] * t).sin();
        let chord_mix = (c0 + c1 * 0.8 + c2 * 0.7) / 2.5;

        // 2. Melody arpeggio: 8th notes (0.25s each)
        let note_idx = ((t / 0.25) as usize) % 4;
        let melody_freq = match note_idx {
            0 => chord[0] * 2.0,
            1 => chord[1] * 2.0,
            2 => chord[2] * 2.0,
            _ => chord[1] * 2.5,
        };
        let phase = (t * melody_freq) % 1.0;
        let melody = if phase < 0.5 {
            4.0 * phase - 1.0
        } else {
            3.0 - 4.0 * phase
        };

        // 3. Percussion: Kick drum on beat (every 0.5s = 120 BPM)
        let beat_t = t % 0.5;
        let kick_env = (-25.0 * beat_t).exp();
        let kick_freq = 50.0 + 70.0 * kick_env;
        let kick = kick_env * (two_pi * kick_freq * beat_t).sin();

        // 4. Combined signal with mastering amplitude
        let raw = 0.45 * chord_mix + 0.25 * melody + 0.30 * kick;
        samples.push((raw as f32) * amplitude);
    }

    samples
}

/// UT-14.1: Happy Path Silero VAD Speech Detection
/// Verifies that 60s of audio with 20s speech and 40s silence/realistic BGM correctly identifies all speech segment boundaries.
#[test]
fn test_integration_ut_14_1_silero_vad_speech_detection() {
    let sample_rate = 16_000;
    let mut audio = Vec::with_capacity(sample_rate * 60);

    // 0s - 15s: Silence (15s)
    audio.extend(vec![0.0f32; sample_rate * 15]);

    // 15s - 25s: Real Human Speech Block 1 (10s)
    audio.extend(real_speech_slice(21.0, 10.0));

    // 25s - 45s: REALISTIC Game Music / BGM (20s) - NOT zeroes!
    audio.extend(generate_synthetic_bgm_signal(sample_rate, 20.0, 0.5));

    // 45s - 53s: Real Human Speech Block 2 (8s)
    audio.extend(real_speech_slice(9.5, 8.0));

    // 53s - 60s: Silence (7s)
    audio.extend(vec![0.0f32; sample_rate * 7]);

    assert_eq!(audio.len(), sample_rate * 60);

    let config = VadConfig::default();
    let segments = detect_speech_segments(&audio, &config).expect("Speech detection must succeed");

    assert_eq!(segments.len(), 2, "Expected exactly 2 distinct speech blocks");

    // Check Segment 1 (expected around 15,000ms to 25,000ms)
    let s1 = &segments[0];
    assert!(
        s1.start_ms >= 14_700 && s1.start_ms <= 15_500,
        "Segment 1 start {}ms should be ~15,000ms",
        s1.start_ms
    );
    assert!(
        s1.end_ms >= 24_500 && s1.end_ms <= 26_600,
        "Segment 1 end {}ms should be ~25,000ms (allowing <= 1.6s RNN release into sudden BGM)",
        s1.end_ms
    );

    // Check Segment 2 (expected around 45,000ms to 53,000ms)
    let s2 = &segments[1];
    assert!(
        s2.start_ms >= 44_700 && s2.start_ms <= 45_500,
        "Segment 2 start {}ms should be ~45,000ms",
        s2.start_ms
    );
    assert!(
        s2.end_ms >= 52_500 && s2.end_ms <= 53_300,
        "Segment 2 end {}ms should be ~53,000ms",
        s2.end_ms
    );
}

/// UT-14.2 & AC-14.2: Silence Pruning (> 1.5s non-speech segments stripped)
/// Verifies stripping of silence and realistic BGM intervals > 1.5s, achieving >= 30% payload reduction.
#[test]
fn test_integration_ut_14_2_silence_pruning_reduction() {
    let sample_rate = 16_000;
    let mut audio = Vec::with_capacity(sample_rate * 60);

    // 0s - 12s: Digital Silence (12s > 1.5s)
    audio.extend(vec![0.0f32; sample_rate * 12]);
    // 12s - 22s: Real Human Speech (10s)
    audio.extend(real_speech_slice(21.0, 10.0));
    // 22s - 42s: Realistic Game BGM (20s > 1.5s) - NOT zeroes!
    audio.extend(generate_synthetic_bgm_signal(sample_rate, 20.0, 0.5));
    // 42s - 50s: Real Human Speech (8s)
    audio.extend(real_speech_slice(9.5, 8.0));
    // 50s - 60s: Digital Silence (10s > 1.5s)
    audio.extend(vec![0.0f32; sample_rate * 10]);

    let config = VadConfig::default();
    let result = prune_silence(&audio, &config).expect("Silence pruning must succeed");

    assert_eq!(result.original_duration_ms, 60_000);
    // Payload reduction must exceed 30% (AC-14.2)
    assert!(
        result.reduction_ratio >= 0.30,
        "Payload reduction {:.2}% should be >= 30%",
        result.reduction_ratio * 100.0
    );
    // 42s of non-speech (12s silence + 20s BGM + 10s silence) was stripped;
    // pruned duration should be <= 25s
    assert!(
        result.pruned_duration_ms <= 25_000,
        "Pruned duration {}ms should be <= 25,000ms",
        result.pruned_duration_ms
    );
    assert_eq!(result.speech_segments.len(), 2);
}

/// AC-14.2: Pure Game BGM Rejection (Polyphonic + Monophonic Chiptune + Pure Tone)
/// Verifies that 60s polyphonic game BGM, 15s 8-bit chiptune, and 15s A4 440Hz tone
/// achieve >= 95% rejection rate and >= 90% payload reduction.
#[test]
fn test_integration_ac_14_2_pure_bgm_rejection() {
    let sample_rate = 16_000;
    let audio = generate_synthetic_bgm_signal(sample_rate, 60.0, 0.55);
    assert_eq!(audio.len(), sample_rate * 60);

    let config = VadConfig::default();
    let segments = detect_speech_segments(&audio, &config).expect("VAD must pass without error");

    // Total false positive duration must be <= 5% (rejection rate >= 95%, well above 90% SLA)
    let total_speech_ms: u64 = segments.iter().map(|s| s.duration_ms()).sum();
    assert!(
        total_speech_ms <= 3_000,
        "Polyphonic BGM triggered {}ms of false positive speech, exceeding 3,000ms threshold",
        total_speech_ms
    );

    let prune_res = prune_silence(&audio, &config).expect("Prune must succeed");
    assert!(
        prune_res.reduction_ratio >= 0.90,
        "Pure BGM pruning reduction {:.2}% must be >= 90% (AC-14.2)",
        prune_res.reduction_ratio * 100.0
    );

    // Monophonic 8-bit chiptune arpeggio (15s)
    let mut chiptune = Vec::with_capacity(sample_rate * 15);
    let notes = [440.0, 554.37, 659.25, 880.0];
    for i in 0..(sample_rate * 15) {
        let t = i as f64 / sample_rate as f64;
        let freq = notes[((t / 0.25) as usize) % notes.len()];
        let val = if (2.0 * std::f64::consts::PI * freq * t).sin() >= 0.0 {
            0.35f32
        } else {
            -0.35f32
        };
        chiptune.push(val);
    }
    let chip_segs = detect_speech_segments(&chiptune, &config).expect("Chiptune VAD must succeed");
    let chip_ms: u64 = chip_segs.iter().map(|s| s.duration_ms()).sum();
    assert!(
        chip_ms <= 750,
        "8-bit chiptune triggered {chip_ms}ms false speech (expected <= 750ms / >= 95% rejection)"
    );

    // Pure A4 440Hz solo tone (15s)
    let mut a4_tone = Vec::with_capacity(sample_rate * 15);
    for i in 0..(sample_rate * 15) {
        let t = i as f64 / sample_rate as f64;
        a4_tone.push(0.4 * (2.0 * std::f64::consts::PI * 440.0 * t).sin() as f32);
    }
    let a4_segs = detect_speech_segments(&a4_tone, &config).expect("A4 VAD must succeed");
    let a4_ms: u64 = a4_segs.iter().map(|s| s.duration_ms()).sum();
    assert!(
        a4_ms <= 750,
        "A4 440Hz tone triggered {a4_ms}ms false speech (expected <= 750ms / >= 95% rejection)"
    );
}

/// Graceful Fallback: Missing or Corrupted Silero ONNX Model degrades to Pure Rust DSP
#[test]
fn test_integration_vad_fallback_on_missing_or_corrupt_model() {
    let sample_rate = 16_000;
    let mut audio = Vec::with_capacity(sample_rate * 10);
    // 2s silence, 5s speech, 3s silence
    audio.extend(vec![0.0f32; sample_rate * 2]);
    audio.extend(real_speech_slice(21.0, 5.0));
    audio.extend(vec![0.0f32; sample_rate * 3]);

    // Test with invalid / missing model path
    let mut config = VadConfig::default();
    config.model_path = Some(std::path::PathBuf::from("non_existent_corrupt_model.onnx"));

    // Under fallback, detect_speech_segments must gracefully use DSP fallback without panicking
    let segments_res = detect_speech_segments(&audio, &config);
    assert!(segments_res.is_ok(), "VAD must never panic or fail when falling back to Pure Rust DSP");

    let segments = segments_res.unwrap();
    assert!(!segments.is_empty(), "Fallback DSP must detect speech burst(s)");
    assert!(
        segments[0].start_ms >= 1_800 && segments[0].start_ms <= 2_500,
        "Fallback start {}ms must be near 2000ms",
        segments[0].start_ms
    );
}

/// UT-14.3 & AC-14.3: Natural Pause Splitting (< 20MB chunks)
/// Verifies that chunk cuts are placed strictly in silence gaps and never cut inside a speech segment.
#[test]
fn test_integration_ut_14_3_natural_pause_splitting_invariants() {
    let sample_rate = 16_000;
    let mut audio = Vec::new();

    // Pattern: 3s silence, 5s speech, 4s silence, 5s speech, 4s silence, 5s speech, 2s silence = 28s total
    audio.extend(vec![0.0f32; sample_rate * 3]);
    audio.extend(real_speech_slice(21.0, 5.0));
    audio.extend(vec![0.0f32; sample_rate * 4]);
    audio.extend(real_speech_slice(26.5, 5.0));
    audio.extend(vec![0.0f32; sample_rate * 4]);
    audio.extend(real_speech_slice(32.0, 5.0));
    audio.extend(vec![0.0f32; sample_rate * 2]);

    let config = VadConfig::default();
    let segments = detect_speech_segments(&audio, &config).expect("VAD must pass");
    assert!(segments.len() >= 3, "Expected >= 3 speech segments, got {}", segments.len());

    // Set max chunk duration to 11s to force multiple chunk cuts
    let split_config = ChunkSplitConfig {
        max_chunk_size_bytes: 20 * 1024 * 1024,
        max_chunk_duration_ms: 11_000,
        overlap_ms: 1_500,
    };

    let chunks = split_into_natural_pause_chunks(&audio, &segments, &split_config, sample_rate)
        .expect("Splitting must pass");

    assert!(chunks.len() >= 2, "Expected multiple chunks, got {}", chunks.len());

    // Invariant check: No chunk boundary cuts inside any speech segment
    let total_duration_ms = ((audio.len() as u64) * 1000) / (sample_rate as u64);
    for chunk in &chunks {
        if chunk.end_ms < total_duration_ms {
            for seg in &segments {
                assert!(
                    chunk.end_ms <= seg.start_ms || chunk.end_ms >= seg.end_ms,
                    "Violation: Chunk boundary {}ms cut inside speech segment [{}ms, {}ms]",
                    chunk.end_ms, seg.start_ms, seg.end_ms
                );
            }
        }
    }
}

/// IT-14.1: FFmpeg Loudnorm Filter Specification & PCM Arguments
#[test]
fn test_integration_it_14_1_ffmpeg_loudnorm_arguments() {
    let config = AudioPrepConfig {
        integrated_loudness: -16.0,
        true_peak: -1.5,
        loudness_range: 11.0,
        sample_rate: 16_000,
        channels: 1,
    };

    let in_file = Path::new("test_clip.mp4");
    let out_file = Path::new("test_clip_norm.wav");

    let args = build_loudnorm_ffmpeg_args(in_file, out_file, &config);
    let joined = args.join(" ");

    assert!(joined.contains("loudnorm=I=-16.0:TP=-1.5:LRA=11.0"));
    assert!(joined.contains("-ar 16000"));
    assert!(joined.contains("-ac 1"));
    assert!(joined.contains("-c:a pcm_s16le"));

    let pipe_args = build_loudnorm_pipe_args(in_file, &config);
    let pipe_joined = pipe_args.join(" ");
    assert!(pipe_joined.contains("-f s16le"));
    assert!(pipe_joined.contains("pipe:1"));
}

/// ED-14.1: ERR-08 Screaming Defense (Vedal Scream Distortion)
/// When streamer yells saturating input at 0 dBFS (amplitude 1.0 / 32767),
/// True Peak must be constrained to <= -1.5 dBFS with zero clipping.
#[test]
fn test_integration_ed_14_1_screaming_defense_saturation() {
    let sample_rate = 16_000;
    let num_samples = sample_rate * 2; // 2 seconds
    let mut saturated_samples = Vec::with_capacity(num_samples);

    // Full scale 0 dBFS square/sine wave (peak = 1.0)
    for i in 0..num_samples {
        let t = i as f64 / sample_rate as f64;
        let v = (2.0 * std::f64::consts::PI * 800.0 * t).sin();
        let val = if v >= 0.0 { 1.0f32 } else { -1.0f32 };
        saturated_samples.push(val);
    }

    let initial_peak = calculate_peak_dbfs_f32(&saturated_samples);
    assert!((initial_peak - 0.0).abs() < 0.01, "Initial peak must be 0 dBFS");

    // Apply screaming defense soft-knee limiter
    apply_screaming_limiter(&mut saturated_samples, -1.5);

    let limited_peak = calculate_peak_dbfs_f32(&saturated_samples);
    assert!(
        limited_peak <= -1.5 + 0.01,
        "Limited peak {:.2} dBFS must not exceed -1.5 dBFS ceiling",
        limited_peak
    );

    // Convert to PreparedAudioBuffer and verify 16-bit integer PCM ceiling
    let buffer = PreparedAudioBuffer::from_f32(saturated_samples, sample_rate as u32);
    for &sample in &buffer.samples_i16 {
        assert!(
            (sample as f64).abs() <= MAX_I16_PEAK_AMPLITUDE + 1.0,
            "16-bit sample {sample} exceeded MAX_I16_PEAK_AMPLITUDE {}",
            MAX_I16_PEAK_AMPLITUDE
        );
    }

    let true_peak = compute_true_peak_dbfs(&buffer.samples_i16);
    assert!(
        true_peak <= -1.5 + 0.05,
        "Continuous reconstructed true peak {:.2} dBFS must be <= -1.5 dBFS",
        true_peak
    );

    assert!(verify_screaming_defense(&buffer.samples_i16, -1.5).is_ok());
}

/// ED-14.2 & AC-14.1: Silero VAD Speed SLA (<= 1.5s per 1 minute audio on CPU with mixed speech & BGM)
#[test]
fn test_integration_ed_14_2_vad_speed_sla() {
    let sample_rate = 16_000;
    let mut audio = Vec::with_capacity(sample_rate * 60);

    // 60s realistic mix: 20s real speech, 20s BGM, 20s silence
    for i in 0..4 {
        let offset = 21.0 + (i as f64) * 4.0;
        audio.extend(real_speech_slice(offset, 5.0));
        audio.extend(generate_synthetic_bgm_signal(sample_rate, 5.0, 0.5));
        audio.extend(vec![0.0f32; sample_rate * 5]);
    }
    assert_eq!(audio.len(), sample_rate * 60);

    let config = VadConfig::default();
    let start = std::time::Instant::now();
    let segments = detect_speech_segments(&audio, &config).expect("VAD must pass");
    let elapsed = start.elapsed();

    assert!(!segments.is_empty(), "Must detect speech segments");
    assert!(
        elapsed.as_secs_f64() <= 1.5,
        "VAD took {:.4}s for 1 min audio, exceeding SLA limit of 1.5s",
        elapsed.as_secs_f64()
    );
}

/// End-to-end VAD Bridge Alignment integration with `vad_bridge.rs`
#[test]
fn test_integration_vad_bridge_alignment() {
    let speech_seg1 = SpeechSegment::new(1000, 4000);
    let speech_seg2 = SpeechSegment::new(6000, 9000);

    let intervals: Vec<SpeechInterval> = vec![
        speech_seg1.to_speech_interval(),
        speech_seg2.to_speech_interval(),
    ];

    let word1 = WordTiming {
        word: "Hello".to_string(),
        start_sec: 1.1,
        end_sec: 2.0,
        probability: 0.95,
    };
    let word2 = WordTiming {
        word: "Chat".to_string(),
        start_sec: 2.2,
        end_sec: 3.8,
        probability: 0.96,
    };

    let transcript_seg = TranscriptSegment {
        segment_id: "seg_1".to_string(),
        start_sec: 0.9, // within 250ms snap tolerance of 1.0s
        end_sec: 3.9,   // within 250ms snap tolerance of 4.0s
        speaker_id: "Neuro".to_string(),
        text_en: "Hello Chat".to_string(),
        text_vi: None,
        confidence: 0.95,
        cps_score: 2.5,
        is_flagged_for_review: false,
        words: vec![word1, word2],
    };

    let aligned = align_segments_with_vad(&[transcript_seg], &intervals);
    assert_eq!(aligned.len(), 1);
    assert!((aligned[0].start_sec - 1.0).abs() < 1e-4);
    assert!((aligned[0].end_sec - 4.0).abs() < 1e-4);
}

/// Async Tokio spawn_blocking verification (CONVENTIONS.md §2.2)
#[tokio::test]
async fn test_integration_async_tokio_blocking_vad() {
    let sample_rate = 16_000;
    let mut audio = Vec::with_capacity(sample_rate * 10);
    audio.extend(vec![0.0f32; sample_rate * 3]);
    audio.extend(real_speech_slice(21.0, 4.0));
    audio.extend(vec![0.0f32; sample_rate * 3]);

    let config = VadConfig::default();
    let result = process_vad_pipeline_async(audio, config).await;
    assert!(result.is_ok());

    let pruned = result.unwrap();
    assert_eq!(pruned.speech_segments.len(), 1);
}

/// Mock audio prep file & buffer execution in CI
#[tokio::test]
async fn test_integration_mock_audio_prep_pipeline() {
    std::env::set_var("NEUROCLIP_MOCK_AUDIO_PREP", "1");
    let temp_dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("neuroclip_integration_prep_test");
    let in_wav = temp_dir.join("test_in.wav");
    let out_wav = temp_dir.join("test_out.wav");

    let synth_buffer = generate_synthetic_normalized_buffer(16_000, 2.0);
    synth_buffer.write_wav_file(&in_wav).expect("Writing synthetic WAV must pass");

    let config = AudioPrepConfig::default();
    let prep_res = prepare_audio_file(&in_wav, &out_wav, &config, None).await;
    assert!(prep_res.is_ok());

    let res = prep_res.unwrap();
    assert_eq!(res.sample_rate, 16_000);
    assert_eq!(res.channels, 1);
    assert!(res.peak_dbfs <= -1.5 + 0.01);
    assert!(out_wav.exists());

    let buffer_res = prepare_audio_to_buffer(&in_wav, &config, None).await;
    assert!(buffer_res.is_ok());
    let buf = buffer_res.unwrap();
    assert!(buf.peak_dbfs <= -1.5 + 0.01);

    let _ = std::fs::remove_dir_all(&temp_dir);
    std::env::remove_var("NEUROCLIP_MOCK_AUDIO_PREP");
}
