#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Challenger 2 Empirical Adversarial Test Harness (TASK-M4-08).
//!
//! Stress-tests:
//! 1. Windows Job Object & Subprocess Lifecycle (ED-08.2)
//! 2. Dual Subtitle Export Correctness & Injection Immunity (IT-08.2 / AC-08.5)
//! 3. Bounded Memory Streaming SHA-256 (ED-08.4)

use std::fs::File;
use std::io::Write;
use std::time::Duration;
use neuroclip_lib::ingestion::streamer::assign_child_to_job_guard;
use neuroclip_lib::stt::{
    compute_file_sha256, escape_ass_text, format_ass_timecode, format_srt_timecode,
    generate_ass_script, generate_srt_script, verify_model, TranscriptSegment,
    TranscriptSegmentsContainer,
};
use sha2::{Digest, Sha256};

// ===========================================================================
// CHALLENGE 1: Windows Job Object & Subprocess Lifecycle (ED-08.2)
// ===========================================================================

#[cfg(windows)]
#[tokio::test]
async fn test_challenger_kill_on_drop_and_job_object_termination() {
    // Spawn a long-running process with kill_on_drop(true) and register to Job Object
    let mut cmd = tokio::process::Command::new("powershell.exe");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", "Start-Sleep -Seconds 60"]);
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());
    cmd.kill_on_drop(true);

    let child = cmd.spawn();
    assert!(child.is_ok(), "Failed to spawn test process");
    let mut child = child.unwrap();

    let pid = child.id().expect("Spawned child must have a valid PID");
    assert!(pid > 0);

    // Assign to Windows Job Object
    assign_child_to_job_guard(&child);

    // Verify child is initially running
    let status_before = child.try_wait().unwrap();
    assert!(status_before.is_none(), "Child should be running before drop");

    // Drop child process handle: kill_on_drop(true) MUST immediately terminate child
    drop(child);

    // Verify child has been terminated at OS level using Win32 OpenProcess
    tokio::time::sleep(Duration::from_millis(150)).await;

    unsafe {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::{
            GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };

        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle != 0 {
            let mut exit_code: u32 = 0;
            let ok = GetExitCodeProcess(handle, &mut exit_code);
            CloseHandle(handle);

            if ok != 0 {
                const STILL_ACTIVE: u32 = 259;
                assert_ne!(
                    exit_code, STILL_ACTIVE,
                    "Process PID {pid} was NOT terminated after dropping child with kill_on_drop(true)"
                );
            }
        }
    }
}

#[tokio::test]
async fn test_challenger_assign_child_to_job_guard_resilience() {
    // Spawning a fast-exiting command to test race condition where process exits
    // before or during assignment
    let mut cmd = if cfg!(windows) {
        let mut c = tokio::process::Command::new("cmd.exe");
        c.args(["/c", "exit 0"]);
        c
    } else {
        tokio::process::Command::new("true")
    };

    cmd.kill_on_drop(true);
    let mut child = cmd.spawn().expect("Failed to spawn command");

    // Immediate assignment
    assign_child_to_job_guard(&child);

    // Wait for it to finish
    let res = child.wait().await;
    assert!(res.is_ok());

    // Subsequent assignment after process termination should not panic
    assign_child_to_job_guard(&child);
}

// ===========================================================================
// CHALLENGE 2: Dual Subtitle Export Correctness & Injection Immunity
// ===========================================================================

