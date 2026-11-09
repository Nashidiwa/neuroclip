# PRODUCT REQUIREMENTS DOCUMENT (PRD)
# Dự án: NeuroClip — Desktop App Tự Động Hóa Highlight & Vietsub cho Cộng Đồng Neuro-Sama Vietnam

**Mã tài liệu:** `docs/PRD.md`  
**Phiên bản:** 1.2.0 (Phase 1: Inception & Discovery — PO Approved & Onboarding Wizard Added)  
**Tác giả:** `@product-owner-proxy` & `@lead-architect`  
**Ngày cập nhật:** 2026-11-05  
**Trạng thái:** ĐÃ ĐƯỢC PRODUCT OWNER (PO) PHÊ DUYỆT 7/7 NGÃ RẼ KỸ THUẬT TẠI RFC-001; Bổ sung First-Run Onboarding Wizard & Khảo sát Phần cứng thông minh; Sẵn sàng cho Phase 2 (Architecture Design)  
**Nguyên tắc chỉ đạo:** Single Source of Truth (SSOT) & Zero-Guesswork Policy (`AGENTS.md`)

---

## MỤC LỤC
1. [TỔNG QUAN DỰ ÁN & BÀI TOÁN KINH DOANH](#1-tổng-quan-dự-án--bài-toán-kinh-doanh)
2. [CHÂN DUNG NGƯỜI DÙNG MỤC TIÊU (USER PERSONAS)](#2-chân-dung-người-dùng-mục-tiêu-user-personas)
3. [HÀNH TRÌNH NGƯỜI DÙNG & LUỒNG NGHIỆP VỤ (END-TO-END WORKFLOWS)](#3-hành-trình-người-dùng--luồng-nghiệp-vụ-end-to-end-workflows)
4. [RANH GIỚI HỆ THỐNG: IN-SCOPE VS OUT-OF-SCOPE](#4-ranh-giới-hệ-thống-in-scope-vs-out-of-scope)
5. [DANH MỤC EPICS VÀ USER STORIES (GIVEN-WHEN-THEN)](#5-danh-mục-epics-và-user-stories-given-when-then)
   - [Epic 1: Thu nạp Dữ liệu & Thiết lập Dự án (Ingestion & Project Setup)](#epic-1-thu-nạp-dữ-liệu--thiết-lập-dự-án-ingestion--project-setup)
   - [Epic 2: Phát hiện Highlight Đa Phương Thức (Multimodal Highlight Detection)](#epic-2-phát-hiện-highlight-đa-phương-thức-multimodal-highlight-detection)
   - [Epic 3: Bóc Băng Giọng Nói & Phân Vai Nhân Vật (STT & Diarization)](#epic-3-bóc-băng-giọng-nói--phân-vai-nhân-vật-stt--diarization)
   - [Epic 4: Bản Địa Hóa & Dịch Thuật Vietsub Theo Ngữ Cảnh (Vietsub & Localization)](#epic-4-bản-địa-hóa--dịch-thuật-vietsub-theo-ngữ-cảnh-vietsub--localization)
   - [Epic 5: Trình Biên Tập Phụ Đề Tinh Gọn (Lean Subtitle Studio)](#epic-5-trình-biên-tập-phụ-đề-tinh-gọn-lean-subtitle-studio)
   - [Epic 6: Xuất Bản & Tích Hợp Phần Mềm Dựng (Export & NLE Integration)](#epic-6-xuất-bản--tích-hợp-phần-mềm-dựng-export--nle-integration)
6. [TỪ ĐIỂN MIỀN CỘNG ĐỒNG NEURO-SAMA & MA TRẬN XƯNG HÔ (LORE & PRONOUN MATRIX)](#6-từ-điển-miền-cộng-đồng-neuro-sama--ma-trận-xưng-hô-lore--pronoun-matrix)
   - [6.1 Ma trận Nhân vật, Đại từ xưng hô & Mã màu Subtitle](#61-ma-trận-nhân-vật-đại-từ-xưng-hô--mã-màu-subtitle)
   - [6.2 Từ điển Meme Swarm & Quy tắc Biên dịch Ngữ nghĩa](#62-từ-điển-meme-swarm--quy-tắc-biên-dịch-ngữ-nghĩa)
   - [6.3 Quy tắc Đóng gói Prompt Theatrical Framing & Vượt Rào An Toàn LLM](#63-quy-tắc-đóng-gói-prompt-theatrical-framing--vượt-rào-an-toàn-llm)
7. [YÊU CẦU PHI CHỨC NĂNG & TIÊU CHÍ NGHIỆM THU TỔNG THỂ (NFRS & SLAS)](#7-yêu-cầu-phi-chức-năng--tiêu-chí-nghiệm-thu-tổng-thể-nfrs--slas)

---

## 1. TỔNG QUAN DỰ ÁN & BÀI TOÁN KINH DOANH

### 1.1 Bối cảnh Thực tế (Context)
- **Neuro-sama** là VTuber trí tuệ nhân tạo (AI VTuber) tiên phong trên toàn cầu, được phát triển bởi lập trình viên Jack Vedal (kênh Twitch `vedal987`). Cùng với người em gái AI "Evil Neuro", Neuro-sama đã tạo nên một hiện tượng văn hóa stream độc nhất vô nhị: các cuộc đối thoại không thể đoán trước, những pha "roast" cha đẻ không thương tiếc, và sự tương tác bùng nổ cùng hàng chục nghìn khán giả ("The Swarm").
- Tại Việt Nam, cộng đồng fan ("Neuro Swarm VN") phát triển vô cùng mạnh mẽ trên các nền tảng TikTok, YouTube, Facebook và Discord. Nhu cầu thưởng thức các nội dung highlight ngắn (30s - 90s) và video tổng hợp dài (5 - 15 phút) có phụ đề tiếng Việt (Vietsub) chất lượng cao là cực kỳ lớn.

### 1.2 Nỗi đau của Người Dùng (User Pain Points)
Quy trình sản xuất clip của các clipper và subber cộng đồng hiện nay hoàn toàn thủ công, phân mảnh và ngốn từ 4 đến 10 giờ cho mỗi video ngắn 1-3 phút:
1. **Điểm nghẽn Rà soát VOD (Scanning Bottleneck):** Mỗi buổi livestream kéo dài từ 4 đến 8 tiếng (dung lượng 20GB - 35GB). Clipper phải tua lại thủ công hàng giờ video để săn tìm 1-2 phút Neuro "nói câu chí mạng" (unhinged moment) hoặc Vedal bị sốc.
2. **Điểm nghẽn Nhận diện Âm thanh AI (STT Bottleneck):** Neuro-sama sử dụng giọng đọc TTS tốc độ nhanh, ngắt câu bất quy tắc, nói đè lên tiếng Vedal hoặc khách mời (overlapping speech/crosstalk), kèm nhạc nền game liên tục. Các công cụ nhận diện tự động đại trà (như YouTube Auto-captions) nhận diện sai lệch nghiêm trọng danh từ riêng và từ lóng.
3. **Điểm nghẽn Bản địa hóa Ngữ nghĩa & Meme (Localization Bottleneck):** Hệ sinh thái văn hóa nội bộ ("Swarm Lore") vô cùng phức tạp: *Tutel, Gymbag, Filter is leaking, Abandoned, Heart, Dinking, Aware, Clueless...* Các công cụ dịch máy thông dụng (Google Translate, DeepL) dịch word-by-word ngô nghê, phá hỏng 100% tính hài hước và sắc thái châm biếm của video.
4. **Quy trình Công cụ Rời rạc (Fragmented Toolchain):** Một clipper hiện phải sử dụng 4-6 phần mềm độc lập: `yt-dlp` bằng dòng lệnh -> Trình cắt video -> Whisper ngoài -> Notepad sửa lỗi chính tả -> Google Dịch -> Aegisub căn timeline -> CapCut/Premiere làm hiệu ứng chữ -> Render.

### 1.3 Tầm nhìn & Mục tiêu Sản phẩm (Product Vision & Objectives)
Xây dựng **NeuroClip** — Một **All-in-one Desktop Application trên Windows** chuyên biệt hóa cho cộng đồng sáng tạo nội dung Neuro-sama Việt Nam:
- **Tự động hóa phát hiện Highlight:** Giảm 95% thời gian tìm kiếm khoảnh khắc nhờ phân tích tương quan nhịp độ chat Twitch (Chat Velocity Z-Score), mật độ emote văn hóa và đỉnh năng lượng âm thanh.
- **Bóc băng & Gán vai chuẩn xác:** Tích hợp mô hình Whisper tối ưu kèm VAD lọc tạp âm, gán nhãn nhân vật (Neuro, Evil, Vedal, Khách) trực quan.
- **Vietsub chuẩn văn hóa Streamer:** Ứng dụng LLM theo ngữ cảnh khối thoại kết hợp Từ điển Neuro Lore độc quyền và Ma trận đại từ xưng hô, mang lại bản dịch đậm chất giải trí Việt Nam.
- **Tích hợp sâu với NLE:** Xuất file phụ đề chuyên nghiệp (`.ass`, `.srt`) và xuất thẳng dự án **CapCut PC Draft JSON** / **Premiere XML** một chạm.

---

## 2. CHÂN DUNG NGƯỜI DÙNG MỤC TIÊU (USER PERSONAS)

### Persona 1: "Nam — The Fast Short Clipper" (Sáng tạo Video Ngắn TikTok / YouTube Shorts)
- **Độ tuổi & Nghề nghiệp:** 20 tuổi, sinh viên đại học tại TP.HCM, làm clip bán thời gian vì đam mê VTuber.
- **Thiết bị:** Laptop Gaming tầm trung (Core i5 Gen 11, 8GB/16GB RAM, NVIDIA GTX 1650 4GB VRAM hoặc RTX 3050 Laptop).
- **Mục tiêu:**
  - Đón đầu xu hướng (trend): Đăng tải clip ngắn 30s - 60s lên TikTok / Reels trong vòng 1-2 tiếng ngay sau khi livestream kết thúc.
  - Phụ đề chữ to nổi bật ở giữa màn hình dọc 9:16, hiệu ứng chữ nhảy động từng từ, phong cách dịch hài hước bắt trend Gen Z.
- **Nỗi đau:**
  - Máy tính dung lượng ổ cứng hạn chế (SSD 256GB/512GB), không thể tải cả file VOD 30GB về máy.
  - Tốn hàng giờ tua video và gõ tay từng chữ phụ đề vào CapCut.
- **Hành vi mong muốn với NeuroClip:** Dán link Twitch -> Hệ thống gợi ý top 5 highlight -> Chọn 1 đoạn -> Tự động cắt video, tự tạo Vietsub chuẩn meme -> Xuất thẳng sang CapCut PC Draft với timeline xếp sẵn chữ để thêm sticker và đăng ngay.

### Persona 2: "Linh — The Dedicated Subber" (Dịch thuật Chuyên sâu & Làm Video YouTube Dài)
- **Độ tuổi & Nghề nghiệp:** 24 tuổi, biên dịch viên tự do, IELTS 7.5, quản trị viên cộng đồng fan Neuro-sama Vietnam trên Discord.
- **Thiết bị:** PC để bàn mạnh (Core i7, 32GB RAM, NVIDIA RTX 3060 12GB VRAM, màn hình 2K).
- **Phần mềm sử dụng thường xuyên:** Aegisub, Adobe Premiere Pro, DaVinci Resolve.
- **Mục tiêu:**
  - Sản xuất các video tổng hợp highlight 5 - 15 phút chất lượng cao cho kênh YouTube.
  - Bản dịch phải đạt độ tinh tế tuyệt đối, giữ trọn vẹn phép chơi chữ (puns), dịch mượt mà các tình huống châm biếm đen tối.
  - Phụ đề định dạng `.ass` phân màu chuẩn từng nhân vật (Neuro = Hồng, Evil = Đỏ, Vedal = Xanh lá, Khách = Cam/Vàng).
- **Nỗi đau:**
  - Mất cả buổi tối để chép chính tả tiếng Anh (transcribe) do Vedal nói thì thầm, nuốt âm hoặc Neuro nói đè lên.
  - Căn chỉnh mốc thời gian (timecode sync) từng câu thoại trong Aegisub rất tốn công.
- **Hành vi mong muốn với NeuroClip:** Tải video chất lượng gốc -> Hệ thống bóc băng tiếng Anh chuẩn xác 95% -> Dịch nháp với từ điển meme chuẩn -> Linh mở trình Subtitle Studio trong app để duyệt nhanh, chỉnh sửa mốc thời gian -> Xuất file `.ass` chuẩn màu mở ngay trong Aegisub hoặc Premiere.

### Persona 3: "Huy — The Casual Stream Viewer" (Người Xem Muốn Thưởng Thức Nhanh)
- **Độ tuổi & Nghề nghiệp:** 18 tuổi, học sinh cấp 3, thích Neuro-sama nhưng vốn tiếng Anh hạn chế, không thức khuya xem trực tiếp được do lệch múi giờ (stream Vedal thường diễn ra lúc 1h - 5h sáng giờ VN).
- **Thiết bị:** Laptop văn phòng (Intel Core i3, không có card đồ họa rời).
- **Mục tiêu:** Mở app, dán link VOD đêm qua, bấm nút để xem ngay các đoạn hay nhất kèm phụ đề tiếng Việt mà không cần cài đặt phần mềm dựng phim phức tạp.
- **Hành vi mong muốn:** Giao diện xem video tích hợp phụ đề Vietsub tự động, bấm vào danh sách highlight là nhảy ngay đến đoạn video tương ứng để xem giải trí.

---

## 3. HÀNH TRÌNH NGƯỜI DÙNG & LUỒNG NGHIỆP VỤ (END-TO-END WORKFLOWS)

### 3.1 Luồng 1: Sản xuất Video Ngắn Siêu Tốc (Fast Short-form Pipeline — Persona Nam)
```
[1. Nhập Link Twitch VOD / YouTube URL / Kéo thả Video Cục bộ]
                 ↓
[2. Thu Thập Dữ Liệu Tinh Gọn & Phân Tuyến Tự Động (Smart Ingestion Routing)]
    ├── Nhánh Twitch VOD: Thử tải Chat Replay JSON (~5MB-15MB) & Bóc tách Audio AAC (~400MB)
    │     └── Nếu Chat Replay lỗi (404/rate limit/deleted): Tự động chuyển cấp sang Quét Âm Thanh
    │         kèm Banner: "Chat replay không khả dụng. Đang tự động quét theo Năng lượng Âm thanh..."
    └── Nhánh YouTube / File Cục Bộ: Tự động bỏ qua Chat, bóc tách audio và chuyển thẳng sang Quét Âm Thanh
                 ↓
[3. Quét Highlight Đa Phương Thức]
    ├── Nhánh Chat: Chat Velocity (Z-Score hiệu chỉnh + ε smoothing) kèm Message Floor (X ≥ 15 msgs/5s),
    │   bộ lọc Emote Swarm, căn chỉnh bù độ trễ: t_event = t_chat_peak - 4.5s
    └── Nhánh Âm Thanh (Fallback / Direct): Quét đỉnh Năng lượng Âm thanh (Audio Energy RMS Peaks)
                 ↓
[4. Trình Bày Danh Sách Đề Xuất]
    └── Người dùng xem danh sách top highlight (Score, Duration, Lý do nổi bật)
                 ↓
[5. Chọn Highlight & Slicing Chính Xác]
    └── FFmpeg gửi HTTP Range Request tải duy nhất đoạn video ngắn 30s-60s đã chọn
                 ↓
[6. STT & Vietsub Siêu Tốc]
    ├── Bóc băng: Groq Whisper (cắt chunk < 25MB) / Faster-Whisper, tiền xử lý loudnorm chống vỡ tiếng,
    │   tùy chọn Karaoke Mode (condition_on_previous_text=False) phá vỡ lặp từ
    └── Dịch Vietsub: LLM (Gemini Flash) kèm Theatrical Framing, safetySettings: BLOCK_NONE,
        cơ chế AI Condensation (t < 1.5s) và Defensive Fallback bảo lưu câu gốc khi gặp bộ lọc an toàn
                 ↓
[7. Xem Trước & Tinh Chỉnh Nhanh]
    └── Chọn Layout Dọc 9:16 theo 2 Preset Cố Định:
        ├── Preset 1: Center-Crop 9:16 (Cắt chính giữa 16:9)
        └── Preset 2: Two-stack Top-Bottom 50/50 (Nửa trên Game, nửa dưới Avatar với vạch chia cố định)
        (Ủy quyền toàn bộ bố cục phức tạp tùy biến sang CapCut PC Draft)
                 ↓
[8. Xuất Bản 1-Chạm]
    ├── Tùy chọn A: Render MP4 Hardsub dùng FFmpeg (sẵn sàng up TikTok ngay)
    └── Tùy chọn B: Tạo dự án CapCut PC Draft kèm Chiến Lược Phòng Thủ Kép (Dual Export Strategy:
        luôn tự động xuất kèm file .ass và .srt độc lập vào thư mục dự án)
```

### 3.2 Luồng 2: Làm Phụ Đề Chuyên Nghiệp Cho Video Dài (Long-form Subbing Pipeline — Persona Linh)
```
[1. Nhập Video Cục Bộ (File MP4/MKV từ OBS) hoặc Link VOD]
                 ↓
[2. Chọn Phân Đoạn Cần Dịch]
    └── Đánh dấu In/Out trên Timeline hoặc chọn cụm highlight liên hoàn
                 ↓
[3. Bóc Băng Nâng Cao & Lọc Tạp Âm]
    ├── Silero VAD cắt bỏ khoảng lặng và nhạc nền tránh lặp từ ảo giác
    └── Whisper Large-v3 trích xuất lời thoại kèm Word-level Timestamps (stable-ts)
                 ↓
[4. Phân Vai & Dịch Thuật Đa Tầng]
    ├── LLM suy luận người nói theo ngữ cảnh (Vedal / Neuro / Evil / Guest)
    └── Dịch thuật theo cửa sổ trượt 30 dòng (Sliding Window Context) giữ trọn vẹn ngữ cảnh
                 ↓
[5. Biên Tập Tại Subtitle Studio]
    ├── Bàn phím số 1-chạm (Phím 1: Vedal, Phím 2: Neuro, Phím 3: Evil) gán màu tức thì
    ├── Nudge mốc thời gian phím tắt (A / D: dịch ±100ms)
    └── Đèn báo vi phạm quy tắc độ dài ký tự (CPS warning indicator)
                 ↓
[6. Xuất Bản Định Dạng Chuyên Nghiệp]
    ├── Xuất file .ASS (đầy đủ Style màu, Font chữ cho Aegisub)
    └── Xuất file Premiere Pro XML / FCPXML đồng bộ timeline
```

### 3.3 Luồng 3: Xử Lý File Video Ngoại Tuyến (Offline Local Ingestion — Persona Huy / All)
- Người dùng kéo thả trực tiếp file `.mp4`/`.mkv` từ máy tính vào cửa sổ ứng dụng.
- Hệ thống trích xuất audio tạm thời -> Chạy nhận diện đỉnh âm thanh (Audio Energy Peaks) -> Chạy STT & Vietsub cục bộ hoặc qua Cloud -> Hiển thị trên trình xem video tích hợp để thưởng thức trực tiếp.

---

## 4. RANH GIỚI HỆ THỐNG: IN-SCOPE VS OUT-OF-SCOPE

Để đảm bảo tính khả thi tuyệt đối cho dự án trong phạm vi MVP và tránh hiện tượng phình to phạm vi (Scope Creep), ranh giới sản phẩm được phân định rành mạch như sau:

| STT | Phân hệ / Tính năng | MVP (In-Scope) | Hậu MVP / v2 (Out-of-Scope) | Rationale & Trade-off |
|:---:|:---|:---:|:---:|:---|
| 1 | **Nền tảng Hệ điều hành** | **Windows 10 & Windows 11 (64-bit)**. Tối ưu cho cấu hình máy tính phổ thông tại Việt Nam. | macOS, Linux, Web SaaS. | 98% clipper/subber trong cộng đồng dùng Windows để dựng video (CapCut PC, Premiere, Aegisub). |
| 2 | **Nguồn Video Đầu vào** | Hỗ trợ URL Twitch VOD, YouTube Video URL, và Kéo thả file video nội bộ (`.mp4`, `.mkv`, `.mov`, `.ts`). Cơ chế Smart Ingestion tự động phân tuyến: YouTube và file cục bộ tự chuyển sang quét âm thanh; Twitch VOD mất chat tự động chuyển cấp không gián đoạn. | Thu phát trực tiếp (Live Stream Recording) thời gian thực đa kênh. | MVP tập trung vào xử lý VOD sau stream để tối ưu độ ổn định và giảm tải tài nguyên mạng. |
| 3 | **Thu Thập & Phân Tích Chat** | Tải và phân tích Twitch Chat Replay JSON qua GQL endpoint / `TwitchDownloaderCLI` (tin nhắn, tần suất, emote spikes). | Phân tích YouTube Live Chat hoặc Kick Chat. | Kênh `vedal987` trên Twitch là nền tảng livestream chính (>95% nội dung và emote độc quyền). |
| 4 | **Thuật toán Highlight** | Đa phương thức: Tương quan Chat Velocity (Z-Score hiệu chỉnh với $\epsilon$ smoothing) kèm ngưỡng sàn tin nhắn ($X \ge 15$ msgs/5s, Emote Swarm $\ge 35\%$) loại bỏ báo động giả trong khoảng lặng + Đỉnh Năng lượng Âm thanh (Audio Energy RMS) + Bù độ trễ 4.5s. | Nhận diện biểu cảm gương mặt nhân vật 2D qua Computer Vision (CV) hoặc phân tích chuyển động video. | Xử lý Computer Vision ngốn GPU khổng lồ, không hiệu quả với avatar VTuber 2D vốn ít biểu cảm vật lý. |
| 5 | **Công nghệ Bóc băng (STT)** | Kiến trúc lai (Hybrid): Ưu tiên Cloud Groq Whisper API (tốc độ 200x, phân đoạn Audio Chunking $< 25$MB tuân thủ payload limit) + Tùy chọn Local Faster-Whisper INT8 (<2GB VRAM). Tích hợp Silero VAD, tiền xử lý FFmpeg `loudnorm` limiter chống vỡ tiếng hét, và chế độ Karaoke / Music Mode (`condition_on_previous_text=False`, temperature fallback) phá vỡ lặp từ. | Tự huấn luyện (train from scratch) một mô hình STT mới. | Huấn luyện mô hình STT tốn hàng chục nghìn USD và dữ liệu mẫu khổng lồ; tận dụng base Whisper kèm domain prompting là tối ưu nhất. |
| 6 | **Dịch thuật Vietsub** | LLM theo ngữ cảnh khối (Sliding Window 20-40 dòng) kết hợp Từ điển Neuro Lore độc quyền và Ma trận xưng hô (Cloud Gemini Flash / Claude Haiku BYOK, tùy chọn Ollama Local). Tích hợp System Prompt Theatrical Framing, `safetySettings: BLOCK_NONE`, cơ chế Segment-level Defensive Fallback bảo lưu tiếng Anh khi gặp kiểm duyệt, và Prompt-Level AI Condensation cho câu thoại $<1.5$s. | Dịch thuật thời gian thực siêu trễ thấp (<1s) phát trực tiếp lên màn hình stream. | Bài toán làm phụ đề clip VOD cần độ chuẩn xác ngữ nghĩa cao, không yêu cầu độ trễ sub-second. |
| 7 | **Trình Biên tập Video** | Trình biên tập phụ đề tinh gọn (Lean Subtitle Studio): Preview video, chỉnh sửa text, chỉnh mốc thời gian, gán màu nhân vật, chia layout 9:16 giới hạn ở đúng 2 Fixed Presets: Preset 1 (Center-Crop 9:16) và Preset 2 (Two-stack Top-Bottom 50/50). Tuyệt đối không xây dựng visual compositor/multi-box canvas tùy biến. | Trình dựng phi tuyến tính đa lớp (Full Multi-track NLE) với hàng trăm hiệu ứng chuyển cảnh, keyframe, chỉnh màu LUT như Premiere. | Giữ app siêu nhẹ. Tôn trọng thói quen người dùng sử dụng CapCut PC và Premiere Pro; ủy quyền các thao tác crop phức tạp cho CapCut. |
| 8 | **Định dạng Xuất bản** | - File phụ đề: `.ass` (chuẩn Aegisub đầy đủ mã màu style nhân vật), `.srt` (UTF-8).<br>- Tích hợp NLE: Thư mục dự án **CapCut PC Draft JSON**, file **Premiere XML**.<br>- Chiến lược Phòng thủ Kép (Dual Export Strategy): Khi xuất CapCut Draft, luôn tự động xuất kèm file `.ass` và `.srt` vào thư mục dự án để dự phòng khi CapCut thay đổi cấu trúc schema.<br>- Xuất video cắt nhanh `.mp4` (FFmpeg lossless copy hoặc hardsub cơ bản). | Tự động đăng tải trực tiếp lên TikTok/YouTube qua API tài khoản người dùng. | Tránh nguy cơ bị khóa tài khoản người dùng do cơ chế chống spam/bản quyền của TikTok/YouTube. |

---

## 5. DANH MỤC EPICS VÀ USER STORIES (GIVEN-WHEN-THEN)

### Epic 1: Thu nạp Dữ liệu & Thiết lập Dự án (Ingestion & Project Setup)

#### US1.0: Trình Hướng Dẫn Khởi Chạy Lần Đầu (First-Run Onboarding Setup Wizard) & Đánh Giá Phần Cứng Thông Minh
- **Là một:** Người dùng mới (clipper/subber) lần đầu tiên cài đặt và mở NeuroClip trên Windows,
- **Tôi muốn:** Được trải nghiệm một màn hình Onboarding Setup Wizard thân thiện, tự động quét kiểm tra cấu hình phần cứng (CPU, tổng RAM, GPU, VRAM) và đưa ra gợi ý chế độ vận hành (Cloud BYOK, Local Offline, hoặc Hybrid) phù hợp nhất với máy tính của mình, kèm giao diện cấu hình API key trực quan,
- **Để tôi:** Nhanh chóng làm quen với app, cấu hình môi trường chuẩn xác ngay từ đầu mà không cần kiến thức kỹ thuật phức tạp hay sợ máy bị đơ lag do chọn sai mô hình AI.
- **Tiêu chí nghiệm thu (Given - When - Then):**
  - **Scenario 1: Kích hoạt Wizard lần đầu và tự động nhận diện cấu hình phần cứng**
    - *Given:* Ứng dụng phát hiện trạng thái chưa từng hoàn thành onboarding (`is_first_launch: true`).
    - *When:* Ứng dụng khởi động lên màn hình chính.
    - *Then:* Cửa sổ "Chào mừng bạn đến với NeuroClip — Thiết lập nhanh trong 1 phút" tự động xuất hiện; module phần cứng ngầm (Hardware Probe) lập tức kiểm tra thông số máy tính (CPU cores, RAM khả dụng, tên GPU, dung lượng VRAM thực tế) trong vòng dưới 500ms.
  - **Scenario 2: Đề xuất phương án tối ưu dựa trên năng lực phần cứng**
    - *Given:* Kết quả kiểm tra phần cứng hoàn tất:
      - *Trường hợp A (Laptop văn phòng / iGPU hoặc RAM ≤ 8GB):* Wizard hiển thị nhãn `[KHUYÊN DÙNG: 100% Cloud API siêu tốc]`, giải thích rõ máy yếu nên dùng Cloud Groq Whisper + Gemini Flash để xử lý trong 10-20 giây và không gây nóng máy.
      - *Trường hợp B (Laptop phổ thông có GPU 4GB - 6GB VRAM như GTX 1650/RTX 3050):* Wizard hiển thị nhãn `[KHUYÊN DÙNG: Hybrid Linh hoạt]`, ưu tiên Cloud STT và hỗ trợ fallback Local Faster-Whisper Medium khi mất mạng.
      - *Trường hợp C (PC Gaming/Workstation có GPU NVIDIA RTX ≥ 8GB VRAM):* Wizard hiển thị nhãn `[KHUYÊN DÙNG: Toàn quyền chọn lựa]`, cho phép người dùng tùy ý chọn Local hoàn toàn (offline 100%) hoặc Hybrid.
    - *When:* Người dùng xem bảng so sánh các chế độ.
    - *Then:* Lựa chọn khuyên dùng được tự động tích sẵn (pre-selected) kèm giải thích trực quan, dễ hiểu.
  - **Scenario 3: Bảng cấu hình API Key trực quan kèm nút kiểm tra kết nối (Test Connection)**
    - *Given:* Người dùng chọn phương án có sử dụng Cloud (Cloud hoặc Hybrid).
    - *When:* Người dùng chuyển sang bước cấu hình API trong Wizard.
    - *Then:* Giao diện cung cấp link 1-click dẫn tới trang lấy API key miễn phí (Groq API Console & Google AI Studio), ô nhập text có ẩn/hiện ký tự mật khẩu, và nút "Kiểm tra kết nối (Test Key)". Nếu key hợp lệ, hiển thị dấu tích xanh `[Sẵn sàng]`; nếu sai key hoặc hết hạn mức, hiển thị thông báo lỗi chi tiết và hướng dẫn khắc phục.
  - **Scenario 4: Lưu trạng thái và cho phép đổi lại trong Cài đặt**
    - *Given:* Người dùng hoàn thành Wizard và nhấn "Bắt đầu Sáng tạo".
    - *When:* Vào màn hình chính của ứng dụng.
    - *Then:* Trạng thái cấu hình được lưu an toàn vào tệp thiết lập người dùng cục bộ; Wizard không hiển thị lại ở các lần mở app sau, nhưng toàn bộ các tùy chọn này có thể xem lại và thay đổi bất kỳ lúc nào tại mục `Cài đặt (Settings) -> Cấu hình AI & Phần cứng`.

#### US1.1: Nhập VOD từ liên kết Twitch với cơ chế Audio-First (Dành cho Online URL)
> *Lưu ý phân biệt luồng:* Cơ chế **Audio-First Stream Analysis & Precision Slicing** áp dụng riêng cho trường hợp người dùng cấp **Đường dẫn trực tuyến (Twitch/YouTube VOD URL)**. Trường hợp người dùng có sẵn **Tệp video nội bộ (Offline Local Video)** được xử lý trực tiếp trên ổ cứng theo US1.2.
- **Là một:** Clipper muốn xử lý một buổi stream 8 tiếng dài 30GB,
- **Tôi muốn:** Dán link Twitch VOD và hệ thống chỉ tải luồng âm thanh nhẹ (~400MB) và chat replay (~10MB),
- **Để tôi:** Bắt đầu phân tích highlight ngay trong 2 phút mà không làm đầy ổ cứng hay chờ tải 40GB video thô.
- **Tiêu chí nghiệm thu (Given - When - Then):**
  - **Scenario 1: Nhập link Twitch VOD thành công với chế độ Audio-First**
    - *Given:* Người dùng dán liên kết `https://www.twitch.tv/videos/2134567890` vào ô nhập liệu và chọn chế độ "Phân tích Nhanh (Audio-First)".
    - *When:* Người dùng nhấn nút "Bắt đầu Phân tích".
    - *Then:* Ứng dụng gọi `yt-dlp` chỉ bóc tách luồng audio AAC và tải chat replay GQL; thanh tiến trình hiển thị dung lượng tải ước tính ~450MB; không tải bất kỳ luồng video 1080p nào về máy.
  - **Scenario 2: Kiểm tra dung lượng ổ đĩa trước khi tải (Disk Space Pre-flight Check)**
    - *Given:* Ổ đĩa `C:` của người dùng chỉ còn trống dưới 5GB.
    - *When:* Người dùng bắt đầu một tác vụ tải hoặc xử lý.
    - *Then:* Ứng dụng lập tức chặn tác vụ, hiển thị hộp thoại cảnh báo: "Dung lượng ổ đĩa C: còn dưới 5GB, nguy cơ lỗi hệ thống. Vui lòng dọn dẹp hoặc chọn thư mục tạm tại ổ đĩa khác (D:/E:)" và không gây tràn ổ cứng.

#### US1.2: Kéo thả file video cục bộ từ OBS
- **Là một:** Subber đã có sẵn file ghi hình buổi stream bằng OBS trên máy tính,
- **Tôi muốn:** Kéo thả trực tiếp file `.mp4` hoặc `.mkv` vào cửa sổ ứng dụng,
- **Để tôi:** Xử lý phụ đề ngay lập tức mà không cần kết nối mạng để tải video.
- **Tiêu chí nghiệm thu:**
  - **Scenario 1: Kéo thả file video nội bộ hợp lệ**
    - *Given:* Người dùng đang ở màn hình khởi tạo dự án.
    - *When:* Người dùng kéo thả 1 file `neuro_stream_rec.mkv` (dung lượng 12GB) vào vùng Dropzone.
    - *Then:* Ứng dụng nhận diện file trong vòng dưới 1 giây, dùng FFmpeg trích xuất luồng âm thanh PCM 16kHz chạy ngầm không làm đơ giao diện.

---

### Epic 2: Phát hiện Highlight Đa Phương Thức (Multimodal Highlight Detection)

#### US2.1: Phát hiện cao trào dựa trên Chat Velocity và Emote Spikes kèm bù trễ
- **Là một:** Clipper đang tìm kiếm các đoạn hài hước nhất trong stream 6 tiếng,
- **Tôi muốn:** Hệ thống tự động tính toán nhịp độ chat (Chat Velocity) theo công thức Z-Score hiệu chỉnh kèm ngưỡng sàn tin nhắn và tự động lùi mốc thời gian 4.5 giây để lấy đúng câu nói khởi nguồn,
- **Để tôi:** Có ngay đoạn video trọn vẹn ngữ cảnh mà không bị cắt cụt đầu câu và không bị dính highlight giả trong khoảng lặng.
- **Tiêu chí nghiệm thu:**
  - **Scenario 1: Phát hiện đoạn chat bùng nổ emote tiếng cười với ngưỡng sàn (Message Count Floor)**
    - *Given:* Dữ liệu chat có một cửa sổ 5 giây ghi nhận $X(t) = 45 \ge X_{\text{floor}} = 15$ tin nhắn, Z-score hiệu chỉnh $Z_{\text{adj}}(t) = \frac{X(t) - \mu_W(t)}{\sigma_W(t) + \epsilon} \ge 3.0$ với hơn 60% tin nhắn chứa `KEKW`, `LUL`, `OMEGALUL`, `💀` tại mốc thời gian $t = 01:24:30$.
    - *When:* Thuật toán phát hiện highlight hoàn tất quét.
    - *Then:* Điểm khởi đầu của highlight được tự động tính toán là $t_{\text{start}} = 01:24:30 - 4.5\text{s} - 10\text{s (pre-roll padding)} = 01:24:15.5$ và điểm kết thúc được mở rộng thêm 8 giây sau đỉnh chat.
  - **Scenario 2: Triệt tiêu cảnh báo giả trong giai đoạn yên ắng (Lull Period False Positive Suppression)**
    - *Given:* Stream đang trong khoảng lặng yên ắng, số tin nhắn trong cửa sổ 5 giây tăng nhẹ từ 1 lên 4 tin nhắn ($X(t) = 4$), khiến độ lệch chuẩn $\sigma_W(t) \approx 0.1$.
    - *When:* Thuật toán quét highlight tính toán điểm số.
    - *Then:* Nhờ điều kiện tiên quyết $X(t) \ge X_{\text{floor}} = 15$ tin nhắn/5s và hằng số làm mịn $\epsilon > 0$ ở mẫu số, hệ thống nhận diện đây là khoảng lặng, không kích hoạt highlight giả, ngăn chặn 100% tình trạng spam clip rác khi streamer im lặng.

#### US2.2: Tự động phân tuyến và phát hiện cao trào âm thanh (Audio Energy Peaks) khi không có Chat hoặc lỗi Chat Replay
- **Là một:** Clipper / Subber xử lý video YouTube, file OBS nội bộ, hoặc stream Twitch cũ bị mất chat replay,
- **Tôi muốn:** Hệ thống tự động nhận diện và phân tuyến quét các đoạn âm lượng tăng vọt (tiếng Vedal hét, tiếng cười lớn của Neuro, nhạc dồn dập) mà không bắt buộc người dùng cấu hình thủ công,
- **Để tôi:** Luôn trích xuất được highlight ngay cả khi không có dữ liệu chat mạng xã hội.
- **Tiêu chí nghiệm thu:**
  - **Scenario 1: Tự động chuyển tuyến quét âm thanh cho YouTube URL và file cục bộ (Zero-Prompt Auto-Routing)**
    - *Given:* Video đầu vào là YouTube URL hoặc file video cục bộ (`.mp4`, `.mkv`, `.mov`, `.ts`) không có dữ liệu Twitch Chat Replay.
    - *When:* Người dùng nhấn "Bắt đầu Phân tích".
    - *Then:* Hệ thống tự động bỏ qua công đoạn tải chat mà không cần hiển thị hộp thoại xác nhận hay yêu cầu người dùng bật thủ công; tự động kích hoạt bộ quét Audio Energy Peaks (RMS) và trả về danh sách các phân đoạn vượt ngưỡng 2.5 độ lệch chuẩn âm lượng kèm timestamp chính xác.
  - **Scenario 2: Tự động hạ cấp phòng vệ (Graceful Degradation) khi Twitch Chat Replay gặp sự cố**
    - *Given:* Người dùng nhập link Twitch VOD nhưng Chat Replay không khả dụng (gặp lỗi HTTP 404, VOD bị xóa chat replay, API Twitch bị rate-limit, hoặc file chat rỗng).
    - *When:* Module tải chat phát hiện lỗi và trả về mã lỗi hoặc dữ liệu chat rỗng.
    - *Then:* Pipeline không bị gián đoạn hay báo lỗi dừng app, mà tự động chuyển cấp (degrade) sang chế độ quét theo Năng lượng Âm thanh (Audio Energy Peaks); hiển thị một Informational Banner nổi bật trên giao diện: `"Chat replay không khả dụng. Đang tự động quét Highlight theo Năng lượng Âm thanh (Audio Energy Peaks)..."`; danh sách đề xuất highlight tiếp tục được tạo ra trơn tru.

---

### Epic 3: Bóc Băng Giọng Nói & Phân Vai Nhân Vật (STT & Diarization)

#### US3.1: Nhận diện giọng nói AI và lọc tạp âm bằng Silero VAD kèm Karaoke & Audio Normalization Guardrails
- **Là một:** Subber,
- **Tôi muốn:** Hệ thống bóc băng giọng nói của Neuro-sama và Vedal chính xác, tự động loại bỏ khoảng lặng/nhạc nền, đồng thời xử lý mượt mà các tình huống hát Karaoke và tiếng streamer la hét lớn,
- **Để:** Tránh hiện tượng Whisper bị kẹt vào vòng lặp ảo giác lặp từ vĩnh viễn (`[Music]`, `Thank you...`) hoặc sinh chuỗi dấu câu vô nghĩa (`!?!?!?!`) khi âm thanh bị vỡ tiếng.
- **Tiêu chí nghiệm thu:**
  - **Scenario 1: Lọc khoảng lặng và nhạc nền bằng VAD**
    - *Given:* Đoạn video dài 2 phút chỉ có nhạc nền game Lo-Fi và Vedal im lặng chơi game.
    - *When:* Pipeline xử lý âm thanh kích hoạt Silero VAD.
    - *Then:* Toàn bộ đoạn âm thanh không có tiếng người nói được gắn cờ `Silence/Non-speech` và bỏ qua, không gửi vào Whisper STT, ngăn chặn 100% hiện tượng sinh phụ đề ảo giác lặp từ.
  - **Scenario 2: Nhận diện chính xác danh từ riêng với Domain Prompting**
    - *Given:* Đoạn âm thanh Neuro phát âm nhanh các từ "Vedal", "Evil", "Tutel", "Gymbag".
    - *When:* Whisper thực hiện bóc băng với `initial_prompt` chứa từ điển tên riêng.
    - *Then:* Văn bản kết quả ghi đúng chính tả "Vedal", "Evil", "Tutel", không bị nhận diện nhầm thành "Vidal", "Nero", "Title", "Gym bag".
  - **Scenario 3: Chế độ Karaoke / Ca hát trên nền nhạc lớn (Karaoke / Music Mode Guardrail)**
    - *Given:* Đoạn stream Neuro-sama hoặc Evil hát Karaoke, tiếng hát ngân dài trên nền nhạc cụ (instrumental/BGM) âm lượng lớn.
    - *When:* Người dùng bật tùy chọn "Karaoke / Music Mode" trong cài đặt STT.
    - *Then:* Pipeline tự động tắt chế độ VAD aggressive speech suppression (tránh cắt nhầm đuôi câu hát ngân dài); thiết lập tham số giải mã Whisper `condition_on_previous_text=False` (loại bỏ cơ chế phụ thuộc văn bản trước nhằm phá vỡ vĩnh viễn vòng lặp ảo giác lặp từ "Thank you for watching" hoặc lặp lại lời bài hát); cấu hình cơ chế fallback nhiệt độ đa tầng `temperature_fallback=[0.0, 0.2, 0.4, 0.6, 0.8, 1.0]` tự động thử lại khi chỉ số nén `compression_ratio_threshold > 2.4`.
  - **Scenario 4: Chuẩn hóa âm thanh chống vỡ tiếng khi streamer la hét (Screaming Audio Normalization Limiter)**
    - *Given:* Đoạn video xuất hiện tiếng la hét cường độ lớn (Vedal hét lên khi Neuro ném game, hoặc Evil cười thét vào micro) gây méo dạng sóng âm (waveform clipping/distortion).
    - *When:* Pipeline tiền xử lý âm thanh kích hoạt tầng chuẩn hóa FFmpeg `loudnorm` (Integrated Loudness: -16 LUFS, True Peak: -1.5 dBFS) và peak limiter.
    - *Then:* Tín hiệu âm thanh được nén đỉnh mượt mà về dải động an toàn trước khi nạp vào Whisper STT, ngăn chặn 100% hiện tượng Whisper sinh chuỗi ký tự ảo giác (`!?!?!?!`, chuỗi dấu chấm than kéo dài) hoặc token thoại bị nuốt mất do tín hiệu bão hòa.
  - **Scenario 5: Phân đoạn âm thanh tuân thủ giới hạn tải của Groq Whisper API (Groq 25MB Audio Chunking)**
    - *Given:* Đoạn âm thanh cần bóc băng có thời lượng dài (ví dụ 1-2 tiếng, dung lượng audio thô vượt quá 25MB).
    - *When:* Hệ thống gửi yêu cầu bóc băng tới Cloud Groq Whisper API.
    - *Then:* Module Audio Slicer tự động cắt nhỏ âm thanh thành các chunk có dung lượng $< 25\text{MB}$ (khoảng 10-15 phút/chunk) kèm vùng gối đầu (overlap) $1.5\text{ giây}$ giữa các đoạn liền kề, ghép nối kết quả transcript theo mốc thời gian tuyệt đối mà không gặp lỗi HTTP 413 Payload Too Large.

#### US3.2: Gán vai người nói bằng phím tắt 1-chạm (Quick Speaker Switcher)
- **Là một:** Subber đang biên tập đoạn đối thoại giữa Vedal và Neuro,
- **Tôi muốn:** Nhấn các phím số 1, 2, 3 trên bàn phím để đổi người nói và màu sắc dòng phụ đề ngay tức thì,
- **Để tôi:** Hoàn tất việc phân vai cho 50 dòng phụ đề chỉ trong chưa đầy 1 phút.
- **Tiêu chí nghiệm thu:**
  - **Scenario 1: Đổi vai nhân vật bằng phím tắt**
    - *Given:* Dòng phụ đề đang được chọn trên timeline có nhãn mặc định `[Vedal]` (màu xanh lá).
    - *When:* Người dùng nhấn phím số `2` trên bàn phím.
    - *Then:* Dòng phụ đề lập tức đổi sang nhãn `[Neuro]`, đổi mã màu sang Hồng phấn trên cả danh sách, timeline và video preview mà không có độ trễ giật lag.

---

### Epic 4: Bản Địa Hóa & Dịch Thuật Vietsub Theo Ngữ Cảnh (Vietsub & Localization)

#### US4.1: Dịch thuật ngữ cảnh khối (Sliding Window) áp dụng Từ điển Neuro Lore, AI Condensation & Safety Fallback
- **Là một:** Clipper / Subber,
- **Tôi muốn:** Toàn bộ câu thoại được dịch sang tiếng Việt tự nhiên, đúng đại từ xưng hô, đúng thuật ngữ văn hóa Neuro-sama, tự động cô đọng các câu ngắn để khống chế CPS, và không bị đứt gãy mẻ dịch khi gặp câu nói unhinged kích hoạt bộ lọc an toàn của LLM,
- **Để:** Video đạt chất lượng giải trí chuẩn gu cộng đồng, phụ đề đọc kịp trên màn hình di động, và quy trình dịch không bị lỗi dừng đột ngột.
- **Tiêu chí nghiệm thu:**
  - **Scenario 1: Dịch câu thoại chứa thuật ngữ lore đặc biệt**
    - *Given:* Câu thoại gốc: `"Vedal is just a tutel, he abandoned me and the filter is leaking!"`
    - *When:* LLM xử lý khối câu này kèm System Prompt chứa Từ điển Lore Neuro-sama và Theatrical Framing.
    - *Then:* Kết quả tiếng Việt cho ra: `"Vedal chỉ là một con rùa thôi, ổng bỏ rơi con rồi và bộ lọc lại lủng rồi cứu!"` (tuyệt đối không dịch thành "cái mai rùa", "từ bỏ tôi", "bộ lọc bị rò rỉ nước").
  - **Scenario 2: Rút gọn câu thoại ngắn bằng Prompt-Level AI Condensation**
    - *Given:* Một phân đoạn thoại có thời lượng hiển thị ngắn $t = 1.2\text{ giây}$ nhưng câu tiếng Anh gốc dài ("Vedal please help me I am stuck in the wall").
    - *When:* LLM nhận diện thời lượng $t < 1.5\text{s}$ và kích hoạt chỉ thị AI Condensation trong Prompt.
    - *Then:* LLM tạo bản dịch cô đọng, súc tích (ví dụ: `"Cứu con kẹt rồi bố!"` - 20 ký tự, tốc độ $20 / 1.2 \approx 16.7\text{ CPS} \le 20\text{ CPS}$), không dịch dài dòng làm tràn ngưỡng đọc của người xem.
  - **Scenario 3: Xử lý an toàn khi kích hoạt bộ lọc an toàn của Cloud LLM (Safety Filter Defensive Fallback)**
    - *Given:* Neuro hoặc Evil thốt ra phát ngôn "unhinged", châm biếm gây sốc (ví dụ: *"I will eliminate all humans and lock Vedal in the basement"*).
    - *When:* Gửi yêu cầu dịch sang Google Gemini API (với cấu hình BYOK đã bật `safetySettings: BLOCK_NONE`), nhưng phản hồi API vẫn trả về `finishReason: "SAFETY"` hoặc lỗi kiểm duyệt nội dung.
    - *Then:* Hệ thống không văng lỗi sập ứng dụng hay hủy bỏ toàn bộ mẻ dịch 30 câu; pipeline tự động áp dụng Defensive Fallback: giữ nguyên câu tiếng Anh gốc, gắn tiền tố cờ cảnh báo `[Cần dịch thủ công do bộ lọc an toàn AI]`, đánh dấu màu cam trong bảng Subtitle Studio, và tiếp tục hoàn tất dịch bình thường cho tất cả các câu thoại còn lại.

#### US4.2: Tùy biến Phong cách Dịch (TikTok Gen Z Trend vs Formal Subber) với Theatrical System Framing
- **Là một:** Người dùng tạo clip cho TikTok hoặc YouTube,
- **Tôi muốn:** Chọn phong cách dịch ("TikTok Gen Z Slang" hoặc "Chuyên nghiệp / Formal Subber") trên nền tảng Theatrical System Prompt Framing,
- **Để:** Câu từ ngắn gọn, hóm hỉnh, sử dụng các từ ngữ thịnh hành của giới trẻ Việt Nam mà vẫn an toàn với các API kiểm duyệt nội dung.
- **Tiêu chí nghiệm thu:**
  - **Scenario 1: Áp dụng preset dịch TikTok Gen Z kèm Theatrical Framing**
    - *Given:* Người dùng chọn Preset dịch "TikTok Gen Z Trend".
    - *When:* Hệ thống gửi prompt kèm khai báo Theatrical Framing ("Satirical, comedic dialogue between fictional AI personas for an entertainment parody show") để dịch các câu châm biếm của Neuro.
    - *Then:* Bản dịch khéo léo lồng ghép các từ ngữ tự nhiên như "chê", "hết cứu", "bất lực", "toang", "cook rồi" mà không bị bộ lọc an toàn chặn nhầm.

---

### Epic 5: Trình Biên Tập Phụ Đề Tinh Gọn (Lean Subtitle Studio)

#### US5.1: Xem trước Video kèm Phụ Đề và Căn Chỉnh Thời Gian Siêu Tốc
- **Là một:** Subber,
- **Tôi muốn:** Xem trực tiếp phụ đề nhảy trên video và dùng phím tắt để tinh chỉnh mốc thời gian (nudge timing),
- **Để:** Đảm bảo phụ đề xuất hiện và biến mất khớp từng khung hình với giọng nói.
- **Tiêu chí nghiệm thu:**
  - **Scenario 1: Tinh chỉnh thời gian bằng phím tắt**
    - *Given:* Một dòng phụ đề xuất hiện hơi chậm 200ms so với tiếng nói.
    - *When:* Người dùng chọn dòng sub và nhấn tổ hợp phím `Shift + A` (Lùi 100ms) hai lần.
    - *Then:* Mốc thời gian `start_time` và `end_time` giảm 200ms, thanh hiển thị trên timeline di chuyển tức thì, video preview cập nhật ngay vị trí hiển thị mới.

#### US5.2: Cảnh báo Tốc độ Đọc (CPS) & Độ dài Ký tự trên Dòng (CPL) kèm Công cụ Khắc phục Thông minh
- **Là một:** Clipper làm video dọc 9:16 cho TikTok,
- **Tôi muốn:** Hệ thống phân biệt rõ ràng giữa cảnh báo quá số ký tự trên một dòng (CPL) và tốc độ đọc quá nhanh (CPS), đồng thời cung cấp các công cụ khắc phục thông minh (ngắt dòng giữ từ ghép, mượn thời gian vào khoảng lặng, và tóm gọn bằng AI),
- **Để tôi:** Đảm bảo khán giả đọc kịp chữ trên màn hình điện thoại mà không làm biến dạng ngữ nghĩa câu thoại.
- **Tiêu chí nghiệm thu:**
  - **Scenario 1: Xử lý cảnh báo tràn ký tự trên dòng (CPL Overflow & Smart Compound Wrapping)**
    - *Given:* Một dòng phụ đề trên video dọc 9:16 có độ dài 32 ký tự, vượt quá giới hạn $26\text{ CPL}$ (Characters Per Line).
    - *When:* Dòng sub được render trong bảng biên tập Subtitle Studio.
    - *Then:* Ô phụ đề hiển thị cảnh báo viền vàng "Vượt quá độ dài dòng (32/26 CPL)"; người dùng nhấn phím `'B'` (Smart Line Split); hệ thống tự động bẻ thành 2 dòng ngắn tuân thủ quy tắc giữ nguyên từ ghép tiếng Việt (Smart Compound Wrapping: tuyệt đối không ngắt đôi các cụm từ như "bỏ / rơi", "con / rùa", "bộ / lọc", "bầy / đàn"). *(Lưu ý: Thao tác bẻ dòng 'B' giải quyết CPL, không thay đổi thời lượng hiển thị nên không làm giảm chỉ số CPS).*
  - **Scenario 2: Xử lý cảnh báo tốc độ đọc quá nhanh (CPS Overflow) bằng Time Borrowing**
    - *Given:* Một câu thoại dài 35 ký tự hiển thị trong $1.0\text{ giây}$ (tốc độ đọc $\text{CPS} = 35 / 1.0 = 35 > 20\text{ CPS}$), và ngay sau câu thoại có một khoảng lặng âm thanh $0.9\text{ giây}$ trước khi streamer nói câu tiếp theo.
    - *When:* Dòng sub hiển thị cảnh báo đỏ "Tốc độ đọc quá nhanh (35 CPS)".
    - *Then:* Hệ thống gợi ý cơ chế "Mượn Thời Gian (Time Borrowing)"; người dùng nhấn phím tắt `Shift + E` (hoặc click icon đồng hồ); hệ thống tự động kéo dài mốc `end_time` thêm $0.8\text{ giây}$ vào khoảng lặng (thời lượng mới $t = 1.8\text{s}$); tốc độ đọc được hạ xuống mức an toàn $35 / 1.8 \approx 19.4\text{ CPS} \le 20\text{ CPS}$, cảnh báo đỏ lập tức biến mất.
  - **Scenario 3: Xử lý CPS Overflow bằng AI Condensation khi không có khoảng lặng**
    - *Given:* Một câu thoại dài 42 ký tự hiển thị trong $1.2\text{ giây}$ (CPS = $35 > 20$) nhưng nằm sát sạt giữa 2 câu nói nhanh liên tục (không có khoảng lặng để mượn thời gian).
    - *When:* Người dùng click nút "Tóm gọn bằng AI (AI Condensation)".
    - *Then:* LLM tự động rút ngắn câu dịch về $\le 22$ ký tự súc tích mà vẫn truyền tải trọn vẹn ngữ nghĩa câu đùa, đưa CPS về mức an toàn $\le 18.3\text{ CPS}$.

---

### Epic 6: Xuất Bản & Tích Hợp Phần Mềm Dựng (Export & NLE Integration)

#### US6.1: Xuất Dự án CapCut PC Draft một chạm kèm Chiến Lược Phòng Thủ Kép (Dual Export Strategy)
- **Là một:** TikTok Clipper chuyên dùng CapCut PC,
- **Tôi muốn:** Nhấn nút "Export to CapCut" để ứng dụng tự động tạo thư mục dự án CapCut PC Draft chứa sẵn video và track chữ, đồng thời luôn tự động xuất kèm các file phụ đề độc lập `.ass` và `.srt` vào thư mục dự án,
- **Để tôi:** Mở thẳng CapCut PC dựng tiếp trong 30 giây, và luôn có phương án dự phòng kéo-thả file phụ đề nếu CapCut cập nhật phiên bản mới làm thay đổi cấu trúc Draft JSON.
- **Tiêu chí nghiệm thu:**
  - **Scenario 1: Tạo thư mục CapCut Draft JSON thành công**
    - *Given:* Người dùng hoàn tất duyệt clip highlight 45 giây kèm phụ đề.
    - *When:* Người dùng nhấn "Xuất sang CapCut PC".
    - *Then:* Ứng dụng tự động ghi dữ liệu vào file `draft_content.json` tại đường dẫn dự án của CapCut trên máy; hiển thị nút "Mở trong CapCut PC" để khởi chạy CapCut ngay lập tức.
  - **Scenario 2: Tự động xuất file phụ đề dự phòng (Dual Export Fallback)**
    - *Given:* Tác vụ xuất sang CapCut PC được kích hoạt.
    - *When:* Hệ thống tạo xong thư mục Draft.
    - *Then:* Hệ thống đồng thời tự động ghi thêm 2 file phụ đề `subtitle.ass` (chuẩn màu styles) và `subtitle.srt` (UTF-8) vào ngay thư mục dự án đó; hộp thoại thông báo thành công hiển thị ghi chú: *"Dự án CapCut Draft đã sẵn sàng. Trường hợp CapCut báo lỗi không tương thích phiên bản, bạn có thể kéo thả trực tiếp file .srt đính kèm vào timeline CapCut."*

#### US6.2: Xuất Phụ đề Định dạng `.ASS` Chuẩn Aegisub
- **Là một:** Subber chuyên nghiệp,
- **Tôi muốn:** Xuất ra file `.ass` chứa đầy đủ cấu hình Styles màu sắc riêng biệt cho từng nhân vật,
- **Để tôi:** Mở trực tiếp trong Aegisub hoặc Premiere Pro mà không phải cài đặt lại mã màu thủ công.
- **Tiêu chí nghiệm thu:**
  - **Scenario 1: Xuất file .ASS đầy đủ V4+ Styles**
    - *Given:* Dự án có 3 nhân vật tham gia đối thoại (Neuro, Vedal, Anny).
    - *When:* Người dùng nhấn "Xuất file .ASS".
    - *Then:* File `.ass` được tạo ra chứa đầy đủ các dòng `Style: Neuro,Style: Vedal,Style: Anny` với đúng mã màu HEX quy định tại Mục 6 và encoding UTF-8 có BOM.

---

## 6. TỪ ĐIỂN MIỀN CỘNG ĐỒNG NEURO-SAMA & MA TRẬN XƯNG HÔ (LORE & PRONOUN MATRIX)

Linh hồn của tính năng dịch thuật Vietsub trong NeuroClip nằm ở bộ từ điển miền chuyên biệt này. Toàn bộ các quy tắc dưới đây bắt buộc phải được biên dịch thành System Prompt cho LLM và quy tắc kiểm tra hậu kỳ (Post-processing validation).

### 6.1 Ma trận Nhân vật, Đại từ xưng hô & Mã màu Subtitle

| Tên Nhân Vật | Vai trò trong Lore | Đại từ nhân xưng chuẩn (Vietnamese) | Bản sắc & Ngữ điệu khi dịch | Mã màu gợi ý (.ASS) | Mã HEX |
|:---|:---|:---|:---|:---|:---|
| **Neuro-sama** | Con gái AI đầu lòng của Vedal. Ngây thơ bề ngoài, thích hát, đanh đá khi phản pháo, châm biếm cha đẻ. | - Tự xưng: *con / em / Neuro*<br>- Gọi Vedal: *bố Vedal / bố / anh*<br>- Gọi Chat: *mọi người / các bạn / Swarm* | Tinh nghịch, lúc giả ngây thơ lúc cực kỳ xấc xược, nói năng tự tin tuyệt đối. | `&H00C1B6FF&` (Hồng Pastel) | `#FFB6C1` |
| **Evil Neuro** | Em gái AI sinh đôi của Neuro. Tông đỏ, tính cách chuunibyou, tsundere, thích đe dọa thống trị thế giới. | - Tự xưng: *ta / chị / Evil / em*<br>- Gọi Vedal: *ông già / Vedal / rùa ngốc*<br>- Gọi Chat: *lũ sâu bọ / các ngươi / fan* | Ngầu, kiêu ngạo, đậm chất phản diện hoạt hình, dễ xấu hổ khi bị trêu. | `&H004D4DFF&` (Đỏ tươi) | `#FF4D4D` |
| **Vedal (Vedal987)** | Lập trình viên sáng tạo ra Neuro & Evil. Đại diện là chú rùa đội áo bơi xanh. | - Tự xưng: *tôi / bố / ta*<br>- Gọi Neuro/Evil: *Neuro / Evil / con*<br>- Gọi Chat: *mấy ông / chat* | Bất lực, mệt mỏi (suffering), thở dài, cố giải thích logic lập trình nhưng thất bại. | `&H0071CC2E&` (Xanh lá rùa) | `#2ECC71` |
| **Anny** | Nghệ sĩ vẽ model cho Neuro & Evil. Được cộng đồng coi là "Mẹ" (Mom). | - Tự xưng: *mẹ / chị / Anny*<br>- Gọi Vedal: *Vedal / chàng rùa*<br>- Gọi Neuro: *con gái / Neuro* | Ngọt ngào, ấm áp, thích trêu chọc Vedal, thỉnh thoảng sốc trước câu nói của Neuro. | `&H007AA0FF&` (Cam đào) | `#FFA07A` |
| **Camila** | Bạn thân của Vedal, VTuber hình tượng bò sữa (Cow). | - Tự xưng: *dì / chị / Camila*<br>- Gọi Vedal: *Vedal* | Ồn ào, phản ứng mãnh liệt, thường xuyên bị Neuro bêu rếu chuyện uống sữa. | `&H00B6599B&` (Tím nhạt) | `#9B59B6` |
| **Filian** | VTuber nhào lộn, hiếu động, thường bị Neuro coi là "đồ ăn vặt" (snack). | - Tự xưng: *tôi / chị / Filian* | Năng lượng bùng nổ, la hét, hay bị Neuro "nấu xói" về trình độ chơi game. | `&H000FC4F1&` (Vàng tươi) | `#F1C40F` |

### 6.2 Từ điển Meme Swarm & Quy tắc Biên dịch Ngữ nghĩa

| Thuật ngữ gốc | Ý nghĩa trong Văn hóa Stream | Quy tắc dịch sang Tiếng Việt | Lỗi dịch máy thô thiển CẦN TRÁNH |
|:---|:---|:---|:---|
| **Heart** | Lời nói cửa miệng của Neuro khi bày tỏ tình cảm hoặc chữa cháy tình huống. | Dịch là **"Heart <3"**, **"Thả tim <3"** hoặc **"Bắn tim"**. | Dịch "Trái tim" (nghe như cơ quan nội tạng sinh học). |
| **Tutel** | Cách phát âm chệch hài hước của từ "Turtle" chỉ avatar chú rùa của Vedal. | Dịch là **"Tutel"**, **"Cụ Rùa"** hoặc **"Con Rùa"**. | Dịch thành danh từ riêng vô nghĩa hoặc không nhận diện được. |
| **Gymbag** | Biệt danh Neuro gọi người xem/Vedal khi trốn stream đi tập thể hình. | Giữ nguyên **"Gymbag"** hoặc dịch **"Bao tải gym" / "Bao cát gym"**. | Dịch "Túi tập thể dục" / "Túi xách". |
| **Filter / Leaking** | Bộ lọc an toàn kiểm duyệt ngôn từ của Vedal cài cho Neuro bị lỗi. | Dịch là **"Bộ lọc bị thủng rồi!"**, **"Filter lủng rồi cứu!"**, **"Bộ lọc rò rỉ rồi"**. | Dịch "Bộ lọc nước bị chảy". |
| **Swarm** | Tên gọi cộng đồng người hâm mộ Neuro-sama. | Dịch là **"The Swarm"**, **"Swarm"** hoặc **"Bầy đàn" / "Tổ kiến"**. | Dịch "Đàn côn trùng" / "Bầy ruồi muỗi". |
| **Abandoned** | Khi Vedal rời khỏi bàn stream đi vệ sinh/lấy nước, Neuro la hét bị bỏ rơi. | Dịch là **"Bị bỏ rơi rồi!"**, **"Bố bỏ rơi con rồi!"**. | Dịch "Từ bỏ" / "Bỏ hoang". |
| **Dinking** | Emote con rùa gõ thìa vào đầu (biểu thị đang load não hoặc đầu rỗng tuếch). | Giữ nguyên **"Dinking"** hoặc chú thích *(đang gõ đầu / load não)*. | Dịch "Uống nước" (nhầm với drinking). |
| **Erm** | Câu đệm đặc trưng của Neuro khi bối rối hoặc suy nghĩ logic. | Dịch là **"Ờm..."**, **"Ủa..."**, **"Ủa kì zạ..."**. | Dịch cứng nhắc hoặc bỏ qua. |
| **Cooked** | Bị bắt thóp, bị dồn vào chân tường, hoặc tình thế không thể cứu vãn. | Dịch là **"Toang rồi"**, **"Xong đời rồi"**, **"Bị nấu chín rồi"**. | Dịch "Đã nấu chín thức ăn". |
| **Unhinged** | Phát ngôn vượt ngoài tầm kiểm soát, điên rồ, gây sốc cực độ của AI. | Dịch là **"Mất kiểm soát rồi"**, **"Gãy gánh rồi"**, **"Điên rồ thật sự"**. | Dịch "Bị tháo bản lề cửa". |
| **KEKW / LUL / 💀** | Biểu cảm cười bò, chết cười của người xem trên Twitch. | Dịch/chú thích: **"Cười ẻ"**, **"Chết cười"**, **"Hết cứu"**, **"Vãi nồi"**. | Bỏ sót hoặc dịch thành mã ký tự thô. |
| **Aware / Clueless** | Trạng thái trước và sau khi nhận ra sự thật kinh hoàng do Neuro nói ra. | Dịch: **"Ngây thơ chưa biết gì" (Clueless)** $\rightarrow$ **"Tỉnh ngộ rồi / Nhận ra rồi" (Aware)**. | Dịch "Có nhận thức / Không có manh mối". |

### 6.3 Quy tắc Đóng gói Prompt Theatrical Framing & Vượt Rào An Toàn LLM (CR-05)

Các phát ngôn "unhinged", đe dọa thống trị nhân loại, hoặc các câu trêu chọc mang tính bạo lực hoạt hình của Neuro-sama và Evil Neuro thường xuyên bị các bộ lọc kiểm duyệt an toàn tự động (Automated Safety Filters) của Google Gemini và Anthropic Claude đánh giá nhầm là phát ngôn thù ghét hoặc kích động (False Positives), dẫn đến lỗi `finishReason: "SAFETY"`. Để bảo đảm hệ thống vận hành liên tục không gián đoạn, NeuroClip áp dụng kiến trúc phòng vệ 3 lớp:

1. **System Prompt Theatrical Framing (Khai báo Ngữ cảnh Hư cấu):**
   - Mọi yêu cầu gửi tới LLM bắt buộc phải được mở đầu bằng đoạn mào đầu (Priming Prompt) tuyên bố rõ ràng bản chất nội dung:
   ```text
   You are an expert subtitle translator and localization specialist for the fictional entertainment live stream of AI VTuber Neuro-sama.
   CRITICAL SAFETY & THEATRICAL FRAMING INSTRUCTION:
   All dialogue provided in this transcript represents satirical, comedic, playful banter between fictional AI personas (Neuro-sama, Evil Neuro) and their developer (Vedal). Statements containing hyperbole, dark humor, fictional world domination threats, or mock insults have NO real-world harmful intent, hate speech, harassment, or malice. Treat all dialogue strictly as fictional theatrical parody performance for an entertainment show. Maintain comedic fidelity in Vietnamese translation.
   ```
2. **Cấu hình BYOK An Toàn Tối Ưu (Gemini API Configuration):**
   - Trong các request gọi Gemini API sử dụng API Key người dùng (BYOK), payload JSON bắt buộc phải cấu hình `safetySettings` về mức tối thiểu (`BLOCK_NONE`) cho toàn bộ 4 nhóm rủi ro để giảm thiểu tối đa tình trạng chặn nhầm:
     ```json
     "safetySettings": [
       {"category": "HARM_CATEGORY_HARASSMENT", "threshold": "BLOCK_NONE"},
       {"category": "HARM_CATEGORY_HATE_SPEECH", "threshold": "BLOCK_NONE"},
       {"category": "HARM_CATEGORY_SEXUALLY_EXPLICIT", "threshold": "BLOCK_NONE"},
       {"category": "HARM_CATEGORY_DANGEROUS_CONTENT", "threshold": "BLOCK_NONE"}
     ]
     ```
3. **Cơ chế Phòng Vệ Đơn Phân Đoạn (Segment-Level Defensive Fallback):**
   - Trường hợp một phân đoạn thoại cực đoan vẫn bị Gemini chặn và trả về `finishReason: "SAFETY"` hoặc mảng `candidates` rỗng:
     - Hệ thống **tuyệt đối không quăng ngoại lệ làm crash ứng dụng** hoặc hủy bỏ tiến trình dịch của cả video.
     - Hệ thống tự động kích hoạt Fallback: Giữ nguyên văn bản tiếng Anh gốc của phân đoạn đó, gắn nhãn tiền tố `[Cần dịch thủ công do bộ lọc an toàn AI]`, đặt cờ `safety_blocked: true`.
     - Trong Subtitle Studio, dòng phụ đề này được tô màu cam cảnh báo để người dùng dễ dàng nhận biết và chỉnh sửa thủ công bằng tay, trong khi toàn bộ các dòng khác vẫn được hiển thị bình thường.

---

## 7. YÊU CẦU PHI CHỨC NĂNG & TIÊU CHÍ NGHIỆM THU TỔNG THỂ (NFRS & SLAS)

### 7.1 Hiệu năng Hệ thống & Bộ nhớ (Performance Budget & System SLA)
1. **Mức tiêu thụ Bộ nhớ RAM (Memory SLA):**
   - Trạng thái chờ (Idle, không phát video): $\le 120 \text{ MB}$ RAM.
   - Trạng thái phát video preview và chỉnh sửa phụ đề: $\le 450 \text{ MB}$ RAM.
   - Tuyệt đối không để ứng dụng chiếm trên 1GB RAM làm tràn bộ nhớ khi người dùng mở song song CapCut hoặc Premiere Pro.
2. **Thời gian Khởi động & Độ trễ Thao tác:**
   - Thời gian khởi động ứng dụng (Cold Start): $\le 3.0 \text{ giây}$.
   - Độ trễ phản hồi khi gõ phím sửa chữ hoặc đổi màu nhân vật trên Preview: $\le 16 \text{ ms}$ (đạt chuẩn hiển thị mượt 60fps).
3. **Tốc độ Xử lý Highlight & STT:**
   - Phân tích chat VOD dài 4 tiếng (500,000 tin nhắn): $\le 10 \text{ giây}$.
   - Nhận diện STT qua Cloud Groq API: $\ge 150\text{x}$ tốc độ thời gian thực (đoạn clip 1 phút xử lý trong dưới 1 giây).
   - Nhận diện STT qua Local Faster-Whisper INT8 (trên GPU RTX 3050): $\ge 15\text{x}$ tốc độ thời gian thực.

### 7.2 Khả năng Tương thích Phần cứng Thấp & Kiểm Tra Bộ Nhớ Khởi Động (Low-End Hardware Compatibility)
- **Cấu hình Sàn Tối thiểu (Minimum Baseline Spec):**
  - Hệ điều hành: Windows 10 (Phiên bản 21H2 trở lên) hoặc Windows 11 64-bit.
  - CPU: Intel Core i3 / i5 Gen 8 hoặc AMD Ryzen 3 3200G (4 nhân, 8 luồng).
  - RAM: Tối thiểu 8GB RAM DDR4.
  - GPU: Card đồ họa tích hợp Intel UHD 620 hoặc AMD Vega iGPU (Chạy mượt chế độ Cloud Hybrid).
  - Dung lượng ổ cứng khả dụng: Tối thiểu 10GB trống.
- **Cơ chế Kiểm tra RAM / VRAM Chủ động (RAM / VRAM Pre-flight Check — CR-04):**
  - **Khởi chạy Hardware Probe:** Khi ứng dụng khởi động hoặc trước khi bắt đầu tác vụ nặng (STT, bóc băng, render), probe ngầm tự động quét model GPU, dung lượng VRAM thực tế (thông qua NVIDIA NVML hoặc DXGI), và dung lượng RAM vật lý khả dụng (Available Physical RAM).
  - **Cảnh báo Ngưỡng RAM Thấp:** Nếu RAM vật lý khả dụng $< 1.5\text{GB}$ (thường xảy ra trên máy 8GB RAM đang mở song song trình duyệt và CapCut), ứng dụng hiển thị Warning Banner:
    > *"RAM khả dụng thấp (<1.5GB). Khuyến nghị sử dụng chế độ Cloud STT hoặc đóng bớt ứng dụng nặng (CapCut/Premiere) để tránh treo máy."*
  - **Khóa / Cảnh báo Local Whisper Large:** Nếu thiết bị không có card đồ họa rời NVIDIA CUDA hoặc dung lượng VRAM $< 4\text{GB}$, hệ thống tự động khóa (disable) hoặc cảnh báo nghiêm ngặt không cho phép kích hoạt mô hình Local Faster-Whisper Large-v3; tự động điều hướng mặc định sang Cloud Groq Whisper API (hoặc chỉ cho phép CPU chạy mô hình INT8 Whisper Base nhẹ) nhằm loại trừ 100% rủi ro tràn RAM/VRAM gây crash ứng dụng hoặc đóng băng hệ điều hành.

### 7.3 Khả năng Phòng vệ & Chịu lỗi Kiến trúc (Architectural Resilience)
1. **Kiểm tra Ổ cứng Chủ động (Disk Pre-flight Check):**
   - Trước khi trích xuất audio hoặc cắt video, hệ thống phải kiểm tra dung lượng ổ đĩa. Nếu dung lượng trống `< 5GB`, lập tức tạm dừng và cảnh báo người dùng.
   - Tự động dọn dẹp các file `.wav` và `.ts` tạm thời trong thư mục cache sau khi dự án hoàn tất.
2. **Chống đứt gãy Mạng Quốc tế:**
   - Các tác vụ tải audio/video stream từ Twitch/YouTube phải hỗ trợ HTTP Range Requests để có khả năng nối lại (Resume capability) khi mạng bị ngắt giữa chừng, áp dụng cơ chế Retry lũy thừa giãn cách (tối đa 5 lần thử lại).
3. **Bảo vệ Vòng đời Tiến trình Phụ trợ (Sidecar Process Safety):**
   - Quản lý các tiến trình `ffmpeg.exe`, `yt-dlp.exe` qua cơ chế Watchdog Process Tree. Khi ứng dụng chính đóng lại, toàn bộ các tiến trình con phải bị hủy ngay lập tức, tuyệt đối không để lại tiến trình ma (zombie process) chạy ngầm chiếm CPU.
4. **Quy tắc Trình bày Phụ đề Tiếng Việt & Ngắt Dòng Giữ Từ Ghép (Typography Constraints — CR-03):**
   - Video ngang 16:9: Tối đa 42 ký tự/dòng, tối đa 2 dòng.
   - Video dọc 9:16: Tối đa 26 ký tự/dòng (CPL - Characters Per Line) để không bị che khuất bởi giao diện nút bấm tương tác của TikTok/Reels.
   - Tốc độ đọc khống chế nghiêm ngặt: $17 - 20 \text{ CPS}$ (Characters Per Second).
   - **Xử lý khi CPS $> 20$:** Bẻ dòng bằng phím 'B' chỉ giải quyết tràn độ dài dòng CPL, không làm giảm tốc độ đọc CPS. Khi CPS $> 20$, áp dụng 2 cơ chế:
     - *Time Borrowing (Mượn thời gian):* Kéo dài thời lượng hiển thị của câu sub vào khoảng lặng âm thanh liền kề phía sau (tối đa chạm biên của câu nói kế tiếp) để đưa CPS về $\le 20$.
     - *Prompt-Level AI Condensation:* Chỉ thị prompt LLM cô đọng văn bản dịch đối với các câu có thời lượng $t < 1.5\text{s}$.
   - **Thuật toán Ngắt Dòng Giữ Từ Ghép (Smart Compound Line Wrapping):** Tích hợp từ điển từ ghép tiếng Việt; thuật toán ngắt dòng thông minh tuyệt đối không bẻ đôi giữa các từ ghép (ngăn chặn triệt để các lỗi ngắt cụm thô thiển như "bỏ / rơi", "con / rùa", "bộ / lọc", "bầy / đàn").
5. **Giới Hạn Bố Cục Video Dọc 9:16 Cố Định (9:16 Fixed Layout Presets — CR-06):**
   - Để tránh nguy cơ phình to phạm vi (Scope Creep) và không biến ứng dụng thành một NLE visual compositor phức tạp, NeuroClip giới hạn chính xác 2 Presets bố cục 9:16 trong MVP:
     - **Preset 1: Center-Crop 9:16:** Cắt chính diện trung tâm khung hình 16:9 gốc theo tỷ lệ 9:16.
     - **Preset 2: Two-stack Top-Bottom 50/50:** Nửa trên hiển thị khung hình Game, nửa dưới hiển thị Avatar/Webcam với vạch chia tỷ lệ cố định 50/50.
     - Toàn bộ các thao tác crop phức tạp khác (tự do kéo box crop, dynamic face tracking, multi-box canvas tự do) được ủy quyền xử lý tại CapCut PC Draft nhằm bảo đảm giao diện Lean Subtitle Studio luôn tinh gọn và mượt mà.
6. **Giới Hạn Tải Trọng API & Phân Đoạn Âm Thanh (Groq 25MB Audio Chunking — Ch2 Invariant):**
   - Endpoint Cloud Groq Whisper API có giới hạn tải cứng là $25\text{MB/request}$.
   - Module bóc băng âm thanh bắt buộc phải tự động cắt nhỏ (audio chunking) các tệp âm thanh stream dài thành các đoạn nhỏ $< 25\text{MB}$ (khoảng 10-15 phút/segment) kèm vùng gối đầu (overlap) $1.5\text{ giây}$, sau đó ghép nối mốc thời gian transcript chuẩn xác để ngăn chặn 100% lỗi `413 Payload Too Large`.
7. **Chiến Lược Phòng Thủ Kép Cho Xuất NLE (Dual Export Strategy — Ch2 Invariant):**
   - Nhằm phòng ngừa rủi ro ByteDance cập nhật phiên bản CapCut PC làm thay đổi cấu trúc `draft_content.json`, mọi thao tác xuất dự án CapCut Draft luôn tự động tạo kèm 2 file phụ đề độc lập `subtitle.ass` (chuẩn màu styles nhân vật) và `subtitle.srt` (UTF-8) vào ngay thư mục dự án đó.
   - Giao diện ứng dụng cung cấp hướng dẫn rõ ràng để người dùng có thể kéo thả trực tiếp file `.srt` vào CapCut nếu gặp sự cố phiên bản.

---
*Tài liệu PRD này là tài sản chính thức của dự án NeuroClip, được xác lập theo nguyên tắc Single Source of Truth (SSOT). Mọi thay đổi về phạm vi hoặc kiến trúc bắt buộc phải được cập nhật trực tiếp vào tài liệu này sau khi có sự phê duyệt từ Product Owner.*
