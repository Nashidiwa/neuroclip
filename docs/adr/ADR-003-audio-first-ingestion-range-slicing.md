# ADR-003: Chiến Lược Thu Nạp Dữ Liệu Audio-First & Tải Phân Đoạn Chính Xác Cho VOD Trực Tuyến

## Trạng thái
**ĐÃ ĐƯỢC CHẤP THUẬN (ACCEPTED)** — Phê duyệt bởi Product Owner ngày 2026-11-05 (Căn cứ phản hồi RFC-001, Câu hỏi 3: Lựa chọn [2] kèm làm rõ về luồng Online URL vs Local Offline File).

## Bối cảnh
Mỗi buổi livestream của Jack Vedal và Neuro-sama thường kéo dài từ 4 đến 8 tiếng. File video 1080p60 thô có dung lượng từ **20GB đến 35GB**.
- Nếu chọn giải pháp **Tải Toàn Bộ Video (Full VOD Download)**:
  - Người dùng phải chờ tải từ 1 đến 2 tiếng tùy tốc độ mạng trước khi bắt đầu xem clip.
  - Nguy cơ tràn ổ đĩa hệ thống (`C:`), đặc biệt trên các laptop có dung lượng ổ cứng hạn hẹp (256GB–512GB).
  - Tốn kém băng thông mạng vô ích khi clipper cuối cùng chỉ cần trích xuất 1–2 phút highlight ngắn.

## Quyết định Kiến trúc
NeuroClip thiết lập **Quy Trình Phân Tuyến Kép Rành Mạch Giữa 2 Nguồn Dữ Liệu Đầu Vào**:

### 1. Phân Tuyến A: Người dùng cung cấp Liên kết Trực tuyến (Twitch / YouTube VOD URL)
Áp dụng chiến lược **Audio-First Stream Analysis & Precision Range Slicing**:
- **Giai đoạn 1 (Thu nạp Siêu nhẹ):** Ứng dụng gọi `yt-dlp` chỉ trích xuất luồng âm thanh trực tuyến định dạng AAC bitrate thấp (~400MB cho 8 tiếng VOD) và tải tệp lịch sử chat replay JSON qua GraphQL API (~10MB–20MB). Không tải bất kỳ luồng video 1080p nào về máy.
- **Giai đoạn 2 (Phân tích & Lựa chọn):** Quét highlight trên luồng audio + chat events. Người dùng xem danh sách khoảnh khắc nổi bật và chọn phân đoạn ưng ý (ví dụ: $t_1 = 01:24:15$ đến $t_2 = 01:25:45$, thời lượng 90 giây).
- **Giai đoạn 3 (Tải Phân đoạn Chính xác - Precision Range Slicing):**
  - Sử dụng FFmpeg truyền tham số `-ss` trước `-i` cùng URL m3u8 stream gốc để gửi HTTP Range Requests:
    ```bash
    ffmpeg -ss 01:24:15 -to 01:25:45 -i "<stream_url>" -c copy -avoid_negative_ts make_zero "clip_slice.mp4"
    ```
  - Quá trình tải và cắt đoạn video 90 giây diễn ra trong vòng 5–10 giây, tiêu tốn chỉ ~50MB băng thông mạng và ổ cứng thay vì 30GB.

### 2. Phân Tuyến B: Người dùng nạp Tệp Video Cục Bộ Ngoại Tuyến (Offline Local File từ OBS / Ổ cứng)
- Khi người dùng kéo thả file video sẵn có (`.mp4`, `.mkv`, `.mov`, `.ts` từ máy quay/OBS), ứng dụng **không thực hiện bất kỳ lệnh tải mạng nào**.
- Rust backend sử dụng FFmpeg trích xuất luồng âm thanh tạm thời (WAV PCM 16kHz mono) vào thư mục cache để phục vụ thuật toán phát hiện Audio Energy Peaks và chạy VAD/STT.
- Giao diện xem trước (Video Preview) liên kết trực tiếp tới file video gốc trên ổ đĩa thông qua custom protocol `asset://` của Tauri v2.

## Hệ quả & Đánh đổi
- **Tích cực:**
  - Tiết kiệm **hơn 95% băng thông mạng** và **dung lượng ổ cứng** cho người dùng khi làm việc với VOD trực tuyến.
  - Giảm thời gian chờ đợi ban đầu từ 90 phút xuống chỉ còn **1–2 phút** để bắt đầu chọn highlight.
  - Xử lý hoàn hảo cả 2 tệp người dùng: làm clip trực tiếp từ stream online hoặc xử lý file ghi hình offline.
- **Thách thức:**
  - Cần bảo đảm `yt-dlp` luôn lấy được URL stream hợp lệ và xử lý cơ chế làm mới token/URL khi stream URL hết hạn sau thời gian dài.
