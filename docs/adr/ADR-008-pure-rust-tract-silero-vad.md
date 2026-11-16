# ADR-008: Khung Nhận Diện Giọng Nói & Lọc Khoảng Lặng Thuần Rust Qua tract-onnx và Silero VAD Đồ Thị Tĩnh 16kHz (Pure Rust Voice Activity Detection via tract-onnx & Silero VAD Static 16kHz Graph)

## Trạng thái
**ĐÃ ĐƯỢC CHẤP THUẬN (ACCEPTED)** — Phê duyệt bởi Product Owner ngày 2026-11-16 (Căn cứ quyết định phê duyệt RFC-002: Lựa chọn Option 2).

## Bối cảnh
Trong chuỗi xử lý AI của NeuroClip (`TASK-M4-14`), module phát hiện giọng nói (Voice Activity Detection - VAD) đóng vai trò là tầng cửa ngõ tiền xử lý âm thanh then chốt trước khi chuyển giao dữ liệu cho máy khách Groq Whisper Cloud API (`TASK-M4-15`) và động cơ dịch thuật Vietsub (`TASK-M4-17`).

Yêu cầu kỹ thuật và nghiệp vụ đối với module này bao gồm:
1. **Lọc sạch khoảng lặng và nhạc nền (`AC-14.2` & `PRD.md`):** Phải loại bỏ triệt để $\ge 90\%$ các phân đoạn im lặng và nhạc nền game không lời (Background Music - BGM) kéo dài $> 1.5\text{s}$, qua đó cắt giảm tối thiểu $30\%$ dung lượng payload gửi lên Cloud STT. Nếu gửi âm thanh chỉ chứa nhạc nền lên Whisper, mô hình sẽ gặp lỗi ảo giác âm thanh (*hallucination*) hoặc lặp từ vô tận (*infinite looping*), làm tăng chi phí API và phá hỏng chất lượng phụ đề.
2. **Hiệu năng thời gian thực (`AC-14.1`):** Bộ lọc VAD phải xử lý 1 phút âm thanh trong vòng $\le 1.5\text{s}$ trên CPU máy cá nhân của người dùng.
3. **Cắt chunk tại điểm nghỉ tự nhiên (`AC-14.3`):** Xác định các điểm ngắt tự nhiên (Natural Pause Points) để phân chia file audio thành các khối $< 20\text{MB}$ khi gửi lên Groq Cloud API, tuyệt đối không cắt ngang một từ đang phát âm.

### Xung đột kỹ thuật ngầm và dữ liệu thực nghiệm đối chứng từ RFC-002
Trong quá trình triển khai thực tế, hệ thống chịu sự chi phối của các ràng buộc đối nghịch:
- **Phương án Heuristic DSP thuần Rust (Option 1):** Mặc dù đạt tốc độ tính toán siêu tốc (~2ms/phút) và không tốn dung lượng bộ cài (0MB, 0 DLL), thực nghiệm đối chứng trên tín hiệu thực tế cho thấy thuật toán DSP Heuristic dựa trên RMS/ZCR/Lowpass thất bại hoàn toàn trước nhạc nền game (tỷ lệ lọc BGM đạt **$0\%$**, nốt đơn A4 440Hz và hợp âm C Major bị nhận diện nhầm thành giọng nói $100\%$). Do đó, Option 1 vi phạm trực tiếp tiêu chí chất lượng của `AC-14.2`.
- **Phương án Official ONNX Runtime C++ (`ort` + `onnxruntime.dll` - Option 3):** Đạt độ chính xác lọc BGM cao ($\ge 95\%$), nhưng file nhị phân `onnxruntime.dll` nặng tới 18MB–24MB, khi nén vào bộ cài NSIS chiếm thêm 10MB–15MB, đẩy kích thước installer của NeuroClip lên **28MB – 35MB (vi phạm nghiêm trọng cam kết trần 15MB – 25MB tại `ADR-002`)**. Hơn nữa, `onnxruntime.dll` phụ thuộc vào `vcruntime140.dll`, gây nguy cơ crash ứng dụng khi khởi động (`0x8007007E - Module Not Found`) trên các máy tính Windows chưa cài đặt Microsoft Visual C++ Redistributable.

