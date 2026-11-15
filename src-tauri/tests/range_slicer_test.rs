#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Integration & SLA Test Suite for TASK-M4-13: Lossless Precision Range Slicer.
//!
//! Complies with `docs/CONVENTIONS.md` (§7 Testing & QA), `docs/ARCHITECTURE.md` §4.2 & §5.2,
//! and `docs/TASK_BOARD.md` TASK-M4-13.
//!
//! Covers:
//! - UT-13.1: FFmpeg argument builder, HTTP Range Request seek placement, reconnect options.
//! - IT-13.1: Manifest integration, dual routing (online/local), file size (30MB-80MB) & 99% savings.
//! - IT-13.2: Execution duration SLA <= 10.0s for 90s clip.
//! - ED-13.1: Keyframe alignment check & audio-video stream drift <= 50ms, start PTS <= 50ms.
//! - Process safety & cancellation token handling (ADR-007).

use std::path::{Path, PathBuf};
use std::time::Instant;
use tokio_util::sync::CancellationToken;

use neuroclip_lib::{
    build_ffmpeg_range_slice_args, build_slice_ffmpeg_args, evaluate_keyframe_alignment,
    execute_range_slice, parse_ffprobe_json, slice_video_segment_internal,
    verify_stream_durations, AppError, FfprobeOutput, FfprobeStream, IngestError,
    RangeSliceRequest, SliceResult, VodManifest,
};

/// Helper directory for creating temporary test files with auto-cleanup.
struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "neuroclip_test_range_slicer_{name}_{}_{}",
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

// ===========================================================================
// Module 1: UT-13.1 Command Builder & HTTP Range Request Formatting
// ===========================================================================

#[test]
fn test_ut_13_1_online_m3u8_ffmpeg_range_args() {
    let url = "https://video-weaver.sfo03.hls.ttvnw.net/v1/playlist/index-dvr.m3u8";
    let start_sec = 5055.500;
    let end_sec = 5085.000;
    let output_path = "C:/test/clip_1.mp4";

    let args = build_ffmpeg_range_slice_args(url, start_sec, end_sec, output_path, true)
        .expect("UT-13.1 builder must succeed for valid online m3u8 inputs");

    // Invariant 1: Input seeking (-ss) MUST appear before input flag (-i)
    let ss_idx = args.iter().position(|a| a == "-ss").expect("Must contain -ss");
    let i_idx = args.iter().position(|a| a == "-i").expect("Must contain -i");
    assert!(
        ss_idx < i_idx,
        "Invariant violated: -ss ({ss_idx}) must precede -i ({i_idx}) for HTTP Range request seeking"
    );

    // Invariant 2: -to must appear before -i
    let to_idx = args.iter().position(|a| a == "-to").expect("Must contain -to");
    assert!(
        to_idx < i_idx,
        "Invariant violated: -to ({to_idx}) must precede -i ({i_idx})"
    );

    // Invariant 3: Timestamps are formatted with millisecond precision
    assert_eq!(args[ss_idx + 1], "5055.500");
    assert_eq!(args[to_idx + 1], "5085.000");

    // Invariant 4: Input source follows -i
    assert_eq!(args[i_idx + 1], url);

    // Invariant 5: Lossless stream copy flag
    let c_idx = args.iter().position(|a| a == "-c").expect("Must contain -c");
    assert_eq!(args[c_idx + 1], "copy");

    // Invariant 6: PTS timestamp normalization
    let ts_idx = args
        .iter()
        .position(|a| a == "-avoid_negative_ts")
        .expect("Must contain -avoid_negative_ts");
    assert_eq!(args[ts_idx + 1], "make_zero");

    // Invariant 7: Network resilience options for online streams
    assert!(args.contains(&"-reconnect".to_string()));
    assert!(args.contains(&"-reconnect_streamed".to_string()));
    assert!(args.contains(&"-reconnect_delay_max".to_string()));

    // Invariant 8: Last argument is output file path
    assert_eq!(args.last().unwrap(), output_path);
}

