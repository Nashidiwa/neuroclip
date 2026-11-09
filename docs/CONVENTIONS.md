# QUY CHUẨN LẬP TRÌNH & ĐẶC TẢ KỸ THUẬT PHÁT TRIỂN (DEVELOPMENT CONVENTIONS)
# Dự án: NeuroClip — Windows Desktop App Hỗ trợ Clipper & Subber Neuro-sama Vietnam

**Mã tài liệu:** `docs/CONVENTIONS.md`  
**Phiên bản:** 1.0.0 (Ban hành chính thức tại Phase 3)  
**Chủ trì:** `@tech-lead`  
**Căn cứ kỹ thuật & Pháp lý:**
- Hiến pháp dự án: `AGENTS.md` (Zero-Guesswork Policy, Single Source of Truth, Gate Checks)
- Kiến trúc hệ thống: `docs/ARCHITECTURE.md` (v1.0.0, Sections 1–7)
- Yêu cầu sản phẩm: `docs/PRD.md` (v1.2.0, Epics 1–6)
- 7 Quyết định kiến trúc đã phê duyệt: `docs/adr/ADR-001` đến `ADR-007`
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

1. **Zero-Guesswork & Single Source of Truth (SSOT):** Mọi module, hàm, giao diện dữ liệu phải tuân thủ đúng các hợp đồng JSON Schema đã duyệt tại Section 5 của `docs/ARCHITECTURE.md` và các quyết định kỹ thuật tại `docs/adr/ADR-001` đến `ADR-007`. Không tự ý thay đổi schema mà không có RFC trình PO.
2. **Memory Safety & Low Footprint Invariant:** Mức chiếm dụng RAM tĩnh của ứng dụng không được vượt quá **100MB** (`ADR-002`). Không bao giờ tải toàn bộ video 30GB vào ổ đĩa hay bộ nhớ; luôn thực hiện Audio-First (`ADR-003`).
3. **Panic-Free Production Backend:** Tuyệt đối không sử dụng `.unwrap()` hoặc `.expect()` trong bất kỳ luồng xử lý thực tế nào (non-test code) của Rust backend. Mọi lỗi phải được xử lý qua kiểu `Result<T, AppError>`.
4. **Strict Concurrency Separation:** Phân tách triệt để giữa I/O bất đồng bộ (Tokio Async) và tính toán nặng tiêu tốn CPU (Audio RMS, VAD, Z-Score). Toàn bộ tác vụ tính toán nặng phải được đẩy sang `tokio::task::spawn_blocking` để không làm nghẽn Event Loop của Tokio.
5. **Privacy & Dual Export Guarantee:** Không bao giờ ghi API key người dùng (Groq, Gemini, OpenAI) hoặc Twitch Token ra file log (`[REDACTED]`). Khi xuất dự án CapCut Draft JSON, luôn xuất song song phụ đề `.ass` và `.srt` độc lập (`ADR-005`).

---

## 2. QUY CHUẨN BACKEND RUST (TAURI V2)

### 2.1 Toolchain, Linter & Formatter

- **Rust Edition:** `2021` (yêu cầu compiler `rustc >= 1.78.0`).
- **Toolchain Target:** `stable-x86_64-pc-windows-msvc`.
- **Clippy Configuration:** Toàn bộ crate `src-tauri` phải áp dụng các cờ kiểm tra nghiêm ngặt trong `src-tauri/src/lib.rs` hoặc file root:
  ```rust
  #![deny(clippy::all)]
  #![warn(clippy::pedantic)]
  #![deny(clippy::unwrap_used)]
  #![deny(clippy::expect_used)]
  #![deny(clippy::panic)]
  #![deny(clippy::todo)]
  #![deny(clippy::unimplemented)]
  // Các ngoại lệ pedantic được phép với lý do kiến trúc:
  #![allow(clippy::module_name_repetitions)]
  #![allow(clippy::must_use_candidate)]
  ```
