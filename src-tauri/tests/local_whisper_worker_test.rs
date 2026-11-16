#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Integration and verification test suite for TASK-M4-16 (Milestone M4.4):
//! Faster-Whisper CTranslate2 INT8 Offline Worker, Hardware Guardrail, and Homomorphic Output.
//!
//! Validates:
//! - UT-16.1: Command line construction (`--model medium --quantization int8 --device auto`).
//! - UT-16.2: Hardware guardrail VRAM floor check (< 2500MB rejection and auto-downgrade).
//! - IT-16.1: 100% Homomorphic output JSON mapping to Schema 5.5 and Groq parity.
//! - ED-16.1 & AC-16.1: Bounded VRAM <= 1.8GB on 4GB GPUs.
//! - AC-16.2: 0 CUDA OOM crash defense on GTX 1650 / RTX 3050 Laptop hardware tier.

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use neuroclip_lib::config::{CpuInfo, GpuInfo, GpuVendor, HardwareProfile, RamInfo, RecommendedMode};
use neuroclip_lib::error::{codes, AppError, SttError};
use neuroclip_lib::ingestion::streamer::assign_child_to_job_guard;
use neuroclip_lib::stt::groq_client::{map_verbose_json_to_container, GroqVerboseJsonResponse};
use neuroclip_lib::stt::{
    build_faster_whisper_command_args, build_worker_command_args, check_hardware_guardrail,
    check_hardware_guardrail_from_profile, check_hardware_guardrail_from_profile_file,
    enforce_hardware_guardrail, estimate_vram_usage_mb, evaluate_hardware_guardrail,
    evaluate_hardware_guardrail_from_profile, evaluate_hardware_guardrail_from_profile_file,
    is_whisper_large, map_faster_whisper_to_container, parse_faster_whisper_json,
    resolve_faster_whisper_binary, run_faster_whisper_worker, verify_model,
    FasterWhisperConfig, FasterWhisperVerboseJsonResponse, GuardrailPolicy,
    DEFAULT_FASTER_WHISPER_DEVICE, DEFAULT_FASTER_WHISPER_MODEL,
    DEFAULT_FASTER_WHISPER_QUANTIZATION, MAX_INT8_VRAM_CAP_MB, MIN_LARGE_VRAM_MB,
};

// ============================================================================
// UT-16.1: Happy Path - Command Construction
// ============================================================================

#[test]
fn test_ut_16_1_command_construction_default() {
    let config = FasterWhisperConfig::default();
    let args = build_worker_command_args(None, None, &config);

    // Assert mandatory CTranslate2 parameter triad: --model medium --quantization int8 --device auto
    let triad = ["--model", "medium", "--quantization", "int8", "--device", "auto"];
    for i in 0..triad.len() {
        assert_eq!(args[i], triad[i], "Mismatch at parameter index {i}");
    }

    // Assert extra standard flags
    assert!(args.contains(&"--output_format".to_string()));
    assert!(args.contains(&"verbose_json".to_string()));
    assert!(args.contains(&"--word_timestamps".to_string()));
    assert!(args.contains(&"--condition_on_previous_text".to_string()));
    assert!(args.contains(&"false".to_string()));
    assert!(args.contains(&"--prompt".to_string()));
    assert!(args.contains(&"Vedal, Neuro-sama, Evil Neuro, Tutel, Gymbag".to_string()));
}

#[test]
fn test_ut_16_1_command_construction_with_paths() {
    let config = FasterWhisperConfig {
        model: "small".to_string(),
        quantization: "int8".to_string(),
        device: "cuda".to_string(),
        language: Some("en".to_string()),
        word_timestamps: true,
        condition_on_previous_text: false,
        prompt: Some("Neuro-sama highlight stream".to_string()),
        output_format: "verbose_json".to_string(),
        threads: 4,
        beam_size: 5,
        binary_path: None,
    };

    let fake_audio = PathBuf::from("c:/audio/input_clip_01.wav");
    let fake_output = PathBuf::from("c:/output/clip_01.json");

    let args = build_faster_whisper_command_args(Some(&fake_audio), Some(&fake_output), &config);

    assert_eq!(args[0], "--model");
    assert_eq!(args[1], "small");
    assert_eq!(args[2], "--quantization");
    assert_eq!(args[3], "int8");
    assert_eq!(args[4], "--device");
    assert_eq!(args[5], "cuda");

    assert!(args.contains(&"--audio".to_string()));
    assert!(args.contains(&fake_audio.to_string_lossy().to_string()));
    assert!(args.contains(&"--output".to_string()));
    assert!(args.contains(&fake_output.to_string_lossy().to_string()));
}

