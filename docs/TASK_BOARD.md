# BẢNG THEO DÕI TIẾN ĐỘ DỰ ÁN (TASK BOARD)
# Dự án: NeuroClip — Windows Desktop App Hỗ trợ Clipper & Subber Neuro-sama Vietnam

**Mã tài liệu:** `docs/TASK_BOARD.md`  
**Phiên bản:** 2.0.0 (Phase 3: Conventions & Task Breakdown Completed — Chờ PO duyệt Gate 3)  
**Ngày cập nhật:** 2026-11-05  
**Chủ trì:** `@product-owner-proxy`, `@tech-lead` & `@qa-reviewer`  
**Quy tắc:** Cập nhật trạng thái liên tục theo từng Phase; tuân thủ nghiêm ngặt nguyên tắc Gate Checks tại `AGENTS.md`.

---

## 1. TỔNG QUAN TIẾN ĐỘ THEO CÁC PHASE & GATE CHECKS

| Phase | Tên Giai Đoạn | Người Chủ Trì | Mục Tiêu & Sản Phẩm Bàn Giao | Cổng Chặn Phê Duyệt (Gate Check) | Trạng Thái |
|:---:|:---|:---:|:---|:---|:---:|
| **Phase 1** | **Inception & Discovery** | `@product-owner-proxy` & `@lead-architect` | Khảo sát kỹ thuật, xác định phạm vi, lập `docs/PRD.md` và `docs/adr/RFC-001-phase1-decisions.md`. Tuyệt đối không viết code. | PO xác nhận Scope và trả lời 7 câu hỏi RFC (`RFC-001`). | **COMPLETED (PASSED)** |
| **Phase 2** | **Architecture Design & Data Contracts** | `@lead-architect` | Thiết lập kiến trúc hệ thống `docs/ARCHITECTURE.md`, dữ liệu luồng pipeline, data contracts, và 7 ADRs chính thức (`ADR-001` đến `ADR-007`). | PO ký duyệt kiến trúc trong `ARCHITECTURE.md` và 7 ADRs. | **COMPLETED (GATE 2 APPROVED)** |
| **Phase 3** | **Conventions & Task Breakdown** | `@tech-lead` & `@qa-reviewer` | Thiết lập quy chuẩn lập trình `docs/CONVENTIONS.md`, phân rã chi tiết 32 tasks Phase 4 kèm tiêu chí nghiệm thu và QA Test Checklists vào `docs/TASK_BOARD.md`. Duy trì Code Freeze. | PO ký duyệt Task Board và cho phép mở khóa tạo mã nguồn. | **READY_FOR_PO_REVIEW (GATE 3)** |
| **Phase 4** | **Implementation & Verification** | `@tech-lead`, Senior Devs & `@qa-reviewer` | Xây dựng mã nguồn hoàn chỉnh (Tauri shell, Sidecars, AI STT, Vietsub, NLE Export), kiểm thử tự động đạt 100% pass và đóng gói bộ cài Windows MSI/NSIS. | Toàn bộ kiểm thử tự động pass; PO nghiệm thu sản phẩm. | **PLANNED (SẴN SÀNG THỰC THI)** |

> **Quy định Duy trì Code Freeze trong Phase 3:**  
> Toàn bộ Phase 3 duy trì trạng thái Code Freeze nghiêm ngặt: **0 file code (.py, .ts, .rs, .js) được tạo trong workspace**. Chỉ sau khi PO chính thức phê duyệt tài liệu bàn giao Phase 3 (Gate 3), Phase 4 mới được kích hoạt để bắt đầu tạo mã nguồn.

---

## 2. CHI TIẾT TIẾN ĐỘ CÁC PHASE ĐÃ HOÀN THÀNH (PHASE 1 - PHASE 3)

### Phase 1: Inception & Discovery (M1) — ĐÃ HOÀN THÀNH (GATE 1 APPROVED)
- [x] **TASK-M1-01:** Rà soát ý tưởng, khảo sát nỗi đau của clipper và subber cộng đồng fan Neuro-sama Vietnam.
- [x] **TASK-M1-02:** Khảo sát các giới hạn kỹ thuật: Whisper VRAM cliff (4GB VRAM), dung lượng VOD 30GB, YouTube anti-bot, Twitch GQL chat API, hiện tượng hallucination trên nhạc nền.
- [x] **TASK-M1-03:** Soạn thảo và tinh chỉnh đặc tả sản phẩm `docs/PRD.md` (v1.2.0: tích hợp toàn diện 6 Change Requests CR-01 -> CR-06, bổ sung US1.0 First-Run Onboarding Wizard & Hardware Probe theo chỉ đạo từ PO).
- [x] **TASK-M1-04:** Lập danh sách 7 bài toán ngã rẽ kỹ thuật cốt tử cho PO `docs/adr/RFC-001-phase1-decisions.md` theo chuẩn Mục 3 `AGENTS.md`.
- [x] **TASK-M1-05:** Nhận phản hồi chính thức từ PO phê duyệt 7/7 lựa chọn tại RFC-001 (1:[3] Hybrid BYOK, 2:[1] Tauri v2, 3:[2] Audio-First Stream Analysis cho Online Link, 4:[3] Hybrid Multi-stage Highlights, 5:[2] Lean Subtitle Studio + NLE Export, 6:[2] LLM Context Diarization + 1-Click Hotkeys, 7:[2] Managed Sidecars).

### Phase 2: Architecture Design & Data Contracts (M2) — ĐÃ HOÀN THÀNH (GATE 2 APPROVED)
- [x] **TASK-M2-01:** Soạn thảo `docs/ARCHITECTURE.md` chi tiết hóa cấu trúc hệ thống (Tauri v2 Shell, Sidecar Subprocess Supervisor, Audio-First Ingestion Streamer, Hybrid AI Engine, First-Run Hardware Probe).
- [x] **TASK-M2-02:** Đặc tả Data Contracts & Schemas (`vod_manifest.json`, `chat_events.json`, `highlight_clusters.json`, `transcript_segments.json`, `subtitle_project.json`, `hardware_profile.json`).
- [x] **TASK-M2-03:** Ban hành 7 Architectural Decision Records chính thức (`docs/adr/ADR-001` đến `ADR-007`) chuyển hóa từ 7 quyết định RFC-001 đã được PO duyệt.
- [x] **TASK-M2-04:** Thiết kế luồng xử lý lỗi (Failure Modes Matrix ERR-01 đến ERR-09: Disk Space Pre-flight, Network Exponential Backoff, VAD Silence Pruning, Gemini Safety Filter Fallback, CapCut Schema Safeguard).
- [x] **TASK-M2-05:** **[GATE 2 PASSED]** PO ký duyệt kiến trúc trong `ARCHITECTURE.md` và thông qua toàn bộ 7 ADRs để chuyển sang Phase 3.

### Phase 3: Conventions & Technical Breakdown (M3) — ĐÃ HOÀN THÀNH (CHỜ DUYỆT GATE 3)
- [x] **TASK-M3-01:** Ban hành bộ quy chuẩn lập trình `docs/CONVENTIONS.md` (Rust backend Edition 2021, clippy lint profile, TypeScript frontend strict mode, Tauri IPC commands/events, Error Codes, Logging standard, Secret Redaction).
- [x] **TASK-M3-02:** Thiết kế cấu trúc thư mục mã nguồn chi tiết (Source Tree Architecture: `src-tauri/`, `src/`, `installer/`, `tests/`).
- [x] **TASK-M3-03:** Xây dựng ma trận kiểm thử tự động toàn diện do `@qa-reviewer` chủ trì (Unit Test, Integration Test, E2E Synthetic Test, Low-end PC Stress Test, Quality Gates & Veto Criteria).
- [x] **TASK-M3-04:** Phân rã 32 task kỹ thuật chi tiết kèm tiêu chí nghiệm thu định lượng (AC) và tích hợp QA Test Checklists cho từng task trong Phase 4 vào `docs/TASK_BOARD.md`.

---

## 3. BẢNG PHÂN RÃ CHI TIẾT 32 TASKS KỸ THUẬT PHASE 4 (TASK-M4-01 ĐẾN TASK-M4-32)

---

### Milestone M4.1: Nền Tảng Ứng Dụng & Hạ Tầng Cốt Lõi (Foundation)

#### TASK-M4-01: Khởi tạo Tauri v2 Native Desktop Shell & Giao diện Acrylic Dark Theme
- **Lead Persona:** `@tech-lead` & Senior Frontend Dev
- **Target Files:**
  - `src-tauri/tauri.conf.json`
  - `src-tauri/Cargo.toml`
  - `src-tauri/src/main.rs`
  - `src/App.tsx`
  - `src/index.html`
  - `src/styles/theme.css`
  - `vite.config.ts`
- **Mô tả & Phạm vi Kỹ thuật:**
  - Khởi tạo project Tauri v2 với WebView2 native trên Windows 10/11 64-bit.
  - Cấu hình cửa sổ ứng dụng: kích thước mặc định 1280x800, min 1024x640, hỗ trợ Windows Mica/Acrylic material.
  - Thiết lập bảng màu Dark Theme phong cách VTuber Neuro-sama: Nền chính `#12131C`, Nền phụ `#1A1C28`, Accent Pink `#FFB6C1` (Neuro), Accent Red `#FF0055` (Evil), Accent Green `#98FB98` (Vedal).
  - Tích hợp font chữ tối ưu hóa cho hiển thị phụ đề và giao diện tiếng Việt: `Be Vietnam Pro` hoặc `Inter`.
- **Giao kèo dữ liệu (Data Contracts):**
  - Đầu vào: Không có.
  - Đầu ra: Cửa sổ desktop hoạt động, render React 18 SPA với hiệu năng cao.
- **Quan hệ phụ thuộc (Dependencies):**
  - Prerequisites: Không (Task khởi đầu).
  - Blocks: TASK-M4-02, TASK-M4-20, TASK-M4-27.
- **Tiêu chí Nghiệm thu Định lượng (AC):**
  - [ ] **AC-01.1:** Ứng dụng khởi động trên Windows 10/11 có thời gian hiển thị giao diện chính (Time to Interactive - TTI) $\le 800\text{ms}$.
  - [ ] **AC-01.2:** Mức chiếm dụng RAM tĩnh (Static Idle RAM) khi chưa nạp video $\le 90\text{MB}$ (đo qua Windows Task Manager).
  - [ ] **AC-01.3:** Tỷ lệ khung hình render giao diện đạt ổn định $60\text{ FPS}$ khi thay đổi kích thước cửa sổ.
- **Checklist Kiểm thử Chất lượng (@qa-reviewer QA Test Checklist):**
  - *Phân loại kiểm thử:* Integration & Frontend Component Test
  - *Unit Test Cases:*
    - [ ] **UT-01.1 (Happy Path):** Kiểm tra hàm nạp theme cấu hình trả về đầy đủ các mã màu chuẩn Neuro-sama (`#12131C`, `#1A1C28`, `#FFB6C1`, `#98FB98`, `#FF0055`).
    - [ ] **UT-01.2 (Failure Path):** Khởi chạy trên môi trường không hỗ trợ Acrylic (Windows 10 build cũ), hàm fall back an toàn về solid background `#12131C` mà không crash.
  - *Integration & Contract Verification:*
    - [ ] **IT-01.1:** Cửa sổ Tauri v2 khởi tạo thành công với kích thước mặc định $1280\times 800$, min size $1024\times 640$.
    - [ ] **IT-01.2:** IPC bridge giữa WebView2 và Tauri Rust core phản hồi lệnh ping/pong trong vòng $\le 5\text{ms}$.
  - *Boundary & Hardware Limits:*
    - [ ] **ED-01.1:** Mức chiếm dụng RAM tĩnh (Static Idle RAM) đo bằng Windows Task Manager $\le 90\text{MB}$ khi chạy liên tục 30 phút.
    - [ ] **ED-01.2:** Thay đổi kích thước cửa sổ (Window Resize) liên tục trong 10 giây duy trì ổn định $\ge 55\text{ FPS}$.
  - *Lệnh thực thi & Tiêu chí xác minh:*
    ```bash
    npm run test -- src/App.test.tsx
    cargo test --bin neuroclip -- --nocapture
    ```
    *Khẳng định: TTI $\le 800\text{ms}$, Idle RAM $\le 90\text{MB}$.*

---

#### TASK-M4-02: Thiết lập Hệ thống Mã Lỗi Hợp Nhất & Giao thức Bao đóng Lỗi IPC
- **Lead Persona:** `@tech-lead`
- **Target Files:**
  - `src-tauri/src/error/mod.rs`
  - `src-tauri/src/error/codes.rs`
  - `src/services/ipc/error.ts`
  - `src/components/common/ErrorBoundary.tsx`
  - `src/components/common/ErrorBanner.tsx`
- **Mô tả & Phạm vi Kỹ thuật:**
  - Triển khai enum `AppError` bằng thư viện `thiserror` ở Rust backend, bao hàm toàn diện các nhóm lỗi: `IngestError`, `HighlightError`, `SttError`, `TranslationError`, `ExportError`, `SidecarError`, `HardwareError`.
  - Định nghĩa chuẩn mã lỗi dạng chuỗi cố định: `ERR_INGEST_DISK_FULL`, `ERR_INGEST_NETWORK_TIMEOUT`, `ERR_INGEST_BOT_BLOCKED`, `ERR_INGEST_CHAT_UNAVAILABLE`, `ERR_STT_PAYLOAD_TOO_LARGE`, `ERR_STT_RATE_LIMIT`, `ERR_STT_VRAM_INSUFFICIENT`, `ERR_TRANSLATE_SAFETY_FILTER`, `ERR_EXPORT_CAPCUT_NOT_FOUND`, `ERR_INTERNAL_ERROR`.
  - Triển khai `serde::Serialize` cho `AppError` để chuyển đổi tự động thành IPC Error Envelope cho TypeScript (`IpcErrorResponse`).
  - Xây dựng React Error Boundary và component `ErrorBanner` hiển thị thông báo thân thiện và nút "Thử lại (Retry)".
- **Giao kèo dữ liệu (Data Contracts):**
  - Đầu vào: Lỗi phát sinh từ các phân hệ Rust và subprocess.
  - Đầu ra: `IpcErrorResponse` chuẩn hóa truyền qua Tauri IPC bridge (`code`, `message`, `details`, `recoverable`).
- **Quan hệ phụ thuộc (Dependencies):**
  - Prerequisites: TASK-M4-01.
  - Blocks: Toàn bộ các tasks IPC (TASK-M4-05 đến TASK-M4-29).
- **Tiêu chí Nghiệm thu Định lượng (AC):**
  - [ ] **AC-02.1:** 100% các hàm Tauri IPC Command trả về kiểu `Result<T, AppError>`, không sử dụng `.unwrap()` hay `.expect()` trong luồng production.
  - [ ] **AC-02.2:** Khi giả lập lỗi mạng hoặc lỗi file, UI hiển thị đúng mã lỗi và nút Retry trong vòng $\le 50\text{ms}$.
  - [ ] **AC-02.3:** Không có bất kỳ unhandled panic nào làm crash process chính của Tauri.
- **Checklist Kiểm thử Chất lượng (@qa-reviewer QA Test Checklist):**
  - *Phân loại kiểm thử:* Unit Test & Serialization Contract Test
  - *Unit Test Cases:*
    - [ ] **UT-02.1 (Happy Path):** Mọi nhánh của enum `AppError` (`IngestError`, `SttError`, `TranslationError`, etc.) chuyển đổi chính xác sang mã chuỗi lỗi (`ERR_INGEST_DISK_FULL`, `ERR_INGEST_BOT_BLOCKED`, `ERR_INGEST_CHAT_UNAVAILABLE`, `ERR_STT_PAYLOAD_TOO_LARGE`, `ERR_TRANSLATE_SAFETY_FILTER`, `ERR_EXPORT_CAPCUT_NOT_FOUND`).
    - [ ] **UT-02.2 (Failure Path):** Lỗi không xác định (`AppError::Internal`) tự động sinh struct `IpcErrorResponse` với `recoverable = false` và mã `ERR_INTERNAL_ERROR`.
  - *Integration & Contract Verification:*
    - [ ] **IT-02.1:** Kết quả tuần tự hóa JSON của `AppError` tương thích 100% với TypeScript interface `IpcErrorResponse`.
    - [ ] **IT-02.2:** React Error Boundary bắt các ngoại lệ từ command invoke và render component `ErrorBanner` hiển thị đúng tiếng Việt kèm nút Retry.
  - *Boundary & Edge Cases:*
    - [ ] **ED-02.1 (Clippy Zero-Panic Gate):** Chạy kiểm tra tĩnh không có bất kỳ `.unwrap()` hoặc `.expect()` nào trong toàn bộ crate `src-tauri`.
  - *Lệnh thực thi & Tiêu chí xác minh:*
    ```bash
    cargo test --lib error::tests
    cargo clippy --all-targets -- -D clippy::unwrap_used -D clippy::expect_used -D clippy::panic
    ```
    *Khẳng định: 0 warning, 0 panic calls, 100% serializable error envelopes.*

---

#### TASK-M4-03: Bộ Điều Phối & Giám Sát Vòng Đời Tiến Trình Con (Subprocess Supervisor)
- **Lead Persona:** Senior Backend Dev (Rust)
- **Target Files:**
  - `src-tauri/src/sidecar/mod.rs`
  - `src-tauri/src/sidecar/supervisor.rs`
  - `src-tauri/src/sidecar/kill_switch.rs`
  - `src/services/ipc/sidecar.ts`
- **Mô tả & Phạm vi Kỹ thuật:**
  - Xây dựng module `SubprocessSupervisor` quản lý việc gọi các sidecar binaries ngoài (`yt-dlp.exe`, `ffmpeg.exe`, `faster-whisper-worker.exe`).
  - Sử dụng `tokio::process::Command` để thực thi bất đồng bộ, liên tục đọc stdout và stderr theo dòng (line-by-line streaming) bằng bounded channels để tránh tràn RAM.
  - Phân tích cú pháp tiến độ từ stdout của ffmpeg (time, bitrate, speed) và yt-dlp (download percentage, ETA) và phát event `sidecar:progress` về UI.
  - Triển khai cơ chế phòng thủ **Kill-on-Drop / Kill-on-Exit**: Sử dụng Windows Job Objects (`winapi::um::jobapi2` / `CreateJobObjectW`) gán cờ `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` để đảm bảo khi NeuroClip tắt hoặc crash, toàn bộ tiến trình con bị hủy triệt để trong $\le 300\text{ms}$ (0 zombie processes).
- **Giao kèo dữ liệu (Data Contracts):**
  - Đầu vào: Lệnh thực thi, tham số CLI, working directory, cancellation token.
  - Đầu ra: Event stream `sidecar:progress` `{ task_id, percentage, current_fps, eta_sec }` và exit code.
- **Quan hệ phụ thuộc (Dependencies):**
  - Prerequisites: TASK-M4-01, TASK-M4-02.
  - Blocks: TASK-M4-06, TASK-M4-07, TASK-M4-08, TASK-M4-11, TASK-M4-13.
- **Tiêu chí Nghiệm thu Định lượng (AC):**
  - [ ] **AC-03.1:** Khi tắt ứng dụng đột ngột (End Task qua Task Manager), 100% các tiến trình `ffmpeg.exe` và `yt-dlp.exe` đang chạy bị Windows OS giải phóng trong vòng $\le 300\text{ms}$ (0 zombie processes).
  - [ ] **AC-03.2:** Bounded channel xử lý dòng stdout giới hạn buffer tối đa 1,000 dòng, không tiêu tốn quá $5\text{MB}$ RAM kể cả khi log sinh ra với tốc độ $10,000\text{ dòng/giây}$.
  - [ ] **AC-03.3:** Phát tín hiệu hủy tác vụ (cancellation) thành công làm dừng tiến trình con trong $\le 200\text{ms}$.
