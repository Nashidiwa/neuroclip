//! Unified error handling infrastructure and IPC serialization protocol for NeuroClip.
//!
//! Every error type in the application is represented under [`AppError`], which implements
//! [`serde::Serialize`] to format into the standardized [`IpcErrorResponse`] envelope.

pub mod codes;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Standardized IPC error response envelope sent across the Tauri bridge to TypeScript.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IpcErrorResponse {
    /// Canonical error code (e.g. "ERR_INGEST_DISK_FULL").
    pub code: String,
    /// User-friendly localized message in Vietnamese.
    pub message: String,
    /// Optional technical details or debug information.
    pub details: Option<String>,
    /// Indicates whether the error can be resolved via user action, retry, or fallback.
    pub recoverable: bool,
}

/// Root application error enum encapsulating all subsystem failure modes.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum AppError {
    #[error("Lỗi phần cứng: {0}")]
    Hardware(#[from] HardwareError),

    #[error("Lỗi thu nạp VOD: {0}")]
    Ingestion(#[from] IngestError),

    #[error("Lỗi tính toán highlight: {0}")]
    Highlight(#[from] HighlightError),

    #[error("Lỗi bóc băng STT: {0}")]
    Stt(#[from] SttError),

    #[error("Lỗi dịch thuật: {0}")]
    Translation(#[from] TranslationError),

    #[error("Lỗi xuất bản NLE: {0}")]
    Export(#[from] ExportError),

    #[error("Lỗi công cụ phụ trợ Sidecar: {0}")]
    Sidecar(#[from] SidecarError),

    #[error("Lỗi lưu trữ ổ đĩa: {0}")]
    Storage(#[from] StorageError),

    #[error("Lỗi cấu hình cài đặt: {0}")]
    Config(#[from] ConfigError),

    #[error("Tác vụ đã bị người dùng hủy")]
    TaskCancelled,

    #[error("Lỗi hệ thống không xác định: {0}")]
    Internal(String),
}

/// Ingestion subsystem errors (VOD download, streaming, chat extraction).
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum IngestError {
    #[error("Dung lượng ổ đĩa không đủ (cần tối thiểu {required_mb}MB, hiện có {available_mb}MB)")]
    DiskSpaceInsufficient { required_mb: u64, available_mb: u64 },

    #[error("YouTube/Twitch kích hoạt cơ chế chặn bot (HTTP 403 / 429): {0}")]
    BotBlocked(String),

    #[error("Twitch Chat Replay không tồn tại hoặc đã bị xóa")]
    ChatReplayUnavailable,

    #[error("Mất kết nối mạng khi tải stream sau 5 lần thử lại")]
    NetworkStreamDropped,

    #[error("URL không hợp lệ hoặc nền tảng không được hỗ trợ: {0}")]
    InvalidUrl(String),

    #[error("yt-dlp thất bại khi trích xuất stream URL: {0}")]
    ExtractionFailed(String),

    #[error("Luồng đang phát trực tiếp không được hỗ trợ trong phiên bản này (chỉ hỗ trợ VOD đã kết thúc)")]
    LiveStreamUnsupported,
}

/// Speech-to-Text subsystem errors (Groq Whisper Cloud & Faster-Whisper Local).
#[derive(Debug, Error, Clone, PartialEq)]
pub enum SttError {
    #[error("Vượt giới hạn dung lượng Groq API (> 25MB, hiện tại: {size_mb:.1}MB)")]
    PayloadTooLarge { size_mb: f32 },

    #[error("Groq API Rate Limit hoặc hết quota (HTTP 429): {0}")]
    RateLimitExceeded(String),

    #[error("Groq API Key không hợp lệ hoặc chưa được cấu hình")]
    InvalidApiKey,

    #[error("Bộ nhớ VRAM không đủ để chạy Faster-Whisper ({available_mb}MB < 2500MB)")]
    InsufficientVram { available_mb: u32 },

    #[error("Lỗi mô hình VAD hoặc xử lý âm thanh: {0}")]
    VadProcessingFailed(String),
}

/// Translation and contextual diarization errors (Google Gemini 1.5 Flash).
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TranslationError {
    #[error("Gemini API Key không hợp lệ hoặc chưa được cấu hình")]
    InvalidApiKey,

    #[error("Bộ lọc an toàn Gemini kích hoạt (finishReason: SAFETY): {0}")]
    SafetyFilterTriggered(String),

    #[error("Gemini API Rate Limit hoặc lỗi máy chủ (HTTP {status}): {message}")]
    ApiRequestFailed { status: u16, message: String },
}

/// NLE timeline and subtitle project export errors (CapCut, ASS, SRT).
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ExportError {
    #[error("Không tìm thấy đường dẫn thư mục CapCut Draft trên máy")]
    CapCutDirectoryNotFound,

    #[error("Lỗi ghi file dự án NLE ({path}): {details}")]
    FileWriteFailed { path: String, details: String },

    #[error("Layout xuất bản không hợp lệ: {0}")]
    InvalidLayout(String),
}

/// Multimodal highlight scoring and clustering errors.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum HighlightError {
    #[error("Lỗi phân tích cú pháp sự kiện chat: {0}")]
    ChatParsingFailed(String),

    #[error("Lỗi phân tích năng lượng âm thanh: {0}")]
    AudioAnalysisFailed(String),

    #[error("Không tìm thấy khoảnh khắc highlight nào thỏa mãn tiêu chí")]
    NoHighlightsFound,

    #[error("Worker tính toán highlight gặp sự cố: {0}")]
    WorkerPanicked(String),
}

/// Managed external subprocess errors (FFmpeg, yt-dlp, Python worker).
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SidecarError {
    #[error("Không tìm thấy công cụ phụ trợ '{name}' tại '{path}'")]
    BinaryNotFound { name: String, path: String },

    #[error("Tiến trình con '{binary}' thất bại với mã thoát {exit_code:?}: {details}")]
    ExecutionFailed {
        binary: String,
        exit_code: Option<i32>,
        details: String,
    },

    #[error("Cập nhật công cụ phụ trợ '{binary}' thất bại: {reason}")]
    UpdateFailed { binary: String, reason: String },

    #[error("Tiến trình con bị hủy do ứng dụng tắt hoặc người dùng dừng")]
    ProcessKilled,
}

/// Hardware probe and capability evaluation errors.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum HardwareError {
    #[error("Không thể đánh giá thông số phần cứng: {0}")]
    ProbeFailed(String),

    #[error("Không tìm thấy GPU tương thích hoặc không hỗ trợ CUDA: {0}")]
    UnsupportedGpu(String),

    #[error("Dung lượng RAM hệ thống không đủ (cần tối thiểu {required_mb}MB, hiện có {total_mb}MB)")]
    InsufficientMemory { total_mb: u64, required_mb: u64 },
}

/// Disk pre-flight checks and cache storage errors.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum StorageError {
    #[error("Không thể kiểm tra dung lượng ổ đĩa ({path}): {message}")]
    DiskCheckFailed { path: String, message: String },

    #[error("Không thể tạo thư mục lưu trữ ({path}): {details}")]
    DirectoryCreationFailed { path: String, details: String },

    #[error("Dọn dẹp bộ nhớ đệm thất bại: {0}")]
    CacheCleanupFailed(String),

    #[error("Không tìm thấy tệp tin: {0}")]
    FileNotFound(String),
}

/// Configuration settings persistence errors.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ConfigError {
    #[error("Không tìm thấy tệp cấu hình: {0}")]
    NotFound(String),

    #[error("Không thể đọc hoặc giải mã cấu hình: {0}")]
    InvalidFormat(String),

    #[error("Không thể lưu tệp cấu hình ({path}): {details}")]
    SaveFailed { path: String, details: String },

    #[error("Giá trị cấu hình không hợp lệ cho khóa '{key}': {message}")]
    InvalidValue { key: String, message: String },
}

impl AppError {
    /// Return the canonical uppercase error code string constant.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            AppError::Ingestion(err) => match err {
                IngestError::DiskSpaceInsufficient { .. } => codes::ERR_INGEST_DISK_FULL,
                IngestError::BotBlocked(_) => codes::ERR_INGEST_BOT_BLOCKED,
                IngestError::ChatReplayUnavailable => codes::ERR_INGEST_CHAT_UNAVAILABLE,
                IngestError::NetworkStreamDropped => codes::ERR_INGEST_NETWORK_TIMEOUT,
                IngestError::InvalidUrl(_) => codes::ERR_INGEST_INVALID_URL,
                IngestError::ExtractionFailed(_) => codes::ERR_INGEST_EXTRACTION_FAILED,
                IngestError::LiveStreamUnsupported => codes::ERR_INGEST_LIVE_STREAM_UNSUPPORTED,
            },
            AppError::Stt(err) => match err {
                SttError::RateLimitExceeded(_) => codes::ERR_STT_RATE_LIMIT,
                SttError::PayloadTooLarge { .. } => codes::ERR_STT_PAYLOAD_TOO_LARGE,
                SttError::InsufficientVram { .. } => codes::ERR_STT_VRAM_INSUFFICIENT,
                SttError::InvalidApiKey => codes::ERR_STT_INVALID_API_KEY,
                SttError::VadProcessingFailed(_) => codes::ERR_STT_VAD_FAILED,
            },
            AppError::Translation(err) => match err {
                TranslationError::SafetyFilterTriggered(_) => codes::ERR_TRANSLATE_SAFETY_FILTER,
                TranslationError::InvalidApiKey => codes::ERR_TRANSLATE_INVALID_API_KEY,
                TranslationError::ApiRequestFailed { .. } => codes::ERR_TRANSLATE_API_FAILED,
            },
            AppError::Export(err) => match err {
                ExportError::CapCutDirectoryNotFound => codes::ERR_EXPORT_CAPCUT_NOT_FOUND,
                ExportError::FileWriteFailed { .. } => codes::ERR_EXPORT_FILE_WRITE_FAILED,
                ExportError::InvalidLayout(_) => codes::ERR_EXPORT_INVALID_LAYOUT,
            },
            AppError::Highlight(err) => match err {
                HighlightError::ChatParsingFailed(_) => codes::ERR_HIGHLIGHT_CHAT_PARSING_FAILED,
                HighlightError::AudioAnalysisFailed(_) => codes::ERR_HIGHLIGHT_AUDIO_ANALYSIS_FAILED,
                HighlightError::NoHighlightsFound => codes::ERR_HIGHLIGHT_NO_HIGHLIGHTS,
                HighlightError::WorkerPanicked(_) => codes::ERR_HIGHLIGHT_WORKER_PANICKED,
            },
            AppError::Sidecar(err) => match err {
                SidecarError::BinaryNotFound { .. } => codes::ERR_SIDECAR_BINARY_NOT_FOUND,
                SidecarError::ExecutionFailed { .. } => codes::ERR_SIDECAR_EXECUTION_FAILED,
                SidecarError::UpdateFailed { .. } => codes::ERR_SIDECAR_UPDATE_FAILED,
                SidecarError::ProcessKilled => codes::ERR_SIDECAR_PROCESS_KILLED,
            },
            AppError::Hardware(err) => match err {
                HardwareError::ProbeFailed(_) => codes::ERR_HARDWARE_PROBE_FAILED,
                HardwareError::UnsupportedGpu(_) => codes::ERR_HARDWARE_UNSUPPORTED_GPU,
                HardwareError::InsufficientMemory { .. } => codes::ERR_HARDWARE_INSUFFICIENT_MEMORY,
            },
            AppError::Storage(err) => match err {
                StorageError::DiskCheckFailed { .. } => codes::ERR_STORAGE_DISK_CHECK_FAILED,
                StorageError::DirectoryCreationFailed { .. } => {
                    codes::ERR_STORAGE_DIRECTORY_CREATION_FAILED
                }
                StorageError::CacheCleanupFailed(_) => codes::ERR_STORAGE_CACHE_CLEANUP_FAILED,
                StorageError::FileNotFound(_) => codes::ERR_STORAGE_FILE_NOT_FOUND,
            },
            AppError::Config(err) => match err {
                ConfigError::NotFound(_) => codes::ERR_CONFIG_NOT_FOUND,
                ConfigError::InvalidFormat(_) => codes::ERR_CONFIG_INVALID_FORMAT,
                ConfigError::SaveFailed { .. } => codes::ERR_CONFIG_SAVE_FAILED,
                ConfigError::InvalidValue { .. } => codes::ERR_CONFIG_INVALID_VALUE,
            },
            AppError::TaskCancelled => codes::ERR_TASK_CANCELLED,
            AppError::Internal(_) => codes::ERR_INTERNAL_ERROR,
        }
    }

    /// Return whether this error can be resolved via user action, retry, or fallback.
    #[must_use]
    pub fn is_recoverable(&self) -> bool {
        match self {
            AppError::Ingestion(err) => match err {
                IngestError::DiskSpaceInsufficient { .. }
                | IngestError::BotBlocked(_)
                | IngestError::ChatReplayUnavailable
                | IngestError::NetworkStreamDropped
                | IngestError::ExtractionFailed(_) => true,
                IngestError::InvalidUrl(_) | IngestError::LiveStreamUnsupported => false,
            },
            AppError::Stt(err) => match err {
                SttError::RateLimitExceeded(_)
                | SttError::InsufficientVram { .. }
                | SttError::VadProcessingFailed(_) => true,
                SttError::PayloadTooLarge { .. } | SttError::InvalidApiKey => false,
            },
            AppError::Translation(err) => match err {
                TranslationError::SafetyFilterTriggered(_)
                | TranslationError::ApiRequestFailed { .. } => true,
                TranslationError::InvalidApiKey => false,
            },
            AppError::Export(err) => match err {
                ExportError::CapCutDirectoryNotFound | ExportError::FileWriteFailed { .. } => true,
                ExportError::InvalidLayout(_) => false,
            },
            AppError::Highlight(err) => match err {
                HighlightError::ChatParsingFailed(_) | HighlightError::AudioAnalysisFailed(_) => {
                    true
                }
                HighlightError::NoHighlightsFound | HighlightError::WorkerPanicked(_) => false,
            },
            AppError::Sidecar(err) => match err {
                SidecarError::BinaryNotFound { .. }
                | SidecarError::ExecutionFailed { .. }
                | SidecarError::UpdateFailed { .. } => true,
                SidecarError::ProcessKilled => false,
            },
            AppError::Hardware(err) => match err {
                HardwareError::ProbeFailed(_) => true,
                HardwareError::UnsupportedGpu(_) | HardwareError::InsufficientMemory { .. } => {
                    false
                }
            },
            AppError::Storage(err) => match err {
                StorageError::DiskCheckFailed { .. }
                | StorageError::DirectoryCreationFailed { .. }
                | StorageError::CacheCleanupFailed(_) => true,
                StorageError::FileNotFound(_) => false,
            },
            AppError::Config(err) => match err {
                ConfigError::NotFound(_) | ConfigError::SaveFailed { .. } => true,
                ConfigError::InvalidFormat(_) | ConfigError::InvalidValue { .. } => false,
            },
            AppError::TaskCancelled | AppError::Internal(_) => false,
        }
    }

    /// Extract technical details or context if present.
    #[must_use]
    pub fn details(&self) -> Option<String> {
        match self {
            AppError::Ingestion(IngestError::BotBlocked(details))
            | AppError::Ingestion(IngestError::ExtractionFailed(details))
            | AppError::Ingestion(IngestError::InvalidUrl(details)) => Some(details.clone()),

            AppError::Stt(SttError::RateLimitExceeded(details))
            | AppError::Stt(SttError::VadProcessingFailed(details)) => Some(details.clone()),

            AppError::Translation(TranslationError::SafetyFilterTriggered(details)) => {
                Some(details.clone())
            }
            AppError::Translation(TranslationError::ApiRequestFailed { message, .. }) => {
                Some(message.clone())
            }

            AppError::Export(ExportError::FileWriteFailed { details, .. }) => {
                Some(details.clone())
            }

            AppError::Highlight(HighlightError::ChatParsingFailed(details))
            | AppError::Highlight(HighlightError::AudioAnalysisFailed(details))
            | AppError::Highlight(HighlightError::WorkerPanicked(details)) => Some(details.clone()),

            AppError::Sidecar(SidecarError::ExecutionFailed { details, .. }) => {
                Some(details.clone())
            }
            AppError::Sidecar(SidecarError::UpdateFailed { reason, .. }) => Some(reason.clone()),
            AppError::Sidecar(SidecarError::BinaryNotFound { path, .. }) => Some(path.clone()),

            AppError::Hardware(HardwareError::ProbeFailed(details))
            | AppError::Hardware(HardwareError::UnsupportedGpu(details)) => Some(details.clone()),

            AppError::Storage(StorageError::DiskCheckFailed { message, .. }) => {
                Some(message.clone())
            }
            AppError::Storage(StorageError::DirectoryCreationFailed { details, .. }) => {
                Some(details.clone())
            }

            AppError::Config(ConfigError::InvalidFormat(details))
            | AppError::Config(ConfigError::SaveFailed { details, .. }) => Some(details.clone()),

            AppError::Internal(details) => Some(details.clone()),

            _ => None,
        }
    }

    /// Convert this error into the standardized IPC error envelope.
    #[must_use]
    pub fn to_ipc_response(&self) -> IpcErrorResponse {
        IpcErrorResponse {
            code: self.code().to_string(),
            message: self.to_string(),
            details: self.details(),
            recoverable: self.is_recoverable(),
        }
    }
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_ipc_response().serialize(serializer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_subsystem_canonical_codes() {
        let ingest_err = AppError::Ingestion(IngestError::DiskSpaceInsufficient {
            required_mb: 5000,
            available_mb: 1500,
        });
        assert_eq!(ingest_err.code(), codes::ERR_INGEST_DISK_FULL);
        assert!(ingest_err.is_recoverable());

        let stt_err = AppError::Stt(SttError::PayloadTooLarge { size_mb: 28.5 });
        assert_eq!(stt_err.code(), codes::ERR_STT_PAYLOAD_TOO_LARGE);
        assert!(!stt_err.is_recoverable());

        let vram_err = AppError::Stt(SttError::InsufficientVram { available_mb: 2000 });
        assert_eq!(vram_err.code(), codes::ERR_STT_VRAM_INSUFFICIENT);
        assert!(vram_err.is_recoverable());

        let trans_err = AppError::Translation(TranslationError::SafetyFilterTriggered(
            "finishReason: SAFETY".to_string(),
        ));
        assert_eq!(trans_err.code(), codes::ERR_TRANSLATE_SAFETY_FILTER);
        assert!(trans_err.is_recoverable());

        let capcut_err = AppError::Export(ExportError::CapCutDirectoryNotFound);
        assert_eq!(capcut_err.code(), codes::ERR_EXPORT_CAPCUT_NOT_FOUND);
        assert!(capcut_err.is_recoverable());

        let chat_err = AppError::Ingestion(IngestError::ChatReplayUnavailable);
        assert_eq!(chat_err.code(), codes::ERR_INGEST_CHAT_UNAVAILABLE);
        assert!(chat_err.is_recoverable());

        let bot_err = AppError::Ingestion(IngestError::BotBlocked("HTTP 403".to_string()));
        assert_eq!(bot_err.code(), codes::ERR_INGEST_BOT_BLOCKED);
        assert!(bot_err.is_recoverable());
    }

    #[test]
    fn test_internal_error_unrecoverable() {
        let internal_err = AppError::Internal("Database connection lost".to_string());
        assert_eq!(internal_err.code(), codes::ERR_INTERNAL_ERROR);
        assert!(!internal_err.is_recoverable());

        let response = internal_err.to_ipc_response();
        assert_eq!(response.code, "ERR_INTERNAL_ERROR");
        assert!(!response.recoverable);
        assert_eq!(response.details, Some("Database connection lost".to_string()));
    }

    #[test]
    fn test_ipc_serialization_contract() -> Result<(), Box<dyn std::error::Error>> {
        let err = AppError::Ingestion(IngestError::DiskSpaceInsufficient {
            required_mb: 5000,
            available_mb: 1200,
        });

        let json = serde_json::to_string(&err)?;
        let parsed: IpcErrorResponse = serde_json::from_str(&json)?;

        assert_eq!(parsed.code, "ERR_INGEST_DISK_FULL");
        assert!(parsed.recoverable);
        assert!(parsed.message.contains("5000MB"));
        assert!(parsed.message.contains("1200MB"));
        assert_eq!(parsed.details, None);

        Ok(())
    }

    #[test]
    fn test_task_cancelled() {
        let cancelled = AppError::TaskCancelled;
        assert_eq!(cancelled.code(), codes::ERR_TASK_CANCELLED);
        assert!(!cancelled.is_recoverable());
        assert_eq!(cancelled.to_string(), "Tác vụ đã bị người dùng hủy");
    }
}
