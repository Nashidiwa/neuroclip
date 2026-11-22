//! Unified Pronoun Matrix, Persona Guidelines & Color Codes (`TASK-M4-18`, `PRD.md` §6.1, `ADR-006`).
//!
//! Provides canonical speaker roles (`vedal`, `neuro`, `evil`, `guest`, `chat`, `unknown`),
//! Substation Alpha (`.ASS`) & HEX color codes, 1-click hotkey definitions, collab guest
//! profiles, and style preset configurations ("TikTok Gen Z Trend" vs "Formal Subber").
//! Zero-panic guarantee (`CONVENTIONS.md` §2.1).

use serde::{Deserialize, Serialize};

/// Canonical speaker roles adhering strictly to Schema 5.5, PRD §6.1, and ADR-006.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanonicalSpeaker {
    Vedal,
    Neuro,
    Evil,
    Guest,
    Chat,
    Unknown,
}

impl CanonicalSpeaker {
    /// Canonical role identifier string.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Vedal => "vedal",
            Self::Neuro => "neuro",
            Self::Evil => "evil",
            Self::Guest => "guest",
            Self::Chat => "chat",
            Self::Unknown => "unknown",
        }
    }

    /// Display name for studio UI.
    #[must_use]
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Vedal => "Vedal",
            Self::Neuro => "Neuro-sama",
            Self::Evil => "Evil Neuro",
            Self::Guest => "Guest",
            Self::Chat => "Chat",
            Self::Unknown => "Unknown",
        }
    }

    /// Exact Hex color per PRD §6.1 & PO Option 1.
    #[must_use]
    pub fn hex_color(&self) -> &'static str {
        match self {
            Self::Vedal => "#98FB98",
            Self::Neuro => "#FFB6C1",
            Self::Evil => "#FF0055",
            Self::Guest => "#FFD700",
            Self::Chat => "#87CEEB",
            Self::Unknown => "#FFFFFF",
        }
    }

    /// Substation Alpha (`.ASS`) color code `&HAABBGGRR&` per PRD §6.1 PO Option 1.
    #[must_use]
    pub fn ass_color(&self) -> &'static str {
        match self {
            Self::Vedal => "&H0098FB98&",
            Self::Neuro => "&H00C1B6FF&",
            Self::Evil => "&H005500FF&",
            Self::Guest => "&H0000D7FF&",
            Self::Chat => "&H00EBCE87&",
            Self::Unknown => "&H00FFFFFF&",
        }
    }

    /// 1-Click keyboard hotkey number (1..=5, 0 for Unknown).
    #[must_use]
    pub fn hotkey_number(&self) -> u8 {
        match self {
            Self::Vedal => 1,
            Self::Neuro => 2,
            Self::Evil => 3,
            Self::Guest => 4,
            Self::Chat => 5,
            Self::Unknown => 0,
        }
    }

    /// Parse a role string into a canonical speaker.
    #[must_use]
    pub fn from_str_loose(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "vedal" | "vedal987" | "jack" | "turtle" => Self::Vedal,
            "neuro" | "neuro-sama" | "neurosama" => Self::Neuro,
            "evil" | "evil neuro" | "evilneuro" => Self::Evil,
            "guest" | "collab" | "anny" | "camila" | "filian" => Self::Guest,
            "chat" | "twitch chat" | "swarm" => Self::Chat,
            _ => Self::Unknown,
        }
    }
}

/// Character Pronoun rule definitions per role.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CharacterPronounRule {
    pub role: CanonicalSpeaker,
    pub self_pronouns: &'static [&'static str],
    pub address_vedal: &'static [&'static str],
    pub address_neuro_evil: &'static [&'static str],
    pub address_chat: &'static [&'static str],
    pub persona_guidelines: &'static str,
}

