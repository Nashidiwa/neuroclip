#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Integration Test Suite for TASK-M4-06 (Audio-First Ingestion) & M4-06 -> M4-13
//! (`video_stream_url` propagation + lazy `yt-dlp -g` fallback).
//!
//! Covers:
//! - UT-06.1 & UT-06.2: Metadata probe, invalid URL rejection, `video_stream_url` extraction
//! - ED-06.4 / AC-06.5: Fast live stream rejection (<= 500ms)
//! - IT-06.1: Schema 5.2 `vod_manifest.json` creation & `video_stream_url` preservation
//! - IT-06.2 & ED-06.1: Pre-flight disk check (5GB) + mock download + progress payload
//! - ED-06.3: Anti-bot CLI args (`--cookies-from-browser`, `--cookies`, `--user-agent`, `Accept-Language`)
//! - Audio PCM stream + `AudioRingBuffer` + `AudioStreamValidator` (44B WAV strip + odd-byte carry-over)
//! - M4-06 -> M4-13: `slice_video_segment_internal` uses `video_stream_url` or lazy `yt-dlp -g` fallback

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio_util::sync::CancellationToken;

use neuroclip_lib::ingestion::{
    build_bot_defense_args, compute_audio_energy_stream, download_audio_stream,
    generate_vod_manifest_with_streams, probe_vod_metadata, resolve_video_stream_url,
    AudioRingBuffer, AudioStreamValidator, BrowserType, CookieSource, IngestProgressPayload,
    VodManifest,
};
use neuroclip_lib::{slice_video_segment_internal, AppError, IngestError};

/// Process-wide async lock synchronizing `NEUROCLIP_MOCK_*` env var mutations across tests.
static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "neuroclip_audio_ingest_test_{name}_{}_{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        std::fs::create_dir_all(&path).unwrap();
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
// UT-06.1 & UT-06.2: Metadata Probe, Invalid URL, & video_stream_url Parsing
// ===========================================================================

#[tokio::test]
async fn test_ut_06_1_and_06_2_probe_invalid_url_and_extract_video_stream_url() {
    let _guard = ENV_LOCK.lock().await;

    // Empty and non-HTTP URLs rejected immediately
    let empty_err = probe_vod_metadata("   ", &CookieSource::None, None)
        .await
        .unwrap_err();
    assert!(matches!(
        empty_err,
        AppError::Ingestion(IngestError::InvalidUrl(_))
    ));

    let bad_scheme_err =
        probe_vod_metadata("ftp://example.com/video.mp4", &CookieSource::None, None)
            .await
            .unwrap_err();
    assert!(matches!(
        bad_scheme_err,
        AppError::Ingestion(IngestError::InvalidUrl(_))
    ));

    // Mock yt-dlp --dump-single-json with requested_formats stream URL
    let mock_json = serde_json::json!({
        "id": "vod_neuro_999",
        "title": "Neuro-sama Karaoke VOD",
        "uploader": "Vedal987",
        "duration": 7200.0,
        "is_live": false,
        "requested_formats": [
            {
                "vcodec": "avc1.64002a",
                "acodec": "none",
                "height": 1080,
                "manifest_url": "https://cdn.example.com/hls/neuro_1080p.m3u8"
            },
            {
                "vcodec": "none",
                "acodec": "mp4a.40.2",
                "url": "https://cdn.example.com/audio/neuro_aac.m4a"
            }
        ]
    });

    std::env::set_var("NEUROCLIP_MOCK_YTDLP_METADATA", mock_json.to_string());
    let res = probe_vod_metadata(
        "https://www.youtube.com/watch?v=vod_neuro_999",
        &CookieSource::None,
        None,
    )
    .await;
    std::env::remove_var("NEUROCLIP_MOCK_YTDLP_METADATA");

    let meta = res.expect("Mock metadata probe must succeed");
    assert_eq!(meta.id, "vod_neuro_999");
    assert_eq!(
        meta.video_stream_url.as_deref(),
        Some("https://cdn.example.com/hls/neuro_1080p.m3u8")
    );
}

// ===========================================================================
// ED-06.4 / AC-06.5: Fast Live Stream Rejection (< 500ms)
// ===========================================================================

