#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Integration & SLA Test Suite for TASK-M4-03: Subprocess Supervisor & Kill-Switch.
//!
//! Covers:
//! - UT-03.1: FFmpeg progress parsing (`time`, `fps`, `bitrate`, `speed`, `percentage`, `eta_sec`)
//! - UT-03.2: `yt-dlp` error line parsing (`Private video`, `404`, `403`, `429`, bot-check)
//! - IT-03.1: `SubprocessSupervisor` real OS shell progress streaming -> `SidecarProgressPayload`
//! - IT-03.2 / AC-03.2: 1,000-line bounded channel capacity & >5,000-line burst drainage
//! - ED-03.1 / AC-03.3: Kill-on-Drop & `CancellationToken` fast process termination (<= 300ms)
//! - ED-03.2 / AC-03.1: `WindowsJobObjectGuard` `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` termination

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio_util::sync::CancellationToken;

use neuroclip_lib::sidecar::{
    compute_sidecar_progress, create_bounded_log_channel, parse_ffmpeg_progress_line,
    parse_ytdlp_error_line, ChildProcessGuard, SidecarProgressPayload, SubprocessConfig,
    SubprocessSupervisor, WindowsJobObjectGuard, BOUNDED_LOG_CAPACITY,
};
use neuroclip_lib::{AppError, IngestError};

// ===========================================================================
// UT-03.1: FFmpeg Progress Line Parser
// ===========================================================================

#[test]
fn test_ut_03_1_parse_ffmpeg_progress_line_and_compute_payload() {
    let line = "frame=  120 fps= 60.0 q=-1.0 size=    1024kB time=00:01:23.45 bitrate= 128.0kbits/s speed= 4.5x";
    let parsed = parse_ffmpeg_progress_line(line).expect("Must parse valid FFmpeg status line");

    assert!((parsed.time_sec - 83.45).abs() < 1e-4);
    assert_eq!(parsed.fps, Some(60.0));
    assert_eq!(parsed.bitrate_kbps, Some(128.0));
    assert_eq!(parsed.speed, Some(4.5));

    let payload = compute_sidecar_progress("task_ffmpeg_1", &parsed, Some(166.9));
    assert_eq!(payload.task_id, "task_ffmpeg_1");
    assert!((payload.percentage - 50.0).abs() < 1e-2);
    assert_eq!(payload.current_fps, Some(60.0));
    // remaining = 83.45s, speed = 4.5x -> ~18.54s -> ceil = 19s
    assert_eq!(payload.eta_sec, Some(19));

    // Unrelated banner line returns None
    assert!(parse_ffmpeg_progress_line("Input #0, matroska,webm, from 'test.mkv':").is_none());
    assert!(parse_ffmpeg_progress_line("frame=10 fps=0.0 time=N/A bitrate=N/A").is_none());
}

// ===========================================================================
// UT-03.2: yt-dlp Error Line Parser
// ===========================================================================

#[test]
fn test_ut_03_2_parse_ytdlp_error_lines() {
    let private_err = parse_ytdlp_error_line(
        "ERROR: [youtube] abc123: Private video. Sign in if you've been granted access",
    )
    .expect("Private video must map to AppError");
    assert!(matches!(
        private_err,
        AppError::Ingestion(IngestError::ExtractionFailed(_))
    ));

    let not_found_err =
        parse_ytdlp_error_line("ERROR: unable to download video data: HTTP Error 404: Not Found")
            .expect("HTTP 404 must map to AppError");
    assert!(matches!(
        not_found_err,
        AppError::Ingestion(IngestError::ExtractionFailed(_))
    ));

    let forbidden_err = parse_ytdlp_error_line("ERROR: HTTP Error 403: Forbidden")
        .expect("HTTP 403 must map to BotBlocked");
    assert!(matches!(
        forbidden_err,
        AppError::Ingestion(IngestError::BotBlocked(_))
    ));

    let bot_err =
        parse_ytdlp_error_line("ERROR: [youtube] xyz: Sign in to confirm you're not a bot.")
            .expect("Bot check must map to BotBlocked");
    assert!(matches!(
        bot_err,
        AppError::Ingestion(IngestError::BotBlocked(_))
    ));

    let bot_curly_err =
        parse_ytdlp_error_line("ERROR: [youtube] xyz: Sign in to confirm you\u{2019}re not a bot.")
            .expect("Curly apostrophe bot check must map to BotBlocked");
    assert!(matches!(
        bot_curly_err,
        AppError::Ingestion(IngestError::BotBlocked(_))
    ));

    let age_gate_err = parse_ytdlp_error_line("ERROR: [youtube] xyz: Sign in to confirm your age")
        .expect("Age gate must map to ExtractionFailed");
    assert!(matches!(
        age_gate_err,
        AppError::Ingestion(IngestError::ExtractionFailed(_))
    ));

    let rate_limit_err = parse_ytdlp_error_line("WARNING: HTTP Error 429: Too Many Requests")
        .expect("HTTP 429 must map to BotBlocked");
    assert!(matches!(
        rate_limit_err,
        AppError::Ingestion(IngestError::BotBlocked(_))
    ));

    assert!(
        parse_ytdlp_error_line("[download]  45.2% of 400.00MiB at 5.20MiB/s ETA 00:42").is_none()
    );
}

