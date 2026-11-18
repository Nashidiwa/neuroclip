//! Adjusted Chat Velocity Z-Score Algorithm with Message Floor.
//!
//! Implements Schema 5.3 and Section 4.3 highlight detection scoring:
//! 1. 5-second time resolution
//! 2. 60-second sliding background window (12 buckets of 5s)
//! 3. Message floor threshold ($X(t) < 15 \implies Z_{\text{adj}}(t) = 0.0$)
//! 4. Smoothing constant $\epsilon = 0.5$:
//!    $$Z_{\text{adj}}(t) = \frac{X(t) - \mu_W(t)}{\sigma_W(t) + 0.5}$$
//! 5. $O(N)$ running cumulative prefix sums with integer accumulators to prevent float drift
//!
//! Complies with `docs/CONVENTIONS.md` (zero-panic invariant, Result<T, AppError>)
//! and `docs/TASK_BOARD.md` §TASK-M4-09.

#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, HighlightError};
use crate::highlight::bucket::{
    bin_chat_events, bin_chat_events_from_file, ChatBucket, DEFAULT_BUCKET_WIDTH_SEC,
};
use crate::ingestion::ChatEvent;

/// Default baseline lookback window in seconds (60.0s = 12 buckets of 5s).
pub const DEFAULT_BASELINE_WINDOW_SEC: f64 = 60.0;

/// Smoothing constant $\epsilon = 0.5$ in the denominator to avoid division by zero (ADR-004 §1).
pub const SMOOTHING_EPSILON: f64 = 0.5;

/// Minimum message count floor required to compute non-zero Z-score (AC-09.3, UT-09.1).
pub const MESSAGE_FLOOR_THRESHOLD: u32 = 15;

/// Output point for the 5-second chat velocity series (`docs/TASK_BOARD.md` §Data Contracts).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatVelocityPoint {
    /// Timestamp in seconds from VOD start ($t_k = k \times 5.0\text{s}$).
    pub timestamp_sec: f64,
    /// Effective message count in this 5-second bucket ($X(t)$).
    pub count: u32,
    /// Adjusted Z-score with message floor and smoothing ($Z_{\text{adj}}(t)$).
    pub z_score: f64,
}

impl ChatVelocityPoint {
    /// Construct a new `ChatVelocityPoint`.
    #[must_use]
    pub const fn new(timestamp_sec: f64, count: u32, z_score: f64) -> Self {
        Self {
            timestamp_sec,
            count,
            z_score,
        }
    }
}

/// Calculate the adjusted Z-Score for a single bucket count with floor gating.
///
/// If `count < floor_threshold`, returns `0.0` immediately.
/// Otherwise, evaluates:
/// $$Z_{\text{adj}} = \frac{\text{count} - \mu}{\sigma + \epsilon}$$
#[must_use]
pub fn calculate_adjusted_zscore(
    count: u32,
    mean: f64,
    std_dev: f64,
    epsilon: f64,
    floor_threshold: u32,
) -> f64 {
    // Message floor gate (AC-09.3, UT-09.1)
    if count < floor_threshold {
        return 0.0;
    }

    let denom = std_dev + epsilon;
    if denom <= 0.0 || denom.is_nan() {
        return 0.0;
    }

    let numerator = f64::from(count) - mean;
    let z = numerator / denom;

    if z.is_nan() || z.is_infinite() {
        0.0
    } else {
        z
    }
}

/// Convenience wrapper for `calculate_adjusted_zscore` using default epsilon (0.5) and floor (15).
#[must_use]
pub fn compute_zscore(count: u32, mean: f64, std_dev: f64) -> f64 {
    calculate_adjusted_zscore(
        count,
        mean,
        std_dev,
        SMOOTHING_EPSILON,
        MESSAGE_FLOOR_THRESHOLD,
    )
}

