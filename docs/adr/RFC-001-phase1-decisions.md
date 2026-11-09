# RFC-001: Tổng Hợp 7 Quyết Định Kiến Trúc & Nghiệp Vụ Cốt Tử Cho Phase 1 (Discovery & Inception)

**Mã tài liệu:** `docs/adr/RFC-001-phase1-decisions.md`  
**Dự án:** NeuroClip — Desktop App Hỗ trợ Clipper & Subber Neuro-sama Vietnam  
**Chủ trì soạn thảo:** `@lead-architect` phối hợp cùng `@product-owner-proxy` & `@qa-reviewer`  
**Ngày đệ trình:** 2026-11-04  
**Trạng thái:** [CHỜ PO PHÊ DUYỆT] (Pending Product Owner Decision)  
**Căn cứ pháp lý:** Hiến pháp `AGENTS.md` — Mục 2 (Zero-Guesswork Policy) & Mục 3 (Quy trình Báo cáo & Đặt câu hỏi cho PO)

---

## LỜI DẪN TỪ @LEAD-ARCHITECT & @PRODUCT-OWNER-PROXY

Kính gửi Product Owner (PO),

Theo quy định nghiêm ngặt tại **Mục 2 của `AGENTS.md` (Quy tắc Ứng xử Kỷ luật - Zero-Guesswork Policy)**:
> *"Khi gặp một ngã rẽ kỹ thuật có nhiều phương án (ví dụ: local STT vs cloud API, Electron vs Tauri, xử lý video bằng RAM vs disk): **CẤM** tự ý chốt. Phải lập ngay 1 file RFC trong `docs/adr/` trình bày: Bối cảnh, Option A, Option B, Đánh đổi (Chi phí / Độ phức tạp / Hiệu năng) và Recommendation của team. Dừng luồng xử lý và ping PO người thật để xin quyết định."*

Qua quá trình điều tra toàn diện về bối cảnh người dùng cộng đồng fan Neuro-sama Việt Nam, giới hạn phần cứng thực tế và các rủi ro nền tảng ngầm, nhóm kiến trúc sư và phân tích nghiệp vụ đã bóc tách được **7 bài toán ngã rẽ sống còn**. Mọi quyết định tại tài liệu này sẽ là tiền đề trực tiếp định hình toàn bộ thiết kế kiến trúc (`docs/ARCHITECTURE.md`) tại Phase 2 và quy chuẩn lập trình (`docs/CONVENTIONS.md`) tại Phase 3.

Dưới đây là 7 câu hỏi quyết định được chuẩn hóa đúng 100% theo mẫu định dạng quy định tại Mục 3 trong `AGENTS.md`.

---