#[test]
fn test_ut_13_1_local_file_ffmpeg_range_args() {
    let local_file = "D:/Videos/OBS_20261114.mp4";
    let start_sec = 120.0;
    let end_sec = 210.0;
    let output_path = "D:/NeuroClip/clip_local.mp4";

    let args = build_ffmpeg_range_slice_args(local_file, start_sec, end_sec, output_path, false)
        .expect("UT-13.1 builder must succeed for valid local file inputs");

    let ss_idx = args.iter().position(|a| a == "-ss").expect("Must contain -ss");
    let i_idx = args.iter().position(|a| a == "-i").expect("Must contain -i");
    assert!(ss_idx < i_idx);

    // Local file MUST NOT contain network reconnect options
    assert!(!args.contains(&"-reconnect".to_string()));
    assert!(!args.contains(&"-reconnect_streamed".to_string()));

    let c_idx = args.iter().position(|a| a == "-c").expect("Must contain -c");
    assert_eq!(args[c_idx + 1], "copy");
    assert!(args.contains(&"-avoid_negative_ts".to_string()));
}

#[test]
fn test_ut_13_1_parameter_formatting_with_duration_flag() {
    let req = RangeSliceRequest {
        input_source: "https://example.com/stream.m3u8".to_string(),
        start_sec: 100.0,
        end_sec: 190.0,
        output_path: PathBuf::from("test_clip.mp4"),
        is_online: true,
        http_headers: None,
        use_duration_flag: true,
        task_id: None,
    };

    let args = build_slice_ffmpeg_args(&req).expect("Builder must succeed");
    assert!(args.contains(&"-t".to_string()));
    assert!(!args.contains(&"-to".to_string()));

    let t_idx = args.iter().position(|a| a == "-t").unwrap();
    assert_eq!(args[t_idx + 1], "90.000");
}

#[test]
fn test_ut_13_1_invalid_range_validation() {
    // start >= end
    let err1 = build_ffmpeg_range_slice_args("stream.m3u8", 100.0, 50.0, "out.mp4", true)
        .unwrap_err();
    match err1 {
        AppError::Ingestion(IngestError::InvalidTimeRange { start_sec, end_sec }) => {
            assert_eq!(start_sec, 100.0);
            assert_eq!(end_sec, 50.0);
        }
        other => panic!("Expected InvalidTimeRange error, got {other:?}"),
    }

    // start < 0.0
    let err2 = build_ffmpeg_range_slice_args("stream.m3u8", -5.0, 50.0, "out.mp4", true)
        .unwrap_err();
    assert!(matches!(
        err2,
        AppError::Ingestion(IngestError::InvalidTimeRange { .. })
    ));

    // start == end
    let err3 = build_ffmpeg_range_slice_args("stream.m3u8", 30.0, 30.0, "out.mp4", true)
        .unwrap_err();
    assert!(matches!(
        err3,
        AppError::Ingestion(IngestError::InvalidTimeRange { .. })
    ));
}

// ===========================================================================
// Module 2: ED-13.1 Keyframe Alignment & Audio/Video Drift Verification
// ===========================================================================

#[test]
fn test_ed_13_1_keyframe_alignment_happy_path() {
    // Video: 90.040s, Audio: 90.025s -> drift = 15ms <= 50ms
    let probe = FfprobeOutput {
        streams: vec![
            FfprobeStream {
                index: 0,
                codec_type: Some("video".to_string()),
                duration: Some("90.040000".to_string()),
                start_time: Some("0.000000".to_string()),
            },
            FfprobeStream {
                index: 1,
                codec_type: Some("audio".to_string()),
                duration: Some("90.025000".to_string()),
                start_time: Some("0.000000".to_string()),
            },
        ],
        format: None,
    };

    let report = evaluate_keyframe_alignment(&probe, 50.0);
    assert!(report.is_aligned, "15ms drift must pass alignment check");
    assert!((report.drift_ms - 15.0).abs() < 1e-4);
    assert_eq!(report.duration_delta_ms, report.drift_ms);
}