/// Compute the adjusted chat velocity series over pre-binned chat buckets.
///
/// Uses an $O(N)$ running cumulative prefix sum algorithm ($\sum X$ and $\sum X^2$)
/// with 64-bit and 128-bit integer accumulators to guarantee $O(1)$ mean and variance
/// computation per bucket and prevent floating-point drift.
///
/// # Arguments
/// * `buckets` - Slice of continuous time buckets (5-second width).
/// * `bucket_width_sec` - Width of each bucket in seconds (default 5.0).
/// * `baseline_window_sec` - Baseline background window in seconds (default 60.0).
///
/// # Errors
/// Returns `AppError::Highlight(HighlightError::ChatParsingFailed)` if parameters are non-finite or non-positive.
pub fn compute_chat_velocity_series(
    buckets: &[ChatBucket],
    bucket_width_sec: f64,
    baseline_window_sec: f64,
) -> Result<Vec<ChatVelocityPoint>, AppError> {
    if !bucket_width_sec.is_finite() || bucket_width_sec <= 0.0 {
        return Err(AppError::Highlight(HighlightError::ChatParsingFailed(
            format!("Invalid bucket width: {bucket_width_sec}s"),
        )));
    }

    if !baseline_window_sec.is_finite() || baseline_window_sec <= 0.0 {
        return Err(AppError::Highlight(HighlightError::ChatParsingFailed(
            format!("Invalid baseline window: {baseline_window_sec}s"),
        )));
    }

    let n = buckets.len();
    if n == 0 {
        return Ok(Vec::new());
    }

    // Number of background lookback buckets (e.g. 60s / 5s = 12)
    let lookback_k = (baseline_window_sec / bucket_width_sec).round().max(1.0) as usize;

    // Build cumulative prefix sums with integer accumulators:
    // P[i] = sum(X[0..i-1]), S[i] = sum(X[0..i-1]^2)
    let mut prefix_sum: Vec<u64> = Vec::with_capacity(n.saturating_add(1));
    let mut prefix_sum_sq: Vec<u128> = Vec::with_capacity(n.saturating_add(1));
    prefix_sum.push(0);
    prefix_sum_sq.push(0);

    let mut running_sum: u64 = 0;
    let mut running_sum_sq: u128 = 0;

    for bucket in buckets {
        let x = u64::from(bucket.effective_count);
        running_sum = running_sum.saturating_add(x);
        let x_u128 = u128::from(x);
        running_sum_sq = running_sum_sq.saturating_add(x_u128 * x_u128);

        prefix_sum.push(running_sum);
        prefix_sum_sq.push(running_sum_sq);
    }

    // Single-pass sliding lookback window evaluation: O(1) per bucket
    let mut series = Vec::with_capacity(n);

    for i in 0..n {
        let start = i.saturating_sub(lookback_k);
        let m = i - start;

        let (mean, std_dev) = if m == 0 {
            // Cold start at the beginning of stream (k=0)
            (0.0, 0.0)
        } else {
            let sum = prefix_sum[i].saturating_sub(prefix_sum[start]);
            let sum_sq = prefix_sum_sq[i].saturating_sub(prefix_sum_sq[start]);
            let m_f = m as f64;
            let mean = (sum as f64) / m_f;
            let mean_sq = (sum_sq as f64) / m_f;
            let variance = (mean_sq - mean * mean).max(0.0);
            (mean, variance.sqrt())
        };

        let count = buckets[i].effective_count;
        let z_score = calculate_adjusted_zscore(
            count,
            mean,
            std_dev,
            SMOOTHING_EPSILON,
            MESSAGE_FLOOR_THRESHOLD,
        );

        series.push(ChatVelocityPoint {
            timestamp_sec: buckets[i].timestamp_sec,
            count,
            z_score,
        });
    }

    Ok(series)
}

/// Convenience function using standard default parameters (5.0s bucket, 60.0s baseline window).
pub fn compute_chat_velocity_series_default(
    buckets: &[ChatBucket],
) -> Result<Vec<ChatVelocityPoint>, AppError> {
    compute_chat_velocity_series(
        buckets,
        DEFAULT_BUCKET_WIDTH_SEC,
        DEFAULT_BASELINE_WINDOW_SEC,
    )
}

/// End-to-end calculation directly from a slice of `ChatEvent`s.
pub fn compute_chat_velocity_series_from_events(
    events: &[ChatEvent],
    bucket_width_sec: f64,
    baseline_window_sec: f64,
    vod_duration_sec: Option<f64>,
) -> Result<Vec<ChatVelocityPoint>, AppError> {
    let buckets = bin_chat_events(events, bucket_width_sec, vod_duration_sec)?;
    compute_chat_velocity_series(&buckets, bucket_width_sec, baseline_window_sec)
}

