//! Offline local video file importer (`TASK-M4-07`).
//!
//! Supports `.mp4`, `.mkv`, `.webm`, `.mov`, `.ts`, `.flv` with zero outbound network calls (`AC-07.3`),
//! extracts 16kHz mono PCM WAV audio (`AC-07.1`), and writes Schema 5.2 `vod_manifest.json`
//! (`source_type: "local_file"`, `video_stream_url: null`, `local_video_path`).

use std::path::{Path, PathBuf};
use std::process::Stdio;

use sha2::{Digest, Sha256};
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use crate::core::storage::check_disk_space;
use crate::error::{AppError, IngestError, SidecarError, StorageError};
use crate::ingestion::range_slicer::{
    parse_ffprobe_json, resolve_ffmpeg_binary, resolve_ffprobe_binary,
};
use crate::ingestion::streamer::{
    build_ffmpeg_file_args, generate_vod_manifest_with_streams, probe_media_duration_fallback,
    IngestProgressPayload, VodManifest,
};
use crate::sidecar::{
    assign_child_to_job_guard, ChildProcessGuard, SubprocessConfig, SubprocessSupervisor,
};

/// Supported local video container extensions (`UT-07.1`).
pub const SUPPORTED_LOCAL_VIDEO_EXTENSIONS: &[&str] = &["mp4", "mkv", "webm", "mov", "ts", "flv"];

/// Check whether a path has a supported local video extension (case-insensitive).
#[must_use]
pub fn is_supported_local_video_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .is_some_and(|ext| SUPPORTED_LOCAL_VIDEO_EXTENSIONS.contains(&ext.as_str()))
}

fn strip_windows_verbatim_prefix(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        let s = path.to_string_lossy();
        if let Some(stripped) = s.strip_prefix(r"\\?\") {
            return PathBuf::from(stripped);
        }
    }
    path
}

/// Validate that `file_path` has a supported video extension and exists as a file on disk.
///
/// # Errors
/// - `AppError::Ingestion(IngestError::InvalidUrl)` for empty path or unsupported extension (`UT-07.2`)
/// - `AppError::Storage(StorageError::FileNotFound)` if the file does not exist
pub fn validate_local_video_path(file_path: &Path) -> Result<PathBuf, AppError> {
    let raw_str = file_path.to_string_lossy();
    if raw_str.trim().is_empty() {
        return Err(AppError::Ingestion(IngestError::InvalidUrl(
            "Đường dẫn tệp video cục bộ không được để trống".to_string(),
        )));
    }

    if !is_supported_local_video_extension(file_path) {
        return Err(AppError::Ingestion(IngestError::InvalidUrl(format!(
            "Định dạng tệp không được hỗ trợ (chỉ hỗ trợ {}): {}",
            SUPPORTED_LOCAL_VIDEO_EXTENSIONS.join(", "),
            file_path.display()
        ))));
    }

    if !file_path.exists() || !file_path.is_file() {
        return Err(AppError::Storage(StorageError::FileNotFound(
            file_path.display().to_string(),
        )));
    }

    let canonical = std::fs::canonicalize(file_path).unwrap_or_else(|_| file_path.to_path_buf());
    Ok(strip_windows_verbatim_prefix(canonical))
}

/// Build offline `FFmpeg` arguments to extract 16kHz mono PCM WAV from a local video file.
///
/// Guarantees zero network flags (no `-reconnect*`) (`ED-07.1`).
#[must_use]
pub fn build_local_wav_extraction_args(input_path: &Path, output_wav: &Path) -> Vec<String> {
    build_ffmpeg_file_args(&input_path.to_string_lossy(), &output_wav.to_string_lossy())
}

/// Compute a deterministic `vod_id` for a local video path.
#[must_use]
pub fn derive_local_vod_id(canonical_path: &Path) -> String {
    use std::fmt::Write as _;

    let stem = canonical_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("local_video");
    let sanitized: String = stem
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .take(24)
        .collect();

    let mut hasher = Sha256::new();
    hasher.update(canonical_path.to_string_lossy().as_bytes());
    let digest = hasher.finalize();
    let mut short_hex = String::with_capacity(8);
    for b in digest.iter().take(4) {
        let _ = write!(short_hex, "{b:02x}");
    }

    let clean_stem = sanitized.trim_matches('_');
    if clean_stem.is_empty() {
        format!("local_{short_hex}")
    } else {
        format!("local_{clean_stem}_{short_hex}")
    }
}