// ===========================================================================
// IT-03.1: SubprocessSupervisor Progress Streaming via Real OS Shell
// ===========================================================================

#[tokio::test]
async fn test_it_03_1_supervisor_streams_progress_from_real_shell() {
    let (binary, args) = if cfg!(windows) {
        (
            "cmd.exe".to_string(),
            vec![
                "/C".to_string(),
                "echo frame=60 fps=60.0 time=00:00:10.00 bitrate=128.0kbits/s speed=2.0x & echo frame=120 fps=59.5 time=00:00:20.00 bitrate=128.0kbits/s speed=2.0x".to_string(),
            ],
        )
    } else {
        (
            "sh".to_string(),
            vec![
                "-c".to_string(),
                "echo 'frame=60 fps=60.0 time=00:00:10.00 bitrate=128.0kbits/s speed=2.0x'; echo 'frame=120 fps=59.5 time=00:00:20.00 bitrate=128.0kbits/s speed=2.0x'".to_string(),
            ],
        )
    };

    let config = SubprocessConfig {
        task_id: "sup_progress_1".to_string(),
        binary,
        args,
        total_duration_sec: Some(40.0),
    };

    let captured: Arc<Mutex<Vec<SidecarProgressPayload>>> = Arc::new(Mutex::new(Vec::new()));
    let captured_clone = Arc::clone(&captured);
    let cb = move |payload: SidecarProgressPayload| {
        captured_clone.lock().unwrap().push(payload);
    };

    let cancel = CancellationToken::new();
    let output = SubprocessSupervisor::run(&config, None, Some(&cb), &cancel)
        .await
        .expect("Supervisor must succeed on valid shell command");

    assert_eq!(output.exit_code, Some(0));

    let events = captured.lock().unwrap();
    assert_eq!(events.len(), 2, "Expected 2 progress callbacks");
    assert_eq!(events[0].task_id, "sup_progress_1");
    assert!((events[0].percentage - 25.0).abs() < 1e-2);
    assert_eq!(events[0].current_fps, Some(60.0));
    assert_eq!(events[0].eta_sec, Some(15));

    assert!((events[1].percentage - 50.0).abs() < 1e-2);
    assert_eq!(events[1].current_fps, Some(59.5));
    assert_eq!(events[1].eta_sec, Some(10));
}

// ===========================================================================
// AC-03.2 / IT-03.2: Bounded 1,000-Line Channel & Burst Drainage (> 5,000 lines)
// ===========================================================================

#[tokio::test]
async fn test_it_03_2_bounded_channel_capacity_and_5000_line_burst() {
    assert_eq!(BOUNDED_LOG_CAPACITY, 1_000);
    let (tx, _rx) = create_bounded_log_channel();
    assert_eq!(tx.capacity(), 1_000);

    let (binary, args) = if cfg!(windows) {
        (
            "cmd.exe".to_string(),
            vec![
                "/C".to_string(),
                "for /L %i in (1,1,5500) do @echo burst_line_%i".to_string(),
            ],
        )
    } else {
        (
            "sh".to_string(),
            vec![
                "-c".to_string(),
                "i=1; while [ $i -le 5500 ]; do echo burst_line_$i; i=$((i+1)); done".to_string(),
            ],
        )
    };

    let config = SubprocessConfig {
        task_id: "burst_test".to_string(),
        binary,
        args,
        total_duration_sec: None,
    };

    let cancel = CancellationToken::new();
    let output = SubprocessSupervisor::run(&config, None, None, &cancel)
        .await
        .expect("5,500-line burst must drain cleanly without deadlock");

    assert_eq!(output.exit_code, Some(0));
    assert_eq!(output.total_stdout_lines, 5_500);
    assert!(
        output.stdout_lines.len() <= BOUNDED_LOG_CAPACITY,
        "Retained stdout lines ({}) must be capped at BOUNDED_LOG_CAPACITY ({BOUNDED_LOG_CAPACITY})",
        output.stdout_lines.len()
    );
}

