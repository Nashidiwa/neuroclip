# ADR-005: Phạm Vi Trình Biên Tập Phụ Đề Tinh Gọn (Lean Subtitle Studio) & Tích Hợp Xuất Bản NLE Đa Nền Tảng

## Trạng thái
**ĐÃ ĐƯỢC CHẤP THUẬN (ACCEPTED)** — Phê duyệt bởi Product Owner ngày 2026-11-05 (Căn cứ phản hồi RFC-001, Câu hỏi 5: Lựa chọn [2]).

## Bối cảnh
Người dùng sản xuất nội dung video có 2 trường phái:
1. Clipper làm video ngắn (TikTok, YouTube Shorts, Facebook Reels): Cần xuất nhanh video dọc 9:16 có phụ đề nổi bật, hiệu ứng chữ phóng to/chuyển màu theo nhịp nói (karaoke text).
2. Subber làm video dài (YouTube 16:9): Cần phụ đề chuẩn ngữ pháp, căn chỉnh mốc thời gian (timing) chuẩn xác từng mili-giây, style màu sắc riêng biệt cho từng nhân vật, và đưa vào CapCut PC / Premiere Pro / DaVinci Resolve để thêm hiệu ứng hình ảnh, sound effects, meme b-roll.
- Nếu chọn xây dựng **Một Trình Biên Tập Video Đầy Đủ (Full In-App Video Editor)**: Phạm vi phát triển quá khổng lồ (Multi-track timeline, video compositing canvas, transition shaders, audio mixing), gây phân tán nguồn lực dự án và cạnh tranh không lại các phần mềm chuyên nghiệp như CapCut PC hay Premiere Pro.

## Quyết định Kiến trúc
NeuroClip định vị là **Cầu nối Tối ưu Hóa Quy Trình (Workflow Accelerator)** với giải pháp:

### 1. Trình Biên Tập Phụ Đề Tinh Gọn (Lean Subtitle Studio)
- Tập trung sâu vào trải nghiệm biên tập phụ đề, không làm visual compositor phức tạp:
  - **Màn hình Preview Video:** Hỗ trợ xem trước video mượt mà, đồng bộ dòng thời gian với danh sách phụ đề.
  - **Giới hạn Bố cục 9:16 ở đúng 2 Preset Cố Định:**
    - *Preset 1: Center-Crop 9:16* (Cắt lấy vùng trung tâm màn hình 16:9).
    - *Preset 2: Two-stack Top-Bottom 50/50* (Nửa trên hiển thị khung gameplay/màn hình chia sẻ, nửa dưới hiển thị avatar nhân vật với vạch phân chia cố định).
    - Mọi nhu cầu tùy biến bố cục phức tạp hơn được ủy quyền hoàn toàn cho CapCut PC.
  - **Thanh chỉ số Tốc độ đọc (CPS Indicator):** Tự động tính toán ký tự trên giây (Characters Per Second) theo ngữ điệu tiếng Việt (mục tiêu $< 17\text{ CPS}$, cảnh báo đỏ khi $> 20\text{ CPS}$). Hỗ trợ cơ chế mượn khoảng lặng (Time Borrowing) để tự động nới rộng thời lượng dòng phụ đề sang khoảng lặng kế tiếp.

### 2. Tích Hợp Xuất Bản NLE Đa Nền Tảng & Chiến Lược Phòng Thủ Kép
- **Xuất Bản Trực Tiếp:**
  - File `.ass` (Advanced SubStation Alpha): Chuẩn mực cao nhất cho Aegisub, lưu giữ 100% mã màu, font chữ, kích thước và viền đổ bóng cho từng nhân vật.
  - File `.srt` (SubRip UTF-8): Định dạng phổ thông tương thích với mọi nền tảng video.
  - File **Premiere Pro XML / FCPXML**: Nhập trực tiếp timeline và các đoạn cắt vào Adobe Premiere Pro.
  - Thư mục dự án **CapCut PC Draft JSON**: Sinh ra cấu trúc thư mục `draft_content.json` của CapCut PC, cho phép người dùng mở CapCut lên là có sẵn video đã cắt kèm track phụ đề tương ứng.
- **Chiến Lược Phòng Thủ Kép (Dual Export Strategy):** Do CapCut thường xuyên thay đổi cấu trúc schema nội bộ của file JSON qua các bản cập nhật, khi người dùng chọn xuất CapCut Draft, ứng dụng **luôn tự động xuất kèm file `.ass` và `.srt` độc lập** vào thư mục dự án để làm phương án dự phòng an toàn tuyệt đối.

## Hệ quả & Đánh đổi
- **Tích cực:** Giữ app siêu nhẹ, bàn giao nhanh giá trị cốt lõi (highlight + Vietsub chuẩn lore), tận dụng tối đa thế mạnh dựng phim của CapCut và Premiere.
- **Thách thức:** Cần duy trì tính tương thích của reverse-engineered CapCut Draft JSON schema khi ByteDance phát hành phiên bản CapCut mới.
