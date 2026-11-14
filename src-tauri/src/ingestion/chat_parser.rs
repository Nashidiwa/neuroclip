//! Streaming chat parser, Schema 5.3 normalizer, and 10s sliding window density heatmap.
//!
//! Complies with docs/CONVENTIONS.md (zero-panic invariant, Result<T, AppError>)
//! and docs/ARCHITECTURE.md §5.3.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, HighlightError, StorageError};

/// Maximum chat timestamp in seconds (~115.7 days = 10,000,000s).
/// Timestamps beyond this limit (or negative / non-finite) are rejected to prevent astronomical memory allocations.
pub const MAX_CHAT_TIMESTAMP_SEC: f64 = 10_000_000.0;

/// Maximum number of heatmap buckets allowed (1,000,001 buckets <= 4MB for count vectors).
pub const MAX_HEATMAP_BUCKETS: usize = 1_000_001;

/// Neuro Swarm Cultural Emote Dictionary (ADR-004 §1, PRD Table §4.4).
pub static NEURO_SWARM_EMOTES: &[&str] = &[
    // Humor / Reaction
    "KEKW", "LUL", "OMEGALUL", "LOLW", "HASTAG", "💀", "LMAO", "LOL", "XD", "WutFace", "ICANT",
    // Surprise / Shock / Cringe
    "Pog", "PogBones", "Aware", "Clueless", "WTF", "monkaS", "monkaW", "DESPAIR", "HUH",
    // Swarm Lore & Characters
    "Tutel", "Gymbag", "Heart", "neuroHeart", "evilHeart", "Dinking", "Vedal", "Evil", "Neuro", "Erm",
    "Filtered", "SoCute", "AYAYA", "Anny", "Camila", "Bedge", "Corpa", "GIGACHAD", "pepeJAM", "CatJAM", "SOUTHSIDE",
];

/// Schema 5.3 Chat Event item (`docs/ARCHITECTURE.md` §5.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatEvent {
    pub timestamp_sec: f64,
    pub author: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub badges: Vec<String>,
    pub emotes_detected: Vec<String>,
    #[serde(default)]
    pub is_swarm_emote: bool,
}

/// Schema 5.3 Chat Events Document (`docs/ARCHITECTURE.md` §5.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatEventsContainer {
    pub vod_id: String,
    pub total_messages: u64,
    pub events: Vec<ChatEvent>,
}

/// Sliding window density heatmap point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatDensityPoint {
    pub timestamp_sec: f64,
    pub message_count: u32,
    pub velocity_mps: f64,
    pub swarm_count: u32,
    pub swarm_ratio: f64,
    pub z_score: f64,
    pub normalized_density: f64,
    #[serde(default)]
    pub is_peak: bool,
}

/// Aliases for architecture and specification compatibility.
pub type HeatmapWindow = ChatDensityPoint;

/// Heatmap response document containing sliding window analytics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatHeatmapResponse {
    pub vod_id: String,
    pub window_seconds: f64,
    pub step_seconds: f64,
    pub total_buckets: usize,
    pub max_velocity_mps: f64,
    pub peak_timestamp_sec: f64,
    pub points: Vec<ChatDensityPoint>,
}

pub type ChatHeatmapContainer = ChatHeatmapResponse;

/// Check if a word or emote ID belongs to the Neuro Swarm cultural dictionary.
#[must_use]
pub fn is_neuro_swarm_emote(token: &str) -> bool {
    let trimmed = token.trim_matches(|c: char| !c.is_alphanumeric() && c != '💀');
    if trimmed.is_empty() {
        return false;
    }
    NEURO_SWARM_EMOTES
        .iter()
        .any(|&e| e.eq_ignore_ascii_case(trimmed) || e == trimmed)
}

/// Tokenize a chat message and extract detected emotes.
#[must_use]
pub fn extract_message_emotes(message: &str) -> (Vec<String>, bool) {
    let mut detected: Vec<String> = Vec::new();
    let mut is_swarm = false;

    // Split words by whitespace
    for word in message.split_whitespace() {
        let clean = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '💀');
        if clean.is_empty() {
            continue;
        }

        if is_neuro_swarm_emote(clean) {
            let matched_name = NEURO_SWARM_EMOTES
                .iter()
                .find(|&&e| e.eq_ignore_ascii_case(clean) || e == clean)
                .copied()
                .unwrap_or(clean);

            if !detected.iter().any(|d| d.eq_ignore_ascii_case(matched_name)) {
                detected.push(matched_name.to_string());
            }
            is_swarm = true;
        }
    }

    // Direct check for skull emoji if attached without whitespace
    if message.contains('💀') && !detected.iter().any(|d| d == "💀") {
        detected.push("💀".to_string());
        is_swarm = true;
    }

    (detected, is_swarm)
}