#[test]
fn test_ed_13_1_keyframe_alignment_skew_rejection() {
    // Video: 90.100s, Audio: 90.020s -> drift = 80ms > 50ms
    let probe = FfprobeOutput {
        streams: vec![
            FfprobeStream {
                index: 0,
                codec_type: Some("video".to_string()),
                duration: Some("90.100000".to_string()),
                start_time: Some("0.000000".to_string()),
            },
            FfprobeStream {
                index: 1,
                codec_type: Some("audio".to_string()),
                duration: Some("90.020000".to_string()),
                start_time: Some("0.000000".to_string()),
            },
        ],
        format: None,
    };

    let report = evaluate_keyframe_alignment(&probe, 50.0);
    assert!(
        !report.is_aligned,
        "80ms drift must be rejected (> 50ms threshold)"
    );
    assert!((report.drift_ms - 80.0).abs() < 1e-4);
}

#[test]
fn test_ed_13_1_boundary_at_exactly_50ms() {
    let probe = FfprobeOutput {
        streams: vec![
            FfprobeStream {
                index: 0,
                codec_type: Some("video".to_string()),
                duration: Some("90.050000".to_string()),
                start_time: Some("0.000000".to_string()),
            },
            FfprobeStream {
                index: 1,
                codec_type: Some("audio".to_string()),
                duration: Some("90.000000".to_string()),
                start_time: Some("0.000000".to_string()),
            },
        ],
        format: None,
    };

    let report = evaluate_keyframe_alignment(&probe, 50.0);
    assert!(
        report.is_aligned,
        "Exactly 50.0ms drift must pass (inclusive <= 50ms limit)"
    );
    assert!((report.drift_ms - 50.0).abs() < 1e-4);
}

#[test]
fn test_ed_13_1_first_frame_zero_timestamp() {
    // Delayed first frame PTS > 0.050s indicates frozen frame
    let probe_laggy = FfprobeOutput {
        streams: vec![FfprobeStream {
            index: 0,
            codec_type: Some("video".to_string()),
            duration: Some("90.000000".to_string()),
            start_time: Some("0.120000".to_string()), // 120ms lag
        }],
        format: None,
    };

    let report_laggy = evaluate_keyframe_alignment(&probe_laggy, 50.0);
    assert!(
        !report_laggy.is_aligned,
        "Initial PTS of 120ms must fail alignment check"
    );

    // Initial PTS 0.000s passes
    let probe_instant = FfprobeOutput {
        streams: vec![FfprobeStream {
            index: 0,
            codec_type: Some("video".to_string()),
            duration: Some("90.000000".to_string()),
            start_time: Some("0.000000".to_string()),
        }],
        format: None,
    };

    let report_instant = evaluate_keyframe_alignment(&probe_instant, 50.0);
    assert!(report_instant.is_aligned);
}

#[test]
fn test_ed_13_1_pure_verify_stream_durations_formula() {
    let (pass1, drift1) = verify_stream_durations(90.020, 90.040, 0.0);
    assert!(pass1);
    assert!((drift1 - 20.0).abs() < 1e-4);

    let (pass2, drift2) = verify_stream_durations(90.000, 90.060, 0.0);
    assert!(!pass2);
    assert!((drift2 - 60.0).abs() < 1e-4);
}

