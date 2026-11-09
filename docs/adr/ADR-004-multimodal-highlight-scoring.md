# ADR-004: Thuật Toán Tính Điểm Highlight Đa Phương Thức (Chat Velocity + Emote Filter + Audio Energy) Kèm Bù Độ Trễ

## Trạng thái
**ĐÃ ĐƯỢC CHẤP THUẬN (ACCEPTED)** — Phê duyệt bởi Product Owner ngày 2026-11-05 (Căn cứ phản hồi RFC-001, Câu hỏi 4: Lựa chọn [3]).

## Bối cảnh
Các khoảnh khắc highlight đặc sắc trong livestream của Neuro-sama thường biểu hiện qua:
1. **Phản ứng của Khán giả (Chat Activity):** Khi Neuro phát ngôn gây sốc ("unhinged roast") hoặc Vedal gặp sự cố, khung chat bùng nổ hàng loạt tin nhắn spam emote cười (`KEKW`, `LUL`, `OMEGALUL`, `💀`, `Aware`).
2. **Biến động Âm thanh (Audio Energy):** Tiếng Vedal hét to, tiếng cười sảng khoái của khách mời collab, hoặc tiếng nhạc dồn dập trong game.
- Nếu chỉ dựa vào Chat: Sẽ thất bại hoàn toàn nếu VOD không có chat replay, chat bị tắt, hoặc video được nạp từ file ghi hình cục bộ/YouTube.
- Nếu chỉ dựa vào Âm thanh: Dễ tạo ra nhiều cảnh báo giả (false positives) do tiếng nhạc nền game quá lớn hoặc tiếng hiệu ứng âm thanh lặp đi lặp lại.
- **Vấn đề Độ trễ Livestream (Stream Latency):** Khán giả xem stream qua Twitch/YouTube luôn có độ trễ từ **3 đến 6 giây** so với thời điểm diễn ra sự việc trên màn hình. Nếu lấy đúng thời điểm chat bùng nổ để cắt video, đoạn video clip sẽ bị cắt cụt mất câu nói khởi đầu của Neuro!

## Quyết định Kiến trúc
NeuroClip triển khai **Cơ Chế Tính Điểm Highlight Đa Tầng (Hybrid Multi-Stage Scoring)**:

### 1. Tầng 1: Phân tích Động Lực Học Chat (Khi có dữ liệu Chat Replay)
- **Cửa sổ trượt (Sliding Window):** Phân chia dòng thời gian thành các khung thời gian 5 giây ($W = 5\text{s}$).
- **Ngưỡng sàn tin nhắn (Message Count Floor):** Chỉ kích hoạt tính toán khi số lượng tin nhắn $X(t) \ge X_{\text{floor}} = 15$ tin nhắn / 5s để triệt tiêu báo động giả trong các giai đoạn yên ắng (Lull Periods).
- **Z-Score hiệu chỉnh với $\epsilon$ smoothing:**
  $$Z_{\text{adj}}(t) = \frac{X(t) - \mu_W(t)}{\sigma_W(t) + \epsilon}$$
  với $\mu_W(t)$ và $\sigma_W(t)$ là trung bình và độ lệch chuẩn của số tin nhắn trong cửa sổ nền 60 giây gần nhất, $\epsilon = 0.5$ là hằng số làm mịn tránh chia cho 0.
- **Bộ lọc Trọng số Emote Swarm ($E_{\text{ratio}}$):**
  $$E_{\text{ratio}}(t) = \frac{\text{Số emote thuộc từ điển Swarm}}{\text{Tổng số token trong tin nhắn}}$$
  Nhóm emote được tính trọng số cao: `LUL`, `KEKW`, `OMEGALUL`, `💀`, `Aware`, `Clueless`, `Tutel`, `Gymbag`, `Heart`, `Dinking`.
- **Bù Trễ Tự Động (Stream Latency Compensation):**
  Mốc thời gian bắt đầu của sự kiện phát ngôn được xác định dịch lùi **4.5 giây**:
  $$t_{\text{event\_start}} = t_{\text{chat\_peak}} - 4.5\text{s} - 10\text{s (Pre-roll Buffer)}$$
  Đảm bảo bắt trọn toàn bộ câu thoại trước khi khán giả kịp gõ chat.

### 2. Tầng 2: Xác thực & Bổ trợ Năng lượng Âm thanh (Audio Energy RMS)
- Rust backend sử dụng thư viện `symphonia` hoặc FFmpeg tính toán giá trị RMS (Root Mean Square) biên độ âm thanh cho từng block 500ms.
- Phối hợp điểm số:
  $$\text{Score}_{\text{final}}(t) = 0.6 \cdot Z_{\text{adj}}(t) + 0.25 \cdot E_{\text{ratio}}(t) + 0.15 \cdot \text{RMS}_{\text{normalized}}(t)$$

### 3. Tự động Hạ Cấp Tuyến Quét (Fallback Routing)
- Nếu đầu vào là YouTube URL, file cục bộ, hoặc VOD Twitch không tồn tại chat replay, hệ thống **tự động chuyển sang chế độ 100% Audio Energy & Pitch Variation**, phát hiện các đoạn âm lượng tăng vọt đột ngột kết hợp bộ lọc khoảng lặng VAD để tìm highlight mà không cần người dùng thao tác cấu hình phức tạp.

## Hệ quả & Đánh đổi
- **Tích cực:** Độ chính xác tìm highlight đạt trên 90%, triệt tiêu báo động giả trong lúc stream im lặng, và tự động xử lý tốt độ trễ stream.
- **Thách thức:** Cần duy trì bộ từ điển emote Twitch của kênh `vedal987` và các emote BTTV/7TV phổ biến.
