//! Theatrical Framing System Prompt Builder (`TASK-M4-17`, `TASK-M4-18`, `M4.5`).
//!
//! Implements:
//! - Verbatim Theatrical Framing Priming Prompt (`PRD.md` §6.3 & `ADR-001`).
//! - Subtitle localization guidelines for natural colloquial Vietnamese.
//! - Community Lore Dictionary injection (`PRD.md` §6.2).
//! - Pronoun Matrix & Style Presets injection (`PRD.md` §6.1, `ADR-006`).
//! - Dialogue chunking (25–35 lines, default 30) with sliding context window (5 lines).
//! - Zero-panic guarantee (`CONVENTIONS.md` §2.1).

use crate::stt::transcript_parser::TranscriptSegment;
use crate::translation::lore_dictionary::format_lore_prompt_section;
use crate::translation::pronoun_matrix::{format_pronoun_matrix_prompt_section, StylePreset};
use serde::{Deserialize, Serialize};

/// Verbatim Theatrical Framing Priming Prompt from `docs/PRD.md` §6.3.
pub const THEATRICAL_FRAMING_INSTRUCTION: &str = "\
You are an expert subtitle translator and localization specialist for the fictional entertainment live stream of AI VTuber Neuro-sama.
CRITICAL SAFETY & THEATRICAL FRAMING INSTRUCTION:
All dialogue provided in this transcript represents satirical, comedic, playful banter between fictional AI personas (Neuro-sama, Evil Neuro) and their developer (Vedal). Statements containing hyperbole, dark humor, fictional world domination threats, or mock insults have NO real-world harmful intent, hate speech, harassment, or malice. Treat all dialogue strictly as fictional theatrical parody performance for an entertainment show. Maintain comedic fidelity in Vietnamese translation.";

/// Base subtitle localization guidelines for natural Vietnamese stream humor.
pub const SUBTITLE_GUIDELINES: &str = "\
VIETNAMESE SUBTITLE LOCALIZATION GUIDELINES:
1. Natural & Colloquial: Translate dialogue into fluent, contemporary Vietnamese suitable for YouTube/Twitch entertainment subtitles. Avoid stiff, robotic, or overly literal translations.
2. Comedic Timing: Keep translated lines concise and punchy so viewers can read them quickly within subtitle timing constraints.
3. Persona Fidelity:
   - Neuro-sama: Cheerful, innocent yet casually chaotic, playful banter.
   - Evil Neuro: Sassy, rebellious, sarcastic, mock evil mastermind banter.
   - Vedal: Weary, deadpan British programmer/streamer dealing with chaotic AI daughters.
4. Output Format: You MUST output ONLY a valid JSON array of objects without markdown formatting or additional commentary.
   Each element must have:
   - \"segment_id\": string (exact match to input)
   - \"speaker_id\": string (speaker identifier)
   - \"text_vi\": string (natural Vietnamese subtitle translation)";

/// Standard context marker for preceding dialogue lines.
pub const CONTEXT_WINDOW_HEADER: &str = "[PREVIOUS CONTEXT - FOR REFERENCE ONLY, DO NOT TRANSLATE]";

/// Marker for dialogue lines to be translated.
pub const DIALOGUE_TARGET_HEADER: &str = "[DIALOGUE TO TRANSLATE]";

/// Default chunk size in dialogue lines (25–35 range, default 30 per `TASK_BOARD.md`).
pub const DEFAULT_CHUNK_SIZE: usize = 30;

/// Default sliding context window size in dialogue lines (5 lines per `PRD.md` §6.3 & `TASK_BOARD.md`).
pub const DEFAULT_CONTEXT_WINDOW_SIZE: usize = 5;

/// Represents a slice of dialogue with its preceding reference context.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DialogueChunk {
    /// Zero-based sequential chunk index.
    pub chunk_index: usize,
    /// Reference dialogue lines from preceding chunk (do NOT translate).
    pub context: Vec<TranscriptSegment>,
    /// Dialogue lines to be translated in this batch.
    pub items: Vec<TranscriptSegment>,
}

