#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::cast_precision_loss,
    clippy::cast_lossless
)]

//! End-to-End Synthetic Pipeline Harness (`TASK-M4-30`, Milestone M4.9).
//!
//! Verifies 100% offline execution across all 6 stages without external network calls:
//! 1. Synthetic fixtures (`synthetic_audio.wav` 3 min, `synthetic_chat.json` 10,000 events) & Hardware Probe
//! 2. Multimodal Highlight Detection (`t = 60s` KEKW burst with 4.5s latency compensation)
//! 3. Range Slice + Screaming Defense (`<= -1.5 dBFS`) + Pure Rust Silero VAD (`tract-onnx`)
//! 4. Mock Groq STT + Mock Gemini Vietsub Translation (`BLOCK_NONE`, Lore Dict, Hybrid Diarization)
//! 5. Vietnamese Smart Compound Line Wrapping + CPS Time Borrowing Optimization
//! 6. CapCut Draft JSON + Dual Export Safeguard (`.ass` & `.srt` UTF-8 BOM) & Premiere FCP7 XML parity
//!
//! Asserts AC-30.1 (<= 45s total E2E runtime) and AC-30.2 (6/6 Data Contracts valid).

use std::path::{Path, PathBuf};

use neuroclip_lib::export::{
    export_premiere_xml_file, load_subtitle_project_json, DraftContent, DraftMetaInfo,
    PremiereXmlExportOptions, UTF8_BOM_BYTES,
};
use neuroclip_lib::highlight::HighlightClustersContainer;
use neuroclip_lib::ingestion::ChatEventsContainer;
use neuroclip_lib::stt::audio_prep::PreparedAudioBuffer;
use neuroclip_lib::stt::transcript_parser::TranscriptSegmentsContainer;
use neuroclip_lib::translation::{
    detect_forbidden_translations, DEFAULT_MAX_CHARS_PER_LINE, VERTICAL_9_16_MAX_CHARS_PER_LINE,
};
use neuroclip_lib::{
    run_synthetic_e2e_pipeline_internal, validate_schema_5_1_hardware_profile,
    validate_schema_5_2_vod_manifest, validate_schema_5_3_chat_events,
    validate_schema_5_4_highlight_clusters, validate_schema_5_5_transcript_segments,
    validate_schema_5_6_subtitle_project, SyntheticE2EOptions,
};

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(label: &str) -> Self {
        let nanos = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let path = std::env::temp_dir().join(format!(
            "neuroclip_e2e_test_{label}_{}_{nanos}",
            std::process::id()
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

fn fixture_paths() -> (PathBuf, PathBuf) {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixtures_dir = manifest_dir.join("tests").join("fixtures");
    (
        fixtures_dir.join("synthetic_audio.wav"),
        fixtures_dir.join("synthetic_chat.json"),
    )
}

#[test]
fn test_e2e_fixtures_integrity_and_specifications() {
    let (audio_path, chat_path) = fixture_paths();
    assert!(
        audio_path.exists(),
        "Missing fixture: {}",
        audio_path.display()
    );
    assert!(
        chat_path.exists(),
        "Missing fixture: {}",
        chat_path.display()
    );

    // 1. Verify synthetic_audio.wav: 3 min (180s), 16kHz mono PCM, contains 0 dBFS peak & silence
    let wav_bytes = std::fs::read(&audio_path).unwrap();
    let audio_buf = PreparedAudioBuffer::from_wav_bytes(&wav_bytes).unwrap();
    assert_eq!(audio_buf.sample_rate, 16_000);
    assert!(
        (audio_buf.duration_sec - 180.0).abs() <= 0.05,
        "Expected 180.0s audio, got {:.3}s",
        audio_buf.duration_sec
    );
    assert!(
        audio_buf.peak_dbfs > -1.5,
        "Fixture must include >= -1.5 dBFS scream/laugh peak to exercise ERR-08 defense (peak={:.2} dBFS)",
        audio_buf.peak_dbfs
    );
    let silent_samples = audio_buf.samples_i16.iter().filter(|&&s| s == 0).count();
    assert!(
        silent_samples >= 16_000 * 60,
        "Expected >= 60s of digital silence in 3-min fixture"
    );

    // 2. Verify synthetic_chat.json: 10,000 events, Schema 5.3 valid, KEKW spike around t = 60.0s
    let chat_bytes = std::fs::read(&chat_path).unwrap();
    let chat_val: serde_json::Value = serde_json::from_slice(&chat_bytes).unwrap();
    validate_schema_5_3_chat_events(&chat_val).expect("Chat fixture must conform to Schema 5.3");

    let chat_container: ChatEventsContainer = serde_json::from_value(chat_val).unwrap();
    assert_eq!(chat_container.total_messages, 10_000);
    assert_eq!(chat_container.events.len(), 10_000);

    let kekw_burst_count = chat_container
        .events
        .iter()
        .filter(|e| {
            e.timestamp_sec >= 60.0
                && e.timestamp_sec < 65.0
                && e.message.contains("KEKW")
                && e.is_swarm_emote
        })
        .count();
    assert!(
        kekw_burst_count >= 3_000,
        "Expected >= 3,000 KEKW Swarm messages in [60.0s, 65.0s), found {kekw_burst_count}"
    );
}

#[tokio::test]
async fn test_e2e_synthetic_vod_pipeline_full_flow() {
    let (audio_path, chat_path) = fixture_paths();
    let test_dir = TestDir::new("full_flow");

    let options = SyntheticE2EOptions {
        audio_fixture_path: audio_path.to_string_lossy().to_string(),
        chat_fixture_path: chat_path.to_string_lossy().to_string(),
        output_dir: Some(test_dir.path().to_string_lossy().to_string()),
        vod_id: Some("synthetic_vod_e2e_001".to_string()),
        target_layout: Some("original_16_9".to_string()),
    };

    let report = run_synthetic_e2e_pipeline_internal(options)
        .await
        .expect("E2E synthetic pipeline must complete cleanly");

    println!(
        "[TASK-M4-30 E2E] passed={} total_ms={} peak={:.1}s bounds=[{:.1}s, {:.1}s] vad_segs={} reduction={:.1}% cps_opt={}",
        report.passed,
        report.total_duration_ms,
        report.detected_peak_sec,
        report.cluster_start_sec,
        report.cluster_end_sec,
        report.vad_speech_segments,
        report.vad_reduction_ratio * 100.0,
        report.cps_optimized_count
    );
    for stage in &report.stages {
        println!(
            "  - Stage {}: passed={} ({}ms) -> {}",
            stage.stage, stage.passed, stage.duration_ms, stage.details
        );
    }

    // AC-30.1: Entire E2E cycle <= 45 seconds
    assert!(
        report.total_duration_ms <= 45_000,
        "AC-30.1 Violated: E2E took {}ms (> 45,000ms limit)",
        report.total_duration_ms
    );
    assert!(report.passed, "E2E report must pass overall");
    assert_eq!(report.stages.len(), 6);
    assert!(report.stages.iter().all(|s| s.passed));

    // AC-30.2: All 6 Data Contracts pass both inside report and via independent reload
    assert!(
        report.schemas.all_passed(),
        "AC-30.2 Violated: schema validation flags = {:?}",
        report.schemas
    );

    let ws = PathBuf::from(&report.workspace_dir);
    type SchemaValidatorFn = fn(&serde_json::Value) -> Result<(), neuroclip_lib::AppError>;
    let schema_checks: [(&str, SchemaValidatorFn); 6] = [
        (
            "hardware_profile.json",
            validate_schema_5_1_hardware_profile,
        ),
        ("vod_manifest.json", validate_schema_5_2_vod_manifest),
        ("chat_events.json", validate_schema_5_3_chat_events),
        (
            "highlight_clusters.json",
            validate_schema_5_4_highlight_clusters,
        ),
        (
            "transcript_segments.json",
            validate_schema_5_5_transcript_segments,
        ),
        (
            "subtitle_project.json",
            validate_schema_5_6_subtitle_project,
        ),
    ];
    for (filename, validator) in schema_checks {
        let file_path = ws.join(filename);
        assert!(
            file_path.exists(),
            "Expected schema output file missing: {}",
            file_path.display()
        );
        let raw_val: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&file_path).unwrap()).unwrap();
        validator(&raw_val).unwrap_or_else(|e| panic!("Schema check failed for {filename}: {e}"));
    }

    // Highlight detection invariants: peak at t=60s + 4.5s latency compensation
    let clusters_doc: HighlightClustersContainer =
        serde_json::from_slice(&std::fs::read(ws.join("highlight_clusters.json")).unwrap())
            .unwrap();
    let top_cluster = clusters_doc
        .clusters
        .first()
        .expect("Top cluster must exist");
    assert!(
        (top_cluster.peak_sec - 60.0).abs() <= 2.5,
        "Expected peak at ~60.0s, got {:.2}s",
        top_cluster.peak_sec
    );
    assert!(
        top_cluster.start_sec <= 55.5 && top_cluster.end_sec >= 60.0,
        "Cluster bounds [{:.1}s, {:.1}s] must compensate 4.5s stream delay before t=60s",
        top_cluster.start_sec,
        top_cluster.end_sec
    );
    assert!(
        top_cluster.score >= 80.0,
        "Expected high multimodal score >= 80.0, got {:.2}",
        top_cluster.score
    );
    assert!(
        top_cluster.top_emotes.iter().any(|e| e == "KEKW"),
        "Top emotes {:?} must include KEKW",
        top_cluster.top_emotes
    );

    // Silero VAD invariants
    assert!(
        report.vad_speech_segments >= 2,
        "Expected >= 2 speech segments from Silero VAD, got {}",
        report.vad_speech_segments
    );
    assert!(
        report.vad_reduction_ratio >= 0.25,
        "Expected >= 25% silence pruning reduction, got {:.2}%",
        report.vad_reduction_ratio * 100.0
    );

    // Translation, Lore Dictionary, Smart Wrapping & CPS Time Borrowing invariants
    let transcript_doc: TranscriptSegmentsContainer =
        serde_json::from_slice(&std::fs::read(ws.join("transcript_segments.json")).unwrap())
            .unwrap();
    assert_eq!(transcript_doc.segments.len(), 6);
    assert!(
        report.cps_optimized_count >= 1,
        "Expected >= 1 segment optimized by CPS Time Borrowing"
    );

    let combined_vi: String = transcript_doc
        .segments
        .iter()
        .filter_map(|s| s.text_vi.as_deref())
        .collect::<Vec<_>>()
        .join("\n");
    for expected_lore in ["Cụ Rùa", "bao tải gym", "The Swarm", "Thả tim <3"] {
        assert!(
            combined_vi.contains(expected_lore),
            "Translated Vietsub must preserve Swarm lore term '{expected_lore}'"
        );
    }
    for seg in &transcript_doc.segments {
        let vi = seg
            .text_vi
            .as_deref()
            .expect("All synthetic segments must have text_vi");
        assert!(
            detect_forbidden_translations(vi).is_empty(),
            "Forbidden literal translation found in {}",
            seg.segment_id
        );
        for line in vi.lines() {
            assert!(
                line.chars().count() <= DEFAULT_MAX_CHARS_PER_LINE,
                "Line exceeded {DEFAULT_MAX_CHARS_PER_LINE} chars: '{line}'"
            );
        }
    }
    assert!(
        combined_vi.contains("bánh mì") && combined_vi.contains("bố Vedal"),
        "Compound words 'bánh mì' and 'bố Vedal' must remain intact after line wrapping"
    );

    // CapCut Draft JSON + Dual Export Safeguard (.ass & .srt with UTF-8 BOM + PO Option 1 colors)
    let draft_dir = PathBuf::from(&report.capcut_draft_dir);
    assert!(draft_dir.exists());
    let content_path = draft_dir.join("draft_content.json");
    let meta_path = draft_dir.join("draft_meta_info.json");
    let ass_path = draft_dir.join("subtitle.ass");
    let srt_path = draft_dir.join("subtitle.srt");
    assert!(content_path.exists());
    assert!(meta_path.exists());
    assert!(ass_path.exists());
    assert!(srt_path.exists());
    assert_eq!(report.dual_export_files.len(), 2);

    let draft_content: DraftContent =
        serde_json::from_slice(&std::fs::read(&content_path).unwrap()).unwrap();
    let draft_meta: DraftMetaInfo =
        serde_json::from_slice(&std::fs::read(&meta_path).unwrap()).unwrap();
    assert_eq!(draft_meta.draft_id, draft_content.id);
    assert_eq!(draft_content.tracks.len(), 3);
    assert_eq!(draft_content.canvas_config.width, 1920);
    assert_eq!(draft_content.canvas_config.height, 1080);
    assert_eq!(draft_content.materials.texts.len(), 6);

    let ass_bytes = std::fs::read(&ass_path).unwrap();
    assert_eq!(&ass_bytes[0..3], UTF8_BOM_BYTES);
    let ass_str = String::from_utf8_lossy(&ass_bytes[3..]);
    assert!(ass_str.contains("&H00C1B6FF,")); // Neuro #FFB6C1
    assert!(ass_str.contains("&H0098FB98,")); // Vedal #98FB98
    assert!(ass_str.contains("&H005500FF,")); // Evil #FF0055
    assert!(ass_str.contains("Cụ Rùa"));

    let srt_bytes = std::fs::read(&srt_path).unwrap();
    assert_eq!(&srt_bytes[0..3], UTF8_BOM_BYTES);
    let srt_str = String::from_utf8_lossy(&srt_bytes[3..]);
    assert!(srt_str.contains("The Swarm"));
}

#[test]
fn test_e2e_all_six_data_contracts_reject_malformed_payloads() {
    assert!(validate_schema_5_1_hardware_profile(&serde_json::json!({})).is_err());
    assert!(validate_schema_5_2_vod_manifest(&serde_json::json!({
        "vod_id": "v1",
        "source_type": "bad_source",
        "title": "Title",
        "duration_seconds": 10.0,
        "audio_path": "a.wav",
        "created_at": "2026-11-24T00:00:00Z"
    }))
    .is_err());
    assert!(validate_schema_5_3_chat_events(&serde_json::json!({
        "vod_id": "v1",
        "total_messages": 5,
        "events": []
    }))
    .is_err());
    assert!(validate_schema_5_4_highlight_clusters(&serde_json::json!({
        "vod_id": "v1",
        "clusters": [{
            "cluster_id": "c1",
            "start_sec": 20.0,
            "end_sec": 10.0,
            "peak_sec": 15.0,
            "score": 80.0,
            "primary_reason": "test"
        }]
    }))
    .is_err());
    assert!(validate_schema_5_5_transcript_segments(&serde_json::json!({
        "clip_id": "clip_1",
        "segments": [{
            "segment_id": "s1",
            "start_sec": 0.0,
            "end_sec": 1.0,
            "speaker_id": "unknown_bot_role",
            "text_en": "hi",
            "confidence": 0.9,
            "cps_score": 2.0
        }]
    }))
    .is_err());
    assert!(validate_schema_5_6_subtitle_project(&serde_json::json!({
        "project_id": "p1",
        "project_name": "name",
        "vod_id": "v1",
        "target_layout": "4_3_invalid",
        "segments": [],
        "updated_at": "invalid-date"
    }))
    .is_err());
}

#[tokio::test]
async fn test_e2e_nle_interchange_capcut_and_premiere_xml_parity() {
    let (audio_path, chat_path) = fixture_paths();
    let test_dir = TestDir::new("nle_parity_916");

    let report = run_synthetic_e2e_pipeline_internal(SyntheticE2EOptions {
        audio_fixture_path: audio_path.to_string_lossy().to_string(),
        chat_fixture_path: chat_path.to_string_lossy().to_string(),
        output_dir: Some(test_dir.path().to_string_lossy().to_string()),
        vod_id: Some("synthetic_vod_e2e_916".to_string()),
        target_layout: Some("preset_center_crop_9_16".to_string()),
    })
    .await
    .expect("9:16 vertical E2E pipeline must succeed");

    assert!(report.passed);
    assert!(report.schemas.all_passed());

    let ws = PathBuf::from(&report.workspace_dir);
    let project = load_subtitle_project_json(&ws.join("subtitle_project.json")).unwrap();
    assert_eq!(project.target_layout, "preset_center_crop_9_16");

    for seg in &project.segments {
        for line in seg.text_vi.as_deref().unwrap_or("").lines() {
            assert!(
                line.chars().count() <= VERTICAL_9_16_MAX_CHARS_PER_LINE,
                "Vertical 9:16 line exceeded {VERTICAL_9_16_MAX_CHARS_PER_LINE} chars: '{line}'"
            );
        }
    }

    let capcut_content: DraftContent = serde_json::from_slice(
        &std::fs::read(PathBuf::from(&report.capcut_draft_dir).join("draft_content.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(capcut_content.canvas_config.width, 1080);
    assert_eq!(capcut_content.canvas_config.height, 1920);

    // Verify Premiere FCP7 XML parity on the same SubtitleProject
    let xml_out = ws.join("project_timeline.xml");
    let xml_res = export_premiere_xml_file(
        &project,
        &xml_out,
        Some(&PremiereXmlExportOptions {
            video_file_path: project.video_file_path.clone(),
            fps: Some(60),
            clips: None,
        }),
    )
    .expect("Premiere XML export from E2E project must succeed");

    assert!(xml_out.exists());
    assert_eq!(xml_res.width, 1080);
    assert_eq!(xml_res.height, 1920);
    let xml_content = std::fs::read_to_string(&xml_out).unwrap();
    assert!(xml_content.contains("<xmeml version=\"4\">"));
    assert!(xml_content.contains("<width>1080</width>"));
    assert!(xml_content.contains("<height>1920</height>"));
    assert!(xml_content.contains("Cụ Rùa"));
}