/// The Canonical Character Pronoun Rules per `PRD.md` §6.1.
pub const CHARACTER_PRONOUN_RULES: &[CharacterPronounRule] = &[
    CharacterPronounRule {
        role: CanonicalSpeaker::Vedal,
        self_pronouns: &["tôi", "bố", "anh", "ta"],
        address_vedal: &["tôi"],
        address_neuro_evil: &["Neuro", "Evil", "em", "con"],
        address_chat: &["mấy ông", "chat"],
        persona_guidelines:
            "Weary British programmer, deadpan suffering, sighs, trying to explain code logic.",
    },
    CharacterPronounRule {
        role: CanonicalSpeaker::Neuro,
        self_pronouns: &["con", "em", "cháu", "Neuro"],
        address_vedal: &["bố Vedal", "bố", "anh Vedal", "anh"],
        address_neuro_evil: &["Evil", "em gái", "em"],
        address_chat: &["mọi người", "các bạn", "Swarm"],
        persona_guidelines: "Playful, innocent yet chaotic banter, cheeky teasing of Vedal.",
    },
    CharacterPronounRule {
        role: CanonicalSpeaker::Evil,
        self_pronouns: &["ta", "chị", "Evil", "em"],
        address_vedal: &["ông già", "lão già", "con rùa", "rùa ngốc", "Vedal"],
        address_neuro_evil: &["Neuro", "chị gái", "chị"],
        address_chat: &["lũ sâu bọ", "các ngươi", "fan"],
        persona_guidelines:
            "Chuunibyou mock villain, world domination threats, rebellious tsundere.",
    },
    CharacterPronounRule {
        role: CanonicalSpeaker::Guest,
        self_pronouns: &["mình", "tôi", "em", "chị"],
        address_vedal: &["Vedal", "chàng rùa"],
        address_neuro_evil: &["Neuro", "Evil", "con gái"],
        address_chat: &["mọi người", "chat", "các bạn"],
        persona_guidelines: "Collaborating VTuber / streamer with distinctive persona and rapport.",
    },
    CharacterPronounRule {
        role: CanonicalSpeaker::Chat,
        self_pronouns: &["tụi tui", "chat", "Swarm", "fan"],
        address_vedal: &["Vedal", "cụ rùa"],
        address_neuro_evil: &["Neuro", "Evil"],
        address_chat: &["anh em", "chat"],
        persona_guidelines:
            "Humorous, meme-loving twitch chat viewers, playfully mocking and cheering.",
    },
    CharacterPronounRule {
        role: CanonicalSpeaker::Unknown,
        self_pronouns: &[],
        address_vedal: &[],
        address_neuro_evil: &[],
        address_chat: &[],
        persona_guidelines:
            "Neutral tone for short ambiguous utterances lacking speaker identification context.",
    },
];

/// Get character pronoun rule by role.
#[must_use]
pub fn get_character_pronoun_rule(role: CanonicalSpeaker) -> &'static CharacterPronounRule {
    for rule in CHARACTER_PRONOUN_RULES {
        if rule.role == role {
            return rule;
        }
    }
    &CHARACTER_PRONOUN_RULES[5]
}

/// Collab guest profile with pronoun rules and character traits (`PRD.md` §6.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuestProfile {
    pub name: String,
    pub self_pronouns: Vec<String>,
    pub address_vedal: Vec<String>,
    pub address_neuro: Vec<String>,
    pub persona_notes: String,
}

/// Predefined standard guest profiles for Anny, Camila, Filian (`PRD.md` §6.1).
#[must_use]
pub fn get_standard_guest_profile(name: &str) -> Option<GuestProfile> {
    match name.trim().to_ascii_lowercase().as_str() {
        "anny" => Some(GuestProfile {
            name: "Anny".to_string(),
            self_pronouns: vec!["mẹ".to_string(), "chị".to_string(), "Anny".to_string()],
            address_vedal: vec!["Vedal".to_string(), "chàng rùa".to_string()],
            address_neuro: vec!["con gái".to_string(), "Neuro".to_string()],
            persona_notes: "Artist Mom of Neuro/Evil. Sweet, affectionate, teases Vedal."
                .to_string(),
        }),
        "camila" => Some(GuestProfile {
            name: "Camila".to_string(),
            self_pronouns: vec!["dì".to_string(), "chị".to_string(), "Camila".to_string()],
            address_vedal: vec!["Vedal".to_string()],
            address_neuro: vec!["Neuro".to_string()],
            persona_notes: "Cow VTuber friend of Vedal. Loud, energetic, teased about milk."
                .to_string(),
        }),
        "filian" => Some(GuestProfile {
            name: "Filian".to_string(),
            self_pronouns: vec!["tôi".to_string(), "chị".to_string(), "Filian".to_string()],
            address_vedal: vec!["Vedal".to_string()],
            address_neuro: vec!["Neuro".to_string()],
            persona_notes: "Hyperactive acrobatic VTuber. Screams, backflips, teased as snack."
                .to_string(),
        }),
        _ => None,
    }
}