// ===========================================================================
// ED-03.1 / AC-03.3: CancellationToken & Kill-on-Drop Terminate Child <= 300ms
// ===========================================================================

#[tokio::test]
async fn test_ed_03_1_cancellation_token_and_drop_kill_child_promptly() {
    // 1. Test CancellationToken inside SubprocessSupervisor
    let (binary, args) = if cfg!(windows) {
        (
            "powershell.exe".to_string(),
            vec![
                "-NoProfile".to_string(),
                "-NonInteractive".to_string(),
                "-Command".to_string(),
                "Start-Sleep -Seconds 30".to_string(),
            ],
        )
    } else {
        ("sleep".to_string(), vec!["30".to_string()])
    };

    let config = SubprocessConfig {
        task_id: "cancel_sla_test".to_string(),
        binary: binary.clone(),
        args: args.clone(),
        total_duration_sec: None,
    };

    let cancel = CancellationToken::new();
    let cancel_trigger = cancel.clone();
    let cancel_fired_at = Arc::new(Mutex::new(None::<Instant>));
    let cancel_fired_clone = Arc::clone(&cancel_fired_at);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(150)).await;
        *cancel_fired_clone.lock().unwrap() = Some(Instant::now());
        cancel_trigger.cancel();
    });

    let res = SubprocessSupervisor::run(&config, None, None, &cancel).await;
    let kill_elapsed = cancel_fired_at
        .lock()
        .unwrap()
        .map(|t| t.elapsed())
        .unwrap_or_default();

    assert!(
        matches!(res, Err(AppError::TaskCancelled)),
        "Expected TaskCancelled, got {res:?}"
    );
    assert!(
        kill_elapsed < Duration::from_millis(300),
        "Cancellation kill latency took too long: {kill_elapsed:?}"
    );

    // 2. Test ChildProcessGuard Drop kill-on-drop directly
    let mut cmd = tokio::process::Command::new(&binary);
    cmd.args(&args).kill_on_drop(true);
    let child = cmd.spawn().expect("Long-lived child must spawn");
    let pid = child.id().expect("Child must have PID");
    let guard = ChildProcessGuard::new(child);

    #[cfg(windows)]
    let sync_handle = {
        use windows_sys::Win32::Foundation::WAIT_TIMEOUT;
        use windows_sys::Win32::System::Threading::{OpenProcess, WaitForSingleObject};
        const SYNCHRONIZE_ACCESS: u32 = 0x0010_0000;
        let handle = unsafe { OpenProcess(SYNCHRONIZE_ACCESS, 0, pid) };
        assert_ne!(handle, 0, "OpenProcess(SYNCHRONIZE) must succeed");
        assert_eq!(
            unsafe { WaitForSingleObject(handle, 0) },
            WAIT_TIMEOUT,
            "Child PID {pid} must still be running before drop"
        );
        handle
    };

    let drop_start = Instant::now();
    drop(guard);

    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
        use windows_sys::Win32::System::Threading::WaitForSingleObject;
        let wait_res = unsafe { WaitForSingleObject(sync_handle, 300) };
        unsafe { CloseHandle(sync_handle) };
        assert_eq!(
            wait_res, WAIT_OBJECT_0,
            "Child PID {pid} did not terminate within 300ms after ChildProcessGuard drop"
        );
    }
    let _ = pid;
    assert!(drop_start.elapsed() <= Duration::from_millis(350));
}

// ===========================================================================
// ED-03.2 / AC-03.1: Windows Job Object JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
// ===========================================================================