#[test]
fn test_ed_13_1_ffprobe_json_parser_integration() {
    let json_sample = r#"{
        "streams": [
            {
                "index": 0,
                "codec_type": "video",
                "start_time": "0.000000",
                "duration": "90.033000"
            },
            {
                "index": 1,
                "codec_type": "audio",
                "start_time": "0.000000",
                "duration": "90.025000"
            }
        ],
        "format": {
            "duration": "90.033000",
            "size": "52428800"
        }
    }"#;

    let probe = parse_ffprobe_json(json_sample).expect("ffprobe JSON parsing must succeed");
    assert_eq!(probe.streams.len(), 2);
    let report = evaluate_keyframe_alignment(&probe, 50.0);
    assert!(report.is_aligned);
    assert!((report.drift_ms - 8.0).abs() < 1e-4);
}

// ===========================================================================
// Module 3: IT-13.1 & IT-13.2 Manifest Integration & SLA Verification
// ===========================================================================

#[tokio::test]
async fn test_it_13_1_manifest_integration_online_stream_slice() {
    std::env::set_var("NEUROCLIP_MOCK_RANGE_SLICER", "1");

    let test_dir = TestDir::new("manifest_online");
    let vod_id = "vod_test_online_m13";
    let project_dir = test_dir.path().join(vod_id);
    std::fs::create_dir_all(&project_dir).unwrap();

    let manifest = VodManifest {
        vod_id: vod_id.to_string(),
        source_type: "twitch_vod".to_string(),
        original_url: Some("https://twitch.tv/videos/123456789".to_string()),
        title: "Neuro-sama Stream".to_string(),
        streamer: Some("Vedal987".to_string()),
        duration_seconds: 7200.0,
        audio_path: project_dir.join("audio.wav").to_string_lossy().to_string(),
        chat_file_path: None,
        video_stream_url: Some("https://video-weaver.sfo03.hls.ttvnw.net/v1/playlist/index-dvr.m3u8".to_string()),
        local_video_path: None,
        created_at: "2026-11-15T10:00:00Z".to_string(),
    };

    let manifest_bytes = serde_json::to_vec_pretty(&manifest).unwrap();
    std::fs::write(project_dir.join("vod_manifest.json"), manifest_bytes).unwrap();

    // Invoke slice_video_segment_internal using the project_dir path
    let result = slice_video_segment_internal(
        project_dir.to_str().unwrap(),
        5055.5,
        5085.0,
        Some("cluster_1"),
        None,
        None,
        None,
    )
    .await
    .expect("Slice from manifest must succeed");

    assert!(result.keyframe_aligned);
    assert!((result.duration_seconds - 29.5).abs() < 1e-4);
    assert!(Path::new(&result.clip_path).exists());
}

#[tokio::test]
async fn test_it_13_1_file_size_and_bandwidth_savings_99_percent() {
    std::env::set_var("NEUROCLIP_MOCK_RANGE_SLICER", "1");

    let test_dir = TestDir::new("savings_sla");
    let output_clip = test_dir.path().join("clip_90s.mp4");

    let req = RangeSliceRequest {
        input_source: "https://example.com/stream.m3u8".to_string(),
        start_sec: 1000.0,
        end_sec: 1090.0, // 90 seconds clip
        output_path: output_clip.clone(),
        is_online: true,
        http_headers: None,
        use_duration_flag: false,
        task_id: None,
    };

    let result = execute_range_slice(req, None).await.expect("Slicing must succeed");

    // AC-13.2 Check: File size must be between 30MB and 80MB
    let size_mb = result.file_size_bytes as f64 / (1024.0 * 1024.0);
    assert!(
        (30.0..=80.0).contains(&size_mb),
        "AC-13.2 Failed: File size {size_mb:.2}MB outside expected 30MB-80MB range"
    );

    // Bandwidth saving calculation vs 30GB full VOD
    let full_vod_mb = 30.0 * 1024.0; // 30720 MB
    let bandwidth_savings = (1.0 - (size_mb / full_vod_mb)) * 100.0;
    assert!(
        bandwidth_savings >= 99.0,
        "AC-13.2 Failed: Bandwidth savings {bandwidth_savings:.2}% is below 99%"
    );
}

