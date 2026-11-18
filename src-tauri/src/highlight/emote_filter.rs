//! Neuro Swarm Emote Density Filtering Engine & 5-Second Time Series Aggregator.
//!
//! Complies with `docs/CONVENTIONS.md` (zero-panic invariant, Result<T, AppError>),
//! `docs/ARCHITECTURE.md` §4.3 & §5.3, and `docs/TASK_BOARD.md` §TASK-M4-10.

#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, HighlightError, StorageError};
use crate::highlight::bucket::{
    DEFAULT_BUCKET_WIDTH_SEC, MAX_ALLOWED_BUCKETS, MAX_CHAT_TIMESTAMP_SEC,
    SPAM_BOT_MAX_MESSAGES_PER_AUTHOR,
};
use crate::highlight::swarm_dict::{default_swarm_dictionary, BuildFastHasher, SwarmDictionary};
use crate::ingestion::ChatEvent;

/// Maximum number of top emotes to preserve per 5-second bucket.
pub const MAX_TOP_EMOTES_PER_BUCKET: usize = 5;

/// Output point for the 5-second Swarm Emote Density series (`docs/TASK_BOARD.md` §Data Contracts).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmoteDensityPoint {
    /// Timestamp in seconds from VOD start ($t_k = k \times 5.0\text{s}$).
    pub timestamp_sec: f64,
    /// Top Swarm emotes occurring in this 5-second bucket, sorted by frequency descending.
    pub top_emotes: Vec<String>,
    /// Normalized Swarm emote ratio $E_{\text{ratio}}(t) \in [0.0, 1.0]$.
    pub e_ratio: f64,
    /// Total vocabulary tokens in this bucket after anti-spam author capping.
    pub total_tokens: u32,
    /// Total count of Swarm cultural emotes in this bucket.
    pub emote_count: u32,
    /// Culturally weighted emote score accounting for specialized weights.
    pub weighted_score: f64,
}

impl EmoteDensityPoint {
    /// Construct a new `EmoteDensityPoint`.
    #[must_use]
    pub const fn new(
        timestamp_sec: f64,
        top_emotes: Vec<String>,
        e_ratio: f64,
        total_tokens: u32,
        emote_count: u32,
        weighted_score: f64,
    ) -> Self {
        Self {
            timestamp_sec,
            top_emotes,
            e_ratio,
            total_tokens,
            emote_count,
            weighted_score,
        }
    }

    /// Alias for emote_count for backward and contract compatibility.
    #[must_use]
    pub const fn swarm_count(&self) -> u32 {
        self.emote_count
    }
}

/// Clean leading and trailing punctuation from a token while preserving `<3` and `💀`.
#[inline]
#[must_use]
pub fn clean_word_token(word: &str) -> &str {
    if word == "<3" {
        return "<3";
    }

    let bytes = word.as_bytes();
    if bytes.is_empty() {
        return "";
    }

    // Fast path: if both first and last bytes are ASCII alphanumeric, no outer punctuation to trim
    let first = bytes[0];
    let last = bytes[bytes.len() - 1];
    if first.is_ascii_alphanumeric() && last.is_ascii_alphanumeric() {
        return word;
    }

    let outer = word.trim_matches(|c: char| {
        matches!(
            c,
            '!' | '?'
                | '.'
                | ','
                | ';'
                | ':'
                | '"'
                | '\''
                | '`'
                | '('
                | ')'
                | '['
                | ']'
                | '{'
                | '}'
                | '~'
                | '*'
                | '^'
                | '-'
                | '_'
                | '/'
                | '\\'
                | '@'
                | '#'
                | '$'
                | '%'
        )
    });

    if outer == "<3" {
        return "<3";
    }

    outer.trim_matches(|c: char| !c.is_alphanumeric() && c != '💀')
}