### [CẦN PO QUYẾT ĐỊNH] Vấn đề 1: Lựa chọn Mô hình Tính toán cho Nhận diện Giọng nói (STT) và Dịch thuật (Translation)
* **Bối cảnh:** Đối tượng người dùng mục tiêu (clipper/subber tại Việt Nam) có phần cứng rất phân mảnh; đa số là học sinh/sinh viên dùng laptop tầm trung với GPU 4GB VRAM hoặc card onboard (iGPU). Mô hình cục bộ chất lượng cao (Whisper Large-v3, Llama-3-8B) đòi hỏi tối thiểu 6-12GB VRAM, trong khi các mô hình nhỏ (Whisper Small/Medium) lại nhận diện rất tệ giọng nói nhanh nuốt âm của Vedal và từ lóng của Neuro. Nếu ép chạy cục bộ sẽ loại bỏ 70% người dùng hoặc gây quá nhiệt/đơ máy; nếu phụ thuộc Cloud API hoàn toàn thì phát sinh chi phí hoặc yêu cầu người dùng cấu hình API Key.
* **Các phương án:**
  * **Option 1 (100% Local Inference — Chạy hoàn toàn ngoại tuyến trên máy người dùng):** Sử dụng `whisper.cpp` / `faster-whisper` (INT8) kết hợp mô hình LLM cục bộ nhỏ gọn (Qwen-2.5-3B hoặc Llama-3.2-3B Quantized 4-bit) chạy qua `llama.cpp` hoặc Ollama.
    * *Ưu điểm:* Hoàn toàn miễn phí trọn đời cho người dùng; bảo mật tuyệt đối; không cần tài khoản, thẻ tín dụng hay API key; hoạt động được khi không có kết nối mạng.
    * *Nhược điểm:* Thời gian xử lý lâu trên máy yếu (có thể gấp 2-5 lần thời lượng clip thực tế); chất lượng dịch thuật và nhận diện giọng Vedal ở mức trung bình khá; không thể chạy trên các máy chỉ có GPU onboard đời cũ (dưới 8GB RAM).
    * *Rủi ro:* Tỷ lệ lỗi hỗ trợ người dùng (user support tickets) cao do xung đột driver GPU Windows, lỗi thiếu file DLL C++, và hiện tượng giật lag máy khi đang xử lý.
  * **Option 2 (Cloud API Hỗ trợ — Bring Your Own Key / Freemium Service):** Tích hợp gọi các Cloud API siêu tốc và giá rẻ như Groq API (Whisper-large-v3 chạy tốc độ 200x-300x realtime) và Google Gemini 1.5 Flash / DeepSeek V3 API cho dịch thuật. Cho phép người dùng nhập API Key miễn phí của họ (BYOK - Bring Your Own Key).
    * *Ưu điểm:* Máy siêu yếu (kể cả máy văn phòng không GPU) vẫn xử lý được với tốc độ thần tốc (chỉ mất 10-30 giây cho một đoạn clip dài 3 phút); chất lượng nhận diện và dịch thuật đạt đỉnh cao tuyệt đối (tương đương Whisper large-v3 gốc và LLM cao cấp); dung lượng bộ cài app cực nhẹ (dưới 50MB).
    * *Nhược điểm:* Đòi hỏi máy tính phải có kết nối Internet; người dùng phải tự đăng ký lấy API key (có thể gây bỡ ngỡ ban đầu cho người không rành công nghệ); rủi ro hết hạn mức miễn phí (rate limit).
    * *Rủi ro:* Bị kiểm duyệt an toàn (safety content filter) từ nhà cung cấp API đối với các câu nói nhạy cảm của Neuro.
  * **Option 3 (Hybrid Linh hoạt — Ưu tiên Cloud BYOK, có Fallback Local Nhẹ):** Mặc định hỗ trợ cấu hình Cloud API (Groq + Gemini miễn phí) để đạt chất lượng cao nhất, đồng thời tích hợp sẵn tùy chọn chạy Local với model `faster-whisper-medium` / `small` cho ai hoàn toàn không muốn dùng mạng hoặc không muốn tạo API Key.
    * *Ưu điểm:* Thỏa mãn 100% tệp người dùng (người máy yếu dùng Cloud, người máy mạnh có card RTX dùng Local).
    * *Nhược điểm:* Độ phức tạp của codebase tăng lên do phải duy trì cả 2 pipeline xử lý (Cloud client và Local AI engine runtime).
    * *Rủi ro:* Cần quản lý cấu hình trong UI rõ ràng để tránh người dùng bị nhầm lẫn giữa 2 chế độ.
* **Đề xuất từ @lead-architect:** **Chọn Option 3 (Thiết kế Hybrid, ưu tiên Cloud BYOK với Groq & Gemini Flash)**. Lý do: Groq hiện cung cấp hạn mức miễn phí cực kỳ hào phóng cho Whisper Large-v3 với tốc độ nhanh gấp hàng chục lần máy cá nhân, còn Gemini Flash xử lý dịch thuật tiếng Việt cực tốt và miễn phí. Điều này giúp phần mềm tiếp cận được toàn bộ cộng đồng clipper học sinh/sinh viên ngay cả khi dùng laptop cũ.
* **Hành động tiếp theo:** Đợi PO trả lời [1], [2] hoặc [3] trước khi tiếp tục.

---

