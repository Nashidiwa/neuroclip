# LỘ TRÌNH PHÁT TRIỂN & TỐI ƯU HẬU v1.0.0 (POST-MVP ROADMAP)

**Mã tài liệu:** `docs/ROADMAP.md`  
**Sản phẩm áp dụng:** NeuroClip Desktop App *(Hiện tại: `v1.0.0 MVP` — In Development)*  
**Phiên bản Tài liệu:** `rev.2 (2026-11-25)`  
**Nguyên tắc:** Ghi nhận tập trung các hạng mục tối ưu hiệu năng, nợ kỹ thuật có chủ đích và tính năng dự kiến cho các bản patch (`v1.0.x`), `v1.1.0` và `v2.0.0` sau khi hoàn thành `v1.0.0 MVP`.

> **Lưu ý về tách biệt phiên bản:** Các mốc `v1.0.x`, `v1.1.0`, `v2.0.0` trong tài liệu này là **Phiên bản Sản phẩm (Product Version)** dự kiến phát hành, hoàn toàn tách biệt với **Phiên bản Tài liệu (`Doc Revision`)**.

---

## 1. Hoàn tất Chặng Cuối v1.0.0 MVP (Đang triển khai)
- `TASK-M4-30` — ✅ Đã hoàn thành: Bộ kiểm thử tự động tích hợp toàn tuyến (E2E Synthetic Pipeline Harness).
- `TASK-M4-31` — ✅ Đã hoàn thành: Đánh giá hiệu năng & kiểm thử tải trên máy cấu hình yếu (Low-End PC Stress Benchmark: Peak Heap `82.21 MB`, OS Peak WS `98.18 MB`, `+0 bytes` leak, `max_tick_interval 17.38 ms`).
- `TASK-M4-32`: Pipeline đóng gói bộ cài đặt Windows NSIS & MSI + `.github/workflows/ci.yml` + WebView2 CSP Release Gate + xuất `SHA256SUMS.txt` (`v1.0.0 MVP` unsigned; chuẩn bị nền cho SignPath).
- `TASK-M4-33` *(Dự kiến sơ bộ — review & chốt lại với PO trước khi làm)*: Đại cập nhật cấu trúc tài liệu (`docs/` 3 tầng, đồng bộ phiên bản sản phẩm vs tài liệu, viết lại `AGENTS.md` và cập nhật `README.md` kèm hướng dẫn SmartScreen + Code Signing Policy + Privacy Policy).

---

## 2. Dự kiến Bản Patch / v1.1.0 (Performance, Hardening & Process)

| Hạng mục | Nguồn / Tham chiếu | Mô tả & Mục tiêu | Độ ưu tiên |
|:---|:---|:---|:---:|
| **Đăng ký Windows Code Signing qua SignPath Foundation (`AGPL-3.0`)** | `TASK-M4-32` / PO `2026-11-25` | Sau khi có workflow build công khai và bản release `v1.0.0` đầu tiên, nộp hồ sơ SignPath Foundation cho dự án open-source `AGPL-3.0`; tích hợp bước ký cho binary NeuroClip (không ký binary bên thứ ba `ffmpeg.exe`/`yt-dlp.exe`) cùng Code Signing Policy & Privacy Policy trong `README.md`. | Cao (Distribution) |
| **Nâng cấp `twitch_gql.rs` sang `reqwest` Async HTTP/2 Keep-Alive** | [`RFC-003`](adr/RFC-003-twitch-gql-http-client.md) (Option 2) | Bật feature TLS (`rustls-tls-native-roots` hoặc `native-tls`) cho `reqwest` trong `Cargo.toml`, triển khai Dual-Client (`reqwest` connection pool + `curl.exe` fallback) để tăng tốc tải Twitch replay chat VOD 8h gấp 5–10× và giảm spawn process. | Cao (Performance) |
| **Thu thập các điểm tối ưu từ Low-End Benchmark (`M4-31`)** | `TASK-M4-31` ✅ Completed | Toàn bộ chỉ tiêu `AC-31.1..3` đều đạt vượt mức (`<= 98.18 MB` vs `500 MB`, `+0 bytes` leak, `17.38 ms` event-loop max interval); tiếp tục theo dõi jitter timing ở test có sẵn `test_ed_15_2_processing_speed_sla` (`groq_client_test`, từng chạm `101.45 ms` vs `< 100 ms` khi chạy song song nặng). | Theo dõi |
| **Chuẩn hóa quy trình Agent Orchestrator thay `/teamwork`** | `TASK-M4-33` / `AGENTS.md` | Xem xét chuyển từ workflow `/teamwork` sang mô hình **orchestrator** tinh gọn hơn cho các chu kỳ phát triển patch và `v1.1+` *(chốt chi tiết cùng PO ở `TASK-M4-33`)*. | Trung bình |

---

## 3. Dự kiến v2.0.0+ (Major Features — Deferred khỏi v1.0.0 MVP)

| Hạng mục | Nguồn / Tham chiếu | Mô tả & Lý do hoãn |
|:---|:---|:---|
| **Render Burn-in FFmpeg MP4 Hardsub** | [`PRD.md`](PRD.md) (Scope v1.3.0) | Cho phép xuất trực tiếp file `.mp4` đã đốt cứng phụ đề `.ass` (kèm style màu nhân vật) cho người dùng không mở CapCut/Premiere. Hoãn sang v2.0 để v1.0.0 tập trung vào tốc độ lossless (`-c copy`) và NLE workflow. |

---
*Tài liệu được khởi tạo ngày 2026-11-24 theo chỉ đạo của PO để tách bạch kế hoạch tương lai khỏi SSOT hiện hành của v1.0.0 MVP.*