/// Zero-allocation inner loop processor for tokenizing and counting emotes.
#[inline]
fn process_message_tokens<F>(
    message: &str,
    dict: &SwarmDictionary,
    total_tokens: &mut u32,
    emote_count: &mut u32,
    weighted_sum: &mut f64,
    mut on_canonical: F,
) where
    F: FnMut(&str),
{
    let has_skull = message.contains('💀');

    for word in message.split_whitespace() {
        if has_skull && word.contains('💀') {
            let skull_count = word.chars().filter(|&c| c == '💀').count() as u32;
            *total_tokens = total_tokens.saturating_add(skull_count);
            *emote_count = emote_count.saturating_add(skull_count);
            if let Some(def) = dict.lookup("💀") {
                *weighted_sum += def.weight * f64::from(skull_count);
            }
            for _ in 0..skull_count {
                on_canonical("💀");
            }

            for piece in word.split('💀') {
                let clean = clean_word_token(piece);
                if !clean.is_empty() {
                    *total_tokens = total_tokens.saturating_add(1);
                    if let Some(def) = dict.lookup(clean) {
                        *emote_count = emote_count.saturating_add(1);
                        *weighted_sum += def.weight;
                        on_canonical(&def.canonical_name);
                    }
                }
            }
        } else {
            let clean = clean_word_token(word);
            if !clean.is_empty() {
                *total_tokens = total_tokens.saturating_add(1);
                if let Some(def) = dict.lookup(clean) {
                    *emote_count = emote_count.saturating_add(1);
                    *weighted_sum += def.weight;
                    on_canonical(&def.canonical_name);
                }
            }
        }
    }
}

/// Tokenize a chat message and extract detected Swarm emotes.
///
/// Returns a tuple containing:
/// 1. `Vec<String>` - Detected emote strings as matched in the text
/// 2. `u32` - Total token count
/// 3. `u32` - Swarm emote count
/// 4. `f64` - Sum of cultural weights for all matched emotes
/// 5. `Vec<String>` - Canonical emote names corresponding to matched emotes
#[must_use]
pub fn extract_message_tokens(
    message: &str,
    dict: &SwarmDictionary,
) -> (Vec<String>, u32, u32, f64, Vec<String>) {
    let mut detected: Vec<String> = Vec::new();
    let mut canonical_detected: Vec<String> = Vec::new();
    let mut total_tokens: u32 = 0;
    let mut emote_tokens: u32 = 0;
    let mut weighted_sum: f64 = 0.0;

    let has_skull = message.contains('💀');

    for word in message.split_whitespace() {
        if has_skull && word.contains('💀') {
            let skull_count = word.chars().filter(|&c| c == '💀').count() as u32;
            total_tokens = total_tokens.saturating_add(skull_count);
            emote_tokens = emote_tokens.saturating_add(skull_count);

            for _ in 0..skull_count {
                detected.push("💀".to_string());
                canonical_detected.push("💀".to_string());
            }

            if let Some(skull_def) = dict.lookup("💀") {
                weighted_sum += skull_def.weight * f64::from(skull_count);
            }

            for piece in word.split('💀') {
                let clean = clean_word_token(piece);
                if !clean.is_empty() {
                    total_tokens = total_tokens.saturating_add(1);
                    if let Some(def) = dict.lookup(clean) {
                        emote_tokens = emote_tokens.saturating_add(1);
                        weighted_sum += def.weight;
                        detected.push(clean.to_string());
                        canonical_detected.push(def.canonical_name.clone());
                    }
                }
            }
        } else {
            let clean = clean_word_token(word);
            if !clean.is_empty() {
                total_tokens = total_tokens.saturating_add(1);
                if let Some(def) = dict.lookup(clean) {
                    emote_tokens = emote_tokens.saturating_add(1);
                    weighted_sum += def.weight;
                    detected.push(clean.to_string());
                    canonical_detected.push(def.canonical_name.clone());
                }
            }
        }
    }

    (
        detected,
        total_tokens,
        emote_tokens,
        weighted_sum,
        canonical_detected,
    )
}

/// Helper to extract detected Swarm emote list from a message (UT-10.1).
///
/// For example, `"KEKW look at that tutel 💀"` returns `["KEKW", "tutel", "💀"]`.
#[must_use]
pub fn extract_swarm_emotes(message: &str) -> Vec<String> {
    let dict = default_swarm_dictionary();
    let (detected, _, _, _, _) = extract_message_tokens(message, &dict);
    detected
}

/// Helper to check if a word token belongs to the Swarm dictionary (UT-10.2).
#[must_use]
pub fn is_swarm_emote(token: &str) -> bool {
    let dict = default_swarm_dictionary();
    let clean = clean_word_token(token);
    dict.contains(clean)
}

/// Pure mathematical calculation of unweighted Emote Swarm Ratio $E_{\text{ratio}} \in [0.0, 1.0]$ (UT-10.3).
#[must_use]
pub fn calculate_emote_ratio(emote_count: u32, total_tokens: u32) -> f64 {
    if total_tokens == 0 {
        0.0
    } else {
        (f64::from(emote_count) / f64::from(total_tokens)).clamp(0.0, 1.0)
    }
}

