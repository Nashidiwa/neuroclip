//! VAD (Voice Activity Detection) segmentation bridge.
//!
//! Aligns transcript segments with natural speech pauses, snaps boundaries within
//! $\pm 250$ms tolerance, and splits segments across pause gaps exceeding 1.0 second.

use serde::{Deserialize, Serialize};

use crate::stt::transcript_parser::{
    calculate_cps, TranscriptSegment, WordTiming, CONFIDENCE_FLAG_THRESHOLD, CPS_FLAG_THRESHOLD,
};

/// Speech activity interval defined by start and end timestamps in seconds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpeechInterval {
    pub start_sec: f64,
    pub end_sec: f64,
}

/// Boundary snapping tolerance in seconds (250ms).
pub const BOUNDARY_SNAP_TOLERANCE_SEC: f64 = 0.25;

/// Word pause gap threshold triggering segment splitting (1.0 second).
pub const WORD_GAP_SPLIT_THRESHOLD_SEC: f64 = 1.0;

/// Align transcript segments with Silero VAD speech activity intervals.
///
/// 1. Snaps segment start/end boundaries if they lie within $\pm 250$ms of a VAD boundary.
/// 2. Splits segments where word-to-word silence exceeds 1.0s into discrete natural phrases.
pub fn align_segments_with_vad(
    segments: &[TranscriptSegment],
    intervals: &[SpeechInterval],
) -> Vec<TranscriptSegment> {
    let mut aligned_segments = Vec::with_capacity(segments.len() * 2);

    for seg in segments {
        let mut snapped_start = seg.start_sec;
        let mut snapped_end = seg.end_sec;

        // Boundary snapping
        for interval in intervals {
            if (seg.start_sec - interval.start_sec).abs() <= BOUNDARY_SNAP_TOLERANCE_SEC {
                snapped_start = interval.start_sec;
            }
            if (seg.end_sec - interval.end_sec).abs() <= BOUNDARY_SNAP_TOLERANCE_SEC {
                snapped_end = interval.end_sec;
            }
        }
        snapped_end = snapped_end.max(snapped_start);

        // Check for pause gaps between words to split into natural sub-segments
        if seg.words.len() > 1 {
            let mut chunk_words: Vec<WordTiming> = Vec::new();
            let mut sub_index = 1;

            for i in 0..seg.words.len() {
                let current_word = match seg.words.get(i) {
                    Some(w) => w.clone(),
                    None => continue,
                };

                let should_split = if let Some(prev_word) = chunk_words.last() {
                    current_word.start_sec - prev_word.end_sec > WORD_GAP_SPLIT_THRESHOLD_SEC
                } else {
                    false
                };

                if should_split && !chunk_words.is_empty() {
                    if let Some(sub_seg) = build_subsegment(seg, &chunk_words, sub_index) {
                        aligned_segments.push(sub_seg);
                        sub_index += 1;
                    }
                    chunk_words.clear();
                }

                chunk_words.push(current_word);
            }

            if !chunk_words.is_empty() {
                if sub_index > 1 {
                    if let Some(sub_seg) = build_subsegment(seg, &chunk_words, sub_index) {
                        aligned_segments.push(sub_seg);
                    }
                } else {
                    // No internal splits occurred
                    let mut updated = seg.clone();
                    updated.start_sec = snapped_start;
                    updated.end_sec = snapped_end;
                    aligned_segments.push(updated);
                }
            }
        } else {
            let mut updated = seg.clone();
            updated.start_sec = snapped_start;
            updated.end_sec = snapped_end;
            aligned_segments.push(updated);
        }
    }

    aligned_segments
}

/// Helper to build a child segment resulting from a word gap split.
fn build_subsegment(
    parent: &TranscriptSegment,
    words: &[WordTiming],
    index: usize,
) -> Option<TranscriptSegment> {
    let first = words.first()?;
    let last = words.last()?;

    let start_sec = first.start_sec;
    let end_sec = last.end_sec.max(start_sec);

    let text_en = words
        .iter()
        .map(|w| w.word.as_str())
        .collect::<Vec<&str>>()
        .join(" ");

    let confidence = if !words.is_empty() {
        let sum: f64 = words.iter().map(|w| w.probability).sum();
        (sum / words.len() as f64).clamp(0.0, 1.0)
    } else {
        parent.confidence
    };

    let cps_score = calculate_cps(&text_en, start_sec, end_sec);
    let is_flagged_for_review =
        confidence < CONFIDENCE_FLAG_THRESHOLD || cps_score > CPS_FLAG_THRESHOLD;

    Some(TranscriptSegment {
        segment_id: format!("{}_{index}", parent.segment_id),
        start_sec,
        end_sec,
        speaker_id: parent.speaker_id.clone(),
        text_en,
        text_vi: None,
        confidence,
        cps_score,
        is_flagged_for_review,
        words: words.to_vec(),
    })
}