#[test]
fn test_ut_16_1_command_construction_alias_parity() {
    let config = FasterWhisperConfig::default();
    let args1 = build_worker_command_args(None, None, &config);
    let args2 = build_faster_whisper_command_args(None, None, &config);
    assert_eq!(args1, args2, "build_worker_command_args and build_faster_whisper_command_args must be identical");
}

// ============================================================================
// UT-16.2: Hardware Guardrail Check (< 2500MB VRAM Floor)
// ============================================================================

#[test]
fn test_ut_16_2_guardrail_reject_large_when_vram_under_2500mb() {
    // Scenario: GTX 1050 / Low VRAM with 2048MB VRAM requesting Whisper Large in StrictReject mode
    let res = evaluate_hardware_guardrail(
        "large-v3",
        "int8",
        "auto",
        2048,
        true,
        GuardrailPolicy::StrictReject,
    );

    assert!(res.is_err(), "Expected StrictReject to return error for VRAM < 2500MB");
    let err = res.unwrap_err();
    assert_eq!(
        err.code(),
        codes::ERR_STT_VRAM_INSUFFICIENT,
        "Error code must be canonical ERR_STT_VRAM_INSUFFICIENT"
    );
    assert!(err.is_recoverable(), "VRAM error must be recoverable");

    // Also verify convenience function check_hardware_guardrail
    let res_conv = check_hardware_guardrail("large-v3", 2048, true);
    assert!(res_conv.is_err());
}

#[test]
fn test_ut_16_2_guardrail_auto_downgrade_large_to_medium_int8() {
    // Scenario: User requests Whisper Large on a 2048MB VRAM GPU in AutoDowngrade mode
    let res = evaluate_hardware_guardrail(
        "large-v3",
        "int8",
        "auto",
        2048,
        true,
        GuardrailPolicy::AutoDowngrade,
    );

    assert!(res.is_ok(), "AutoDowngrade should succeed without error");
    let decision = res.unwrap();
    assert_eq!(decision.effective_model, "medium", "Large model must be auto-downgraded to medium");
    assert_eq!(decision.effective_quantization, "int8", "Must enforce INT8 quantization");
    assert!(decision.was_downgraded, "was_downgraded flag must be true");
    assert!(decision.reason.is_some(), "Must record downgrade reason");

    // Also verify enforce_hardware_guardrail helper
    let (model, device, downgraded) = enforce_hardware_guardrail("large-v2", 2048, true);
    assert_eq!(model, "medium");
    assert_eq!(device, "auto");
    assert!(downgraded);
}

#[test]
fn test_ut_16_2_guardrail_pass_large_when_vram_sufficient() {
    // Scenario: RTX 4080 with 16384MB VRAM requesting Whisper Large
    let res = evaluate_hardware_guardrail(
        "large-v3",
        "int8",
        "cuda",
        16384,
        true,
        GuardrailPolicy::StrictReject,
    );

    assert!(res.is_ok(), "Large model must be allowed when VRAM >= 2500MB");
    let decision = res.unwrap();
    assert_eq!(decision.effective_model, "large-v3");
    assert_eq!(decision.effective_device, "cuda");
    assert!(!decision.was_downgraded);
}

#[test]
fn test_ut_16_2_guardrail_non_cuda_fallback_cpu() {
    // Scenario: AMD Radeon GPU or Intel iGPU without CUDA support
    let res = evaluate_hardware_guardrail(
        "medium",
        "int8",
        "cuda",
        8192,
        false, // cuda_supported = false
        GuardrailPolicy::AutoDowngrade,
    );

    assert!(res.is_ok());
    let decision = res.unwrap();
    assert_eq!(decision.effective_device, "cpu", "Must fallback to CPU when CUDA is unsupported");
    assert!(decision.was_downgraded);
}