#[tokio::test]
async fn test_ed_06_4_fast_live_stream_rejection_under_500ms() {
    let _guard = ENV_LOCK.lock().await;

    let live_json = serde_json::json!({
        "id": "live_stream_1",
        "title": "Neuro Live Now!",
        "is_live": true,
        "live_status": "is_live"
    });
    std::env::set_var("NEUROCLIP_MOCK_YTDLP_METADATA", live_json.to_string());

    let started = Instant::now();
    let res = probe_vod_metadata("https://www.twitch.tv/vedal987", &CookieSource::None, None).await;
    let elapsed = started.elapsed();
    std::env::remove_var("NEUROCLIP_MOCK_YTDLP_METADATA");

    assert!(matches!(
        res,
        Err(AppError::Ingestion(IngestError::LiveStreamUnsupported))
    ));
    assert!(
        elapsed < Duration::from_millis(500),
        "Live rejection exceeded 500ms SLA: {elapsed:?}"
    );
}

// ===========================================================================
// IT-06.1: Schema 5.2 vod_manifest.json & video_stream_url Preservation
// ===========================================================================

#[tokio::test]
async fn test_it_06_1_manifest_preserves_video_stream_url_and_schema_5_2() {
    let test_dir = TestDir::new("it_06_1");
    let audio_path = test_dir.path().join("vod_1_audio.m4a");
    std::fs::write(&audio_path, b"AUDIO").unwrap();

    let manifest = generate_vod_manifest_with_streams(
        "vod_1",
        "twitch_vod",
        Some("https://www.twitch.tv/videos/123456"),
        "Neuro Stream",
        Some("vedal987"),
        3600.0,
        &audio_path,
        test_dir.path(),
        Some("https://video-weaver.example.ttvnw.net/v1/playlist/index.m3u8"),
        None,
    )
    .expect("generate_vod_manifest_with_streams must succeed");

    assert_eq!(manifest.vod_id, "vod_1");
    assert_eq!(manifest.source_type, "twitch_vod");
    assert_eq!(
        manifest.video_stream_url.as_deref(),
        Some("https://video-weaver.example.ttvnw.net/v1/playlist/index.m3u8")
    );
    assert_eq!(manifest.local_video_path, None);

    let saved_path = test_dir.path().join("vod_manifest.json");
    let saved: VodManifest = serde_json::from_slice(&std::fs::read(&saved_path).unwrap()).unwrap();
    assert_eq!(saved.video_stream_url, manifest.video_stream_url);
}

// ===========================================================================
// IT-06.2 & ED-06.1: Disk Space Check + Mock Audio Download + Progress
// ===========================================================================

#[tokio::test]
async fn test_it_06_2_and_ed_06_1_disk_check_and_mock_download_progress() {
    let _guard = ENV_LOCK.lock().await;
    let test_dir = TestDir::new("it_06_2");

    // 1. Insufficient disk space (< 5120MB) blocks download immediately
    std::env::set_var("NEUROCLIP_MOCK_DISK_FREE_MB", "3000");
    std::env::set_var("NEUROCLIP_MOCK_INGEST_RUN", "1");

    let err = download_audio_stream(
        "https://www.twitch.tv/videos/111",
        test_dir.path(),
        "vod_low_disk",
        &CookieSource::None,
        None,
        |_| {},
        CancellationToken::new(),
    )
    .await
    .unwrap_err();

    assert!(matches!(
        err,
        AppError::Ingestion(IngestError::DiskSpaceInsufficient {
            required_mb: 5120,
            available_mb: 3000
        })
    ));

    // 2. Sufficient disk space (20480MB) succeeds & emits progress payload
    std::env::set_var("NEUROCLIP_MOCK_DISK_FREE_MB", "20480");
    let events: Arc<Mutex<Vec<IngestProgressPayload>>> = Arc::new(Mutex::new(Vec::new()));
    let events_clone = Arc::clone(&events);

    let audio_res = download_audio_stream(
        "https://www.twitch.tv/videos/111",
        test_dir.path(),
        "vod_ok_disk",
        &CookieSource::None,
        None,
        move |p| {
            events_clone.lock().unwrap().push(p);
        },
        CancellationToken::new(),
    )
    .await;

    std::env::remove_var("NEUROCLIP_MOCK_DISK_FREE_MB");
    std::env::remove_var("NEUROCLIP_MOCK_INGEST_RUN");

    let audio_path = audio_res.expect("Mock download must succeed when disk >= 5120MB");
    assert!(audio_path.exists());
    let captured = events.lock().unwrap();
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].task_id, "vod_ok_disk");
    assert!((captured[0].percent - 100.0).abs() < 1e-4);
}

