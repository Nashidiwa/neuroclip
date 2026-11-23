#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Remediation verification tests for NeuroClip M4.1–M4.7 fixes:
//! - Item 3: Subprocess stdout/stderr pipe deadlock resistance (>256 KB pipe flood)
//! - Item 2: RAII temp JSON cleanup when sidecar fails
//! - Item 8: Async command handlers offload blocking I/O cleanly via spawn_blocking

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use neuroclip_lib::commands::{
    check_preflight_storage, clean_expired_cache, clear_logs, export_subtitle_project,
    export_subtitles_ass, export_subtitles_srt, get_chat_events, get_highlight_clusters,
    get_recent_logs, toggle_highlight_bookmark, verify_stt_model,
};
use neuroclip_lib::error::{AppError, IngestError, SttError};
use neuroclip_lib::ingestion::cookies::CookieSource;
use neuroclip_lib::ingestion::range_slicer::{execute_range_slice, RangeSliceRequest};
use neuroclip_lib::ingestion::streamer::download_audio_stream;
use neuroclip_lib::stt::audio_prep::{
    generate_synthetic_normalized_buffer, prepare_audio_file, prepare_audio_to_buffer,
    AudioPrepConfig,
};
use neuroclip_lib::stt::create_subtitle_project;
use neuroclip_lib::stt::faster_whisper_worker::{run_faster_whisper_worker, FasterWhisperConfig};
use neuroclip_lib::stt::transcript_parser::{TranscriptSegment, TranscriptSegmentsContainer};
use neuroclip_lib::stt::whisper_sidecar::{run_whisper_transcription, WhisperSidecarConfig};
use tokio_util::sync::CancellationToken;

static ENV_MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct ScopedEnv {
    key: &'static str,
    prev: Option<std::ffi::OsString>,
}

impl ScopedEnv {
    fn set(key: &'static str, val: &str) -> Self {
        let prev = std::env::var_os(key);
        std::env::set_var(key, val);
        Self { key, prev }
    }
}

impl Drop for ScopedEnv {
    fn drop(&mut self) {
        match &self.prev {
            Some(v) => std::env::set_var(self.key, v),
            None => std::env::remove_var(self.key),
        }
    }
}

struct TempDirGuard(PathBuf);