#[tokio::test]
async fn test_ed_03_2_windows_job_object_guard_kills_assigned_child_on_drop() {
    let job_guard = WindowsJobObjectGuard::new().expect("WindowsJobObjectGuard::new must succeed");

    let mut cmd = if cfg!(windows) {
        let mut c = tokio::process::Command::new("powershell.exe");
        c.args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Start-Sleep -Seconds 30",
        ]);
        c
    } else {
        let mut c = tokio::process::Command::new("sleep");
        c.arg("30");
        c
    };

    let mut child = cmd.spawn().expect("Long-lived child must spawn");
    let pid = child.id().expect("Spawned child must have PID");
    job_guard
        .assign_child_process(&child)
        .expect("assign_child_process must succeed");

    #[cfg(windows)]
    let sync_handle = {
        use windows_sys::Win32::Foundation::WAIT_TIMEOUT;
        use windows_sys::Win32::System::Threading::{OpenProcess, WaitForSingleObject};
        const SYNCHRONIZE_ACCESS: u32 = 0x0010_0000;
        let handle = unsafe { OpenProcess(SYNCHRONIZE_ACCESS, 0, pid) };
        assert_ne!(
            handle, 0,
            "OpenProcess(SYNCHRONIZE) must succeed on active child"
        );
        assert_eq!(
            unsafe { WaitForSingleObject(handle, 0) },
            WAIT_TIMEOUT,
            "Child PID {pid} must still be alive before WindowsJobObjectGuard drop"
        );
        handle
    };

    // Dropping the Job Object closes the job handle -> OS immediately kills assigned processes
    let drop_start = Instant::now();
    drop(job_guard);

    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
        use windows_sys::Win32::System::Threading::WaitForSingleObject;
        assert_ne!(
            sync_handle, 0,
            "OpenProcess(SYNCHRONIZE) must succeed on active child"
        );
        let wait_res = unsafe { WaitForSingleObject(sync_handle, 300) };
        unsafe { CloseHandle(sync_handle) };
        assert_eq!(
            wait_res, WAIT_OBJECT_0,
            "OS did not kill PID {pid} within 300ms after WindowsJobObjectGuard drop"
        );
    }

    let status = tokio::time::timeout(Duration::from_millis(300), child.wait())
        .await
        .expect("Child future must resolve promptly after job close");
    assert!(status.is_ok());
    assert!(drop_start.elapsed() <= Duration::from_millis(350));
}

// ===========================================================================
// CR (\r) Stream Splitting, Bounded 5,000 CR Updates, & yt-dlp Error Priority
// ===========================================================================

#[tokio::test]
async fn test_cr_delimited_ffmpeg_progress_streams_in_real_time() {
    let (binary, args) = if cfg!(windows) {
        (
            "powershell.exe".to_string(),
            vec![
                "-NoProfile".to_string(),
                "-NonInteractive".to_string(),
                "-Command".to_string(),
                concat!(
                    "[Console]::Error.Write(\"frame=30 fps=60.0 time=00:00:01.00 bitrate=128.0kbits/s speed=1.0x`r\");",
                    "Start-Sleep -Milliseconds 30;",
                    "[Console]::Error.Write(\"frame=60 fps=60.0 time=00:00:02.00 bitrate=128.0kbits/s speed=1.0x`r\");",
                    "Start-Sleep -Milliseconds 30;",
                    "[Console]::Error.Write(\"frame=90 fps=60.0 time=00:00:03.00 bitrate=128.0kbits/s speed=1.0x`r\");",
                    "Start-Sleep -Milliseconds 30;",
                    "[Console]::Error.Write(\"frame=120 fps=60.0 time=00:00:04.00 bitrate=128.0kbits/s speed=1.0x`r\n\")"
                ).to_string(),
            ],
        )
    } else {
        (
            "sh".to_string(),
            vec![
                "-c".to_string(),
                concat!(
                    "printf 'frame=30 fps=60.0 time=00:00:01.00 bitrate=128.0kbits/s speed=1.0x\\r'; sleep 0.03;",
                    "printf 'frame=60 fps=60.0 time=00:00:02.00 bitrate=128.0kbits/s speed=1.0x\\r'; sleep 0.03;",
                    "printf 'frame=90 fps=60.0 time=00:00:03.00 bitrate=128.0kbits/s speed=1.0x\\r'; sleep 0.03;",
                    "printf 'frame=120 fps=60.0 time=00:00:04.00 bitrate=128.0kbits/s speed=1.0x\\r\\n'"
                ).to_string(),
            ],
        )
    };

    let config = SubprocessConfig {
        task_id: "cr_progress_test".to_string(),
        binary,
        args,
        total_duration_sec: Some(10.0),
    };

    let captured: Arc<Mutex<Vec<(Instant, SidecarProgressPayload)>>> =
        Arc::new(Mutex::new(Vec::new()));
    let captured_clone = Arc::clone(&captured);
    let cb = move |payload: SidecarProgressPayload| {
        captured_clone
            .lock()
            .unwrap()
            .push((Instant::now(), payload));
    };

    let cancel = CancellationToken::new();
    let output = SubprocessSupervisor::run(&config, None, Some(&cb), &cancel)
        .await
        .expect("CR-delimited FFmpeg progress stream must succeed");

    assert_eq!(output.exit_code, Some(0));
    assert_eq!(output.total_stderr_lines, 4);

    let events = captured.lock().unwrap();
    assert_eq!(
        events.len(),
        4,
        "Expected 4 separate real-time CR callbacks"
    );
    assert!((events[0].1.percentage - 10.0).abs() < 1e-2);
    assert!((events[1].1.percentage - 20.0).abs() < 1e-2);
    assert!((events[2].1.percentage - 30.0).abs() < 1e-2);
    assert!((events[3].1.percentage - 40.0).abs() < 1e-2);
    assert!(
        events[3].0.duration_since(events[0].0) >= Duration::from_millis(50),
        "Callbacks must stream in real time rather than collapsing until exit"
    );
}