/// Probe the media duration (in seconds) of a local video file offline using `ffprobe`.
///
/// # Errors
/// Returns `AppError::Sidecar` if `ffprobe` is missing or execution fails, or
/// `AppError::TaskCancelled` if cancelled.
pub async fn probe_local_video_duration(
    file_path: &Path,
    cancellation_token: &CancellationToken,
) -> Result<f64, AppError> {
    if cancellation_token.is_cancelled() {
        return Err(AppError::TaskCancelled);
    }

    if let Ok(mock_val) = std::env::var("NEUROCLIP_MOCK_LOCAL_IMPORT") {
        let parsed = mock_val.trim().parse::<f64>().unwrap_or(120.0);
        return Ok(if parsed.is_finite() && parsed > 0.0 {
            parsed
        } else {
            120.0
        });
    }

    let ffprobe_bin = resolve_ffprobe_binary();
    let mut cmd = Command::new(&ffprobe_bin);
    cmd.args([
        "-v",
        "error",
        "-show_entries",
        "format=duration:stream=duration",
        "-of",
        "json",
    ])
    .arg(file_path.as_os_str())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .kill_on_drop(true);

    let child = cmd.spawn().map_err(|e| {
        AppError::Sidecar(SidecarError::BinaryNotFound {
            name: "ffprobe".to_string(),
            path: format!("{ffprobe_bin}: {e}"),
        })
    })?;

    assign_child_to_job_guard(&child);
    let mut guard = ChildProcessGuard::new(child);

    let Some(child_proc) = guard.take_child() else {
        return Err(AppError::Internal(
            "ChildProcessGuard missing ffprobe process".to_string(),
        ));
    };

    let output = tokio::select! {
        biased;
        () = cancellation_token.cancelled() => {
            return Err(AppError::TaskCancelled);
        }
        res = child_proc.wait_with_output() => {
            res.map_err(|e| AppError::Sidecar(SidecarError::ExecutionFailed {
                binary: "ffprobe".to_string(),
                exit_code: None,
                details: e.to_string(),
            }))?
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(AppError::Sidecar(SidecarError::ExecutionFailed {
            binary: "ffprobe".to_string(),
            exit_code: output.status.code(),
            details: stderr,
        }));
    }

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let duration = if let Ok(parsed) = parse_ffprobe_json(&stdout_str) {
        parsed
            .format
            .as_ref()
            .and_then(|f| f.duration.as_deref())
            .and_then(|s| s.trim().parse::<f64>().ok())
            .filter(|d| d.is_finite() && *d > 0.0)
            .or_else(|| {
                parsed
                    .streams
                    .iter()
                    .filter_map(|s| s.duration.as_deref())
                    .filter_map(|s| s.trim().parse::<f64>().ok())
                    .filter(|d| d.is_finite() && *d > 0.0)
                    .max_by(f64::total_cmp)
            })
            .unwrap_or(0.0)
    } else {
        stdout_str.trim().parse::<f64>().unwrap_or(0.0)
    };
    if duration.is_finite() && duration > 0.0 {
        Ok(duration)
    } else {
        Ok(0.0)
    }
}

fn build_synthetic_pcm_wav_bytes(sample_count: usize) -> Vec<u8> {
    let data_bytes = u32::try_from(sample_count.saturating_mul(2)).unwrap_or(0);
    let riff_size = data_bytes.saturating_add(36);
    let mut wav = Vec::with_capacity(44 + data_bytes as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&riff_size.to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes()); // PCM chunk size
    wav.extend_from_slice(&1u16.to_le_bytes()); // AudioFormat = PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // NumChannels = 1
    wav.extend_from_slice(&16_000u32.to_le_bytes()); // SampleRate = 16000
    wav.extend_from_slice(&32_000u32.to_le_bytes()); // ByteRate = 16000 * 1 * 2
    wav.extend_from_slice(&2u16.to_le_bytes()); // BlockAlign = 2
    wav.extend_from_slice(&16u16.to_le_bytes()); // BitsPerSample = 16
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_bytes.to_le_bytes());
    for i in 0..sample_count {
        let sample = i16::try_from((i % 256) * 64).unwrap_or(0);
        wav.extend_from_slice(&sample.to_le_bytes());
    }
    wav
}