/// End-to-end calculation directly from a chat log file on disk (JSON or NDJSON).
pub fn compute_chat_velocity_series_from_file<P: AsRef<Path>>(
    file_path: P,
    bucket_width_sec: f64,
    baseline_window_sec: f64,
    vod_duration_sec: Option<f64>,
) -> Result<Vec<ChatVelocityPoint>, AppError> {
    let buckets = bin_chat_events_from_file(file_path, bucket_width_sec, vod_duration_sec)?;
    compute_chat_velocity_series(&buckets, bucket_width_sec, baseline_window_sec)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
pub mod tests {
    use super::*;
    use crate::highlight::bucket::bin_chat_events_default;

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

    /// UT-09.1: Message floor threshold clamp ($X < 15 \implies Z_{\text{adj}} = 0.0$).
    #[test]
    fn test_ut_09_1_message_floor() {
        // Under threshold: 14 messages
        let z1 = calculate_adjusted_zscore(14, 0.0, 0.0, 0.5, 15);
        assert_eq!(z1, 0.0, "X=14 must yield Z=0.0 even with mu=0, sigma=0");

        let z2 = calculate_adjusted_zscore(14, 1.0, 0.2, 0.5, 15);
        assert_eq!(z2, 0.0, "X=14 must yield Z=0.0 with non-zero baseline");

        let z3 = calculate_adjusted_zscore(0, 0.0, 0.0, 0.5, 15);
        assert_eq!(z3, 0.0, "X=0 must yield Z=0.0");

        // Threshold boundary: 15 messages (floor is strictly < 15, so 15 evaluates)
        let z_boundary = calculate_adjusted_zscore(15, 10.0, 2.0, 0.5, 15);
        assert!((z_boundary - 2.0).abs() < 1e-6); // (15 - 10) / (2.0 + 0.5) = 5 / 2.5 = 2.0
    }

    /// UT-09.2: Smoothing constant $\epsilon = 0.5$ and zero variance background.
    /// $X(t) = 45$, $\mu = 10.0$, $\sigma = 0.0 \implies Z = \frac{45 - 10}{0.0 + 0.5} = 70.0$.
    #[test]
    fn test_ut_09_2_smoothing_epsilon() {
        let z = calculate_adjusted_zscore(45, 10.0, 0.0, 0.5, 15);
        assert!(
            (z - 70.0).abs() < 1e-6,
            "Expected Z=70.0 for X=45, mu=10.0, sigma=0.0, epsilon=0.5; got {z}"
        );

        // Also verify via full series sliding window
        // Create 12 background buckets with 10 messages each (indices 0..11)
        // Followed by bucket 12 with 45 messages.
        let mut buckets = Vec::new();
        for i in 0..12 {
            buckets.push(ChatBucket::new(i, (i as f64) * 5.0, 10, 10));
        }
        buckets.push(ChatBucket::new(12, 60.0, 45, 45));

        let series = compute_chat_velocity_series_default(&buckets).unwrap();
        assert_eq!(series.len(), 13);

        let pt = &series[12];
        assert_eq!(pt.count, 45);
        assert!(
            (pt.z_score - 70.0).abs() < 1e-6,
            "Sliding window bucket 12 expected Z=70.0, got {}",
            pt.z_score
        );
    }

    /// UT-09.3: Spam bot author rate limiting: 20 messages by 1 author $\implies \le 3$ effective $\implies Z = 0.0$.
    #[test]
    fn test_ut_09_3_spambot_filter() {
        let mut events = Vec::new();
        for i in 0..20 {
            events.push(make_event(
                1.0 + f64::from(i) * 0.1,
                "bad_spambot",
                "CLICK HERE",
            ));
        }

        let buckets = bin_chat_events_default(&events, None).unwrap();
        assert_eq!(buckets[0].raw_count, 20);
        assert_eq!(buckets[0].effective_count, 3);

        let series = compute_chat_velocity_series_default(&buckets).unwrap();
        assert_eq!(series[0].count, 3);
        assert_eq!(
            series[0].z_score, 0.0,
            "Spam bot flood capped at 3 must fall below floor 15 and produce Z=0.0"
        );
    }

    /// IT-09.1: Full series 5-second resolution across VOD.
    #[test]
    fn test_it_09_1_chat_velocity_series_5s_resolution() {
        let vod_dur = 300.0; // 5 minutes = 60 buckets
        let mut events = Vec::new();

        // Background banter
        for b in 0..60 {
            for a in 0..5 {
                events.push(make_event(
                    f64::from(b) * 5.0 + 1.0,
                    &format!("user_{a}"),
                    "chatting",
                ));
            }
        }

        // Spike at minute 2 (bucket 24, t = 120.0s): 10 authors sending 3 messages each = 30 messages
        for a in 0..10 {
            for m in 0..3 {
                events.push(make_event(
                    120.0 + f64::from(m) * 0.5,
                    &format!("spike_user_{a}"),
                    "LETS GOOO",
                ));
            }
        }

        let series = compute_chat_velocity_series_from_events(
            &events,
            DEFAULT_BUCKET_WIDTH_SEC,
            DEFAULT_BASELINE_WINDOW_SEC,
            Some(vod_dur),
        )
        .unwrap();

        assert_eq!(
            series.len(),
            60,
            "Must have exactly 60 buckets for 300s VOD"
        );

        // Verify timestamps are strictly spaced by 5.0s
        for (i, pt) in series.iter().enumerate() {
            let expected_ts = (i as f64) * 5.0;
            assert!(
                (pt.timestamp_sec - expected_ts).abs() < 1e-6,
                "Bucket {i} timestamp mismatch: expected {expected_ts}, got {}",
                pt.timestamp_sec
            );
        }

        // Background buckets had 5 messages < 15 -> z_score must be 0.0
        assert_eq!(series[0].count, 5);
        assert_eq!(series[0].z_score, 0.0);

        // Spike bucket had 35 messages >= 15 -> z_score must be positive and significant
        let spike_pt = &series[24];
        assert_eq!(spike_pt.count, 35);
        assert!(
            spike_pt.z_score > 10.0,
            "Spike Z-score should be high, got {}",
            spike_pt.z_score
        );
    }

    /// ED-09.1: Benchmark 500,000 messages processed in $\le 1.2\text{s}$.
    #[test]
    fn test_ed_09_1_performance_500k_messages() {
        // Generate 500,000 events across 5,760 buckets (~8 hours of VOD)
        const TOTAL_EVENTS: usize = 500_000;
        let mut events = Vec::with_capacity(TOTAL_EVENTS);

        for i in 0..TOTAL_EVENTS {
            let ts = ((i % 5760) as f64) * 5.0 + 0.1 * ((i % 40) as f64);
            let author = format!("viewer_{}", i % 500);
            events.push(make_event(ts, &author, "Neuro is so smart"));
        }

        let start_time = std::time::Instant::now();

        let buckets = bin_chat_events_default(&events, Some(28800.0)).unwrap();
        let series = compute_chat_velocity_series_default(&buckets).unwrap();

        let elapsed = start_time.elapsed();
        println!(
            "ED-09.1: Processed {} messages into {} buckets in {:?}",
            TOTAL_EVENTS,
            series.len(),
            elapsed
        );

        assert_eq!(series.len(), 5760);
        assert!(
            elapsed.as_millis() <= 1200,
            "ED-09.1 SLA failed: took {elapsed:?} (limit 1.2s)"
        );
    }

    /// Cold start behavior at stream beginning (i=0)
    #[test]
    fn test_cold_start_stream_beginning() {
        let buckets = vec![
            ChatBucket::new(0, 0.0, 30, 30),
            ChatBucket::new(1, 5.0, 10, 10),
        ];

        let series = compute_chat_velocity_series_default(&buckets).unwrap();
        assert_eq!(series.len(), 2);

        // At i=0, M=0 -> mu=0.0, sigma=0.0 -> denom=0.5 -> Z = (30 - 0) / 0.5 = 60.0
        assert!(
            (series[0].z_score - 60.0).abs() < 1e-6,
            "Cold start Z expected 60.0, got {}",
            series[0].z_score
        );

        // At i=1, count=10 < 15 -> Z = 0.0
        assert_eq!(series[1].z_score, 0.0);
    }
}