// ===========================================================================
// ED-06.3: Anti-Bot Defense CLI Arguments
// ===========================================================================

#[test]
fn test_ed_06_3_bot_defense_cli_args() {
    let args_browser = build_bot_defense_args(&CookieSource::Browser(BrowserType::Chrome), None);
    assert!(args_browser.contains(&"--cookies-from-browser".to_string()));
    assert!(args_browser.contains(&"chrome".to_string()));
    assert!(args_browser.contains(&"--user-agent".to_string()));
    assert!(args_browser.iter().any(|a| a.contains("Accept-Language")));

    let args_file = build_bot_defense_args(
        &CookieSource::File("C:\\cookies.txt".to_string()),
        Some("CustomAgent/1.0"),
    );
    assert!(args_file.contains(&"--cookies".to_string()));
    assert!(args_file.contains(&"C:\\cookies.txt".to_string()));
    assert!(args_file.contains(&"CustomAgent/1.0".to_string()));
}

// ===========================================================================
// AudioStreamValidator, AudioRingBuffer, & compute_audio_energy_stream
// ===========================================================================

#[tokio::test]
async fn test_audio_stream_validator_ring_buffer_and_energy_stream() {
    let _guard = ENV_LOCK.lock().await;

    // 1. AudioStreamValidator strips 44-byte RIFF/WAVE header and carries over odd trailing byte
    let mut validator = AudioStreamValidator::new();
    let mut wav_header = Vec::with_capacity(47);
    wav_header.extend_from_slice(b"RIFF");
    wav_header.extend_from_slice(&40u32.to_le_bytes()); // RIFF chunk size
    wav_header.extend_from_slice(b"WAVE");
    wav_header.extend_from_slice(b"fmt ");
    wav_header.extend_from_slice(&16u32.to_le_bytes()); // PCM fmt chunk size
    wav_header.extend_from_slice(&1u16.to_le_bytes()); // AudioFormat = 1 (PCM)
    wav_header.extend_from_slice(&1u16.to_le_bytes()); // NumChannels = 1
    wav_header.extend_from_slice(&16_000u32.to_le_bytes()); // SampleRate = 16000
    wav_header.extend_from_slice(&32_000u32.to_le_bytes()); // ByteRate = 32000
    wav_header.extend_from_slice(&2u16.to_le_bytes()); // BlockAlign = 2
    wav_header.extend_from_slice(&16u16.to_le_bytes()); // BitsPerSample = 16
    wav_header.extend_from_slice(b"data");
    wav_header.extend_from_slice(&4u32.to_le_bytes());
    // Append 3 bytes (1 full i16 sample [0x01, 0x00] + 1 odd carry byte [0x02])
    wav_header.extend_from_slice(&[0x01, 0x00, 0x02]);

    let c1 = validator.process_chunk(&wav_header, false).unwrap();
    assert_eq!(c1.samples, vec![1i16]);

    // Next chunk supplies second byte [0x00] of carried sample -> [0x02, 0x00] = 2i16
    let c2 = validator.process_chunk(&[0x00], true).unwrap();
    assert_eq!(c2.samples, vec![2i16]);

    // 2. AudioRingBuffer circular push/pop without losing samples
    let mut ring = AudioRingBuffer::with_capacity(4);
    assert_eq!(ring.push_samples(&[10, 20, 30]), 3);
    let mut out = [0i16; 2];
    assert_eq!(ring.pop_samples(&mut out), 2);
    assert_eq!(out, [10, 20]);
    assert_eq!(ring.push_samples(&[40, 50, 60]), 3);
    let mut out3 = [0i16; 3];
    assert_eq!(ring.pop_samples(&mut out3), 3);
    assert_eq!(out3, [30, 40, 50]);

    // 3. compute_audio_energy_stream mock
    std::env::set_var("NEUROCLIP_MOCK_FFMPEG_STREAM", "1");
    let energy_res =
        compute_audio_energy_stream("mock_source", 2, None, CancellationToken::new()).await;
    std::env::remove_var("NEUROCLIP_MOCK_FFMPEG_STREAM");

    let windows = energy_res.expect("Mock audio energy stream must succeed");
    assert_eq!(windows.len(), 2);
    assert!(windows[0].rms_energy > 0.0);
}