#[derive(Debug, Clone, Copy, Default)]
struct LocalImportRuntimeOptions {
    skip_disk_check: bool,
    mock_duration: Option<f64>,
}

fn finalize_local_import_manifest(
    vod_id: &str,
    canonical_video: &Path,
    duration_seconds: f64,
    audio_cache_path: &Path,
    project_dir: &Path,
) -> Result<VodManifest, AppError> {
    let title = canonical_video
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("Local Video")
        .to_string();

    let canonical_str = canonical_video.to_string_lossy().to_string();
    generate_vod_manifest_with_streams(
        vod_id,
        "local_file",
        None,
        &title,
        None,
        duration_seconds,
        audio_cache_path,
        project_dir,
        None,
        Some(&canonical_str),
    )
}

fn prepare_local_import_paths(
    file_path: &Path,
    project_root: &Path,
    cache_root: &Path,
    skip_disk_check: bool,
) -> Result<(PathBuf, String, PathBuf, PathBuf), AppError> {
    let canonical_video = validate_local_video_path(file_path)?;
    let vod_id = derive_local_vod_id(&canonical_video);

    let project_dir = project_root.join(&vod_id);
    std::fs::create_dir_all(&project_dir).map_err(|e| {
        AppError::Storage(StorageError::DirectoryCreationFailed {
            path: project_dir.display().to_string(),
            details: e.to_string(),
        })
    })?;

    std::fs::create_dir_all(cache_root).map_err(|e| {
        AppError::Storage(StorageError::DirectoryCreationFailed {
            path: cache_root.display().to_string(),
            details: e.to_string(),
        })
    })?;

    if !skip_disk_check {
        check_disk_space(Some(cache_root))?;
    }

    let audio_cache_path = cache_root.join(format!("{vod_id}_audio_cache.wav"));
    Ok((canonical_video, vod_id, project_dir, audio_cache_path))
}

async fn run_local_audio_extraction(
    canonical_video: &Path,
    audio_cache_path: &Path,
    effective_task_id: &str,
    initial_duration: f64,
    runtime_opts: LocalImportRuntimeOptions,
    cancellation_token: &CancellationToken,
) -> Result<f64, AppError> {
    if runtime_opts.mock_duration.is_some() || std::env::var("NEUROCLIP_MOCK_LOCAL_IMPORT").is_ok()
    {
        if cancellation_token.is_cancelled() {
            return Err(AppError::TaskCancelled);
        }
        let audio_cache_write_path = audio_cache_path.to_path_buf();
        tokio::task::spawn_blocking(move || {
            let wav_bytes = build_synthetic_pcm_wav_bytes(1600);
            std::fs::write(&audio_cache_write_path, wav_bytes).map_err(|e| {
                AppError::Storage(StorageError::DirectoryCreationFailed {
                    path: audio_cache_write_path.display().to_string(),
                    details: e.to_string(),
                })
            })
        })
        .await
        .map_err(|e| AppError::Internal(format!("Mock WAV write task panicked: {e}")))??;
        return Ok(initial_duration);
    }

    let ffmpeg_bin = resolve_ffmpeg_binary();
    let args = build_local_wav_extraction_args(canonical_video, audio_cache_path);
    let config = SubprocessConfig {
        task_id: effective_task_id.to_string(),
        binary: ffmpeg_bin,
        args,
        total_duration_sec: if initial_duration > 0.0 {
            Some(initial_duration)
        } else {
            None
        },
    };
    SubprocessSupervisor::run(&config, None, None, cancellation_token).await?;

    if initial_duration > 0.0 {
        return Ok(initial_duration);
    }
    let probed = probe_media_duration_fallback(audio_cache_path, cancellation_token).await;
    if probed.is_finite() && probed > 0.0 {
        Ok(probed)
    } else {
        Ok(0.0)
    }
}