/// Raw line parsing intermediate representations.
#[derive(Deserialize)]
struct RawTwitchEdge {
    node: Option<RawTwitchNode>,
}

#[derive(Deserialize)]
struct RawTwitchNode {
    #[serde(alias = "content_offset_seconds", alias = "contentOffsetSeconds")]
    content_offset_seconds: Option<f64>,
    commenter: Option<RawTwitchCommenter>,
    message: Option<RawTwitchMessage>,
}

#[derive(Deserialize)]
struct RawTwitchCommenter {
    #[serde(alias = "display_name", alias = "displayName")]
    display_name: Option<String>,
    login: Option<String>,
}

#[derive(Deserialize)]
struct RawTwitchMessage {
    fragments: Option<Vec<RawTwitchFragment>>,
}

#[derive(Deserialize)]
struct RawTwitchFragment {
    text: Option<String>,
    emoticon: Option<RawTwitchEmoticon>,
}

#[derive(Deserialize)]
struct RawTwitchEmoticon {
    #[serde(alias = "emoticon_id", alias = "emoticonID")]
    emoticon_id: Option<String>,
}

#[derive(Deserialize)]
struct RawYouTubeLine {
    #[serde(alias = "replayChatItemAction")]
    replay_chat_item_action: Option<RawYtReplayAction>,
}

#[derive(Deserialize)]
struct RawYtReplayAction {
    actions: Option<Vec<RawYtActionItem>>,
    #[serde(alias = "videoOffsetTimeMsec")]
    video_offset_time_msec: Option<String>,
}

#[derive(Deserialize)]
struct RawYtActionItem {
    #[serde(alias = "addChatItemAction")]
    add_chat_item_action: Option<RawYtAddChatItemAction>,
}

#[derive(Deserialize)]
struct RawYtAddChatItemAction {
    item: Option<RawYtItem>,
}

#[derive(Deserialize)]
struct RawYtItem {
    #[serde(alias = "liveChatTextMessageRenderer")]
    live_chat_text_message_renderer: Option<RawYtMessageRenderer>,
    #[serde(alias = "liveChatPaidMessageRenderer")]
    live_chat_paid_message_renderer: Option<RawYtMessageRenderer>,
}

#[derive(Deserialize)]
struct RawYtMessageRenderer {
    message: Option<RawYtMessageBody>,
    #[serde(alias = "authorName")]
    author_name: Option<RawYtAuthorName>,
    #[serde(alias = "videoOffsetTimeMsec")]
    video_offset_time_msec: Option<String>,
}

#[derive(Deserialize)]
struct RawYtMessageBody {
    runs: Option<Vec<RawYtRun>>,
}

#[derive(Deserialize)]
struct RawYtRun {
    text: Option<String>,
    emoji: Option<RawYtEmoji>,
}

#[derive(Deserialize)]
struct RawYtEmoji {
    shortcuts: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct RawYtAuthorName {
    #[serde(alias = "simpleText")]
    simple_text: Option<String>,
}

/// Parse a single line from either YouTube live chat, Twitch GQL comment, or normalized JSON.
#[must_use]
pub fn parse_raw_chat_line(line: &str) -> Option<ChatEvent> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    // 1. Try parsing directly as normalized ChatEvent
    if let Ok(event) = serde_json::from_str::<ChatEvent>(trimmed) {
        if event.timestamp_sec.is_finite()
            && event.timestamp_sec >= 0.0
            && event.timestamp_sec <= MAX_CHAT_TIMESTAMP_SEC
            && !event.author.trim().is_empty()
            && (!event.message.trim().is_empty() || !event.emotes_detected.is_empty())
        {
            return Some(event);
        }
    }

    // 2. Try parsing as Twitch GQL Edge / Node
    if let Ok(edge) = serde_json::from_str::<RawTwitchEdge>(trimmed) {
        if let Some(node) = edge.node {
            if let Some(event) = parse_twitch_node(&node) {
                return Some(event);
            }
        }
    }