#[test]
fn test_challenger_ass_exact_color_codes_and_styles() {
    let container = TranscriptSegmentsContainer {
        clip_id: "clip_colors".to_string(),
        segments: vec![
            TranscriptSegment {
                segment_id: "seg_1".to_string(),
                start_sec: 0.0,
                end_sec: 2.0,
                speaker_id: "neuro".to_string(),
                text_en: "Neuro text".to_string(),
                text_vi: None,
                confidence: 1.0,
                cps_score: 5.0,
                is_flagged_for_review: false,
                words: vec![],
            },
            TranscriptSegment {
                segment_id: "seg_2".to_string(),
                start_sec: 2.0,
                end_sec: 4.0,
                speaker_id: "evil".to_string(),
                text_en: "Evil text".to_string(),
                text_vi: None,
                confidence: 1.0,
                cps_score: 5.0,
                is_flagged_for_review: false,
                words: vec![],
            },
            TranscriptSegment {
                segment_id: "seg_3".to_string(),
                start_sec: 4.0,
                end_sec: 6.0,
                speaker_id: "vedal".to_string(),
                text_en: "Vedal text".to_string(),
                text_vi: None,
                confidence: 1.0,
                cps_score: 5.0,
                is_flagged_for_review: false,
                words: vec![],
            },
            TranscriptSegment {
                segment_id: "seg_4".to_string(),
                start_sec: 6.0,
                end_sec: 8.0,
                speaker_id: "guest".to_string(),
                text_en: "Guest text".to_string(),
                text_vi: None,
                confidence: 1.0,
                cps_score: 5.0,
                is_flagged_for_review: false,
                words: vec![],
            },
            TranscriptSegment {
                segment_id: "seg_5".to_string(),
                start_sec: 8.0,
                end_sec: 10.0,
                speaker_id: "chat".to_string(),
                text_en: "Chat text".to_string(),
                text_vi: None,
                confidence: 1.0,
                cps_score: 5.0,
                is_flagged_for_review: false,
                words: vec![],
            },
            TranscriptSegment {
                segment_id: "seg_6".to_string(),
                start_sec: 10.0,
                end_sec: 12.0,
                speaker_id: "unknown_person".to_string(),
                text_en: "Default text".to_string(),
                text_vi: None,
                confidence: 1.0,
                cps_score: 5.0,
                is_flagged_for_review: false,
                words: vec![],
            },
        ],
    };

    let ass = generate_ass_script(&container);

    // Verify exact &HAABBGGRR color specifications
    assert!(ass.contains("Style: Neuro,Arial,54,&H00B6B6FF"), "Neuro color must be &H00B6B6FF");
    assert!(ass.contains("Style: Evil,Arial,54,&H000000DC"), "Evil color must be &H000000DC");
    assert!(ass.contains("Style: Vedal,Arial,54,&H0066CC66"), "Vedal color must be &H0066CC66");
    assert!(ass.contains("Style: Guest,Arial,54,&H00FFCC33"), "Guest color must be &H00FFCC33");
    assert!(ass.contains("Style: Chat,Arial,48,&H00CCCCCC"), "Chat color must be &H00CCCCCC");
    assert!(ass.contains("Style: Default,Arial,52,&H00FFFFFF"), "Default color must be &H00FFFFFF");

    // Verify each speaker segment uses their specific style
    assert!(ass.contains(",Neuro,Neuro,0,0,0,,Neuro text"));
    assert!(ass.contains(",Evil,Evil,0,0,0,,Evil text"));
    assert!(ass.contains(",Vedal,Vedal,0,0,0,,Vedal text"));
    assert!(ass.contains(",Guest,Guest,0,0,0,,Guest text"));
    assert!(ass.contains(",Chat,Chat,0,0,0,,Chat text"));
    assert!(ass.contains(",Default,Default,0,0,0,,Default text"));
}

#[test]
fn test_challenger_ass_tag_injection_and_multiline_escaping() {
    let adversarial_inputs = [
        ("{\\pos(960,540)\\c&H0000FF&}Injected tag", "(\\pos(960,540)\\c&H0000FF&)Injected tag"),
        ("Nested {{brackets}} and unclosed {", "Nested ((brackets)) and unclosed ("),
        ("Line 1\r\nLine 2\nLine 3\rLine 4", "Line 1\\NLine 2\\NLine 3Line 4"),
        ("Special symbols: & < > \" ' , ; :", "Special symbols: & < > \" ' , ; :"),
        ("Vietnamese: Đậu phộng, bánh tráng nướng, cà phê sữa đá", "Vietnamese: Đậu phộng, bánh tráng nướng, cà phê sữa đá"),
    ];

    for (raw, expected) in adversarial_inputs {
        let escaped = escape_ass_text(raw);
        assert_eq!(escaped, expected);
        // CRITICAL INVARIANT: No literal curly braces must survive
        assert!(!escaped.contains('{'), "Escaped text must not contain '{{': {escaped}");
        assert!(!escaped.contains('}'), "Escaped text must not contain '}}': {escaped}");
        assert!(!escaped.contains('\r'), "Escaped text must not contain carriage return: {escaped}");
        assert!(!escaped.contains('\n'), "Escaped text must not contain raw newline: {escaped}");
    }
}

#[test]
fn test_challenger_srt_sequential_numbering_and_timestamp_syntax() {
    // Generate 120 segments to verify multi-digit sequential ordering
    let mut segments = Vec::new();
    for i in 0..120 {
        segments.push(TranscriptSegment {
            segment_id: format!("seg_{i:04}"),
            start_sec: (i as f64) * 2.5,
            end_sec: (i as f64) * 2.5 + 2.0,
            speaker_id: "vedal".to_string(),
            text_en: format!("Sentence {i}"),
            text_vi: None,
            confidence: 0.95,
            cps_score: 8.0,
            is_flagged_for_review: false,
            words: vec![],
        });
    }

    let container = TranscriptSegmentsContainer {
        clip_id: "clip_120_seg".to_string(),
        segments,
    };

    let srt = generate_srt_script(&container);

    // Verify 1-based sequential index presence
    for i in 1..=120 {
        let expected_index = format!("{i}\n");
        assert!(srt.contains(&expected_index), "SRT missing sequence number {i}");
    }

    // Verify timecode format: HH:MM:SS,mmm --> HH:MM:SS,mmm
    assert!(srt.contains("00:00:00,000 --> 00:00:02,000"));
    assert!(srt.contains("00:00:02,500 --> 00:00:04,500"));
    // Segment 100: start_sec = 250.0 (4m 10s), end_sec = 252.0
    assert!(srt.contains("00:04:10,000 --> 00:04:12,000"));

    // Verify no trailing spaces on timestamps and proper delimiter
    assert!(!srt.contains("-->00:"));
    assert!(!srt.contains(".000")); // SRT uses commas, not dots!
}

