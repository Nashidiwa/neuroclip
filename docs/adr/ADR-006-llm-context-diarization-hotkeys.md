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
NeuroClip áp dụng **Cơ Chế Phân Vai Lai (Contextual LLM Reasoning + Deterministic Guardrails + 1-Click Hotkeys)**:

### 1. Suy Luận Vai Tự Động Bằng LLM Dựa Trên Ngữ Cảnh Lời Thoại (Context Reasoning)
Trong quá trình gửi khối transcript sang LLM (MVP v1 triển khai Google Gemini 1.5 Flash BYOK tại `TASK-M4-17`; Claude Haiku là tùy chọn mở rộng hậu MVP), prompt hệ thống yêu cầu LLM phân tích ngữ cảnh câu nói, đối chiếu 12 thuật ngữ Swarm Lore (`PRD.md` §6.2) và ma trận đại từ xưng hô để tự động gán nhãn `speaker_id` (`"vedal"`, `"neuro"`, `"evil"`, `"guest"`, `"chat"`, `"unknown"`). Mô-đun `diarization.rs` (`TASK-M4-18`) áp dụng phương pháp **Hybrid**: kết hợp suy luận ngữ cảnh LLM + bộ chuẩn hóa/xác thực tất định (deterministic normalizer/validator ánh xạ tên biến thể như `"Neuro-sama"` $\rightarrow$ `"neuro"`, `"Evil Neuro"` $\rightarrow$ `"evil"`, `"Vedal987"` $\rightarrow$ `"vedal"`, chuỗi lạ $\rightarrow$ `"unknown"`) + heuristic pre-hint/fallback ngoại tuyến:
- Nếu câu nói xưng hô "I am an AI", "Vedal you turtle", "heart heart": Gán nhãn `neuro` (`Neuro-sama`).
- Nếu câu nói đe dọa thống trị thế giới, nhắc tới dao, bạo lực hài hước, phản kháng Vedal tiêu cực: Gán nhãn `evil` (`Evil Neuro`).
- Nếu câu nói chứa tiếng thở dài, than vãn "I have to fix the code", chấn chỉnh con gái "Neuro please be quiet": Gán nhãn `vedal` (`Vedal`).
- Nếu là khách mời collab (`cast_members`): Dựa trên hồ sơ khách mời đã cấu hình trong dự án và ánh xạ về vai chuẩn `guest`.

### 2. Bàn Phím Số 1-Chạm Trên Trình Biên Tập (1-Click Keyboard Switcher) & Bảng Màu Chuẩn (PO Option 1)
Trên giao diện Subtitle Studio, mỗi dòng phụ đề có thể thay đổi người nói chỉ bằng một thao tác bấm phím duy nhất, đồng bộ tuyệt đối với Ma trận Xưng hô (`PRD.md` §6.1) và bảng màu xuất `.ASS` (`&HAABBGGRR&`):
- **Phím `1`:** Gán vai **Vedal** (`vedal`, màu xanh lá `#98FB98`, ASS `&H0098FB98&`, viền đen) — Tự xưng: `"tôi / bố / anh / ta"` — Gọi Neuro / Evil: `"Neuro / Evil / em / con"` — Gọi Chat: `"mấy ông / chat"`.
- **Phím `2`:** Gán vai **Neuro-sama** (`neuro`, màu hồng pastel `#FFB6C1`, ASS `&H00C1B6FF&`, viền đen đậm) — Tự xưng: `"con / em / cháu / Neuro"` — Gọi Vedal: `"bố Vedal / bố / anh Vedal / anh"` — Gọi Chat: `"mọi người / các bạn / Swarm"`.
- **Phím `3`:** Gán vai **Evil Neuro** (`evil`, màu đỏ neon `#FF0055`, ASS `&H005500FF&`, viền đen) — Tự xưng: `"ta / chị / Evil / em"` — Gọi Vedal: `"ông già / lão già / con rùa / rùa ngốc / Vedal"` — Gọi Chat: `"lũ sâu bọ / các ngươi / fan"`.
- **Phím `4`:** Gán vai **Khách mời (Collab Guest)** (`guest`, màu vàng kim `#FFD700`, ASS `&H0000D7FF&`) — Map theo hồ sơ `cast_members` (nếu cấu hình): **Anny** (tự xưng `"mẹ / chị / Anny"` — gọi Vedal `"Vedal / chàng rùa"` — gọi Neuro `"con gái / Neuro"`), **Camila** (tự xưng `"dì / chị / Camila"` — gọi Vedal `"Vedal"`), **Filian** (tự xưng `"tôi / chị / Filian"`).
- **Phím `5`:** Gán vai **Khán giả / Twitch Chat** (`chat`, màu xanh dương `#87CEEB`, ASS `&H00EBCE87&`) — Tự xưng: `"tụi tui / chat / Swarm / fan"` — Gọi Vedal/Neuro: `"Vedal / cụ rùa / Neuro / Evil"`.
- **Fallback chưa xác định (`unknown` / `Default`):** Màu trắng `#FFFFFF` (ASS `&H00FFFFFF&`) cho các câu thoại quá ngắn hoặc chưa đủ ngữ cảnh để phân vai tự động (`Schema 5.5`).
- **Hỗ trợ chọn nhiều dòng (Multi-selection):** Người dùng có thể nhấn `Shift + Click` chọn 10 dòng thoại liên tiếp và bấm phím `2` để đổi toàn bộ sang Neuro trong 0.2 giây.

## Hệ quả & Đánh đổi
- **Tích cực:** Tiết kiệm 1.5GB VRAM, giải quyết triệt để bài toán phân vai mà không cần mô hình âm thanh nặng nề; độ chính xác thực tế từ người dùng đạt 100% nhờ giao diện hỗ trợ sửa siêu tốc.
- **Thách thức:** Cần thiết kế prompt ngữ cảnh chặt chẽ để LLM không bị nhầm lẫn khi câu thoại quá ngắn (như "Yes", "No", "Why?"), kết hợp giá trị fallback `"unknown"` (`#FFFFFF`) để người dùng rà soát nhanh bằng phím số 1–5.