- **Checklist Kiểm thử Chất lượng (@qa-reviewer QA Test Checklist):**
  - *Phân loại kiểm thử:* Integration & Operating System Resilience Test
  - *Unit Test Cases:*
    - [ ] **UT-03.1 (Happy Path):** Phân tích chuỗi đầu ra stdout của FFmpeg (`time=00:01:23.45 bitrate= 128.0kbits/s speed= 4.5x`) trích xuất chính xác timecode và tốc độ xử lý.
    - [ ] **UT-03.2 (Failure Path):** Phân tích stdout lỗi của `yt-dlp` (`ERROR: Private video` hoặc `HTTP Error 404`) trích xuất đúng enum lỗi nghiệp vụ.
  - *Integration & Contract Verification:*
    - [ ] **IT-03.1:** Chạy subprocess giả lập qua `SubprocessSupervisor`, stream sự kiện tiến độ `sidecar:progress` phát đều đặn về Frontend với payload `{ task_id, percentage, current_fps, eta_sec }`.
    - [ ] **IT-03.2:** Bounded channel giới hạn buffer 1,000 dòng; khi subprocess in ra $10,000\text{ dòng/s}$, memory của Rust không tăng quá $5\text{MB}$.
  - *Boundary & Hardware Limits (Zombie Process Elimination):**
    - [ ] **ED-03.1 (Kill-on-Drop):** Khởi chạy một tiến trình con tính toán dài hạn; drop instance `Command` trong Rust. Assert tiến trình con trong Windows OS biến mất trong $\le 200\text{ms}$.
    - [ ] **ED-03.2 (Hard Crash / Windows Job Object):** Giả lập crash process chính của Tauri (SIGKILL). Assert Windows Job Object tự động thu hồi toàn bộ các tiến trình `ffmpeg.exe` và `yt-dlp.exe` đang chạy con trong $\le 300\text{ms}$ (0 zombie processes).
  - *Lệnh thực thi & Tiêu chí xác minh:*
    ```bash
    cargo test --test subprocess_supervisor_test -- --nocapture
    ```
    *Khẳng định: 0 zombie process sau khi drop; cancellation dừng subprocess $\le 200\text{ms}$.*

---

#### TASK-M4-04: Hạ Tầng Ghi Log Cấu Trúc, Xoay Vòng Tệp & Che Giấu Thông Tin Nhạy Cảm
- **Lead Persona:** Senior Backend Dev (Rust)
- **Target Files:**
  - `src-tauri/src/core/logging.rs`
  - `src-tauri/src/core/secrets.rs`
  - `src-tauri/src/commands/log_cmd.rs`
- **Mô tả & Phạm vi Kỹ thuật:**
  - Thiết lập hạ tầng logging sử dụng crate `tracing` và `tracing-appender`.
  - Cấu hình format log: JSON có cấu trúc trong Release mode, Pretty-printed console trong Debug mode.
  - Lưu file log tại `%LOCALAPPDATA%/NeuroClip/logs/neuroclip-YYYY-MM-DD.log`.
  - Triển khai cơ chế xoay vòng file (Log Rotation): Daily rotation, giới hạn thư mục log $\le 50\text{MB}$, lưu trữ tối đa 14 ngày.
  - Xây dựng tầng lọc bảo mật tự động (`SecretRedactionLayer`): Tự động phát hiện và thay thế các chuỗi nhạy cảm bằng `[REDACTED]` trước khi ghi log hoặc gửi event: Groq API Key (`gsk_*`), Gemini API Key (`AIza*`), OpenAI API Key (`sk-*`), Twitch OAuth tokens.
- **Giao kèo dữ liệu (Data Contracts):**
  - Đầu vào: Các sự kiện `tracing::info!`, `tracing::warn!`, `tracing::error!`.
  - Đầu ra: File log an toàn và UI log viewer command.
- **Quan hệ phụ thuộc (Dependencies):**
  - Prerequisites: TASK-M4-01, TASK-M4-02.
  - Blocks: Tất cả các tasks xử lý API key và subprocess.
- **Tiêu chí Nghiệm thu Định lượng (AC):**
  - [ ] **AC-04.1:** 100% các chuỗi API Key và Auth Token xuất hiện trong log được thay thế bằng `[REDACTED]`, không để lọt bất kỳ chuỗi key thực tế nào.
  - [ ] **AC-04.2:** File log không bao giờ vượt quá $50\text{MB}$ tổng dung lượng trên ổ đĩa.
  - [ ] **AC-04.3:** Overhead hiệu năng của logging $\le 2\%$ tổng thời gian thực thi CPU.
- **Checklist Kiểm thử Chất lượng (@qa-reviewer QA Test Checklist):**
  - *Phân loại kiểm thử:* Unit Test & Security Privacy Test
  - *Unit Test Cases:*
    - [ ] **UT-04.1 (Happy Path - Secret Redaction):** Hàm `redact_secret("gsk_1234567890abcdef12345678")` trả về đúng chuỗi `"gsk_[REDACTED]5678"`.
    - [ ] **UT-04.2 (Edge Case - Short Key):** Chuỗi khóa ngắn $\le 8$ ký tự trả về hoàn toàn `"[REDACTED]"`.
    - [ ] **UT-04.3 (All Provider Tokens):** Kiểm tra bộ lọc bắt chính xác: Groq (`gsk_*`), Gemini (`AIza*`), OpenAI (`sk-*`), Twitch token (`auth-token=*`).
  - *Integration & Contract Verification:*
    - [ ] **IT-04.1:** Ghi log qua macro `tracing::info!(groq_key = %key, "Config loaded")`. Kiểm tra file log thực tế trên ổ cứng tuyệt đối KHÔNG chứa key gốc.
    - [ ] **IT-04.2:** Kiểm tra cơ chế xoay vòng log hàng ngày (Daily Rotation) và dọn dẹp log quá 14 ngày hoặc tổng thư mục log $> 50\text{MB}$.
  - *Boundary & Edge Cases:*
    - [ ] **ED-04.1:** Ghi 100,000 dòng log liên tục, assert CPU overhead $\le 2\%$ và file log tự ngắt thành file mới khi vượt quá $10\text{MB}$.
  - *Lệnh thực thi & Tiêu chí xác minh:*
    ```bash
    cargo test --lib core::logging::tests
    ```
    *Khẳng định: Regex scan file log sinh ra tìm kiếm chuỗi API key mẫu trả về 0 match.*

---

### Milestone M4.2: Luồng Thu Nạp Dữ Liệu Audio-First & Phân Tích Chat (Ingestion & Chat)

#### TASK-M4-05: Kiểm Tra Dung Lượng Ổ Đĩa Pre-flight & Quản Lý Thư Mục Bộ Nhớ Đệm Dự Án
- **Lead Persona:** Senior Backend Dev (Rust)
- **Target Files:**
  - `src-tauri/src/core/storage.rs`
  - `src-tauri/src/commands/storage_cmd.rs`
  - `src/stores/useProjectStore.ts`
- **Mô tả & Phạm vi Kỹ thuật:**
  - Triển khai kiểm tra dung lượng ổ đĩa trước khi tải (Pre-flight Disk Check) sử dụng crate `sysinfo` hoặc Windows Win32 `GetDiskFreeSpaceExW`.
  - Kiểm tra xem ổ đĩa chứa thư mục làm việc (mặc định là ổ `C:`) có dung lượng trống $\ge 5\text{GB}$ hay không. Nếu $< 5\text{GB}$, chặn tác vụ và trả về mã lỗi `ERR_INGEST_DISK_FULL` kèm gợi ý chọn ổ đĩa khác.
  - Khởi tạo và quản lý cấu trúc thư mục làm việc: `%LOCALAPPDATA%/NeuroClip/cache/` và `%LOCALAPPDATA%/NeuroClip/projects/<project_id>/`.
  - Xây dựng chức năng dọn dẹp cache tự động cho các file tạm cũ hơn 7 ngày.
- **Giao kèo dữ liệu (Data Contracts):**
  - Đầu vào: Đường dẫn ổ đĩa cần kiểm tra.
  - Đầu ra: `{ free_space_mb: number, is_sufficient: boolean, cache_dir: string }`.
- **Quan hệ phụ thuộc (Dependencies):**
  - Prerequisites: TASK-M4-02, TASK-M4-03.
  - Blocks: TASK-M4-06, TASK-M4-07, TASK-M4-13.
- **Tiêu chí Nghiệm thu Định lượng (AC):**
  - [ ] **AC-05.1:** Lệnh kiểm tra ổ đĩa hoàn tất trong vòng $\le 50\text{ms}$.
  - [ ] **AC-05.2:** Khi ổ đĩa còn dưới 5GB trống, tác vụ tải bị chặn ngay lập tức, không cho phép ghi file rác gây treo hệ điều hành.
  - [ ] **AC-05.3:** Lệnh dọn dẹp xóa sạch các file `.tmp` và `.wav` tạm thời mà không ảnh hưởng tới file dự án đã lưu.
- **Checklist Kiểm thử Chất lượng (@qa-reviewer QA Test Checklist):**
  - *Phân loại kiểm thử:* Unit Test & System Integration Test
  - *Unit Test Cases:*
    - [ ] **UT-05.1 (Happy Path):** Khi ổ đĩa trống $20\text{GB} \ge 5\text{GB}$, hàm `check_disk_space()` trả về `Ok(DiskStatus { is_sufficient: true, free_mb: 20480 })`.
    - [ ] **UT-05.2 (Failure Path - Disk Full):** Khi ổ đĩa trống $3.2\text{GB} < 5\text{GB}$, hàm trả về lỗi `Err(IngestError::DiskSpaceInsufficient { required_mb: 5120, available_mb: 3276 })`.
  - *Integration & Contract Verification:*
    - [ ] **IT-05.1:** Lệnh IPC `check_preflight_storage` trả về thông số ổ đĩa trong $\le 50\text{ms}$.
    - [ ] **IT-05.2:** Khởi tạo đúng cấu trúc thư mục `%LOCALAPPDATA%/NeuroClip/cache/` và `%LOCALAPPDATA%/NeuroClip/projects/`.
  - *Boundary & Edge Cases (ERR-01 Defense):**
    - [ ] **ED-05.1:** Giả lập ổ `C:` bị đầy $(<5\text{GB})$, người dùng chọn thư mục cache tại ổ `D:`. Hệ thống ghi nhận đường dẫn mới và tiếp tục cho phép tải.
    - [ ] **ED-05.2:** Lệnh `clean_expired_cache` xóa sạch toàn bộ file `.tmp` cũ hơn 7 ngày mà không xóa nhầm file dự án `.json`.
  - *Lệnh thực thi & Tiêu chí xác minh:*
    ```bash
    cargo test --lib core::storage::tests
    ```
    *Khẳng định: Chặn tải 100% khi dung lượng đĩa $< 5\text{GB}$.*

---

#### TASK-M4-06: Động Cơ Thu Nạp Luồng Âm Thanh Trực Tuyến Audio-First Không Tải Toàn Bộ Video
- **Lead Persona:** Senior Backend Dev (Rust) & `@tech-lead`
- **Target Files:**
  - `src-tauri/src/ingestion/streamer.rs`
  - `src-tauri/src/ingestion/cookies.rs`
  - `src-tauri/src/commands/ingest_cmd.rs`
  - `src/services/ipc/ingestion.ts`
  - `src/components/dashboard/IngestionDashboard.tsx`
- **Mô tả & Phạm vi Kỹ thuật:**
  - Tiếp nhận URL VOD trực tuyến từ Twitch hoặc YouTube.
  - Kiểm tra trạng thái phát trực tiếp: Nếu metadata trả về `is_live == true`, từ chối nạp với mã lỗi `ERR_INGEST_LIVE_STREAM_UNSUPPORTED` và thông báo giải thích rõ NeuroClip MVP chỉ hỗ trợ VOD đã kết thúc, ngăn chặn nguy cơ ghi hình vô hạn làm tràn ổ đĩa.
  - Cơ chế phòng thủ YouTube Anti-Bot & Throttling (`ERR-04`, `ARCHITECTURE.md` §6):
    - Hỗ trợ truyền tham số nạp Cookie từ trình duyệt: `--cookies-from-browser <browser>` (`chrome`, `firefox`, `edge`) hoặc nạp tệp xuất sẵn `--cookies <path_to_cookies.txt>`.
    - Triển khai cơ chế luân chuyển User-Agent (User-Agent Rotation): Luân phiên sử dụng các chuỗi User-Agent của các trình duyệt desktop phổ biến trên Windows 10/11 kèm headers giả lập trình duyệt hợp lệ để vượt qua kiểm tra fingerprint bot của YouTube.
  - Gọi `yt-dlp` với cờ trích xuất metadata `--dump-single-json` để đọc tiêu đề, độ dài, streamer, danh sách format.
  - Sử dụng `yt-dlp` để chỉ tải luồng âm thanh AAC bitrate thấp (format 140 của YouTube hoặc audio-only của Twitch, bitrate 128kbps - 160kbps).
  - Truyền tham số phục hồi mạng tự động: `--retries 5 --fragment-retries 10`.
  - Sinh và lưu tệp giao kèo `vod_manifest.json` theo đúng Schema 5.2 trong `ARCHITECTURE.md`.
- **Giao kèo dữ liệu (Data Contracts):**
  - Đầu vào: `vod_url: string`, `audio_first_mode: boolean`, `cookies_path: Option<String>`, `browser_cookies: Option<String>`.
  - Đầu ra: `vod_manifest.json` (Tuân thủ Schema 5.2: `vod_id`, `source_type`, `title`, `duration_seconds`, `audio_path`, `created_at`).
- **Quan hệ phụ thuộc (Dependencies):**
  - Prerequisites: TASK-M4-03, TASK-M4-05.
  - Blocks: TASK-M4-09, TASK-M4-11, TASK-M4-14.
- **Tiêu chí Nghiệm thu Định lượng (AC):**
  - [ ] **AC-06.1:** Đối với một VOD dài 8 tiếng (video gốc dung lượng ~30GB), hệ thống chỉ tải tệp audio có dung lượng $\le 450\text{MB}$ (tiết kiệm $\ge 98.5\%$ dung lượng mạng và ổ đĩa).
  - [ ] **AC-06.2:** Thời gian thu nạp audio hoàn tất trong $\le 90\text{ giây}$ trên đường truyền mạng tiêu chuẩn 50Mbps.
  - [ ] **AC-06.3:** File `vod_manifest.json` sinh ra hợp lệ 100% với JSON Schema 5.2.
  - [ ] **AC-06.4:** Hỗ trợ nạp cookie từ tệp `cookies.txt` hoặc trích xuất tự động qua `--cookies-from-browser` (Chrome/Firefox/Edge) và xoay vòng User-Agent, vượt qua thử thách HTTP 403 Bot Check của YouTube khi có cấu hình cookie.
  - [ ] **AC-06.5:** Từ chối xử lý URL livestream đang phát trực tiếp (`is_live == true`) trong $\le 500\text{ms}$, ngăn chặn nguy cơ ghi hình vô hạn làm tràn ổ đĩa.
- **Checklist Kiểm thử Chất lượng (@qa-reviewer QA Test Checklist):**
  - *Phân loại kiểm thử:* Integration & Network Invariant Test
  - *Unit Test Cases:*
    - [ ] **UT-06.1 (Happy Path):** Phân tích metadata JSON từ `yt-dlp --dump-single-json` đọc chuẩn xác `duration_seconds`, `title`, `uploader`.
    - [ ] **UT-06.2 (Failure Path):** Xử lý khi URL không hợp lệ (URL rác, link private) trả về `Err(IngestError::InvalidUrl)`.
  - *Integration & Contract Verification (Schema 5.2):**
    - [ ] **IT-06.1:** Gọi lệnh tải audio stream cho VOD trực tuyến, kiểm tra file sinh ra `vod_manifest.json` khớp 100% với Schema 5.2 trong `ARCHITECTURE.md`.
    - [ ] **IT-06.2:** Quá trình tải phát event `ingest:progress` với các trường `{ taskId, stage, percent, speedKbps, etaSec }` cập nhật mỗi giây.
  - *Boundary & Network Limits (Audio-First Invariant & ERR-03, ERR-04):**
    - [ ] **ED-06.1 (Network Conservation):** Cho VOD 8 tiếng (video gốc 30GB), dung lượng file audio tải về $\le 450\text{MB}$ (tiết kiệm $\ge 98.5\%$ băng thông và dung lượng đĩa).
    - [ ] **ED-06.2 (Network Interruption & Resume):** Giả lập ngắt mạng 3 giây trong khi đang tải audio; yt-dlp tự động retry và hoàn tất file audio toàn vẹn không bị hỏng header AAC.
    - [ ] **ED-06.3 (YouTube Anti-Bot Defense via Cookies & UA Rotation):** Giả lập YouTube trả về HTTP 403 Bot Block; truyền tham số `--cookies-from-browser chrome` hoặc nạp `cookies.txt` kèm User-Agent desktop tiêu chuẩn, xác nhận `yt-dlp` trích xuất thành công audio stream mà không bị chặn.
    - [ ] **ED-06.4 (Live Stream Rejection):** Nạp URL luồng đang phát trực tiếp (`is_live: true`), hệ thống nhận diện và trả về lỗi `ERR_INGEST_LIVE_STREAM_UNSUPPORTED`, không tải luồng vô hạn.
  - *Lệnh thực thi & Tiêu chí xác minh:*
    ```bash
    cargo test --test audio_ingest_integration_test -- --nocapture
    ```
    *Khẳng định: Không tải file video 1080p, `vod_manifest.json` pass schema validator, chống bot thành công với cookies/UA.*

---

#### TASK-M4-07: Bộ Nhập Khẩu Video Cục Bộ Ngoại Tuyến & Giao Thức Stream Tauri Asset
- **Lead Persona:** Senior Backend Dev (Rust) & Senior Frontend Dev
- **Target Files:**
  - `src-tauri/src/ingestion/local_importer.rs`
  - `src-tauri/src/core/custom_protocol.rs`
  - `src/components/dashboard/DropZone.tsx`
- **Mô tả & Phạm vi Kỹ thuật:**
  - Hỗ trợ người dùng kéo thả file video sẵn có từ máy tính (`.mp4`, `.mkv`, `.mov`, `.ts` từ OBS Studio).
  - Sử dụng FFmpeg trích xuất luồng âm thanh tạm thời vào cache:
    `ffmpeg -i input.mp4 -vn -acodec pcm_s16le -ar 16000 -ac 1 audio_cache.wav` (không tiêu tốn băng thông mạng).
  - Đăng ký giao thức tùy chỉnh `asset://` trong Tauri v2 để WebView2 có thể stream video trực tiếp từ đường dẫn ổ đĩa Windows nội bộ với đầy đủ HTTP Range headers.
  - Sinh `vod_manifest.json` với `source_type: "local_file"`.
- **Giao kèo dữ liệu (Data Contracts):**
  - Đầu vào: `file_path: string`.
  - Đầu ra: `vod_manifest.json` và file `audio_cache.wav` 16kHz mono.
- **Quan hệ phụ thuộc (Dependencies):**
  - Prerequisites: TASK-M4-03, TASK-M4-05.
  - Blocks: TASK-M4-11, TASK-M4-14, TASK-M4-20.
- **Tiêu chí Nghiệm thu Định lượng (AC):**
  - [ ] **AC-07.1:** Trích xuất audio từ file MP4 2 tiếng trên ổ cứng SSD hoàn tất trong vòng $\le 10\text{ giây}$.
  - [ ] **AC-07.2:** Trình phát video trên UI thông qua `asset://` phát video 1080p60fps mượt mà, thời gian phản hồi khi kéo tua timeline $\le 100\text{ms}$.
  - [ ] **AC-07.3:** Không thực hiện bất kỳ network request nào ra bên ngoài khi người dùng chọn chế độ file cục bộ.
