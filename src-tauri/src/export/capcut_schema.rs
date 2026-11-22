//! CapCut PC Draft JSON Schema definitions.
//!
//! Provides strong-typed serialization models for `draft_content.json`, `draft_meta_info.json`,
//! and `root_meta_info.json`, supporting multi-track timelines (video, audio, text),
//! materials catalog, normalized coordinate transforms, and microsecond timeranges (1s = 1,000,000us).

use crate::error::{AppError, ExportError};
use serde::{Deserialize, Serialize};

/// 1 second = 1,000,000 microseconds in CapCut timeline.
pub const MICROS_PER_SEC: f64 = 1_000_000.0;

/// Minimum duration in microseconds (approx 1 frame at 30fps) to prevent timeline clip dropping.
pub const MIN_CLIP_DURATION_MICROS: i64 = 33_333;

/// Default bottom-centered vertical transform coordinate in normalized canvas.
pub const DEFAULT_SUBTITLE_TRANSFORM_Y: f64 = -0.8;

/// Default centered horizontal transform coordinate in normalized canvas.
pub const DEFAULT_SUBTITLE_TRANSFORM_X: f64 = 0.0;

/// Convert seconds to CapCut timeline microseconds.
#[must_use]
pub fn sec_to_micros(sec: f64) -> i64 {
    if sec.is_finite() && sec >= 0.0 {
        #[allow(clippy::cast_possible_truncation)]
        let micros = (sec * MICROS_PER_SEC).round() as i64;
        micros
    } else {
        0
    }
}

/// Convert hex color string (e.g. `"#FFB6C1"`) to normalized RGB float components `[0.0 .. 1.0]`.
#[must_use]
pub fn hex_to_normalized_rgb(hex: &str) -> [f64; 3] {
    let clean = hex.trim().trim_start_matches('#');
    if clean.len() != 6 {
        return [1.0, 1.0, 1.0];
    }
    let r = f64::from(u8::from_str_radix(&clean[0..2], 16).unwrap_or(255)) / 255.0;
    let g = f64::from(u8::from_str_radix(&clean[2..4], 16).unwrap_or(255)) / 255.0;
    let b = f64::from(u8::from_str_radix(&clean[4..6], 16).unwrap_or(255)) / 255.0;
    [r, g, b]
}

/// Canvas configuration defining canvas dimensions and aspect ratio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanvasConfig {
    pub width: u32,
    pub height: u32,
    pub ratio: String,
}

/// Validate and resolve canvas configuration from target layout string.
///
/// # Errors
/// Returns [`AppError::Export`] with [`ExportError::InvalidLayout`] if layout is unsupported.
pub fn canvas_config_for_layout(layout: &str) -> Result<CanvasConfig, AppError> {
    match layout {
        "original_16_9" => Ok(CanvasConfig {
            width: 1920,
            height: 1080,
            ratio: "16:9".to_string(),
        }),
        "preset_center_crop_9_16" | "preset_two_stack_9_16" => Ok(CanvasConfig {
            width: 1080,
            height: 1920,
            ratio: "9:16".to_string(),
        }),
        other => Err(AppError::Export(ExportError::InvalidLayout(
            other.to_string(),
        ))),
    }
}

/// Time range in microseconds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: i64,
    pub duration: i64,
}

/// Video material entry in `draft_content.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DraftVideoMaterial {
    pub id: String,
    pub path: String,
    pub duration: i64,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub material_name: String,
}

/// Audio material entry in `draft_content.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DraftAudioMaterial {
    pub id: String,
    pub path: String,
    pub duration: i64,
    #[serde(default)]
    pub material_name: String,
}

/// Text material entry in `draft_content.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DraftTextMaterial {
    pub id: String,
    pub content: String,
    pub font_size: f64,
    pub text_color: String,
    pub transform_x: f64,
    pub transform_y: f64,
    pub alignment: i32,
    pub typesetting: i32,
    pub check_flag: i32,
}

/// Container for all materials catalog in `draft_content.json`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DraftMaterials {
    pub videos: Vec<DraftVideoMaterial>,
    pub audios: Vec<DraftAudioMaterial>,
    pub texts: Vec<DraftTextMaterial>,
}

/// Segment clip on a timeline track.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DraftSegment {
    pub id: String,
    pub material_id: String,
    pub target_timerange: TimeRange,
    pub source_timerange: TimeRange,
    pub render_index: i32,
    #[serde(default)]
    pub extra_material_refs: Vec<String>,
}