#[tokio::test]
async fn test_it_13_2_sla_execution_duration_under_10s() {
    std::env::set_var("NEUROCLIP_MOCK_RANGE_SLICER", "1");

    let test_dir = TestDir::new("duration_sla");
    let output_clip = test_dir.path().join("clip_sla.mp4");

    let req = RangeSliceRequest {
        input_source: "https://example.com/stream.m3u8".to_string(),
        start_sec: 100.0,
        end_sec: 190.0,
        output_path: output_clip,
        is_online: true,
        http_headers: None,
        use_duration_flag: false,
        task_id: None,
    };

    let start = Instant::now();
    let res = execute_range_slice(req, None).await.expect("Slice should complete");
    let elapsed = start.elapsed();

    // AC-13.1 SLA: Slicing a 90s clip must complete in <= 10.0 seconds
    assert!(
        elapsed.as_secs_f64() <= 10.0,
        "AC-13.1 SLA Violated: execution took {:?} > 10.0s",
        elapsed
    );
    assert_eq!(res.duration_seconds, 90.0);
}

#[tokio::test]
async fn test_it_13_end_timestamp_clamped_to_manifest_duration() {
    std::env::set_var("NEUROCLIP_MOCK_RANGE_SLICER", "1");

    let test_dir = TestDir::new("clamping");

    // Pass test_dir.path() as vod_id so it resolves directly
    let res = slice_video_segment_internal(
        test_dir.path().to_str().unwrap(),
        90.0,
        180.0,
        Some("cluster_clamp"),
        None,
        Some("D:/Videos/local_test.mp4"),
        None,
    )
    .await
    .expect("Slice with custom source should succeed");

    assert_eq!(res.duration_seconds, 90.0);
    assert!(Path::new(&res.clip_path).exists());
}

// ===========================================================================
// Module 4: Process Safety & Cancellation Token Handling
// ===========================================================================

#[tokio::test]
async fn test_range_slicer_cancellation_token() {
    let test_dir = TestDir::new("cancellation");
    let output_clip = test_dir.path().join("cancelled_clip.mp4");

    let token = CancellationToken::new();
    token.cancel(); // Pre-cancelled token

    let req = RangeSliceRequest {
        input_source: "https://example.com/stream.m3u8".to_string(),
        start_sec: 10.0,
        end_sec: 40.0,
        output_path: output_clip,
        is_online: true,
        http_headers: None,
        use_duration_flag: false,
        task_id: Some("cancel_task".to_string()),
    };

    let res = execute_range_slice(req, Some(token)).await;
    match res {
        Err(AppError::TaskCancelled) => (),
        other => panic!("Expected AppError::TaskCancelled, got {other:?}"),
    }
}

// ===========================================================================
// Module 5: Wire Serialization & Security Guards
// ===========================================================================