async fn import_local_video_file_inner<F>(
    file_path: &Path,
    project_root: &Path,
    cache_root: &Path,
    task_id: Option<&str>,
    progress_callback: F,
    cancellation_token: CancellationToken,
    runtime_opts: LocalImportRuntimeOptions,
) -> Result<VodManifest, AppError>
where
    F: Fn(IngestProgressPayload) + Send + Sync + 'static,
{
    let file_path_owned = file_path.to_path_buf();
    let project_root_owned = project_root.to_path_buf();
    let cache_root_owned = cache_root.to_path_buf();
    let skip_disk_check = runtime_opts.skip_disk_check;

    let (canonical_video, vod_id, project_dir, audio_cache_path) =
        tokio::task::spawn_blocking(move || {
            prepare_local_import_paths(
                &file_path_owned,
                &project_root_owned,
                &cache_root_owned,
                skip_disk_check,
            )
        })
        .await
        .map_err(|e| {
            AppError::Internal(format!("Local import preparation task panicked: {e}"))
        })??;

    let effective_task_id = task_id
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(vod_id.as_str())
        .to_string();

    progress_callback(IngestProgressPayload {
        task_id: effective_task_id.clone(),
        stage: "probing_local_video".to_string(),
        percent: 10.0,
        speed_kbps: 0.0,
        eta_sec: 0,
    });

    let initial_duration = match runtime_opts.mock_duration {
        Some(d) => d,
        None => probe_local_video_duration(&canonical_video, &cancellation_token).await?,
    };

    progress_callback(IngestProgressPayload {
        task_id: effective_task_id.clone(),
        stage: "extracting_audio".to_string(),
        percent: 30.0,
        speed_kbps: 0.0,
        eta_sec: 0,
    });

    let duration_seconds = run_local_audio_extraction(
        &canonical_video,
        &audio_cache_path,
        &effective_task_id,
        initial_duration,
        runtime_opts,
        &cancellation_token,
    )
    .await?;

    let vod_id_for_manifest = vod_id;
    let manifest = tokio::task::spawn_blocking(move || {
        finalize_local_import_manifest(
            &vod_id_for_manifest,
            &canonical_video,
            duration_seconds,
            &audio_cache_path,
            &project_dir,
        )
    })
    .await
    .map_err(|e| AppError::Internal(format!("Manifest generation task panicked: {e}")))??;

    progress_callback(IngestProgressPayload {
        task_id: effective_task_id,
        stage: "complete".to_string(),
        percent: 100.0,
        speed_kbps: 0.0,
        eta_sec: 0,
    });

    Ok(manifest)
}