    // 3. Try parsing as direct Twitch Node
    if let Ok(node) = serde_json::from_str::<RawTwitchNode>(trimmed) {
        if let Some(event) = parse_twitch_node(&node) {
            return Some(event);
        }
    }

    // 4. Try parsing as YouTube Live Chat Replay line
    if let Ok(yt) = serde_json::from_str::<RawYouTubeLine>(trimmed) {
        if let Some(event) = parse_youtube_line(&yt) {
            return Some(event);
        }
    }

    None
}

fn parse_twitch_node(node: &RawTwitchNode) -> Option<ChatEvent> {
    let timestamp_sec = node.content_offset_seconds?;
    if !timestamp_sec.is_finite() || timestamp_sec < 0.0 || timestamp_sec > MAX_CHAT_TIMESTAMP_SEC {
        return None;
    }

    let author = node
        .commenter
        .as_ref()
        .and_then(|c| c.display_name.clone().or_else(|| c.login.clone()))
        .unwrap_or_else(|| "Anonymous".to_string());

    let mut message_parts: Vec<String> = Vec::new();
    let mut detected_emotes: Vec<String> = Vec::new();
    let mut is_swarm = false;

    if let Some(msg) = &node.message {
        if let Some(fragments) = &msg.fragments {
            for frag in fragments {
                if let Some(text) = &frag.text {
                    message_parts.push(text.clone());
                    let (frag_emotes, frag_swarm) = extract_message_emotes(text);
                    for e in frag_emotes {
                        if !detected_emotes.contains(&e) {
                            detected_emotes.push(e);
                        }
                    }
                    if frag_swarm {
                        is_swarm = true;
                    }
                }
                if let Some(emoticon) = &frag.emoticon {
                    if let Some(id) = &emoticon.emoticon_id {
                        if is_neuro_swarm_emote(id) {
                            is_swarm = true;
                        }
                    }
                }
            }
        }
    }

    let full_message = message_parts.join("").trim().to_string();
    if full_message.is_empty() && detected_emotes.is_empty() {
        return None;
    }

    Some(ChatEvent {
        timestamp_sec,
        author,
        message: full_message,
        badges: Vec::new(),
        emotes_detected: detected_emotes,
        is_swarm_emote: is_swarm,
    })
}