/// Internal bucket accumulator for emote filtering.
#[derive(Default)]
struct EmoteBucketAcc<'a> {
    author_counts: HashMap<&'a str, u32, BuildFastHasher>,
    total_tokens: u32,
    emote_count: u32,
    weighted_sum: f64,
    canonical_frequencies: HashMap<String, u32, BuildFastHasher>,
}

/// Compute the 5-second Swarm Emote Density series from a slice of `ChatEvent`s.
///
/// # Arguments
/// * `events` - Slice of chat events to process.
/// * `dict` - Optional custom Swarm dictionary (uses default if `None`).
/// * `bucket_width_sec` - Width of each bucket in seconds (must be 5.0 for IT-10.1 alignment).
/// * `vod_duration_sec` - Optional total VOD duration in seconds to pad the timeline.
///
/// # Errors
/// Returns `AppError::Highlight(HighlightError::ChatParsingFailed)` if parameters are invalid
/// or bucket allocation exceeds limits.
#[allow(clippy::too_many_lines)]
pub fn compute_emote_density_series(
    events: &[ChatEvent],
    dict: Option<&SwarmDictionary>,
    bucket_width_sec: f64,
    vod_duration_sec: Option<f64>,
) -> Result<Vec<EmoteDensityPoint>, AppError> {
    if !bucket_width_sec.is_finite() || bucket_width_sec <= 0.0 || bucket_width_sec > 3600.0 {
        return Err(AppError::Highlight(HighlightError::ChatParsingFailed(
            format!("Invalid bucket width: {bucket_width_sec}s (must be > 0.0 and <= 3600.0)"),
        )));
    }

    if let Some(dur) = vod_duration_sec {
        if !dur.is_finite() || dur < 0.0 || dur > MAX_CHAT_TIMESTAMP_SEC {
            return Err(AppError::Highlight(HighlightError::ChatParsingFailed(
                format!("Invalid VOD duration: {dur}s"),
            )));
        }
    }

    let default_dict;
    let active_dict = if let Some(d) = dict {
        d
    } else {
        default_dict = default_swarm_dictionary();
        &default_dict
    };

    // Pass 1: Find maximum timestamp without extra vector allocations
    let mut max_event_ts: f64 = 0.0;
    let mut has_valid_events = false;

    for event in events {
        let ts = event.timestamp_sec;
        if ts.is_finite()
            && (0.0..=MAX_CHAT_TIMESTAMP_SEC).contains(&ts)
            && (!has_valid_events || ts > max_event_ts)
        {
            max_event_ts = ts;
            has_valid_events = true;
        }
    }

    // Determine total buckets needed - strictly matching bucket.rs 1:1
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
        return Err(AppError::Highlight(HighlightError::ChatParsingFailed(
            format!(
            "Calculated bucket count {total_buckets} exceeds maximum limit of {MAX_ALLOWED_BUCKETS}"
        ),
        )));
    }

    if total_buckets == 0 {
        return Ok(Vec::new());
    }

    // Pass 2: Bin events with in-bucket per-author spam bot deduplication (max 3 messages)
    let mut accumulators: Vec<EmoteBucketAcc<'_>> = Vec::new();
    accumulators.resize_with(total_buckets, EmoteBucketAcc::default);

    for event in events {
        let ts = event.timestamp_sec;
        if ts.is_finite() && (0.0..=MAX_CHAT_TIMESTAMP_SEC).contains(&ts) {
            let bucket_idx = (ts / bucket_width_sec).floor() as usize;
            if let Some(acc) = accumulators.get_mut(bucket_idx) {
                let author_key = if event.author.is_empty() {
                    "anonymous"
                } else {
                    event.author.as_str()
                };

                let author_msg_count = acc.author_counts.entry(author_key).or_insert(0);
                if *author_msg_count < SPAM_BOT_MAX_MESSAGES_PER_AUTHOR {
                    *author_msg_count = author_msg_count.saturating_add(1);

                    // High-performance token scanning without per-message Vec allocations
                    process_message_tokens(
                        &event.message,
                        active_dict,
                        &mut acc.total_tokens,
                        &mut acc.emote_count,
                        &mut acc.weighted_sum,
                        |canonical_name| {
                            if let Some(counter) = acc.canonical_frequencies.get_mut(canonical_name)
                            {
                                *counter = counter.saturating_add(1);
                            } else {
                                acc.canonical_frequencies
                                    .insert(canonical_name.to_string(), 1);
                            }
                        },
                    );
                }
            }
        }
    }

    // Pass 3: Produce continuous time series with exact 5s timestamps, e_ratio, and top_emotes
    let mut series = Vec::with_capacity(total_buckets);
    for (i, acc) in accumulators.into_iter().enumerate() {
        let e_ratio = calculate_emote_ratio(acc.emote_count, acc.total_tokens);

        let weighted_score = if acc.total_tokens == 0 {
            0.0
        } else {
            (acc.weighted_sum / f64::from(acc.total_tokens)).max(0.0)
        };

        // Extract top emotes sorted by frequency descending, then name ascending
        let mut freq_pairs: Vec<(String, u32)> = acc.canonical_frequencies.into_iter().collect();
        freq_pairs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        let top_emotes: Vec<String> = freq_pairs
            .into_iter()
            .take(MAX_TOP_EMOTES_PER_BUCKET)
            .map(|(name, _)| name)
            .collect();

        series.push(EmoteDensityPoint {
            timestamp_sec: (i as f64) * bucket_width_sec,
            top_emotes,
            e_ratio,
            total_tokens: acc.total_tokens,
            emote_count: acc.emote_count,
            weighted_score,
        });
    }

    Ok(series)
}