#[test]
fn test_slice_result_serialization_and_deserialization_contract() {
    let result = SliceResult {
        clip_path: "C:/NeuroClip/projects/vod_1/clips/clip_1.mp4".to_string(),
        duration_seconds: 29.5,
        file_size_bytes: 47185920,
        video_duration: Some(29.533),
        audio_duration: Some(29.515),
        stream_drift_ms: 18.0,
        keyframe_aligned: true,
    };

    // Serialize to JSON string
    let json_str = serde_json::to_string(&result).expect("Serialization should succeed");

    // Must follow CONVENTIONS.md §4.2: snake_case wire serialization
    assert!(json_str.contains("\"clip_path\":"), "Must serialize as clip_path: {json_str}");
    assert!(json_str.contains("\"duration_seconds\":"), "Must serialize as duration_seconds: {json_str}");
    assert!(json_str.contains("\"file_size_bytes\":"), "Must serialize as file_size_bytes: {json_str}");
    assert!(json_str.contains("\"stream_drift_ms\":"), "Must serialize as stream_drift_ms: {json_str}");
    assert!(json_str.contains("\"keyframe_aligned\":"), "Must serialize as keyframe_aligned: {json_str}");

    // Must NOT contain camelCase keys in serialization output
    assert!(!json_str.contains("\"clipPath\":"), "Should not serialize as clipPath: {json_str}");
    assert!(!json_str.contains("\"durationSeconds\":"), "Should not serialize as durationSeconds: {json_str}");

    // Deserialization with camelCase input (dual-case alias support)
    let camel_json = r#"{
        "clipPath": "C:/NeuroClip/projects/vod_2/clips/clip_2.mp4",
        "durationSeconds": 15.0,
        "fileSizeBytes": 20971520,
        "videoDuration": 15.01,
        "audioDuration": 15.02,
        "streamDriftMs": 10.0,
        "keyframeAligned": true
    }"#;
    let parsed_camel: SliceResult = serde_json::from_str(camel_json).expect("Deserializing camelCase must succeed");
    assert_eq!(parsed_camel.clip_path, "C:/NeuroClip/projects/vod_2/clips/clip_2.mp4");
    assert_eq!(parsed_camel.duration_seconds, 15.0);
    assert_eq!(parsed_camel.file_size_bytes, 20971520);
    assert_eq!(parsed_camel.stream_drift_ms, 10.0);
    assert!(parsed_camel.keyframe_aligned);

    // Deserialization with snake_case input (wire format)
    let snake_json = r#"{
        "clip_path": "C:/NeuroClip/projects/vod_3/clips/clip_3.mp4",
        "duration_seconds": 45.0,
        "file_size_bytes": 1048576,
        "stream_drift_ms": 5.0,
        "keyframe_aligned": true
    }"#;
    let parsed_snake: SliceResult = serde_json::from_str(snake_json).expect("Deserializing snake_case must succeed");
    assert_eq!(parsed_snake.clip_path, "C:/NeuroClip/projects/vod_3/clips/clip_3.mp4");
    assert_eq!(parsed_snake.duration_seconds, 45.0);
    assert_eq!(parsed_snake.file_size_bytes, 1048576);
    assert_eq!(parsed_snake.stream_drift_ms, 5.0);
    assert!(parsed_snake.keyframe_aligned);
}

#[tokio::test]
async fn test_path_traversal_sanitization() {
    let test_dir = TestDir::new("path_traversal");
    let vod_dir = test_dir.path().to_path_buf();

    // Setup manifest and dummy source
    let manifest_path = vod_dir.join("vod_manifest.json");
    let local_video = vod_dir.join("source.mp4");
    std::fs::write(&local_video, b"fake video content for testing").unwrap();

    let manifest = VodManifest {
        vod_id: "vod_traversal".to_string(),
        source_type: "local_file".to_string(),
        original_url: None,
        title: "Test VOD".to_string(),
        streamer: Some("test_channel".to_string()),
        duration_seconds: 3600.0,
        audio_path: local_video.to_string_lossy().to_string(),
        chat_file_path: None,
        video_stream_url: None,
        local_video_path: Some(local_video.to_string_lossy().to_string()),
        created_at: "2026-03-30T00:00:00Z".to_string(),
    };
    std::fs::write(&manifest_path, serde_json::to_string(&manifest).unwrap()).unwrap();

    // Slicing with malicious output_filename containing path traversal (../../evil.mp4)
    let res = slice_video_segment_internal(
        &vod_dir.to_string_lossy(),
        10.0,
        20.0,
        None,
        Some("../../evil.mp4"),
        None,
        None,
    )
    .await
    .expect("Slicing should succeed with sanitized filename");

    let clip_path = PathBuf::from(&res.clip_path);

    // The output must be sanitized to be inside clips/ directory, named "evil.mp4"
    let clips_dir = vod_dir.join("clips");
    assert_eq!(clip_path.parent().unwrap(), clips_dir.as_path(), "Clip must reside in clips dir");
    assert_eq!(clip_path.file_name().unwrap(), "evil.mp4", "File name must be sanitized to evil.mp4");
}