fn parse_youtube_line(yt: &RawYouTubeLine) -> Option<ChatEvent> {
    let action = yt.replay_chat_item_action.as_ref()?;
    let action_items = action.actions.as_ref()?;
    let first_item = action_items.first()?.add_chat_item_action.as_ref()?.item.as_ref()?;

    let renderer = first_item
        .live_chat_text_message_renderer
        .as_ref()
        .or(first_item.live_chat_paid_message_renderer.as_ref())?;

    let offset_ms_str = renderer
        .video_offset_time_msec
        .as_ref()
        .or(action.video_offset_time_msec.as_ref())?;

    let offset_ms: f64 = offset_ms_str.parse().ok()?;
    let timestamp_sec = offset_ms / 1000.0;
    if !timestamp_sec.is_finite() || timestamp_sec < 0.0 || timestamp_sec > MAX_CHAT_TIMESTAMP_SEC {
        return None;
    }

    let author = renderer
        .author_name
        .as_ref()
        .and_then(|a| a.simple_text.clone())
        .unwrap_or_else(|| "Anonymous".to_string());

    let mut message_parts: Vec<String> = Vec::new();
    let mut detected_emotes: Vec<String> = Vec::new();
    let mut is_swarm = false;

    if let Some(body) = &renderer.message {
        if let Some(runs) = &body.runs {
            for run in runs {
                if let Some(text) = &run.text {
                    message_parts.push(text.clone());
                    let (run_emotes, run_swarm) = extract_message_emotes(text);
                    for e in run_emotes {
                        if !detected_emotes.contains(&e) {
                            detected_emotes.push(e);
                        }
                    }
                    if run_swarm {
                        is_swarm = true;
                    }
                }
                if let Some(emoji) = &run.emoji {
                    if let Some(shortcuts) = &emoji.shortcuts {
                        for s in shortcuts {
                            if is_neuro_swarm_emote(s) {
                                is_swarm = true;
                                if !detected_emotes.contains(s) {
                                    detected_emotes.push(s.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let full_message = message_parts.join("").trim().to_string();
    if full_message.is_empty() && detected_emotes.is_empty() {
        return None;
    }

    Some(ChatEvent {
        timestamp_sec,
        author,
        message: full_message,
        badges: Vec::new(),
        emotes_detected: detected_emotes,
        is_swarm_emote: is_swarm,
    })
}

/// Reusable streaming normalizer ensuring bounded heap RAM consumption <= 80MB.
pub struct StreamingChatNormalizer {
    vod_id: String,
    cache_dir: PathBuf,
}

impl StreamingChatNormalizer {
    #[must_use]
    pub fn new(vod_id: impl Into<String>, cache_dir: impl AsRef<Path>) -> Self {
        Self {
            vod_id: vod_id.into(),
            cache_dir: cache_dir.as_ref().to_path_buf(),
        }
    }

    #[must_use]
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Normalize raw chat NDJSON file into Schema 5.3 `chat_events.json` and `chat_events.ndjson`.
    ///
    /// Peak RAM consumption is strictly <= 80MB (invariant AC-08.2) through
    /// two-pass disk buffered stream processing.
    pub fn normalize_file(
        &self,
        raw_input_path: &Path,
        output_json_path: &Path,
        cancel_token: &CancellationToken,
    ) -> Result<(u64, PathBuf), AppError> {
        if !raw_input_path.exists() {
            return Err(AppError::Storage(StorageError::FileNotFound(
                raw_input_path.display().to_string(),
            )));
        }

        let output_ndjson_path = output_json_path.with_extension("ndjson");

        // Pass 1: Stream raw lines -> normalize each line -> write to output.ndjson
        let raw_file = File::open(raw_input_path).map_err(|e| {
            AppError::Highlight(HighlightError::ChatParsingFailed(format!(
                "Failed to open raw chat file: {e}"
            )))
        })?;
        let mut reader = BufReader::with_capacity(64 * 1024, raw_file);

        let ndjson_file = File::create(&output_ndjson_path).map_err(|e| {
            AppError::Storage(StorageError::DirectoryCreationFailed {
                path: output_ndjson_path.display().to_string(),
                details: e.to_string(),
            })
        })?;
        let mut ndjson_writer = BufWriter::with_capacity(64 * 1024, ndjson_file);

        let mut byte_buf = Vec::with_capacity(4096);
        let mut total_messages: u64 = 0;
        let mut check_counter: u32 = 0;

        loop {
            byte_buf.clear();
            let bytes_read = match reader.read_until(b'\n', &mut byte_buf) {
                Ok(b) => b,
                Err(e) => {
                    tracing::warn!("Read error in raw chat stream: {e}");
                    break;
                }
            };
            if bytes_read == 0 {
                break;
            }

            check_counter = check_counter.wrapping_add(1);
            if check_counter.is_multiple_of(1000) && cancel_token.is_cancelled() {
                return Err(AppError::TaskCancelled);
            }

            let line_str = String::from_utf8_lossy(&byte_buf);
            if let Some(event) = parse_raw_chat_line(&line_str) {
                if let Ok(serialized) = serde_json::to_string(&event) {
                    if let Err(e) = ndjson_writer.write_all(serialized.as_bytes()) {
                        return Err(AppError::Internal(format!("Failed writing NDJSON: {e}")));
                    }
                    if let Err(e) = ndjson_writer.write_all(b"\n") {
                        return Err(AppError::Internal(format!("Failed writing NDJSON: {e}")));
                    }
                    total_messages = total_messages.saturating_add(1);
                }
            }
        }

        ndjson_writer
            .flush()
            .map_err(|e| AppError::Internal(format!("Failed to flush NDJSON writer: {e}")))?;

        // Pass 2: Assemble target Schema 5.3 `chat_events.json` from `output_ndjson_path`
        let target_file = File::create(output_json_path).map_err(|e| {
            AppError::Storage(StorageError::DirectoryCreationFailed {
                path: output_json_path.display().to_string(),
                details: e.to_string(),
            })
        })?;
        let mut json_writer = BufWriter::with_capacity(64 * 1024, target_file);

        // Write container header
        let header = format!(
            "{{\"vod_id\":{},\"total_messages\":{},\"events\":[",
            serde_json::to_string(&self.vod_id).unwrap_or_else(|_| "\"\"".to_string()),
            total_messages
        );
        json_writer
            .write_all(header.as_bytes())
            .map_err(|e| AppError::Internal(format!("Failed writing JSON header: {e}")))?;

        // Stream NDJSON lines into events array
        let ndjson_read_file = File::open(&output_ndjson_path).map_err(|e| {
            AppError::Internal(format!("Failed reading NDJSON for assembly: {e}"))
        })?;
        let mut ndjson_reader = BufReader::with_capacity(64 * 1024, ndjson_read_file);

        let mut first = true;
        loop {
            byte_buf.clear();
            let bytes_read = match ndjson_reader.read_until(b'\n', &mut byte_buf) {
                Ok(b) => b,
                Err(_) => break,
            };
            if bytes_read == 0 {
                break;
            }

            let line_str = String::from_utf8_lossy(&byte_buf);
            let trimmed = line_str.trim();
            if trimmed.is_empty() {
                continue;
            }

            if !first {
                json_writer
                    .write_all(b",")
                    .map_err(|e| AppError::Internal(format!("JSON write error: {e}")))?;
            } else {
                first = false;
            }

            json_writer
                .write_all(trimmed.as_bytes())
                .map_err(|e| AppError::Internal(format!("JSON write error: {e}")))?;
        }

        // Write footer
        json_writer
            .write_all(b"]}")
            .map_err(|e| AppError::Internal(format!("Failed writing JSON footer: {e}")))?;
        json_writer
            .flush()
            .map_err(|e| AppError::Internal(format!("Failed flushing JSON output: {e}")))?;

        Ok((total_messages, output_ndjson_path))
    }

    /// Compute 10s sliding window density heatmap from NDJSON or JSON file.
    ///
    /// Applies spam bot filter (max 3 messages per author per window),
    /// noise-floor clamp ($X < 15 \Rightarrow Z = 0$), and smoothed Z-score with $\epsilon = 0.5$.
    pub fn compute_density_heatmap(
        &self,
        chat_source_path: &Path,
        window_sec: f64,
        step_sec: f64,
    ) -> Result<ChatHeatmapResponse, AppError> {
        let win_sec = if window_sec <= 0.0 || !window_sec.is_finite() { 10.0 } else { window_sec };
        let st_sec = if step_sec <= 0.0 || !step_sec.is_finite() { win_sec } else { step_sec };

        // Accumulate raw bucket data: index -> (author -> count, swarm_count)
        struct BucketAccumulator {
            author_counts: HashMap<String, u32>,
            swarm_count: u32,
        }

        let mut buckets: HashMap<usize, BucketAccumulator> = HashMap::new();
        let mut max_bucket_idx: usize = 0;

        let mut ingest_event = |event: ChatEvent| {
            if !event.timestamp_sec.is_finite()
                || event.timestamp_sec < 0.0
                || event.timestamp_sec > MAX_CHAT_TIMESTAMP_SEC
            {
                return;
            }
            let raw_bucket_idx = (event.timestamp_sec / st_sec).floor();
            if !raw_bucket_idx.is_finite() || raw_bucket_idx < 0.0 {
                return;
            }
            let bucket_idx = raw_bucket_idx as usize;
            if bucket_idx >= MAX_HEATMAP_BUCKETS {
                return;
            }
            if bucket_idx > max_bucket_idx {
                max_bucket_idx = bucket_idx;
            }

            let entry = buckets.entry(bucket_idx).or_insert_with(|| BucketAccumulator {
                author_counts: HashMap::new(),
                swarm_count: 0,
            });

            *entry.author_counts.entry(event.author).or_insert(0) += 1;
            if event.is_swarm_emote || !event.emotes_detected.is_empty() {
                entry.swarm_count = entry.swarm_count.saturating_add(1);
            }
        };

        // Check if source file is an assembled Schema 5.3 JSON container (.json and not .ndjson)
        let is_json_doc = chat_source_path
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
            && !chat_source_path
                .file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|name| name.ends_with(".ndjson"));

        let mut parsed_as_container = false;
        if is_json_doc {
            if let Ok(file) = File::open(chat_source_path) {
                let reader = BufReader::with_capacity(64 * 1024, file);
                if let Ok(container) = serde_json::from_reader::<_, ChatEventsContainer>(reader) {
                    parsed_as_container = true;
                    for event in container.events {
                        ingest_event(event);
                    }
                }
            }
        }

        if !parsed_as_container {
            let file = File::open(chat_source_path).map_err(|e| {
                AppError::Highlight(HighlightError::ChatParsingFailed(format!(
                    "Failed to open chat file for heatmap: {e}"
                )))
            })?;
            let mut reader = BufReader::with_capacity(64 * 1024, file);
            let mut byte_buf = Vec::with_capacity(2048);

            loop {
                byte_buf.clear();
                let bytes_read = match reader.read_until(b'\n', &mut byte_buf) {
                    Ok(b) => b,
                    Err(_) => break,
                };
                if bytes_read == 0 {
                    break;
                }

                let line_str = String::from_utf8_lossy(&byte_buf);
                if let Some(event) = parse_raw_chat_line(&line_str) {
                    ingest_event(event);
                }
            }
        }

        let total_buckets = if buckets.is_empty() {
            0
        } else {
            (max_bucket_idx + 1).min(MAX_HEATMAP_BUCKETS)
        };

        if total_buckets == 0 {
            return Ok(ChatHeatmapResponse {
                vod_id: self.vod_id.clone(),
                window_seconds: win_sec,
                step_seconds: st_sec,
                total_buckets: 0,
                max_velocity_mps: 0.0,
                peak_timestamp_sec: 0.0,
                points: Vec::new(),
            });
        }

        let mut effective_counts: Vec<u32> = vec![0; total_buckets];
        let mut swarm_counts: Vec<u32> = vec![0; total_buckets];

        // Apply spam bot filter (max 3 messages per author per window) and extract counts
        for (idx, mut acc) in buckets {
            if idx < total_buckets {
                let effective: u32 = acc.author_counts.values().map(|&c| c.min(3)).sum();
                effective_counts[idx] = effective;
                swarm_counts[idx] = acc.swarm_count;
                acc.author_counts.clear(); // Free memory immediately
            }
        }

        // Calculate moving average and standard deviation over 60s baseline window
        // For step = 10s, 60s baseline corresponds to 6 buckets
        let baseline_buckets = ((60.0 / st_sec).round() as usize).max(1);

        let mut max_velocity_mps: f64 = 0.0;
        let mut peak_timestamp_sec: f64 = 0.0;
        let mut raw_points: Vec<ChatDensityPoint> = Vec::with_capacity(total_buckets);

        for i in 0..total_buckets {
            let count = effective_counts[i];
            let swarm = swarm_counts[i];
            let timestamp_sec = (i as f64) * st_sec;
            let velocity_mps = (count as f64) / win_sec;

            if velocity_mps > max_velocity_mps {
                max_velocity_mps = velocity_mps;
                peak_timestamp_sec = timestamp_sec;
            }

            let swarm_ratio = if count > 0 {
                (swarm as f64) / (count as f64)
            } else {
                0.0
            };

            // Noise-floor guardrail (AC-09.3 / UT-09.1): if X(t) < 15, Z = 0.0
            let z_score = if count < 15 {
                0.0
            } else {
                let start_idx = i.saturating_sub(baseline_buckets);
                let window_slice = &effective_counts[start_idx..=i];
                let n = window_slice.len() as f64;
                let sum: f64 = window_slice.iter().map(|&c| c as f64).sum();
                let mean = sum / n;
                let variance: f64 = window_slice
                    .iter()
                    .map(|&c| {
                        let diff = (c as f64) - mean;
                        diff * diff
                    })
                    .sum::<f64>()
                    / n;
                let std_dev = variance.sqrt();
                // Smoothed Z-score with epsilon = 0.5 (UT-09.2) to prevent division by zero
                ((count as f64) - mean) / (std_dev + 0.5)
            };

            let is_peak = z_score >= 2.0 && count >= 15;

            raw_points.push(ChatDensityPoint {
                timestamp_sec,
                message_count: count,
                velocity_mps,
                swarm_count: swarm,
                swarm_ratio,
                z_score,
                normalized_density: 0.0, // calculated in second pass
                is_peak,
            });
        }

        // Normalize density to [0.0, 1.0]
        for pt in &mut raw_points {
            pt.normalized_density = if max_velocity_mps > 0.0 {
                pt.velocity_mps / max_velocity_mps
            } else {
                0.0
            };
        }

        Ok(ChatHeatmapResponse {
            vod_id: self.vod_id.clone(),
            window_seconds: win_sec,
            step_seconds: st_sec,
            total_buckets,
            max_velocity_mps,
            peak_timestamp_sec,
            points: raw_points,
        })
    }
}
