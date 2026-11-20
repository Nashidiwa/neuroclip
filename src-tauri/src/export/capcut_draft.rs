//! CapCut PC Draft project generator with Dual Export Safeguard.
//!
//! Implements auto-detection of Windows CapCut drafts directory,
//! project directory generation, creation of `draft_content.json` and `draft_meta_info.json`,
//! and invariant Dual Export Safeguard (always writes `subtitle.ass` and `subtitle.srt`).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, ExportError};
use crate::export::capcut_schema::{
    canvas_config_for_layout, hex_to_normalized_rgb, sec_to_micros,
    DraftAudioMaterial, DraftContent, DraftMaterials, DraftMetaInfo,
    DraftMetaMaterialItem, DraftMetaMaterials, DraftSegment, DraftStoreItem,
    DraftTextMaterial, DraftTrack, DraftVideoMaterial, RootMetaInfo, TimeRange,
    DEFAULT_SUBTITLE_TRANSFORM_X, DEFAULT_SUBTITLE_TRANSFORM_Y,
    MIN_CLIP_DURATION_MICROS,
};
use crate::export::{export_ass_project_file, export_srt_project_file, SubtitleProject};

static CAPCUT_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Generate a unique 36-character UUID string for CapCut draft entities in RFC 4122 v4 format.
#[must_use]
pub fn generate_capcut_id() -> String {
    let count = CAPCUT_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    #[allow(clippy::cast_possible_truncation)]
    let hi = (count >> 32) as u32;
    #[allow(clippy::cast_possible_truncation)]
    let mid = (count >> 16) as u16;
    #[allow(clippy::cast_possible_truncation)]
    let low1 = (count & 0x0FFF) as u16;
    #[allow(clippy::cast_possible_truncation)]
    let low2 = ((count >> 4) & 0x0FFF) as u16;
    format!("{hi:08x}-{mid:04x}-4{low1:03x}-8{low2:03x}-{count:012x}")
}

/// Result returned from successful CapCut Draft generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapCutExportResult {
    pub project_dir: String,
    pub project_path: String,
    pub draft_name: String,
    pub draft_content_path: String,
    pub draft_meta_info_path: String,
    pub ass_backup_path: String,
    pub srt_backup_path: String,
    pub dual_safeguard_files: Vec<String>,
    pub duration_ms: u64,
}

/// Detect the default CapCut drafts directory on Windows or fallback.
///
/// Path: `%LOCALAPPDATA%/CapCut/User Data/Projects/com.lveditor.draft/`.
///
/// # Errors
/// Returns [`AppError::Export`] with [`ExportError::CapCutDirectoryNotFound`] if environment variables cannot be read.
pub fn default_capcut_drafts_dir() -> Result<PathBuf, AppError> {
    let local_app_data = std::env::var("LOCALAPPDATA")
        .or_else(|_| std::env::var("USERPROFILE").map(|up| format!("{up}\\AppData\\Local")))
        .map_err(|_| AppError::Export(ExportError::CapCutDirectoryNotFound))?;

    let path = PathBuf::from(local_app_data)
        .join("CapCut")
        .join("User Data")
        .join("Projects")
        .join("com.lveditor.draft");

    Ok(path)
}

/// Detect or resolve CapCut drafts root directory.
///
/// If `override_dir` is provided, returns that path and ensures it exists.
/// Otherwise, checks if default CapCut directory exists or can be created if CapCut is installed.
///
/// # Errors
/// Returns [`AppError::Export`] with [`ExportError::CapCutDirectoryNotFound`] if CapCut is not installed
/// and no override directory is provided.
pub fn detect_capcut_drafts_dir(override_dir: Option<&Path>) -> Result<PathBuf, AppError> {
    if let Some(custom) = override_dir {
        if !custom.exists() {
            std::fs::create_dir_all(custom).map_err(|e| {
                AppError::Export(ExportError::FileWriteFailed {
                    path: custom.display().to_string(),
                    details: e.to_string(),
                })
            })?;
        }
        return Ok(custom.to_path_buf());
    }

    let default_dir = default_capcut_drafts_dir()?;
    if default_dir.exists() {
        return Ok(default_dir);
    }

    // Check if CapCut installation root exists
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        let capcut_root = PathBuf::from(local_app_data).join("CapCut");
        if capcut_root.exists() {
            std::fs::create_dir_all(&default_dir).map_err(|e| {
                AppError::Export(ExportError::FileWriteFailed {
                    path: default_dir.display().to_string(),
                    details: e.to_string(),
                })
            })?;
            return Ok(default_dir);
        }
    }

    Err(AppError::Export(ExportError::CapCutDirectoryNotFound))
}