- **Code Formatter (`rustfmt.toml`):** Đặt tại `src-tauri/rustfmt.toml`:
  ```toml
  max_width = 100
  hard_tabs = false
  tab_spaces = 4
  newline_style = "Windows"
  use_field_init_shorthand = true
  use_try_shorthand = true
  imports_granularity = "Crate"
  group_imports = "StdExternalCrate"
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
  - Quản lý token thông qua `CancellationRegistry` lưu trữ trong `tauri::State<AppState>`.
  - Khi người dùng bấm nút "Hủy (Cancel)" trên UI, backend kích hoạt `.cancel()` trên token tương ứng.
- **Giám Sát Tiến Trình Con (Subprocess Supervision - `ADR-007`):**
  - Tất cả các lệnh gọi `ffmpeg.exe` hoặc `yt-dlp.exe` qua `tokio::process::Command` **phải kích hoạt `.kill_on_drop(true)`**:
    ```rust
    let mut cmd = tokio::process::Command::new(ffmpeg_path);
    cmd.kill_on_drop(true); // Đảm bảo tự sát tiến trình khi future bị drop
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    ```
  - Trên Windows, kết hợp với Windows Job Objects (`winapi::um::jobapi2` / `CreateJobObjectW`) gán cờ `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` để đảm bảo khi ứng dụng chính bị terminate hoặc crash, toàn bộ subprocess con bị hệ điều hành dọn dẹp sạch sẽ trong vòng $\le 300\text{ms}$ (0 zombie processes).

### 2.3 An Toàn Bộ Nhớ & Bounded Buffers (Chống Tràn RAM/VRAM)

- **Giới hạn Bounded Buffers:** Tuyệt đối không đọc toàn bộ file âm thanh 400MB hoặc stream video vào `Vec<u8>` trong RAM.
  - Phải xử lý stream theo khối kích thước cố định (`buf_size = 64 * 1024` - 64KB) hoặc sử dụng file tạm thời trên ổ đĩa (`tempfile::NamedTempFile`).
  - Phân tích stdout/stderr của subprocess theo dòng bằng `tokio::io::BufReader` và `lines()`. Giới hạn kênh đệm (bounded channel) không vượt quá 1,000 dòng.
- **Pre-allocation cho Chat Data Structures:** Khi xử lý `chat_events.json`, luôn khởi tạo vector với dung lượng dự tính (`Vec::with_capacity(estimated_len)`) để tránh realloc liên tục trong bộ nhớ heap.
- **Bảo Vệ VRAM (CUDA OOM Guardrail - `ADR-001`):** Trước khi gọi `faster-whisper` cục bộ, phải kiểm tra thông số từ `HardwareProbeEngine`. Nếu VRAM khả dụng `< 2500MB`, lập tức chặn khởi tạo mô hình Local Whisper Large, tự động chuyển về INT8 Medium/Small hoặc trả về mã lỗi gợi ý chuyển sang Cloud BYOK Groq API.

### 2.4 Chính Sách Panic-Free & Xử Lý Result

- Tuyệt đối cấm sử dụng `.unwrap()` hoặc `.expect()` trong non-test code.
- Sử dụng toán tử `?` để lan truyền lỗi lên tầng cao hơn.
- Với các trường hợp ngoại lệ bắt buộc phải có giá trị mặc định, sử dụng `.unwrap_or(default)` hoặc `.unwrap_or_else(|| ...)`.
- Mọi hàm Tauri Command phải trả về kiểu `Result<T, AppError>`.

### 2.5 Cấu Trúc Thư Mục Chuẩn `src-tauri/`

```
src-tauri/
├── Cargo.toml
├── tauri.conf.json
├── rustfmt.toml
├── src/
│   ├── main.rs                   # Điểm vào ứng dụng, khởi tạo Tauri runtime
│   ├── lib.rs                    # Plugin registry, cấu hình Tauri builder
│   ├── error.rs                  # Root AppError, IpcErrorResponse, ErrorCode
│   ├── core/                     # Hạ tầng dùng chung của hệ thống
│   │   ├── mod.rs
│   │   ├── hardware.rs           # HardwareProbeEngine (CPU, RAM, GPU, VRAM DXGI/WMI)
│   │   ├── config.rs             # Quản lý cài đặt & nạp JSON từ %APPDATA%/NeuroClip/
│   │   ├── state.rs              # AppState, Arc<Mutex<...>> và Arc<RwLock<...>>
│   │   ├── storage.rs            # Kiểm tra đĩa cứng pre-flight, dọn dẹp cache
│   │   ├── logging.rs            # Khởi tạo tracing subscriber, daily rotation
│   │   ├── secrets.rs            # Hàm lọc che giấu API keys ([REDACTED])
│   │   └── cancellation.rs       # Task cancellation token registry
│   ├── ingestion/                # Thu nạp VOD và video cục bộ (ADR-003)
│   │   ├── mod.rs
│   │   ├── audio_streamer.rs     # yt-dlp audio stream extraction (~400MB)
│   │   ├── range_slicer.rs       # FFmpeg HTTP range slicing (-ss / -to copy)
│   │   ├── chat_downloader.rs    # Tải và chuẩn hóa Twitch GQL chat replay
│   │   └── local_importer.rs     # Nạp file OBS MP4/MKV và trích xuất PCM WAV
│   ├── highlight/                # Động cơ tính điểm Highlight đa phương thức (ADR-004)
│   │   ├── mod.rs
│   │   ├── zscore.rs             # Sliding window Z-score + Message floor (X >= 15)
│   │   ├── emote_filter.rs       # Trọng số Swarm emote & tỷ lệ E_ratio
│   │   ├── audio_energy.rs       # Đo đạc RMS amplitude peaks
│   │   └── fusion.rs             # Phối hợp đa phương thức & bù trễ stream (-4.5s)
│   ├── stt/                      # Phân hệ bóc băng giọng nói (ADR-001)
│   │   ├── mod.rs
│   │   ├── chunker.rs            # Chia nhỏ audio < 25MB kèm overlap 1.5s cho Groq
│   │   ├── groq_client.rs        # HTTP client gửi tới Groq Whisper Large-v3
│   │   ├── local_whisper.rs      # Worker process kết nối Faster-Whisper INT8
│   │   └── vad.rs                # Silero VAD & bộ lọc âm lượng loudnorm
│   ├── translation/              # Phân hệ dịch thuật & phân vai (ADR-001, ADR-006)
│   │   ├── mod.rs
│   │   ├── gemini_client.rs      # Google Gemini 1.5 Flash client (BLOCK_NONE)
│   │   ├── prompt_builder.rs     # Đóng gói Prompt Theatrical Framing & Từ điển Lore
│   │   ├── diarization.rs        # Suy luận vai nhân vật từ ngữ cảnh
│   │   └── condensation.rs       # Rút gọn câu thoại ngắn (t < 1.5s) bảo toàn CPS
│   ├── export/                   # Xuất bản NLE & Phụ đề (ADR-005)
│   │   ├── mod.rs
│   │   ├── capcut.rs             # Sinh thư mục CapCut Draft JSON (draft_content.json)
│   │   ├── ass.rs                # Xuất file .ass chuẩn màu style nhân vật cho Aegisub
│   │   ├── srt.rs                # Xuất file .srt UTF-8 phổ quát
│   │   └── premiere.rs           # Xuất file Final Cut Pro 7 / Premiere XML
│   ├── sidecar/                  # Quản trị công cụ phụ trợ (ADR-007)
│   │   ├── mod.rs
│   │   ├── supervisor.rs         # Vòng đời tiến trình subprocess (kill-on-drop)
│   │   ├── ffmpeg.rs             # Wrapper thực thi ffmpeg.exe
│   │   ├── ytdlp.rs              # Wrapper thực thi yt-dlp.exe
│   │   └── updater.rs            # Tự động nâng cấp yt-dlp từ GitHub Release kèm SHA256
│   └── commands/                 # Cổng tiếp nhận Tauri IPC Commands
│       ├── mod.rs
│       ├── hardware_cmd.rs       # probe_hardware_capabilities, test_api_credentials
│       ├── ingestion_cmd.rs      # start_vod_ingestion, import_local_video
│       ├── highlight_cmd.rs      # compute_highlights, bookmark_highlight
│       ├── stt_cmd.rs            # transcribe_highlight_clip
│       ├── translation_cmd.rs    # translate_vietsub, update_speaker_role
│       ├── export_cmd.rs         # export_capcut_draft, export_ass_file
│       ├── storage_cmd.rs        # check_preflight_storage, clear_cache
│       └── settings_cmd.rs       # get_settings, save_settings
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
    }
  },
  "include": ["src"]
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
    "plugins": ["prettier-plugin-tailwindcss"]
  }
  ```

### 3.3 Cấu Trúc Thư Mục Chuẩn `src/`

```
src/
├── index.html
├── main.tsx                      # Root mount, QueryClientProvider, ErrorBoundary
├── App.tsx                       # Layout navigation, StatusBar, Global hotkeys
├── assets/                       # Static SVGs, images, logo
├── components/                   # React UI components
│   ├── common/                   # Button, Input, Modal, Slider, Tooltip, Badge, Dropzone
│   ├── layout/                   # Header, Sidebar, WindowTitleBar, StatusBar
│   ├── onboarding/               # OnboardingWizardModal, HardwareBanner, KeyValidator
│   ├── dashboard/                # IngestionDashboard, UrlInputForm, DiskGuardBanner
│   ├── highlight/                # HighlightViewer, HighlightCard, ScoreBadge, ReasonTag
│   ├── studio/                   # SubtitleStudio, VideoCanvas, TimelineBar, SegmentRow,
│   │                             # CPSIndicator, SpeakerBadge, HotkeyHintBar
│   └── export/                   # ExportDialog, PresetSelector, DualSafeguardNotice
├── hooks/                        # Custom React hooks
│   ├── useTauriCommand.ts        # Typed invoke wrapper with loading & typed error
│   ├── useTauriEvent.ts          # Strongly typed event listener with auto-cleanup
│   ├── useGlobalHotkeys.ts       # Bàn phím số 1-5 gán vai, Shift+A/D timing nudge
│   ├── useVideoPlayback.ts       # Video playback sync, canvas RAF render loop
│   └── useDebounce.ts            # Debounce/throttle hooks cho slider & input
├── stores/                       # Quản lý trạng thái bằng Zustand
│   ├── projectStore.ts           # VOD manifest, highlight clusters, active clip
│   ├── subtitleStore.ts          # Segments array, active segment ID, undo/redo stack
│   ├── settingsStore.ts          # API keys (Groq/Gemini), hardware profile, user prefs
│   └── taskStore.ts              # In-progress background tasks, percent %, logs
├── services/                     # Giao tiếp API & Backend IPC
│   ├── ipc/                      # Wrapper Tauri IPC có kiểu tĩnh
│   │   ├── hardwareApi.ts
│   │   ├── ingestionApi.ts
│   │   ├── highlightApi.ts
│   │   ├── sttApi.ts
│   │   ├── translationApi.ts
│   │   ├── exportApi.ts
│   │   └── sidecarApi.ts
│   └── storage/                  # Helper lưu cache tạm thời
├── types/                        # Khai báo TypeScript types (khớp Data Contracts)
│   ├── contracts.ts              # HardwareProfile, VodManifest, ChatEvents, etc.
│   ├── error.ts                  # AppError, IpcErrorResponse, ErrorCode
│   ├── events.ts                 # Tauri event payload contracts
│   └── models.ts                 # SpeakerRole, LayoutPreset, TimelineSelection
├── utils/                        # Các hàm thuần túy (Pure functions)
│   ├── timecode.ts               # formatTimecode(sec) -> "HH:MM:SS.mmm", parseTimecode
│   ├── cps.ts                    # calculateCps, getCpsStatusColor (<17 green, >20 red)
│   ├── formatters.ts             # formatBytes, formatPercentage, formatDuration
│   └── cn.ts                     # Classnames merger: clsx + twMerge
└── styles/
    ├── globals.css               # Tailwind directives, custom scrollbars, dark VTuber theme
    └── tailwind.config.js