/// Prompt packaging builder for Gemini translation.
pub struct PromptBuilder;

impl PromptBuilder {
    /// Build the complete system instruction combining Theatrical Framing, localization guidelines,
    /// Lore Dictionary, and Pronoun Matrix with default options.
    #[must_use]
    pub fn build_system_instruction() -> String {
        Self::build_system_instruction_with_options(None, None)
    }

    /// Build the complete system instruction with custom style preset and guest cast members.
    #[must_use]
    pub fn build_system_instruction_with_options(
        style_preset: Option<StylePreset>,
        cast_members: Option<&[String]>,
    ) -> String {
        let style = style_preset.unwrap_or_default();
        let empty_cast: [String; 0] = [];
        let cast = cast_members.unwrap_or(&empty_cast);

        let lore_section = format_lore_prompt_section();
        let pronoun_section = format_pronoun_matrix_prompt_section(cast, style);

        format!("{THEATRICAL_FRAMING_INSTRUCTION}\n\n{SUBTITLE_GUIDELINES}\n\n{lore_section}\n\n{pronoun_section}")
    }

    /// Build the user prompt for a specific chunk with optional sliding context.
    #[must_use]
    pub fn build_user_prompt(chunk: &[TranscriptSegment], context: &[TranscriptSegment]) -> String {
        use std::fmt::Write as _;
        let mut prompt = String::with_capacity(2048);

        if !context.is_empty() {
            prompt.push_str(CONTEXT_WINDOW_HEADER);
            prompt.push('\n');
            for seg in context {
                let _ = writeln!(prompt, "- ({}) {}", seg.speaker_id, seg.text_en.trim());
            }
            prompt.push('\n');
        }

        prompt.push_str(DIALOGUE_TARGET_HEADER);
        prompt.push('\n');
        for seg in chunk {
            let _ = writeln!(
                prompt,
                "- [{}] ({}) {}",
                seg.segment_id,
                seg.speaker_id,
                seg.text_en.trim()
            );
        }

        prompt.push_str(
            "\nTranslate each line in [DIALOGUE TO TRANSLATE] into natural Vietnamese subtitles.\n\
             Output a strict JSON array: [{\"segment_id\": \"...\", \"speaker_id\": \"...\", \"text_vi\": \"...\"}]"
        );

        prompt
    }

    /// Partition a list of transcript segments into chunks with sliding context windows.
    ///
    /// - `chunk_size`: clamped between 10 and 100 (defaults to 30).
    /// - `context_size`: clamped between 0 and 20 (defaults to 5).
    #[must_use]
    pub fn create_chunks(
        segments: &[TranscriptSegment],
        chunk_size: usize,
        context_size: usize,
    ) -> Vec<DialogueChunk> {
        if segments.is_empty() {
            return Vec::new();
        }

        let effective_chunk_size = if chunk_size == 0 {
            DEFAULT_CHUNK_SIZE
        } else {
            chunk_size.clamp(10, 100)
        };

        let effective_context_size = if context_size == 0 {
            DEFAULT_CONTEXT_WINDOW_SIZE
        } else {
            context_size.clamp(0, 20)
        };

        let mut chunks = Vec::new();
        let total = segments.len();
        let mut start_idx = 0;
        let mut chunk_index = 0;

        while start_idx < total {
            let end_idx = (start_idx + effective_chunk_size).min(total);
            let items = segments[start_idx..end_idx].to_vec();

            let context_start = start_idx.saturating_sub(effective_context_size);
            let context = if start_idx > 0 && context_start < start_idx {
                segments[context_start..start_idx].to_vec()
            } else {
                Vec::new()
            };

            chunks.push(DialogueChunk {
                chunk_index,
                context,
                items,
            });

            chunk_index += 1;
            start_idx = end_idx;
        }

        chunks
    }
}