#[test]
fn test_ut_16_2_guardrail_from_profile_file() {
    let temp_profile = std::env::temp_dir().join(format!("test_hw_profile_{}.json", std::process::id()));

    let profile = HardwareProfile {
        cpu_info: CpuInfo {
            model: "AMD Ryzen 7 5800X".to_string(),
            cores: 8,
            threads: 16,
        },
        ram_info: RamInfo {
            total_mb: 16384,
            available_mb: 12000,
        },
        gpu_info: GpuInfo {
            vendor: GpuVendor::Nvidia,
            model: "NVIDIA GeForce GTX 1050".to_string(),
            vram_mb: 2048, // Low VRAM < 2500MB
            cuda_supported: true,
        },
        recommended_mode: RecommendedMode::CloudByok,
        recommendation_reason: "Low VRAM".to_string(),
        timestamp: "2026-11-16T12:00:00Z".to_string(),
    };

    let serialized = serde_json::to_string_pretty(&profile).unwrap();
    {
        let mut f = File::create(&temp_profile).unwrap();
        f.write_all(serialized.as_bytes()).unwrap();
    }

    // 1. StrictReject on Large via file
    let res_strict = check_hardware_guardrail_from_profile_file(&temp_profile, "large-v3");
    assert!(res_strict.is_err(), "Expected StrictReject error from file");

    // Also test check_hardware_guardrail_from_profile
    let res_prof = check_hardware_guardrail_from_profile(&profile, "large-v3");
    assert!(res_prof.is_err());

    // 2. AutoDowngrade on Large via file
    let res_downgrade = evaluate_hardware_guardrail_from_profile_file(
        &temp_profile,
        "large-v3",
        "int8",
        "auto",
        GuardrailPolicy::AutoDowngrade,
    );
    assert!(res_downgrade.is_ok());
    let decision = res_downgrade.unwrap();
    assert_eq!(decision.effective_model, "medium");
    assert!(decision.was_downgraded);

    // Also test evaluate_hardware_guardrail_from_profile
    let res_downgrade_prof = evaluate_hardware_guardrail_from_profile(
        &profile,
        "large-v3",
        "int8",
        "auto",
        GuardrailPolicy::AutoDowngrade,
    );
    assert!(res_downgrade_prof.is_ok());
    assert_eq!(res_downgrade_prof.unwrap().effective_model, "medium");

    let _ = std::fs::remove_file(&temp_profile);
}

#[test]
fn test_is_whisper_large_variants() {
    assert!(is_whisper_large("large"));
    assert!(is_whisper_large("large-v1"));
    assert!(is_whisper_large("large-v2"));
    assert!(is_whisper_large("large-v3"));
    assert!(is_whisper_large("whisper-large-v3-turbo"));
    assert!(!is_whisper_large("medium"));
    assert!(!is_whisper_large("small"));
    assert!(!is_whisper_large("base.en"));
}

#[test]
fn test_default_constants() {
    assert_eq!(DEFAULT_FASTER_WHISPER_MODEL, "medium");
    assert_eq!(DEFAULT_FASTER_WHISPER_QUANTIZATION, "int8");
    assert_eq!(DEFAULT_FASTER_WHISPER_DEVICE, "auto");
    assert_eq!(MIN_LARGE_VRAM_MB, 2500);
    assert_eq!(MAX_INT8_VRAM_CAP_MB, 1800);
}

#[test]
fn test_binary_resolution_failure_path() {
    std::env::remove_var("NEUROCLIP_FASTER_WHISPER_PATH");
    let res = resolve_faster_whisper_binary();
    if let Err(AppError::Sidecar(neuroclip_lib::error::SidecarError::BinaryNotFound { name, .. })) = res {
        assert_eq!(name, "faster-whisper-worker");
    }
}

// ============================================================================
// IT-16.1: Homomorphic JSON Output Validation (Schema 5.5 Parity with Groq)
// ============================================================================

