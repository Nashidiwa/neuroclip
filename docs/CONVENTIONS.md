# QUY CHUẨN LẬP TRÌNH & ĐẶC TẢ KỸ THUẬT PHÁT TRIỂN (DEVELOPMENT CONVENTIONS)
# Dự án: NeuroClip — Windows Desktop App Hỗ trợ Clipper & Subber Neuro-sama Vietnam

**Mã tài liệu:** `docs/CONVENTIONS.md`  
**Phiên bản:** 1.0.0 (Ban hành chính thức tại Phase 3 — Đồng bộ Phase 4 `M4-01..M4-25`)  
**Chủ trì:** `@tech-lead`  
**Căn cứ kỹ thuật & Pháp lý:**
- Hiến pháp dự án: `AGENTS.md` (Zero-Guesswork Policy, Single Source of Truth, Gate Checks)
- Kiến trúc hệ thống: `docs/ARCHITECTURE.md` (v1.0.0, Sections 1–7)
- Yêu cầu sản phẩm: `docs/PRD.md` (v1.3.0, Epics 1–6)
- 8 Quyết định kiến trúc đã phê duyệt: `docs/adr/ADR-001` đến `ADR-008`
- Bảng phân rã task & ma trận kiểm thử: `docs/TASK_BOARD.md`

---

