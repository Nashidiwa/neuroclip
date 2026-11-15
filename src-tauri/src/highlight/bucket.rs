//! 5-Second Fixed Time Binning and In-Bucket Anti-Spam Deduplication.
//!
//! Complies with `docs/CONVENTIONS.md` (zero-panic invariant, Result<T, AppError>)
//! and `docs/TASK_BOARD.md` §TASK-M4-09.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, HighlightError, StorageError};
use crate::ingestion::ChatEvent;

/// Default bucket time window in seconds ($W = 5.0\text{s}$).
pub const DEFAULT_BUCKET_WIDTH_SEC: f64 = 5.0;

/// Maximum number of messages counted per author in a single 5-second bucket (AC-09.2, UT-09.3).
pub const SPAM_BOT_MAX_MESSAGES_PER_AUTHOR: u32 = 3;

/// Maximum allowable chat timestamp in seconds (~115.7 days = 10,000,000s).
/// Timestamps beyond this limit (or negative / non-finite) are rejected to prevent astronomical memory allocations.
pub const MAX_CHAT_TIMESTAMP_SEC: f64 = 10_000_000.0;

/// Maximum number of buckets allowed in memory (~115.7 days at 5s = 2,000,001 buckets <= 64MB).
pub const MAX_ALLOWED_BUCKETS: usize = 2_000_001;

/// Fixed-width time bucket representing aggregated chat activity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatBucket {
    /// Zero-based sequential bucket index ($k$).
    pub index: usize,
    /// Bucket start timestamp in seconds ($t_k = k \times W$).
    pub timestamp_sec: f64,
    /// Raw unconstrained message count in this time window.
    pub raw_count: u32,
    /// Effective message count after per-author spam bot deduplication ($X(t)$).
    pub effective_count: u32,
}

impl ChatBucket {
    /// Construct a new `ChatBucket`.
    #[must_use]
    pub const fn new(index: usize, timestamp_sec: f64, raw_count: u32, effective_count: u32) -> Self {
        Self {
            index,
            timestamp_sec,
            raw_count,
            effective_count,
        }
    }

    /// Accessor for effective message count after spam bot deduplication.
    #[must_use]
    pub const fn filtered_count(&self) -> u32 {
        self.effective_count
    }
}

/// Accumulator for a single bucket during event binning.
#[derive(Default)]
struct BucketAcc<'a> {
    raw_count: u32,
    author_counts: HashMap<&'a str, u32>,
}

/// Bin a slice of `ChatEvent`s into uniform fixed-width buckets with author rate limiting.
///
/// # Arguments
/// * `events` - Slice of chat events to bin.
/// * `bucket_width_sec` - Width of each bucket in seconds (defaults to 5.0).
/// * `vod_duration_sec` - Optional total VOD duration in seconds to extend the timeline.
///
/// # Errors
/// Returns `AppError::Highlight(HighlightError::ChatParsingFailed)` if parameters are invalid
/// or bucket allocation exceeds limits.
pub fn bin_chat_events(
    events: &[ChatEvent],
    bucket_width_sec: f64,
    vod_duration_sec: Option<f64>,
) -> Result<Vec<ChatBucket>, AppError> {
    if !bucket_width_sec.is_finite() || bucket_width_sec <= 0.0 || bucket_width_sec > 3600.0 {
        return Err(AppError::Highlight(HighlightError::ChatParsingFailed(format!(
            "Invalid bucket width: {bucket_width_sec}s (must be > 0.0 and <= 3600.0)"
        ))));
    }

    if let Some(dur) = vod_duration_sec {
        if !dur.is_finite() || dur < 0.0 || dur > MAX_CHAT_TIMESTAMP_SEC {
            return Err(AppError::Highlight(HighlightError::ChatParsingFailed(format!(
                "Invalid VOD duration: {dur}s"
            ))));
        }
    }

    // Pass 1: Filter and sanitize events, tracking maximum valid timestamp
    let mut valid_events: Vec<&ChatEvent> = Vec::with_capacity(events.len().min(100_000));
    let mut max_event_ts: f64 = 0.0;
    let mut has_valid_events = false;

    for event in events {
        let ts = event.timestamp_sec;
        if ts.is_finite() && (0.0..=MAX_CHAT_TIMESTAMP_SEC).contains(&ts) {
            valid_events.push(event);
            if !has_valid_events || ts > max_event_ts {
                max_event_ts = ts;
                has_valid_events = true;
            }
        }
    }

    // Determine total buckets needed
    let total_buckets = if has_valid_events {
        let max_bucket_from_events = (max_event_ts / bucket_width_sec).floor() as usize;
        let min_buckets_from_dur = vod_duration_sec.map_or(0, |dur| {
            let count = (dur / bucket_width_sec).ceil();
            if count.is_finite() && count > 0.0 {
                count as usize
            } else {
                0
            }
        });
        (max_bucket_from_events.saturating_add(1)).max(min_buckets_from_dur)
    } else {
        match vod_duration_sec {
            Some(dur) if dur > 0.0 => {
                let count = (dur / bucket_width_sec).ceil();
                if !count.is_finite() || count < 0.0 {
                    0
                } else {
                    count as usize
                }
            }
            _ => 0,
        }
    };

    if total_buckets > MAX_ALLOWED_BUCKETS {
        return Err(AppError::Highlight(HighlightError::ChatParsingFailed(format!(
            "Calculated bucket count {total_buckets} exceeds maximum limit of {MAX_ALLOWED_BUCKETS}"
        ))));
    }

    if total_buckets == 0 {
        return Ok(Vec::new());
    }

    // Pass 2: Aggregate into buckets using efficient borrowed string keys
    let mut accumulators: Vec<BucketAcc<'_>> = Vec::new();
    accumulators.resize_with(total_buckets, BucketAcc::default);

    for event in valid_events {
        let bucket_idx = (event.timestamp_sec / bucket_width_sec).floor() as usize;
        if bucket_idx < total_buckets {
            if let Some(acc) = accumulators.get_mut(bucket_idx) {
                acc.raw_count = acc.raw_count.saturating_add(1);
                let author_key = if event.author.trim().is_empty() {
                    "anonymous"
                } else {
                    event.author.as_str()
                };
                let counter = acc.author_counts.entry(author_key).or_insert(0);
                *counter = counter.saturating_add(1);
            }
        }
    }

    // Pass 3: Produce continuous time series with spam bot author cap (AC-09.2, UT-09.3)
    let mut buckets = Vec::with_capacity(total_buckets);
    for (i, acc) in accumulators.into_iter().enumerate() {
        let effective_count: u32 = acc
            .author_counts
            .values()
            .map(|&c| c.min(SPAM_BOT_MAX_MESSAGES_PER_AUTHOR))
            .sum();

        buckets.push(ChatBucket {
            index: i,
            timestamp_sec: (i as f64) * bucket_width_sec,
            raw_count: acc.raw_count,
            effective_count,
        });
    }

    Ok(buckets)
}