/// Sanitize project name to be safe for Windows filesystem.
#[must_use]
pub fn sanitize_project_name(name: &str) -> String {
    let clean: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let trimmed = clean.trim().trim_matches('.');
    if trimmed.is_empty() {
        "NeuroClip_Highlight".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Generate project folder name: `NeuroClip_Highlight_<timestamp>`.
#[must_use]
pub fn generate_project_folder_name(project_name: &str) -> String {
    let sanitized = sanitize_project_name(project_name);
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S").to_string();
    if sanitized.starts_with("NeuroClip_") {
        format!("{sanitized}_{timestamp}")
    } else {
        format!("NeuroClip_Highlight_{timestamp}")
    }
}

/// Build `DraftContent` struct for `draft_content.json`.
///
/// # Errors
/// Returns [`AppError::Export`] with [`ExportError::InvalidLayout`] if `project.target_layout` is unsupported.
#[allow(clippy::too_many_lines)]
pub fn build_capcut_draft_content(
    project: &SubtitleProject,
    video_path: Option<&str>,
) -> Result<DraftContent, AppError> {
    let canvas_config = canvas_config_for_layout(&project.target_layout)?;
    let draft_id = generate_capcut_id();

    let seg_count = project.segments.len();
    let mut video_materials = Vec::with_capacity(1);
    let mut audio_materials = Vec::with_capacity(1);
    let mut text_materials = Vec::with_capacity(seg_count);

    let mut video_segments = Vec::with_capacity(1);
    let mut audio_segments = Vec::with_capacity(1);
    let mut text_segments = Vec::with_capacity(seg_count);

    // Determine max duration from segments
    let mut max_segment_end_sec: f64 = 0.0;
    for seg in &project.segments {
        if seg.end_sec > max_segment_end_sec {
            max_segment_end_sec = seg.end_sec;
        }
    }
    let total_micros = sec_to_micros(max_segment_end_sec.max(1.0));

    // 1. Video & Audio Tracks (Track 0 & Track 1)
    if let Some(v_path) = video_path {
        let video_mat_id = generate_capcut_id();
        let audio_mat_id = generate_capcut_id();

        let mat_name = Path::new(v_path)
            .file_name()
            .map_or_else(|| "clip_slice.mp4".to_string(), |f| f.to_string_lossy().to_string());

        video_materials.push(DraftVideoMaterial {
            id: video_mat_id.clone(),
            path: v_path.to_string(),
            duration: total_micros,
            width: canvas_config.width,
            height: canvas_config.height,
            material_name: mat_name,
        });

        audio_materials.push(DraftAudioMaterial {
            id: audio_mat_id.clone(),
            path: v_path.to_string(),
            duration: total_micros,
            material_name: "audio_track".to_string(),
        });

        video_segments.push(DraftSegment {
            id: generate_capcut_id(),
            material_id: video_mat_id,
            target_timerange: TimeRange {
                start: 0,
                duration: total_micros,
            },
            source_timerange: TimeRange {
                start: 0,
                duration: total_micros,
            },
            render_index: 0,
            extra_material_refs: Vec::new(),
        });

        audio_segments.push(DraftSegment {
            id: generate_capcut_id(),
            material_id: audio_mat_id,
            target_timerange: TimeRange {
                start: 0,
                duration: total_micros,
            },
            source_timerange: TimeRange {
                start: 0,
                duration: total_micros,
            },
            render_index: 0,
            extra_material_refs: Vec::new(),
        });
    }

    // 2. Subtitle Text Track (Track 2)
    for (idx, seg) in project.segments.iter().enumerate() {
        let text_mat_id = generate_capcut_id();
        let raw_text = seg.text_vi.as_deref().unwrap_or(&seg.text_en);

        // Resolve speaker style (PO Option 1)
        let speaker_key = seg.speaker_id.to_ascii_lowercase();
        let color_hex = project
            .speaker_styles
            .get(&speaker_key)
            .map_or("#FFFFFF", |s| s.color_hex.as_str());

        let norm_rgb = hex_to_normalized_rgb(color_hex);
        let char_count = raw_text.chars().count();

        // Build double-encoded inner content JSON
        let escaped_text =
            serde_json::to_string(raw_text).unwrap_or_else(|_| "\"\"".to_string());
        let content_json = format!(
            r#"{{"styles":[{{"fill":{{"alpha":1.0,"content":{{"solid":{{"color":[{:.6},{:.6},{:.6}]}}}}}},"range":[0,{}]}}],"text":{}}}"#,
            norm_rgb[0], norm_rgb[1], norm_rgb[2], char_count, escaped_text
        );

        text_materials.push(DraftTextMaterial {
            id: text_mat_id.clone(),
            content: content_json,
            font_size: 8.0,
            text_color: color_hex.to_string(),
            transform_x: DEFAULT_SUBTITLE_TRANSFORM_X,
            transform_y: DEFAULT_SUBTITLE_TRANSFORM_Y,
            alignment: 1,   // Center
            typesetting: 0, // Horizontal
            check_flag: 7,
        });

        let start_micros = sec_to_micros(seg.start_sec);
        let raw_duration = sec_to_micros((seg.end_sec - seg.start_sec).max(0.0));
        let duration_micros = if raw_duration < MIN_CLIP_DURATION_MICROS {
            MIN_CLIP_DURATION_MICROS
        } else {
            raw_duration
        };

        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let render_idx = 1000 + (idx as i32);

        text_segments.push(DraftSegment {
            id: generate_capcut_id(),
            material_id: text_mat_id,
            target_timerange: TimeRange {
                start: start_micros,
                duration: duration_micros,
            },
            source_timerange: TimeRange {
                start: 0,
                duration: duration_micros,
            },
            render_index: render_idx,
            extra_material_refs: Vec::new(),
        });
    }

    let tracks = vec![
        DraftTrack {
            id: generate_capcut_id(),
            track_type: "video".to_string(),
            attribute: 0,
            flag: 0,
            segments: video_segments,
        },
        DraftTrack {
            id: generate_capcut_id(),
            track_type: "audio".to_string(),
            attribute: 0,
            flag: 0,
            segments: audio_segments,
        },
        DraftTrack {
            id: generate_capcut_id(),
            track_type: "text".to_string(),
            attribute: 0,
            flag: 0,
            segments: text_segments,
        },
    ];

    Ok(DraftContent {
        id: draft_id,
        name: project.project_name.clone(),
        fps: 30.0,
        duration: total_micros,
        version: 2,
        canvas_config,
        materials: DraftMaterials {
            videos: video_materials,
            audios: audio_materials,
            texts: text_materials,
        },
        tracks,
    })
}

/// Build `DraftMetaInfo` for `draft_meta_info.json`.
#[must_use]
pub fn build_capcut_draft_meta_info(
    content: &DraftContent,
    project_dir: &Path,
    drafts_root: &Path,
    video_path: Option<&str>,
) -> DraftMetaInfo {
    let now_ms = Utc::now().timestamp_millis();
    let mut meta_items = Vec::new();

    if let Some(v_path) = video_path {
        if let Some(first_video) = content.materials.videos.first() {
            meta_items.push(DraftMetaMaterialItem {
                id: first_video.id.clone(),
                file_path: v_path.to_string(),
                metetype: "video".to_string(),
                duration: content.duration,
                width: content.canvas_config.width,
                height: content.canvas_config.height,
                extra_info: first_video.material_name.clone(),
                material_type: 0,
            });
        }
    }

    DraftMetaInfo {
        draft_id: content.id.clone(),
        draft_name: content.name.clone(),
        draft_fold_path: project_dir.display().to_string(),
        draft_root_path: drafts_root.display().to_string(),
        draft_type: 0,
        tm_draft_create: now_ms,
        tm_draft_modify: now_ms,
        tm_duration: content.duration,
        draft_cover: String::new(),
        draft_materials: DraftMetaMaterials {
            videos: Vec::new(),
            audios: Vec::new(),
            texts: Vec::new(),
            draft_materials: meta_items,
        },
    }
}

/// Best-effort update of `root_meta_info.json` if it exists in `drafts_root`.
///
/// Registers the new draft into CapCut's home screen `all_draft_store`.
/// Safe and non-fatal on any file I/O or JSON parsing error.
pub fn update_root_meta_info(drafts_root: &Path, item: &DraftStoreItem) {
    let root_meta_path = drafts_root.join("root_meta_info.json");
    if !root_meta_path.exists() {
        return;
    }

    let Ok(data) = std::fs::read_to_string(&root_meta_path) else {
        return;
    };

    let Ok(mut meta) = serde_json::from_str::<RootMetaInfo>(&data) else {
        return;
    };

    // If not already present, push and save back
    if !meta.all_draft_store.iter().any(|d| d.draft_id == item.draft_id) {
        meta.all_draft_store.push(item.clone());
        if let Ok(bytes) = serde_json::to_vec_pretty(&meta) {
            let _ = std::fs::write(&root_meta_path, bytes);
        }
    }
}

/// Export CapCut Draft project folder with unconditional Dual Export Safeguard (ERR-09).
///
/// Always writes `subtitle.ass` and `subtitle.srt` (with UTF-8 BOM `[0xEF, 0xBB, 0xBF]`)
/// into the project folder alongside `draft_content.json` and `draft_meta_info.json`.
///
/// # Errors
/// Returns [`AppError::Export`] with canonical error codes:
/// - [`ExportError::CapCutDirectoryNotFound`] if CapCut directory cannot be resolved.
/// - [`ExportError::FileWriteFailed`] if writing files or directories fails.
/// - [`ExportError::InvalidLayout`] if `project.target_layout` is unsupported.
pub fn export_capcut_draft_folder(
    project: &SubtitleProject,
    video_path: Option<&str>,
    custom_drafts_dir: Option<&Path>,
) -> Result<CapCutExportResult, AppError> {
    let start_time = Instant::now();

    // 1. Detect / Resolve drafts root
    let drafts_root = detect_capcut_drafts_dir(custom_drafts_dir)?;

    // 2. Generate project folder: NeuroClip_Highlight_<timestamp>
    let folder_name = generate_project_folder_name(&project.project_name);
    let project_dir = drafts_root.join(&folder_name);
    if !project_dir.exists() {
        std::fs::create_dir_all(&project_dir).map_err(|e| {
            AppError::Export(ExportError::FileWriteFailed {
                path: project_dir.display().to_string(),
                details: e.to_string(),
            })
        })?;
    }

    // 3. MANDATORY DUAL EXPORT SAFEGUARD (ERR-09):
    // Invariant rule: ALWAYS generate and write subtitle.ass and subtitle.srt into that project directory FIRST!
    let ass_path = project_dir.join("subtitle.ass");
    export_ass_project_file(project, &ass_path)?;

    let srt_path = project_dir.join("subtitle.srt");
    export_srt_project_file(project, &srt_path)?;

    // 4. Generate draft_content.json
    let effective_video_path = video_path.or(project.video_file_path.as_deref());
    let content = build_capcut_draft_content(project, effective_video_path)?;
    let content_path = project_dir.join("draft_content.json");
    let content_bytes = serde_json::to_vec(&content).map_err(|e| {
        AppError::Export(ExportError::FileWriteFailed {
            path: content_path.display().to_string(),
            details: e.to_string(),
        })
    })?;
    std::fs::write(&content_path, content_bytes).map_err(|e| {
        AppError::Export(ExportError::FileWriteFailed {
            path: content_path.display().to_string(),
            details: e.to_string(),
        })
    })?;

    // 5. Generate draft_meta_info.json
    let meta = build_capcut_draft_meta_info(
        &content,
        &project_dir,
        &drafts_root,
        effective_video_path,
    );
    let meta_path = project_dir.join("draft_meta_info.json");
    let meta_bytes = serde_json::to_vec(&meta).map_err(|e| {
        AppError::Export(ExportError::FileWriteFailed {
            path: meta_path.display().to_string(),
            details: e.to_string(),
        })
    })?;
    std::fs::write(&meta_path, meta_bytes).map_err(|e| {
        AppError::Export(ExportError::FileWriteFailed {
            path: meta_path.display().to_string(),
            details: e.to_string(),
        })
    })?;

    // 6. Best-effort update of root_meta_info.json
    let store_item = DraftStoreItem {
        draft_id: meta.draft_id.clone(),
        draft_name: meta.draft_name.clone(),
        draft_fold_path: meta.draft_fold_path.clone(),
        draft_root_path: meta.draft_root_path.clone(),
        tm_draft_create: meta.tm_draft_create,
        tm_draft_modify: meta.tm_draft_modify,
        draft_type: 0,
    };
    update_root_meta_info(&drafts_root, &store_item);

    #[allow(clippy::cast_possible_truncation)]
    let duration_ms = start_time.elapsed().as_millis() as u64;

    let ass_str = ass_path.display().to_string();
    let srt_str = srt_path.display().to_string();
    let project_dir_str = project_dir.display().to_string();

    Ok(CapCutExportResult {
        project_dir: project_dir_str.clone(),
        project_path: project_dir_str,
        draft_name: folder_name,
        draft_content_path: content_path.display().to_string(),
        draft_meta_info_path: meta_path.display().to_string(),
        ass_backup_path: ass_str.clone(),
        srt_backup_path: srt_str.clone(),
        dual_safeguard_files: vec![ass_str, srt_str],
        duration_ms,
    })
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::cast_precision_loss,
    clippy::cast_lossless
)]
mod tests {
    use super::*;
    use crate::export::create_subtitle_project;
    use crate::stt::transcript_parser::{TranscriptSegment, TranscriptSegmentsContainer, WordTiming};

    fn make_test_container() -> TranscriptSegmentsContainer {
        TranscriptSegmentsContainer {
            clip_id: "clip_m4_25_test".to_string(),
            segments: vec![TranscriptSegment {
                segment_id: "seg_1".to_string(),
                start_sec: 1.0,
                end_sec: 3.5,
                speaker_id: "neuro".to_string(),
                text_en: "Hello tutel".to_string(),
                text_vi: Some("Chào con rùa".to_string()),
                confidence: 0.98,
                cps_score: 12.0,
                is_flagged_for_review: false,
                words: vec![WordTiming {
                    word: "Hello".to_string(),
                    start_sec: 1.0,
                    end_sec: 2.0,
                    probability: 0.99,
                }],
            }],
        }
    }

    #[test]
    fn test_sanitize_project_name() {
        assert_eq!(
            sanitize_project_name("Neuro: <Vedal> *Highlight*?"),
            "Neuro_ _Vedal_ _Highlight__"
        );
        assert_eq!(sanitize_project_name("..."), "NeuroClip_Highlight");
        assert_eq!(sanitize_project_name("   "), "NeuroClip_Highlight");
    }

    #[test]
    fn test_generate_project_folder_name() {
        let name = generate_project_folder_name("Vedal_Banter");
        assert!(name.starts_with("NeuroClip_Highlight_"));

        let neuro_name = generate_project_folder_name("NeuroClip_EvilStream");
        assert!(neuro_name.starts_with("NeuroClip_EvilStream_"));
    }

    #[test]
    fn test_build_capcut_draft_content_three_tracks() {
        let container = make_test_container();
        let project = create_subtitle_project(
            container,
            "CapCut Test Project",
            "vod_test_1",
            Some("preset_center_crop_9_16"),
        );

        let content = build_capcut_draft_content(&project, Some("C:\\media\\clip.mp4")).unwrap();

        assert_eq!(content.tracks.len(), 3);
        assert_eq!(content.tracks[0].track_type, "video");
        assert_eq!(content.tracks[1].track_type, "audio");
        assert_eq!(content.tracks[2].track_type, "text");

        assert_eq!(content.materials.texts.len(), 1);
        let text_mat = &content.materials.texts[0];
        assert_eq!(text_mat.transform_x, 0.0);
        assert_eq!(text_mat.transform_y, -0.8);
        assert_eq!(text_mat.text_color, "#FFB6C1");
        assert!(text_mat.content.contains("Chào con rùa"));
    }

    #[test]
    fn test_export_capcut_draft_folder_with_safeguard() {
        let container = make_test_container();
        let project = create_subtitle_project(
            container,
            "Safeguard Test",
            "vod_test_2",
            Some("original_16_9"),
        );

        let temp_dir = std::env::temp_dir().join(format!(
            "neuroclip_capcut_test_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));

        let result = export_capcut_draft_folder(
            &project,
            Some("C:\\media\\test.mp4"),
            Some(&temp_dir),
        )
        .unwrap();

        assert!(Path::new(&result.project_dir).exists());
        assert!(Path::new(&result.draft_content_path).exists());
        assert!(Path::new(&result.draft_meta_info_path).exists());
        assert!(Path::new(&result.ass_backup_path).exists());
        assert!(Path::new(&result.srt_backup_path).exists());

        // Verify UTF-8 BOM in backup files
        let ass_bytes = std::fs::read(&result.ass_backup_path).unwrap();
        assert_eq!(&ass_bytes[0..3], &[0xEF, 0xBB, 0xBF]);

        let srt_bytes = std::fs::read(&result.srt_backup_path).unwrap();
        assert_eq!(&srt_bytes[0..3], &[0xEF, 0xBB, 0xBF]);

        // SLA benchmark check
        assert!(result.duration_ms <= 200);

        let _ = std::fs::remove_dir_all(temp_dir);
    }
}