- **Checklist Kiểm thử Chất lượng (@qa-reviewer QA Test Checklist):**
  - *Phân loại kiểm thử:* Integration & Media Streaming Test
  - *Unit Test Cases:*
    - [ ] **UT-07.1 (Happy Path):** Nhận diện các đuôi mở rộng file hợp lệ: `.mp4`, `.mkv`, `.mov`, `.ts`, `.flv`.
    - [ ] **UT-07.2 (Failure Path):** Nhận diện file không phải video (`.txt`, `.exe`, `.zip`) trả về lỗi định dạng không hỗ trợ.
  - *Integration & Contract Verification:*
    - [ ] **IT-07.1:** Gọi lệnh FFmpeg trích xuất luồng âm thanh PCM WAV 16kHz mono từ file MP4 nội bộ vào thư mục cache hoàn tất trong $\le 10\text{ giây}$ cho video 2 tiếng.
    - [ ] **IT-07.2:** Sinh `vod_manifest.json` với `source_type: "local_file"`, `video_stream_url: null`, `local_video_path: "..."`.
  - *Boundary & Performance Limits:*
    - [ ] **ED-07.1 (Zero-Network Invariant):** Giám sát toàn bộ network socket trong suốt quá trình import file cục bộ: khẳng định 0 bytes truyền ra Internet.
    - [ ] **ED-07.2 (Smooth Playback via Asset Protocol):** Trình phát WebView2 kết nối `asset://` tua nhanh tới mốc 1 giờ 30 phút; video phản hồi khung hình trong $\le 100\text{ms}$.
  - *Lệnh thực thi & Tiêu chí xác minh:*
    ```bash
    cargo test --lib ingestion::local_importer::tests
    ```
    *Khẳng định: Trích xuất audio local $\le 10\text{s}$, zero network packets.*

---

#### TASK-M4-08: Bộ Tải & Phân Tích Lịch Sử Chat Replay Twitch GQL Chuẩn Hóa
- **Lead Persona:** Senior Backend Dev (Rust)
- **Target Files:**
  - `src-tauri/src/ingestion/chat_parser.rs`
  - `src-tauri/src/ingestion/twitch_gql.rs`
  - `src/services/ipc/chat.ts`
- **Mô tả & Phạm vi Kỹ thuật:**
  - Tải toàn bộ lịch sử tin nhắn của Twitch VOD thông qua `yt-dlp --write-subs` hoặc truy vấn trực tiếp Twitch GQL API endpoint `https://gql.twitch.tv/gql` với pagination cursor (`client-id: kimne78kx3ncx6brgo4mv6wki5h1ko`).
  - Xử lý cơ chế phòng vệ khi Chat Replay không tồn tại (VOD quá hạn, chat bị xóa, hoặc HTTP 404): Trả về cảnh báo mềm, không làm crash luồng và kích hoạt cờ `has_chat_replay = false` để chuyển tuyến sang Audio RMS.
  - Phân tích cú pháp dữ liệu JSON thô thành cấu trúc vector trong Rust: bóc tách timestamp từng mili-giây, tên tác giả, nội dung tin nhắn, danh sách emote Twitch/BTTV/7TV.
  - Xuất ra tệp dữ liệu chuẩn hóa `chat_events.json` tuân thủ Schema 5.3 trong `ARCHITECTURE.md`.
- **Giao kèo dữ liệu (Data Contracts):**
  - Đầu vào: `vod_id: string`, hoặc đường dẫn file chat JSON thô.
  - Đầu ra: `chat_events.json` (Tuân thủ Schema 5.3: `vod_id`, `total_messages`, `events: [...]`).
- **Quan hệ phụ thuộc (Dependencies):**
  - Prerequisites: TASK-M4-03, TASK-M4-06.
  - Blocks: TASK-M4-09, TASK-M4-10, TASK-M4-12.
- **Tiêu chí Nghiệm thu Định lượng (AC):**
  - [ ] **AC-08.1:** Tải và phân tích 500,000 dòng chat (stream 8 tiếng đông người xem) hoàn tất trong vòng $\le 15\text{ giây}$.
  - [ ] **AC-08.2:** Bộ nhớ RAM tiêu thụ trong quá trình phân tích 500,000 dòng chat được giải phóng ngay sau khi ghi file, đỉnh RAM tăng không quá $80\text{MB}$.
  - [ ] **AC-08.3:** Khi gặp VOD không có chat, hệ thống bắt mã lỗi `ERR_INGEST_CHAT_UNAVAILABLE` và kích hoạt luồng fallback âm thanh trong vòng $\le 200\text{ms}$.
- **Checklist Kiểm thử Chất lượng (@qa-reviewer QA Test Checklist):**
  - *Phân loại kiểm thử:* Unit Test & Resilient Fallback Test
  - *Unit Test Cases:*
    - [ ] **UT-08.1 (Happy Path - Parsing):** Phân tích 1 payload chat GQL thô chứa 100 tin nhắn; bóc tách đúng `timestamp_sec`, `author`, `message`, và mảng `emotes_detected`.
    - [ ] **UT-08.2 (Happy Path - Swarm Emote Flag):** Tin nhắn chứa `KEKW` hoặc `Tutel` được gán cờ `is_swarm_emote = true`.
  - *Integration & Contract Verification (Schema 5.3):**
    - [ ] **IT-08.1:** File `chat_events.json` sinh ra tuân thủ 100% JSON Schema 5.3 trong `ARCHITECTURE.md`.
    - [ ] **IT-08.2:** Phân tích cú pháp 500,000 dòng chat hoàn tất trong $\le 15\text{ giây}$.
  - *Boundary & Fallback Limits (ERR-05 Defense):**
    - [ ] **ED-08.1 (Chat 404 / Unavailable):** Giả lập Twitch trả về HTTP 404 hoặc VOD bị tắt chat. Hàm bắt lỗi `ChatReplayUnavailable`, phát tín hiệu chuyển tuyến sang Audio RMS, hiển thị Informational Banner vàng trên UI mà KHÔNG dừng pipeline.
    - [ ] **ED-08.2 (RAM Bound):** Peak heap memory trong quá trình xử lý 500k messages không tăng quá $80\text{MB}$; giải phóng toàn bộ RAM sau khi ghi file xuống đĩa.
  - *Lệnh thực thi & Tiêu chí xác minh:*
    ```bash
    cargo test --test chat_parser_test -- --nocapture
    ```
    *Khẳng định: `chat_events.json` schema valid; 404 kích hoạt Audio RMS fallback $\le 200\text{ms}$.*

---

### Milestone M4.3: Động Cơ Trí Tuệ Phát Hiện Highlight Đa Phương Thức (Intelligence & Highlights)

#### TASK-M4-09: Thuật Toán Tính Điểm Tốc Độ Chat Z-Score Hiệu Chỉnh Kèm Ngưỡng Sàn Tin Nhắn
- **Lead Persona:** Senior Backend Dev (Rust) & `@lead-architect`
- **Target Files:**
  - `src-tauri/src/highlight/zscore.rs`
  - `src-tauri/src/highlight/bucket.rs`
- **Mô tả & Phạm vi Kỹ thuật:**
  - Gom toàn bộ tin nhắn từ `chat_events.json` vào các khung thời gian có độ rộng cố định $W = 5\text{ giây}$.
  - Áp dụng ngưỡng sàn tin nhắn: Nếu số lượng tin nhắn trong khung $X(t) < 15\text{ msgs}$, gán trực tiếp điểm Z-score $Z_{\text{adj}}(t) = 0$ để triệt tiêu hoàn toàn báo động giả trong các giai đoạn stream yên ắng.
  - Tính toán cửa sổ trượt nền trong phạm vi 60 giây gần nhất để tính giá trị trung bình $\mu_W(t)$ và độ lệch chuẩn $\sigma_W(t)$.
  - Triển khai công thức Z-Score hiệu chỉnh với hằng số làm mịn $\epsilon = 0.5$:
    $$Z_{\text{adj}}(t) = \frac{X(t) - \mu_W(t)}{\sigma_W(t) + 0.5}$$
  - Tối ưu hóa thuật toán với độ phức tạp $O(N)$ bằng kỹ thuật Cumulative Sum / Sliding Window state.
- **Giao kèo dữ liệu (Data Contracts):**
  - Đầu vào: `chat_events.json`.
  - Đầu ra: Mảng các giá trị `chat_velocity_series: Vec<{ timestamp_sec: f64, count: u32, z_score: f64 }>`.
- **Quan hệ phụ thuộc (Dependencies):**
  - Prerequisites: TASK-M4-08.
  - Blocks: TASK-M4-12.
- **Tiêu chí Nghiệm thu Định lượng (AC):**
  - [ ] **AC-09.1:** Tính toán Z-score cho 500,000 tin nhắn hoàn tất trong thời gian $\le 1.2\text{ giây}$ trên CPU Intel i5 / AMD Ryzen 5.
  - [ ] **AC-09.2:** Khi chat bị spam bởi 1 người (spam bot cùng tác giả trong 5s), thuật toán loại bỏ bớt tin nhắn trùng tác giả để tránh spike giả.
  - [ ] **AC-09.3:** 100% các giai đoạn có $X(t) < 15$ trả về Z-score bằng 0.
- **Checklist Kiểm thử Chất lượng (@qa-reviewer QA Test Checklist):**
  - *Phân loại kiểm thử:* Pure Logic Math Unit Test
  - *Unit Test Cases (Toán học chính xác):*
    - [ ] **UT-09.1 (Ngưỡng sàn $X < 15$ msgs):** Cho khung 5 giây có $X(t) = 14$ tin nhắn, bất kể giá trị trung bình $\mu$ và độ lệch chuẩn $\sigma$, khẳng định $Z_{\text{adj}}(t) = 0.0$.
    - [ ] **UT-09.2 (Độ làm mịn $\epsilon = 0.5$):** Cho khung 5 giây có $X(t) = 45$ tin nhắn, $\mu = 10.0$, $\sigma = 0.0$. Tính $Z_{\text{adj}} = \frac{45 - 10}{0.0 + 0.5} = \frac{35}{0.5} = 70.0$. Không bị chia cho 0.
    - [ ] **UT-09.3 (Spam Bot Filter):** Một tác giả gửi 20 tin nhắn giống hệt nhau trong 5 giây; thuật toán deduplicate chỉ tính tối đa 3 tin nhắn từ cùng 1 author trong 1 bucket, triệt tiêu spike giả mạo.
  - *Integration & Contract Verification:*
    - [ ] **IT-09.1:** Xuất ra mảng `chat_velocity_series` có độ phân giải 5 giây cho toàn bộ thời lượng VOD.
  - *Boundary & Performance Limits:*
    - [ ] **ED-09.1:** Xử lý 500,000 tin nhắn bằng thuật toán trượt $O(N)$ hoàn tất trong $\le 1.2\text{ giây}$ trên CPU standard.
  - *Lệnh thực thi & Tiêu chí xác minh:*
    ```bash
    cargo test --lib highlight::zscore::tests
    ```
    *Khẳng định: 100% test cases toán học pass; triệt tiêu 100% false positives trong lull periods.*

---

#### TASK-M4-10: Động Cơ Trọng Số Emote Đặc Thù Văn Hóa Neuro Swarm
- **Lead Persona:** Senior Backend Dev (Rust) & `@product-owner-proxy`
- **Target Files:**
  - `src-tauri/src/highlight/emote_filter.rs`
  - `src-tauri/src/highlight/swarm_dict.rs`
- **Mô tả & Phạm vi Kỹ thuật:**
  - Xây dựng từ điển dữ liệu tĩnh về các biểu cảm đặc thù của kênh `vedal987` và cộng đồng VTuber:
    - Nhóm Hài hước: `KEKW`, `LUL`, `OMEGALUL`, `💀`, `LMAO`, `LOL`.
    - Nhóm Bất ngờ: `Pog`, `PogBones`, `Aware`, `Clueless`, `WTF`.
    - Nhóm Văn hóa Swarm: `Tutel`, `Gymbag`, `Heart`, `Dinking`, `Vedal`, `Evil`, `Neuro`.
  - Phân tích tỷ lệ mật độ Emote Swarm trong từng khung thời gian 5 giây:
    $$E_{\text{ratio}}(t) = \frac{\sum \text{Số lượng Swarm Emotes}}{\text{Tổng số token từ vựng}}$$
  - Gán trọng số tăng cường (Bonus multiplier) khi phát hiện sự bùng nổ của các emote đặc trưng.
- **Giao kèo dữ liệu (Data Contracts):**
  - Đầu vào: `chat_events.json` và bộ từ điển `swarm_dict.json`.
  - Đầu ra: Mảng `emote_density_series: Vec<{ timestamp_sec: f64, top_emotes: Vec<String>, e_ratio: f64 }>`.
- **Quan hệ phụ thuộc (Dependencies):**
  - Prerequisites: TASK-M4-08.
  - Blocks: TASK-M4-12.
- **Tiêu chí Nghiệm thu Định lượng (AC):**
  - [ ] **AC-10.1:** Nhận diện chính xác 100% các emote trong từ điển Swarm chuẩn (kể cả case-insensitive hoặc ký tự unicode như `💀`).
  - [ ] **AC-10.2:** Tốc độ quét emote đạt $\ge 500,000\text{ tokens/giây}$.
- **Checklist Kiểm thử Chất lượng (@qa-reviewer QA Test Checklist):**
  - *Phân loại kiểm thử:* Pure Logic Tokenizer Test
  - *Unit Test Cases:*
    - [ ] **UT-10.1 (Happy Path - Swarm Match):** Chuỗi tin nhắn `"KEKW look at that tutel 💀"` bóc tách chính xác 3 emotes: `["KEKW", "tutel", "💀"]`.
    - [ ] **UT-10.2 (Case Insensitivity & Unicode):** Nhận diện chính xác `kekw`, `KEKW`, `Kekw`, `TUTEL`, `tutel`, `💀`, `PogBones`.
    - [ ] **UT-10.3 (Emote Ratio Calculation):** Bucket có tổng số 50 tokens từ vựng, trong đó có 25 emotes thuộc từ điển Swarm $\rightarrow$ Khẳng định $E_{\text{ratio}} = 25 / 50 = 0.50$ (50%).
  - *Integration & Contract Verification:*
    - [ ] **IT-10.1:** Tích hợp với `chat_velocity_series` sinh mảng `emote_density_series` khớp từng timestamp bucket.
  - *Boundary & Performance Limits:*
    - [ ] **ED-10.1:** Tốc độ quét tokenizer đạt $\ge 500,000\text{ tokens/s}$ nhờ tối ưu hóa qua Aho-Corasick automaton hoặc hash set tĩnh `phf`.
  - *Lệnh thực thi & Tiêu chí xác minh:*
    ```bash
    cargo test --lib highlight::emote_filter::tests
    ```
    *Khẳng định: 100% emotes trong danh mục PRD Mục 6 được nhận diện chính xác.*

---

#### TASK-M4-11: Bộ Quét Đỉnh Năng Lượng Âm Thanh (Audio RMS Peak Detector)
- **Lead Persona:** Senior Backend Dev (Rust)
- **Target Files:**
  - `src-tauri/src/highlight/audio_rms.rs`
  - `src-tauri/src/highlight/energy_analyzer.rs`
- **Mô tả & Phạm vi Kỹ thuật:**
  - Đọc file âm thanh từ `vod_manifest.json` (`audio_path`).
  - Sử dụng FFmpeg với bộ lọc `astats` hoặc thư viện xử lý âm thanh Rust `symphonia` để tính toán giá trị năng lượng Root Mean Square (RMS) theo từng block $500\text{ms}$.
  - Chuẩn hóa giá trị RMS về thang đo $[0, 1.0]$.
  - Nhận diện các đoạn âm thanh có biên độ tăng vọt đột ngột (Audio Energy Spikes).
  - Đóng vai trò là kênh bổ trợ khi có Chat Replay và kênh phân tích chính khi không có Chat Replay.
- **Giao kèo dữ liệu (Data Contracts):**
  - Đầu vào: Đường dẫn tệp âm thanh (AAC hoặc WAV).
  - Đầu ra: Mảng `audio_rms_series: Vec<{ timestamp_sec: f64, rms_energy: f64, is_peak: boolean }>`.
- **Quan hệ phụ thuộc (Dependencies):**
  - Prerequisites: TASK-M4-03, TASK-M4-06, TASK-M4-07.
  - Blocks: TASK-M4-12.
- **Tiêu chí Nghiệm thu Định lượng (AC):**
  - [ ] **AC-11.1:** Phân tích RMS toàn bộ file audio 8 tiếng hoàn tất trong vòng $\le 45\text{ giây}$ (xử lý native đa luồng).
  - [ ] **AC-11.2:** Nhận diện chuẩn xác các đoạn âm thanh vượt ngưỡng âm lượng nền $\ge 18\text{dB}$.
- **Checklist Kiểm thử Chất lượng (@qa-reviewer QA Test Checklist):**
  - *Phân loại kiểm thử:* Audio Processing Unit & Integration Test
  - *Unit Test Cases:*
    - [ ] **UT-11.1 (Happy Path - RMS Math):** Cho mảng mẫu âm thanh PCM 16-bit hình sin biên độ tối đa, hàm tính RMS trả về giá trị xấp xỉ $1.0$. Với đoạn âm thanh im lặng, RMS trả về $0.0$.
    - [ ] **UT-11.2 (Peak Threshold):** Nhận diện đỉnh âm lượng khi RMS vượt $\ge 2.5$ độ lệch chuẩn so với baseline nền 30 giây lân cận.
  - *Integration & Contract Verification:*
    - [ ] **IT-11.1:** Xuất ra mảng `audio_rms_series` với độ phân giải block $500\text{ms}$.
  - *Boundary & Performance Limits (Screaming Loudness & Speed):**
    - [ ] **ED-11.1:** Phân tích RMS toàn bộ file audio 8 tiếng hoàn tất trong vòng $\le 45\text{ giây}$ trên CPU 4 nhân.
    - [ ] **ED-11.2:** Đoạn Vedal hét to hoặc Neuro cười rú lên (biên độ tăng $> 18\text{dB}$) được đánh dấu `is_peak = true` chính xác 100%.
  - *Lệnh thực thi & Tiêu chí xác minh:*
    ```bash
    cargo test --lib highlight::audio_rms::tests
    ```
    *Khẳng định: Quét 8h audio $\le 45\text{s}$, RMS normalize nằm trọn vẹn trong $[0.0, 1.0]$.*

---

#### TASK-M4-12: Hợp Nhất Đa Phương Thức & Bộ Bù Độ Trễ Livestream 4.5 Giây
- **Lead Persona:** `@lead-architect` & Senior Backend Dev (Rust)
- **Target Files:**
  - `src-tauri/src/highlight/fusion.rs`
  - `src-tauri/src/highlight/cluster.rs`
  - `src-tauri/src/commands/highlight_cmd.rs`
  - `src/components/highlight/HighlightViewer.tsx`
- **Mô tả & Phạm vi Kỹ thuật:**
  - Hợp nhất 3 kênh dữ liệu theo công thức trọng số đa tầng:
    $$\text{Score}_{\text{final}}(t) = 0.6 \cdot Z_{\text{adj}}(t) + 0.25 \cdot E_{\text{ratio}}(t) + 0.15 \cdot \text{RMS}_{\text{normalized}}(t)$$
    *(Khi không có Chat: $\text{Score}_{\text{final}}(t) = 1.0 \cdot \text{RMS}_{\text{normalized}}(t)$ kèm bộ lọc tần số).*
  - Triển khai **Bộ bù độ trễ livestream (Stream Latency Compensation) & Giới hạn biên thời gian chặt chẽ (End Timecode Clamping with Dynamic Padding)**:
    $$t_{\text{start}} = \max\left(0.0,\; t_{\text{chat\_peak}} - 4.5\text{s} - 10\text{s (Pre-roll Buffer)}\right)$$
    $$t_{\text{end}} = \min\left(t_{\text{chat\_peak}} + 15\text{s (Post-roll Buffer)},\; \text{vod\_manifest.duration\_seconds}\right)$$
    - **Quy tắc Dynamic Padding ở cuối Stream:** Nếu đỉnh chat xảy ra ở sát cuối VOD ($t_{\text{chat\_peak}} + 15\text{s} > \text{duration\_seconds}$), hệ thống tự động co giãn Post-roll Buffer theo dung lượng thời gian thực còn lại:
      $$\text{Post-roll Buffer} = \max\left(0.0,\; \text{duration\_seconds} - t_{\text{chat\_peak}}\right)$$
      đồng thời tự động bù phần thời lượng thiếu hụt sang Pre-roll Buffer (tăng từ 10s lên tối đa 20s nếu $t_{\text{start}} > 0$) để bảo đảm độ dài đoạn highlight đạt tối thiểu $\ge 15\text{s}$ mà tuyệt đối không vượt quá mốc kết thúc stream (`duration_seconds`), ngăn ngừa triệt để lỗi invalid range slicing cho FFmpeg ở TASK-M4-13.
  - Gom các đỉnh lân cận trong khoảng $30\text{s}$ thành một cụm duy nhất (Clustering & Deduplication).
  - Xếp hạng và trích xuất top 15–20 cụm điểm cao nhất, lưu vào tệp `highlight_clusters.json` theo Schema 5.4 trong `ARCHITECTURE.md`.