## MỤC LỤC
1. [NGUYÊN TẮC PHÁT TRIỂN CỐT LÕI (CORE INVARIANTS)](#1-nguyên-tắc-phát-triển-cốt-lõi-core-invariants)
2. [QUY CHUẨN BACKEND RUST (TAURI V2)](#2-quy-chuẩn-backend-rust-tauri-v2)
   - 2.1 [Toolchain, Linter & Formatter](#21-toolchain-linter--formatter)
   - 2.2 [Bất Đồng Bộ & Mô Hình Luồng Tokio Runtime](#22-bất-đồng-bộ--mô-hình-luồng-tokio-runtime)
   - 2.3 [An Toàn Bộ Nhớ & Bounded Buffers (Chống Tràn RAM/VRAM)](#23-an-toàn-bộ-nhớ--bounded-buffers-chống-tràn-ramvram)
   - 2.4 [Chính Sách Panic-Free & Xử Lý Result](#24-chính-sách-panic-free--xử-lý-result)
   - 2.5 [Cấu Trúc Thư Mục Chuẩn `src-tauri/`](#25-cấu-trúc-thư-mục-chuẩn-src-tauri)
3. [QUY CHUẨN FRONTEND TYPESCRIPT (REACT 18/19 + VITE)](#3-quy-chuẩn-frontend-typescript-react-1819--vite)
   - 3.1 [Cấu Hình TypeScript Strict Mode & Compile Invariants](#31-cấu-hình-typescript-strict-mode--compile-invariants)
   - 3.2 [Linter & Code Formatter (ESLint 9 Flat Config + Prettier)](#32-linter--code-formatter-eslint-9-flat-config--prettier)
   - 3.3 [Cấu Trúc Thư Mục Chuẩn `src/`](#33-cấu-trúc-thư-mục-chuẩn-src)
   - 3.4 [Quy Ước Đặt Tên Toàn Diện (Naming Conventions)](#34-quy-ước-đặt-tên-toàn-diện-naming-conventions)
   - 3.5 [Hiệu Năng Timeline & Virtualized Subtitle Rendering](#35-hiệu-năng-timeline--virtualized-subtitle-rendering)
4. [GIAO THỨC TRUYỀN THÔNG LIÊN TIẾN TRÌNH IPC (TAURI COMMANDS & EVENTS)](#4-giao-thức-truyền-thông-liên-tiến-trình-ipc-tauri-commands--events)
   - 4.1 [Quy Ước Định Danh & Chữ Ký Tauri Command](#41-quy-ước-định-danh--chữ-ký-tauri-command)
   - 4.2 [Hệ Thống Streaming Sự Kiện Bất Đồng Bộ (Event Streaming)](#42-hệ-thống-streaming-sự-kiện-bất-đồng-bộ-event-streaming)
   - 4.3 [Giao Kèo Serialization Dữ Liệu Đồng Bộ](#43-giao-kèo-serialization-dữ-liệu-đồng-bộ)
5. [GIAO THỨC XỬ LÝ LỖI TOÀN HỆ THỐNG (ERROR HANDLING PROTOCOL)](#5-giao-thức-xử-lý-lỗi-toàn-hệ-thống-error-handling-protocol)
   - 5.1 [Cây Lỗi Rust Phân Cấp Dùng `thiserror`](#51-cây-lỗi-rust-phân-cấp-dùng-thiserror)
   - 5.2 [Cấu Trúc Vỏ Bọc Lỗi Tuần Tự Hóa IPC (IpcError Envelope)](#52-cấu-trúc-vỏ-bọc-lỗi-tuần-tự-hóa-ipc-ipcerror-envelope)
   - 5.3 [Chiến Lược Phục Hồi Lỗi Ở Tầng Frontend (Error Boundaries & Retries)](#53-chiến-lược-phục-hồi-lỗi-ở-tầng-frontend-error-boundaries--retries)
6. [TIÊU CHUẨN GHI NHẬT KÝ & GIÁM SÁT HỆ THỐNG (LOGGING & TELEMETRY)](#6-tiêu-chuẩn-ghi-nhật-ký--giám-sát-hệ-thống-logging--telemetry)
   - 6.1 [Khung Structured Logging Dùng `tracing`](#61-khung-structured-logging-dùng-tracing)
   - 6.2 [Phân Định Cấp Độ Log (Log Levels) & Ngữ Cảnh](#62-phân-định-cấp-độ-log-log-levels--ngữ-cảnh)
   - 6.3 [Bảo Mật Tuyệt Đối & Khử Trùng Thông Tin Nhạy Cảm (Secrets Redaction)](#63-bảo-mật-tuyệt-đối--khử-trùng-thông-tin-nhạy-cảm-secrets-redaction)
   - 6.4 [Vị Trí Lưu Trữ, Xoay Vòng & Dọn Dẹp File Nhật Ký](#64-vị-trí-lưu-trữ-xoay-vòng--dọn-dẹp-file-nhật-ký)
7. [QUY CHUẨN KIỂM THỬ VÀ BẢO ĐẢM CHẤT LƯỢNG (TESTING & QA)](#7-quy-chuẩn-kiểm-thử-và-bảo-đảm-chất-lượng-testing--qa)
   - 7.1 [Unit & Integration Testing Phía Rust](#71-unit--integration-testing-phía-rust)
   - 7.2 [Unit & Component Testing Phía Frontend](#72-unit--component-testing-phía-frontend)
   - 7.3 [Checklist Kiểm Định Trước Khi Hợp Nhất (Pre-merge Verification)](#73-checklist-kiểm-định-trước-khi-hợp-nhất-pre-merge-verification)

---

## 1. NGUYÊN TẮC PHÁT TRIỂN CỐT LÕI (CORE INVARIANTS)

Mọi kỹ sư và agent tham gia triển khai mã nguồn trong Phase 4 phải tuân thủ nghiêm ngặt 5 nguyên tắc bất biến sau:

1. **Zero-Guesswork & Single Source of Truth (SSOT):** Mọi module, hàm, giao diện dữ liệu phải tuân thủ đúng các hợp đồng JSON Schema đã duyệt tại Section 5 của `docs/ARCHITECTURE.md` và các quyết định kỹ thuật tại `docs/adr/ADR-001` đến `ADR-008`. Không tự ý thay đổi schema mà không có RFC trình PO.
2. **Memory Safety & Low Footprint Invariant:** Mức chiếm dụng RAM tĩnh của ứng dụng phải giữ trong trần kỹ thuật **$\le 100\text{MB}$** (mục tiêu tối ưu $50\text{–}80\text{MB}$ theo `ADR-002`/`ADR-008`; chỉ tiêu nghiệm thu `TASK_BOARD AC-01.2` $\le 90\text{MB}$; trần sản phẩm `PRD`/`ARCHITECTURE` $\le 120\text{MB}$). Không bao giờ tải toàn bộ video 30GB vào ổ đĩa hay bộ nhớ; luôn thực hiện Audio-First (`ADR-003`).
3. **Panic-Free Production Backend:** Tuyệt đối không sử dụng `.unwrap()` hoặc `.expect()` trong bất kỳ luồng xử lý thực tế nào (non-test code) của Rust backend. Mọi lỗi phải được xử lý qua kiểu `Result<T, AppError>`.
4. **Strict Concurrency Separation:** Phân tách triệt để giữa I/O bất đồng bộ (Tokio Async) và tính toán nặng tiêu tốn CPU (Audio RMS, VAD, Z-Score). Toàn bộ tác vụ tính toán nặng phải được đẩy sang `tokio::task::spawn_blocking` để không làm nghẽn Event Loop của Tokio.
5. **Privacy & Dual Export Guarantee:** Không bao giờ ghi API key người dùng (Groq, Gemini, OpenAI) hoặc Twitch Token ra file log (`[REDACTED]`). Khi xuất dự án CapCut Draft JSON, luôn xuất song song phụ đề `.ass` và `.srt` độc lập (`ADR-005`).

---

## 2. QUY CHUẨN BACKEND RUST (TAURI V2)

### 2.1 Toolchain, Linter & Formatter

- **Rust Edition:** `2021` (yêu cầu compiler `rustc >= 1.78.0`).
- **Toolchain Target:** `stable-x86_64-pc-windows-msvc`.
- **Clippy Configuration:** Toàn bộ crate `src-tauri` phải áp dụng các cờ kiểm tra nghiêm ngặt trong `src-tauri/src/lib.rs`:
  ```rust
  #![deny(clippy::all)]
  #![warn(clippy::pedantic)]
  #![deny(clippy::unwrap_used)]
  #![deny(clippy::expect_used)]
  #![deny(clippy::panic)]
  #![deny(clippy::todo)]
  #![deny(clippy::unimplemented)]
  #![allow(clippy::module_name_repetitions)]
  #![allow(clippy::must_use_candidate)]
  #![allow(clippy::doc_markdown, clippy::missing_errors_doc)]
  #![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
  ```
  - **Chính sách `clippy::pedantic` có chọn lọc (PO Decision 4 Option 1):** Tuyệt đối **KHÔNG tắt toàn bộ `clippy::pedantic`**. Chỉ được phép cấp `#[allow(...)]` cục bộ có chọn lọc cho:
    1. Các lint hình thức ở crate root (`module_name_repetitions`, `must_use_candidate`, `doc_markdown`, `missing_errors_doc`);
    2. Các phép ép kiểu số học DSP/audio/parser trên các module xử lý tín hiệu (`cast_precision_loss`, `cast_possible_truncation`, `cast_sign_loss`, `approx_constant`);
    3. Một số ít ngoại lệ cấp hàm (`too_many_lines`, `too_many_arguments`, `needless_pass_by_value` đối với tham số `String` bắt buộc của Tauri IPC commands, `unnecessary_wraps`).
- **Code Formatter (`rustfmt.toml`):** Đặt tại `src-tauri/rustfmt.toml` (tương thích 100% toolchain `stable`, không dùng option nightly):
  ```toml
  max_width = 100
  hard_tabs = false
  tab_spaces = 4
  newline_style = "Windows"
  use_field_init_shorthand = true
  use_try_shorthand = true
  reorder_imports = true
  edition = "2021"
  ```

### 2.2 Bất Đồng Bộ & Mô Hình Luồng Tokio Runtime

- **Runtime:** Sử dụng `#[tokio::main]` do Tauri v2 quản lý.
- **Quy tắc phân tách Async I/O vs Blocking Compute:**
  - **Async I/O:** Sử dụng `tokio::fs`, `tokio::process::Command`, và `reqwest` bất đồng bộ.
  - **Blocking Compute:** Các tác vụ phân tích âm thanh RMS (`symphonia`), lọc Silero VAD, thuật toán Z-score trên 500,000 sự kiện chat, hoặc giải mã JSON dung lượng lớn (>5MB) **BẮT BUỘC** phải chạy qua `tokio::task::spawn_blocking`:
    ```rust
    // ĐÚNG: Đẩy tính toán RMS nặng sang threadpool blocking
    let rms_energy = tokio::task::spawn_blocking(move || {
        compute_audio_rms_sync(&audio_path, window_ms)
    }).await.map_err(|e| AppError::Highlight(HighlightError::WorkerPanicked(e.to_string())))??;

    // SAI: Chạy tính toán nặng trực tiếp trong async fn làm nghẽn tokio worker
    let rms_energy = compute_audio_rms_sync(&audio_path, window_ms)?; // VI PHẠM!
    ```
- **Cơ Chế Hủy Tác Vụ Dài Hạn (Task Cancellation):**
  - Mọi tác vụ nền dài hạn (tải audio VOD, cắt range slicing, STT chunking, export) phải nhận một `tokio_util::sync::CancellationToken`.
  - Quản lý cấu hình và hồ sơ phần cứng qua `tauri::State<AppState>` (`config: RwLock<AppConfig>`, `hardware_profile: RwLock<Option<HardwareProfile>>` trong `commands/config_cmd.rs`), còn các `CancellationToken` đang chạy được quản lý qua registry `ACTIVE_TASKS` (`commands/ingest_cmd.rs`) và `ACTIVE_SLICE_TASKS` (`commands/slice_cmd.rs`).
  - Khi người dùng bấm nút "Hủy (Cancel)" trên UI, backend kích hoạt `.cancel()` trên token tương ứng.
- **Giám Sát Tiến Trình Con (Subprocess Supervision - `ADR-007`):**
  - Tất cả các lệnh gọi `ffmpeg.exe` hoặc `yt-dlp.exe` qua `tokio::process::Command` **phải kích hoạt `.kill_on_drop(true)`**:
    ```rust
    let mut cmd = tokio::process::Command::new(ffmpeg_path);
    cmd.kill_on_drop(true); // Đảm bảo tự sát tiến trình khi future bị drop
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    ```
  - Trên Windows, kết hợp với Windows Job Objects (`windows_sys::Win32::System::JobObjects` / `CreateJobObjectW` trong `src-tauri/src/sidecar/kill_switch.rs`) gán cờ `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` để đảm bảo khi ứng dụng chính bị terminate hoặc crash, toàn bộ subprocess con bị hệ điều hành dọn dẹp sạch sẽ trong vòng $\le 300\text{ms}$ (0 zombie processes).

### 2.3 An Toàn Bộ Nhớ & Bounded Buffers (Chống Tràn RAM/VRAM)

- **Giới hạn Bounded Buffers:** Tuyệt đối không đọc toàn bộ file âm thanh 400MB hoặc stream video vào `Vec<u8>` trong RAM.
  - Phải xử lý stream theo khối kích thước cố định (`buf_size = 64 * 1024` - 64KB) hoặc sử dụng file tạm thời trên ổ đĩa (`tempfile::NamedTempFile`).
  - Phân tích stdout/stderr của subprocess theo dòng bằng `tokio::io::BufReader` / `AsyncReadExt` và giới hạn kênh đệm (bounded channel) không vượt quá 1,000 dòng (`BOUNDED_LOG_CAPACITY = 1_000` trong `sidecar/supervisor.rs`).
- **Pre-allocation cho Chat Data Structures:** Khi xử lý `chat_events.json`, luôn khởi tạo vector với dung lượng dự tính (`Vec::with_capacity(estimated_len)`) để tránh realloc liên tục trong bộ nhớ heap.
- **Bảo Vệ VRAM (CUDA OOM Guardrail - `ADR-001`):** Trước khi gọi `faster-whisper` cục bộ, phải kiểm tra thông số từ `probe_system_hardware()` (`config/hardware.rs`) qua `evaluate_hardware_guardrail_from_profile()` (`stt/faster_whisper_worker.rs`). Nếu VRAM khả dụng `< 2500MB` (`MIN_LARGE_VRAM_MB`), lập tức chặn khởi tạo mô hình Local Whisper Large, tự động hạ cấp về INT8 Medium/Small (`MAX_INT8_VRAM_CAP_MB = 1800`, với policy `auto_downgrade`) hoặc trả về lỗi `ERR_STT_CUDA_OOM` (với policy `strict_reject`) gợi ý chuyển sang Cloud BYOK Groq API.

### 2.4 Chính Sách Panic-Free & Xử Lý Result

- Tuyệt đối cấm sử dụng `.unwrap()` hoặc `.expect()` trong non-test code.
- Sử dụng toán tử `?` để lan truyền lỗi lên tầng cao hơn.
- Với các trường hợp ngoại lệ bắt buộc phải có giá trị mặc định, sử dụng `.unwrap_or(default)` hoặc `.unwrap_or_else(|| ...)`.
- Mọi hàm Tauri Command phải trả về kiểu `Result<T, AppError>`.

### 2.5 Cấu Trúc Thư Mục Chuẩn `src-tauri/`

```
src-tauri/
├── Cargo.toml
├── build.rs
├── tauri.conf.json
├── rustfmt.toml
├── capabilities/
│   └── default.json              # Tauri v2 capabilities & permissions
├── resources/
│   └── models/
│       └── silero_vad_16k.onnx   # Mô hình Silero VAD v5 đồ thị tĩnh 16kHz (ADR-008)
├── src/
│   ├── main.rs                   # Điểm vào ứng dụng, khởi tạo Tauri runtime
│   ├── lib.rs                    # Clippy lints, đăng ký asset:// & 42 Tauri IPC commands (10 modules)
│   ├── error/                    # Cây lỗi & mã lỗi hợp nhất (48 mã ERR_*)
│   │   ├── mod.rs                # AppError, IpcErrorResponse, details(), is_recoverable()
│   │   └── codes.rs              # 48 canonical ERR_* constants
│   ├── config/                   # Cấu hình & phần cứng (phần backend sớm của M4-27/M4-28)
│   │   ├── mod.rs
│   │   ├── app_config.rs         # Quản lý cài đặt %APPDATA%/NeuroClip/app_config.json
│   │   ├── atomic.rs             # Ghi cấu hình nguyên tử (Atomic OS Rename + sync_all)
│   │   └── hardware.rs           # Probe CPU, RAM, GPU VRAM (DXGI / Registry)
│   ├── logging/                  # Ghi log cấu trúc & che giấu secrets (re-export qua core::logging)
│   │   ├── mod.rs                # Tracing NDJSON subscriber & DEFAULT_LOG_FILTER
│   │   ├── rotation.rs           # Daily log rotation & retention policy (14 ngày, <= 50MB)
│   │   └── secrets.rs            # Redact API keys & OAuth tokens ([REDACTED])
│   ├── core/                     # Hạ tầng dùng chung của hệ thống
│   │   ├── mod.rs                # Export storage, custom_protocol & re-export logging
│   │   ├── storage.rs            # Pre-flight disk check (5GB), thư mục cache/projects, validate_vod_id
│   │   ├── custom_protocol.rs    # Giao thức asset:// bất đồng bộ hỗ trợ HTTP 206 Range
│   │   └── [keyring.rs]          # (Target M4-28) Windows Credential Locker + CI In-memory fallback
│   ├── sidecar/                  # Quản trị công cụ phụ trợ & kill-switch (ADR-007)
│   │   ├── mod.rs
│   │   ├── supervisor.rs         # SubprocessSupervisor, bounded 1,000-line channel, sidecar:progress
│   │   ├── kill_switch.rs        # ChildProcessGuard & WindowsJobObjectGuard (JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE)
│   │   └── [updater.rs]          # (Target M4-29) Tự động cập nhật yt-dlp từ GitHub Release + SHA256
│   ├── ingestion/                # Thu nạp VOD, chat replay và video cục bộ (ADR-003)
│   │   ├── mod.rs
│   │   ├── streamer.rs           # yt-dlp audio stream extraction (~400MB), VodMetadata, ingest:progress
│   │   ├── cookies.rs            # Nạp cookies trình duyệt / cookies.txt & User-Agent rotation
│   │   ├── buffer.rs             # BoundedAudioQueue giới hạn RAM
│   │   ├── validator.rs          # AudioStreamValidator kiểm tra luồng & header
│   │   ├── local_importer.rs     # Nạp file OBS nội bộ (.mp4/.mkv/.webm/.mov/.ts/.flv) & trích PCM WAV
│   │   ├── range_slicer.rs       # FFmpeg input-seeking HTTP Range slicing (-ss / -to -c copy)
│   │   ├── chat_parser.rs        # Chuẩn hóa ChatEventsContainer & bộ lọc emote Swarm
│   │   ├── twitch_gql.rs         # Tải Twitch GQL chat replay theo cursor pagination
│   │   └── youtube_chat.rs       # Bóc tách YouTube Live Chat qua yt-dlp
│   ├── highlight/                # Động cơ tính điểm Highlight đa phương thức (ADR-004)
│   │   ├── mod.rs
│   │   ├── bucket.rs             # Gom bucket cửa sổ thời gian cố định 5s
│   │   ├── zscore.rs             # Sliding window Z-score + Message floor (X >= 15) + spam filter
│   │   ├── emote_filter.rs       # Tokenizer & tỷ lệ mật độ Emote Swarm (E_ratio)
│   │   ├── swarm_dict.rs         # Từ điển FNV-1a zero-allocation các biểu cảm Swarm
│   │   ├── audio_rms.rs          # Tính toán RMS 500ms & chuẩn hóa dBFS
│   │   ├── energy_analyzer.rs    # Baseline trượt 30s & phát hiện đỉnh biên độ âm thanh
│   │   ├── fusion.rs             # Hợp nhất đa phương thức & bù trễ stream (-4.5s - 10s pre-roll)
│   │   └── cluster.rs            # Phân cụm 30s, NMS <= 50% & Dynamic Padding cuối VOD
│   ├── stt/                      # Phân hệ bóc băng giọng nói (ADR-001, ADR-008)
│   │   ├── mod.rs
│   │   ├── audio_prep.rs         # FFmpeg EBU R128 loudnorm + soft-knee limiter (ERR-08 defense)
│   │   ├── vad.rs                # Pure Rust tract-onnx Silero VAD 16kHz + DSP fallback
│   │   ├── vad_bridge.rs         # Cầu nối căn chỉnh speech intervals & natural pause chunking
│   │   ├── groq_client.rs        # HTTP client Groq Whisper Large-v3 (<20MB chunks, stt:progress)
│   │   ├── faster_whisper_worker.rs # Worker local Faster-Whisper INT8 + VRAM guardrail
│   │   ├── whisper_sidecar.rs    # Sidecar whisper.cpp Q5_K GGUF (bổ sung sớm từ M4-08 legacy)
│   │   ├── model_manager.rs      # Danh mục mô hình & kiểm tra toàn vẹn SHA-256 streaming
│   │   ├── transcript_parser.rs  # Chuẩn hóa Schema 5.5 transcript_segments.json & CPS
│   │   └── subtitle_export.rs    # Facade tương thích ngược re-export từ crate::export (bắt buộc giữ nguyên vì suite.node.test.ts kiểm tra)
│   ├── translation/              # Phân hệ dịch thuật & phân vai (ADR-001, ADR-006)
│   │   ├── mod.rs
│   │   ├── gemini_client.rs      # Google Gemini 1.5 Flash client (BLOCK_NONE, ERR-07 fallback)
│   │   ├── prompt_builder.rs     # Đóng gói Prompt Theatrical Framing & Sliding Window
│   │   ├── lore_dictionary.rs    # Từ điển 12 thuật ngữ Swarm Lore
│   │   ├── pronoun_matrix.rs     # Ma trận đại từ xưng hô nhân vật
│   │   ├── diarization.rs        # Hybrid context diarization + deterministic normalizer
│   │   ├── line_wrapper.rs       # Ngắt dòng bảo toàn từ ghép tiếng Việt
│   │   └── cps_optimizer.rs      # Tối ưu CPS & Time Borrowing
│   ├── export/                   # Xuất bản NLE & Phụ đề (ADR-005, M4-24..M4-25; Target M4-26)
│   │   ├── mod.rs                # SubtitleProject, SpeakerStyle, UTF8_BOM, write_file_with_bom
│   │   ├── ass_exporter.rs       # Xuất .ass UTF-8 with BOM theo target_layout (subtitle_export.rs re-export từ đây)
│   │   ├── srt_exporter.rs       # Xuất .srt UTF-8 with BOM
│   │   ├── capcut_draft.rs       # Sinh CapCut Draft JSON + Dual Export Safeguard
│   │   ├── capcut_schema.rs      # DraftContent, DraftMetaInfo, canvas_config_for_layout
│   │   └── [premiere_xml.rs]     # (Target M4-26) Sinh Premiere Pro / FCP7 XML (<xmeml version="4">)
│   └── commands/                 # Cổng tiếp nhận 42 Tauri IPC Commands (10 modules)
│       ├── mod.rs
│       ├── config_cmd.rs         # ping, get_app_config, save_app_config, get_hardware_profile, detect_hardware
│       ├── log_cmd.rs            # get_recent_logs, clear_logs, log_frontend_event
│       ├── storage_cmd.rs        # check_preflight_storage, initialize_storage, initialize_project, clean_expired_cache
│       ├── ingest_cmd.rs         # start_vod_ingestion, import_local_video, cancel_vod_ingestion, probe_vod, get_vod_manifest, extract_pcm_stream
│       ├── chat_cmd.rs           # start_chat_extraction, get_chat_events, get_chat_velocity_heatmap
│       ├── stt_cmd.rs            # start_whisper_transcription, start_faster_whisper_transcription, transcribe_highlight_clip, verify_stt_model (kèm re-export tương thích từ export_cmd)
│       ├── highlight_cmd.rs      # detect_highlights, get_highlight_clusters, toggle_highlight_bookmark, toggle_cluster_bookmark
│       ├── slice_cmd.rs          # slice_video_segment, cancel_slice_video_segment
│       ├── translation_cmd.rs    # translate_vietsub, test_gemini_connection, update_speaker_role, wrap_vietnamese_subtitles, optimize_transcript_cps
│       └── export_cmd.rs         # export_subtitles_ass, export_subtitles_srt, export_subtitle_project, export_subtitles_project_ass, export_subtitles_project_srt, export_capcut_draft (Target M4-26: export_premiere_xml & export:progress)
└── tests/                        # 24 Integration & Adversarial Rust test files + fixtures/silero_test.wav
```

---

## 3. QUY CHUẨN FRONTEND TYPESCRIPT (REACT 18/19 + VITE)

### 3.1 Cấu Hình TypeScript Strict Mode & Compile Invariants

File `tsconfig.json` bắt buộc kích hoạt chế độ nghiêm ngặt cao nhất:
```json
{
  "compilerOptions": {
    "target": "ES2022",
    "lib": ["DOM", "DOM.Iterable", "ES2022"],
    "module": "ESNext",
    "moduleResolution": "bundler",
    "jsx": "react-jsx",
    "strict": true,
    "noImplicitAny": true,
    "strictNullChecks": true,
    "strictFunctionTypes": true,
    "strictBindCallApply": true,
    "strictPropertyInitialization": true,
    "noImplicitThis": true,
    "alwaysStrict": true,
    "exactOptionalPropertyTypes": true,
    "noUncheckedIndexedAccess": true,
    "noImplicitReturns": true,
    "noFallthroughCasesInSwitch": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "allowSyntheticDefaultImports": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "baseUrl": ".",
    "paths": {
      "@/*": ["src/*"]
    },
    "types": ["vitest/globals", "@testing-library/jest-dom"],
    "noEmit": true,
    "allowImportingTsExtensions": true
  },
  "include": ["src", "vite.config.ts"]
}
```
**Quy tắc:**
- Tuyệt đối cấm sử dụng kiểu `any`. Trong trường hợp dữ liệu chưa xác định rõ cấu trúc, sử dụng `unknown` kèm type guards.
- Khi truy cập phần tử mảng theo index (do bật `noUncheckedIndexedAccess`), phải luôn kiểm tra `if (item !== undefined)`.

### 3.2 Linter & Code Formatter (ESLint 9 Flat Config + Prettier)

- **ESLint 9 (`eslint.config.js`):**
  - Cấm khai báo biến không dùng (`@typescript-eslint/no-unused-vars` với ngoại lệ tiền tố `_`).
  - Kiểm tra tính tuân thủ quy tắc React Hooks (`eslint-plugin-react-hooks`).
  - Cấm type assertion nguy hiểm (`as any`).
- **Prettier (`.prettierrc`):**
  ```json
  {
    "semi": true,
    "singleQuote": true,
    "trailingComma": "all",
    "tabWidth": 2,
    "printWidth": 100,
    "endOfLine": "auto",
    "plugins": ["prettier-plugin-tailwindcss"]
  }
  ```

### 3.3 Cấu Trúc Thư Mục Chuẩn `src/`

*(Ghi chú: Cây thư mục dưới đây phản ánh cấu trúc thực tế đã triển khai qua `M4-01..M4-25` cùng các mục `[target M4-26..M4-29]` thuộc kiến trúc đích của Phase 4 — tuyệt đối không xem các file chưa tới milestone là thiếu sót của `M4-01..M4-25`.)*

```
index.html                        # Entry HTML gốc tại repo root
vite.config.ts                    # Cấu hình Vite tại repo root
tailwind.config.js                # Tailwind tokens & bảng màu PO Option 1 tại repo root
src/
├── main.tsx                      # Root mount bọc <GlobalErrorBoundary><App /></GlobalErrorBoundary>
├── App.tsx                       # Acrylic Dark Shell + thanh chuyển 4 tabs (overview / ingestion / highlights / studio)
├── App.test.tsx
├── components/                   # React UI components
│   ├── common/                   # ErrorBoundary.tsx (GlobalErrorBoundary), ErrorBanner.tsx & tests
│   ├── dashboard/                # IngestionDashboard.tsx, DropZone.tsx, DropZone.test.tsx
│   ├── highlight/                # HighlightViewer.tsx, index.ts
│   ├── [layout/]                 # (Target Phase 4) Header, Sidebar, WindowTitleBar, StatusBar
│   ├── [onboarding/]             # (Target M4-27/M4-28) HardwareRecommendation, OnboardingWizardModal, ApiKeyTester
│   ├── studio/                   # SubtitleStudio, VideoCanvas, TimelineScrubber, VirtualizedSegmentList, SegmentRow, TimecodeNudge, CharacterHotkeyBar, CPSIndicator & tests
│   └── export/                   # ExportModal.tsx, ExportModal.test.tsx (CapCut Draft, .ass, .srt; Target M4-26 bổ sung Premiere XML & progress bar)
├── hooks/                        # Custom React hooks (useStudioHotkeys.ts, useStudioHotkeys.test.ts)
├── stores/                       # Quản lý trạng thái client (createStore / Zustand-compatible API)
│   ├── createStore.ts            # Store factory dùng React.useSyncExternalStore với API tương thích Zustand
│   ├── createStore.test.ts       # Unit tests kiểm chứng subscribe, selector memoization & curried API
│   ├── settingsStore.ts          # API keys, cấu hình app & hồ sơ người dùng (exports useSettingsStore)
│   ├── useProjectStore.ts        # Project workspace & preflight storage state
│   ├── useIngestionStore.ts      # Trạng thái thu nạp VOD / local import & ingest:progress
│   ├── useChatStore.ts           # Dữ liệu chat events, heatmap & chat:fallback
│   ├── useHighlightStore.ts      # Highlight clusters & bookmark state
│   ├── useTranscriptStore.ts     # STT (Groq / Faster-Whisper), translation & speaker role state
│   ├── useLogStore.ts            # Ring-buffer log viewer (1,000 dòng FIFO)
│   ├── useStudioStore.ts         # Trạng thái video playback, layout (aspectRatio, preset916) & timeline sync
│   └── [useHardwareStore.ts]     # (Target M4-27) Trạng thái hardware profile onboarding
├── services/                     # Giao tiếp Backend IPC, cấu hình & logging
│   ├── ipc/                      # Wrapper Tauri IPC có kiểu tĩnh (zero-any)
│   │   ├── error.ts              # 48 mã lỗi ERR_* (48/48 parity với codes.rs) & ERROR_MESSAGES_VI
│   │   ├── sidecar.ts            # Listener sidecar:progress (snake_case & camelCase)
│   │   ├── storage.ts            # check_preflight_storage, initialize_storage, initialize_project, clean_expired_cache
│   │   ├── ingestion.ts          # start_vod_ingestion, import_local_video, cancel_vod_ingestion, probe_vod, ...
│   │   ├── chat.ts               # start_chat_extraction, get_chat_events, get_chat_velocity_heatmap, chat:fallback
│   │   ├── highlight.ts          # detect_highlights, get_highlight_clusters, toggle_*_bookmark
│   │   ├── slice.ts              # slice_video_segment, cancel_slice_video_segment
│   │   ├── stt.ts                # transcribe_highlight_clip, start_faster_whisper_transcription, stt:progress, stt:chunk
│   │   ├── translation.ts        # translate_vietsub, test_gemini_connection, update_speaker_role, wrap_vietnamese_subtitles, optimize_transcript_cps, translation:progress
│   │   └── export.ts             # exportSubtitlesAss, exportSubtitlesSrt, exportSubtitleProject, exportSubtitlesDual, exportSubtitlesProjectAss, exportSubtitlesProjectSrt, exportCapCutDraft (Target M4-26: exportPremiereXml)
│   ├── config/                   # hardware.ts, settings.ts
│   └── logging/                  # index.ts, logger.ts, redactor.ts, types.ts
├── types/                        # Khai báo TypeScript types (khớp Data Contracts)
│   ├── chat.ts
│   ├── export.ts                 # SubtitleExportFormat, TargetLayout, CapCutExportOptions/Result, VTUBER_PALETTE_*
│   ├── highlight.ts
│   ├── range_slicer.ts
│   ├── transcript.ts             # Schema 5.5/5.6, SpeakerRole, GuardrailPolicy, SttProgressPayload
│   └── translation.ts            # TranslateVietsubOptions, TranslationProgressPayload
├── utils/                        # Các hàm thuần túy (text_wrap.ts, cps.ts; formatStudioTimecode trong stores/useStudioStore.ts)
├── styles/
│   ├── globals.css               # Tailwind directives, custom scrollbars, dark VTuber theme
│   ├── theme.css                 # CSS variables cho bảng màu PO Option 1
│   └── theme.ts                  # Typed theme palette tokens (Neuro #FFB6C1, Evil #FF0055, Vedal #98FB98, Guest #FFD700, Chat #87CEEB)
└── test/                         # setup.ts, typecheck.node.ts, suite.node.test.ts & Vitest suites
```

- **Kiến trúc State Management (`createStore` & Lộ trình nâng cấp `zustand`):**
  - Toàn bộ 8 store hiện tại (`settingsStore.ts`, `useProjectStore.ts`, `useIngestionStore.ts`, `useChatStore.ts`, `useHighlightStore.ts`, `useTranscriptStore.ts`, `useLogStore.ts`, `useStudioStore.ts`) được xây dựng trên `src/stores/createStore.ts` sử dụng `React.useSyncExternalStore` với **API tương thích hoàn toàn với Zustand** (`createStore<T>(initializer)`, curried `createStore<T>()(initializer)`, `create` alias, `useStore(selector?, equalityFn?)` kèm snapshot memoization, `getState()`, `getInitialState()`, `setState(partial, replace?)`, `subscribe(listener)`).
  - Kiến trúc này **chạy ổn định cho `M4-01..M4-25` không cần cài gói `zustand` rời**, đồng thời sẵn sàng tương thích nếu sau này chốt cài đặt gói `zustand` trong `package.json`, mà không làm thay đổi chữ ký API của các store hoặc component đang sử dụng.

### 3.4 Quy Ước Đặt Tên Toàn Diện (Naming Conventions)

| Đối tượng | Quy ước | Ví dụ |
|---|---|---|
| **React Component** | `PascalCase` | `SubtitleStudio.tsx`, `HighlightViewer.tsx` |
| **Custom Hook** | `camelCase` (bắt đầu bằng `use`) | `useStudioHotkeys.ts`, `useVideoPlayback.ts` |
| **State Store** | `use<Domain>Store.ts` (hoặc `<domain>Store.ts`) | `useTranscriptStore.ts`, `settingsStore.ts`, `useStudioStore.ts` |
| **Tauri IPC Command** | `snake_case` (cả Rust và TS invoke) | `start_vod_ingestion`, `start_faster_whisper_transcription` |
| **Tauri Event Name** | `kebab-colon` (`<domain>:<action>`) | `ingest:progress`, `stt:progress`, `stt:chunk`, `sidecar:progress` |
| **TypeScript Type/Interface** | `PascalCase` | `HardwareProfile`, `TranscriptSegment` |
| **TypeScript Enum / Const** | `SCREAMING_SNAKE_CASE` | `ERROR_CODES`, `VALID_SPEAKER_ROLES` |
| **File tiện ích / Utils** | `camelCase` hoặc `snake_case` (nhất quán theo module) | `cps.ts`, `text_wrap.ts`, `range_slicer.ts` |
| **Tailwind Class Names** | Utility classes / kebab-case | `flex items-center justify-between bg-bg-secondary` |

### 3.5 Hiệu Năng Timeline & Virtualized Subtitle Rendering

Trong các VOD dài 4–8 tiếng, số lượng phân đoạn phụ đề có thể lên tới 500–2,000 dòng. Để đảm bảo UI phản hồi tức thì với tốc độ 60fps:
1. **Virtualized Segment List:** Danh sách dòng phụ đề trong `SubtitleStudio` **BẮT BUỘC** sử dụng kỹ thuật ảo hóa danh sách (custom `useVirtualizer` trong `VirtualizedSegmentList.tsx`). Chỉ render các dòng hiện diện trong viewport của bảng cuộn.
2. **Binary Search & Multi-segment Lookup cho Active Subtitle Rendering:**
   Khi video đang phát, con trỏ thời gian `currentTime` phát ra liên tục 60 lần/giây. Để hỗ trợ trọn vẹn các tình huống hội thoại nói đè lên nhau (Overlapping dialogue / banter đặc trưng giữa Neuro-sama và Vedal hoặc khách mời / Evil-Neuro):
   - Thuật toán tra cứu phụ đề **BẮT BUỘC trả về một mảng `SubtitleSegment[]`** (danh sách toàn bộ các phân đoạn đang kích hoạt tại mốc thời gian $t$), thay vì chỉ trả về một giá trị vô hướng đơn lẻ `SubtitleSegment`.
   - **Quy trình tra cứu tối ưu $O(\log N + K)$:**
     * **Bước 1:** Sử dụng Binary Search trên mảng phụ đề đã sắp xếp theo `start_sec` để định vị chỉ mục phân đoạn bắt đầu gần nhất với mốc thời gian phát $t$ trong thời gian $O(\log N)$.
     * **Bước 2 (K-nearest Active Scan):** Thực hiện quét lân cận sang trái và phải tính từ điểm tìm được để thu thập toàn bộ $K$ phân đoạn đồng thời thỏa mãn điều kiện:
       $$\text{start\_sec} \le t \le \text{end\_sec}$$
     * **Bước 3:** Trả về mảng `activeSegments: SubtitleSegment[]` (hoặc `TranscriptSegment[]`).
   - **Hiển thị xếp chồng độc lập (Stacked Overlay):** Video Canvas / Subtitle Overlay render đồng thời tất cả các dòng phụ đề có trong `activeSegments`, áp dụng đúng bảng màu PO Option 1 theo từng `speaker_id` (`vedal`: `#98FB98` / `&H0098FB98&`, `neuro`: `#FFB6C1` / `&H00C1B6FF&`, `evil`: `#FF0055` / `&H005500FF&`, `guest`: `#FFD700` / `&H0000D7FF&`, `chat`: `#87CEEB` / `&H00EBCE87&`, `unknown`: `#FFFFFF` / `&H00FFFFFF&`) mà không bị nuốt mất thoại của bất kỳ ai.
3. **Throttling trên Playback Seeker:** Sự kiện kéo thanh tua video (time scrubber slider) phải được throttle qua `requestAnimationFrame` (hoặc debounce 16ms) để tránh làm nghẽn luồng render của Canvas/Video Element.
4. **React.memo trên Segment Row:** Component `SegmentRow` phải được bọc trong `React.memo` với hàm so sánh tùy chỉnh kiểm tra: `segment_id`, `start_sec`, `end_sec`, `text_vi`, `speaker_id` và `isActive`.

---

## 4. GIAO THỨC TRUYỀN THÔNG LIÊN TIẾN TRÌNH IPC (TAURI COMMANDS & EVENTS)

### 4.1 Quy Ước Định Danh & Chữ Ký Tauri Command

- **Tên Command:** Đồng nhất viết theo dạng `snake_case` ở cả Rust backend và TypeScript frontend.
- **42 Tauri Commands đã đăng ký trong `src-tauri/src/lib.rs` (10 modules):**
  - `config_cmd`: `ping`, `get_app_config`, `save_app_config`, `get_hardware_profile`, `detect_hardware`
  - `log_cmd`: `get_recent_logs`, `clear_logs`, `log_frontend_event`
  - `storage_cmd`: `check_preflight_storage`, `initialize_storage`, `initialize_project`, `clean_expired_cache`
  - `ingest_cmd`: `start_vod_ingestion`, `import_local_video`, `cancel_vod_ingestion`, `probe_vod`, `get_vod_manifest`, `extract_pcm_stream`
  - `chat_cmd`: `start_chat_extraction`, `get_chat_events`, `get_chat_velocity_heatmap`
  - `stt_cmd`: `start_whisper_transcription`, `start_faster_whisper_transcription`, `transcribe_highlight_clip`, `verify_stt_model` *(kèm re-export tương thích `export_subtitles_ass`, `export_subtitles_srt` từ `export_cmd`)*
  - `highlight_cmd`: `detect_highlights`, `get_highlight_clusters`, `toggle_highlight_bookmark`, `toggle_cluster_bookmark`
  - `slice_cmd`: `slice_video_segment`, `cancel_slice_video_segment`
  - `translation_cmd`: `translate_vietsub`, `test_gemini_connection`, `update_speaker_role`, `wrap_vietnamese_subtitles`, `optimize_transcript_cps`
  - `export_cmd`: `export_subtitles_ass`, `export_subtitles_srt`, `export_subtitle_project`, `export_subtitles_project_ass`, `export_subtitles_project_srt`, `export_capcut_draft` *(Target M4-26: `export_premiere_xml`)*
- **Kiểu trả về chuẩn:** Mọi hàm command phía Rust phải trả về `Result<T, AppError>`.
- **Ví dụ chữ ký Command:**
  ```rust
  // Rust Backend (src-tauri/src/commands/ingest_cmd.rs)
  #[tauri::command]
  pub async fn start_vod_ingestion(
      vod_url: String,
      options: Option<IngestOptions>,
      task_id: Option<String>,
      app: tauri::AppHandle,
  ) -> Result<VodManifest, AppError> {
      // Logic xử lý...
  }
  ```
  ```typescript
  // TypeScript Frontend Service (src/services/ipc/ingestion.ts)
  export async function startVodIngestion(
    vodUrl: string,
    options?: IngestOptions,
    taskId?: string,
  ): Promise<VodManifest> {
    return await invoke<VodManifest>('start_vod_ingestion', {
      vodUrl,
      options: options ?? null,
      taskId: taskId ?? null,
    });
  }
  ```

### 4.2 Hệ Thống Streaming Sự Kiện Bất Đồng Bộ (Event Streaming)

Các tiến trình dài hạn (tải audio, tính highlight, STT, dịch thuật, export) không bắt UI phải chờ đợi lệnh `invoke` kết thúc mà liên tục phát sóng tiến độ về giao diện qua kênh sự kiện (Tauri Event Channel).

- **Định dạng tên Event:** `<domain>:<action>`
- **Danh mục Sự Kiện Chuẩn (khớp chính xác JSON wire payload hiện hành & Phase 4 targets):**

| Tên Event | Wire JSON Payload (Exact Serde Keys) | Trạng thái & Mô tả |
|---|---|---|
| `sidecar:progress` | `{ task_id: string, percentage: number, current_fps: number \| null, eta_sec: number \| null }` *(snake_case; `sidecar.ts` nhận cả camelCase)* | **Đã triển khai (`sidecar/supervisor.rs`):** Tiến độ FFmpeg / yt-dlp sidecar |
| `ingest:progress` | `{ taskId: string, stage: string, percent: number, speedKbps: number, etaSec: number }` *(camelCase)* | **Đã triển khai (`ingestion/streamer.rs`, `ingest_cmd.rs`):** Tiến độ thu nạp stream / trích PCM |
| `chat:progress` | `{ taskId: string, stage: string, percent: number, messagesExtracted: number, currentOffsetSec: number }` *(camelCase)* | **Đã triển khai (`ingestion/twitch_gql.rs`, `chat_cmd.rs`):** Tiến độ tải chat replay |
| `chat:fallback` | `{ taskId: string, code: string, message: string, audioFallbackActive: boolean }` *(camelCase)* | **Đã triển khai (`commands/chat_cmd.rs`):** Thông báo hạ cấp sang Audio RMS khi chat lỗi |
| `slice:progress` | `{ taskId: string, stage: string, percent: number }` *(camelCase)* | **Đã triển khai (`commands/slice_cmd.rs`):** Tiến độ cắt range slicing |
| `stt:progress` | `{ taskId: string, stage: string, percent: number, etaSec?: number \| null, currentChunk: number, totalChunks: number }` *(camelCase)* | **Đã triển khai (`stt/whisper_sidecar.rs`, `commands/stt_cmd.rs`):** Tiến độ bóc băng Groq / Local Whisper / Faster-Whisper |
| `stt:chunk` | `{ clip_id: string, chunk_index: number, segment: TranscriptSegment }` *(snake_case; `stt.ts` nhận cả camelCase)* | **Đã triển khai (`commands/stt_cmd.rs`):** Stream từng phân đoạn thoại bóc băng xong tức thì |
| `translation:progress` | `{ clipId: string, translatedCount: number, totalCount: number }` *(camelCase)* | **Đã triển khai (`translation/mod.rs`, `translation_cmd.rs`):** Tiến độ dịch Vietsub theo lô |
| `highlight:progress` | `{ taskId: string, stage: 'chat_parsing' \| 'audio_rms' \| 'clustering', percent: number }` | *(Target Phase 4 — hiện `detect_highlights` chạy đồng bộ trên `spawn_blocking`)* |
| `export:progress` | `{ projectId: string, percent: number, currentStep: string }` | *(Target M4-26 — tiến độ xuất NLE & progress bar)* |
| `sidecar:status` | `{ binary: 'yt-dlp' \| 'ffmpeg', status: 'ready' \| 'updating' \| 'failed', message?: string }` | *(Target M4-29 — trạng thái tự cập nhật sidecar)* |

- **Frontend Hook Quản Lý Event (`useTauriEvent`):**
  Frontend phải sử dụng hook chuẩn để tự động hủy đăng ký (unlisten) khi component bị unmount, ngăn chặn triệt để hiện tượng rò rỉ bộ nhớ (memory leaks):
  ```typescript
  export function useTauriEvent<T>(
    eventName: string,
    handler: (payload: T) => void,
  ) {
    useEffect(() => {
      let unlistenFn: (() => void) | undefined;
      let isMounted = true;

      listen<T>(eventName, (event) => {
        if (isMounted) handler(event.payload);
      }).then((unlisten) => {
        unlistenFn = unlisten;
      });

      return () => {
        isMounted = false;
        if (unlistenFn) unlistenFn();
      };
    }, [eventName, handler]);
  }
  ```

### 4.3 Giao Kèo Serialization Dữ Liệu Đồng Bộ

- Toàn bộ các struct dữ liệu trao đổi qua IPC (hoặc ghi xuống disk) phải tương thích 100% với các JSON Schemas tại Section 5 của `ARCHITECTURE.md`.
- Sử dụng thuộc tính `#[serde(rename_all = "snake_case")]` (hoặc định danh tường minh tương ứng) trên các struct Rust để các trường dữ liệu như `start_sec`, `speaker_id`, `chat_z_score` khớp hoàn toàn giữa Rust và TypeScript.

---

## 5. GIAO THỨC XỬ LÝ LỖI TOÀN HỆ THỐNG (ERROR HANDLING PROTOCOL)

### 5.1 Cây Lỗi Rust Phân Cấp Dùng `thiserror`

Hệ thống triển khai cây lỗi thống nhất tại **`src-tauri/src/error/mod.rs`** và danh mục 48 hằng số mã lỗi tại **`src-tauri/src/error/codes.rs`** (đồng bộ 48/48 với `src/services/ipc/error.ts`):
```rust
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum AppError {
    #[error("Lỗi phần cứng: {0}")]
    Hardware(#[from] HardwareError),

    #[error("Lỗi thu nạp VOD: {0}")]
    Ingestion(#[from] IngestError),

    #[error("Lỗi tính toán highlight: {0}")]
    Highlight(#[from] HighlightError),

    #[error("Lỗi bóc băng STT: {0}")]
    Stt(#[from] SttError),

    #[error("Lỗi dịch thuật: {0}")]
    Translation(#[from] TranslationError),

    #[error("Lỗi xuất bản NLE: {0}")]
    Export(#[from] ExportError),

    #[error("Lỗi công cụ phụ trợ Sidecar: {0}")]
    Sidecar(#[from] SidecarError),

    #[error("Lỗi lưu trữ ổ đĩa: {0}")]
    Storage(#[from] StorageError),

    #[error("Lỗi cấu hình cài đặt: {0}")]
    Config(#[from] ConfigError),

    #[error("Tác vụ đã bị người dùng hủy")]
    TaskCancelled,

    #[error("Lỗi hệ thống không xác định: {0}")]
    Internal(String),
}
```
Các sub-enum (`IngestError`, `SttError`, `TranslationError`, `ExportError`, `HighlightError`, `SidecarError`, `HardwareError`, `StorageError`, `ConfigError`) được định nghĩa đầy đủ tại `src-tauri/src/error/mod.rs` với `AppError::code()`, `AppError::is_recoverable()` và `AppError::details()`.

### 5.2 Cấu Trúc Vỏ Bọc Lỗi Tuần Tự Hóa IPC & 48 Mã Lỗi Canonical (`codes.rs` ↔ `error.ts`)

Khi trả lỗi về phía TypeScript qua Tauri IPC, `AppError` được tuần tự hóa thành struct `IpcErrorResponse` (với `code: self.code().to_string()`, `message: self.to_string()`, `details: self.details()`, `recoverable: self.is_recoverable()`):
```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IpcErrorResponse {
    pub code: String,             // Mã lỗi định danh, ví dụ: "ERR_INGEST_DISK_FULL"
    pub message: String,          // Thông điệp thân thiện cho người dùng (tiếng Việt)
    pub details: Option<String>,  // Chi tiết kỹ thuật phục vụ debug (AppError::details())
    pub recoverable: bool,        // Cờ báo hiệu lỗi có thể phục hồi (AppError::is_recoverable())
}
```

**Danh mục Đầy đủ 48 Mã Lỗi Canonical (Parity 48/48 giữa `src-tauri/src/error/codes.rs` và `src/services/ipc/error.ts`):**

| # | Mã Lỗi (`codes.rs` / `error.ts`) | Biến thể Rust `AppError` | `recoverable` |
|---:|---|---|---|
| 1 | `ERR_INGEST_DISK_FULL` | `IngestError::DiskSpaceInsufficient` | `true` |
| 2 | `ERR_INGEST_BOT_BLOCKED` | `IngestError::BotBlocked` | `true` |
| 3 | `ERR_INGEST_CHAT_UNAVAILABLE` | `IngestError::ChatReplayUnavailable` | `true` |
| 4 | `ERR_INGEST_NETWORK_TIMEOUT` | `IngestError::NetworkStreamDropped` | `true` |
| 5 | `ERR_INGEST_INVALID_URL` | `IngestError::InvalidUrl` | `false` |
| 6 | `ERR_INGEST_EXTRACTION_FAILED` | `IngestError::ExtractionFailed` | `true` |
| 7 | `ERR_INGEST_LIVE_STREAM_UNSUPPORTED` | `IngestError::LiveStreamUnsupported` | `false` |
| 8 | `ERR_INGEST_SLICE_FAILED` | `IngestError::SliceFailed` | `true` |
| 9 | `ERR_INGEST_KEYFRAME_ALIGNMENT_FAILED` | `IngestError::KeyframeAlignmentFailed` | `false` |
| 10 | `ERR_INGEST_INVALID_TIME_RANGE` | `IngestError::InvalidTimeRange` | `false` |
| 11 | `ERR_STT_RATE_LIMIT` | `SttError::RateLimitExceeded` | `true` |
| 12 | `ERR_STT_PAYLOAD_TOO_LARGE` | `SttError::PayloadTooLarge` | `false` |
| 13 | `ERR_STT_VRAM_INSUFFICIENT` | `SttError::InsufficientVram` | `true` |
| 14 | `ERR_STT_CUDA_OOM` | `SttError::CudaOom` | `true` |
| 15 | `ERR_STT_INVALID_API_KEY` | `SttError::InvalidApiKey` | `false` |
| 16 | `ERR_STT_VAD_FAILED` | `SttError::VadProcessingFailed` | `true` |
| 17 | `ERR_STT_MODEL_NOT_FOUND` | `SttError::ModelNotFound` | `true` |
| 18 | `ERR_STT_MODEL_INTEGRITY_FAILED` | `SttError::ModelIntegrityFailed` | `true` |
| 19 | `ERR_STT_SIDECAR_FAILED` | `SttError::SidecarExecutionFailed` | `true` |
| 20 | `ERR_STT_PARSER_FAILED` | `SttError::ParserFailed` | `true` |
| 21 | `ERR_STT_INVALID_TIMESTAMP` | `SttError::InvalidTimestamp` | `true` |
| 22 | `ERR_TRANSLATE_SAFETY_FILTER` | `TranslationError::SafetyFilterTriggered` | `true` |
| 23 | `ERR_TRANSLATE_INVALID_API_KEY` | `TranslationError::InvalidApiKey` | `false` |
| 24 | `ERR_TRANSLATE_API_FAILED` | `TranslationError::ApiRequestFailed` | `true` |
| 25 | `ERR_EXPORT_CAPCUT_NOT_FOUND` | `ExportError::CapCutDirectoryNotFound` | `true` |
| 26 | `ERR_EXPORT_FILE_WRITE_FAILED` | `ExportError::FileWriteFailed` | `true` |
| 27 | `ERR_EXPORT_INVALID_LAYOUT` | `ExportError::InvalidLayout` | `false` |
| 28 | `ERR_HIGHLIGHT_CHAT_PARSING_FAILED` | `HighlightError::ChatParsingFailed` | `true` |
| 29 | `ERR_HIGHLIGHT_AUDIO_ANALYSIS_FAILED` | `HighlightError::AudioAnalysisFailed` | `true` |
| 30 | `ERR_HIGHLIGHT_NO_HIGHLIGHTS` | `HighlightError::NoHighlightsFound` | `false` |
| 31 | `ERR_HIGHLIGHT_WORKER_PANICKED` | `HighlightError::WorkerPanicked` | `false` |
| 32 | `ERR_SIDECAR_BINARY_NOT_FOUND` | `SidecarError::BinaryNotFound` | `true` |
| 33 | `ERR_SIDECAR_EXECUTION_FAILED` | `SidecarError::ExecutionFailed` | `true` |
| 34 | `ERR_SIDECAR_UPDATE_FAILED` | `SidecarError::UpdateFailed` | `true` |
| 35 | `ERR_SIDECAR_PROCESS_KILLED` | `SidecarError::ProcessKilled` | `false` |
| 36 | `ERR_HARDWARE_PROBE_FAILED` | `HardwareError::ProbeFailed` | `true` |
| 37 | `ERR_HARDWARE_UNSUPPORTED_GPU` | `HardwareError::UnsupportedGpu` | `false` |
| 38 | `ERR_HARDWARE_INSUFFICIENT_MEMORY` | `HardwareError::InsufficientMemory` | `false` |
| 39 | `ERR_STORAGE_DISK_CHECK_FAILED` | `StorageError::DiskCheckFailed` | `true` |
| 40 | `ERR_STORAGE_DIRECTORY_CREATION_FAILED` | `StorageError::DirectoryCreationFailed` | `true` |
| 41 | `ERR_STORAGE_CACHE_CLEANUP_FAILED` | `StorageError::CacheCleanupFailed` | `true` |
| 42 | `ERR_STORAGE_FILE_NOT_FOUND` | `StorageError::FileNotFound` | `false` |
| 43 | `ERR_CONFIG_NOT_FOUND` | `ConfigError::NotFound` | `true` |
| 44 | `ERR_CONFIG_INVALID_FORMAT` | `ConfigError::InvalidFormat` | `false` |
| 45 | `ERR_CONFIG_SAVE_FAILED` | `ConfigError::SaveFailed` | `true` |
| 46 | `ERR_CONFIG_INVALID_VALUE` | `ConfigError::InvalidValue` | `false` |
| 47 | `ERR_TASK_CANCELLED` | `AppError::TaskCancelled` | `false` |
| 48 | `ERR_INTERNAL_ERROR` | `AppError::Internal` | `false` |

### 5.3 Chiến Lược Phục Hồi Lỗi Ở Tầng Frontend (Error Boundaries & Retries)

1. **React Error Boundaries:**
   - Đặt `GlobalErrorBoundary` bọc quanh toàn bộ `App.tsx` để hứng các lỗi render bất ngờ, hiển thị màn hình cứu hộ ("Khôi phục phiên làm việc").
   - Đặt `ModuleErrorBoundary` tại các phân vùng độc lập (`SubtitleStudio`, `HighlightViewer`) để lỗi ở một khung không làm crash toàn bộ app.
2. **Xử Lý Lỗi Tự Phục Hồi (Auto-Recovery & Informational Banners):**
   - **Khi gặp `ERR_INGEST_DISK_FULL`:** Chặn tác vụ tải trước khi thực thi; hiển thị Banner cảnh báo màu vàng/đỏ và mở hộp thoại cho phép người dùng chọn phân vùng hoặc thư mục lưu trữ tạm khác có dung lượng trống $\ge 5\text{GB}$ (`PRD US2.1`, `ADR-003`).
   - **Khi gặp `ERR_INGEST_BOT_BLOCKED`:** Không crash ứng dụng; tự động kích hoạt tiến trình kiểm tra cập nhật `yt-dlp` ngầm và hiển thị Banner hướng dẫn người dùng nạp Cookie (`cookies.txt` hoặc trích xuất từ trình duyệt) trong mục Cài đặt (`PRD US2.1`, `ADR-007`).
   - **Khi gặp `ERR_INGEST_CHAT_UNAVAILABLE`:** Không hiển thị modal đỏ báo lỗi dừng app. Tự động chuyển cấp sang Quét Highlight Năng Lượng Âm Thanh kèm Banner vàng thông tin: *"Chat replay không khả dụng. Đang tự động quét Highlight theo Năng lượng Âm thanh..."* (`PRD US2.2`).
   - **Khi gặp `ERR_INGEST_NETWORK_TIMEOUT`:** Hiển thị Banner cảnh báo mạng chập chờn kèm nút bấm "Thử lại ngay" (Retry), đồng thời tự động kích hoạt Exponential Backoff retry tối đa 5 lần (`ERR-03`).
   - **Khi gặp `ERR_TRANSLATE_SAFETY_FILTER`:** Không làm hỏng cả mẻ dịch 30 câu. Giữ nguyên câu tiếng Anh gốc, gắn tiền tố cờ `[Cần dịch thủ công do bộ lọc an toàn AI]`, đánh dấu hàng đó màu vàng cam trong Subtitle Studio (`PRD US4.1`).
   - **Khi gặp `ERR_STT_RATE_LIMIT`:** Frontend kích hoạt bộ đếm thời gian lùi Exponential Backoff (chờ 5s, 10s, 20s) và tự động thử lại tối đa 3 lần.

---

## 6. TIÊU CHUẨN GHI NHẬT KÝ & GIÁM SÁT HỆ THỐNG (LOGGING & TELEMETRY)

### 6.1 Khung Structured Logging Dùng `tracing`

- Sử dụng hệ sinh thái crate: `tracing`, `tracing-subscriber`, `tracing-appender`.
- **Chế độ Debug (`cfg!(debug_assertions)`):**
  - Ghi ra terminal stdout với định dạng màu sắc trực quan (pretty format), hiển thị tên module và số dòng code.
- **Chế độ Release:**
  - Ghi đồng thời ra stdout (JSON thu gọn) và file nhật ký với cơ chế ghi không chặn (`tracing_appender::non_blocking`).
  - Định dạng file log là JSON có cấu trúc (Structured JSON Lines) để dễ dàng lọc và phân tích lỗi:
    ```json
    {"timestamp":"2026-11-05T08:30:15.123Z","level":"INFO","target":"neuroclip::ingestion","message":"Bắt đầu tải audio VOD","vod_id":"123456","bitrate_kbps":128}
    ```

### 6.2 Phân Định Cấp Độ Log (Log Levels) & Ngữ Cảnh

| Cấp độ | Định nghĩa & Mục đích sử dụng | Ví dụ |
|:---:|---|---|
| **TRACE** | Chi tiết luồng nội bộ cực sâu; chỉ bật khi cần gỡ lỗi chuyên biệt. | Từng chunk stdout của FFmpeg, từng token emote được regex match. |
| **DEBUG** | Thông số kỹ thuật phục vụ lập trình viên trong quá trình phát triển. | Mốc thời gian VAD silence intervals, giá trị Z-score từng bucket 5s, HTTP header sanitized. |
| **INFO** | Các mốc sự kiện quan trọng trong vòng đời ứng dụng và hành vi người dùng. | Khởi động app, bắt đầu/hoàn tất tải VOD, phát hiện 15 highlight, lưu dự án CapCut thành công. |
| **WARN** | Các sự cố bất thường nhưng hệ thống đã tự phục hồi hoặc hạ cấp an toàn. | Chat replay bị 404 chuyển sang Audio Energy, Gemini safety filter kích hoạt giữ câu gốc, API retry lần 2. |
| **ERROR** | Lỗi nghiêm trọng khiến tác vụ hiện tại thất bại và cần can thiệp. | Tràn ổ đĩa C, tiến trình FFmpeg bị crash mã thoát khác 0, API key không hợp lệ, hỏng file project JSON. |

### 6.3 Bảo Mật Tuyệt Đối & Khử Trùng Thông Tin Nhạy Cảm (Secrets Redaction)

**QUY TẮC BẢO MẬT BẤT BIẾN:** Tuyệt đối cấm in các thông tin nhạy cảm sau ra console hoặc file log:
1. Groq Cloud API Key (`gsk_*`)
2. Google Gemini API Key (`AIza*`)
3. OpenAI API Key (`sk-*`)
4. Twitch OAuth Tokens, Session Cookies (`auth-token`)

**Cơ chế khử trùng (Sanitization Layer):**
Triển khai hàm lọc chuỗi hoặc custom tracing layer:
```rust
pub fn redact_secret(key: &str) -> String {
    if key.len() <= 8 {
        return "[REDACTED]".to_string();
    }
    format!("{}[REDACTED]{}", &key[..4], &key[key.len() - 4..])
}

// Khi log cấu hình:
tracing::info!(
    groq_configured = !settings.groq_key.is_empty(),
    groq_preview = %redact_secret(&settings.groq_key),
    "Đã nạp cài đặt API Keys"
);
```

### 6.4 Vị Trí Lưu Trữ, Xoay Vòng & Dọn Dẹp File Nhật Ký

- **Thư mục lưu trữ:** `%LOCALAPPDATA%\NeuroClip\logs\`
  (ví dụ: `C:\Users\<User>\AppData\Local\NeuroClip\logs\`).
- **Quy tắc đặt tên file:** `neuroclip-YYYY-MM-DD.log`.
- **Cơ chế xoay vòng (Daily Rotation):** Sử dụng `tracing_appender::rolling::daily`.
- **Chính sách dọn dẹp (Retention Policy):**
  - Tại mỗi lần khởi động ứng dụng, `StorageManager` trong Rust sẽ quét thư mục logs:
    - Xóa toàn bộ file nhật ký cũ có thời gian khởi tạo **vượt quá 14 ngày**.
    - Nếu tổng dung lượng thư mục logs vượt quá **50MB**, tự động xóa các file log cũ nhất cho tới khi dung lượng còn dưới 30MB.

---

## 7. QUY CHUẨN KIỂM THỬ VÀ BẢO ĐẢM CHẤT LƯỢNG (TESTING & QA)

### 7.1 Unit & Integration Testing Phía Rust

- **Quy tắc tổ chức file:**
  - Unit tests cho các module pure logic (Z-score, Emote parsing, CPS, Timecode) được viết ngay trong file source tương ứng trong module `#[cfg(test)] mod tests { ... }`.
  - Integration tests cho các luồng nghiệp vụ lớn (Subprocess supervisor, CapCut generator, Audio chunker) được đặt trong thư mục `src-tauri/tests/`.
- **Mocking External Services:** Tuyệt đối không gọi thật ra Groq hay Gemini API trong các test case tự động. Sử dụng crate `wiremock` hoặc trait abstraction để mock HTTP responses.
- **Lệnh chạy kiểm tra tiêu chuẩn (từ root repo hoặc `cd src-tauri`):**
  ```powershell
  # Chạy kiểm tra định dạng
  cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check

  # Chạy clippy nghiêm ngặt (giữ nguyên clippy::pedantic, 0 warning)
  cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings

  # Chạy toàn bộ test suites song song (25 active test binaries: 1 lib + 24 integration = 409 tests; [[bin]] test = false)
  cargo test --manifest-path src-tauri/Cargo.toml --all-targets
  ```

### 7.2 Unit & Component Testing Phía Frontend

- **Testing Framework:** Vitest + React Testing Library + `@testing-library/jest-dom`, kết hợp Node.js Native Test Runner (`node --experimental-strip-types --test src/test/suite.node.test.ts`) cho contract & AST assertions.
- **Vị trí file test:** Đặt song song với file component/store/service (vd: `DropZone.test.tsx`, `createStore.test.ts`, `error.test.ts`, `sidecar.test.ts`, `SubtitleStudio.test.tsx`, `VirtualizedSegmentList.test.tsx`, `CharacterHotkeyBar.test.tsx`, `CPSIndicator.test.tsx`, `useStudioHotkeys.test.ts`, `ExportModal.test.tsx`, `export.test.ts`, cùng các target mở rộng `cps.test.ts`).
- **Mục tiêu kiểm thử tối thiểu:**
  - Kiểm thử thuật toán tính điểm CPS và đổi màu cảnh báo đỏ khi CPS > 20.
  - Kiểm thử bộ phím tắt 1-chạm đổi màu nhân vật (Phím 1 -> Vedal `#98FB98`, Phím 2 -> Neuro `#FFB6C1`, Phím 3 -> Evil `#FF0055`, Phím 4 -> Guest `#FFD700`, Phím 5 -> Chat `#87CEEB`).
  - Kiểm thử cơ chế mượn khoảng lặng (Time Borrowing) khi căn chỉnh phụ đề.
- **Lệnh chạy kiểm tra phía frontend (`package.json`):**
  ```powershell
  # Kiểm tra TypeScript strict + zero-any invariant
  npm run typecheck         # tsc --noEmit && node --experimental-strip-types src/test/typecheck.node.ts

  # Chạy linter ESLint 9
  npm run lint              # eslint .

  # Kiểm tra định dạng Prettier (+ prettier-plugin-tailwindcss)
  npm run format:check      # prettier --check src/

  # Build production Vite + TypeScript check
  npm run build             # tsc --noEmit && vite build

  # Chạy Vitest unit & component tests (36 files, 667 tests)
  npm run test:vitest       # hoặc: npm run test:run

  # Chạy Node Native contract & AST test runner (52 suites, 135 tests)
  npm test                  # node --experimental-strip-types --test src/test/suite.node.test.ts
  ```

### 7.3 Checklist Kiểm Định Trước Khi Hợp Nhất (Pre-merge Verification)

Mọi Pull Request / Task thực hiện trong Phase 4 phải vượt qua Checklist 6 điểm sau trước khi bàn giao cho `@qa-reviewer`:

- [ ] **Lint & Format Clean:** `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`, `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings`, `npm run lint` và `npm run format:check` đều pass với 0 warning.
- [ ] **Type Safe & Build Clean:** `npm run typecheck` và `npm run build` đạt 0 lỗi; không chứa bất kỳ type `any` nào trong frontend.
- [ ] **Panic Free:** Không có `.unwrap()` hoặc `.expect()` trong source code Rust backend (`clippy::unwrap_used` đã deny).
- [ ] **Data Contract Adherence:** Khớp 100% các trường dữ liệu và kiểu với Section 5 của `docs/ARCHITECTURE.md` và 48 mã lỗi trong `codes.rs` / `error.ts`.
- [ ] **Privacy Redaction Confirmed:** Không in bất kỳ token hoặc API key nào ra console hay log file.
- [ ] **All Tests Passed:** 100% `cargo test --manifest-path src-tauri/Cargo.toml --all-targets`, `npm run test:vitest` (`test:run`) và `npm test` đều chuyển màu xanh (PASS).

---
*Tài liệu này là Quy chuẩn kỹ thuật chính thức của dự án NeuroClip, đã được `@tech-lead` hoàn thiện trong Phase 3 và đệ trình Product Owner phê duyệt ban hành.*