/// Compute Swarm Emote Density series using the default 5.0-second bucket window.
pub fn compute_emote_density_series_default(
    events: &[ChatEvent],
    vod_duration_sec: Option<f64>,
) -> Result<Vec<EmoteDensityPoint>, AppError> {
    compute_emote_density_series(events, None, DEFAULT_BUCKET_WIDTH_SEC, vod_duration_sec)
}

/// Intermediate chat event representation for flexible JSON/NDJSON file loading.
#[derive(Deserialize)]
struct RawChatEventFileItem {
    timestamp_sec: f64,
    #[serde(default)]
    author: String,
    #[serde(default)]
    message: String,
}

/// Compute Swarm Emote Density series loaded directly from a JSON or NDJSON chat log file.
pub fn compute_emote_density_series_from_file<P: AsRef<Path>>(
    file_path: P,
    dict: Option<&SwarmDictionary>,
    bucket_width_sec: f64,
    vod_duration_sec: Option<f64>,
) -> Result<Vec<EmoteDensityPoint>, AppError> {
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
            if let Ok(raw) = serde_json::from_str::<RawChatEventFileItem>(trimmed) {
                parsed_events.push(ChatEvent {
                    timestamp_sec: raw.timestamp_sec,
                    author: raw.author,
                    message: raw.message,
                    badges: Vec::new(),
                    emotes_detected: Vec::new(),
                    is_swarm_emote: false,
                });
            } else if let Ok(event) = serde_json::from_str::<ChatEvent>(trimmed) {
                parsed_events.push(event);
            }
        }
        parsed_events
    } else {
        let container: Result<crate::ingestion::ChatEventsContainer, _> =
            serde_json::from_reader(reader);
        if let Ok(c) = container {
            c.events
        } else {
            let file_retry = File::open(path).map_err(|e| {
                AppError::Storage(StorageError::DiskCheckFailed {
                    path: path.display().to_string(),
                    message: e.to_string(),
                })
            })?;
            if let Ok(events) =
                serde_json::from_reader::<_, Vec<ChatEvent>>(BufReader::new(file_retry))
            {
                events
            } else {
                let file_retry2 = File::open(path).map_err(|e| {
                    AppError::Storage(StorageError::DiskCheckFailed {
                        path: path.display().to_string(),
                        message: e.to_string(),
                    })
                })?;
                let raw_items = serde_json::from_reader::<_, Vec<RawChatEventFileItem>>(
                    BufReader::new(file_retry2),
                )
                .map_err(|e| {
                    AppError::Highlight(HighlightError::ChatParsingFailed(format!(
                        "Failed to parse chat events JSON: {e}"
                    )))
                })?;
                raw_items
                    .into_iter()
                    .map(|raw| ChatEvent {
                        timestamp_sec: raw.timestamp_sec,
                        author: raw.author,
                        message: raw.message,
                        badges: Vec::new(),
                        emotes_detected: Vec::new(),
                        is_swarm_emote: false,
                    })
                    .collect()
            }
        }
    };

    compute_emote_density_series(&events, dict, bucket_width_sec, vod_duration_sec)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
pub mod tests {
    use super::*;

    fn create_test_event(timestamp_sec: f64, author: &str, message: &str) -> ChatEvent {
        ChatEvent {
            timestamp_sec,
            author: author.to_string(),
            message: message.to_string(),
            badges: Vec::new(),
            emotes_detected: Vec::new(),
            is_swarm_emote: false,
        }
    }