```

### 3.4 Quy Ước Đặt Tên Toàn Diện (Naming Conventions)

| Đối tượng | Quy ước | Ví dụ |
|---|---|---|
| **React Component** | `PascalCase` | `SubtitleStudio.tsx`, `HighlightCard.tsx` |
| **Custom Hook** | `camelCase` (bắt đầu bằng `use`) | `useGlobalHotkeys.ts`, `useVideoPlayback.ts` |
| **Zustand Store** | `camelCase` (kết thúc bằng `Store`) | `subtitleStore.ts`, `settingsStore.ts` |
| **Tauri IPC Command** | `snake_case` (cả Rust và TS invoke) | `start_vod_ingestion`, `export_capcut_draft` |
| **Tauri Event Name** | `kebab-colon` (`<domain>:<action>`) | `ingest:progress`, `stt:chunk`, `export:progress` |
| **TypeScript Type/Interface** | `PascalCase` | `HardwareProfile`, `TranscriptSegment` |
| **TypeScript Enum / Const** | `SCREAMING_SNAKE_CASE` | `MAX_PAYLOAD_SIZE`, `DEFAULT_PRESET` |
| **File tiện ích / Utils** | `camelCase` | `timecode.ts`, `formatters.ts` |
| **Tailwind Class Names** | Utility classes / kebab-case | `flex items-center justify-between bg-zinc-900` |

### 3.5 Hiệu Năng Timeline & Virtualized Subtitle Rendering

Trong các VOD dài 4–8 tiếng, số lượng phân đoạn phụ đề có thể lên tới 500–2,000 dòng. Để đảm bảo UI phản hồi tức thì với tốc độ 60fps:
1. **Virtualized Segment List:** Danh sách dòng phụ đề trong `SubtitleStudio` **BẮT BUỘC** sử dụng kỹ thuật ảo hóa danh sách (dùng `@tanstack/react-virtual` hoặc custom virtualizer). Chỉ render các dòng hiện diện trong viewport của bảng cuộn.
2. **Binary Search & Multi-segment Lookup cho Active Subtitle Rendering:**
   Khi video đang phát, con trỏ thời gian `currentTime` phát ra liên tục 60 lần/giây. Để hỗ trợ trọn vẹn các tình huống hội thoại nói đè lên nhau (Overlapping dialogue / banter đặc trưng giữa Neuro-sama và Vedal hoặc khách mời / Evil-Neuro):
   - Thuật toán tra cứu phụ đề **BẮT BUỘC trả về một mảng `SubtitleSegment[]`** (danh sách toàn bộ các phân đoạn đang kích hoạt tại mốc thời gian $t$), thay vì chỉ trả về một giá trị vô hướng đơn lẻ `SubtitleSegment`.
   - **Quy trình tra cứu tối ưu $O(\log N + K)$:**
     * **Bước 1:** Sử dụng Binary Search trên mảng phụ đề đã sắp xếp theo `start_sec` để định vị chỉ mục phân đoạn bắt đầu gần nhất với mốc thời gian phát $t$ trong thời gian $O(\log N)$.
     * **Bước 2 (K-nearest Active Scan):** Thực hiện quét lân cận sang trái và phải tính từ điểm tìm được để thu thập toàn bộ $K$ phân đoạn đồng thời thỏa mãn điều kiện:
       $$\text{start\_sec} \le t \le \text{end\_sec}$$
     * **Bước 3:** Trả về mảng `activeSegments: SubtitleSegment[]` (hoặc `TranscriptSegment[]`).
   - **Hiển thị xếp chồng độc lập (Stacked Overlay):** Video Canvas / Subtitle Overlay render đồng thời tất cả các dòng phụ đề có trong `activeSegments`, áp dụng đúng màu viền và kiểu chữ riêng biệt theo từng `speaker_id` (ví dụ: phụ đề viền xanh của Vedal và phụ đề viền hồng của Neuro cùng hiển thị xếp tầng mà không bị nuốt mất thoại của bất kỳ ai).
3. **Throttling trên Playback Seeker:** Sự kiện kéo thanh tua video (time scrubber slider) phải được throttle qua `requestAnimationFrame` (hoặc debounce 16ms) để tránh làm nghẽn luồng render của Canvas/Video Element.
4. **React.memo trên Segment Row:** Component `SegmentRow` phải được bọc trong `React.memo` với hàm so sánh tùy chỉnh kiểm tra: `segment_id`, `start_sec`, `end_sec`, `text_vi`, `speaker_id` và `isActive`.

---

## 4. GIAO THỨC TRUYỀN THÔNG LIÊN TIẾN TRÌNH IPC (TAURI COMMANDS & EVENTS)

### 4.1 Quy Ước Định Danh & Chữ Ký Tauri Command

- **Tên Command:** Đồng nhất viết theo dạng `snake_case` ở cả Rust backend và TypeScript frontend.
- **Kiểu trả về chuẩn:** Mọi hàm command phía Rust phải trả về `Result<T, AppError>`.
- **Ví dụ chữ ký Command:**
  ```rust
  // Rust Backend (src-tauri/src/commands/ingestion_cmd.rs)
  #[tauri::command]
  pub async fn start_vod_ingestion(
      url: String,
      audio_only: bool,
      app_state: tauri::State<'_, Arc<AppState>>,
      app_handle: tauri::AppHandle,
  ) -> Result<VodManifest, AppError> {
      // Logic xử lý...
  }
  ```
  ```typescript
  // TypeScript Frontend Service (src/services/ipc/ingestionApi.ts)
  export async function startVodIngestion(
    url: string,
    audioOnly: boolean = true,
  ): Promise<VodManifest> {
    return await invoke<VodManifest>('start_vod_ingestion', {
      url,
      audioOnly,
    });
  }
  ```

### 4.2 Hệ Thống Streaming Sự Kiện Bất Đồng Bộ (Event Streaming)

Các tiến trình dài hạn (tải audio, tính highlight, STT, export) không bắt UI phải chờ đợi lệnh `invoke` kết thúc mà liên tục phát sóng tiến độ về giao diện qua kênh sự kiện (Tauri Event Channel).

- **Định dạng tên Event:** `<domain>:<action>`
- **Danh mục Sự Kiện Tiêu Chuẩn:**

| Tên Event | Dữ liệu Payload (Payload Interface) | Mô tả & Tần suất |
|---|---|---|
| `ingest:progress` | `{ taskId: string, stage: string, percent: number, speedKbps: number, etaSec: number }` | Báo tiến độ tải stream audio (1 lần/giây) |
| `highlight:progress` | `{ taskId: string, stage: 'chat_parsing' \| 'audio_rms' \| 'clustering', percent: number }` | Báo tiến độ quét highlight Z-score/RMS |
| `stt:progress` | `{ clipId: string, currentChunk: number, totalChunks: number, percent: number }` | Tiến độ bóc băng Groq / Faster-Whisper |
| `stt:chunk` | `{ clipId: string, chunkIndex: number, segment: TranscriptSegment }` | Stream từng đoạn thoại bóc băng xong tức thì |
| `translation:progress` | `{ clipId: string, translatedCount: number, totalCount: number }` | Tiến độ dịch theo lô (batch sliding window) |
| `export:progress` | `{ projectId: string, percent: number, currentStep: string }` | Tiến độ ghi thư mục CapCut Draft / render MP4 |
| `sidecar:status` | `{ binary: 'yt-dlp' \| 'ffmpeg', status: 'ready' \| 'updating' \| 'failed', message?: string }` | Trạng thái tự cập nhật binary phụ trợ |

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

Hệ thống triển khai cây lỗi thống nhất tại `src-tauri/src/error.rs`:
```rust
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
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