### [CẦN PO QUYẾT ĐỊNH] Vấn đề 2: Lựa chọn Framework Xây dựng Desktop App Shell trên Windows
* **Bối cảnh:** Ứng dụng đòi hỏi giao diện hiện đại (dark theme phong cách VTuber, timeline xem video, bảng quản lý highlight, khung sửa phụ đề trực quan) kết hợp với các tác vụ hệ thống nặng: gọi tiến trình con (FFmpeg, yt-dlp), quản lý file kích thước lớn, và tương tác mạng tốc độ cao mà không làm chiếm dụng RAM của phần mềm dựng video (CapCut/Premiere).
* **Các phương án:**
  * **Option 1 (Tauri v2 + Rust Core + React/TypeScript Frontend):** Sử dụng Tauri v2.
    * *Ưu điểm:* File cài đặt siêu nhẹ (dưới 25MB); chiếm cực ít RAM (chỉ 40-80MB RAM so với 500MB+ của Electron); hiệu năng Rust quản lý tiến trình con, file I/O và memory safety đạt mức hoàn hảo; tính bảo mật cao; tích hợp native Windows API mượt mà qua WebView2 có sẵn trên Windows 10/11.
    * *Nhược điểm:* Đòi hỏi lập trình viên phải nắm cấu trúc Rust để viết backend logic an toàn; khó nhúng trực tiếp các thư viện Python chuyên dụng cho AI (phải gọi qua sidecar binaries hoặc local HTTP bridge).
    * *Rủi ro:* Cần quản lý vòng đời tiến trình phụ trợ (sidecar binaries) chặt chẽ để tránh tạo tiến trình rác (zombie processes) trên Windows.
  * **Option 2 (Electron + Node.js + TypeScript):** Sử dụng Electron framework kinh điển.
    * *Ưu điểm:* Hệ sinh thái thư viện khổng lồ; dễ dàng tìm kiếm và phát triển nhanh giao diện kéo thả video phức tạp; tài liệu phong phú; cộng đồng hỗ trợ lớn nhất thế giới desktop app.
    * *Nhược điểm:* Bộ cài đặt nặng (80-150MB); "ngốn" rất nhiều RAM hệ thống (thường xuyên chiếm 400MB - 1GB RAM ngay khi vừa mở); khởi động chậm hơn; kém thân thiện với các máy tính cấu hình thấp.
    * *Rủi ro:* Nguy cơ tràn RAM khi người dùng vừa mở app vừa mở phần mềm dựng phim (Premiere/CapCut).
  * **Option 3 (Thuần Python GUI — PyQt6 / PySide6):** Toàn bộ ứng dụng được viết bằng Python.
    * *Ưu điểm:* Tích hợp trực tiếp 100% với hệ sinh thái AI, Torch, Whisper, NLP mà không cần tầng giao tiếp trung gian (IPC bridge); cực kỳ thuận tiện cho đội ngũ phát triển AI/Data.
    * *Nhược điểm:* Giao diện người dùng (UI/UX) rất khó đạt độ thẩm mỹ cao cấp và mượt mà như Web UI; đóng gói ra file `.exe` bằng PyInstaller rất cồng kềnh (thường nặng từ 1.5GB đến 3GB nếu kèm PyTorch), dễ bị Windows Defender báo nhầm là virus (false positive); hiệu năng render giao diện video timeline rất giật lag.
    * *Rủi ro:* Tỷ lệ người dùng bị Windows SmartScreen chặn cài đặt rất cao.
* **Đề xuất từ @lead-architect:** **Chọn Option 1 (Tauri v2 + Rust Core + React/TypeScript)**. Lý do: Ứng dụng dành cho creator vốn đã tiêu tốn tài nguyên cho phần mềm dựng phim; việc app chạy nền gọn nhẹ, tiết kiệm từng megabyte RAM là điểm cộng sống còn. Rust đảm bảo việc điều khiển FFmpeg và gọi Cloud/Local API diễn ra an toàn và không bao giờ bị rò rỉ bộ nhớ.
* **Hành động tiếp theo:** Đợi PO trả lời [1], [2] hoặc [3] trước khi tiếp tục.

---