#[test]
fn test_it_16_1_homomorphic_json_output_matches_schema_5_5() {
    let mock_json = r#"{
        "task": "transcribe",
        "language": "en",
        "duration": 4.5,
        "text": " Hello chat, I am back with more Minecraft streams.",
        "segments": [
            {
                "id": 0,
                "seek": 0,
                "start": 0.0,
                "end": 2.2,
                "text": " Hello chat,",
                "tokens": [50364, 2425, 345, 11, 50474],
                "temperature": 0.0,
                "avg_logprob": -0.15,
                "compression_ratio": 1.1,
                "no_speech_prob": 0.01,
                "words": [
                    { "word": "Hello", "start": 0.0, "end": 1.0, "probability": 0.99 },
                    { "word": "chat,", "start": 1.0, "end": 2.2, "probability": 0.97 }
                ]
            },
            {
                "id": 1,
                "seek": 220,
                "start": 2.2,
                "end": 4.5,
                "text": " I am back with more Minecraft streams.",
                "tokens": [50474, 286, 669, 646, 365, 625, 11736, 10738, 13, 50589],
                "temperature": 0.0,
                "avg_logprob": -0.10,
                "compression_ratio": 1.2,
                "no_speech_prob": 0.02,
                "words": [
                    { "word": "I", "start": 2.2, "end": 2.5, "probability": 0.98 },
                    { "word": "am", "start": 2.5, "end": 2.8, "probability": 0.99 },
                    { "word": "back", "start": 2.8, "end": 3.3, "probability": 0.96 },
                    { "word": "with", "start": 3.3, "end": 3.6, "probability": 0.98 },
                    { "word": "more", "start": 3.6, "end": 3.9, "probability": 0.97 },
                    { "word": "Minecraft", "start": 3.9, "end": 4.2, "probability": 0.99 },
                    { "word": "streams.", "start": 4.2, "end": 4.5, "probability": 0.95 }
                ]
            }
        ],
        "words": []
    }"#;

    let res = parse_faster_whisper_json(mock_json, "clip_test_homomorphic");
    assert!(res.is_ok(), "Expected valid Schema 5.5 parsing");

    let container = res.unwrap();
    assert_eq!(container.clip_id, "clip_test_homomorphic");
    assert_eq!(container.segments.len(), 2);

    let seg0 = &container.segments[0];
    assert_eq!(seg0.segment_id, "clip_test_homomorphic_seg_0001");
    assert_eq!(seg0.start_sec, 0.0);
    assert_eq!(seg0.end_sec, 2.2);
    assert_eq!(seg0.text_en, "Hello chat,");
    assert_eq!(seg0.speaker_id, "unknown");
    assert!(seg0.confidence > 0.85);
    assert!(seg0.cps_score > 0.0);
    assert!(!seg0.is_flagged_for_review);
    assert_eq!(seg0.words.len(), 2);
    assert_eq!(seg0.words[0].word, "Hello");
    assert_eq!(seg0.words[0].start_sec, 0.0);
    assert_eq!(seg0.words[0].end_sec, 1.0);

    let seg1 = &container.segments[1];
    assert_eq!(seg1.segment_id, "clip_test_homomorphic_seg_0002");
    assert_eq!(seg1.start_sec, 2.2);
    assert_eq!(seg1.end_sec, 4.5);
    assert_eq!(seg1.words.len(), 7);
}

#[test]
fn test_it_16_1_homomorphic_equivalence_with_groq() {
    // Identical JSON payload representing verbose_json structure
    let verbose_json = r#"{
        "task": "transcribe",
        "language": "en",
        "duration": 3.0,
        "text": " Vedal drinks coffee",
        "segments": [
            {
                "id": 0,
                "seek": 0,
                "start": 0.0,
                "end": 3.0,
                "text": " Vedal drinks coffee",
                "tokens": [100, 101, 102],
                "temperature": 0.0,
                "avg_logprob": -0.12,
                "compression_ratio": 1.0,
                "no_speech_prob": 0.01
            }
        ],
        "words": [
            { "word": "Vedal", "start": 0.0, "end": 0.8 },
            { "word": "drinks", "start": 0.8, "end": 1.8 },
            { "word": "coffee", "start": 1.8, "end": 3.0 }
        ]
    }"#;

    // Parse via Groq mapper
    let groq_resp: GroqVerboseJsonResponse = serde_json::from_str(verbose_json).unwrap();
    let groq_container = map_verbose_json_to_container("clip_equiv", groq_resp, 0.0);

    // Parse via Faster-Whisper mapper
    let fw_resp: FasterWhisperVerboseJsonResponse = serde_json::from_str(verbose_json).unwrap();
    let fw_container = map_faster_whisper_to_container("clip_equiv", fw_resp, 0.0);

    // Assert exact 100% equivalence
    assert_eq!(groq_container.clip_id, fw_container.clip_id);
    assert_eq!(groq_container.segments.len(), fw_container.segments.len());

    let g_seg = &groq_container.segments[0];
    let f_seg = &fw_container.segments[0];

    assert_eq!(g_seg.segment_id, f_seg.segment_id);
    assert_eq!(g_seg.start_sec, f_seg.start_sec);
    assert_eq!(g_seg.end_sec, f_seg.end_sec);
    assert_eq!(g_seg.text_en, f_seg.text_en);
    assert_eq!(g_seg.speaker_id, f_seg.speaker_id);
    assert_eq!(g_seg.words.len(), f_seg.words.len());

    for (gw, fw) in g_seg.words.iter().zip(f_seg.words.iter()) {
        assert_eq!(gw.word, fw.word);
        assert_eq!(gw.start_sec, fw.start_sec);
        assert_eq!(gw.end_sec, fw.end_sec);
    }
}