/// Convenience alias for `bin_chat_events`.
pub fn bucket_chat_events(
    events: &[ChatEvent],
    bucket_width_sec: f64,
    vod_duration_sec: Option<f64>,
) -> Result<Vec<ChatBucket>, AppError> {
    bin_chat_events(events, bucket_width_sec, vod_duration_sec)
}

/// Bin chat events using the default 5.0-second bucket window.
pub fn bin_chat_events_default(
    events: &[ChatEvent],
    vod_duration_sec: Option<f64>,
) -> Result<Vec<ChatBucket>, AppError> {
    bin_chat_events(events, DEFAULT_BUCKET_WIDTH_SEC, vod_duration_sec)
}

/// Bin chat events loaded directly from a JSON or NDJSON chat log file.
pub fn bin_chat_events_from_file<P: AsRef<Path>>(
    file_path: P,
    bucket_width_sec: f64,
    vod_duration_sec: Option<f64>,
) -> Result<Vec<ChatBucket>, AppError> {
    let path = file_path.as_ref();
    if !path.exists() {
        return Err(AppError::Storage(StorageError::FileNotFound(
            path.display().to_string(),
        )));
    }

    let file = File::open(path).map_err(|e| {
        AppError::Storage(StorageError::DiskCheckFailed {
            path: path.display().to_string(),
            message: e.to_string(),
        })
    })?;

    let is_ndjson = path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("ndjson"));

    let reader = BufReader::new(file);

    let events: Vec<ChatEvent> = if is_ndjson {
        let mut parsed_events = Vec::new();
        for line_res in reader.lines() {
            let line = line_res.map_err(|e| {
                AppError::Highlight(HighlightError::ChatParsingFailed(format!(
                    "Failed to read line: {e}"
                )))
            })?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Ok(event) = serde_json::from_str::<ChatEvent>(trimmed) {
                parsed_events.push(event);
            }
        }
        parsed_events
    } else {
        // Attempt container parsing, then raw array parsing
        let container: Result<crate::ingestion::ChatEventsContainer, _> =
            serde_json::from_reader(reader);
        match container {
            Ok(c) => c.events,
            Err(_) => {
                let file_retry = File::open(path).map_err(|e| {
                    AppError::Storage(StorageError::DiskCheckFailed {
                        path: path.display().to_string(),
                        message: e.to_string(),
                    })
                })?;
                serde_json::from_reader::<_, Vec<ChatEvent>>(BufReader::new(file_retry)).map_err(
                    |e| {
                        AppError::Highlight(HighlightError::ChatParsingFailed(format!(
                            "Failed to parse chat events JSON: {e}"
                        )))
                    },
                )?
            }
        }
    };

    bin_chat_events(&events, bucket_width_sec, vod_duration_sec)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
pub mod tests {
    use super::*;

    fn make_event(ts: f64, author: &str, msg: &str) -> ChatEvent {
        ChatEvent {
            timestamp_sec: ts,
            author: author.to_string(),
            message: msg.to_string(),
            badges: Vec::new(),
            emotes_detected: Vec::new(),
            is_swarm_emote: false,
        }
    }