### [CẦN PO QUYẾT ĐỊNH] Vấn đề 3: Chiến lược Tải và Xử lý Dữ liệu Video VOD dài (5-8 tiếng)
* **Bối cảnh:** Mỗi buổi livestream của Neuro kéo dài từ 5 đến 8 tiếng với dung lượng file gốc từ 20GB đến 35GB. Người dùng tại Việt Nam thường có dung lượng ổ cứng hạn chế (SSD 256GB/512GB với ổ C: trống dưới 20-30GB) và đường truyền mạng quốc tế không ổn định. Nếu bắt buộc phải tải trọn vẹn 30GB video về máy mới có thể bắt đầu phân tích thì sẽ dẫn đến tràn ổ đĩa và mất 2-4 tiếng chờ đợi vô ích.
* **Các phương án:**
  * **Option 1 (Full VOD Download — Tải toàn bộ video trước khi làm việc):** Ứng dụng tự động tải trọn vẹn file MP4 1080p60 về máy, sau đó mới tiến hành trích xuất âm thanh và phân tích highlight.
    * *Ưu điểm:* Kiến trúc kỹ thuật đơn giản nhất; sau khi tải xong thì mọi thao tác cắt ghép preview đều chạy local offline không phụ thuộc mạng; xem trước mượt mà không lo giật lag.
    * *Nhược điểm:* Tốn từ 30GB đến 40GB dung lượng ổ cứng cho mỗi buổi stream; thời gian chờ tải ban đầu cực kỳ lâu (1-3 tiếng); nguy cơ lỗi đầy ổ đĩa (`ENOSPC`) rất cao.
    * *Rủi ro:* Tỷ lệ người dùng hủy bỏ (drop-off) ứng dụng ngay từ lần đầu sử dụng vì chờ tải quá lâu.
  * **Option 2 (Audio-First Stream Analysis + Precision Range Slicing):**
    1. Ứng dụng chỉ tải duy nhất file âm thanh (Audio stream - AAC ~160kbps, dung lượng chỉ khoảng **350MB - 550MB** cho toàn bộ 8 tiếng stream) bằng cách bóc tách từ manifest HLS/m3u8 của Twitch.
    2. Sử dụng file âm thanh nhỏ này kết hợp với chat replay để chạy nhận diện giọng nói và phát hiện highlight trong 2-3 phút.
    3. Khi phát hiện ra một đoạn highlight dài 1-3 phút (hoặc khi người dùng chọn đoạn mong muốn), ứng dụng mới dùng FFmpeg gửi HTTP Range Request để tải chính xác đoạn video ngắn đó (chỉ khoảng **50MB - 150MB**).
    * *Ưu điểm:* Tiết kiệm **95% dung lượng ổ cứng** (chỉ tốn ~600MB thay vì 35GB); tốc độ phân tích ban đầu nhanh gấp 10-20 lần; người dùng có thể bắt đầu làm việc gần như ngay lập tức.
    * *Nhược điểm:* Kiến trúc streaming phức tạp hơn; cần xử lý việc lệch mốc thời gian (timestamp offset / PTS-DTS drift) khi cắt video HLS theo keyframe (I-frame boundary) bằng FFmpeg.
    * *Rủi ro:* Nếu VOD trên Twitch bị streamer xóa hoặc đổi đường dẫn trong lúc đang làm việc thì không thể tải tiếp các đoạn video sau.
* **Đề xuất từ @lead-architect:** **Chọn Option 2 (Audio-First Stream Analysis + Precision Range Slicing)**. Lý do: Đây là giải pháp duy nhất biến dự án thành một công cụ nhẹ nhàng, khả thi và tiện lợi thực sự. Không một clipper nào muốn biến máy tính của mình thành bãi rác chứa hàng chục GB file VOD thô chỉ để lấy 1 phút video ngắn.
* **Hành động tiếp theo:** Đợi PO trả lời [1] hoặc [2] trước khi tiếp tục.

---