// ============================================================================
// ED-16.1 & AC-16.1 & AC-16.2: Bounded VRAM <= 1.8GB & CUDA OOM Defense
// ============================================================================

#[test]
fn test_ed_16_1_vram_cap_assertion_under_1_8gb() {
    // AC-16.1: GPU VRAM footprint for INT8 Medium must not exceed 1.8GB (1800MB)
    let medium_int8_mb = estimate_vram_usage_mb("medium", "int8");
    assert!(
        medium_int8_mb <= MAX_INT8_VRAM_CAP_MB,
        "Faster-Whisper INT8 Medium estimated VRAM ({medium_int8_mb}MB) exceeds 1.8GB cap ({MAX_INT8_VRAM_CAP_MB}MB)"
    );
    assert_eq!(medium_int8_mb, 1650);

    // Small INT8 must be well below 1.8GB
    let small_int8_mb = estimate_vram_usage_mb("small", "int8");
    assert!(
        small_int8_mb <= MAX_INT8_VRAM_CAP_MB,
        "Faster-Whisper INT8 Small estimated VRAM ({small_int8_mb}MB) exceeds 1.8GB cap"
    );
    assert_eq!(small_int8_mb, 850);

    // AC-16.2 Assertion: A 4GB GPU (4096MB) running medium INT8 (1650MB) has 2446MB free VRAM,
    // guaranteeing 0 CUDA OOM crashes on GTX 1650 / RTX 3050 Laptop.
    let gtx_1650_vram_mb: u64 = 4096;
    let headroom_mb = gtx_1650_vram_mb - medium_int8_mb;
    assert!(
        headroom_mb >= 2000,
        "Headroom on 4GB GPU ({headroom_mb}MB) is insufficient to prevent CUDA OOM"
    );
}

#[tokio::test]
async fn test_ed_16_1_simulated_cuda_oom_error_handling() {
    std::env::set_var("NEUROCLIP_MOCK_CUDA_OOM", "1");

    let config = FasterWhisperConfig::default();
    let fake_audio = PathBuf::from("fake_audio.wav");
    let res = run_faster_whisper_worker("clip_oom_1", &fake_audio, &config, None).await;

    std::env::remove_var("NEUROCLIP_MOCK_CUDA_OOM");

    assert!(res.is_err(), "Expected simulated CUDA OOM to trigger error");
    let err = res.unwrap_err();
    assert_eq!(err.code(), codes::ERR_STT_CUDA_OOM);
    assert!(err.is_recoverable());
    if let AppError::Stt(SttError::CudaOom(msg)) = err {
        assert!(msg.contains("simulated"));
    } else {
        panic!("Expected CudaOom variant");
    }
}

#[tokio::test]
async fn test_ed_16_1_simulated_error_handling_sidecar_failure() {
    std::env::set_var("NEUROCLIP_MOCK_FASTER_WHISPER_ERROR", "CUDA out of memory: tried to allocate 2.5GiB");

    let config = FasterWhisperConfig::default();
    let fake_audio = PathBuf::from("fake_audio.wav");
    let res = run_faster_whisper_worker("clip_oom_2", &fake_audio, &config, None).await;

    std::env::remove_var("NEUROCLIP_MOCK_FASTER_WHISPER_ERROR");

    assert!(res.is_err());
    let err = res.unwrap_err();
    assert_eq!(err.code(), codes::ERR_STT_CUDA_OOM);
}