/// Timeline track (Video, Audio, Text).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DraftTrack {
    pub id: String,
    #[serde(rename = "type")]
    pub track_type: String,
    pub attribute: i32,
    pub flag: i32,
    pub segments: Vec<DraftSegment>,
}

/// Root structure of `draft_content.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DraftContent {
    pub id: String,
    pub name: String,
    pub fps: f64,
    pub duration: i64,
    pub version: i32,
    pub canvas_config: CanvasConfig,
    pub materials: DraftMaterials,
    pub tracks: Vec<DraftTrack>,
}

/// Material item registered in `draft_meta_info.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DraftMetaMaterialItem {
    pub id: String,
    #[serde(rename = "file_Path")]
    pub file_path: String,
    pub metetype: String,
    pub duration: i64,
    pub width: u32,
    pub height: u32,
    pub extra_info: String,
    #[serde(rename = "type")]
    pub material_type: i32,
}

/// Materials container in `draft_meta_info.json`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DraftMetaMaterials {
    pub videos: Vec<serde_json::Value>,
    pub audios: Vec<serde_json::Value>,
    pub texts: Vec<serde_json::Value>,
    pub draft_materials: Vec<DraftMetaMaterialItem>,
}

/// Root structure of `draft_meta_info.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DraftMetaInfo {
    pub draft_id: String,
    pub draft_name: String,
    pub draft_fold_path: String,
    pub draft_root_path: String,
    pub draft_type: i32,
    pub tm_draft_create: i64,
    pub tm_draft_modify: i64,
    pub tm_duration: i64,
    pub draft_cover: String,
    pub draft_materials: DraftMetaMaterials,
}

/// Single draft entry inside `root_meta_info.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftStoreItem {
    pub draft_id: String,
    pub draft_name: String,
    pub draft_fold_path: String,
    pub draft_root_path: String,
    pub tm_draft_create: i64,
    pub tm_draft_modify: i64,
    #[serde(default)]
    pub draft_type: i32,
}

/// Root structure of `root_meta_info.json` for home screen draft indexing.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RootMetaInfo {
    #[serde(default)]
    pub all_draft_store: Vec<DraftStoreItem>,
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::cast_precision_loss,
    clippy::cast_lossless
)]
mod tests {
    use super::*;

    #[test]
    fn test_sec_to_micros() {
        assert_eq!(sec_to_micros(0.0), 0);
        assert_eq!(sec_to_micros(1.0), 1_000_000);
        assert_eq!(sec_to_micros(2.5), 2_500_000);
        assert_eq!(sec_to_micros(-1.0), 0);
        assert_eq!(sec_to_micros(f64::NAN), 0);
    }

    #[test]
    fn test_hex_to_normalized_rgb() {
        // Pure White
        let white = hex_to_normalized_rgb("#FFFFFF");
        assert!((white[0] - 1.0).abs() < 1e-4);
        assert!((white[1] - 1.0).abs() < 1e-4);
        assert!((white[2] - 1.0).abs() < 1e-4);

        // Neuro Pink (#FFB6C1)
        let pink = hex_to_normalized_rgb("#FFB6C1");
        assert!((pink[0] - (255.0 / 255.0)).abs() < 1e-4);
        assert!((pink[1] - (182.0 / 255.0)).abs() < 1e-4);
        assert!((pink[2] - (193.0 / 255.0)).abs() < 1e-4);

        // Fallback for invalid hex
        let fallback = hex_to_normalized_rgb("invalid");
        assert_eq!(fallback, [1.0, 1.0, 1.0]);
    }

    #[test]
    fn test_canvas_config_for_layout() {
        let original = canvas_config_for_layout("original_16_9").unwrap();
        assert_eq!(original.width, 1920);
        assert_eq!(original.height, 1080);
        assert_eq!(original.ratio, "16:9");

        let crop = canvas_config_for_layout("preset_center_crop_9_16").unwrap();
        assert_eq!(crop.width, 1080);
        assert_eq!(crop.height, 1920);
        assert_eq!(crop.ratio, "9:16");

        let stack = canvas_config_for_layout("preset_two_stack_9_16").unwrap();
        assert_eq!(stack.width, 1080);
        assert_eq!(stack.height, 1920);
        assert_eq!(stack.ratio, "9:16");

        let err = canvas_config_for_layout("invalid_aspect");
        assert!(err.is_err());
    }
}