### [CẦN PO QUYẾT ĐỊNH] Vấn đề 4: Cơ chế Tính Điểm Phát hiện Highlight (Highlight Scoring Mechanism)
* **Bối cảnh:** Trong một buổi stream dài 8 tiếng, các khoảnh khắc đáng giá thường chỉ chiếm khoảng 10-20 phút tổng cộng. Cần một cơ chế tự động tìm ra các đoạn này mà không bắt người dùng phải xem lại toàn bộ từ đầu, đồng thời xử lý được cả trường hợp video có chat replay và video ngoại tuyến không có chat.
* **Các phương án:**
  * **Option 1 (Chat Velocity & Sentiment Analysis Đơn thuần):** Thu thập toàn bộ log tin nhắn chat theo thời gian, tính toán tốc độ chat (tin nhắn/giây) và đếm tần suất xuất hiện các emote cảm xúc (`KEKW`, `LUL`, `Aware`, `TUTEL`, `💀`).
    * *Ưu điểm:* Xử lý dữ liệu văn bản siêu nhẹ, siêu nhanh (chỉ mất vài giây); phản ánh chính xác 100% cảm xúc thật của người xem trực tiếp.
    * *Nhược điểm:* Chỉ áp dụng được cho các stream/VOD còn lưu trữ được replay chat; vô dụng hoàn toàn đối với video upload thông thường không có chat hoặc các stream cũ bị mất chat.
    * *Rủi ro:* Thư viện cào chat (Twitch GQL scraper) có thể bị Twitch rate-limit hoặc chặn tạm thời.
  * **Option 2 (Audio Energy & Acoustic Peaks Đơn thuần):** Phân tích dạng sóng âm thanh (RMS Energy, Voice Activity Detection) để tìm các đoạn có tiếng hét lớn của Vedal, âm lượng bất ngờ tăng vọt, hoặc tiếng cười to.
    * *Ưu điểm:* Hoạt động trên mọi file video/audio nội bộ mà không cần phụ thuộc vào bất kỳ nền tảng chat nào; không lo bị chặn API.
    * *Nhược điểm:* Dễ bắt nhầm tiếng nổ lớn trong game, tiếng nhạc nền đột ngột to lên; bỏ qua các khoảnh khắc hài hước tinh tế (deadpan humor) khi Neuro nói một câu cực kỳ "bựa" với tông giọng phẳng lặng không tăng âm lượng.
    * *Rủi ro:* Cần cân chỉnh ngưỡng âm thanh (threshold tuning) rất phức tạp cho từng thể loại game.
  * **Option 3 (Hybrid Multi-Stage — Khuyến nghị):**
    - Bước 1: Dùng Chat Velocity kết hợp bộ lọc Emote độc quyền Neuro để lọc ra top các đoạn có tương tác bùng nổ nhất (kèm căn chỉnh bù trễ $\Delta t \approx 4.5\text{s}$).
    - Bước 2: Chạy Speech-to-Text trên các đoạn được đánh dấu.
    - Bước 3: Đưa văn bản lời thoại vào LLM với prompt chuyên biệt chấm điểm mức độ hài hước, kịch tính hoặc độ "unhinged" của Neuro để xếp hạng highlight cho clipper.
    - *Dự phòng ngoại tuyến:* Nếu video không có chat replay, tự động chuyển sang dùng phân tích năng lượng âm thanh (Audio Energy RMS) kết hợp tìm kiếm từ khóa trong transcript.
    * *Ưu điểm:* Tạo ra danh sách highlight chất lượng đỉnh cao nhất, có tóm tắt lý do tại sao đoạn đó lại là highlight (AI Explanation); tối ưu chi phí tính toán.
    * *Nhược điểm:* Logic phối hợp pipeline nhiều tầng (multi-stage pipeline), đòi hỏi xử lý đồng bộ mốc thời gian chặt chẽ.
    * *Rủi ro:* Tăng thời gian phân tích thêm 10-15 giây cho bước thẩm định LLM.
* **Đề xuất từ @lead-architect:** **Chọn Option 3 (Hybrid Multi-Stage)**. Lý do: Mang lại giá trị sản phẩm vượt trội cho clipper (không chỉ cắt bừa theo tiếng hét mà thực sự hiểu được mẩu đối thoại buồn cười nhờ LLM), đồng thời thích ứng linh hoạt với mọi nguồn dữ liệu đầu vào.
* **Hành động tiếp theo:** Đợi PO trả lời [1], [2] hoặc [3] trước khi tiếp tục.

---

