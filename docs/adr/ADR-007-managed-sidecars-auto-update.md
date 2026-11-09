# ADR-007: Quản Lý & Tự Động Cập Nhật Các Công Cụ Phụ Trợ (Managed Self-Updating Sidecars: FFmpeg & yt-dlp)

## Trạng thái
**ĐÃ ĐƯỢC CHẤP THUẬN (ACCEPTED)** — Phê duyệt bởi Product Owner ngày 2026-11-05 (Căn cứ phản hồi RFC-001, Câu hỏi 7: Lựa chọn [2]).

## Bối cảnh
NeuroClip phụ thuộc vào 2 công cụ dòng lệnh (CLI tools) cốt lõi bên ngoài:
1. **FFmpeg (`ffmpeg.exe`):** Dùng để trích xuất âm thanh, đo đạc năng lượng âm thanh RMS, tải phân đoạn stream HTTP range slicing, và render video/hardsub.
2. **yt-dlp (`yt-dlp.exe`):** Dùng để trích xuất metadata video, link m3u8 stream trực tiếp từ Twitch/YouTube, và tải Twitch Chat Replay JSON.
- **Vấn đề Nghiệp vụ:** Cả YouTube và Twitch thường xuyên thay đổi thuật toán, token xác thực bot và API bảo vệ. `yt-dlp` phải cập nhật liên tục (trung bình 1–2 tuần/lần). Nếu bắt người dùng tự cài `yt-dlp` và `ffmpeg` thủ công:
  - 80% người dùng không rành kỹ thuật sẽ gặp lỗi biến môi trường `PATH`, không tìm thấy file thực thi.
  - Mỗi khi YouTube/Twitch chặn tải, ứng dụng sẽ bị tê liệt và người dùng không biết cách tự nâng cấp `yt-dlp`.

## Quyết định Kiến trúc
NeuroClip triển khai giải pháp **Managed Self-Updating Sidecars** do Rust backend giám sát trực tiếp:

### 1. Đóng Gói Bộ Công Cụ Cố Định Khi Cài Đặt (Bundled Binaries)
- Bộ cài đặt ứng dụng (MSI/NSIS) đóng gói sẵn bản portable tĩnh của:
  - `ffmpeg.exe` (bản build gpl-shared hoặc essentials tĩnh, kích thước tối ưu).
  - `yt-dlp.exe` (phiên bản ổn định tại thời điểm build).
  - Được đặt trong thư mục nội bộ an toàn của app: `%APPDATA%/NeuroClip/bin/` (hoặc `resources/sidecars/`).

### 2. Cơ Chế Kiểm Tra & Tự Động Cập Nhật Ngầm (Background Self-Updater)
- Khi ứng dụng khởi động (hoặc khi phát hiện lệnh `yt-dlp` trả về mã lỗi 403 Forbidden / Bot Detection từ YouTube/Twitch):
  - Rust backend gửi một truy vấn nhẹ tới GitHub API (`https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest`) để so sánh version tag hiện tại với release mới nhất.
  - Nếu có bản mới: Tải file `yt-dlp.exe` mới về thư mục tạm, kiểm tra checksum SHA256 an toàn, và thay thế file cũ mà không làm gián đoạn người dùng.
  - Hiển thị thông báo nhỏ ở góc dưới giao diện: *"yt-dlp đã được tự động cập nhật lên phiên bản mới nhất để tương thích với Twitch/YouTube"*.

## Hệ quả & Đánh đổi
- **Tích cực:** Trải nghiệm "Out-of-the-box" (cài là chạy), không đòi hỏi cấu hình hệ thống phức tạp; triệt tiêu 90% lỗi do API thay đổi.
- **Thách thức:** Cần quản lý quyền ghi file an toàn trong Windows và xử lý cơ chế retry nếu việc tải bản cập nhật gặp lỗi mạng.