- **Giao kèo dữ liệu (Data Contracts):**
  - Đầu vào: `chat_velocity_series`, `emote_density_series`, `audio_rms_series`, `vod_manifest.duration_seconds`.
  - Đầu ra: `highlight_clusters.json` (Tuân thủ Schema 5.4: `vod_id`, `clusters: [{ cluster_id, start_sec, end_sec, peak_sec, score, primary_reason, top_emotes }]`).
- **Quan hệ phụ thuộc (Dependencies):**
  - Prerequisites: TASK-M4-09, TASK-M4-10, TASK-M4-11.
  - Blocks: TASK-M4-13, TASK-M4-14, TASK-M4-20.
- **Tiêu chí Nghiệm thu Định lượng (AC):**
  - [ ] **AC-12.1:** Tổng thời gian hợp nhất điểm và phân cụm $\le 500\text{ms}$.
  - [ ] **AC-12.2:** Mốc thời gian clip highlight luôn bắt đầu trước đỉnh chat ít nhất 10 giây, bảo đảm bắt trọn câu nói khởi đầu của streamer trong 100% các đoạn test.
  - [ ] **AC-12.3:** Không có 2 cụm highlight nào bị gối đầu lên nhau quá 50% thời lượng.
  - [ ] **AC-12.4:** 100% các cụm highlight thỏa mãn điều kiện biên bất biến: $0.0 \le t_{\text{start}} < t_{\text{end}} \le \text{vod\_manifest.duration\_seconds}$. Tuyệt đối không sinh timecode vượt quá thời lượng VOD kể cả khi đỉnh chat nằm ở giây cuối cùng của stream.
- **Checklist Kiểm thử Chất lượng (@qa-reviewer QA Test Checklist):**
  - *Phân loại kiểm thử:* Fusion Algorithm & Contract Test (Schema 5.4)
  - *Unit Test Cases (Công thức Fusion & Bù trễ):*
    - [ ] **UT-12.1 (Full Multimodal Weighting):** Cho $Z_{\text{adj}} = 10.0$ (chuẩn hóa về $1.0$), $E_{\text{ratio}} = 0.8$, $\text{RMS} = 0.6$. Khẳng định $\text{Score} = 0.6(1.0) + 0.25(0.8) + 0.15(0.6) = 0.6 + 0.2 + 0.09 = 0.89$ ($89/100$).
    - [ ] **UT-12.2 (Audio-Only Fallback Weighting):** Khi không có chat ($Z=0, E=0$), $\text{Score} = 1.0 \times \text{RMS}_{\text{normalized}}$.
    - [ ] **UT-12.3 (Bù độ trễ 4.5s & Padding):** Đỉnh chat ghi nhận tại $t_{\text{peak}} = 01:24:30.0$ ($5070.0\text{s}$) trên VOD dài $7200.0\text{s}$. Khẳng định:
      $t_{\text{start}} = 5070.0 - 4.5 - 10.0 = 5055.5\text{s}$ ($01:24:15.5$).
      $t_{\text{end}} = 5070.0 + 15.0 = 5085.0\text{s}$ ($01:24:45.0$).
  - *Integration & Contract Verification (Schema 5.4):**
    - [ ] **IT-12.1:** Gom cụm (Clustering) các đỉnh cách nhau $\le 30\text{s}$ thành 1 cụm duy nhất.
    - [ ] **IT-12.2:** Xuất ra tệp `highlight_clusters.json` tuân thủ 100% JSON Schema 5.4 trong `ARCHITECTURE.md`.
  - *Boundary & Edge Cases (Clamping & Clustered Range Bounds):*
    - [ ] **ED-12.1:** Nếu $t_{\text{start}}$ tính ra $< 0$, tự động ép về $0.0\text{s}$ (không sinh timecode âm).
    - [ ] **ED-12.2:** Xếp hạng trích xuất đúng top 15–20 cụm điểm cao nhất, thời gian xử lý toàn bộ $\le 500\text{ms}$.
    - [ ] **ED-12.3 (Clamp End Timecode to VOD Duration & Dynamic Padding):** Cho VOD có tổng thời lượng $10,000.0\text{s}$, giả lập đỉnh chat xảy ra sát cuối stream tại $t_{\text{peak}} = 9,992.0\text{s}$. Khẳng định $t_{\text{end}}$ được clamp chính xác tại $10,000.0\text{s}$ (không vượt quá `duration_seconds`); Pre-roll Buffer tự động bù thêm để clip đạt độ dài $22.5\text{s}$; khẳng định không sinh timecode vượt quá EOF gây lỗi cho FFmpeg ở TASK-M4-13.
  - *Lệnh thực thi & Tiêu chí xác minh:*
    ```bash
    cargo test --lib highlight::fusion::tests
    ```
    *Khẳng định: `highlight_clusters.json` pass schema; mốc $t_{\text{start}}$ luôn đi trước đỉnh chat $\ge 14.5\text{s}$; $t_{\text{end}} \le \text{duration\_seconds}$ 100%.*

---

#### TASK-M4-13: Bộ Tải Phân Đoạn Video Chính Xác Không Tải Thừa (Lossless Precision Range Slicer)
- **Lead Persona:** Senior Backend Dev (Rust)
- **Target Files:**
  - `src-tauri/src/ingestion/range_slicer.rs`
  - `src-tauri/src/commands/slice_cmd.rs`
- **Mô tả & Phạm vi Kỹ thuật:**
  - Khi người dùng chọn 1 cụm highlight trên UI (thời lượng 30s–90s):
    - Nếu nguồn là Online VOD: Lấy URL stream gốc m3u8 từ `vod_manifest.json`, truyền tham số cho FFmpeg thực hiện HTTP Range Request cắt trực tiếp qua mạng không tải cả video:
      `ffmpeg -ss <start> -to <end> -i "<stream_m3u8_url>" -c copy -avoid_negative_ts make_zero "clip_<id>.mp4"`
    - Nếu nguồn là Local File: Cắt nhanh trực tiếp từ file nội bộ bằng stream copy `-c copy`.
  - Kiểm tra tính toàn vẹn của tệp video cắt ra (Keyframe alignment).
- **Giao kèo dữ liệu (Data Contracts):**
  - Đầu vào: `cluster_id: string`, `start_sec: number`, `end_sec: number`.
  - Đầu ra: Đường dẫn tệp video phân đoạn `clip_slice.mp4`.
- **Quan hệ phụ thuộc (Dependencies):**
  - Prerequisites: TASK-M4-03, TASK-M4-05, TASK-M4-06, TASK-M4-12.
  - Blocks: TASK-M4-20, TASK-M4-25, TASK-M4-26.
- **Tiêu chí Nghiệm thu Định lượng (AC):**
  - [ ] **AC-13.1:** Tải và cắt một đoạn clip 90 giây từ VOD trực tuyến diễn ra trong vòng $\le 10\text{ giây}$ trên đường truyền mạng tiêu chuẩn.
  - [ ] **AC-13.2:** Dung lượng file tải về chỉ dao động từ $30\text{MB} - 80\text{MB}$ thay vì 30GB video.
  - [ ] **AC-13.3:** Video cắt bằng `-c copy` đồng bộ hoàn hảo hình và tiếng, không bị đứng hình ở giây đầu tiên.
- **Checklist Kiểm thử Chất lượng (@qa-reviewer QA Test Checklist):**
  - *Phân loại kiểm thử:* Subprocess & Lossless Video Slicing Test
  - *Unit Test Cases:*
    - [ ] **UT-13.1 (Happy Path - Parameter Formatting):** Sinh câu lệnh FFmpeg HTTP Range request chuẩn xác:
      `ffmpeg -ss <start> -to <end> -i "<m3u8_url>" -c copy -avoid_negative_ts make_zero "clip.mp4"`.
  - *Integration & Contract Verification:*
    - [ ] **IT-13.1:** Cắt phân đoạn 90 giây từ stream m3u8 trực tuyến, file video `clip_slice.mp4` sinh ra có dung lượng dao động $30\text{MB} - 80\text{MB}$ (thay vì tải 30GB).
    - [ ] **IT-13.2:** Thời gian tải và cắt phân đoạn 90 giây $\le 10\text{ giây}$ trên mạng 50Mbps.
  - *Boundary & Video Quality Limits:*
    - [ ] **ED-13.1 (Keyframe Alignment Check):** Kiểm tra tệp MP4 cắt ra bằng ffprobe: audio stream và video stream có duration lệch nhau không quá $50\text{ms}$, giây đầu tiên hiển thị khung hình ngay tức thì.
  - *Lệnh thực thi & Tiêu chí xác minh:*
    ```bash
    cargo test --test range_slicer_test -- --nocapture
    ```
    *Khẳng định: Video cắt mượt mà, file size $\le 80\text{MB}$, thời gian cắt $\le 10\text{s}$.*

---

### Milestone M4.4: Động Cơ Nhận Diện Giọng Nói & Tách Thoại (STT & VAD)

#### TASK-M4-14: Bộ Tiền Xử Lý Âm Thanh Chuẩn Hóa & Lọc Khoảng Lặng Silero VAD
- **Lead Persona:** Senior Backend Dev (Rust) & `@tech-lead`
- **Target Files:**
  - `src-tauri/src/stt/audio_prep.rs`
  - `src-tauri/src/stt/vad.rs`
- **Mô tả & Phạm vi Kỹ thuật:**
  - Áp dụng bộ lọc âm thanh `loudnorm` của FFmpeg:
    `ffmpeg -i clip.mp4 -af "loudnorm=I=-16:TP=-1.5:LRA=11" clip_norm.wav`
    nhằm làm dịu biên độ khi streamer la hét (chống hiện tượng vỡ tiếng clipping).
  - Tích hợp mô hình **Silero VAD (Voice Activity Detection)** chạy qua ONNX Runtime:
    - Quét file âm thanh, nhận diện các mốc có tiếng người nói.
    - Cắt bỏ hoàn toàn các khoảng lặng và đoạn chỉ có nhạc nền game kéo dài $> 1.5\text{ giây}$.
  - Xác định các điểm ngắt tự nhiên (Natural Pause Points) để chia nhỏ file audio thành các chunks $< 20\text{MB}$ khi gửi lên Groq Cloud API.
- **Giao kèo dữ liệu (Data Contracts):**
  - Đầu vào: File audio thô của clip (`clip_slice.mp4`).
  - Đầu ra: File audio đã chuẩn hóa âm lượng và danh sách `speech_segments: Vec<{ start_ms: u64, end_ms: u64 }>`.
- **Quan hệ phụ thuộc (Dependencies):**
  - Prerequisites: TASK-M4-03, TASK-M4-13.
  - Blocks: TASK-M4-15, TASK-M4-16.
- **Tiêu chí Nghiệm thu Định lượng (AC):**
  - [ ] **AC-14.1:** Bộ lọc Silero VAD xử lý 1 phút âm thanh trong vòng $\le 1.5\text{ giây}$ trên CPU.
  - [ ] **AC-14.2:** Loại bỏ triệt để $\ge 90\%$ các đoạn im lặng/nhạc không lời, giúp giảm tối thiểu 30% dung lượng payload gửi lên cloud.
  - [ ] **AC-14.3:** Điểm cắt chunk luôn nằm trong khoảng lặng âm thanh, không bao giờ cắt ngang một từ đang phát âm.
- **Checklist Kiểm thử Chất lượng (@qa-reviewer QA Test Checklist):**
  - *Phân loại kiểm thử:* Audio DSP & ONNX VAD Unit Test
  - *Unit Test Cases:*
    - [ ] **UT-14.1 (Happy Path - Silero VAD Speech Detection):** Đưa vào file âm thanh 60 giây gồm 20s nói xen kẽ 40s im lặng/nhạc game; Silero VAD trả về chính xác danh sách các mốc thời gian có tiếng người nói.
    - [ ] **UT-14.2 (Silence Pruning):** Cắt bỏ triệt để $\ge 90\%$ các đoạn im lặng/nhạc không lời kéo dài $> 1.5\text{ giây}$.
    - [ ] **UT-14.3 (Natural Pause Splitting):** Điểm cắt chia chunk luôn nằm tại các khoảng lặng giữa 2 câu nói, không bao giờ cắt ngang giữa một từ đang phát âm.
  - *Integration & Contract Verification:*
    - [ ] **IT-14.1:** Tích hợp bộ lọc FFmpeg `loudnorm=I=-16:TP=-1.5:LRA=11` nén đỉnh biên độ âm thanh trước khi đưa vào VAD.
  - *Boundary & Audio Limits (ERR-08 Screaming Defense):**
    - [ ] **ED-14.1 (Vedal Scream Distortion):** Đưa vào đoạn audio streamer la hét bão hòa biên độ đỉnh ($0\text{dBFS}$). Sau khi qua `loudnorm`, True Peak được khống chế ở mức $\le -1.5\text{dBFS}$, dạng sóng không bị clipping.
    - [ ] **ED-14.2 (VAD Speed):** Thời gian xử lý VAD trên 1 phút âm thanh $\le 1.5\text{ giây}$ trên CPU.
  - *Lệnh thực thi & Tiêu chí xác minh:*
    ```bash
    cargo test --lib stt::vad::tests
    ```
    *Khẳng định: Giảm $\ge 30\%$ payload size gửi cloud, điểm cắt chunk an toàn.*

---

#### TASK-M4-15: Máy Khách Groq Whisper Cloud API Kèm Phân Khối Nhỏ & Chế Độ Chống Lặp Từ
- **Lead Persona:** Senior Backend Dev (Rust)
- **Target Files:**
  - `src-tauri/src/stt/groq_client.rs`
  - `src-tauri/src/commands/stt_cmd.rs`
  - `src/services/ipc/stt.ts`
- **Mô tả & Phạm vi Kỹ thuật:**
  - Xây dựng HTTP Client giao tiếp với Groq Cloud endpoint: `https://api.groq.com/openai/v1/audio/transcriptions`.
  - Sử dụng mô hình `whisper-large-v3` với tham số word-level timestamps (`timestamp_granularities[]=word&timestamp_granularities[]=segment`).
  - Tự động chia file audio lớn thành các khối $< 20\text{MB}$ tại các điểm ngắt của Silero VAD (TASK-M4-14) để không bao giờ vượt giới hạn $25\text{MB}$ của Groq.
  - Triển khai cờ chống lặp từ (Karaoke Mode): truyền `condition_on_previous_text=false` khi phát hiện phân đoạn có nhạc nền để triệt tiêu hiện tượng Whisper lặp lại vô tận.
  - Xử lý mã lỗi HTTP 429 (Rate Limit): Áp dụng thuật toán Exponential Backoff tự động retry sau $1\text{s}, 2\text{s}, 4\text{s}$.
- **Giao kèo dữ liệu (Data Contracts):**
  - Đầu vào: File audio đã cắt chunk và Groq API Key.
  - Đầu ra: Raw transcript JSON kèm word-level timestamps và segment timestamps.
- **Quan hệ phụ thuộc (Dependencies):**
  - Prerequisites: TASK-M4-02, TASK-M4-04, TASK-M4-14, TASK-M4-28.
  - Blocks: TASK-M4-17, TASK-M4-18.
- **Tiêu chí Nghiệm thu Định lượng (AC):**
  - [ ] **AC-15.1:** Tốc độ bóc băng cho đoạn video clip 3 phút đạt mức $\le 12\text{ giây}$ (tốc độ xử lý $\ge 200x\text{ realtime}$).
  - [ ] **AC-15.2:** 100% các file payload gửi lên Groq có kích thước $\le 20\text{MB}$, không có lỗi HTTP 413 Payload Too Large.
  - [ ] **AC-15.3:** Triệt tiêu hoàn toàn lỗi lặp từ vô tận khi bóc băng các đoạn clip ca hát/karaoke.
- **Checklist Kiểm thử Chất lượng (@qa-reviewer QA Test Checklist):**
  - *Phân loại kiểm thử:* Mock HTTP Integration & Protocol Test
  - *Unit Test Cases:*
    - [ ] **UT-15.1 (Happy Path - Chunk Sizing):** Audio dài 45 phút được chia thành các chunks có dung lượng $\le 20\text{MB}$ (ngưỡng an toàn dưới mức $25\text{MB}$ của Groq) với vùng overlap $1.5\text{ giây}$.
    - [ ] **UT-15.2 (Karaoke / Anti-Repetition Guardrail):** Khi phát hiện cờ Music Mode, request gửi lên Groq bắt buộc chứa `condition_on_previous_text=false`.
  - *Integration & Contract Verification (`wiremock`):**
    - [ ] **IT-15.1:** Mock Groq API endpoint `/openai/v1/audio/transcriptions`. Kiểm tra parse kết quả trả về bóc tách đầy đủ word-level timestamps và segment text.
    - [ ] **IT-15.2 (Rate Limit Exponential Backoff):** Giả lập Groq trả về HTTP 429. Client tự động chờ và retry sau $1\text{s}, 2\text{s}, 4\text{s}$ (tối đa 3 lần).
  - *Boundary & Network Limits (ERR-06 Defense):**
    - [ ] **ED-15.1 (Payload Limit Strict Assertion):** Assert 100% các file payload gửi lên Groq có kích thước $< 25\text{MB}$, ngăn chặn triệt để lỗi HTTP 413 Payload Too Large.
    - [ ] **ED-15.2 (Processing Speed SLA):** Tốc độ bóc băng cho clip 3 phút hoàn tất trong $\le 12\text{ giây}$ ($\ge 200x\text{ realtime}$).
  - *Lệnh thực thi & Tiêu chí xác minh:*
    ```bash
    cargo test --test groq_client_test -- --nocapture
    ```
    *Khẳng định: Không có lỗi HTTP 413; phá vỡ vòng lặp lặp từ; tốc độ $\ge 200x$ realtime.*

---

#### TASK-M4-16: Công Nhân Xử Lý Ngoại Tuyến Faster-Whisper INT8 Dự Phòng Trên Máy
- **Lead Persona:** Senior Backend Dev (Rust) & Systems Dev
- **Target Files:**
  - `src-tauri/src/stt/faster_whisper_worker.rs`
  - `src-tauri/src/stt/model_manager.rs`
- **Mô tả & Phạm vi Kỹ thuật:**
  - Xây dựng worker subprocess hoặc C/C++ FFI binding bọc quanh thư viện `faster-whisper` (CTranslate2).
  - Sử dụng mô hình Whisper `medium` hoặc `small` với quantization `int8` để đảm bảo mức tiêu thụ VRAM luôn $< 2\text{GB}$.
  - Triển khai **Hardware Guardrail**: Đọc thông số VRAM khả dụng từ `hardware_profile.json`. Nếu VRAM $< 3\text{GB}$, khóa cứng tùy chọn Whisper Large, tự động ép về INT8 Medium hoặc cảnh báo người dùng chuyển sang Groq Cloud API (`ERR_STT_CUDA_OOM`).
  - Phục vụ như một phương án ngoại tuyến 100% khi người dùng không có kết nối Internet.
- **Giao kèo dữ liệu (Data Contracts):**
  - Đầu vào: File audio WAV và đường dẫn thư mục model.
  - Đầu ra: Raw transcript JSON đồng dạng với Groq API output.
- **Quan hệ phụ thuộc (Dependencies):**
  - Prerequisites: TASK-M4-03, TASK-M4-14, TASK-M4-27.
  - Blocks: TASK-M4-17, TASK-M4-18.
- **Tiêu chí Nghiệm thu Định lượng (AC):**
  - [ ] **AC-16.1:** Mức chiếm dụng bộ nhớ GPU VRAM khi chạy Faster-Whisper INT8 không vượt quá $1.8\text{GB}$.
  - [ ] **AC-16.2:** Không gây hiện tượng CUDA Out-of-Memory (OOM) làm crash ứng dụng trên các GPU 4GB VRAM như GTX 1650 / RTX 3050 Laptop.