/// Import a local video file completely offline (`TASK-M4-07`).
///
/// 1. Validates supported extension & file existence.
/// 2. Checks pre-flight disk space (`>= 5GB`).
/// 3. Probes duration via `ffprobe` (zero outbound network).
/// 4. Extracts 16kHz mono PCM WAV via `ffmpeg` (`-vn -sn -dn -acodec pcm_s16le -ac 1 -ar 16000`).
/// 5. Persists Schema 5.2 `vod_manifest.json` (`source_type: "local_file"`, `video_stream_url: null`).
///
/// # Errors
/// Returns `AppError` if validation, storage check, probing, or audio extraction fails.
pub async fn import_local_video_file<F>(
    file_path: &Path,
    project_root: &Path,
    cache_root: &Path,
    task_id: Option<&str>,
    progress_callback: F,
    cancellation_token: CancellationToken,
) -> Result<VodManifest, AppError>
where
    F: Fn(IngestProgressPayload) + Send + Sync + 'static,
{
    import_local_video_file_inner(
        file_path,
        project_root,
        cache_root,
        task_id,
        progress_callback,
        cancellation_token,
        LocalImportRuntimeOptions::default(),
    )
    .await
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    struct TempTestDir {
        path: PathBuf,
    }

    impl TempTestDir {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "neuroclip_local_importer_{name}_{}_{}",
                std::process::id(),
                chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self { path }
        }
    }

    impl Drop for TempTestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn test_ut_07_1_supported_extensions_case_insensitive() {
        let dir = TempTestDir::new("ut_07_1");
        for ext in ["mp4", "MP4", "mkv", "Mkv", "webm", "mov", "ts", "flv"] {
            let p = dir.path.join(format!("sample.{ext}"));
            std::fs::write(&p, b"stub_video").unwrap();
            let validated = validate_local_video_path(&p);
            assert!(
                validated.is_ok(),
                "Expected extension .{ext} to be accepted, got {validated:?}"
            );
        }
    }

    #[test]
    fn test_ut_07_2_rejects_unsupported_and_missing_files() {
        let dir = TempTestDir::new("ut_07_2");
        for ext in ["txt", "exe", "zip", "pdf"] {
            let p = dir.path.join(format!("bad.{ext}"));
            std::fs::write(&p, b"not_a_video").unwrap();
            let err = validate_local_video_path(&p).unwrap_err();
            assert!(
                matches!(err, AppError::Ingestion(IngestError::InvalidUrl(_))),
                "Expected InvalidUrl for .{ext}, got {err:?}"
            );
        }

        let empty_err = validate_local_video_path(Path::new("")).unwrap_err();
        assert!(matches!(
            empty_err,
            AppError::Ingestion(IngestError::InvalidUrl(_))
        ));

        let missing_mp4 = dir.path.join("does_not_exist.mp4");
        let missing_err = validate_local_video_path(&missing_mp4).unwrap_err();
        assert!(matches!(
            missing_err,
            AppError::Storage(StorageError::FileNotFound(_))
        ));
    }

    #[test]
    fn test_ed_07_1_ffmpeg_args_zero_outbound_network() {
        let args = build_local_wav_extraction_args(
            Path::new("D:/Videos/NeuroClip_Stream.mp4"),
            Path::new("D:/Cache/audio_cache.wav"),
        );

        assert!(args.contains(&"-vn".to_string()));
        assert!(args.contains(&"-sn".to_string()));
        assert!(args.contains(&"-dn".to_string()));
        assert!(args.contains(&"pcm_s16le".to_string()));
        assert!(args.contains(&"1".to_string()));
        assert!(args.contains(&"16000".to_string()));

        // Strictly offline: no reconnect flags, no http/https URLs
        assert!(!args.iter().any(|a| a.contains("reconnect")));
        assert!(!args
            .iter()
            .any(|a| a.starts_with("http://") || a.starts_with("https://")));
    }

    #[tokio::test]
    async fn test_it_07_1_and_it_07_2_mock_local_import_wav_and_manifest() {
        let dir = TempTestDir::new("it_07_1_2");
        let video_path = dir.path.join("Neuro_Local_VOD.mkv");
        std::fs::write(&video_path, b"FAKE_MKV_CONTENT").unwrap();
        let projects_root = dir.path.join("projects");
        let cache_root = dir.path.join("cache");

        let events: Arc<Mutex<Vec<IngestProgressPayload>>> = Arc::new(Mutex::new(Vec::new()));
        let events_clone = Arc::clone(&events);

        let result = import_local_video_file_inner(
            &video_path,
            &projects_root,
            &cache_root,
            Some("local_task_1"),
            move |payload| {
                events_clone.lock().unwrap().push(payload);
            },
            CancellationToken::new(),
            LocalImportRuntimeOptions {
                skip_disk_check: true,
                mock_duration: Some(345.5),
            },
        )
        .await;

        let manifest = result.expect("Local import should succeed in offline mock mode");
        assert_eq!(manifest.source_type, "local_file");
        assert_eq!(manifest.original_url, None);
        assert_eq!(manifest.video_stream_url, None);
        assert!(manifest.local_video_path.is_some());
        assert!((manifest.duration_seconds - 345.5).abs() < 1e-4);

        let audio_file = PathBuf::from(&manifest.audio_path);
        assert!(audio_file.exists(), "16kHz WAV audio cache must exist");
        let wav_bytes = std::fs::read(&audio_file).unwrap();
        assert!(wav_bytes.starts_with(b"RIFF"));
        assert_eq!(&wav_bytes[8..12], b"WAVE");

        let manifest_json_path = projects_root
            .join(&manifest.vod_id)
            .join("vod_manifest.json");
        assert!(manifest_json_path.exists());
        let saved_manifest: VodManifest =
            serde_json::from_slice(&std::fs::read(&manifest_json_path).unwrap()).unwrap();
        assert_eq!(saved_manifest.source_type, "local_file");
        assert_eq!(saved_manifest.video_stream_url, None);

        let captured = events.lock().unwrap();
        assert!(captured
            .iter()
            .any(|p| p.stage == "complete" && (p.percent - 100.0).abs() < 1e-4));
    }
}