#[derive(Debug, Error)]
pub enum IngestError {
    #[error("Dung lượng ổ đĩa không đủ (cần tối thiểu {required_mb}MB)")]
    DiskSpaceInsufficient { required_mb: u64, available_mb: u64 },

    #[error("URL không hợp lệ hoặc nền tảng không được hỗ trợ: {0}")]
    InvalidUrl(String),

    #[error("yt-dlp thất bại khi trích xuất stream URL: {0}")]
    ExtractionFailed(String),

    #[error("YouTube/Twitch kích hoạt cơ chế chặn bot (HTTP 403 / 429): {0}")]
    BotBlocked(String),

    #[error("Twitch Chat Replay không tồn tại hoặc đã bị xóa")]
    ChatReplayUnavailable,

    #[error("Mất kết nối mạng khi tải stream sau 5 lần thử lại")]
    NetworkStreamDropped,
}

#[derive(Debug, Error)]
pub enum SttError {
    #[error("Vượt giới hạn dung lượng Groq API (> 25MB)")]
    PayloadTooLarge { size_mb: f32 },

    #[error("Groq API Rate Limit hoặc hết quota (HTTP 429): {0}")]
    RateLimitExceeded(String),

    #[error("Groq API Key không hợp lệ hoặc chưa được cấu hình")]
    InvalidApiKey,