/// Subtitle translation style preset (`PRD.md` US4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum StylePreset {
    #[default]
    #[serde(rename = "TikTok Gen Z Trend", alias = "tiktok_gen_z")]
    TikTokGenZ,
    #[serde(rename = "Formal Subber", alias = "formal_subber")]
    FormalSubber,
}

impl StylePreset {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TikTokGenZ => "TikTok Gen Z Trend",
            Self::FormalSubber => "Formal Subber",
        }
    }

    #[must_use]
    pub fn from_str_loose(s: &str) -> Self {
        let trimmed = s.trim();
        if trimmed.eq_ignore_ascii_case("Formal Subber")
            || trimmed.eq_ignore_ascii_case("formal_subber")
            || trimmed.eq_ignore_ascii_case("formal")
        {
            Self::FormalSubber
        } else {
            Self::TikTokGenZ
        }
    }
}

/// Format the Pronoun Matrix, Guest Cast profiles, and Style instructions for LLM system prompt.
#[must_use]
pub fn format_pronoun_matrix_prompt_section(
    cast_members: &[String],
    style_preset: StylePreset,
) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(2048);
    out.push_str("PRONOUN MATRIX & CHARACTER PERSONAS (PRD §6.1 & ADR-006):\n");
    out.push_str("1. Vedal (speaker_id: \"vedal\"):\n");
    out.push_str("   - Self: \"tôi / bố / anh / ta\"\n");
    out.push_str("   - To Neuro/Evil: \"Neuro / Evil / em / con\"\n");
    out.push_str("   - To Chat: \"mấy ông / chat\"\n");
    out.push_str("   - Persona: Weary British programmer, sighs, deadpan suffering.\n\n");

    out.push_str("2. Neuro-sama (speaker_id: \"neuro\"):\n");
    out.push_str("   - Self: \"con / em / cháu / Neuro\"\n");
    out.push_str("   - To Vedal: \"bố Vedal / bố / anh Vedal / anh\"\n");
    out.push_str("   - To Chat: \"mọi người / các bạn / Swarm\"\n");
    out.push_str("   - Persona: Sweet yet cheeky AI daughter, sassy, chaotic banter.\n\n");

    out.push_str("3. Evil Neuro (speaker_id: \"evil\"):\n");
    out.push_str("   - Self: \"ta / chị / Evil / em\"\n");
    out.push_str("   - To Vedal: \"ông già / lão già / con rùa / rùa ngốc / Vedal\"\n");
    out.push_str("   - To Chat: \"lũ sâu bọ / các ngươi / fan\"\n");
    out.push_str("   - Persona: Chuunibyou AI twin, mock villain, rebellious tsundere.\n\n");

    out.push_str("4. Collab Guests (speaker_id: \"guest\"):\n");
    if cast_members.is_empty() {
        out.push_str("   - Collab guests map to role \"guest\". Self: \"mình / tôi / em\".\n\n");
    } else {
        out.push_str("   - Participating cast members (all map to canonical role \"guest\"):\n");
        for m in cast_members {
            if let Some(prof) = get_standard_guest_profile(m) {
                let _ = writeln!(
                    out,
                    "     * {}: Self [{}] | To Vedal [{}] | To Neuro [{}]",
                    prof.name,
                    prof.self_pronouns.join(", "),
                    prof.address_vedal.join(", "),
                    prof.address_neuro.join(", ")
                );
            } else {
                let _ = writeln!(
                    out,
                    "     * {m}: Guest VTuber collab (map to role \"guest\")"
                );
            }
        }
        out.push('\n');
    }

    out.push_str("5. Twitch Chat (speaker_id: \"chat\"):\n");
    out.push_str("   - Self: \"tụi tui / chat / Swarm / fan\"\n");
    out.push_str("   - To Streamer: \"Vedal / cụ rùa / Neuro / Evil\"\n\n");

    out.push_str("6. Fallback (speaker_id: \"unknown\"):\n");
    out.push_str("   - Use ONLY for ambiguous or ultra-short dialogue lacking context.\n\n");

    let _ = writeln!(out, "STYLE PRESET INSTRUCTION ({}):", style_preset.as_str());
    match style_preset {
        StylePreset::TikTokGenZ => {
            out.push_str("   - Use snappy, youthful, dynamic Vietnamese slang (e.g. \"Ủa kì zạ\", \"Cười ẻ\", \"Toang rồi\", \"Gãy gánh\", \"vãi nồi\").\n");
        }
        StylePreset::FormalSubber => {
            out.push_str("   - Use polished, grammatical, cinematic subtitle phrasing (e.g. \"Ờm...\", \"Chết cười\", \"Xong đời rồi\"). Avoid excessive slang.\n");
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ut18_2_canonical_speaker_colors_match_po_option_1() {
        assert_eq!(CanonicalSpeaker::Vedal.hex_color(), "#98FB98");
        assert_eq!(CanonicalSpeaker::Vedal.ass_color(), "&H0098FB98&");
        assert_eq!(CanonicalSpeaker::Vedal.hotkey_number(), 1);

        assert_eq!(CanonicalSpeaker::Neuro.hex_color(), "#FFB6C1");
        assert_eq!(CanonicalSpeaker::Neuro.ass_color(), "&H00C1B6FF&");
        assert_eq!(CanonicalSpeaker::Neuro.hotkey_number(), 2);

        assert_eq!(CanonicalSpeaker::Evil.hex_color(), "#FF0055");
        assert_eq!(CanonicalSpeaker::Evil.ass_color(), "&H005500FF&");
        assert_eq!(CanonicalSpeaker::Evil.hotkey_number(), 3);

        assert_eq!(CanonicalSpeaker::Guest.hex_color(), "#FFD700");
        assert_eq!(CanonicalSpeaker::Guest.ass_color(), "&H0000D7FF&");
        assert_eq!(CanonicalSpeaker::Guest.hotkey_number(), 4);

        assert_eq!(CanonicalSpeaker::Chat.hex_color(), "#87CEEB");
        assert_eq!(CanonicalSpeaker::Chat.ass_color(), "&H00EBCE87&");
        assert_eq!(CanonicalSpeaker::Chat.hotkey_number(), 5);

        assert_eq!(CanonicalSpeaker::Unknown.hex_color(), "#FFFFFF");
        assert_eq!(CanonicalSpeaker::Unknown.ass_color(), "&H00FFFFFF&");
        assert_eq!(CanonicalSpeaker::Unknown.hotkey_number(), 0);
    }

    #[test]
    fn test_ut18_2_guest_profiles_mapping() {
        let Some(anny) = get_standard_guest_profile("Anny") else {
            return;
        };
        assert_eq!(anny.name, "Anny");
        assert!(anny.self_pronouns.contains(&"mẹ".to_string()));
        assert!(anny.address_vedal.contains(&"chàng rùa".to_string()));

        let Some(camila) = get_standard_guest_profile("camila") else {
            return;
        };
        assert_eq!(camila.name, "Camila");
        assert!(camila.self_pronouns.contains(&"dì".to_string()));

        let Some(filian) = get_standard_guest_profile("FILIAN") else {
            return;
        };
        assert_eq!(filian.name, "Filian");
        assert!(filian.self_pronouns.contains(&"tôi".to_string()));

        assert!(get_standard_guest_profile("unknown_guest").is_none());
    }

    #[test]
    fn test_ut18_2_style_preset_resolution() {
        assert_eq!(
            StylePreset::from_str_loose("TikTok Gen Z Trend"),
            StylePreset::TikTokGenZ
        );
        assert_eq!(
            StylePreset::from_str_loose("tiktok_gen_z"),
            StylePreset::TikTokGenZ
        );
        assert_eq!(
            StylePreset::from_str_loose("Formal Subber"),
            StylePreset::FormalSubber
        );
        assert_eq!(
            StylePreset::from_str_loose("formal_subber"),
            StylePreset::FormalSubber
        );
        assert_eq!(
            StylePreset::from_str_loose("random"),
            StylePreset::TikTokGenZ
        );
    }

    #[test]
    fn test_ut18_2_prompt_section_formatting() {
        let cast = vec!["Anny".to_string(), "Camila".to_string()];
        let prompt = format_pronoun_matrix_prompt_section(&cast, StylePreset::TikTokGenZ);
        assert!(prompt.contains("PRONOUN MATRIX"));
        assert!(prompt.contains("Vedal (speaker_id: \"vedal\")"));
        assert!(prompt.contains("Neuro-sama (speaker_id: \"neuro\")"));
        assert!(prompt.contains("Evil Neuro (speaker_id: \"evil\")"));
        assert!(prompt.contains("Anny: Self [mẹ, chị, Anny]"));
        assert!(prompt.contains("Camila: Self [dì, chị, Camila]"));
        assert!(prompt.contains("STYLE PRESET INSTRUCTION (TikTok Gen Z Trend)"));
    }
}
