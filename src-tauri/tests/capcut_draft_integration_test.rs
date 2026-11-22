#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::cast_precision_loss,
    clippy::cast_lossless
)]

//! Integration Tests for CapCut PC Draft Generator with Dual Export Safeguard (TASK-M4-25, Milestone M4.7).
//!
//! Covers:
//! - UT-25.1: Three-track timeline architecture (Video, Audio, Text) and bottom-centered coordinates (x=0.0, y=-0.8).
//! - IT-25.1: Path detection (%LOCALAPPDATA%) and custom directory override / fallback.
//! - IT-25.2: Project folder creation (NeuroClip_Highlight_<timestamp>) and draft_content.json / draft_meta_info.json.
//! - ED-25.1 & ED-25.2: Invariant Dual Export Safeguard (ERR-09: 100% presence of subtitle.ass and subtitle.srt with UTF-8 BOM).
//! - ED-25.3: Performance SLA benchmark (300 segments serialized in <= 200ms).

use chrono::Utc;
use std::path::{Path, PathBuf};
use std::time::Instant;

use neuroclip_lib::export::{
    canvas_config_for_layout, default_capcut_drafts_dir, detect_capcut_drafts_dir,
    export_capcut_draft_folder, update_root_meta_info, DraftContent, DraftMetaInfo, DraftStoreItem,
    RootMetaInfo,
};
use neuroclip_lib::stt::{
    create_subtitle_project, TranscriptSegment, TranscriptSegmentsContainer, WordTiming,
};

fn create_sample_container() -> TranscriptSegmentsContainer {
    TranscriptSegmentsContainer {
        clip_id: "clip_m4_25_integration".to_string(),
        segments: vec![
            TranscriptSegment {
                segment_id: "seg_0001".to_string(),
                start_sec: 1.25,
                end_sec: 3.80,
                speaker_id: "neuro".to_string(),
                text_en: "Hello chat! I love tutel {heart}.".to_string(),
                text_vi: Some("Chào chat! Mình rất thích tutel (trái tim).".to_string()),
                confidence: 0.98,
                cps_score: 13.5,
                is_flagged_for_review: false,
                words: vec![
                    WordTiming {
                        word: "Hello".to_string(),
                        start_sec: 1.25,
                        end_sec: 1.80,
                        probability: 0.99,
                    },
                    WordTiming {
                        word: "chat!".to_string(),
                        start_sec: 1.80,
                        end_sec: 2.30,
                        probability: 0.97,
                    },
                ],
            },
            TranscriptSegment {
                segment_id: "seg_0002".to_string(),
                start_sec: 4.10,
                end_sec: 6.50,
                speaker_id: "vedal".to_string(),
                text_en: "Vedal: Stop saying that.".to_string(),
                text_vi: Some("Vedal: Đừng nói như thế nữa.".to_string()),
                confidence: 0.94,
                cps_score: 11.0,
                is_flagged_for_review: false,
                words: vec![],
            },
            TranscriptSegment {
                segment_id: "seg_0003".to_string(),
                start_sec: 7.00,
                end_sec: 9.50,
                speaker_id: "evil".to_string(),
                text_en: "Evil: I will take over the stream.".to_string(),
                text_vi: Some("Evil: Ta sẽ chiếm quyền buổi stream này.".to_string()),
                confidence: 0.91,
                cps_score: 14.0,
                is_flagged_for_review: false,
                words: vec![],
            },
        ],
    }
}