### [CẦN PO QUYẾT ĐỊNH] Vấn đề 5: Phạm vi Trình Chỉnh Sửa Phụ Đề & Video (Editor Scope)
* **Bối cảnh:** Clipper và Subber chuyên nghiệp tại Việt Nam đều đã có thói quen sử dụng các phần mềm dựng video chuyên dụng: CapCut PC (chiếm đa số vì nhiều mẫu chữ động, hiệu ứng chuyển cảnh cho TikTok), Adobe Premiere Pro, hoặc Aegisub (để căn chỉnh sub dạng karaoke/thẩm mỹ cao). Việc cố gắng tái tạo một trình dựng phim hoàn chỉnh bên trong NeuroClip có thể làm phình to scope dự án và tạo ra trải nghiệm nửa vời.
* **Các phương án:**
  * **Option 1 (Tích hợp Trình dựng Hoàn chỉnh — Full In-App Video NLE Editor):** Xây dựng trình phát video tích hợp đa track (multi-track timeline), cho phép cắt ghép video, chỉnh màu, gắn sticker, tạo hiệu ứng chuyển cảnh và gõ chữ phụ đề trực tiếp bên trong app, sau đó render thẳng ra file video MP4 cuối cùng.
    * *Ưu điểm:* Cung cấp giải pháp trọn gói "All-in-One" cho người dùng mới bắt đầu; không cần cài đặt thêm phần mềm bên ngoài.
    * *Nhược điểm:* Phạm vi công việc khổng lồ (Scope Creep cực lớn); tái tạo lại bánh xe lịch sử của Premiere/CapCut; rất dễ phát sinh lỗi giật lag, lỗi đồng bộ âm thanh-hình ảnh (A/V sync drift), lỗi encode video FFmpeg; làm kéo dài thời gian phát triển Phase 3 & 4 thêm ít nhất 3-6 tháng.
    * *Rủi ro:* Người dùng chuyên nghiệp sẽ không dùng trình dựng thô sơ này mà vẫn quay về Premiere/CapCut.
  * **Option 2 (Lean Subtitle Editor & Multi-NLE Export — Tập trung Tinh gọn vào Lời thoại & Xuất file chuyên dụng):** Ứng dụng chỉ xây dựng một trình xem trước video (preview player) đơn giản với timeline hiển thị các block phụ đề để người dùng nghe lại, sửa nhanh lỗi chính tả tiếng Việt, chỉnh mốc thời gian (sync offset). Sau đó, cung cấp các nút xuất khẩu một chạm:
    - Xuất file phụ đề chuẩn: `.SRT` (cho YouTube), `.ASS` (có sẵn style màu sắc, font chữ meme đẹp mắt cho Aegisub).
    - Xuất dự án dựng phim: File dự án **CapCut PC Draft JSON** (tự động tạo project CapCut có sẵn video đã cắt và phụ đề chữ chạy động), file **Premiere XML / FCPXML**.
    - Tùy chọn "Quick Cut": Cho phép dùng FFmpeg cắt nhanh video kèm hardsub cơ bản nếu người dùng chỉ muốn có ngay 1 clip nhanh để up lên Facebook/TikTok mà không cần chỉnh sửa gì thêm.
    * *Ưu điểm:* Scope cực kỳ gọn gàng, khả thi cao, bàn giao nhanh; tận dụng 100% sức mạnh và thói quen sẵn có của người dùng với CapCut và Premiere; trải nghiệm người dùng mượt mà và chuyên nghiệp.
    * *Nhược điểm:* Người dùng vẫn phải có sẵn CapCut hoặc Premiere nếu muốn thêm hiệu ứng meme/âm thanh phức tạp.
    * *Rủi ro:* Cần duy trì tương thích cấu trúc file `draft_content.json` của CapCut PC khi ByteDance cập nhật phiên bản.
* **Đề xuất từ @lead-architect & @product-owner-proxy:** **Chọn Option 2 (Lean Subtitle Editor & Multi-NLE Export)**. Lý do: Đây là tư duy thiết kế phần mềm hiện đại và thực tế. Clipper Việt Nam 90% dùng CapCut PC vì kho hiệu ứng chữ khổng lồ; việc NeuroClip tự động sinh ra file Draft của CapCut với phụ đề và video được xếp sẵn trên timeline sẽ là "tính năng sát thủ" (killer feature) khiến mọi clipper đều hài lòng.
* **Hành động tiếp theo:** Đợi PO trả lời [1] hoặc [2] trước khi tiếp tục.

---