#[tokio::test]
async fn test_5000_cr_only_updates_bounded_without_single_string_accumulation() {
    let (binary, args) = if cfg!(windows) {
        (
            "powershell.exe".to_string(),
            vec![
                "-NoProfile".to_string(),
                "-NonInteractive".to_string(),
                "-Command".to_string(),
                concat!(
                    "$sb = [System.Text.StringBuilder]::new();",
                    "for ($i = 1; $i -le 5000; $i++) {",
                    " [void]$sb.Append(\"frame=$i fps=60.0 time=00:00:01.00`r\");",
                    " if (($i % 500) -eq 0) { [Console]::Error.Write($sb.ToString()); [void]$sb.Clear() }",
                    "}"
                )
                .to_string(),
            ],
        )
    } else {
        (
            "sh".to_string(),
            vec![
                "-c".to_string(),
                "i=1; while [ $i -le 5000 ]; do printf 'frame=%d fps=60.0 time=00:00:01.00\\r' \"$i\"; i=$((i+1)); done"
                    .to_string(),
            ],
        )
    };

    let config = SubprocessConfig {
        task_id: "cr_5000_burst".to_string(),
        binary,
        args,
        total_duration_sec: Some(10.0),
    };

    let cancel = CancellationToken::new();
    let output = SubprocessSupervisor::run(&config, None, None, &cancel)
        .await
        .expect("5,000 CR-only updates must drain cleanly");

    assert_eq!(output.exit_code, Some(0));
    assert_eq!(output.total_stderr_lines, 5_000);
    assert_eq!(output.stderr_lines.len(), BOUNDED_LOG_CAPACITY);
    assert!(
        output.stderr_lines.iter().all(|l| l.len() < 256),
        "CR updates must be split into bounded individual lines without single-string accumulation"
    );
    assert_eq!(
        output.stderr_lines.last().map(String::as_str),
        Some("frame=5000 fps=60.0 time=00:00:01.00")
    );
}

#[tokio::test]
async fn test_ytdlp_error_priority_survives_preceding_error_and_1000_line_eviction() {
    let (binary, args) = if cfg!(windows) {
        (
            "powershell.exe".to_string(),
            vec![
                "-NoProfile".to_string(),
                "-NonInteractive".to_string(),
                "-Command".to_string(),
                concat!(
                    "[Console]::Error.WriteLine('ERROR: [youtube] xyz: Unable to download webpage');",
                    "[Console]::Error.WriteLine('ERROR: HTTP Error 403: Forbidden');",
                    "$sb = [System.Text.StringBuilder]::new();",
                    "for ($i = 1; $i -le 1200; $i++) { [void]$sb.AppendLine(\"debug_tail_$i\") };",
                    "[Console]::Error.Write($sb.ToString());",
                    "exit 1"
                )
                .to_string(),
            ],
        )
    } else {
        (
            "sh".to_string(),
            vec![
                "-c".to_string(),
                concat!(
                    "echo 'ERROR: [youtube] xyz: Unable to download webpage' >&2;",
                    "echo 'ERROR: HTTP Error 403: Forbidden' >&2;",
                    "i=1; while [ $i -le 1200 ]; do echo \"debug_tail_$i\" >&2; i=$((i+1)); done;",
                    "exit 1"
                )
                .to_string(),
            ],
        )
    };

    let config = SubprocessConfig {
        task_id: "ytdlp_priority_test".to_string(),
        binary,
        args,
        total_duration_sec: None,
    };

    let cancel = CancellationToken::new();
    let res = SubprocessSupervisor::run(&config, None, None, &cancel).await;

    assert!(
        matches!(
            res,
            Err(AppError::Ingestion(IngestError::BotBlocked(ref msg))) if msg.contains("403")
        ),
        "Expected BotBlocked to override preceding generic ERROR and survive >1,000 trailing lines, got {res:?}"
    );
}
