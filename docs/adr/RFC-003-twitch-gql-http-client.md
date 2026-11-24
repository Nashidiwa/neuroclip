# RFC-003: Chuyển Đổi HTTP Client Cho Twitch GQL Ingestion

**Mã tài liệu:** `docs/adr/RFC-003-twitch-gql-http-client.md`  
**Phiên bản Sản phẩm áp dụng:** `v1.0.0 (MVP)`  
**Phiên bản Tài liệu:** `rev.1 (2026-11-24)`  
**Trạng thái:** `[ĐÃ PHÊ DUYỆT - OPTION 1 cho v1.0.0 MVP]` *(PO chốt ngày 2026-11-24; Option 2 đưa vào `docs/ROADMAP.md` cho bản patch / v1.1+)*  
**Liên quan:** `TASK-M4-29B` (`AC-29B.3`), `src-tauri/src/ingestion/twitch_gql.rs`, `docs/ROADMAP.md`

---

## 1. Bối cảnh (Context)
- `src-tauri/src/ingestion/twitch_gql.rs` hiện thực hiện tải replay chat của Twitch VOD theo cơ chế phân trang con trỏ (cursor-pagination).
- Hiện tại, mỗi trang (page) chat được tải bằng cách spawn 1 tiến trình `curl.exe` độc lập (`execute_twitch_gql_curl`).
- Với một VOD dài 8 tiếng (50,000 – 100,000 tin nhắn chat, tương đương 500 – 1,000 trang GQL), hệ thống phải spawn 500 – 1,000 tiến trình con `curl.exe`, mỗi lần phải bắt tay lại TLS 1.3 từ đầu, gây lãng phí 100 – 200 giây độ trễ mạng tích lũy.
- Tuy nhiên, trong `src-tauri/Cargo.toml`, `reqwest` hiện được khai báo `default-features = false, features = ["json"]` (chưa bật TLS backend). Nếu chuyển sang `reqwest` async client, cần bổ sung feature TLS (`rustls-tls-native-roots` hoặc `native-tls`).

## 2. Các phương án kỹ thuật (Options)

### Option 1: Giữ `curl.exe` subprocess cho v1.0.0 MVP (`TASK-M4-29B`), cập nhật SHA256 query hash *(ĐÃ CHỌN CHO v1.0.0)*
- **Cơ chế**: Tiếp tục dùng `curl.exe` subprocess qua Windows Job Guard, cập nhật hằng số `TWITCH_COMMENTS_SHA256` sang hash đang hoạt động (`b70a3591ff0f4e0313d126c6a1502d79a1c02baebb288227c582044aa76adf6a`).
- **Ưu điểm**:
  - Giữ nguyên `Cargo.toml` / dependency tree ổn định ngay trước chặng E2E (`M4-30`), Stress Test (`M4-31`) và đóng gói (`M4-32`); 0 bytes tăng dung lượng binary.
  - Giữ nguyên cơ chế cô lập tiến trình con bằng Windows Job Objects.
  - Thỏa mãn trọn vẹn tiêu chí `AC-29B.3` ("đánh giá xong phương án reqwest").
- **Nhược điểm**: Thời gian tải chat các VOD rất dài chậm hơn so với HTTP Keep-Alive.

### Option 2: Nâng cấp `Cargo.toml` kích hoạt `reqwest` TLS và dùng Dual-Client *(DỰ KIẾN CHO BẢN PATCH / v1.1+)*
- **Cơ chế**: Thêm `rustls-tls-native-roots` (hoặc `native-tls`) vào `reqwest` trong `Cargo.toml`. Cung cấp `execute_twitch_gql_reqwest` với HTTP keep-alive connection pool, đồng thời giữ fallback `curl.exe` (tương tự pattern trong `sidecar/updater.rs`).
- **Ưu điểm**:
  - Tốc độ tải chat nhanh gấp 5–10 lần trên các VOD lớn nhờ tái sử dụng kết nối TLS.
  - Tự chủ, giảm phụ thuộc vào `curl.exe` trên hệ điều hành.
- **Nhược điểm**:
  - Tăng kích thước file thực thi `neuroclip.exe` thêm ~1.5–2.5 MB.
  - Thay đổi cây dependency và cần test tương thích TLS/anti-bot riêng cho bản phát hành kế tiếp.

## 3. Quyết định chính thức của PO (2026-11-24)
- **Phê duyệt Option 1 cho `v1.0.0 (MVP)`**: Hoàn tất `TASK-M4-29B` với `curl.exe` + Windows Job Guard + `TWITCH_COMMENTS_SHA256` mới để đảm bảo v1.0.0 MVP ổn định, về đích an toàn.
- **Ghi nhận Option 2 vào `docs/ROADMAP.md`**: Lên kế hoạch triển khai nâng cấp `reqwest` + TLS keep-alive cho `twitch_gql.rs` trong bản patch tối ưu hoặc `v1.1.0` sau khi `v1.0.0` đã hoàn chỉnh và kiểm chứng thực tế.
