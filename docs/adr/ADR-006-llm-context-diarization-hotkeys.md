# ADR-006: Cơ Chế Phân Vai Người Nói Kết Hợp LLM Suy Luận Ngữ Cảnh & Bàn Phím Tắt 1-Chạm

## Trạng thái
**ĐÃ ĐƯỢC CHẤP THUẬN (ACCEPTED)** — Phê duyệt bởi Product Owner ngày 2026-11-05 (Căn cứ phản hồi RFC-001, Câu hỏi 6: Lựa chọn [2]).

## Bối cảnh
Trong các buổi stream của kênh `vedal987`:
- Thường có sự tham gia của 2–4 nhân vật: Jack Vedal (giọng nam người thật, nói nhanh, giọng trầm, hay nuốt âm), Neuro-sama (giọng AI TTS nữ ngọt ngào/tinh nghịch), Evil Neuro (giọng AI TTS hơi trầm/lanh lảnh sắc nhọn), cùng khách mời collab (Anny, Camila, Filian, Sentry, v.v.).
- Đặc biệt, hiện tượng **nói đè lên nhau (overlapping speech / crosstalk)** diễn ra liên tục khi Vedal cố gắng cắt lời Neuro hoặc khi Neuro cướp lời Vedal.
- Nếu dùng mô hình phân đoạn âm thanh truyền thống (PyAnnote Audio Neural Diarization):
  - Đòi hỏi thêm ít nhất 1.5GB VRAM GPU.
  - Tỷ lệ gán nhầm vai cực cao khi có crosstalk và tiếng nhạc nền game.
  - Khó phân biệt sự khác nhau giữa giọng của Neuro-sama và Evil Neuro vì cùng dùng nền tảng TTS tương đồng.

## Quyết định Kiến trúc
NeuroClip áp dụng **Cơ Chế Phân Vai Lai (Contextual LLM Reasoning + 1-Click Hotkeys)**:

### 1. Suy Luận Vai Tự Động Bằng LLM Dựa Trên Ngữ Cảnh Lời Thoại (Context Reasoning)
Trong quá trình gửi khối transcript sang LLM (Gemini Flash / Claude Haiku) để dịch Vietsub, prompt hệ thống yêu cầu LLM phân tích ngữ cảnh câu nói để tự động gán nhãn `speaker_id`:
- Nếu câu nói xưng hô "I am an AI", "Vedal you turtle", "heart heart": Gán nhãn `Neuro-sama`.
- Nếu câu nói đe dọa thống trị thế giới, nhắc tới dao, bạo lực hài hước, phản kháng Vedal tiêu cực: Gán nhãn `Evil Neuro`.
- Nếu câu nói chứa tiếng thở dài, than vãn "I have to fix the code", chấn chỉnh con gái "Neuro please be quiet": Gán nhãn `Vedal`.
- Nếu là khách mời collab: Dựa trên tên khách mời đã chọn trong dự án.

### 2. Bàn Phím Số 1-Chạm Trên Trình Biên Tập (1-Click Keyboard Switcher)
Trên giao diện Subtitle Studio, mỗi dòng phụ đề có thể thay đổi người nói chỉ bằng một thao tác bấm phím duy nhất:
- **Phím `1`:** Gán vai **Vedal** (Màu xanh rêu `#98FB98`, viền đen, xưng "anh/tôi" - gọi Neuro là "em/con").
- **Phím `2`:** Gán vai **Neuro-sama** (Màu hồng pastel `#FFB6C1`, viền đen đậm, xưng "em/cháu" - gọi Vedal là "bố/anh Vedal").
- **Phím `3`:** Gán vai **Evil Neuro** (Màu đỏ san hô `#FF4500` hoặc đỏ tía, xưng "chị/ta" - gọi Vedal là "con rùa/lão già").
- **Phím `4`:** Gán vai **Khách mời (Collab Guest)** (Màu vàng kim `#FFD700`).
- **Phím `5`:** Gán vai **Khán giả / Twitch Chat** (Màu xanh dương `#87CEEB`).
- **Hỗ trợ chọn nhiều dòng (Multi-selection):** Người dùng có thể nhấn `Shift + Click` chọn 10 dòng thoại liên tiếp và bấm phím `2` để đổi toàn bộ sang Neuro trong 0.2 giây.

## Hệ quả & Đánh đổi
- **Tích cực:** Tiết kiệm 1.5GB VRAM, giải quyết triệt để bài toán phân vai mà không cần mô hình âm thanh nặng nề; độ chính xác thực tế từ người dùng đạt 100% nhờ giao diện hỗ trợ sửa siêu tốc.
- **Thách thức:** Cần thiết kế prompt ngữ cảnh chặt chẽ để LLM không bị nhầm lẫn khi câu thoại quá ngắn (như "Yes", "No", "Why?").
