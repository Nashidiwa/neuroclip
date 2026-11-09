# ADR-001: Lựa Chọn Kiến Trúc Lai (Hybrid BYOK) Cho Nhận Diện Giọng Nói (STT) và Dịch Thuật Vietsub

## Trạng thái
**ĐÃ ĐƯỢC CHẤP THUẬN (ACCEPTED)** — Phê duyệt bởi Product Owner ngày 2026-11-05 (Căn cứ phản hồi RFC-001, Câu hỏi 1: Lựa chọn [3]).

## Bối cảnh
Người dùng mục tiêu của NeuroClip (các clipper và subber cộng đồng fan Neuro-sama tại Việt Nam) sở hữu cấu hình máy tính rất đa dạng và phân mảnh: từ laptop văn phòng iGPU, laptop gaming sinh viên với card đồ họa tầm trung 4GB–6GB VRAM (GTX 1650, RTX 3050), cho tới PC đồ họa mạnh với card RTX 8GB–16GB VRAM.
- Nếu chọn giải pháp **100% Local Inference**: Các mô hình chất lượng cao (Whisper Large-v3, LLM dịch thuật 8B) đòi hỏi tối thiểu 6GB–12GB VRAM. Khi chạy trên máy tính yếu hoặc chỉ có card onboard, hệ thống sẽ bị tràn bộ nhớ (Out-Of-Memory - OOM), gây đơ máy hoặc thời gian bóc băng chậm gấp 3–5 lần thời gian thực của video.
- Nếu chọn giải pháp **100% Cloud API**: Tốc độ xử lý siêu nhanh (<15 giây cho clip 3 phút) và không chiếm dụng tài nguyên máy, nhưng hoàn toàn phụ thuộc vào kết nối Internet và yêu cầu người dùng phải cấu hình API Key.

## Quyết định Kiến trúc
NeuroClip áp dụng **Kiến trúc Lai (Hybrid Architecture - Bring Your Own Key / BYOK)**:
1. **Chế độ Mặc định (Default - Cloud BYOK):**
   - **STT Engine:** Groq Cloud API với mô hình `whisper-large-v3` (tốc độ xử lý lên tới 200x–300x realtime, hỗ trợ hạn mức Free Tier hào phóng). Tự động phân đoạn âm thanh thành các chunks `< 25MB` để tuân thủ giới hạn payload của Groq.
   - **Translation Engine:** Google Gemini 1.5 Flash API qua Google AI Studio (tốc độ cao, hạn mức miễn phí rộng rãi, khả năng hiểu tiếng lóng và ngữ cảnh tiếng Việt xuất sắc).
2. **Chế độ Ngoại tuyến Dự phòng (Fallback / Offline Local Mode):**
   - Tích hợp sẵn runtime `faster-whisper` với quantization INT8 (`model: medium` hoặc `small`), tiêu thụ `< 2GB VRAM`, cho phép chạy hoàn toàn không cần Internet hoặc khi máy người dùng có card đồ họa rời mạnh.
3. **First-Run Onboarding Setup Wizard & Hardware Probe (Yêu cầu bổ sung từ PO):**
   - Khi người dùng mở app lần đầu tiên, hệ thống kích hoạt Wizard kiểm tra cấu hình phần cứng:
     - Đọc CPU cores, tổng RAM, GPU vendor (NVIDIA / AMD / Intel), VRAM thực tế.
     - Tự động đưa ra gợi ý chế độ tối ưu (khuyên dùng Cloud cho máy yếu/iGPU, Hybrid cho máy trung bình, Local cho máy RTX $\ge 8$GB VRAM).
     - Cung cấp giao diện trực quan hỗ trợ lấy Free API Key 1-click kèm nút "Kiểm tra kết nối (Test Connection)".
     - Người dùng có thể tùy chỉnh lại bất kỳ lúc nào trong màn hình Cài đặt (`Settings`).

## Hệ quả & Đánh đổi
- **Tích cực:**
  - 100% người dùng tiếp cận được phần mềm: từ laptop văn phòng không GPU đến máy trạm cấu hình cao.
  - Tốc độ bóc băng và dịch thuật đạt mức tối đa (dưới 30 giây cho video 3 phút ở chế độ Cloud).
  - Không phát sinh chi phí vận hành máy chủ backend cho dự án (người dùng dùng Free Tier của Groq và Google).
- **Thách thức:**
  - Cần duy trì 2 adapter triển khai cho STT (`GroqWhisperClient` và `LocalFasterWhisperProcess`).
  - Cần cơ chế Theatrical Framing Prompt để vượt qua bộ lọc an toàn của Cloud LLM khi dịch các câu thoại châm biếm ("unhinged") của Neuro-sama.