    #[test]
    fn test_ut_10_1_happy_path_swarm_match() {
        let msg = "KEKW look at that tutel 💀";
        let detected = extract_swarm_emotes(msg);
        assert_eq!(
            detected,
            vec!["KEKW".to_string(), "tutel".to_string(), "💀".to_string()]
        );
    }

    #[test]
    fn test_ut_10_2_token_boundaries_and_punctuation() {
        assert!(is_swarm_emote("kekw"));
        assert!(is_swarm_emote("KEKW"));
        assert!(is_swarm_emote("Kekw"));
        assert!(is_swarm_emote("TUTEL"));
        assert!(is_swarm_emote("tutel"));
        assert!(is_swarm_emote("💀"));
        assert!(is_swarm_emote("PogBones"));
        assert!(is_swarm_emote("<3"));
        assert!(is_swarm_emote("(tutel)!"));
        assert!(is_swarm_emote("Erm..."));
        assert!(is_swarm_emote("SOOP!"));

        // Negative non-matches
        assert!(!is_swarm_emote("medieval"));
        assert!(!is_swarm_emote("talking"));
        assert!(!is_swarm_emote("pogo"));
        assert!(!is_swarm_emote("neurological"));
        assert!(!is_swarm_emote("hello"));
    }

    #[test]
    fn test_ut_10_3_emote_ratio_calculation() {
        // UT-10.3: 50 tokens, 25 swarm emotes => 25 / 50 = 0.50
        let ratio = calculate_emote_ratio(25, 50);
        assert!((ratio - 0.50).abs() < 1e-6);

        // 0 tokens => 0.0
        assert_eq!(calculate_emote_ratio(0, 0), 0.0);

        // Clamping to [0.0, 1.0]
        assert_eq!(calculate_emote_ratio(60, 50), 1.0);
        assert_eq!(calculate_emote_ratio(50, 50), 1.0);
        assert_eq!(calculate_emote_ratio(0, 50), 0.0);
    }

    #[test]
    fn test_spambot_author_cap_at_3() {
        let mut events = Vec::new();
        // Spammer posts 10 messages with 3 KEKWs in bucket 0 [0.0, 5.0)
        for _ in 0..10 {
            events.push(create_test_event(2.0, "spambot", "KEKW KEKW KEKW"));
        }

        let series = compute_emote_density_series(&events, None, 5.0, None).expect("Must succeed");
        assert_eq!(series.len(), 1);
        // Only 3 messages counted: 3 * 3 = 9 tokens, 9 emotes
        assert_eq!(series[0].total_tokens, 9);
        assert_eq!(series[0].emote_count, 9);
        assert!((series[0].e_ratio - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_top_emotes_ranking_and_limit() {
        let events = vec![
            create_test_event(1.0, "user1", "KEKW KEKW KEKW Tutel Tutel"),
            create_test_event(2.0, "user2", "Erm SOOP 💀 KEKW"),
        ];

        let series = compute_emote_density_series(&events, None, 5.0, None).expect("Must succeed");
        assert_eq!(series.len(), 1);
        let top = &series[0].top_emotes;

        // KEKW appears 4 times, Tutel 2 times, Erm 1, SOOP 1, 💀 1
        assert_eq!(top[0], "KEKW");
        assert_eq!(top[1], "Tutel");
        assert!(top.len() <= MAX_TOP_EMOTES_PER_BUCKET);
    }

    #[test]
    fn test_ed_10_1_performance_microbenchmark() {
        // Generate 50,000 messages (microbenchmark in unit test)
        let mut events = Vec::with_capacity(50_000);
        for i in 0..50_000 {
            let ts = f64::from(i) * 0.05; // 20 msgs per second
            let author = format!("user_{}", i % 500);
            let msg = match i % 5 {
                0 => "KEKW look at that tutel 💀",
                1 => "just normal chat message here",
                2 => "SOOP SOOP Erm <3",
                3 => "PogBones holy unhinged moment",
                _ => "lol lmao XD",
            };
            events.push(create_test_event(ts, &author, msg));
        }

        let start = std::time::Instant::now();
        let series = compute_emote_density_series(&events, None, 5.0, None).expect("Must succeed");
        let elapsed = start.elapsed();

        println!(
            "Microbenchmark: 50,000 messages processed in {:.2}ms ({} buckets)",
            elapsed.as_secs_f64() * 1000.0,
            series.len()
        );
        assert!(elapsed.as_secs_f64() < 0.5);
    }
}
