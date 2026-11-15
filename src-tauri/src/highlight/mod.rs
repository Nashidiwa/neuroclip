//! NeuroClip Multimodal Highlight Detection Subsystem
//!
//! Complies with `docs/CONVENTIONS.md` (zero-panic invariant, Result<T, AppError>),
//! `docs/ARCHITECTURE.md` §4.3 & §5.3, and `docs/TASK_BOARD.md` TASK-M4-09.

pub mod bucket;
pub mod zscore;

pub use bucket::{
    bin_chat_events, bin_chat_events_default, bin_chat_events_from_file, bucket_chat_events,
    ChatBucket, DEFAULT_BUCKET_WIDTH_SEC, MAX_ALLOWED_BUCKETS, MAX_CHAT_TIMESTAMP_SEC,
    SPAM_BOT_MAX_MESSAGES_PER_AUTHOR,
};

pub use zscore::{
    calculate_adjusted_zscore, compute_chat_velocity_series,
    compute_chat_velocity_series_default, compute_chat_velocity_series_from_events,
    compute_chat_velocity_series_from_file, compute_zscore, ChatVelocityPoint,
    DEFAULT_BASELINE_WINDOW_SEC, MESSAGE_FLOOR_THRESHOLD, SMOOTHING_EPSILON,
};