    #[error("Bộ nhớ VRAM không đủ để chạy Faster-Whisper ({available_mb}MB < 2500MB)")]
    InsufficientVram { available_mb: u32 },

    #[error("Lỗi mô hình VAD hoặc xử lý âm thanh: {0}")]
    VadProcessingFailed(String),
}

#[derive(Debug, Error)]
pub enum TranslationError {
    #[error("Gemini API Key không hợp lệ hoặc chưa được cấu hình")]
    InvalidApiKey,

    #[error("Bộ lọc an toàn Gemini kích hoạt (finishReason: SAFETY): {0}")]
    SafetyFilterTriggered(String),

    #[error("Gemini API Rate Limit hoặc lỗi máy chủ (HTTP {status}): {message}")]
    ApiRequestFailed { status: u16, message: String },
}

#[derive(Debug, Error)]
pub enum ExportError {
    #[error("Không tìm thấy đường dẫn thư mục CapCut Draft trên máy")]
    CapCutDirectoryNotFound,

    #[error("Lỗi ghi file dự án NLE ({path}): {source}")]
    FileWriteFailed {
        path: String,
        #[source]
        source: std::io::Error,
    },
}
```

### 5.2 Cấu Trúc Vỏ Bọc Lỗi Tuần Tự Hóa IPC (IpcError Envelope)

Khi trả lỗi về phía TypeScript qua Tauri IPC, `AppError` được chuyển đổi thành struct có cấu trúc `IpcErrorResponse`:
```rust
#[derive(Debug, Serialize)]
pub struct IpcErrorResponse {
    pub code: String,            // Mã lỗi định danh, ví dụ: "ERR_INGEST_DISK_FULL"
    pub message: String,         // Thông điệp thân thiện cho người dùng (tiếng Việt)
    pub details: Option<String>,  // Chi tiết kỹ thuật / stack trace phục vụ debug
    pub recoverable: bool,       // Cờ báo hiệu lỗi có thể khắc phục được qua Retry/Fallback
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let (code, recoverable) = match self {
            AppError::Ingestion(IngestError::DiskSpaceInsufficient { .. }) => {
                ("ERR_INGEST_DISK_FULL", true)
            }
            AppError::Ingestion(IngestError::BotBlocked(_)) => {
                ("ERR_INGEST_BOT_BLOCKED", true)
            }
            AppError::Ingestion(IngestError::ChatReplayUnavailable) => {
                ("ERR_INGEST_CHAT_UNAVAILABLE", true)
            }
            AppError::Ingestion(IngestError::NetworkStreamDropped) => {
                ("ERR_INGEST_NETWORK_TIMEOUT", true)
            }
            AppError::Stt(SttError::RateLimitExceeded(_)) => {
                ("ERR_STT_RATE_LIMIT", true)
            }
            AppError::Stt(SttError::PayloadTooLarge { .. }) => {
                ("ERR_STT_PAYLOAD_TOO_LARGE", false)
            }
            AppError::Stt(SttError::InsufficientVram { .. }) => {
                ("ERR_STT_VRAM_INSUFFICIENT", true)
            }
            AppError::Translation(TranslationError::SafetyFilterTriggered(_)) => {
                ("ERR_TRANSLATE_SAFETY_FILTER", true)
            }
            AppError::Export(ExportError::CapCutDirectoryNotFound) => {
                ("ERR_EXPORT_CAPCUT_NOT_FOUND", true)
            }
            _ => ("ERR_INTERNAL_ERROR", false),
        };