## Quyết định Kiến trúc
NeuroClip chính thức áp dụng **Chiến lược Phòng Vệ Kép (Defensive Dual-Engine Strategy)** dựa trên engine suy luận nơ-ron thuần Rust:

### 1. Động Cơ VAD Chính (Primary VAD Engine: `tract-onnx` + Silero VAD Static 16kHz Graph)
- **Engine suy luận thuần Rust bản địa:** Tích hợp crate `tract-onnx` (thư viện suy luận mạng nơ-ron chất lượng cao do Sonos phát triển) biên dịch tĩnh trực tiếp vào nhị phân Rust backend của ứng dụng Tauri v2. Không sử dụng và không đóng gói bất kỳ file `.dll` C++ bên ngoài nào.
- **Mô hình Silero VAD đồ thị tĩnh 16kHz (`silero_vad_16k.onnx`):**
  - Sử dụng mạng học sâu Silero VAD v5 (kiến trúc STFT kết hợp Convolutional và Recurrent LSTM/GRU) đã được tối ưu hóa và đóng băng đồ thị cố định tại tần số lấy mẫu $16,000\text{Hz}$.
  - Kích thước tensor đầu vào được cố định ở `[1, 576]` mẫu (gồm 64 mẫu ngữ cảnh + 512 mẫu khung $32\text{ms}$ tại 16kHz) cùng tensor trạng thái ẩn `[2, 1, 128]` (`state` / `stateN`), đồng thời vẫn tương thích đường dẫn v4 (`[1, 512]`, `h`/`c` `[2, 1, 64]`).
  - Loại bỏ hoàn toàn các toán tử điều khiển rẽ nhánh động (toán tử `If` đa tần số 8k/16k trong file gốc), giúp mô hình tương thích 100% với `tract-onnx` mà không gặp lỗi unsupported operators.
  - Do module `audio_prep.rs` của NeuroClip đã chuẩn hóa toàn bộ luồng âm thanh đầu vào về $16\text{kHz}$ đơn kênh (mono), việc đóng băng đồ thị tại 16kHz là hoàn toàn tự nhiên và tối ưu.
- **Tuân thủ 100% ngân sách bộ cài và tài nguyên của `ADR-002`:**
  - File mô hình tĩnh `silero_vad_16k.onnx` chỉ nặng $1,279,182$ bytes ($\approx 1.28\text{MB}$ thô).
  - Mã máy bổ sung từ `tract-onnx` chỉ tăng thêm $\approx 1.0\text{MB}$ sau nén.
  - Tổng mức tăng ròng của bộ cài chỉ $\approx 2.2\text{MB}$, giữ kích thước installer của NeuroClip ở mức **$16.7\text{MB}$**, nằm trọn vẹn trong ngân sách cam kết **15MB – 25MB** của `ADR-002`.
  - Mức chiếm dụng RAM khi chạy suy luận chỉ $5\text{MB} - 10\text{MB}$, RAM tĩnh ứng dụng giữ ở mức **$50\text{MB} - 60\text{MB}$**, thỏa mãn trọn vẹn trần **40MB – 80MB** của `ADR-002`.
- **Miễn nhiễm hoàn toàn với lỗi môi trường Windows:** File thực thi Rust độc lập, 0 phụ thuộc MSVC C++ Redistributable, loại bỏ triệt để rủi ro lỗi `0x8007007E`.

### 2. Động Cơ Dự Phòng An Toàn (Fallback / Safety-Net Engine: Pure Rust DSP)
- Thuật toán Heuristic DSP hiện tại trong `src-tauri/src/stt/vad.rs` được lưu giữ và chuyển đổi thành hàm dự phòng: `compute_frame_speech_prob_fallback`.
- Trong trường hợp cực hiếm khi nạp file mô hình tĩnh ONNX thất bại (do lỗi phân mảnh ổ cứng, lỗi I/O file hoặc tập lệnh CPU không tương thích), hệ thống tự động giáng cấp (*graceful degradation*) chuyển sang chế độ DSP Heuristic.
- Toàn bộ quá trình fallback ghi nhận nhật ký cảnh báo có cấu trúc (`tracing::warn!`), tuyệt đối không panic làm gián đoạn luồng làm việc của người dùng, tuân thủ nguyên tắc **Panic-Free Production Backend** tại `docs/CONVENTIONS.md`.