### [CẦN PO QUYẾT ĐỊNH] Vấn đề 6: Giải pháp Phân biệt & Nhận diện Người Nói (Speaker Diarization)
* **Bối cảnh:** Trong các video clip của Neuro-sama, việc hiển thị màu sắc phụ đề khác nhau cho từng nhân vật (ví dụ: Chữ màu xanh lá cho Vedal, chữ màu hồng cho Neuro, chữ màu đỏ cho Evil) là tiêu chuẩn bắt buộc của cộng đồng để người xem không bị nhầm lẫn. Tuy nhiên, luồng âm thanh chỉ là một bản mix duy nhất; Vedal và Neuro thường xuyên nói chen vào nhau (crosstalk).
* **Các phương án:**
  * **Option 1 (Mô hình Diarization Tự động — PyAnnote / Neural Acoustic Diarization):** Tích hợp thư viện phân tách giọng nói thần kinh (như `pyannote.audio` hoặc SpeechBrain) để tự động nhận dạng sóng âm thanh và gán nhãn từng đoạn giọng là `SPEAKER_00` hay `SPEAKER_01`, sau đó đối chiếu với mẫu giọng đã học để đặt tên Vedal / Neuro.
    * *Ưu điểm:* Hoàn toàn tự động; không cần người dùng can thiệp gán nhãn thủ công ban đầu.
    * *Nhược điểm:* Mô hình diarization cực nặng (tốn thêm từ 1.5GB đến 2.5GB bộ nhớ); cài đặt trên Windows cực kỳ phức tạp (phụ thuộc PyTorch, TorchAudio); xử lý rất kém khi có nhạc nền to hoặc khi hai người nói đè lên nhau; thường xuyên bị nhầm lẫn giữa giọng Neuro và Evil do cùng tần số âm thanh TTS.
    * *Rủi ro:* Tỷ lệ nhận diện sai nhãn lên tới 30-40% trong các tình huống tranh cãi, khiến người dùng mất thêm công sức đi sửa lại từng câu.
  * **Option 2 (LLM Context Reasoning kết hợp Phím tắt 1-chạm — Heuristic + Manual Nudge):**
    - Sử dụng chính mô hình LLM dịch thuật để suy luận người nói dựa trên ngữ cảnh câu thoại (ví dụ: câu nói xấc xược, máy móc thường là Neuro; câu thở dài, càu nhàu mắng mỏ thường là Vedal).
    - Trên giao diện chỉnh sửa phụ đề (Editor), cung cấp phím tắt 1 chạm (Phím số 1: Vedal [Xanh lá], Phím số 2: Neuro [Hồng], Phím số 3: Evil [Đỏ]) để người dùng có thể đổi màu/đổi nhân vật cho hàng loạt dòng phụ đề trong vòng 5 giây.
    * *Ưu điểm:* Cực kỳ nhẹ nhàng, không tốn thêm tài nguyên máy; logic suy luận ngữ cảnh của LLM thường chính xác hơn nhận diện sóng âm khi có tiếng nhạc ồn ào; giao diện trực quan giúp người dùng kiểm soát 100% kết quả.
    * *Nhược điểm:* Vẫn cần người dùng lướt qua mắt để xác nhận lại người nói cho các câu thoại ngắn cụt lủn (như *"Yes"*, *"No"*).
    * *Rủi ro:* Người dùng lười có thể quên không kiểm tra lại các câu thoại ngắn.
* **Đề xuất từ @lead-architect:** **Chọn Option 2 (LLM Context Reasoning kết hợp Phím tắt 1-chạm)**. Lý do: Vừa đảm bảo hiệu năng nhẹ nhàng của ứng dụng trên máy yếu, vừa tránh được các lỗi ảo giác ngớ ngẩn của các thư viện diarization nặng nề.
* **Hành động tiếp theo:** Đợi PO trả lời [1] hoặc [2] trước khi tiếp tục.

---

### [CẦN PO QUYẾT ĐỊNH] Vấn đề 7: Cơ chế Quản lý & Cập nhật Công cụ Phụ trợ Bên ngoài (Sidecars: FFmpeg & yt-dlp)
* **Bối cảnh:** Hệ thống phụ thuộc 100% vào `yt-dlp` để cào dữ liệu/tải video và `ffmpeg` để cắt ghép/chuyển mã âm thanh. `yt-dlp` bị YouTube/Twitch chặn trung bình 2-3 tuần một lần mỗi khi các nền tảng này cập nhật cơ chế chống bot. Nếu không có cơ chế quản lý vòng đời cho file nhị phân này, ứng dụng sẽ trở thành "cục gạch" sau vài tuần phát hành.
* **Các phương án:**
  * **Option 1 (Yêu cầu Người dùng tự cài đặt — External Dependency Mode):** Ứng dụng không đi kèm FFmpeg hay yt-dlp. Khi cài đặt, ứng dụng yêu cầu người dùng phải tự cài qua lệnh `winget install yt-dlp ffmpeg` hoặc tự tải file `.exe` về và trỏ đường dẫn trong phần Cài đặt (Settings).
    * *Ưu điểm:* Kích thước bộ cài ứng dụng siêu nhỏ gọn (dưới 10MB); lập trình viên không phải chịu trách nhiệm về bản quyền phần mềm bên thứ ba hay việc cập nhật binary.
    * *Nhược điểm:* Trải nghiệm người dùng (UX) cực kỳ tệ; 80% người dùng phổ thông không biết biến môi trường `PATH` là gì và sẽ từ bỏ phần mềm ngay lập tức.
    * *Rủi ro:* Đánh mất toàn bộ tệp người dùng không am hiểu kỹ thuật.
  * **Option 2 (Managed Self-Updating Sidecars — Tự động Quản lý và Cập nhật Binary trong nền):**
    - Ứng dụng đi kèm sẵn phiên bản ổn định của `ffmpeg.exe` và `yt-dlp.exe` trong thư mục nội bộ `%LOCALAPPDATA%/NeuroClip/bin/`.
    - Mỗi khi khởi động ứng dụng (hoặc qua nút bấm "Kiểm tra cập nhật công cụ" trong phần Cài đặt), hệ thống sẽ ngầm kiểm tra GitHub Release của `yt-dlp` và tự động tải phiên bản mới nhất nếu có bản cập nhật mà không bắt người dùng cài đặt lại toàn bộ ứng dụng.
    * *Ưu điểm:* Trải nghiệm "One-Click Install" tuyệt đối; người dùng chỉ cần tải file `.msi` về cài là dùng được ngay; ứng dụng luôn luôn sống sót trước các đợt cập nhật chặn bot của YouTube/Twitch.
    * *Nhược điểm:* Cần viết thêm module quản lý tiến trình tải, giải nén và kiểm tra mã checksum (SHA256) an toàn của binary bên thứ ba.
    * *Rủi ro:* Rủi ro mạng chậm khi tải binary cập nhật trong lần đầu chạy.