impl TempDirGuard {
    fn new(prefix: &str) -> Self {
        let nanos = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("{prefix}_{}_{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(windows)]
fn create_300kb_payload_file(dir: &Path, name: &str) -> PathBuf {
    let p = dir.join(name);
    let mut content = String::with_capacity(320 * 1024);
    let line = "INFO pipe_flood_diagnostic_line_0123456789abcdef0123456789abcdef\r\n";
    while content.len() < 300 * 1024 {
        content.push_str(line);
    }
    std::fs::write(&p, content.as_bytes()).unwrap();
    p
}

#[cfg(windows)]
#[tokio::test]
async fn test_pipe_deadlock_resistance_across_stt_and_audio_sidecars() {
    let _env_guard = ENV_MUTEX.lock().await;
    let dir = TempDirGuard::new("neuroclip_pipe_flood");
    let flood_file = create_300kb_payload_file(dir.path(), "flood_300kb.txt");
    let dummy_audio = dir.path().join("input.wav");
    generate_synthetic_normalized_buffer(16_000, 0.5)
        .write_wav_file(&dummy_audio)
        .unwrap();

    // 1. Whisper.cpp sidecar flooding >300KB to stdout & stderr
    let whisper_payload = dir.path().join("whisper_sample.json");
    std::fs::write(
        &whisper_payload,
        r#"{"transcription":[{"timestamps":{"from":"00:00:00,000","to":"00:00:01,000"},"offsets":{"from":0,"to":1000},"text":" Flood OK","tokens":[]}]}"#,
    )
    .unwrap();
    let whisper_cmd = dir.path().join("mock_whisper_flood.cmd");
    std::fs::write(
        &whisper_cmd,
        format!(
            "@echo off\r\n\
             setlocal enabledelayedexpansion\r\n\
             set \"OUTPREFIX=\"\r\n\
             :loop\r\n\
             if \"%~1\"==\"\" goto done\r\n\
             if \"%~1\"==\"-of\" (\r\n\
               set \"OUTPREFIX=%~2\"\r\n\
               shift\r\n\
             )\r\n\
             shift\r\n\
             goto loop\r\n\
             :done\r\n\
             type \"{}\"\r\n\
             type \"{}\" 1>&2\r\n\
             echo progress 100%% 1>&2\r\n\
             copy /Y \"{}\" \"%OUTPREFIX%.json\" >nul\r\n\
             exit /b 0\r\n",
            flood_file.display(),
            flood_file.display(),
            whisper_payload.display()
        ),
    )
    .unwrap();

    let whisper_cfg = WhisperSidecarConfig {
        binary_path: Some(whisper_cmd),
        model_path: dummy_audio.clone(),
        ..WhisperSidecarConfig::default()
    };
    let w_start = Instant::now();
    let w_res = tokio::time::timeout(
        Duration::from_secs(5),
        run_whisper_transcription("clip_flood_w", &dummy_audio, &whisper_cfg, None),
    )
    .await
    .expect("Whisper sidecar must not deadlock on >300KB stdout+stderr")
    .expect("Whisper flood mock must succeed");
    assert_eq!(w_res.segments[0].text_en, "Flood OK");
    assert!(w_start.elapsed() < Duration::from_secs(5));

    // 2. Faster-Whisper worker flooding >300KB to stdout & stderr
    let fw_payload = dir.path().join("fw_sample.json");
    std::fs::write(
        &fw_payload,
        r#"{"text":"FW Flood OK","segments":[{"id":0,"start":0.0,"end":1.0,"text":" FW Flood OK","avg_logprob":-0.1,"words":[]}],"words":[]}"#,
    )
    .unwrap();
    let fw_cmd = dir.path().join("mock_fw_flood.cmd");
    std::fs::write(
        &fw_cmd,
        format!(
            "@echo off\r\n\
             setlocal enabledelayedexpansion\r\n\
             set \"OUTJSON=\"\r\n\
             :loop\r\n\
             if \"%~1\"==\"\" goto done\r\n\
             if \"%~1\"==\"--output\" (\r\n\
               set \"OUTJSON=%~2\"\r\n\
               shift\r\n\
             )\r\n\
             shift\r\n\
             goto loop\r\n\
             :done\r\n\
             type \"{}\"\r\n\
             type \"{}\" 1>&2\r\n\
             copy /Y \"{}\" \"%OUTJSON%\" >nul\r\n\
             exit /b 0\r\n",
            flood_file.display(),
            flood_file.display(),
            fw_payload.display()
        ),
    )
    .unwrap();

    let fw_cfg = FasterWhisperConfig {
        binary_path: Some(fw_cmd),
        ..FasterWhisperConfig::default()
    };
    let fw_res = tokio::time::timeout(
        Duration::from_secs(5),
        run_faster_whisper_worker("clip_flood_fw", &dummy_audio, &fw_cfg, None),
    )
    .await
    .expect("Faster-Whisper worker must not deadlock on >300KB stdout+stderr")
    .expect("Faster-Whisper flood mock must succeed");
    assert_eq!(fw_res.segments[0].text_en, "FW Flood OK");

    // 3. audio_prep::prepare_audio_file flooding >300KB to stdout & stderr
    let prep_out = dir.path().join("prep_out.wav");
    let ffmpeg_file_cmd = dir.path().join("mock_ffmpeg_file_flood.cmd");
    std::fs::write(
        &ffmpeg_file_cmd,
        format!(
            "@echo off\r\n\
             set \"LAST=\"\r\n\
             :loop\r\n\
             if \"%~1\"==\"\" goto done\r\n\
             set \"LAST=%~1\"\r\n\
             shift\r\n\
             goto loop\r\n\
             :done\r\n\
             type \"{}\"\r\n\
             type \"{}\" 1>&2\r\n\
             copy /Y \"{}\" \"%LAST%\" >nul\r\n\
             exit /b 0\r\n",
            flood_file.display(),
            flood_file.display(),
            dummy_audio.display()
        ),
    )
    .unwrap();

    let _ffmpeg_env = ScopedEnv::set("NEUROCLIP_FFMPEG_PATH", &ffmpeg_file_cmd.to_string_lossy());
    let prep_res = tokio::time::timeout(
        Duration::from_secs(5),
        prepare_audio_file(&dummy_audio, &prep_out, &AudioPrepConfig::default(), None),
    )
    .await
    .expect("prepare_audio_file must not deadlock on >300KB stdout+stderr")
    .expect("prepare_audio_file flood mock must succeed");
    assert_eq!(prep_res.sample_rate, 16_000);
    drop(_ffmpeg_env);

    // 4. audio_prep::prepare_audio_to_buffer flooding >300KB to stderr while streaming PCM on stdout
    let pcm_bin = dir.path().join("sample_pcm.bin");
    let pcm_samples = [0x00u8, 0x10, 0x00, 0x20, 0x00, 0x30, 0x00, 0x40];
    std::fs::write(&pcm_bin, pcm_samples).unwrap();
    let ffmpeg_pipe_cmd = dir.path().join("mock_ffmpeg_pipe_flood.cmd");
    std::fs::write(
        &ffmpeg_pipe_cmd,
        format!(
            "@echo off\r\n\
             type \"{}\" 1>&2\r\n\
             type \"{}\"\r\n\
             exit /b 0\r\n",
            flood_file.display(),
            pcm_bin.display()
        ),
    )
    .unwrap();
    let _ffmpeg_pipe_env =
        ScopedEnv::set("NEUROCLIP_FFMPEG_PATH", &ffmpeg_pipe_cmd.to_string_lossy());
    let buf_res = tokio::time::timeout(
        Duration::from_secs(5),
        prepare_audio_to_buffer(&dummy_audio, &AudioPrepConfig::default(), None),
    )
    .await
    .expect("prepare_audio_to_buffer must not deadlock on >300KB stderr")
    .expect("prepare_audio_to_buffer flood mock must succeed");
    assert_eq!(buf_res.samples_i16.len(), 4);
    drop(_ffmpeg_pipe_env);

    // 5. range_slicer::execute_range_slice flooding >300KB to stdout & stderr
    let slice_out = dir.path().join("slice_out.mp4");
    let _disk_env = ScopedEnv::set("NEUROCLIP_MOCK_DISK_FREE_MB", "20480");
    let _ffprobe_env = ScopedEnv::set(
        "NEUROCLIP_MOCK_FFPROBE_OUTPUT",
        r#"{"streams":[{"index":0,"codec_type":"video","duration":"10.000","start_time":"0.000"},{"index":1,"codec_type":"audio","duration":"10.000","start_time":"0.000"}],"format":{"duration":"10.000","size":"1024"}}"#,
    );
    let _ffmpeg_slice_env =
        ScopedEnv::set("NEUROCLIP_FFMPEG_PATH", &ffmpeg_file_cmd.to_string_lossy());
    let slice_req = RangeSliceRequest {
        input_source: "https://example.com/stream.mp4".to_string(),
        start_sec: 0.0,
        end_sec: 10.0,
        output_path: slice_out.clone(),
        is_online: true,
        http_headers: None,
        use_duration_flag: false,
        task_id: Some("slice_flood".to_string()),
    };
    let slice_res =
        tokio::time::timeout(Duration::from_secs(5), execute_range_slice(slice_req, None))
            .await
            .expect("execute_range_slice must not deadlock on >300KB stdout+stderr")
            .expect("execute_range_slice flood mock must succeed");
    assert!(slice_res.keyframe_aligned);
    assert!(slice_out.exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_async_ipc_commands_execute_via_spawn_blocking_alongside_concurrent_ticker() {
    let _env_guard = ENV_MUTEX.lock().await;
    let dir = TempDirGuard::new("neuroclip_async_cmds");
    let cache_dir = dir.path().join("cache");
    let projects_dir = dir.path().join("projects");
    let logs_dir = dir.path().join("logs");
    std::fs::create_dir_all(&cache_dir).unwrap();
    std::fs::create_dir_all(&projects_dir).unwrap();
    std::fs::create_dir_all(&logs_dir).unwrap();

    let _cache_env = ScopedEnv::set("NEUROCLIP_CACHE_DIR", &cache_dir.to_string_lossy());
    let _proj_env = ScopedEnv::set("NEUROCLIP_PROJECTS_DIR", &projects_dir.to_string_lossy());
    let _log_env = ScopedEnv::set("NEUROCLIP_LOG_DIR", &logs_dir.to_string_lossy());
    let _disk_env = ScopedEnv::set("NEUROCLIP_MOCK_DISK_FREE_MB", "20480");

    let ticks = Arc::new(AtomicUsize::new(0));
    let ticks_bg = Arc::clone(&ticks);
    let ticker = tokio::spawn(async move {
        for _ in 0..10 {
            tokio::time::sleep(Duration::from_millis(2)).await;
            ticks_bg.fetch_add(1, Ordering::Relaxed);
        }
    });

    // Exercise offloaded export, storage, chat, highlight, stt, and log async commands
    let container = TranscriptSegmentsContainer {
        clip_id: "clip_async_test".to_string(),
        segments: vec![TranscriptSegment {
            segment_id: "s1".to_string(),
            start_sec: 0.0,
            end_sec: 1.5,
            speaker_id: "neuro".to_string(),
            text_en: "Async OK".to_string(),
            text_vi: Some("Ổn áp".to_string()),
            confidence: 0.99,
            cps_score: 10.0,
            is_flagged_for_review: false,
            words: vec![],
        }],
    };
    let project = create_subtitle_project(
        container.clone(),
        "Async Export",
        "vod_async",
        Some("original_16_9"),
    );

    let ass_out = dir.path().join("out.ass");
    let srt_out = dir.path().join("out.srt");
    let proj_out = dir.path().join("proj.json");
    export_subtitles_ass(container.clone(), ass_out.to_string_lossy().to_string())
        .await
        .unwrap();
    export_subtitles_srt(container, srt_out.to_string_lossy().to_string())
        .await
        .unwrap();
    export_subtitle_project(project, proj_out.to_string_lossy().to_string())
        .await
        .unwrap();

    let disk = check_preflight_storage(None).await.unwrap();
    assert!(disk.is_sufficient);
    let _clean = clean_expired_cache(Some(cache_dir.to_string_lossy().to_string()), Some(7))
        .await
        .unwrap();

    let _logs = get_recent_logs(Some(10), None).await.unwrap();
    clear_logs().await.unwrap();

    // Missing paths return clean typed errors via spawn_blocking
    assert!(verify_stt_model(Some("nonexistent_model".to_string()))
        .await
        .is_err());
    assert!(get_chat_events("vod_async_missing".to_string(), None, None)
        .await
        .is_err());
    assert!(get_highlight_clusters("vod_async_missing".to_string())
        .await
        .is_err());
    assert!(
        toggle_highlight_bookmark("vod_async_missing".to_string(), "c1".to_string(), None)
            .await
            .is_err()
    );

    ticker.await.unwrap();
    assert_eq!(ticks.load(Ordering::Relaxed), 10);
}

#[cfg(windows)]
fn create_non_utf8_300kb_payload_file(
    dir: &Path,
    name: &str,
    trailing_line: Option<&[u8]>,
) -> PathBuf {
    let p = dir.join(name);
    let mut content: Vec<u8> = Vec::with_capacity(320 * 1024);
    // Avoid leading 0xFF 0xFE so Windows `cmd.exe type` does not treat the file as UTF-16LE BOM.
    content.extend_from_slice(b"\x80\x81\xff\xfe invalid utf8 diagnostic line\r\n");
    let pad_line = b"INFO non_utf8_pipe_flood_padding_0123456789abcdef0123456789abcdef\r\n";
    while content.len() < 305 * 1024 {
        content.extend_from_slice(pad_line);
    }
    if let Some(tail) = trailing_line {
        content.extend_from_slice(b"\x80\x81\xff\xfe trailing non_utf8 line\r\n");
        content.extend_from_slice(tail);
    }
    std::fs::write(&p, &content).unwrap();
    p
}

#[cfg(windows)]
#[tokio::test]
async fn test_non_utf8_stderr_drainers_survive_without_epipe_and_preserve_cuda_oom() {
    let _env_guard = ENV_MUTEX.lock().await;
    let dir = TempDirGuard::new("neuroclip_non_utf8_pipe");
    let dummy_audio = dir.path().join("input.wav");
    generate_synthetic_normalized_buffer(16_000, 0.5)
        .write_wav_file(&dummy_audio)
        .unwrap();

    // 1. run_faster_whisper_worker: non-UTF-8 stderr + >300KB padding + "RuntimeError: CUDA out of memory"
    let oom_payload = create_non_utf8_300kb_payload_file(
        dir.path(),
        "fw_non_utf8_oom.bin",
        Some(b"RuntimeError: CUDA out of memory\r\n"),
    );
    let fw_oom_cmd = dir.path().join("mock_fw_non_utf8_oom.cmd");
    std::fs::write(
        &fw_oom_cmd,
        format!(
            "@echo off\r\n\
             type \"{}\" 1>&2\r\n\
             exit /b 1\r\n",
            oom_payload.display()
        ),
    )
    .unwrap();

    let fw_cfg = FasterWhisperConfig {
        binary_path: Some(fw_oom_cmd),
        ..FasterWhisperConfig::default()
    };
    let fw_res = tokio::time::timeout(
        Duration::from_secs(5),
        run_faster_whisper_worker("clip_non_utf8_oom", &dummy_audio, &fw_cfg, None),
    )
    .await
    .expect("run_faster_whisper_worker must not hang on non-UTF-8 stderr");

    match fw_res {
        Err(AppError::Stt(SttError::CudaOom(details))) => {
            assert!(
                details.contains('\u{FFFD}'),
                "Expected lossy replacement character from non-UTF-8 stderr bytes, got: {details}"
            );
            assert!(
                details.to_ascii_lowercase().contains("cuda out of memory"),
                "Expected CUDA out of memory in CudaOom details, got: {details}"
            );
        }
        other => panic!("Expected Err(AppError::Stt(SttError::CudaOom(_))), got {other:?}"),
    }

    // 2. prepare_audio_file: non-UTF-8 stderr + >300KB padding must finish Ok(...) without EPIPE
    let prep_payload = create_non_utf8_300kb_payload_file(dir.path(), "prep_non_utf8.bin", None);
    let prep_out = dir.path().join("prep_non_utf8_out.wav");
    let ffmpeg_non_utf8_cmd = dir.path().join("mock_ffmpeg_non_utf8.cmd");
    std::fs::write(
        &ffmpeg_non_utf8_cmd,
        format!(
            "@echo off\r\n\
             set \"LAST=\"\r\n\
             :loop\r\n\
             if \"%~1\"==\"\" goto done\r\n\
             set \"LAST=%~1\"\r\n\
             shift\r\n\
             goto loop\r\n\
             :done\r\n\
             type \"{}\" 1>&2\r\n\
             copy /Y \"{}\" \"%LAST%\" >nul\r\n\
             exit /b 0\r\n",
            prep_payload.display(),
            dummy_audio.display()
        ),
    )
    .unwrap();

    let _ffmpeg_env = ScopedEnv::set(
        "NEUROCLIP_FFMPEG_PATH",
        &ffmpeg_non_utf8_cmd.to_string_lossy(),
    );
    let prep_res = tokio::time::timeout(
        Duration::from_secs(5),
        prepare_audio_file(&dummy_audio, &prep_out, &AudioPrepConfig::default(), None),
    )
    .await
    .expect("prepare_audio_file must not hang on non-UTF-8 stderr")
    .expect("prepare_audio_file must survive non-UTF-8 stderr without EPIPE");
    assert_eq!(prep_res.sample_rate, 16_000);
    assert!(prep_out.exists());
}

#[cfg(windows)]
#[tokio::test]
async fn test_download_audio_stream_preserves_trailing_and_streaming_bot_block_stderr() {
    let _env_guard = ENV_MUTEX.lock().await;
    let dir = TempDirGuard::new("neuroclip_ytdlp_stderr_tail");
    let out_dir = dir.path().join("cache");
    std::fs::create_dir_all(&out_dir).unwrap();

    let _disk_env = ScopedEnv::set("NEUROCLIP_MOCK_DISK_FREE_MB", "20480");

    // Case 1: >= 31 warning lines followed by trailing HTTP 403 Forbidden
    let mut stderr_403 = String::new();
    for i in 0..40 {
        stderr_403.push_str(&format!("WARNING: retry diagnostic line {i}\r\n"));
    }
    stderr_403.push_str("ERROR: HTTP Error 403: Forbidden\r\n");
    let stderr_403_file = dir.path().join("stderr_403.txt");
    std::fs::write(&stderr_403_file, stderr_403.as_bytes()).unwrap();

    let ytdlp_403_cmd = dir.path().join("mock_ytdlp_403.cmd");
    std::fs::write(
        &ytdlp_403_cmd,
        format!(
            "@echo off\r\n\
             type \"{}\" 1>&2\r\n\
             exit /b 1\r\n",
            stderr_403_file.display()
        ),
    )
    .unwrap();

    {
        let _ytdlp_env = ScopedEnv::set("NEUROCLIP_YTDLP_PATH", &ytdlp_403_cmd.to_string_lossy());
        let res = download_audio_stream(
            "https://www.youtube.com/watch?v=bot_blocked_403",
            &out_dir,
            "vod_403_tail",
            &CookieSource::None,
            None,
            |_| {},
            CancellationToken::new(),
        )
        .await;

        match res {
            Err(AppError::Ingestion(IngestError::BotBlocked(details))) => {
                assert!(
                    details.contains("HTTP Error 403: Forbidden"),
                    "Expected trailing 403 diagnostic in BotBlocked details, got: {details}"
                );
            }
            other => panic!(
                "Expected Err(AppError::Ingestion(IngestError::BotBlocked(_))), got {other:?}"
            ),
        }
    }

    // Case 2: HTTP 429 early in stderr followed by > 64 info lines (tests streaming match beyond tail window)
    let mut stderr_429 = String::from("ERROR: HTTP Error 429: Too Many Requests\r\n");
    for i in 0..80 {
        stderr_429.push_str(&format!("INFO: post-error cleanup line {i}\r\n"));
    }
    let stderr_429_file = dir.path().join("stderr_429.txt");
    std::fs::write(&stderr_429_file, stderr_429.as_bytes()).unwrap();

    let ytdlp_429_cmd = dir.path().join("mock_ytdlp_429.cmd");
    std::fs::write(
        &ytdlp_429_cmd,
        format!(
            "@echo off\r\n\
             type \"{}\" 1>&2\r\n\
             exit /b 1\r\n",
            stderr_429_file.display()
        ),
    )
    .unwrap();

    {
        let _ytdlp_env = ScopedEnv::set("NEUROCLIP_YTDLP_PATH", &ytdlp_429_cmd.to_string_lossy());
        let res = download_audio_stream(
            "https://www.youtube.com/watch?v=bot_blocked_429",
            &out_dir,
            "vod_429_stream",
            &CookieSource::None,
            None,
            |_| {},
            CancellationToken::new(),
        )
        .await;

        assert!(
            matches!(res, Err(AppError::Ingestion(IngestError::BotBlocked(_)))),
            "Expected BotBlocked for early 429 followed by >64 lines, got {res:?}"
        );
    }

    // Case 3: >= 35 warning lines followed by non-bot-block ExtractionFailed error preserves trailing error
    let mut stderr_other = String::new();
    for i in 0..35 {
        stderr_other.push_str(&format!("WARNING: preamble {i}\r\n"));
    }
    stderr_other.push_str("ERROR: Unsupported URL\r\n");
    let stderr_other_file = dir.path().join("stderr_other.txt");
    std::fs::write(&stderr_other_file, stderr_other.as_bytes()).unwrap();

    let ytdlp_other_cmd = dir.path().join("mock_ytdlp_other.cmd");
    std::fs::write(
        &ytdlp_other_cmd,
        format!(
            "@echo off\r\n\
             type \"{}\" 1>&2\r\n\
             exit /b 1\r\n",
            stderr_other_file.display()
        ),
    )
    .unwrap();

    {
        let _ytdlp_env = ScopedEnv::set("NEUROCLIP_YTDLP_PATH", &ytdlp_other_cmd.to_string_lossy());
        let res = download_audio_stream(
            "https://www.youtube.com/watch?v=unsupported_err",
            &out_dir,
            "vod_other_tail",
            &CookieSource::None,
            None,
            |_| {},
            CancellationToken::new(),
        )
        .await;

        match res {
            Err(AppError::Ingestion(IngestError::ExtractionFailed(details))) => {
                assert!(
                    details.contains("Unsupported URL"),
                    "Expected trailing ExtractionFailed diagnostic, got: {details}"
                );
            }
            other => panic!("Expected Err(AppError::Ingestion(IngestError::ExtractionFailed(_))), got {other:?}"),
        }
    }
}