// ===========================================================================
// M4-06 -> M4-13 Integration: video_stream_url + Lazy yt-dlp -g Fallback
// ===========================================================================

#[tokio::test]
async fn test_m4_06_to_m4_13_slice_uses_stream_url_and_lazy_ytdlp_g_fallback() {
    let _guard = ENV_LOCK.lock().await;
    let test_dir = TestDir::new("m4_06_m4_13");

    // 1. resolve_video_stream_url rejects webpage URL echo and accepts real CDN stream URL
    std::env::set_var(
        "NEUROCLIP_MOCK_YTDLP_STREAM_URL",
        "https://www.youtube.com/watch?v=abc123",
    );
    let echo_err = resolve_video_stream_url(
        "https://www.youtube.com/watch?v=abc123",
        &CookieSource::None,
        None,
    )
    .await
    .unwrap_err();
    assert!(matches!(
        echo_err,
        AppError::Ingestion(IngestError::ExtractionFailed(_))
    ));

    // 2. Manifest missing video_stream_url triggers lazy yt-dlp -g fallback and persists it
    let project_dir = test_dir.path().join("vod_lazy_resolve");
    std::fs::create_dir_all(&project_dir).unwrap();
    let audio_path = project_dir.join("audio.m4a");
    std::fs::write(&audio_path, b"AUDIO").unwrap();

    generate_vod_manifest_with_streams(
        "vod_lazy_resolve",
        "youtube_vod",
        Some("https://www.youtube.com/watch?v=abc123"),
        "Lazy Resolve Test",
        Some("Vedal987"),
        600.0,
        &audio_path,
        &project_dir,
        None, // video_stream_url absent initially
        None,
    )
    .unwrap();

    std::env::set_var(
        "NEUROCLIP_MOCK_YTDLP_STREAM_URL",
        "https://rr1.googlevideo.com/videoplayback?id=resolved_stream",
    );
    std::env::set_var("NEUROCLIP_MOCK_RANGE_SLICER", "1");
    std::env::set_var("NEUROCLIP_MOCK_DISK_FREE_MB", "20480");

    let slice_res = slice_video_segment_internal(
        project_dir.to_str().unwrap(),
        10.0,
        100.0,
        Some("c_lazy"),
        None,
        None,
        None,
    )
    .await;

    std::env::remove_var("NEUROCLIP_MOCK_YTDLP_STREAM_URL");
    std::env::remove_var("NEUROCLIP_MOCK_RANGE_SLICER");
    std::env::remove_var("NEUROCLIP_MOCK_DISK_FREE_MB");

    let result = slice_res.expect("Slice with lazy yt-dlp -g fallback must succeed");
    assert!(result.keyframe_aligned);
    assert!(Path::new(&result.clip_path).exists());

    // Confirm resolved stream URL was persisted into vod_manifest.json (not webpage original_url)
    let persisted: VodManifest =
        serde_json::from_slice(&std::fs::read(project_dir.join("vod_manifest.json")).unwrap())
            .unwrap();
    assert_eq!(
        persisted.video_stream_url.as_deref(),
        Some("https://rr1.googlevideo.com/videoplayback?id=resolved_stream")
    );
}