### 3. Quy Chuẩn Bất Đồng Bộ & An Toàn Bộ Nhớ (Tokio Concurrency Invariant)
- Tuân thủ nghiêm ngặt Mục 2.2 của `docs/CONVENTIONS.md`, toàn bộ tác vụ tính toán VAD nặng trên mảng mẫu âm thanh float32 **bắt buộc phải chạy trong `tokio::task::spawn_blocking`**, bảo đảm không phong tỏa luồng I/O async của Tokio runtime.
- Bảo toàn trọn vẹn cấu trúc giao kèo dữ liệu `SpeechSegment { start_ms: u64, end_ms: u64 }` và chuyển đổi liền mạch với `SpeechInterval` của `vad_bridge.rs`.
- Thực hiện cắt nhỏ audio thành các chunks $< 20\text{MB}$ tại các điểm nghỉ tự nhiên (Natural Pause Splitting) với độ đệm an toàn, không bao giờ cắt đứt từ thoại (`AC-14.3`).

## Hệ quả & Đánh đổi

### Tích cực (Benefits)
1. **Chất lượng âm học vượt trội (`AC-14.2`):** Đạt tỷ lệ lọc sạch khoảng lặng và nhạc nền game BGM từ $95\% - 98\%$. Cắt giảm từ $35\% - 55\%$ dung lượng payload gửi lên Cloud STT trên các livestream thực tế của Neuro-sama, triệt tiêu hiện tượng lặp từ vô tận của Whisper.
2. **Tốc độ CPU siêu tốc (`AC-14.1`):** Thời gian suy luận 1 phút âm thanh trên CPU chỉ mất **$40\text{ms} - 65\text{ms}$** (nhanh hơn thời gian thực ~1000 lần, vượt xa chỉ tiêu trần $\le 1500\text{ms}$).
3. **Bảo toàn định vị Desktop App siêu nhẹ (`ADR-002`):** Installer duy trì ở mức $\approx 16.7\text{MB}$ (ngân sách 15–25MB), RAM tĩnh duy trì $\approx 52\text{MB}$ (ngân sách 40–80MB).
4. **Trải nghiệm phân phối hoàn hảo trên Windows:** Cài đặt là chạy ngay (Out-of-the-box), không đòi hỏi người dùng cài thêm Microsoft Visual C++ Redistributable, loại bỏ 100% khiếu nại về lỗi thiếu DLL.
5. **Khả năng tự phục hồi bền bỉ:** Cơ chế fallback DSP đảm bảo tính liên tục của ứng dụng trong mọi tình huống ngoại lệ.

### Thách thức & Biện pháp Giảm thiểu (Trade-offs & Mitigations)
- **Thách thức:** Đòi hỏi file mô hình Silero VAD phải được đóng băng cố định ở đồ thị 16kHz (`silero_vad_16k.onnx`) để tương thích với `tract-onnx` (loại bỏ toán tử `If`).
  - *Biện pháp giảm thiểu:* Thực hiện kiểm thử tích hợp tự động (`stt_vad_test.rs`) kiểm tra tính toàn vẹn và độ chính xác của file mô hình ngay trong quy trình build và test của CI/CD.
- **Thách thức:** Tốc độ suy luận của `tract-onnx` (~50ms/phút) chậm hơn một chút so với ONNX Runtime C++ gia tốc AVX-512 (~20ms/phút).
  - *Biện pháp giảm thiểu:* Mức chênh lệch 30ms trên mỗi 60s âm thanh là hoàn toàn không thể nhận biết bởi giác quan người dùng và vẫn nhanh hơn 30 lần so với giới hạn trần 1.500ms của `AC-14.1`.