#[test]
fn test_challenger_timecode_boundary_stress() {
    // 0.0 seconds
    assert_eq!(format_ass_timecode(0.0), "0:00:00.00");
    assert_eq!(format_srt_timecode(0.0), "00:00:00,000");

    // 59.999 seconds:
    // In ASS (centiseconds, 2 decimals): 59.999 rounds to 60.00 -> 0:01:00.00
    // In SRT (milliseconds, 3 decimals): 59.999 is exactly 59s 999ms -> 00:00:59,999
    assert_eq!(format_ass_timecode(59.999), "0:01:00.00");
    assert_eq!(format_srt_timecode(59.999), "00:00:59,999");

    // 59.9996 seconds: both round up to 60 seconds (1 minute)
    assert_eq!(format_ass_timecode(59.9996), "0:01:00.00");
    assert_eq!(format_srt_timecode(59.9996), "00:01:00,000");

    // 3599.999 seconds:
    // In ASS: rounds up to 3600s -> 1:00:00.00
    // In SRT: exactly 59m 59s 999ms -> 00:59:59,999
    assert_eq!(format_ass_timecode(3599.999), "1:00:00.00");
    assert_eq!(format_srt_timecode(3599.999), "00:59:59,999");

    // 3599.9996 seconds: both round up to 3600 seconds (1 hour)
    assert_eq!(format_ass_timecode(3599.9996), "1:00:00.00");
    assert_eq!(format_srt_timecode(3599.9996), "01:00:00,000");

    // Negative timestamps (must clamp to 0 safely without panic)
    assert_eq!(format_ass_timecode(-999.0), "0:00:00.00");
    assert_eq!(format_srt_timecode(-999.0), "00:00:00,000");

    // NaN and Inf (must clamp to 0 safely without panic)
    assert_eq!(format_ass_timecode(f64::NAN), "0:00:00.00");
    assert_eq!(format_srt_timecode(f64::NAN), "00:00:00,000");
    assert_eq!(format_ass_timecode(f64::INFINITY), "0:00:00.00");
    assert_eq!(format_srt_timecode(f64::NEG_INFINITY), "00:00:00,000");

    // Large duration: 25 hours (90000.0 seconds)
    assert_eq!(format_ass_timecode(90000.0), "25:00:00.00");
    assert_eq!(format_srt_timecode(90000.0), "25:00:00,000");
}

// ===========================================================================
// CHALLENGE 3: Bounded Memory Streaming SHA-256 (ED-08.4)
// ===========================================================================

#[test]
fn test_challenger_streaming_sha256_large_payload() {
    // Create a 5MB synthetic file in temp dir to stress streaming calculation
    let temp_file = std::env::temp_dir().join("challenger_5mb_model.bin");
    let total_bytes: usize = 5 * 1024 * 1024; // 5 MB

    let mut expected_hasher = Sha256::new();
    {
        let mut f = File::create(&temp_file).unwrap();
        // Write in 128KB patterns
        let block_size = 131072;
        let mut block = vec![0u8; block_size];
        for (i, item) in block.iter_mut().enumerate() {
            *item = ((i * 31 + 17) % 256) as u8;
        }

        let mut written = 0;
        while written < total_bytes {
            let to_write = (total_bytes - written).min(block_size);
            f.write_all(&block[..to_write]).unwrap();
            expected_hasher.update(&block[..to_write]);
            written += to_write;
        }
    }

    let expected_sha256 = format!("{:x}", expected_hasher.finalize());

    // Compute via model_manager::compute_file_sha256
    let actual_sha256 = compute_file_sha256(&temp_file);
    let _ = std::fs::remove_file(&temp_file);

    assert!(actual_sha256.is_ok(), "compute_file_sha256 failed on 5MB file");
    assert_eq!(actual_sha256.unwrap(), expected_sha256);
}

#[test]
fn test_challenger_model_verify_fast_fails() {
    // Fast-fail test 1: Non-existent file
    let missing_path = std::env::temp_dir().join("non_existent_whisper_xyz.bin");
    let res1 = verify_model("base.en-q5_k", Some(&missing_path));
    assert!(res1.is_err());
    assert_eq!(res1.unwrap_err().code(), "ERR_STT_MODEL_NOT_FOUND");

    // Fast-fail test 2: Tampered file with wrong hash
    let tampered_path = std::env::temp_dir().join("tampered_whisper_xyz.bin");
    {
        let mut f = File::create(&tampered_path).unwrap();
        f.write_all(b"tampered content").unwrap();
    }
    let res2 = verify_model("base.en-q5_k", Some(&tampered_path));
    let _ = std::fs::remove_file(&tampered_path);

    assert!(res2.is_err());
    assert_eq!(res2.unwrap_err().code(), "ERR_STT_MODEL_INTEGRITY_FAILED");
}