- **Checklist Kiểm thử Chất lượng (@qa-reviewer QA Test Checklist):**
  - *Phân loại kiểm thử:* Worker Process & Hardware Guardrail Test
  - *Unit Test Cases:*
    - [ ] **UT-16.1 (Happy Path - Command Construction):** Sinh tham số gọi worker CTranslate2 với `--model medium --quantization int8 --device auto`.
    - [ ] **UT-16.2 (Hardware Guardrail Check):** Kiểm tra thông số VRAM từ `hardware_profile.json`. Nếu VRAM $< 2500\text{MB}$, từ chối khởi tạo Whisper Large, tự động ép về INT8 Medium hoặc trả về lỗi `ERR_STT_CUDA_OOM`.
  - *Integration & Contract Verification:*
    - [ ] **IT-16.1:** Output transcript JSON từ local worker có cấu trúc đồng dạng 100% với output của Groq Cloud API.
  - *Boundary & Hardware Limits (ERR-02 Defense):**
    - [ ] **ED-16.1 (VRAM Cap < 2GB):** Đo mức chiếm dụng bộ nhớ GPU khi chạy Faster-Whisper INT8 trên GPU 4GB VRAM (GTX 1650): khẳng định VRAM sử dụng không vượt quá $1.8\text{GB}$, không gây lỗi CUDA OOM.
  - *Lệnh thực thi & Tiêu chí xác minh:*
    ```bash
    cargo test --test local_whisper_worker_test -- --nocapture
    ```
    *Khẳng định: VRAM peak $\le 1.8\text{GB}$, 0 CUDA crash trên hệ thống 4GB VRAM.*

---

### Milestone M4.5: Bản Địa Hóa, Ngữ Cảnh Lore & Dịch Thuật Vietsub (Localization & Translation)

#### TASK-M4-17: Máy Khách Google Gemini 1.5 Flash Kèm Prompt Đóng Gói Kịch Bản Sân Khấu (Theatrical Framing)
- **Lead Persona:** Senior Backend Dev (Rust) & `@tech-lead`
- **Target Files:**
  - `src-tauri/src/translation/gemini_client.rs`
  - `src-tauri/src/translation/prompt_builder.rs`
  - `src/services/ipc/translation.ts`
- **Mô tả & Phạm vi Kỹ thuật:**
  - Xây dựng HTTP Client giao tiếp với Google Gemini 1.5 Flash API qua Google AI Studio endpoint.
  - Triển khai cơ chế đóng gói **Theatrical Framing System Prompt** (`ADR-001` & `PRD` Mục 6.3):
    Đặt bối cảnh LLM như một biên kịch chuyên nghiệp dịch phụ đề cho một vở hài kịch sitcom hư cấu; thiết lập `safetySettings: BLOCK_NONE` cho toàn bộ các danh mục nhạy cảm.
  - Triển khai **Defensive Safety Fallback**: Nếu Gemini vẫn kích hoạt cờ an toàn (`finishReason: SAFETY`) đối với một câu nói quá unhinged, hệ thống tự động giữ nguyên câu gốc tiếng Anh kèm cờ `is_flagged_for_review: true` và tiếp tục dịch các câu tiếp theo.
  - Áp dụng kỹ thuật phân đoạn hội thoại (Chunking 25–35 dòng) kèm cửa sổ trượt (Sliding Context Window 5 dòng trước) để giữ mạch xưng hô nhất quán.
- **Giao kèo dữ liệu (Data Contracts):**
  - Đầu vào: Mảng transcript tiếng Anh thô và Gemini API Key.
  - Đầu ra: Bản dịch tiếng Việt sơ bộ kèm thông tin gán vai.
- **Quan hệ phụ thuộc (Dependencies):**
  - Prerequisites: TASK-M4-02, TASK-M4-04, TASK-M4-15, TASK-M4-28.
  - Blocks: TASK-M4-18, TASK-M4-19.
- **Tiêu chí Nghiệm thu Định lượng (AC):**
  - [ ] **AC-17.1:** Thời gian dịch một phân đoạn 50 dòng thoại đạt $\le 4.0\text{ giây}$.
  - [ ] **AC-17.2:** Vượt qua bộ lọc an toàn của Google đối với các câu nói châm biếm đen tối của Neuro với tỷ lệ thành công $\ge 95\%$.
  - [ ] **AC-17.3:** Khi gặp câu bị chặn an toàn, ứng dụng bảo lưu câu tiếng Anh gốc và hiển thị cờ cảnh báo trên UI thay vì trả về lỗi rỗng.
- **Checklist Kiểm thử Chất lượng (@qa-reviewer QA Test Checklist):**
  - *Phân loại kiểm thử:* Mock HTTP Integration & Defensive Fallback Test
  - *Unit Test Cases:*
    - [ ] **UT-17.1 (Happy Path - Prompt Packaging):** Kiểm tra nội dung System Prompt gửi tới Gemini chứa đầy đủ khai báo Theatrical Framing và chỉ thị dịch thuật sang tiếng Việt tự nhiên.
    - [ ] **UT-17.2 (Safety Settings Validation):** Kiểm tra request JSON gửi đi bắt buộc chứa cấu hình `safetySettings: BLOCK_NONE` cho toàn bộ 4 nhóm nội dung.
  - *Integration & Contract Verification (`wiremock`):**
    - [ ] **IT-17.1:** Mock Gemini endpoint `v1beta/models/gemini-1.5-flash:generateContent`. Kiểm tra parse kết quả dịch trả về cho lô 30 câu thoại.
    - [ ] **IT-17.2:** Thời gian xử lý dịch 50 dòng thoại đạt $\le 4.0\text{ giây}$.
  - *Boundary & Edge Cases (ERR-07 Defensive Safety Fallback):**
    - [ ] **ED-17.1 (Safety Triggered Fallback):** Giả lập Gemini phản hồi `finishReason: "SAFETY"` trên 1 câu nói unhinged. Pipeline KHÔNG văng lỗi làm hủy mẻ dịch; tự động giữ nguyên câu tiếng Anh gốc, gắn cờ `is_flagged_for_review = true`, hiển thị cảnh báo trên UI và tiếp tục dịch bình thường các câu còn lại.
  - *Lệnh thực thi & Tiêu chí xác minh:*
    ```bash
    cargo test --test gemini_client_test -- --nocapture
    ```
    *Khẳng định: Vượt bộ lọc an toàn $\ge 95\%$; câu bị chặn an toàn được bảo lưu tiếng Anh gốc.*

---

#### TASK-M4-18: Từ Điển Thuật Ngữ Neuro Lore, Ma Trận Đại Từ Xưng Hô & Tự Động Gán Vai Ngữ Cảnh
- **Lead Persona:** `@product-owner-proxy` & Senior Backend Dev (Rust)
- **Target Files:**
  - `src-tauri/src/translation/lore_dictionary.rs`
  - `src-tauri/src/translation/pronoun_matrix.rs`
  - `src-tauri/src/translation/diarization.rs`
- **Mô tả & Phạm vi Kỹ thuật:**
  - Tích hợp từ điển miền tri thức cộng đồng Neuro-sama vào System Prompt:
    - *Tutel / Turtle* $\rightarrow$ "Con rùa" / "Bố rùa" (chỉ Vedal).
    - *Gymbag* $\rightarrow$ "Gymbag" / "Fan cuồng".
    - *Filter is leaking* $\rightarrow$ "Lộ bản chất rồi" / "Bộ lọc hỏng rồi".
    - *Dinking* $\rightarrow$ "Suy nghĩ sâu xa" / "Não to".
    - *Heart Heart* $\rightarrow$ "Bắn tim bắn tim".
  - Triển khai **Ma trận đại từ xưng hô (Pronoun Matrix)**:
    - Vedal nói với Neuro: xưng "anh/tôi" - gọi "em/con".
    - Neuro nói với Vedal: xưng "em/cháu" - gọi "bố/anh Vedal".
    - Evil nói với Vedal: xưng "chị/ta" - gọi "con rùa/lão già".
  - Phân tích ngữ cảnh câu nói để tự động điền `speaker_id`: `"vedal"`, `"neuro"`, `"evil"`, `"guest"`, `"chat"`.
- **Giao kèo dữ liệu (Data Contracts):**
  - Đầu vào: Transcript segments và cấu hình nhân vật tham gia stream.
  - Đầu ra: Segments có gắn `speaker_id` chuẩn xác và nội dung Vietsub bản địa hóa.
- **Quan hệ phụ thuộc (Dependencies):**
  - Prerequisites: TASK-M4-17.
  - Blocks: TASK-M4-19, TASK-M4-20, TASK-M4-22.
- **Tiêu chí Nghiệm thu Định lượng (AC):**
  - [ ] **AC-18.1:** 100% các thuật ngữ trong Lore Dictionary được dịch chuẩn nghĩa theo văn hóa Swarm VN, không có hiện tượng dịch word-by-word ngô nghê.
  - [ ] **AC-18.2:** Độ chính xác tự động gán nhãn người nói (`speaker_id`) dựa trên ngữ cảnh đạt $\ge 85\%$ trước khi người dùng cần can thiệp thủ công.
- **Checklist Kiểm thử Chất lượng (@qa-reviewer QA Test Checklist):**
  - *Phân loại kiểm thử:* Pure Logic Translation & Diarization Unit Test
  - *Unit Test Cases:*
    - [ ] **UT-18.1 (Lore Dictionary Match):**
      - `"Tutel"` $\rightarrow$ Assert kết quả dịch chứa "Con rùa" hoặc "Bố rùa".
      - `"Gymbag"` $\rightarrow$ Assert dịch thành "Gymbag" hoặc "Bao cát gym".
      - `"Filter is leaking"` $\rightarrow$ Assert dịch thành "Bộ lọc lủng rồi" hoặc "Lộ bản chất rồi".
      - `"Heart Heart"` $\rightarrow$ Assert dịch thành "Bắn tim bắn tim" / "Thả tim <3".
    - [ ] **UT-18.2 (Pronoun Matrix Consistency):**
      - Đoạn thoại Vedal nói với Neuro: chuẩn "anh/bố" - "em/con".
      - Đoạn thoại Evil nói với Vedal: xưng "chị/ta" - gọi "ông già/con rùa".
    - [ ] **UT-18.3 (Speaker ID Diarization):** Phân tích ngữ cảnh câu nói tự động gán đúng `speaker_id` trong `["vedal", "neuro", "evil", "guest", "chat"]` với độ chính xác $\ge 85\%$.
  - *Integration & Contract Verification:*
    - [ ] **IT-18.1:** Cập nhật trường `speaker_id` và `text_vi` vào mảng transcript segments.
  - *Lệnh thực thi & Tiêu chí xác minh:*
    ```bash
    cargo test --lib translation::lore_dictionary::tests
    ```
    *Khẳng định: 100% thuật ngữ trong PRD Mục 6 được dịch chuẩn theo văn hóa Swarm VN.*

---

#### TASK-M4-19: Động Cơ Ngắt Dòng Thông Minh Tiếng Việt & Tối Ưu Hóa Tốc Độ Đọc (CPS)
- **Lead Persona:** Senior Backend Dev (Rust) & Senior Frontend Dev
- **Target Files:**
  - `src-tauri/src/translation/line_wrapper.rs`
  - `src-tauri/src/translation/cps_optimizer.rs`
  - `src/utils/text_wrap.ts`
- **Mô tả & Phạm vi Kỹ thuật:**
  - Áp dụng thuật toán ngắt dòng bảo toàn cấu trúc ngữ pháp:
    - Giới hạn tối đa $38 - 42\text{ ký tự/dòng}$, tối đa 2 dòng trên màn hình.
    - Không bao giờ ngắt đôi một từ ghép tiếng Việt ở cuối dòng (cấm ngắt *"bánh / mì"*, *"trí / tuệ"*).
  - Tính toán chỉ số tốc độ đọc CPS: $\text{CPS} = \text{Ký tự} / \text{Thời lượng (s)}$.
  - Áp dụng thuật toán **Mượn khoảng lặng (Time Borrowing)**:
    Nếu $\text{CPS} > 20$, tự động nới rộng mốc `end_sec` sang phải tối đa $800\text{ms}$ vào khoảng lặng kế tiếp để hạ CPS về mức an toàn $\le 17\text{ CPS}$.
  - Sinh và lưu tệp `transcript_segments.json` tuân thủ Schema 5.5 trong `ARCHITECTURE.md`.
- **Giao kèo dữ liệu (Data Contracts):**
  - Đầu vào: Segments tiếng Việt thô và speech timestamps.
  - Đầu ra: `transcript_segments.json` (Tuân thủ Schema 5.5: `clip_id`, `segments: [{ segment_id, start_sec, end_sec, speaker_id, text_en, text_vi, cps_score }]`).
- **Quan hệ phụ thuộc (Dependencies):**
  - Prerequisites: TASK-M4-18.
  - Blocks: TASK-M4-20, TASK-M4-21, TASK-M4-23, TASK-M4-24.
- **Tiêu chí Nghiệm thu Định lượng (AC):**
  - [ ] **AC-19.1:** 100% các dòng phụ đề tiếng Việt tuân thủ giới hạn $\le 42\text{ ký tự/dòng}$ và không có dòng nào bị bẻ đôi từ ghép.
  - [ ] **AC-19.2:** Thuật toán Time Borrowing nới rộng thời gian thành công cho các câu nói nhanh mà không bao giờ đè lên mốc bắt đầu của câu thoại kế tiếp.
  - [ ] **AC-19.3:** File `transcript_segments.json` sinh ra hợp lệ 100% với JSON Schema 5.5.
- **Checklist Kiểm thử Chất lượng (@qa-reviewer QA Test Checklist):**
  - *Phân loại kiểm thử:* Pure Logic String & Math Unit Test
  - *Unit Test Cases:*
    - [ ] **UT-19.1 (Smart Compound Wrapping):** Đưa vào chuỗi tiếng Việt dài 50 ký tự chứa từ ghép `"bánh mì"` và `"bố Vedal"`. Assert thuật toán bẻ dòng thành 2 dòng $\le 42$ ký tự, điểm ngắt dòng tuyệt đối không nằm giữa `"bánh"` và `"mì"`.
    - [ ] **UT-19.2 (CPS Math):** Câu thoại có 40 ký tự tiếng Việt hiển thị trong $1.5\text{ giây} \rightarrow \text{CPS} = 40 / 1.5 \approx 26.7\text{ CPS} > 20$ (Vượt ngưỡng đỏ).
    - [ ] **UT-19.3 (Time Borrowing Calculation):** Câu thoại có $\text{CPS} = 26.7$, phía sau có khoảng lặng $1.0\text{s}$ trước câu kế tiếp. Thuật toán tự động nới rộng mốc `end_sec` thêm $0.8\text{s}$ $\rightarrow \text{CPS mới} = 40 / 2.3 \approx 17.4\text{ CPS} \le 20$.
  - *Integration & Contract Verification (Schema 5.5):**
    - [ ] **IT-19.1:** Xuất ra tệp `transcript_segments.json` tuân thủ 100% JSON Schema 5.5 trong `ARCHITECTURE.md`.
  - *Boundary & Edge Cases:*
    - [ ] **ED-19.1 (Collision Avoidance):** Phép mượn thời gian không bao giờ kéo dài `end_sec` đè lên hoặc vượt quá `start_sec` của câu thoại tiếp theo (khoảng cách tối thiểu $50\text{ms}$).
  - *Lệnh thực thi & Tiêu chí xác minh:*
    ```bash
    cargo test --lib translation::line_wrapper::tests
    cargo test --lib translation::cps_optimizer::tests
    ```
    *Khẳng định: Không ngắt đôi từ ghép tiếng Việt, `transcript_segments.json` schema valid.*

---

### Milestone M4.6: Trình Soạn Thảo Phụ Đề Tinh Gọn (Lean Subtitle Studio UI)

#### TASK-M4-20: Màn Hình Biên Tập Subtitle Studio & Khung Xem Video Đồng Bộ Timeline
- **Lead Persona:** Senior Frontend Dev & `@tech-lead`
- **Target Files:**
  - `src/components/studio/SubtitleStudio.tsx`
  - `src/components/studio/VideoCanvas.tsx`
  - `src/components/studio/TimelineScrubber.tsx`
  - `src/stores/useStudioStore.ts`
- **Mô tả & Phạm vi Kỹ thuật:**
  - Xây dựng giao diện Subtitle Studio gồm: Trình phát video và Danh sách phụ đề.
  - Sử dụng thẻ HTML5 `<video>` kết nối qua custom protocol `asset://` để phát mượt mà video phân đoạn (`clip_slice.mp4`).
  - Hỗ trợ **2 Preset Cố Định Cho Video Dọc 9:16** (`ADR-005`):
    - *Preset 1: Center-Crop 9:16* (Cắt lấy trung tâm khung hình 16:9).
    - *Preset 2: Two-stack Top-Bottom 50/50* (Nửa trên stream gameplay, nửa dưới avatar với vạch ngăn cố định).
  - Đồng bộ hai chiều (Bi-directional Sync): Khi video chạy, tự động cuộn đến dòng phụ đề tương ứng; khi click dòng phụ đề, video tự nhảy đến mốc thời gian đó.
- **Giao kèo dữ liệu (Data Contracts):**
  - Đầu vào: `clip_slice.mp4` và `transcript_segments.json`.
  - Đầu ra: Trạng thái playback đồng bộ trong Zustand `useStudioStore`.
- **Quan hệ phụ thuộc (Dependencies):**
  - Prerequisites: TASK-M4-01, TASK-M4-07, TASK-M4-13, TASK-M4-19.
  - Blocks: TASK-M4-21, TASK-M4-22, TASK-M4-23.
- **Tiêu chí Nghiệm thu Định lượng (AC):**
  - [ ] **AC-20.1:** Video phát mượt mà $60\text{ FPS}$, không giật hình khi tua timeline.
  - [ ] **AC-20.2:** Chuyển đổi qua lại giữa 2 Preset 9:16 tức thì trong $\le 50\text{ms}$ mà không cần re-render toàn bộ video.
  - [ ] **AC-20.3:** Độ trễ đồng bộ giữa vị trí đầu đọc video và dòng phụ đề được active $\le 30\text{ms}$.
- **Checklist Kiểm thử Chất lượng (@qa-reviewer QA Test Checklist):**
  - *Phân loại kiểm thử:* Frontend React Component & Video Playback Test
  - *Unit Test Cases:*
    - [ ] **UT-20.1 (Preset Switch):** Chọn chuyển đổi giữa `Preset 1 (Center-Crop 9:16)` và `Preset 2 (Two-stack Top-Bottom 50/50)` cập nhật đúng CSS layout styles và video container aspect ratio.
  - *Integration & Contract Verification:*
    - [ ] **IT-20.1:** Video HTML5 nạp qua giao thức `asset://` phát video 1080p60 mượt mà.
    - [ ] **IT-20.2 (Bi-directional Sync):** Khi video phát tới $t = 15.4\text{s}$, danh sách phụ đề tự động active dòng phụ đề bao hàm mốc thời gian đó; độ trễ đồng bộ $\le 30\text{ms}$.
  - *Boundary & Performance Limits:*
    - [ ] **ED-20.1:** Kéo tua thanh scrubber liên tục trong 10 giây, FPS của video canvas không tụt dưới 55 FPS.
  - *Lệnh thực thi & Tiêu chí xác minh:*
    ```bash
    npm run test -- src/components/studio/SubtitleStudio.test.tsx
    ```
    *Khẳng định: Render mượt mà $60\text{ FPS}$, độ trễ đồng bộ timeline $\le 30\text{ms}$.*

---

#### TASK-M4-21: Danh Sách Phụ Đề Ảo Hóa Hiệu Năng Cao & Biên Tập Trực Tiếp (Virtualized Segment List)
- **Lead Persona:** Senior Frontend Dev
- **Target Files:**
  - `src/components/studio/VirtualizedSegmentList.tsx`
  - `src/components/studio/SegmentRow.tsx`
  - `src/components/studio/TimecodeNudge.tsx`