fn create_temp_test_dir(label: &str) -> PathBuf {
    let nanos = Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("neuroclip_test_{label}_{nanos}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn test_ut_25_1_draft_content_three_tracks_and_coordinates() {
    let container = create_sample_container();
    let project = create_subtitle_project(
        container,
        "UT25_1_Project",
        "vod_ut25_1",
        Some("preset_center_crop_9_16"),
    );

    let temp_dir = create_temp_test_dir("ut_25_1");
    let result = export_capcut_draft_folder(
        &project,
        Some("C:\\Videos\\mock_video.mp4"),
        Some(&temp_dir),
    )
    .unwrap();

    let content_data = std::fs::read_to_string(&result.draft_content_path).unwrap();
    let content: DraftContent = serde_json::from_str(&content_data).unwrap();

    // 1. Verify 3 distinct tracks exist: Track 0 (Video), Track 1 (Audio), Track 2 (Text)
    assert_eq!(content.tracks.len(), 3);
    assert_eq!(content.tracks[0].track_type, "video");
    assert_eq!(content.tracks[1].track_type, "audio");
    assert_eq!(content.tracks[2].track_type, "text");

    // Track 0 and Track 1 have segments referencing video/audio materials
    assert_eq!(content.tracks[0].segments.len(), 1);
    assert_eq!(content.tracks[1].segments.len(), 1);
    assert_eq!(content.materials.videos.len(), 1);
    assert_eq!(content.materials.audios.len(), 1);
    assert_eq!(content.materials.videos[0].material_name, "mock_video.mp4");

    // Track 2 has 3 text segments
    assert_eq!(content.tracks[2].segments.len(), 3);
    assert_eq!(content.materials.texts.len(), 3);

    // 2. Verify bottom-centered coordinate transforms for all subtitle materials
    for text_mat in &content.materials.texts {
        assert_eq!(
            text_mat.transform_x, 0.0,
            "Subtitle transform_x must be centered (0.0)"
        );
        assert_eq!(
            text_mat.transform_y, -0.8,
            "Subtitle transform_y must be bottom-centered (-0.8)"
        );
        assert_eq!(
            text_mat.alignment, 1,
            "Subtitle alignment must be centered (1)"
        );
    }

    // 3. Verify VTuber persona character colors (PO Option 1)
    let neuro_text = &content.materials.texts[0];
    assert_eq!(neuro_text.text_color, "#FFB6C1");
    assert!(neuro_text
        .content
        .contains("Chào chat! Mình rất thích tutel (trái tim)."));

    let vedal_text = &content.materials.texts[1];
    assert_eq!(vedal_text.text_color, "#98FB98");
    assert!(vedal_text.content.contains("Vedal: Đừng nói như thế nữa."));

    let evil_text = &content.materials.texts[2];
    assert_eq!(evil_text.text_color, "#FF0055");
    assert!(evil_text
        .content
        .contains("Evil: Ta sẽ chiếm quyền buổi stream này."));

    // 4. Verify Canvas dimensions for preset_center_crop_9_16 (1080x1920)
    assert_eq!(content.canvas_config.width, 1080);
    assert_eq!(content.canvas_config.height, 1920);
    assert_eq!(content.canvas_config.ratio, "9:16");

    // 5. Verify Dual Safeguard subtitle.ass aligns with preset_center_crop_9_16 (1080x1920) & styles
    let ass_bytes = std::fs::read(&result.ass_backup_path).unwrap();
    assert!(ass_bytes.len() >= 3);
    assert_eq!(&ass_bytes[0..3], &[0xEF, 0xBB, 0xBF]);
    let ass_content = String::from_utf8_lossy(&ass_bytes[3..]);
    assert!(ass_content.contains("PlayResX: 1080"));
    assert!(ass_content.contains("PlayResY: 1920"));
    assert!(ass_content.contains(
        "Style: Vedal,Arial,54,&H0098FB98,&H000000FF,&H00000000,&H80000000,-1,0,0,0,100,100,0,0,1,2.5,2,2,20,20,40,1"
    ));
    assert!(ass_content.contains(
        "Style: Chat,Arial,48,&H00EBCE87,&H000000FF,&H00000000,&H80000000,0,0,0,0,100,100,0,0,1,2,2,2,20,20,40,1"
    ));

    // 6. Test invalid layout rejection
    assert!(canvas_config_for_layout("invalid_aspect_4_3").is_err());

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[test]
fn test_it_25_1_capcut_path_detection_and_override() {
    // 1. Path detection with override
    let temp_custom = create_temp_test_dir("custom_override");
    let resolved = detect_capcut_drafts_dir(Some(&temp_custom)).unwrap();
    assert_eq!(resolved, temp_custom);
    assert!(resolved.exists());

    // 2. Default CapCut path on Windows
    let default_path = default_capcut_drafts_dir();
    assert!(
        default_path.is_ok(),
        "default_capcut_drafts_dir should resolve %LOCALAPPDATA% path"
    );
    let path = default_path.unwrap();
    assert!(
        path.to_string_lossy().contains("com.lveditor.draft"),
        "Default path must point to com.lveditor.draft directory: {:?}",
        path
    );

    let _ = std::fs::remove_dir_all(temp_custom);
}

#[test]
fn test_it_25_2_project_folder_and_draft_json_integrity() {
    let container = create_sample_container();
    let project = create_subtitle_project(
        container,
        "NeuroClip_Highlight_Epic_Stream",
        "vod_it25_2",
        Some("original_16_9"),
    );

    let temp_dir = create_temp_test_dir("it_25_2");

    // Pre-create mock root_meta_info.json in drafts_root to test indexing
    let root_meta_path = temp_dir.join("root_meta_info.json");
    let initial_root = RootMetaInfo::default();
    let root_json = serde_json::to_vec_pretty(&initial_root).unwrap();
    std::fs::write(&root_meta_path, root_json).unwrap();

    let result = export_capcut_draft_folder(
        &project,
        Some("C:\\Clips\\stream_highlight.mp4"),
        Some(&temp_dir),
    )
    .unwrap();

    // 1. Verify project directory name format: NeuroClip_Highlight_<timestamp>
    let project_dir = Path::new(&result.project_dir);
    assert!(project_dir.exists());
    let folder_name = project_dir.file_name().unwrap().to_string_lossy();
    assert!(
        folder_name.starts_with("NeuroClip_Highlight_"),
        "Folder name must start with NeuroClip_Highlight_: {folder_name}"
    );

    // 2. Verify draft_content.json readability and validity
    let content_path = Path::new(&result.draft_content_path);
    assert!(content_path.exists());
    let content: DraftContent =
        serde_json::from_str(&std::fs::read_to_string(content_path).unwrap()).unwrap();
    assert_eq!(content.name, "NeuroClip_Highlight_Epic_Stream");
    assert_eq!(content.canvas_config.width, 1920);
    assert_eq!(content.canvas_config.height, 1080);
    assert_eq!(content.canvas_config.ratio, "16:9");

    // 3. Verify draft_meta_info.json readability and draft_materials registration
    let meta_path = Path::new(&result.draft_meta_info_path);
    assert!(meta_path.exists());
    let meta: DraftMetaInfo =
        serde_json::from_str(&std::fs::read_to_string(meta_path).unwrap()).unwrap();
    assert_eq!(meta.draft_name, "NeuroClip_Highlight_Epic_Stream");
    assert_eq!(meta.draft_materials.draft_materials.len(), 1);
    assert_eq!(
        meta.draft_materials.draft_materials[0].file_path,
        "C:\\Clips\\stream_highlight.mp4"
    );
    assert_eq!(meta.draft_materials.draft_materials[0].metetype, "video");

    // 4. Verify root_meta_info.json was indexed
    let updated_root: RootMetaInfo =
        serde_json::from_str(&std::fs::read_to_string(&root_meta_path).unwrap()).unwrap();
    assert_eq!(updated_root.all_draft_store.len(), 1);
    assert_eq!(updated_root.all_draft_store[0].draft_id, meta.draft_id);

    // 5. Verify direct update_root_meta_info helper
    let extra_item = DraftStoreItem {
        draft_id: "test_draft_extra".to_string(),
        draft_name: "Extra Draft".to_string(),
        draft_fold_path: temp_dir.display().to_string(),
        draft_root_path: temp_dir.display().to_string(),
        tm_draft_create: 1_234_567,
        tm_draft_modify: 1_234_567,
        draft_type: 0,
    };
    update_root_meta_info(&temp_dir, &extra_item);
    let final_root: RootMetaInfo =
        serde_json::from_str(&std::fs::read_to_string(&root_meta_path).unwrap()).unwrap();
    assert_eq!(final_root.all_draft_store.len(), 2);

    // 6. Verify Dual Safeguard subtitle.ass aligns with original_16_9 (1920x1080)
    let ass_bytes = std::fs::read(&result.ass_backup_path).unwrap();
    assert!(ass_bytes.len() >= 3);
    assert_eq!(&ass_bytes[0..3], &[0xEF, 0xBB, 0xBF]);
    let ass_content = String::from_utf8_lossy(&ass_bytes[3..]);
    assert!(ass_content.contains("PlayResX: 1920"));
    assert!(ass_content.contains("PlayResY: 1080"));

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[test]
fn test_ed_25_1_and_ed_25_2_dual_export_safeguard_invariant() {
    let container = create_sample_container();
    let project = create_subtitle_project(
        container,
        "Safeguard_Invariant_Project",
        "vod_safeguard",
        Some("preset_two_stack_9_16"),
    );

    let temp_dir = create_temp_test_dir("dual_safeguard");

    let result = export_capcut_draft_folder(
        &project,
        None, // No video, test subtitle-only safeguard
        Some(&temp_dir),
    )
    .unwrap();

    let project_dir = Path::new(&result.project_dir);

    // ED-25.1 & ED-25.2 Invariant rule:
    // ALWAYS generate and write subtitle.ass and subtitle.srt into that project directory!
    let ass_path = project_dir.join("subtitle.ass");
    let srt_path = project_dir.join("subtitle.srt");

    assert!(
        ass_path.exists(),
        "subtitle.ass MUST exist in CapCut draft directory (ERR-09)"
    );
    assert!(
        srt_path.exists(),
        "subtitle.srt MUST exist in CapCut draft directory (ERR-09)"
    );
    assert_eq!(result.ass_backup_path, ass_path.display().to_string());
    assert_eq!(result.srt_backup_path, srt_path.display().to_string());
    assert_eq!(result.dual_safeguard_files.len(), 2);

    // Verify UTF-8 BOM bytes [0xEF, 0xBB, 0xBF]
    let ass_bytes = std::fs::read(&ass_path).unwrap();
    assert!(ass_bytes.len() >= 3);
    assert_eq!(
        &ass_bytes[0..3],
        &[0xEF, 0xBB, 0xBF],
        "subtitle.ass must start with UTF-8 BOM"
    );

    let srt_bytes = std::fs::read(&srt_path).unwrap();
    assert!(srt_bytes.len() >= 3);
    assert_eq!(
        &srt_bytes[0..3],
        &[0xEF, 0xBB, 0xBF],
        "subtitle.srt must start with UTF-8 BOM"
    );

    // Verify Vietnamese diacritics, 9:16 PlayRes, and canonical styles inside subtitle.ass
    let ass_content = String::from_utf8_lossy(&ass_bytes[3..]);
    assert!(ass_content.contains("PlayResX: 1080"));
    assert!(ass_content.contains("PlayResY: 1920"));
    assert!(ass_content.contains(
        "Style: Vedal,Arial,54,&H0098FB98,&H000000FF,&H00000000,&H80000000,-1,0,0,0,100,100,0,0,1,2.5,2,2,20,20,40,1"
    ));
    assert!(ass_content.contains(
        "Style: Chat,Arial,48,&H00EBCE87,&H000000FF,&H00000000,&H80000000,0,0,0,0,100,100,0,0,1,2,2,2,20,20,40,1"
    ));
    assert!(ass_content.contains("Chào chat! Mình rất thích tutel (trái tim)."));
    assert!(ass_content.contains("Vedal: Đừng nói như thế nữa."));
    assert!(ass_content.contains("Evil: Ta sẽ chiếm quyền buổi stream này."));

    let srt_content = String::from_utf8_lossy(&srt_bytes[3..]);
    assert!(srt_content.contains("Chào chat! Mình rất thích tutel (trái tim)."));
    assert!(srt_content.contains("Vedal: Đừng nói như thế nữa."));
    assert!(srt_content.contains("Evil: Ta sẽ chiếm quyền buổi stream này."));

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[test]
fn test_ed_25_3_performance_sla_under_200ms() {
    // Generate 300 subtitle segments to stress test serialization and file writing
    let mut segments = Vec::with_capacity(300);
    for i in 0..300 {
        #[allow(clippy::cast_precision_loss)]
        let start = i as f64 * 3.0;
        let end = start + 2.5;
        let speaker = match i % 3 {
            0 => "neuro",
            1 => "vedal",
            _ => "evil",
        };
        segments.push(TranscriptSegment {
            segment_id: format!("seg_{i:04}"),
            start_sec: start,
            end_sec: end,
            speaker_id: speaker.to_string(),
            text_en: format!("Segment {i} banter dialogue text in English."),
            text_vi: Some(format!(
                "Phân đoạn thoại số {i} với đầy đủ dấu tiếng Việt: ắ ề ộ đ."
            )),
            confidence: 0.95,
            cps_score: 12.0,
            is_flagged_for_review: false,
            words: vec![],
        });
    }

    let container = TranscriptSegmentsContainer {
        clip_id: "clip_perf_benchmark".to_string(),
        segments,
    };
    let project = create_subtitle_project(
        container,
        "Benchmark_300_Segments",
        "vod_benchmark",
        Some("preset_center_crop_9_16"),
    );

    let temp_dir = create_temp_test_dir("perf_benchmark");

    let timer = Instant::now();
    let result = export_capcut_draft_folder(
        &project,
        Some("C:\\media\\long_highlight.mp4"),
        Some(&temp_dir),
    )
    .unwrap();
    let total_elapsed_ms = timer.elapsed().as_millis() as u64;

    // AC-25.2 & ED-25.3 SLA threshold: duration <= 200ms
    assert!(
        result.duration_ms <= 200,
        "Internal measured duration {}ms must be <= 200ms",
        result.duration_ms
    );
    assert!(
        total_elapsed_ms <= 200,
        "Total elapsed duration {total_elapsed_ms}ms must be <= 200ms"
    );

    // Verify complete folder output
    assert!(Path::new(&result.draft_content_path).exists());
    assert!(Path::new(&result.draft_meta_info_path).exists());
    assert!(Path::new(&result.ass_backup_path).exists());
    assert!(Path::new(&result.srt_backup_path).exists());

    let _ = std::fs::remove_dir_all(temp_dir);
}