    #[test]
    fn test_bucket_5s_fixed_binning() {
        let events = vec![
            make_event(0.5, "user1", "hi"),
            make_event(4.9, "user2", "hey"),
            make_event(5.0, "user3", "hello"),
            make_event(7.5, "user4", "kekw"),
            make_event(10.0, "user5", "yo"),
        ];

        let buckets = bin_chat_events_default(&events, None).unwrap();
        assert_eq!(buckets.len(), 3);

        // Bucket 0 [0.0, 5.0)
        assert_eq!(buckets[0].index, 0);
        assert!((buckets[0].timestamp_sec - 0.0).abs() < 1e-6);
        assert_eq!(buckets[0].raw_count, 2);
        assert_eq!(buckets[0].effective_count, 2);

        // Bucket 1 [5.0, 10.0)
        assert_eq!(buckets[1].index, 1);
        assert!((buckets[1].timestamp_sec - 5.0).abs() < 1e-6);
        assert_eq!(buckets[1].raw_count, 2);
        assert_eq!(buckets[1].effective_count, 2);

        // Bucket 2 [10.0, 15.0)
        assert_eq!(buckets[2].index, 2);
        assert!((buckets[2].timestamp_sec - 10.0).abs() < 1e-6);
        assert_eq!(buckets[2].raw_count, 1);
        assert_eq!(buckets[2].effective_count, 1);
    }

    #[test]
    fn test_spam_bot_deduplication_cap_at_3() {
        // UT-09.3 requirement: 20 messages by 1 author in 5s bucket
        let mut events = Vec::new();
        for i in 0..20 {
            events.push(make_event(1.0 + (i as f64) * 0.1, "spambot9000", "BUY BITCOIN"));
        }

        let buckets = bin_chat_events_default(&events, None).unwrap();
        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets[0].raw_count, 20);
        assert_eq!(buckets[0].effective_count, 3);
        assert_eq!(buckets[0].filtered_count(), 3);
    }

    #[test]
    fn test_multiple_authors_deduplication() {
        let mut events = Vec::new();
        // user1 sends 10 messages -> capped at 3
        for _ in 0..10 {
            events.push(make_event(2.0, "user1", "spam"));
        }
        // user2 sends 2 messages -> kept as 2
        for _ in 0..2 {
            events.push(make_event(2.5, "user2", "normal"));
        }
        // user3 sends 5 messages -> capped at 3
        for _ in 0..5 {
            events.push(make_event(3.0, "user3", "fast"));
        }

        let buckets = bin_chat_events_default(&events, None).unwrap();
        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets[0].raw_count, 17);
        // 3 (user1) + 2 (user2) + 3 (user3) = 8
        assert_eq!(buckets[0].effective_count, 8);
    }

    #[test]
    fn test_invalid_timestamps_sanitization() {
        let events = vec![
            make_event(-5.0, "troll", "time traveler"),
            make_event(f64::NAN, "broken", "nan"),
            make_event(f64::INFINITY, "inf", "infinity"),
            make_event(12_000_000.0, "future", "far future"),
            make_event(2.5, "legit", "hello world"),
        ];

        let buckets = bin_chat_events_default(&events, None).unwrap();
        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets[0].raw_count, 1);
        assert_eq!(buckets[0].effective_count, 1);
    }

    #[test]
    fn test_continuous_series_with_zero_buckets() {
        let events = vec![
            make_event(1.0, "user1", "first"),
            make_event(21.0, "user2", "gap of 3 buckets"),
        ];

        let buckets = bin_chat_events_default(&events, None).unwrap();
        // 21.0 / 5.0 = 4.2 -> index 4 -> 5 buckets total (0, 1, 2, 3, 4)
        assert_eq!(buckets.len(), 5);
        assert_eq!(buckets[0].raw_count, 1);
        assert_eq!(buckets[1].raw_count, 0);
        assert_eq!(buckets[2].raw_count, 0);
        assert_eq!(buckets[3].raw_count, 0);
        assert_eq!(buckets[4].raw_count, 1);
    }

    #[test]
    fn test_vod_duration_padding() {
        let events = vec![make_event(2.0, "user", "early")];
        // VOD duration 30.0s -> 6 buckets (0..5)
        let buckets = bin_chat_events_default(&events, Some(30.0)).unwrap();
        assert_eq!(buckets.len(), 6);
        assert_eq!(buckets[0].raw_count, 1);
        for b in &buckets[1..] {
            assert_eq!(b.raw_count, 0);
            assert_eq!(b.effective_count, 0);
        }
    }

    #[test]
    fn test_invalid_parameters_error() {
        let events = vec![make_event(1.0, "user", "hi")];
        assert!(bin_chat_events(&events, -1.0, None).is_err());
        assert!(bin_chat_events(&events, 0.0, None).is_err());
        assert!(bin_chat_events(&events, f64::NAN, None).is_err());
        assert!(bin_chat_events(&events, 5.0, Some(-10.0)).is_err());
    }
}
