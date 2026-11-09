# ADR-002: Lựa Chọn Tauri v2 (Rust + React/TypeScript) Làm Desktop App Shell Trên Windows

## Trạng thái
**ĐÃ ĐƯỢC CHẤP THUẬN (ACCEPTED)** — Phê duyệt bởi Product Owner ngày 2026-11-05 (Căn cứ phản hồi RFC-001, Câu hỏi 2: Lựa chọn [1]).

## Bối cảnh
Người dùng mục tiêu (clipper/subber) thường xuyên chạy NeuroClip song song với các phần mềm biên tập video chuyên nghiệp rất nặng như CapCut PC, Adobe Premiere Pro, DaVinci Resolve, OBS Studio, Aegisub.
- **Electron:** Đóng gói toàn bộ nhân Chromium và Node.js runtime, thường xuyên tiêu tốn 400MB–800MB RAM khi chạy nền và có bộ cài đặt >120MB. Trên các máy laptop 8GB–16GB RAM, Electron sẽ cạnh tranh tài nguyên với Premiere/CapCut, dẫn tới hiện tượng giật lag, drop frame hoặc crash khi render.
- **PyQt6 / PySide6:** Thuận tiện kết nối script Python, nhưng dung lượng đóng gói cồng kềnh (PyInstaller thường tạo bundle 150MB+), khó xây dựng UI/UX hiện đại (Dark theme phong cách VTuber, timeline mượt mà) và phụ thuộc vào hệ sinh thái Python runtime trên Windows.

## Quyết định Kiến trúc
NeuroClip lựa chọn **Tauri v2** với cấu trúc:
1. **Core Backend (Rust):**
   - Quản lý vòng đời ứng dụng, giao tiếp native Windows API thông qua Microsoft WebView2 (đã tích hợp sẵn trên 99% máy tính Windows 10 và 11).
   - Đóng vai trò **Subprocess Supervisor**: quản lý các tiến trình con sidecars (`ffmpeg.exe`, `yt-dlp.exe`, `TwitchDownloaderCLI.exe`) với cơ chế giám sát I/O không chặn (non-blocking async), bảo đảm giải phóng triệt để tài nguyên khi tắt app.
   - Quản lý tệp cục bộ, trích xuất âm thanh, tính toán thuật toán Chat Velocity Z-score với hiệu năng native cao nhất và an toàn bộ nhớ (Memory Safety).
2. **Frontend UI (React 18 + TypeScript + Tailwind CSS / Vite):**
   - Xây dựng giao diện Dark Mode hiện đại, mượt mà, tối ưu tốc độ render timeline và bảng quản lý highlight.
   - Giao tiếp với Rust core thông qua cơ chế Tauri IPC (Invoke Commands & Event Listeners) với dữ liệu serialize dạng JSON có kiểu tĩnh (TypeScript interfaces).

## Hệ quả & Đánh đổi
- **Tích cực:**
  - **Siêu nhẹ:** Mức chiếm dụng RAM chỉ từ **40MB – 80MB** (nhẹ hơn Electron 8–10 lần), giải phóng hoàn toàn bộ nhớ cho CapCut/Premiere.
  - **Bộ cài đặt nhỏ gọn:** Bộ cài MSI/NSIS chỉ khoảng **15MB – 25MB**, tải và cài đặt trong 10 giây.
  - **Khởi động tức thì:** Thời gian bật app < 1 giây trên ổ cứng SSD.
- **Thách thức:**
  - Đòi hỏi lập trình viên phía backend phải viết bằng ngôn ngữ Rust.
  - Việc tích hợp các module AI Python cục bộ (Faster-Whisper) cần triển khai qua tiến trình sidecar độc lập hoặc compiled binary/C++ DLL (thay vì nhúng trực tiếp in-process).