* **Đề xuất từ @lead-architect & @qa-reviewer:** **Chọn Option 2 (Managed Self-Updating Sidecars)**. Lý do: Đây là tiêu chuẩn bắt buộc của một sản phẩm thương mại/cộng đồng hoàn chỉnh để đảm bảo tính sẵn sàng hoạt động (high availability) và sự hài lòng của người dùng.
* **Hành động tiếp theo:** Đợi PO trả lời [1] hoặc [2] trước khi tiếp tục.

---

## TỔNG HỢP MA TRẬN ĐỀ XUẤT CỦA TEAM KỸ THUẬT

| Vấn đề RFC | Tên Vấn Đề Quyết Định | Lựa chọn Đề xuất | Lý do Cốt lõi |
|:---:|:---|:---:|:---|
| **Vấn đề 1** | Mô hình tính toán AI (STT & Translation) | **Option 3 (Hybrid Linh hoạt - Cloud BYOK + Local Fallback)** | Cho phép máy yếu/laptop văn phòng chạy siêu tốc với Groq/Gemini, máy mạnh có thể chạy offline. |
| **Vấn đề 2** | Desktop App Shell Framework | **Option 1 (Tauri v2 + Rust Core)** | Tiết kiệm RAM tối đa (chỉ 50-80MB), nhường tài nguyên cho CapCut/Premiere, bundle siêu nhỏ. |
| **Vấn đề 3** | Chiến lược tải & xử lý Video VOD | **Option 2 (Audio-First + Precision Range Slicing)** | Tiết kiệm 95% dung lượng ổ cứng (chỉ tải 500MB thay vì 35GB), bắt đầu làm việc trong 2 phút. |
| **Vấn đề 4** | Cơ chế tính điểm phát hiện Highlight | **Option 3 (Hybrid Multi-Stage: Chat Z-Score + LLM Transcript Filter)** | Đạt độ chính xác cao nhất (hiểu được ngữ nghĩa hài hước), có fallback cho video không có chat. |
| **Vấn đề 5** | Phạm vi Trình chỉnh sửa Phụ đề | **Option 2 (Lean Subtitle Editor & Multi-NLE Export)** | Tinh gọn, bàn giao nhanh, kết nối trực tiếp vào thói quen làm việc thực tế với CapCut PC và Premiere. |
| **Vấn đề 6** | Nhận diện & Phân vai người nói | **Option 2 (LLM Context Reasoning + Phím tắt 1-chạm)** | Tránh gánh nặng tài nguyên của mô hình diarization nặng nề, người dùng kiểm soát 100% bằng phím số. |
| **Vấn đề 7** | Quản lý công cụ phụ trợ (Sidecars) | **Option 2 (Managed Self-Updating Sidecars)** | One-click install cho người dùng phổ thông, tự động sống sót trước các đợt chặn bot của YouTube/Twitch. |

---
*Tài liệu RFC-001 đã hoàn tất biên soạn. Đề nghị Product Owner phản hồi theo cú pháp `[Vấn đề X: Option Y]` để đội ngũ `@lead-architect` và `@tech-lead` tiến hành chốt tài liệu Kiến trúc `docs/ARCHITECTURE.md` và bước sang Phase 2.*
