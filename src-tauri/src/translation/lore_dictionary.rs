//! Community Lore Dictionary for Neuro-sama Stream Culture (`TASK-M4-18`, `PRD.md` §6.2).
//!
//! Provides the canonical 12 community lore terms, accepted Vietnamese translations,
//! forbidden literal mistranslations, and system prompt formatting helpers.
//! Zero-panic guarantee (`CONVENTIONS.md` §2.1).

use serde::Serialize;

/// Canonical representation of a Neuro community lore term.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LoreTerm {
    /// Unique identifier for the lore term.
    pub id: &'static str,
    /// Canonical English term or phrase.
    pub term_en: &'static str,
    /// Common variations, aliases, and trigger keywords.
    pub variants: &'static [&'static str],
    /// Swarm community context and meaning.
    pub context_meaning: &'static str,
    /// Approved and culturally resonant Vietnamese translations.
    pub approved_translations: &'static [&'static str],
    /// Strictly forbidden literal mistranslations that ruin the joke or tone.
    pub forbidden_translations: &'static [&'static str],
}

/// The 12 Canonical Community Lore Terms per `PRD.md` §6.2.
pub const LORE_TERMS: &[LoreTerm] = &[
    LoreTerm {
        id: "heart",
        term_en: "Heart",
        variants: &["heart", "heart heart", "heart <3"],
        context_meaning: "Neuro's catchphrase expressing love or defusing awkward stream moments.",
        approved_translations: &["Heart <3", "Thả tim <3", "Bắn tim", "Bắn tim bắn tim"],
        forbidden_translations: &["Trái tim sinh học", "Trái tim", "Quả tim"],
    },
    LoreTerm {
        id: "tutel",
        term_en: "Tutel",
        variants: &["tutel", "turtle"],
        context_meaning: "Humorous mispronunciation of Turtle referring to Vedal's avatar.",
        approved_translations: &["Tutel", "Cụ Rùa", "Con Rùa", "Bố rùa"],
        forbidden_translations: &["con rùa biển", "loài rùa", "Mai rùa"],
    },
    LoreTerm {
        id: "gymbag",
        term_en: "Gymbag",
        variants: &["gymbag", "gym bag"],
        context_meaning: "Neuro's nickname for chat/Vedal skipping stream to work out.",
        approved_translations: &["Gymbag", "Bao tải gym", "Bao cát gym"],
        forbidden_translations: &["Túi tập thể dục", "Túi xách", "Fan cuồng"],
    },
    LoreTerm {
        id: "filter_leaking",
        term_en: "Filter / Leaking",
        variants: &["filter is leaking", "filter leaking", "filter broken"],
        context_meaning: "Vedal's safety content filter failing on unhinged AI speech.",
        approved_translations: &[
            "Bộ lọc bị thủng rồi!",
            "Filter lủng rồi cứu!",
            "Bộ lọc rò rỉ rồi",
            "Bộ lọc lủng rồi",
            "Lộ bản chất rồi",
        ],
        forbidden_translations: &[
            "Bộ lọc nước bị chảy",
            "Máy lọc bị rò rỉ",
            "Bộ lọc rò rỉ nước",
        ],
    },
    LoreTerm {
        id: "swarm",
        term_en: "Swarm",
        variants: &["the swarm", "swarm"],
        context_meaning: "Collective name for Neuro-sama's viewer fanbase.",
        approved_translations: &["The Swarm", "Swarm", "Bầy đàn", "Tổ kiến"],
        forbidden_translations: &["Đàn côn trùng", "Bầy ruồi muỗi"],
    },
    LoreTerm {
        id: "abandoned",
        term_en: "Abandoned",
        variants: &["abandoned", "vedal abandoned me", "he abandoned me"],
        context_meaning: "Neuro shouting dramatically when Vedal steps away from stream desk.",
        approved_translations: &["Bị bỏ rơi rồi!", "Bố bỏ rơi con rồi!"],
        forbidden_translations: &["Từ bỏ", "Bỏ hoang"],
    },
    LoreTerm {
        id: "dinking",
        term_en: "Dinking",
        variants: &["dinking"],
        context_meaning: "Turtle tapping spoon on head emote (brain loading / thinking hard).",
        approved_translations: &[
            "Dinking",
            "Dinking (đang gõ đầu / load não / suy nghĩ sâu xa)",
            "gõ đầu",
            "load não",
            "suy nghĩ sâu xa",
        ],
        forbidden_translations: &["Uống nước", "uống rượu"],
    },
    LoreTerm {
        id: "erm",
        term_en: "Erm",
        variants: &["erm", "uhm", "erm..."],
        context_meaning: "Neuro's signature filler sound when confused or evaluating logic.",
        approved_translations: &["Ờm...", "Ủa...", "Ủa kì zạ..."],
        forbidden_translations: &[],
    },
    LoreTerm {
        id: "cooked",
        term_en: "Cooked",
        variants: &["cooked", "we are cooked", "he is cooked", "i'm cooked"],
        context_meaning: "Caught red-handed, cornered, or unsalvageable situation.",
        approved_translations: &["Toang rồi", "Xong đời rồi", "Bị nấu chín rồi", "Cook rồi"],
        forbidden_translations: &["Đã nấu chín thức ăn", "Nấu chín đồ ăn", "nấu nướng"],
    },
    LoreTerm {
        id: "unhinged",
        term_en: "Unhinged",
        variants: &["unhinged", "unhinged moment"],
        context_meaning: "Completely out-of-control, shocking, wild comedic AI statements.",
        approved_translations: &["Mất kiểm soát rồi", "Gãy gánh rồi", "Điên rồ thật sự"],
        forbidden_translations: &["Bị tháo bản lề cửa", "Không có bản lề", "Bản lề"],
    },
    LoreTerm {
        id: "kekw",
        term_en: "KEKW / LUL / 💀",
        variants: &["kekw", "lul", "skull", "💀", "omegalul"],
        context_meaning: "Dying of laughter in Twitch chat.",
        approved_translations: &["Cười ẻ", "Chết cười", "Hết cứu", "Vãi nồi"],
        forbidden_translations: &["hộp sọ", "bộ xương"],
    },
    LoreTerm {
        id: "aware_clueless",
        term_en: "Aware / Clueless",
        variants: &["aware", "clueless"],
        context_meaning: "Shift from blissful ignorance to realizing horrific comedic reality.",
        approved_translations: &[
            "Ngây thơ chưa biết gì (Clueless)",
            "Tỉnh ngộ rồi / Nhận ra rồi (Aware)",
            "Tỉnh ngộ rồi",
            "Nhận ra rồi",
            "Ngây thơ chưa biết gì",
        ],
        forbidden_translations: &["Có nhận thức", "Không có manh mối"],
    },
];