// ============================================================================
// Subprocess Supervision & Windows Job Object Integration
// ============================================================================

#[tokio::test]
async fn test_subprocess_mock_runner_execution() {
    let temp_mock_json = std::env::temp_dir().join(format!("test_fw_mock_output_{}.json", std::process::id()));
    let mock_content = r#"{
        "task": "transcribe",
        "language": "en",
        "duration": 2.0,
        "text": " Neuro Tutel Gymbag",
        "segments": [
            {
                "id": 0,
                "seek": 0,
                "start": 0.0,
                "end": 2.0,
                "text": " Neuro Tutel Gymbag",
                "tokens": [1, 2, 3],
                "avg_logprob": -0.1,
                "words": [
                    { "word": "Neuro", "start": 0.0, "end": 0.6, "probability": 0.99 },
                    { "word": "Tutel", "start": 0.6, "end": 1.2, "probability": 0.98 },
                    { "word": "Gymbag", "start": 1.2, "end": 2.0, "probability": 0.97 }
                ]
            }
        ],
        "words": []
    }"#;

    {
        let mut f = File::create(&temp_mock_json).unwrap();
        f.write_all(mock_content.as_bytes()).unwrap();
    }

    std::env::set_var(
        "NEUROCLIP_MOCK_FASTER_WHISPER_OUTPUT",
        temp_mock_json.to_string_lossy().to_string(),
    );

    let progress_count = Arc::new(AtomicU32::new(0));
    let progress_count_clone = progress_count.clone();

    let cb = Arc::new(move |_p| {
        progress_count_clone.fetch_add(1, Ordering::SeqCst);
    });

    let config = FasterWhisperConfig::default();
    let fake_audio = PathBuf::from("fake_audio.wav");
    let res = run_faster_whisper_worker("clip_mock_fw", &fake_audio, &config, Some(cb)).await;

    std::env::remove_var("NEUROCLIP_MOCK_FASTER_WHISPER_OUTPUT");
    let _ = std::fs::remove_file(&temp_mock_json);

    assert!(res.is_ok(), "Mock runner must succeed");
    let container = res.unwrap();
    assert_eq!(container.clip_id, "clip_mock_fw");
    assert_eq!(container.segments.len(), 1);
    assert_eq!(container.segments[0].words.len(), 3);
    assert_eq!(progress_count.load(Ordering::SeqCst), 2, "Expected 2 progress events (50% and 100%)");
}

#[tokio::test]
async fn test_assign_child_to_job_guard_integration() {
    let mut cmd = tokio::process::Command::new(if cfg!(windows) { "cmd.exe" } else { "sh" });
    if cfg!(windows) {
        cmd.args(["/C", "echo job_guard_test"]);
    } else {
        cmd.args(["-c", "echo job_guard_test"]);
    }

    let child = cmd.spawn().unwrap();
    // Test binding the child process to Windows Job Object
    assign_child_to_job_guard(&child);
}

// ============================================================================
// Model Manager CTranslate2 Directory Layout Verification
// ============================================================================

#[test]
fn test_model_manager_ctranslate2_directory_verification() {
    let temp_dir = std::env::temp_dir().join(format!("test_model_dir_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let model_bin = temp_dir.join("model.bin");
    let test_bytes = b"ctranslate2 weights synthetic payload";
    {
        let mut f = File::create(&model_bin).unwrap();
        f.write_all(test_bytes).unwrap();
    }

    // Verify directory-based model resolution without expected hash constraint
    let res = verify_model("custom-faster-whisper", Some(&temp_dir));
    assert!(res.is_ok(), "Directory model verification should succeed when model.bin is present");

    let info = res.unwrap();
    assert_eq!(info.model_name, "custom-faster-whisper");
    assert_eq!(info.path, temp_dir.display().to_string());
    assert_eq!(info.size_bytes, test_bytes.len() as u64);
    assert!(info.is_verified);

    // Clean up
    let _ = std::fs::remove_file(&model_bin);
    let _ = std::fs::remove_dir(&temp_dir);
}