- **Mô tả & Phạm vi Kỹ thuật:**
  - Sử dụng thư viện ảo hóa `@tanstack/react-virtual` để hiển thị danh sách hàng trăm dòng phụ đề mà không gây tụt FPS hay lag trình duyệt.
  - Mỗi hàng phụ đề (`SegmentRow`) hiển thị:
    - Badge Người nói (Speaker) có màu sắc tương ứng.
    - Mốc thời gian In/Out (phút:giây.mili-giây).
    - Ô chỉnh sửa nhanh Text tiếng Anh và Text tiếng Việt (Inline Textarea tự co giãn).
    - Nút Nudge thời gian nhanh: Phím tắt `A` lùi $100\text{ms}$, `D` tiến $100\text{ms}$ cho mốc bắt đầu/kết thúc.
- **Giao kèo dữ liệu (Data Contracts):**
  - Đầu vào: Danh sách `segments` từ Zustand store.
  - Đầu ra: Cập nhật trực tiếp `segments` trong bộ nhớ và lưu vào `subtitle_project.json`.
- **Quan hệ phụ thuộc (Dependencies):**
  - Prerequisites: TASK-M4-20.
  - Blocks: TASK-M4-22, TASK-M4-23, TASK-M4-24.
- **Tiêu chí Nghiệm thu Định lượng (AC):**
  - [ ] **AC-21.1:** Tốc độ cuộn danh sách phụ đề đạt $60\text{ FPS}$ ngay cả với các dự án dài hơn 500 dòng thoại.
  - [ ] **AC-21.2:** Các thao tác sửa văn bản phản hồi tức thì dưới $16\text{ms}$ (0 input lag).
  - [ ] **AC-21.3:** Nudge thời gian cập nhật chính xác từng $100\text{ms}$ vào state và hiển thị ngay trên UI.
- **Checklist Kiểm thử Chất lượng (@qa-reviewer QA Test Checklist):**
  - *Phân loại kiểm thử:* Frontend Virtualization & State Management Test
  - *Unit Test Cases:*
    - [ ] **UT-21.1 (Timecode Nudge):** Bấm phím tắt `Shift + A` giảm mốc thời gian $100\text{ms}$, `Shift + D` tăng mốc thời gian $100\text{ms}$ chính xác vào store.
    - [ ] **UT-21.2 (Inline Edit):** Gõ sửa nội dung trong ô Textarea tiếng Việt cập nhật giá trị `text_vi` vào Zustand store tức thì.
  - *Integration & Contract Verification:*
    - [ ] **IT-21.1:** Render danh sách chứa 1,000 dòng phụ đề bằng `@tanstack/react-virtual`. Kiểm tra DOM chỉ render số lượng DOM nodes tương ứng với viewport (15–20 rows).
  - *Boundary & Performance Limits:*
    - [ ] **ED-21.1:** Cuộn danh sách 1,000 dòng với tốc độ cao đạt $60\text{ FPS}$; độ trễ phản hồi khi gõ phím sửa text $\le 16\text{ms}$.
  - *Lệnh thực thi & Tiêu chí xác minh:*
    ```bash
    npm run test -- src/components/studio/VirtualizedSegmentList.test.tsx
    ```
    *Khẳng định: DOM nodes ảo hóa $\le 25$ phần tử, cuộn mượt mà 60 FPS.*

---

#### TASK-M4-22: Bàn Phím Số 1-Chạm Đổi Phong Cách Nhân Vật & Thanh Công Cụ Đa Chọn
- **Lead Persona:** Senior Frontend Dev
- **Target Files:**
  - `src/components/studio/CharacterHotkeyBar.tsx`
  - `src/hooks/useStudioHotkeys.ts`
- **Mô tả & Phạm vi Kỹ thuật:**
  - Bắt sự kiện bàn phím toàn cục trên màn hình Subtitle Studio (`ADR-006`):
    - **Phím `1`:** Đổi người nói thành **Vedal** (Màu xanh rêu `#98FB98`, viền đen).
    - **Phím `2`:** Đổi người nói thành **Neuro-sama** (Màu hồng pastel `#FFB6C1`, viền đen đậm).
    - **Phím `3`:** Đổi người nói thành **Evil Neuro** (Màu đỏ `#FF0055`, viền đen sắc cạnh).
    - **Phím `4`:** Đổi người nói thành **Khách mời / Collab** (Màu vàng kim `#FFD700`).
    - **Phím `5`:** Đổi người nói thành **Twitch Chat / Swarm** (Màu xanh dương `#87CEEB`).
  - Hỗ trợ thao tác chọn nhiều dòng bằng `Shift + Click` hoặc `Ctrl + Click`: Người dùng có thể bôi đen nhiều dòng thoại cùng lúc và bấm phím số để đổi style hàng loạt.
- **Giao kèo dữ liệu (Data Contracts):**
  - Đầu vào: Sự kiện phím bấm `keydown` (`1`, `2`, `3`, `4`, `5`).
  - Đầu ra: Thay đổi trường `speaker_id` của các segments được chọn.
- **Quan hệ phụ thuộc (Dependencies):**
  - Prerequisites: TASK-M4-20, TASK-M4-21.
  - Blocks: TASK-M4-24.
- **Tiêu chí Nghiệm thu Định lượng (AC):**
  - [ ] **AC-22.1:** Thao tác bấm phím 1–5 đổi màu và vai nhân vật hoàn tất trong vòng $\le 50\text{ms}$ (cảm nhận tức thì 0ms).
  - [ ] **AC-22.2:** Gán vai thành công cho cùng lúc $\ge 20$ dòng phụ đề được bôi đen mà không có lỗi state.
- **Checklist Kiểm thử Chất lượng (@qa-reviewer QA Test Checklist):**
  - *Phân loại kiểm thử:* Frontend Hotkey & Styling Test
  - *Unit Test Cases (Bộ phím tắt 1-5):*
    - [ ] **UT-22.1 (Key '1' - Vedal):** Bấm phím `1` gán `speaker_id = "vedal"`, đổi style màu chữ thành xanh lá `#98FB98`.
    - [ ] **UT-22.2 (Key '2' - Neuro):** Bấm phím `2` gán `speaker_id = "neuro"`, đổi style màu chữ thành hồng phấn `#FFB6C1`.
    - [ ] **UT-22.3 (Key '3' - Evil):** Bấm phím `3` gán `speaker_id = "evil"`, đổi style màu chữ thành đỏ `#FF0055`.
    - [ ] **UT-22.4 (Key '4' - Guest):** Bấm phím `4` gán `speaker_id = "guest"`, đổi style màu chữ thành vàng kim `#FFD700`.
    - [ ] **UT-22.5 (Key '5' - Chat):** Bấm phím `5` gán `speaker_id = "chat"`, đổi style màu chữ thành xanh dương `#87CEEB`.
  - *Integration & Contract Verification:*
    - [ ] **IT-22.1:** Bôi đen chọn nhiều dòng (Multi-select 20 rows qua `Shift+Click`), bấm phím `2` gán đồng loạt cả 20 dòng sang Neuro trong vòng 1 lượt bấm phím.
  - *Boundary & UI Speed Limits:*
    - [ ] **ED-22.1:** Thao tác bấm phím đổi màu nhân vật phản hồi lên màn hình trong $\le 50\text{ms}$.
  - *Lệnh thực thi & Tiêu chí xác minh:*
    ```bash
    npm run test -- src/hooks/useStudioHotkeys.test.ts
    ```
    *Khẳng định: Gán vai 1-chạm hoàn tất $\le 50\text{ms}$, multi-selection 20 rows không lỗi.*

---

#### TASK-M4-23: Đèn Cảnh Báo Tốc Độ Đọc Trực Quan & Trợ Lý Nới Rộng Thời Gian Thủ Công
- **Lead Persona:** Senior Frontend Dev
- **Target Files:**
  - `src/components/studio/CPSIndicator.tsx`
  - `src/utils/cps.ts`
- **Mô tả & Phạm vi Kỹ thuật:**
  - Hiển thị badge chỉ số CPS cạnh mỗi dòng phụ đề:
    - Màu Xanh lá: $\text{CPS} < 17$ (Tốc độ đọc lý tưởng).
    - Màu Vàng: $17 \le \text{CPS} \le 20$ (Tốc độ đọc nhanh nhưng chấp nhận được).
    - Màu Đỏ nhấp nháy: $\text{CPS} > 20$ (Quá nhanh, người xem đọc không kịp!).
  - Khi đèn đỏ bật, hiển thị nút bấm 1-chạm *"Nới thời gian (Borrow Silence)"*: Cho phép người dùng kéo dài thời gian hiển thị câu thoại vào khoảng lặng kế tiếp chỉ bằng một click chuột.
- **Giao kèo dữ liệu (Data Contracts):**
  - Đầu vào: `text_vi` và thời lượng `(end_sec - start_sec)`.
  - Đầu ra: Giá trị CPS và badge giao diện tương ứng.
- **Quan hệ phụ thuộc (Dependencies):**
  - Prerequisites: TASK-M4-19, TASK-M4-21.
  - Blocks: TASK-M4-24.
- **Tiêu chí Nghiệm thu Định lượng (AC):**
  - [ ] **AC-23.1:** Chỉ số CPS được tính toán lại tức thì theo thời gian thực mỗi khi người dùng gõ sửa ký tự trong ô văn bản.
  - [ ] **AC-23.2:** Hiển thị cảnh báo đỏ chính xác 100% cho mọi dòng phụ đề có $\text{CPS} > 20$.
- **Checklist Kiểm thử Chất lượng (@qa-reviewer QA Test Checklist):**
  - *Phân loại kiểm thử:* Frontend Component & User Interaction Test
  - *Unit Test Cases:*
    - [ ] **UT-23.1 (Badge Color Logic):**
      - $\text{CPS} < 17 \rightarrow$ Badge màu Xanh lá ("Tốt").
      - $17 \le \text{CPS} \le 20 \rightarrow$ Badge màu Vàng ("Nhanh").
      - $\text{CPS} > 20 \rightarrow$ Badge màu Đỏ nhấp nháy ("Quá nhanh!").
  - *Integration & Contract Verification:*
    - [ ] **IT-23.1:** Khi đèn đỏ bật, click nút *"Nới thời gian (Borrow Silence)"* tự động gọi hàm Time Borrowing, kéo dài `end_sec` và lập tức chuyển badge từ Đỏ sang Vàng hoặc Xanh.
  - *Boundary & Realtime Limits:*
    - [ ] **ED-23.1:** Khi người dùng đang gõ thêm hoặc xóa ký tự trong ô text, badge CPS tính toán lại và cập nhật màu sắc ngay theo thời gian thực.
  - *Lệnh thực thi & Tiêu chí xác minh:*
    ```bash
    npm run test -- src/components/studio/CPSIndicator.test.tsx
    ```
    *Khẳng định: Cảnh báo đỏ chính xác 100% khi $\text{CPS} > 20$, 1-click borrow silence hạ CPS tức thì.*

---

### Milestone M4.7: Phân Hệ Xuất Bản Đa Nền Tảng & NLE Export (Export Engine)

#### TASK-M4-24: Bộ Xuất Phụ Đề Chuẩn Màu Aegisub (.ASS) & Phụ Đề Quốc Tế (.SRT)
- **Lead Persona:** Senior Backend Dev (Rust) & `@tech-lead`
- **Target Files:**
  - `src-tauri/src/export/ass_exporter.rs`
  - `src-tauri/src/export/srt_exporter.rs`
  - `src-tauri/src/commands/export_cmd.rs`
  - `src/services/ipc/export.ts`
- **Mô tả & Phạm vi Kỹ thuật:**
  - Xây dựng module sinh tệp `.ass` (Advanced SubStation Alpha v4.00+):
    - Khởi tạo khối `[V4+ Styles]` với định nghĩa màu sắc, viền, bóng đổ riêng cho từng nhân vật theo bảng màu chuẩn:
      - `Style: Vedal`: Màu chữ `#98FB98`, Outline đen 2.5px.
      - `Style: Neuro`: Màu chữ `#FFB6C1`, Outline đen đậm 3.0px.
      - `Style: Evil`: Màu chữ `#FF0055`, Outline đen sắc nét 3.0px.
      - `Style: Guest`: Màu chữ `#FFD700`.
      - `Style: Chat`: Màu chữ `#87CEEB`.
    - Sinh khối `[Events]` chứa các dòng hội thoại với timecode chuẩn `0:00:00.00`.
  - Xây dựng module sinh tệp `.srt` chuẩn UTF-8 (hỗ trợ đầy đủ dấu thanh tiếng Việt).
  - Tích hợp và lưu vào tệp tổng thể `subtitle_project.json` tuân thủ Schema 5.6 trong `ARCHITECTURE.md`.
- **Giao kèo dữ liệu (Data Contracts):**
  - Đầu vào: `subtitle_project.json` (danh sách segments, styles, project metadata).
  - Đầu ra: File `subtitle.ass` và `subtitle.srt` xuất ra thư mục người dùng chỉ định.
- **Quan hệ phụ thuộc (Dependencies):**
  - Prerequisites: TASK-M4-19, TASK-M4-21, TASK-M4-22.
  - Blocks: TASK-M4-25, TASK-M4-26.
- **Tiêu chí Nghiệm thu Định lượng (AC):**
  - [ ] **AC-24.1:** File `.ass` mở trên phần mềm Aegisub hiển thị chính xác 100% màu sắc, font chữ và không bị lỗi hiển thị ký tự tiếng Việt (UTF-8 with BOM).
  - [ ] **AC-24.2:** Thời gian sinh file `.ass` và `.srt` cho dự án 300 câu thoại $\le 200\text{ms}$.
- **Checklist Kiểm thử Chất lượng (@qa-reviewer QA Test Checklist):**
  - *Phân loại kiểm thử:* Subtitle Serialization & Format Standards Test
  - *Unit Test Cases:*
    - [ ] **UT-24.1 (ASS Color Conversion):** Màu Hex Vedal `#98FB98` chuyển đổi chính xác sang mã màu ASS BGR `&H0098FB98&`. Màu Neuro `#FFB6C1` chuyển sang ASS BGR `&H00C1B6FF&`.
    - [ ] **UT-24.2 (ASS Header V4+ Styles):** Khối `[V4+ Styles]` chứa đầy đủ định nghĩa style riêng cho từng nhân vật với đúng font chữ, size, outline, shadow.
    - [ ] **UT-24.3 (SRT Timecode Format):** Khối SRT sinh chuẩn định dạng timecode `00:01:23,450 --> 00:01:25,800`.
  - *Integration & Contract Verification (Schema 5.6):**
    - [ ] **IT-24.1:** Sinh tệp `subtitle_project.json` tuân thủ 100% JSON Schema 5.6 trong `ARCHITECTURE.md`.
    - [ ] **IT-24.2:** File `.ass` và `.srt` được ghi ra đĩa với encoding UTF-8 có BOM.
  - *Boundary & Encoding Limits:*
    - [ ] **ED-24.1:** Xuất 300 câu phụ đề tiếng Việt chứa đầy đủ dấu thanh phức tạp (`ể`, `ễ`, `ợ`, `ứ`, `đ`); mở file `.ass` trên Aegisub hiển thị hoàn hảo 100% không có ký tự rác.
    - [ ] **ED-24.2:** Thời gian sinh file `.ass` và `.srt` cho dự án 300 câu $\le 200\text{ms}$.
  - *Lệnh thực thi & Tiêu chí xác minh:*
    ```bash
    cargo test --lib export::ass_exporter::tests
    cargo test --lib export::srt_exporter::tests
    ```
    *Khẳng định: File .ass và .srt chuẩn format, mở trên Aegisub chuẩn màu và tiếng Việt 100%.*

---

#### TASK-M4-25: Bộ Sinh Thư Mục Dự Án CapCut PC Draft JSON Kèm Chiến Lược Phòng Thủ Kép
- **Lead Persona:** Senior Backend Dev (Rust) & `@lead-architect`
- **Target Files:**
  - `src-tauri/src/export/capcut_draft.rs`
  - `src-tauri/src/export/capcut_schema.rs`
  - `src/components/export/ExportModal.tsx`
- **Mô tả & Phạm vi Kỹ thuật:**
  - Tự động dò tìm thư mục dự án CapCut PC trên Windows:
    `%LOCALAPPDATA%/CapCut/User Data/Projects/com.lveditor.draft/`.
  - Tạo một thư mục dự án CapCut mới: `NeuroClip_Highlight_<timestamp>/`.
  - Sinh tệp `draft_content.json`: Cấu trúc JSON dịch ngược của CapCut PC:
    - Track 0: Video track chứa file video phân đoạn (`clip_slice.mp4`).
    - Track 1: Audio track đồng bộ.
    - Track 2: Text material track chứa toàn bộ các câu phụ đề tiếng Việt với vị trí tọa độ, kích thước font chữ, màu sắc theo vai nhân vật.
  - Sinh tệp `draft_meta_info.json`.
  - Kích hoạt **Chiến lược Phòng thủ Kép (Dual Export Safeguard)** (`ADR-005`): Luôn tự động copy kèm file `subtitle.ass` và `subtitle.srt` vào cùng thư mục dự án của CapCut để người dùng không bao giờ mất phụ đề nếu CapCut cập nhật phiên bản đổi schema JSON.
- **Giao kèo dữ liệu (Data Contracts):**
  - Đầu vào: Dữ liệu dự án từ `subtitle_project.json` và video slice path.
  - Đầu ra: Thư mục CapCut Draft chứa `draft_content.json`, `draft_meta_info.json`, `subtitle.ass`, `subtitle.srt`.
- **Quan hệ phụ thuộc (Dependencies):**
  - Prerequisites: TASK-M4-13, TASK-M4-24.
  - Blocks: TASK-M4-30.
- **Tiêu chí Nghiệm thu Định lượng (AC):**
  - [ ] **AC-25.1:** Mở CapCut PC lên thấy ngay dự án mới xuất hiện trên màn hình chính của CapCut; mở dự án thấy sẵn video và track chữ nằm khớp dòng thời gian.
  - [ ] **AC-25.2:** Thời gian sinh thư mục CapCut Draft hoàn tất trong vòng $\le 200\text{ms}$.
  - [ ] **AC-25.3:** 100% các lần xuất CapCut Draft đều có mặt 2 file dự phòng `subtitle.ass` và `subtitle.srt` trong thư mục.
- **Checklist Kiểm thử Chất lượng (@qa-reviewer QA Test Checklist):**
  - *Phân loại kiểm thử:* NLE Integration & Dual Safeguard Test
  - *Unit Test Cases:*
    - [ ] **UT-25.1 (Draft Structure Construction):** Sinh struct `draft_content.json` chứa: Track 0 (Video track), Track 1 (Audio track), Track 2 (Text subtitle track) với tọa độ canh giữa dưới đáy khung hình.
  - *Integration & Contract Verification:*
    - [ ] **IT-25.1:** Tự động phát hiện đúng đường dẫn CapCut Windows: `%LOCALAPPDATA%/CapCut/User Data/Projects/com.lveditor.draft/`.
    - [ ] **IT-25.2:** Tạo thư mục dự án `NeuroClip_Highlight_<timestamp>/` chứa `draft_content.json` và `draft_meta_info.json`. Mở CapCut PC lên nhận diện dự án ngay tại màn hình trang chủ.
  - *Boundary & Fallback Limits (ERR-09 Dual Export Invariant):**
    - [ ] **ED-25.1 (Chiến Lược Phòng Thủ Kép):** Bất kể xuất CapCut thành công hay thất bại, hệ thống LUÔN TỰ ĐỘNG sao lưu 2 file độc lập `subtitle.ass` và `subtitle.srt` vào cùng thư mục dự án đó.
    - [ ] **ED-25.2 (CapCut Schema Safeguard):** Kiểm tra file backup tồn tại 100% trong mọi lần test xuất bản; người dùng luôn có phương án kéo thả file `.srt` vào CapCut nếu phiên bản CapCut mới thay đổi schema JSON.
    - [ ] **ED-25.3:** Thời gian sinh toàn bộ thư mục CapCut Draft $\le 200\text{ms}$.
  - *Lệnh thực thi & Tiêu chí xác minh:*
    ```bash
    cargo test --test capcut_draft_integration_test -- --nocapture
    ```
    *Khẳng định: 100% thư mục CapCut Draft có chứa kèm file `subtitle.ass` và `subtitle.srt`.*