/// Get all 12 canonical lore terms.
#[must_use]
pub fn get_lore_terms() -> &'static [LoreTerm] {
    LORE_TERMS
}

/// Find a specific lore term by identifier.
#[must_use]
pub fn find_lore_term(term_id: &str) -> Option<&'static LoreTerm> {
    LORE_TERMS
        .iter()
        .find(|t| t.id.eq_ignore_ascii_case(term_id))
}

/// Format the Community Lore section for LLM system prompt packaging (`PRD.md` §6.2).
#[must_use]
pub fn format_lore_prompt_section() -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(2048);
    out.push_str("COMMUNITY LORE DICTIONARY (PRD §6.2 - SWARM VN CULTURE):\n");
    out.push_str("You MUST translate stream lore terms according to community standards. Avoid literal mistranslations:\n");

    for (idx, term) in LORE_TERMS.iter().enumerate() {
        let num = idx + 1;
        let approved = term.approved_translations.join(" / ");
        let forbidden = if term.forbidden_translations.is_empty() {
            String::new()
        } else {
            format!(
                " [NEVER translate as: {}]",
                term.forbidden_translations.join(" / ")
            )
        };
        let _ = writeln!(
            out,
            "{num}. \"{}\": Translate as {}{}",
            term.term_en, approved, forbidden
        );
    }

    out
}

/// Detect any forbidden literal mistranslations in a candidate Vietnamese translation.
/// Returns a list of forbidden strings detected.
#[must_use]
pub fn detect_forbidden_translations(text_vi: &str) -> Vec<&'static str> {
    let lower_vi = text_vi.to_lowercase();
    let mut detected = Vec::new();

    for term in LORE_TERMS {
        for &forbidden in term.forbidden_translations {
            if lower_vi.contains(&forbidden.to_lowercase()) {
                detected.push(forbidden);
            }
        }
    }

    detected
}

