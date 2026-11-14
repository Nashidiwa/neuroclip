#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Tests for Whisper.cpp sidecar process supervisor and lifecycle management (TASK-M4-08).

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use neuroclip_lib::ingestion::streamer::assign_child_to_job_guard;
use neuroclip_lib::stt::{
    run_whisper_transcription, WhisperSidecarConfig,
};

#[tokio::test]
async fn test_whisper_mock_runner_execution() {
    let temp_mock_json = std::env::temp_dir().join("test_whisper_mock_output.json");
    let mock_content = r#"{
        "transcription": [
            {
                "timestamps": { "from": "00:00:00,000", "to": "00:00:02,500" },
                "offsets": { "from": 0, "to": 2500 },
                "text": " Hi chat I am back",
                "tokens": [
                    {
                        "text": " Hi",
                        "offsets": { "from": 0, "to": 500 },
                        "p": 0.99
                    },
                    {
                        "text": " chat",
                        "offsets": { "from": 500, "to": 1200 },
                        "p": 0.97
                    },
                    {
                        "text": " I",
                        "offsets": { "from": 1200, "to": 1500 },
                        "p": 0.98
                    },
                    {
                        "text": " am",
                        "offsets": { "from": 1500, "to": 1800 },
                        "p": 0.98
                    },
                    {
                        "text": " back",
                        "offsets": { "from": 1800, "to": 2500 },
                        "p": 0.96
                    }
                ]
            }
        ]
    }"#;

    {
        let mut f = File::create(&temp_mock_json).unwrap();
        f.write_all(mock_content.as_bytes()).unwrap();
    }

    // Set mock env var
    std::env::set_var("NEUROCLIP_MOCK_WHISPER_OUTPUT", temp_mock_json.to_string_lossy().to_string());

    let config = WhisperSidecarConfig {
        binary_path: None,
        model_path: PathBuf::from("mock_model.bin"),
        threads: 2,
        language: "en".to_string(),
        prompt: None,
    };

    let fake_audio = PathBuf::from("fake_audio.wav");
    let res = run_whisper_transcription("clip_mock_99", &fake_audio, &config, None).await;

    // Clean up env var
    std::env::remove_var("NEUROCLIP_MOCK_WHISPER_OUTPUT");
    let _ = std::fs::remove_file(&temp_mock_json);

    assert!(res.is_ok(), "Expected mock runner to succeed");
    let container = res.unwrap();
    assert_eq!(container.clip_id, "clip_mock_99");
    assert_eq!(container.segments.len(), 1);
    assert_eq!(container.segments[0].text_en, "Hi chat I am back");
    assert_eq!(container.segments[0].words.len(), 5);
}

#[tokio::test]
async fn test_whisper_mock_error_triggers_sidecar_execution_failed() {
    std::env::set_var("NEUROCLIP_MOCK_WHISPER_ERROR", "Simulated GPU Out Of Memory");

    let config = WhisperSidecarConfig::default();
    let fake_audio = PathBuf::from("fake_audio.wav");
    let res = run_whisper_transcription("clip_err_1", &fake_audio, &config, None).await;

    std::env::remove_var("NEUROCLIP_MOCK_WHISPER_ERROR");

    assert!(res.is_err());
    let err = res.unwrap_err();
    assert_eq!(err.code(), "ERR_STT_SIDECAR_FAILED");
}

#[tokio::test]
async fn test_assign_child_to_job_guard_integration() {
    // Spawn a dummy process to confirm assign_child_to_job_guard does not panic
    let mut cmd = if cfg!(windows) {
        let mut c = tokio::process::Command::new("cmd.exe");
        c.args(["/c", "echo test"]);
        c
    } else {
        let mut c = tokio::process::Command::new("echo");
        c.arg("test");
        c
    };

    cmd.kill_on_drop(true);
    let child = cmd.spawn();
    assert!(child.is_ok());

    let mut child = child.unwrap();
    assign_child_to_job_guard(&child);

    let status = child.wait().await;
    assert!(status.is_ok());
}