---

#### TASK-M4-26: Bộ Xuất Dòng Thời Gian Premiere Pro XML & FCP7 XML
- **Lead Persona:** Senior Backend Dev (Rust)
- **Target Files:**
  - `src-tauri/src/export/premiere_xml.rs`
- **Mô tả & Phạm vi Kỹ thuật:**
  - Sinh tệp XML theo chuẩn định dạng Final Cut Pro 7 / Premiere XML Interchange Format:
    - Thẻ `<sequence>`: Cấu hình tỷ lệ khung hình $1920\times 1080$ hoặc $1080\times 1920$ (nếu chọn preset dọc 9:16).
    - Thẻ `<track>`: Video track chứa các clip đã cắt và Audio track tương ứng.
    - Đặt các Marker chứa nội dung phụ đề tiếng Việt đồng bộ mốc thời gian.
  - Đảm bảo tương thích hoàn hảo khi kéo thả file XML vào Adobe Premiere Pro 2022–2025 và DaVinci Resolve 18–19.
- **Giao kèo dữ liệu (Data Contracts):**
  - Đầu vào: `subtitle_project.json` và danh sách video clips.
  - Đầu ra: Tệp `project_timeline.xml`.
- **Quan hệ phụ thuộc (Dependencies):**
  - Prerequisites: TASK-M4-13, TASK-M4-24.
  - Blocks: TASK-M4-30.
- **Tiêu chí Nghiệm thu Định lượng (AC):**
  - [ ] **AC-26.1:** Import file XML vào Premiere Pro không bị báo lỗi "The project appears to be damaged" hoặc mất liên kết media.
  - [ ] **AC-26.2:** Thời gian sinh file XML $\le 100\text{ms}$.
- **Checklist Kiểm thử Chất lượng (@qa-reviewer QA Test Checklist):**
  - *Phân loại kiểm thử:* NLE Interchange XML Test
  - *Unit Test Cases:*
    - [ ] **UT-26.1 (XML Schema Compliance):** Sinh thẻ `<xmeml version="4">`, sequence 1920x1080 (hoặc 1080x1920 nếu chọn preset dọc 9:16), frame rate 60fps, đồng bộ video/audio tracks và markers.
  - *Integration & Contract Verification:*
    - [ ] **IT-26.1:** Import file `project_timeline.xml` vào Adobe Premiere Pro và DaVinci Resolve không bị báo lỗi cú pháp XML hay mất liên kết media clips.
  - *Boundary & Speed Limits:*
    - [ ] **ED-26.1:** Thời gian sinh file XML cho clip highlight $\le 100\text{ms}$.
  - *Lệnh thực thi & Tiêu chí xác minh:*
    ```bash
    cargo test --lib export::premiere_xml::tests
    ```
    *Khẳng định: XML schema valid, import mượt mà trên Premiere Pro.*

---

### Milestone M4.8: Khởi Động Lần Đầu, Bảo Mật Keyring & Quản Lý Sidecars (System Integration)

#### TASK-M4-27: Động Cơ Thăm Dò Năng Lực Phần Cứng & Đề Xuất Cấu Hình Thông Minh
- **Lead Persona:** Senior Backend Dev (Rust) & Systems Dev
- **Target Files:**
  - `src-tauri/src/hardware/probe.rs`
  - `src-tauri/src/hardware/wmi.rs`
  - `src-tauri/src/commands/hardware_cmd.rs`
  - `src/components/onboarding/HardwareRecommendation.tsx`
  - `src/stores/useHardwareStore.ts`
- **Mô tả & Phạm vi Kỹ thuật:**
  - Xây dựng module `HardwareProbeEngine` sử dụng crate `sysinfo` và Windows Win32 API / DirectX DXGI để đọc thông số máy tính:
    - Số nhân/luồng CPU, Model CPU.
    - Dung lượng RAM tổng cộng và RAM khả dụng (MB).
    - Card màn hình: Vendor, Model, Dung lượng VRAM thực tế (MB), Hỗ trợ CUDA.
  - Triển khai thuật toán đề xuất cấu hình:
    - Nếu GPU có VRAM $\ge 8\text{GB}$ (NVIDIA RTX): Đề xuất chế độ **Local Faster-Whisper** hoặc **Hybrid**.
    - Nếu GPU có VRAM $4\text{GB} - 6\text{GB}$: Đề xuất chế độ **Hybrid** (Groq STT Cloud + Local Video Processing).
    - Nếu chỉ có iGPU hoặc RAM $\le 8\text{GB}$: Đề xuất chế độ **Cloud BYOK** để máy chạy nhẹ nhất.
  - Xuất ra cấu trúc `hardware_profile.json` theo đúng Schema 5.1 trong `ARCHITECTURE.md`.
- **Giao kèo dữ liệu (Data Contracts):**
  - Đầu vào: Hệ điều hành Windows OS.
  - Đầu ra: `hardware_profile.json` (Tuân thủ Schema 5.1: `cpu_info`, `ram_info`, `gpu_info`, `recommended_mode`, `recommendation_reason`).
- **Quan hệ phụ thuộc (Dependencies):**
  - Prerequisites: TASK-M4-01, TASK-M4-02.
  - Blocks: TASK-M4-16, TASK-M4-28.
- **Tiêu chí Nghiệm thu Định lượng (AC):**
  - [ ] **AC-27.1:** Quá trình quét phần cứng hoàn tất trong vòng $\le 200\text{ms}$ ngay khi app vừa bật.
  - [ ] **AC-27.2:** Nhận diện chính xác 100% dung lượng VRAM thực tế của card đồ họa rời NVIDIA (không bị nhầm với Shared System Memory).
  - [ ] **AC-27.3:** File `hardware_profile.json` hợp lệ 100% với JSON Schema 5.1.
- **Checklist Kiểm thử Chất lượng (@qa-reviewer QA Test Checklist):**
  - *Phân loại kiểm thử:* Hardware Detection & Contract Test (Schema 5.1)
  - *Unit Test Cases (Thuật toán khuyến nghị cấu hình):*
    - [ ] **UT-27.1 (iGPU / Low RAM):** CPU i5-1135G7, 8GB RAM, Intel Iris Xe $\rightarrow$ Đề xuất `recommended_mode = "cloud_byok"` kèm lý do "Máy tính văn phòng/iGPU nên dùng Cloud để xử lý siêu tốc và không nóng máy".
    - [ ] **UT-27.2 (Mid-range GPU):** CPU Ryzen 5 5600H, 16GB RAM, NVIDIA GTX 1650 (4GB VRAM) $\rightarrow$ Đề xuất `recommended_mode = "hybrid"`.
    - [ ] **UT-27.3 (High-end GPU):** CPU i7-13700K, 32GB RAM, NVIDIA RTX 3080 (10GB VRAM) $\rightarrow$ Đề xuất `recommended_mode = "offline_local"` hoặc `"hybrid"`.
  - *Integration & Contract Verification (Schema 5.1):**
    - [ ] **IT-27.1:** Đọc thông số qua Win32 WMI / DXGI; xuất file `hardware_profile.json` tuân thủ 100% JSON Schema 5.1 trong `ARCHITECTURE.md`.
    - [ ] **IT-27.2:** Quá trình quét phần cứng hoàn tất trong $\le 200\text{ms}$ khi khởi động ứng dụng.
  - *Boundary & Hardware Precision:*
    - [ ] **ED-27.1:** Nhận diện chính xác 100% Dedicated VRAM của card NVIDIA rời (không bị nhầm lẫn với Shared Memory của Windows).
  - *Lệnh thực thi & Tiêu chí xác minh:*
    ```bash
    cargo test --lib hardware::probe::tests
    ```
    *Khẳng định: Thời gian probe $\le 200\text{ms}$, `hardware_profile.json` schema valid.*

---

#### TASK-M4-28: Giao Diện Wizard Khởi Chạy Lần Đầu & Lưu Trữ API Key Vào Két Sắt Windows Credential
- **Lead Persona:** Senior Frontend Dev & Senior Backend Dev (Rust)
- **Target Files:**
  - `src/components/onboarding/OnboardingWizardModal.tsx`
  - `src/components/onboarding/ApiKeyTester.tsx`
  - `src-tauri/src/core/keyring.rs`
  - `src-tauri/src/commands/config_cmd.rs`
- **Mô tả & Phạm vi Kỹ thuật:**
  - Hiển thị `OnboardingWizardModal` ở lần khởi động đầu tiên (kiểm tra cờ `is_first_run` trong config).
  - Hiển thị bảng tóm tắt cấu hình máy tính và gợi ý chế độ làm việc tối ưu từ TASK-M4-27.
  - Cung cấp form nhập BYOK API Keys (Groq Cloud API Key, Google Gemini API Key):
    - Có liên kết 1-click dẫn tới trang lấy Free API Key chính thức của Groq và Google AI Studio.
    - Nút "Kiểm tra kết nối (Test Connection)": Gửi request thử nghiệm siêu nhẹ tới endpoint để xác thực tính hợp lệ của key ngay trên giao diện.
  - Lưu trữ API Key an toàn & Cơ chế Fallback phòng vệ 2 tầng:
    - **Tầng 1 (Primary):** Sử dụng Windows Credential Locker (qua crate `keyring-rs`) để lưu trữ API key an toàn trong hệ điều hành, **tuyệt đối không lưu plain text API key trong file JSON cấu hình**.
    - **Tầng 2 (Headless CI / Non-GUI Fallback Provider):** Khi ứng dụng chạy trong phiên dịch vụ phi tương tác (Non-interactive Service / Headless Session trên GitHub Actions CI runner, Windows Sandbox) hoặc khi service Windows Credential Manager (`VaultSvc`) bị vô hiệu hóa / trả về lỗi hệ điều hành `ERROR_NO_SUCH_LOGON_SESSION` (0x520) hoặc `PlatformFailure`:
      * Hệ thống tự động chuyển sang cơ chế **In-memory Session Credential Provider** (`Arc<RwLock<HashMap<String, String>>>`) hoặc Mock Keyring Provider.
      * Cho phép lưu và đọc tạm thời API key trong suốt vòng đời phiên làm việc mà không văng lỗi unhandled, đảm bảo 100% các bài test tự động trên CI/CD pipeline và môi trường sandbox chạy thông suốt mà không crash.
- **Giao kèo dữ liệu (Data Contracts):**
  - Đầu vào: Khóa API người dùng nhập vào form.
  - Đầu ra: Cập nhật Windows Credential (hoặc In-memory Session Provider) và lưu cài đặt chung vào `%APPDATA%/NeuroClip/config.json`.
- **Quan hệ phụ thuộc (Dependencies):**
  - Prerequisites: TASK-M4-01, TASK-M4-04, TASK-M4-27.
  - Blocks: TASK-M4-15, TASK-M4-17.
- **Tiêu chí Nghiệm thu Định lượng (AC):**
  - [ ] **AC-28.1:** Kiểm tra kết nối API Key trả về kết quả thành công/thất bại trong $\le 2.0\text{ giây}$.
  - [ ] **AC-28.2:** 100% các API Key được mã hóa an toàn trong Windows Credential Manager, không thể tìm thấy plain text trong thư mục app.
  - [ ] **AC-28.3:** Wizard chỉ hiển thị duy nhất ở lần khởi động đầu tiên (hoặc khi người dùng chủ động mở lại từ Settings).
  - [ ] **AC-28.4:** Hỗ trợ cơ chế Fallback an toàn trên môi trường Headless CI / Sandbox: Khi `keyring-rs` không thể kết nối tới Windows Credential Manager, hệ thống tự động kích hoạt In-memory Session Credential Provider, đảm bảo 100% test suite CI/CD chạy qua mà không crash.
- **Checklist Kiểm thử Chất lượng (@qa-reviewer QA Test Checklist):**
  - *Phân loại kiểm thử:* Security & Frontend Wizard Test
  - *Unit Test Cases:*
    - [ ] **UT-28.1 (First Run Flag):** Khi `is_first_run = true`, modal Onboarding tự động hiển thị; khi `is_first_run = false`, modal ẩn và vào thẳng Dashboard.
    - [ ] **UT-28.2 (Keyring Failure Fallback & Headless CI Mock Provider):** Giả lập `keyring-rs` trả về `Err(PlatformFailure)` hoặc chạy test trong môi trường CI headless (không có GUI logon session); assert hệ thống tự động chuyển sang In-memory Mock Provider, lưu và đọc lại API key an toàn trong suốt vòng đời phiên làm việc.
  - *Integration & Contract Verification (Windows Credential Locker):**
    - [ ] **IT-28.1:** Lưu Groq Key và Gemini Key qua `keyring-rs` vào Windows Credential Locker; đọc lại thành công khi backend cần gọi API.
    - [ ] **IT-28.2 (Test Connection Button):** Click "Kiểm tra kết nối" gửi request siêu nhẹ tới Groq/Gemini, hiển thị tích xanh `[Sẵn sàng]` trong $\le 2.0\text{ giây}$.
  - *Boundary & Security Limits (Zero Plaintext Secrets Invariant):**
    - [ ] **ED-28.1 (Plaintext Audit):** Quét toàn bộ file cấu hình `%APPDATA%/NeuroClip/config.json` và file tạm: khẳng định TUYỆT ĐỐI KHÔNG chứa plain text của bất kỳ API Key nào.
  - *Lệnh thực thi & Tiêu chí xác minh:*
    ```bash
    cargo test --lib core::keyring::tests
    npm run test -- src/components/onboarding/OnboardingWizardModal.test.tsx
    ```
    *Khẳng định: API Keys lưu an toàn trong Windows Credential Manager, hỗ trợ fallback in-memory session trên CI headless, 0 plaintext on disk.*

---

#### TASK-M4-29: Bộ Quản Lý & Tự Động Cập Nhật Công Cụ Phụ Trợ (Managed Sidecars Auto-Updater)
- **Lead Persona:** Senior Backend Dev (Rust)
- **Target Files:**
  - `src-tauri/src/sidecar/updater.rs`
  - `src-tauri/src/sidecar/manifest.rs`
  - `src-tauri/src/commands/updater_cmd.rs`
- **Mô tả & Phạm vi Kỹ thuật:**
  - Quản lý các file thực thi trong `%APPDATA%/NeuroClip/bin/`: `yt-dlp.exe` và `ffmpeg.exe`.
  - Triển khai cơ chế kiểm tra cập nhật `yt-dlp` (`ADR-007`):
    - Gửi request tới GitHub Releases API: `https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest`.
    - Kích hoạt kiểm tra: 1) Mỗi tuần 1 lần khi mở app, hoặc 2) Khi phát hiện lệnh `yt-dlp` trả về lỗi HTTP 403 / Bot Blocked (`ERR_INGEST_BOT_BLOCKED`).
  - Tải file thực thi mới về thư mục tạm, kiểm tra mã băm SHA256 từ release manifest.
  - Thực hiện tráo đổi an toàn (Atomic Swap): Đổi tên file cũ thành `.old`, chuyển file mới vào vị trí, và xóa file `.old` sau khi hoàn tất.
  - **Phòng vệ đa tầng chống YouTube Anti-Bot (`ERR-04`, `ARCHITECTURE.md` §6):**
    - Bên cạnh việc cập nhật binary `yt-dlp.exe` tự động, hệ thống thiết lập cơ chế phối hợp với `TASK-M4-06`: khi phiên bản mới nhất vẫn gặp lỗi HTTP 403 / Bot Challenge (do yêu cầu PO-token hoặc IP challenge từ YouTube), hệ thống tự động kích hoạt tham số nạp Cookie (`--cookies-from-browser` từ Chrome/Firefox/Edge hoặc tệp `cookies.txt`) và xoay vòng User-Agent hợp lệ để bảo đảm 100% vượt qua rào cản bot check.
- **Giao kèo dữ liệu (Data Contracts):**
  - Đầu vào: GitHub Release metadata và checksum SHA256.
  - Đầu ra: Binary `yt-dlp.exe` được cập nhật mới nhất.
- **Quan hệ phụ thuộc (Dependencies):**
  - Prerequisites: TASK-M4-02, TASK-M4-03.
  - Blocks: TASK-M4-06, TASK-M4-08.
- **Tiêu chí Nghiệm thu Định lượng (AC):**
  - [ ] **AC-29.1:** Xác thực mã băm SHA256 thành công trước khi ghi đè file thực thi, ngăn ngừa nguy cơ tải file hỏng hoặc bị can thiệp.
  - [ ] **AC-29.2:** Quá trình cập nhật diễn ra ngầm, không làm treo giao diện người dùng.
  - [ ] **AC-29.3:** Tự động khôi phục (Rollback) về phiên bản cũ nếu việc thay thế file gặp sự cố.
  - [ ] **AC-29.4:** Phối hợp cơ chế tự cập nhật binary với cơ chế nạp Cookie và xoay vòng User-Agent: khi cập nhật binary mới vẫn bị HTTP 403, cung cấp cờ và hướng dẫn nạp cookie (`cookies.txt` / browser cookies) để vượt qua anti-bot trong 100% trường hợp.
- **Checklist Kiểm thử Chất lượng (@qa-reviewer QA Test Checklist):**
  - *Phân loại kiểm thử:* Subprocess Updater & Security Hash Verification Test
  - *Unit Test Cases:*
    - [ ] **UT-29.1 (SHA256 Hash Verification):** Tải file binary mới về thư mục tạm, tính toán băm SHA256; so sánh khớp 100% với hash trong release manifest mới cho phép thay thế.
    - [ ] **UT-29.2 (Tampered Binary Rejection):** Nếu mã băm SHA256 không khớp, lập tức hủy bỏ file tạm, giữ nguyên file thực thi cũ.
  - *Integration & Contract Verification:*
    - [ ] **IT-29.1:** Triển khai cơ chế tráo đổi an toàn (Atomic Swap): Đổi tên binary cũ thành `yt-dlp.exe.old`, ghi file mới vào `yt-dlp.exe`, và xóa file `.old`.
  - *Boundary & Auto-Recovery Limits (ERR-04 Defense):**
    - [ ] **ED-29.1 (Bot Block Trigger & Multi-layer Defense):** Khi lệnh `yt-dlp` trả về mã lỗi HTTP 403 Bot Blocked (`ERR_INGEST_BOT_BLOCKED`), hệ thống tự động kích hoạt tiến trình update ngầm không làm gián đoạn UI; nếu bản mới nhất vẫn bị chặn, kích hoạt luân chuyển User-Agent và hướng dẫn nạp cookie xác thực.
  - *Lệnh thực thi & Tiêu chí xác minh:*
    ```bash
    cargo test --test sidecar_updater_test -- --nocapture
    ```
    *Khẳng định: Xác thực SHA256 chuẩn xác 100%, atomic swap an toàn, phòng vệ đa tầng chống bot HTTP 403.*

---

### Milestone M4.9: Kiểm Thử End-To-End & Đóng Gói Bộ Cài Windows (E2E & Packaging)

#### TASK-M4-30: Bộ Kiểm Thử Tự Động Tích Hợp Toàn Tuyến (E2E Synthetic Pipeline Harness)
- **Lead Persona:** `@qa-reviewer` & `@tech-lead`
- **Target Files:**
  - `tests/e2e/synthetic_vod_pipeline.rs`
  - `tests/fixtures/synthetic_chat.json`
  - `tests/fixtures/synthetic_audio.wav`
  - `src-tauri/src/commands/test_harness_cmd.rs`
- **Mô tả & Phạm vi Kỹ thuật:**
  - Xây dựng kịch bản kiểm thử tự động toàn tuyến (End-to-End Synthetic Pipeline) không phụ thuộc mạng ngoài:
    1. Nạp file audio 3 phút giả lập (có tiếng nói, tiếng cười, và khoảng lặng).
    2. Nạp file chat 10,000 dòng giả lập (có cụm spam KEKW tại $t = 60\text{s}$).
    3. Chạy thuật toán phát hiện Highlight: Kiểm tra cụm highlight tìm được có bao hàm mốc $t = 60\text{s}$ kèm bù trễ 4.5s.
    4. Cắt phân đoạn âm thanh $\rightarrow$ Tiền xử lý loudnorm $\rightarrow$ Silero VAD.
    5. Mock STT & Translation $\rightarrow$ Sinh phụ đề Vietsub.
    6. Xuất bản CapCut Draft JSON $\rightarrow$ Kiểm tra file `draft_content.json`, `draft_meta_info.json`, `subtitle.ass`, `subtitle.srt` có tồn tại và đúng định dạng.