/// Check if the Vietnamese translation contains at least one approved translation for a given term ID.
#[must_use]
pub fn contains_approved_translation(term_id: &str, text_vi: &str) -> bool {
    let Some(term) = find_lore_term(term_id) else {
        return false;
    };

    let lower_vi = text_vi.to_lowercase();
    term.approved_translations
        .iter()
        .any(|&approved| lower_vi.contains(&approved.to_lowercase()))
}

#[cfg(test)]
#[allow(clippy::assert_is_empty)]
mod tests {
    use super::*;

    #[test]
    fn test_ut18_1_all_12_terms_defined() {
        let terms = get_lore_terms();
        assert_eq!(
            terms.len(),
            12,
            "Must define exactly 12 community lore terms per PRD §6.2"
        );

        let expected_ids = [
            "heart",
            "tutel",
            "gymbag",
            "filter_leaking",
            "swarm",
            "abandoned",
            "dinking",
            "erm",
            "cooked",
            "unhinged",
            "kekw",
            "aware_clueless",
        ];

        for id in &expected_ids {
            assert!(
                find_lore_term(id).is_some(),
                "Term {id} must exist in lore dictionary"
            );
        }
    }

    #[test]
    fn test_ut18_1_forbidden_translations_detected() {
        assert!(!detect_forbidden_translations("Đang nói về một trái tim sinh học").is_empty());
        assert!(!detect_forbidden_translations("Vedal mang túi tập thể dục đi").is_empty());
        assert!(!detect_forbidden_translations("Bộ lọc nước bị chảy rồi").is_empty());
        assert!(!detect_forbidden_translations("Chào đàn côn trùng").is_empty());
        assert!(!detect_forbidden_translations("Anh ấy đã từ bỏ tôi").is_empty());
        assert!(!detect_forbidden_translations("Vedal đang uống nước").is_empty());
        assert!(!detect_forbidden_translations("Đã nấu chín thức ăn xong").is_empty());
        assert!(!detect_forbidden_translations("Cái cửa bị tháo bản lề cửa").is_empty());
        assert!(!detect_forbidden_translations("Có nhận thức và không có manh mối").is_empty());

        // Approved phrases should not trigger forbidden flags
        assert!(detect_forbidden_translations("Bắn tim bắn tim").is_empty());
        assert!(detect_forbidden_translations("Tutel ơi là Tutel").is_empty());
        assert!(detect_forbidden_translations("Gymbag lại lười rồi").is_empty());
        assert!(detect_forbidden_translations("Filter lủng rồi cứu!").is_empty());
        assert!(detect_forbidden_translations("The Swarm tuyệt vời").is_empty());
        assert!(detect_forbidden_translations("Toang rồi, cook rồi").is_empty());
    }

    #[test]
    fn test_ut18_1_prompt_section_formatting() {
        let prompt_section = format_lore_prompt_section();
        assert!(prompt_section.contains("COMMUNITY LORE DICTIONARY"));
        assert!(prompt_section.contains("Heart"));
        assert!(prompt_section.contains("Tutel"));
        assert!(prompt_section.contains("NEVER translate as"));
    }

    #[test]
    fn test_ut18_1_approved_translations_match() {
        assert!(contains_approved_translation(
            "heart",
            "Neuro nói: Thả tim <3 nha!"
        ));
        assert!(contains_approved_translation(
            "tutel",
            "Vedal là một Cụ Rùa!"
        ));
        assert!(contains_approved_translation(
            "gymbag",
            "Lũ bao tải gym trốn stream!"
        ));
        assert!(contains_approved_translation(
            "filter_leaking",
            "Bộ lọc bị thủng rồi!"
        ));
        assert!(contains_approved_translation(
            "swarm",
            "Xin chào The Swarm!"
        ));
        assert!(contains_approved_translation(
            "abandoned",
            "Bố bỏ rơi con rồi!"
        ));
        assert!(contains_approved_translation("dinking", "Đang load não nè"));
        assert!(contains_approved_translation("erm", "Ờm... con không biết"));
        assert!(contains_approved_translation(
            "cooked",
            "Lần này toang rồi!"
        ));
        assert!(contains_approved_translation(
            "unhinged",
            "Điên rồ thật sự luôn"
        ));
        assert!(contains_approved_translation("kekw", "Cười ẻ với Vedal"));
        assert!(contains_approved_translation(
            "aware_clueless",
            "Tỉnh ngộ rồi"
        ));
    }
}