        let response = IpcErrorResponse {
            code: code.to_string(),
            message: self.to_string(),
            details: None,
            recoverable,
        };
        response.serialize(serializer)
    }
}
```

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
- **Lệnh chạy kiểm tra tiêu chuẩn:**
  ```bash
  # Chạy kiểm tra định dạng
  cargo fmt --all -- --check

  # Chạy clippy nghiêm ngặt
  cargo clippy --all-targets --all-features -- -D warnings

  # Chạy toàn bộ test suites
  cargo test --all-targets
  ```

### 7.2 Unit & Component Testing Phía Frontend

- **Testing Framework:** Vitest + React Testing Library + `@testing-library/jest-dom`.
- **Vị trí file test:** Đặt song song với file component hoặc hook:
  - `SubtitleStudio.tsx` $\rightarrow$ `SubtitleStudio.test.tsx`
  - `useGlobalHotkeys.ts` $\rightarrow$ `useGlobalHotkeys.test.ts`
  - `cps.ts` $\rightarrow$ `cps.test.ts`
- **Mục tiêu kiểm thử tối thiểu:**
  - Kiểm thử thuật toán tính điểm CPS và đổi màu cảnh báo đỏ khi CPS > 20.
  - Kiểm thử bộ phím tắt 1-chạm đổi màu nhân vật (Phím 1 -> Vedal xanh lá, Phím 2 -> Neuro hồng).
  - Kiểm thử cơ chế mượn khoảng lặng (Time Borrowing) khi căn chỉnh phụ đề.
- **Lệnh chạy kiểm tra phía frontend:**
  ```bash
  # Chạy kiểm tra TypeScript type check
  npm run typecheck   # tsc --noEmit

  # Chạy linter
  npm run lint        # eslint .

  # Chạy unit tests
  npm run test        # vitest run
  ```

### 7.3 Checklist Kiểm Định Trước Khi Hợp Nhất (Pre-merge Verification)

Mọi Pull Request / Task thực hiện trong Phase 4 phải vượt qua Checklist 6 điểm sau trước khi bàn giao cho `@qa-reviewer`:

- [ ] **Lint & Format Clean:** `cargo clippy` và `npm run lint` không có bất kỳ warning nào.
- [ ] **Type Safe:** `tsc --noEmit` đạt 0 lỗi; không chứa bất kỳ type `any` nào trong frontend.
- [ ] **Panic Free:** Không có `.unwrap()` hoặc `.expect()` trong source code Rust backend (`clippy::unwrap_used` đã deny).
- [ ] **Data Contract Adherence:** Khớp 100% các trường dữ liệu và kiểu với Section 5 của `docs/ARCHITECTURE.md`.
- [ ] **Privacy Redaction Confirmed:** Không in bất kỳ token hoặc API key nào ra console hay log file.
- [ ] **All Tests Passed:** 100% unit tests và integration tests liên quan đều chuyển màu xanh (PASS).

---
*Tài liệu này là Quy chuẩn kỹ thuật chính thức của dự án NeuroClip, đã được `@tech-lead` hoàn thiện trong Phase 3 và đệ trình Product Owner phê duyệt ban hành.*