#[tokio::test]
async fn test_probe_vod_metadata_null_formats_fallback_and_audio_only_omitted_vcodec() {
    let _guard = ENV_LOCK.lock().await;
    let webpage_url = "https://www.youtube.com/watch?v=edge_cases_1";

    // 1. "formats": null and "requested_formats": null must deserialize safely without error
    let null_formats_json = serde_json::json!({
        "id": "v_null_formats",
        "title": "Null Formats Test",
        "formats": null,
        "requested_formats": null
    });
    std::env::set_var(
        "NEUROCLIP_MOCK_YTDLP_METADATA",
        null_formats_json.to_string(),
    );
    let meta_null = probe_vod_metadata(webpage_url, &CookieSource::None, None)
        .await
        .expect("Null formats/requested_formats must succeed");
    std::env::remove_var("NEUROCLIP_MOCK_YTDLP_METADATA");
    assert_eq!(meta_null.video_stream_url, None);

    // 2. 1080p format with invalid/null/webpage URL must fall back to 720p format with valid stream URL
    let fallback_720p_json = serde_json::json!({
        "id": "v_fallback_720p",
        "title": "720p Fallback Test",
        "formats": [
            {
                "vcodec": "avc1.64002a",
                "acodec": "mp4a.40.2",
                "height": 1080,
                "url": webpage_url
            },
            {
                "vcodec": "avc1.64002a",
                "acodec": "mp4a.40.2",
                "height": 1080,
                "url": null
            },
            {
                "vcodec": "avc1.4d401f",
                "acodec": "mp4a.40.2",
                "height": 720,
                "url": "https://cdn.example.com/stream_720p.mp4"
            }
        ]
    });
    std::env::set_var(
        "NEUROCLIP_MOCK_YTDLP_METADATA",
        fallback_720p_json.to_string(),
    );
    let meta_720p = probe_vod_metadata(webpage_url, &CookieSource::None, None)
        .await
        .expect("720p fallback metadata probe must succeed");
    std::env::remove_var("NEUROCLIP_MOCK_YTDLP_METADATA");
    assert_eq!(
        meta_720p.video_stream_url.as_deref(),
        Some("https://cdn.example.com/stream_720p.mp4")
    );

    // 3. Audio-only format with omitted vcodec must NOT be selected as video_stream_url
    let audio_omitted_vcodec_json = serde_json::json!({
        "id": "v_audio_only_omitted_vcodec",
        "title": "Audio Only Omitted Vcodec",
        "formats": [
            {
                "acodec": "mp4a.40.2",
                "url": "https://cdn.example.com/audio_only.m4a"
            }
        ]
    });
    std::env::set_var(
        "NEUROCLIP_MOCK_YTDLP_METADATA",
        audio_omitted_vcodec_json.to_string(),
    );
    let meta_audio_only = probe_vod_metadata(webpage_url, &CookieSource::None, None)
        .await
        .expect("Audio-only omitted vcodec probe must succeed");
    std::env::remove_var("NEUROCLIP_MOCK_YTDLP_METADATA");
    assert_eq!(meta_audio_only.video_stream_url, None);
}

#[tokio::test]
async fn test_slice_rejects_corrupted_video_stream_url_equal_to_original_url() {
    let _guard = ENV_LOCK.lock().await;
    let test_dir = TestDir::new("corrupt_url_echo");
    let project_dir = test_dir.path().join("vod_corrupt_url");
    std::fs::create_dir_all(&project_dir).unwrap();
    let audio_path = project_dir.join("audio.m4a");
    std::fs::write(&audio_path, b"AUDIO").unwrap();

    let webpage_url = "https://www.youtube.com/watch?v=corrupt123";
    generate_vod_manifest_with_streams(
        "vod_corrupt_url",
        "youtube_vod",
        Some(webpage_url),
        "Corrupt Stream URL Manifest",
        Some("Vedal987"),
        600.0,
        &audio_path,
        &project_dir,
        Some(webpage_url), // corrupted manifest where video_stream_url == original_url
        None,
    )
    .unwrap();

    std::env::set_var(
        "NEUROCLIP_MOCK_YTDLP_STREAM_URL",
        "https://rr2.googlevideo.com/videoplayback?id=fresh_cdn_stream",
    );
    std::env::set_var("NEUROCLIP_MOCK_RANGE_SLICER", "1");
    std::env::set_var("NEUROCLIP_MOCK_DISK_FREE_MB", "20480");

    let slice_res = slice_video_segment_internal(
        project_dir.to_str().unwrap(),
        5.0,
        35.0,
        Some("c_corrupt_guard"),
        None,
        None,
        None,
    )
    .await;

    std::env::remove_var("NEUROCLIP_MOCK_YTDLP_STREAM_URL");
    std::env::remove_var("NEUROCLIP_MOCK_RANGE_SLICER");
    std::env::remove_var("NEUROCLIP_MOCK_DISK_FREE_MB");

    let result = slice_res.expect("Slice must reject webpage URL and resolve via yt-dlp -g");
    assert!(Path::new(&result.clip_path).exists());

    let persisted: VodManifest =
        serde_json::from_slice(&std::fs::read(project_dir.join("vod_manifest.json")).unwrap())
            .unwrap();
    assert_eq!(
        persisted.video_stream_url.as_deref(),
        Some("https://rr2.googlevideo.com/videoplayback?id=fresh_cdn_stream")
    );
}