- **Giao kèo dữ liệu (Data Contracts):**
  - Đầu vào: Fixture test files.
  - Đầu ra: Báo cáo kết quả kiểm thử E2E Pipeline (Pass/Fail).
- **Quan hệ phụ thuộc (Dependencies):**
  - Prerequisites: Hoàn thành toàn bộ các tasks từ TASK-M4-01 đến TASK-M4-26.
  - Blocks: TASK-M4-32.
- **Tiêu chí Nghiệm thu Định lượng (AC):**
  - [ ] **AC-30.1:** Toàn bộ chu trình E2E chạy tự động hoàn tất trong vòng $\le 45\text{ giây}$.
  - [ ] **AC-30.2:** 100% các assertions về Data Contracts (tất cả 6 schemas) đều vượt qua thành công.
- **Checklist Kiểm thử Chất lượng (@qa-reviewer QA Test Checklist):**
  - *Phân loại kiểm thử:* End-to-End System Integration Test
  - *Kịch bản kiểm thử toàn tuyến (E2E Test Flow):*
    1. Nạp file audio 3 phút giả lập `synthetic_audio.wav` và chat 10,000 dòng `synthetic_chat.json`.
    2. Kích hoạt động cơ Highlight: Assert phát hiện cụm highlight chứa đỉnh chat tại $t = 60.0\text{s}$ kèm bù trễ 4.5s.
    3. Cắt phân đoạn âm thanh lossless range slice $\rightarrow$ Tiền xử lý `loudnorm` $\rightarrow$ Lọc khoảng lặng VAD.
    4. Bóc băng STT (Mock Groq) $\rightarrow$ Dịch thuật Vietsub (Mock Gemini) với từ điển Lore.
    5. Kiểm tra tự động ngắt dòng compound wrapping và tính toán CPS.
    6. Xuất bản CapCut Draft JSON $\rightarrow$ Kiểm tra file `draft_content.json`, `draft_meta_info.json`, `subtitle.ass`, `subtitle.srt` tồn tại và đầy đủ dữ liệu.
  - *Tiêu chí nghiệm thu định lượng (AC):*
    - [ ] **AC-30.1:** Toàn bộ chu trình E2E chạy tự động hoàn tất trong vòng $\le 45\text{ giây}$.
    - [ ] **AC-30.2:** 100% assertions về tính toàn vẹn của 6 Data Contracts (Schemas 5.1 đến 5.6) đều PASS.
  - *Lệnh thực thi & Tiêu chí xác minh:*
    ```bash
    cargo test --test synthetic_vod_pipeline_e2e -- --nocapture
    ```
    *Khẳng định: E2E Pipeline chạy xanh hoàn toàn trong $\le 45\text{s}$.*

---

#### TASK-M4-31: Bộ Đánh Giá Hiệu Năng & Kiểm Thử Tải Trên Máy Cấu Hình Yếu (Low-End PC Stress Benchmark)
- **Lead Persona:** `@qa-reviewer` & Senior Backend Dev (Rust)
- **Target Files:**
  - `tests/stress/low_end_benchmark.rs`
  - `tests/stress/memory_leak_check.rs`
- **Mô tả & Phạm vi Kỹ thuật:**
  - Xây dựng bài kiểm tra tải mô phỏng môi trường máy tính cấu hình tối thiểu (8GB RAM, iGPU, CPU 4 nhân):
    - Bơm 500,000 dòng chat để kiểm tra thuật toán Z-score và Emote filter.
    - Xử lý liên tục 5 clip highlight liên tiếp để kiểm tra hiện tượng rò rỉ bộ nhớ (Memory Leak).
    - Giới hạn VRAM giả lập ở mức 2GB để kiểm tra cơ chế phòng vệ CUDA OOM.
- **Giao kèo dữ liệu (Data Contracts):**
  - Đầu vào: Dữ liệu tải lớn (500k messages, audio stream 8 tiếng).
  - Đầu ra: Báo cáo Memory Profile & CPU Benchmark.
- **Quan hệ phụ thuộc (Dependencies):**
  - Prerequisites: TASK-M4-08, TASK-M4-09, TASK-M4-11, TASK-M4-16.
  - Blocks: TASK-M4-32.
- **Tiêu chí Nghiệm thu Định lượng (AC):**
  - [ ] **AC-31.1:** Mức chiếm dụng RAM đỉnh (Peak RAM) trong suốt quá trình xử lý tải nặng không vượt quá $500\text{MB}$.
  - [ ] **AC-31.2:** Sau khi tác vụ hoàn thành, RAM tự động giải phóng về mức tĩnh $\le 100\text{MB}$ (0 byte memory leak).
  - [ ] **AC-31.3:** Không có tình trạng đơ chuột (UI unresponsiveness) kéo dài quá $100\text{ms}$.
- **Checklist Kiểm thử Chất lượng (@qa-reviewer QA Test Checklist):**
  - *Phân loại kiểm thử:* Stress & Memory Profiling Benchmark
  - *Kịch bản kiểm thử tải nặng (Stress Scenario):*
    - Bơm liên tục 500,000 sự kiện chat vào bộ nhớ để tính toán Z-score và Emote density.
    - Xử lý liên tục 5 clip highlight nối tiếp nhau trong một phiên làm việc kéo dài.
    - Giới hạn VRAM khả dụng giả lập ở mức $2\text{GB}$.
  - *Tiêu chí nghiệm thu định lượng (AC):*
    - [ ] **AC-31.1 (Peak RAM Cap):** Mức chiếm dụng RAM đỉnh (Peak RAM) trong suốt quá trình xử lý 500k messages không vượt quá $500\text{MB}$.
    - [ ] **AC-31.2 (Zero Memory Leak):** Sau khi các tác vụ kết thúc 10 giây, bộ nhớ RAM tự động được dọn dẹp giải phóng về mức tĩnh $\le 100\text{MB}$ (0 byte memory leak).
    - [ ] **AC-31.3 (UI Thread Unresponsiveness):** Không có thời điểm nào giao diện chính bị đơ quá $100\text{ms}$.
  - *Lệnh thực thi & Tiêu chí xác minh:*
    ```bash
    cargo test --test low_end_pc_stress_benchmark -- --nocapture --release
    ```
    *Khẳng định: Peak RAM $\le 500\text{MB}$, RAM sau xử lý $\le 100\text{MB}$, 0 memory leak.*

---

#### TASK-M4-32: Thiết Lập Pipeline Đóng Gói Bộ Cài Đặt Windows NSIS & MSI Chính Thức
- **Lead Persona:** `@tech-lead` & Systems Dev
- **Target Files:**
  - `src-tauri/tauri.conf.json`
  - `.github/workflows/release-windows.yml`
  - `installer/nsis/hooks.nsi`
- **Mô tả & Phạm vi Kỹ thuật:**
  - Cấu hình đóng gói bộ cài đặt chính thức cho Windows 10/11 64-bit sử dụng định dạng NSIS (`.exe`) và WiX (`.msi`).
  - Đóng gói sẵn các file thực thi phụ trợ portable: `ffmpeg.exe` và `yt-dlp.exe` vào thư mục tài nguyên của app (`resources/sidecars/`).
  - Thiết lập icon ứng dụng (Neuro-sama icon đầy đủ các kích thước 32x32, 128x128, 256x256, 512x512).
  - Viết kịch bản NSIS hook:
    - Tạo shortcut ngoài Desktop và Start Menu.
    - Đảm bảo trình gỡ cài đặt (Uninstaller) dọn sạch sẽ thư mục cache tạm thời trong `%LOCALAPPDATA%/NeuroClip/cache/`.
- **Giao kèo dữ liệu (Data Contracts):**
  - Đầu vào: Mã nguồn và sidecar binaries.
  - Đầu ra: File cài đặt hoàn chỉnh `NeuroClip_v1.0.0_x64-setup.exe` và `.msi`.
- **Quan hệ phụ thuộc (Dependencies):**
  - Prerequisites: Hoàn thành toàn bộ các tasks từ TASK-M4-01 đến TASK-M4-31.
  - Blocks: Nghiệm thu bàn giao sản phẩm cuối cùng cho PO.
- **Tiêu chí Nghiệm thu Định lượng (AC):**
  - [ ] **AC-32.1:** Kích thước bộ cài đặt NSIS đóng gói hoàn chỉnh $\le 120\text{MB}$ (đã bao gồm ffmpeg và yt-dlp).
  - [ ] **AC-32.2:** Cài đặt và gỡ cài đặt mượt mà trên máy Windows 10/11 sạch không gặp lỗi quyền Administrator.
  - [ ] **AC-32.3:** Khởi chạy app ngay sau khi cài đặt thành công và hiển thị đúng Onboarding Wizard.
- **Checklist Kiểm thử Chất lượng (@qa-reviewer QA Test Checklist):**
  - *Phân loại kiểm thử:* Packaging & Deployment Test
  - *Kịch bản kiểm thử đóng gói:*
    - Đóng gói installer dạng NSIS (`.exe`) và WiX (`.msi`) trên môi trường Windows 10/11 64-bit sạch.
    - Kiểm tra các file thực thi phụ trợ portable (`ffmpeg.exe`, `yt-dlp.exe`) được nhúng sẵn vào thư mục tài nguyên cài đặt.
    - Kiểm tra kịch bản NSIS hook tạo shortcut ngoài Desktop, Start Menu và đăng ký Icon ứng dụng chuẩn xác.
    - Kiểm tra trình gỡ cài đặt (Uninstaller) xóa sạch sẽ thư mục cài đặt và dọn dẹp cache tạm trong `%LOCALAPPDATA%/NeuroClip/cache/`.
  - *Tiêu chí nghiệm thu định lượng (AC):*
    - [ ] **AC-32.1:** Kích thước bộ cài đặt NSIS đóng gói hoàn chỉnh $\le 120\text{MB}$.
    - [ ] **AC-32.2:** Cài đặt và gỡ cài đặt thành công trên máy Windows 10/11 sạch không cần quyền Administrator đặc biệt.
    - [ ] **AC-32.3:** Khởi chạy app ngay sau khi cài đặt hiển thị đúng Onboarding Wizard trong $\le 800\text{ms}$.
  - *Lệnh thực thi & Tiêu chí xác minh:*
    ```bash
    npm run tauri build
    ```
    *Khẳng định: Installer size $\le 120\text{MB}$, cài đặt sạch sẽ, uninstaller dọn cache 100%.*

---

## 4. BIỂU ĐỒ PHỤ THUỘC & ĐƯỜNG GĂNG TIẾN ĐỘ (DEPENDENCY GRAPH & CRITICAL PATH)

### 4.1 Bảng Ma Trận Phụ Thuộc Của Toàn Bộ 32 Tasks

```
[M4.1] TASK-M4-01 (Tauri Shell)
       │
       ├──► TASK-M4-02 (Error Hierarchy)
       │    │
       │    ├──► TASK-M4-03 (Subprocess Supervisor)
       │    │    ├──► TASK-M4-05 (Storage & Pre-flight)
       │    │    │    ├──► TASK-M4-06 (Audio-First Streamer)
       │    │    │    │    ├──► TASK-M4-08 (Chat Downloader)
       │    │    │    │    │    ├──► TASK-M4-09 (Z-Score Engine) ──┐
       │    │    │    │    │    └──► TASK-M4-10 (Emote Swarm)   ───┤
       │    │    │    │    │                                      ▼
       │    │    │    │    └─────────────────────────────► TASK-M4-12 (Multimodal Fusion)
       │    │    │    │                                            │
       │    │    │    │                                            ▼
       │    │    │    └──────────────────────────────► TASK-M4-13 (Range Slicer)
       │    │    │                                                 │
       │    │    │                                                 ▼
       │    │    │                                     TASK-M4-14 (Audio Prep & VAD)
       │    │    │                                                 │
       │    │    │                                                 ▼
       │    │    │                                     TASK-M4-15 (Groq Whisper Client)
       │    │    │                                                 │
       │    │    │                                                 ▼
       │    │    │                                     TASK-M4-17 (Gemini Translation)
       │    │    │                                                 │
       │    │    │                                                 ▼
       │    │    │                                     TASK-M4-18 (Lore Dict & Diarization)
       │    │    │                                                 │
       │    │    │                                                 ▼
       │    │    │                                     TASK-M4-19 (Line-Wrap & CPS)
       │    │    │                                                 │
       │    │    │                                                 ▼
       │    │    │                                     TASK-M4-20 (Studio Core UI)
       │    │    │                                                 │
       │    │    │                                                 ▼
       │    │    │                                     TASK-M4-21 (Virtualized Sub List)
       │    │    │                                                 │
       │    │    │                                                 ▼
       │    │    │                                     TASK-M4-22 (Character Hotkeys 1-5)
       │    │    │                                                 │
       │    │    │                                                 ▼
       │    │    │                                     TASK-M4-24 (ASS / SRT Exporter)
       │    │    │                                                 │
       │    │    │                                                 ▼
       │    │    │                                     TASK-M4-25 (CapCut Draft JSON)
       │    │    │                                                 │
       │    │    │                                                 ▼
       │    │    │                                     TASK-M4-30 (E2E Integration Test)
       │    │    │                                                 │
       │    │    │                                                 ▼
       │    │    │                                     TASK-M4-32 (Windows Packaging)
       │    │    │
       │    │    ├──► TASK-M4-07 (Local Video Importer) ──► TASK-M4-11 (Audio RMS Detector) ──► (Hòa vào M4-12)
       │    │    └──► TASK-M4-29 (Sidecar Auto-Updater) ──► (Bảo vệ M4-06 & M4-08)
       │    │
       │    ├──► TASK-M4-04 (Logging & Secret Redaction)
       │    └──► TASK-M4-27 (Hardware Probe) ─────────► TASK-M4-28 (Onboarding Wizard) ──► (Cung cấp Key cho M4-15 & M4-17)
       │                                             └──► TASK-M4-16 (Faster-Whisper Fallback)
```

### 4.2 Đường Găng Kỹ Thuật (Critical Path Analysis)
Đường găng xuyên suốt của dự án kéo dài qua chuỗi 18 tasks tuần tự:
$$\text{TASK-M4-01} \rightarrow \text{M4-02} \rightarrow \text{M4-03} \rightarrow \text{M4-05} \rightarrow \text{M4-06} \rightarrow \text{M4-08} \rightarrow \text{M4-09} \rightarrow \text{M4-12} \rightarrow \text{M4-13} \rightarrow \text{M4-14} \rightarrow \text{M4-15} \rightarrow \text{M4-17} \rightarrow \text{M4-18} \rightarrow \text{M4-19} \rightarrow \text{M4-20} \rightarrow \text{M4-24} \rightarrow \text{M4-25} \rightarrow \text{M4-30} \rightarrow \text{M4-32}$$

Các nhánh công việc có thể triển khai song song (Parallel Tracks):
- **Nhánh Track A (Phần cứng & Wizard):** TASK-M4-27 $\rightarrow$ TASK-M4-28 (song song với Nhóm Ingestion M4.2).
- **Nhánh Track B (Âm thanh & Local Import):** TASK-M4-07 $\rightarrow$ TASK-M4-11 (song song với Chat Parser M4-08).
- **Nhánh Track C (Local STT Worker):** TASK-M4-16 (song song với Groq Cloud Client M4-15).
- **Nhánh Track D (NLE XML Export):** TASK-M4-26 (song song với CapCut Draft Generator M4-25).

---

## 5. TIÊU CHUẨN CỔNG CHẤT LƯỢNG & ĐIỀU KIỆN PHỦ QUYẾT (QUALITY GATES & VETO CRITERIA)

Để đảm bảo kỷ luật tuyệt đối trong Phase 4, `@qa-reviewer` thiết lập **5 Cổng Chất Lượng Bắt Buộc (Quality Gates)**. Bất kỳ sự vi phạm nào sẽ dẫn đến lệnh **PHỦ QUYẾT (VETO)** ngay lập tức và chặn hợp nhất mã nguồn:

1. **Chính Sách Không Panic Tuyệt Đối (Zero-Unhandled Panic Policy):**
   Cấm triệt để `.unwrap()`, `.expect()`, hoặc `panic!()` trong bất kỳ đoạn mã non-test nào của Rust backend (`clippy::unwrap_used`, `clippy::expect_used` đã deny).
2. **Quy Tắc Kiểm Soát Bộ Nhớ & Giới Hạn VRAM (Memory Leak & VRAM Cliff Rules):**
   RAM tĩnh Idle $\le 90\text{MB}$. RAM đỉnh tải nặng $\le 500\text{MB}$ và giải phóng về $\le 100\text{MB}$ sau khi xong tác vụ. Nếu VRAM $< 2500\text{MB}$, cấm chạy Whisper Large cục bộ.
3. **Tuân Thủ 100% Giao Kèo Dữ Liệu (Data Contracts Adherence 100%):**
   Mọi JSON trao đổi qua IPC hoặc lưu xuống đĩa phải khớp 100% với 6 JSON Schemas quy định tại Section 5 của `docs/ARCHITECTURE.md`.
4. **Chống Rò Rỉ Bí Mật (Zero Secret Leakage Guarantee):**
   Tuyệt đối cấm in bất kỳ API Key nào (Groq, Gemini, OpenAI) hoặc Twitch OAuth token ra console hay file log (`[REDACTED]`).
5. **Cam Kết Phòng Thủ Kép Xuất Bản (Dual Export Safeguard Invariant):**
   Mỗi khi xuất dự án CapCut PC Draft JSON (`draft_content.json`), hệ thống bắt buộc phải tự động ghi song song 2 file phụ đề độc lập `subtitle.ass` và `subtitle.srt` vào cùng thư mục dự án đó (`ADR-005`).

---

## 6. QUY TRÌNH THỰC THI & LỆNH KIỂM THỬ TỰ ĐỘNG TRONG CI/CD

Trước khi bàn giao bất kỳ task nào trong Phase 4 cho `@qa-reviewer`, lập trình viên bắt buộc phải chạy bộ lệnh kiểm tra toàn diện sau trên Windows PowerShell:

```powershell
# 1. Kiểm tra định dạng mã nguồn (Formatting Check)
cargo fmt --all -- --check
npm run format:check

# 2. Kiểm tra tĩnh & Linter nghiêm ngặt (Static Analysis & Clippy)
cargo clippy --all-targets --all-features -- -D warnings -D clippy::unwrap_used -D clippy::expect_used -D clippy::panic
npm run lint

# 3. Kiểm tra kiểu tĩnh TypeScript (Type Check)
npm run typecheck

# 4. Chạy toàn bộ Unit Tests phía Backend Rust
cargo test --lib --all-features

# 5. Chạy toàn bộ Unit & Component Tests phía Frontend
npm run test:run

# 6. Chạy các bài kiểm thử tích hợp & E2E Pipeline giả lập
cargo test --test synthetic_vod_pipeline_e2e -- --nocapture

# 7. Chạy bài kiểm thử tải nặng & Memory Leak
cargo test --test low_end_pc_stress_benchmark -- --nocapture --release
```

**Bảng Cam Kết SLAs Nghiệm Thu Tự Động:**
- Mức chiếm dụng RAM tĩnh: $\le 90\text{MB}$ (SLA $\le 120\text{MB}$).
- Thời gian khởi động ứng dụng: $\le 800\text{ms}$ (SLA $\le 2.0\text{s}$).
- Thời gian quét phần cứng Onboarding: $\le 200\text{ms}$ (SLA $\le 1.0\text{s}$).
- Tốc độ bóc băng Groq STT (Clip 3 phút): $\le 12\text{ giây}$ (SLA $\le 30\text{s}$).
- Tốc độ dịch thuật Gemini Flash (50 câu): $\le 4.0\text{ giây}$ (SLA $\le 10\text{s}$).
- Thời gian sinh CapCut Draft JSON: $\le 200\text{ms}$ (SLA $\le 1.0\text{s}$).

---
*Task board được quản lý và cập nhật liên tục bởi `@product-owner-proxy`, `@tech-lead` và `@qa-reviewer`.*
