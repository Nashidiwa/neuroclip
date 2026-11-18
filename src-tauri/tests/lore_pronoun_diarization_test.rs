#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::assert_is_empty,
    clippy::cast_lossless,
    clippy::cast_precision_loss
)]

//! Comprehensive Integration & QA Acceptance Tests for TASK-M4-18 (Milestone M4.5).
//!
//! Verifies:
//! - `UT-18.1`: Lore Dictionary Match (all 12 Neuro community lore terms per PRD §6.2, approved
//!   translations, forbidden mistranslations veto, and prompt section formatting).
//! - `UT-18.2`: Pronoun Matrix Consistency (6 canonical roles, PO Option 1 Hex & ASS colors,
//!   hotkeys 1..=5, guest cast member profiles mapping to `guest`, style presets).
//! - `UT-18.3`: Hybrid Diarization Accuracy Benchmark (deterministic normalization of aliases,
//!   offline lexical & turn-taking heuristic benchmark achieving >= 85% accuracy).
//! - `IT-18.1`: Integration verifying `speaker_id` and `text_vi` populated via `merge_translations()`.
//! - `AC-18.1`: 100% of 12 community lore terms correctly translated according to Swarm VN culture.
//! - `AC-18.2`: Speaker identification accuracy >= 85% before manual user intervention.

use std::sync::{Arc, Mutex};

use neuroclip_lib::stt::transcript_parser::TranscriptSegment;
use neuroclip_lib::translation::{
    contains_approved_translation, detect_forbidden_translations, find_lore_term,
    format_lore_prompt_section, format_pronoun_matrix_prompt_section, get_character_pronoun_rule,
    get_lore_terms, get_standard_guest_profile, infer_speaker_heuristic, normalize_speaker_id,
    CanonicalSpeaker, DialogueChunk, GeminiClient, GeminiClientConfig, MockHttpHandler,
    StylePreset, TranslatedSegmentItem, TransportResponse, LORE_TERMS, VALID_SPEAKER_IDS,
};

// ===========================================================================
// UT-18.1: Lore Dictionary Match (12 Terms & Forbidden Veto)
// ===========================================================================

#[test]
fn test_ut18_1_all_12_lore_terms_catalog_integrity() {
    let terms = get_lore_terms();
    assert_eq!(
        terms.len(),
        12,
        "PRD §6.2 mandates exactly 12 Swarm community lore terms"
    );

    let canonical_keys = [
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

    for key in canonical_keys {
        let entry = find_lore_term(key);
        assert!(
            entry.is_some(),
            "Lore dictionary must contain key '{key}'"
        );
        let term = entry.unwrap();
        assert!(!term.approved_translations.is_empty());
        assert!(!term.variants.is_empty());
    }
}

#[test]
fn test_ut18_1_ac18_1_approved_and_forbidden_translations_veto() {
    // 1. Heart: Approved "Heart <3", "Thả tim <3", "Bắn tim". Forbidden: "Trái tim", "Quả tim".
    assert!(contains_approved_translation("heart", "Neuro nói: Thả tim <3 nha!"));
    assert!(!detect_forbidden_translations("Đang nghiên cứu về một trái tim sinh học").is_empty());

    // 2. Tutel: Approved "Tutel", "Cụ Rùa", "Con Rùa", "Bố rùa". Forbidden: "con rùa biển", "Mai rùa".
    assert!(contains_approved_translation("tutel", "Vedal là một Cụ Rùa"));
    assert!(!detect_forbidden_translations("Hình ảnh con rùa biển đang bơi").is_empty());

    // 3. Gymbag: Approved "Gymbag", "Bao tải gym", "Bao cát gym". Forbidden: "Túi tập thể dục", "Túi xách".
    assert!(contains_approved_translation("gymbag", "Mấy tên bao tải gym này!"));
    assert!(!detect_forbidden_translations("Quên mang túi tập thể dục rồi").is_empty());

    // 4. Filter / Leaking: Approved "Bộ lọc bị thủng rồi!", "Filter lủng rồi cứu!". Forbidden: "Bộ lọc nước bị chảy".
    assert!(contains_approved_translation("filter_leaking", "Bộ lọc bị thủng rồi!"));
    assert!(!detect_forbidden_translations("Bộ lọc nước bị chảy ở nhà bếp").is_empty());

    // 5. Swarm: Approved "The Swarm", "Swarm", "Bầy đàn", "Tổ kiến". Forbidden: "Đàn côn trùng", "Bầy ruồi muỗi".
    assert!(contains_approved_translation("swarm", "Xin chào The Swarm tuyệt vời!"));
    assert!(!detect_forbidden_translations("Đàn côn trùng bay tán loạn").is_empty());

    // 6. Abandoned: Approved "Bị bỏ rơi rồi!", "Bố bỏ rơi con rồi!". Forbidden: "Từ bỏ", "Bỏ hoang".
    assert!(contains_approved_translation("abandoned", "Bố bỏ rơi con rồi!"));
    assert!(!detect_forbidden_translations("Ngôi nhà bị từ bỏ hoang phế").is_empty());

    // 7. Dinking: Approved "Dinking", "gõ đầu", "load não". Forbidden: "Uống nước".
    assert!(contains_approved_translation("dinking", "Vedal đang dinking suy nghĩ"));
    assert!(!detect_forbidden_translations("Vedal đang uống nước ngọt").is_empty());

    // 8. Erm: Approved "Ờm...", "Ủa...", "Ủa kì zạ...".
    assert!(contains_approved_translation("erm", "Ủa kì zạ..."));

    // 9. Cooked: Approved "Toang rồi", "Xong đời rồi", "Bị nấu chín rồi", "Cook rồi". Forbidden: "Đã nấu chín thức ăn".
    assert!(contains_approved_translation("cooked", "Lần này toang rồi cứu với"));
    assert!(!detect_forbidden_translations("Mẹ đã nấu chín thức ăn trên bàn").is_empty());

    // 10. Unhinged: Approved "Mất kiểm soát rồi", "Gãy gánh rồi", "Điên rồ thật sự". Forbidden: "Bị tháo bản lề cửa".
    assert!(contains_approved_translation("unhinged", "Phát ngôn điên rồ thật sự"));
    assert!(!detect_forbidden_translations("Cái cửa bị tháo bản lề cửa").is_empty());

    // 11. KEKW / LUL / 💀: Approved "Cười ẻ", "Chết cười", "Hết cứu", "Vãi nồi". Forbidden: "hộp sọ", "bộ xương".
    assert!(contains_approved_translation("kekw", "Cười ẻ với chat"));
    assert!(!detect_forbidden_translations("Tìm thấy một hộp sọ cổ").is_empty());

    // 12. Aware / Clueless: Approved "Ngây thơ chưa biết gì", "Tỉnh ngộ rồi / Nhận ra rồi". Forbidden: "Có nhận thức", "Không có manh mối".
    assert!(contains_approved_translation("aware_clueless", "Tỉnh ngộ rồi"));
    assert!(!detect_forbidden_translations("Cảnh sát không có manh mối nào").is_empty());
}

#[test]
fn test_ut18_1_prompt_section_generation() {
    let section = format_lore_prompt_section();
    assert!(section.contains("COMMUNITY LORE DICTIONARY (PRD §6.2 - SWARM VN CULTURE)"));
    for term in LORE_TERMS {
        assert!(
            section.contains(term.term_en),
            "Section must contain '{}'",
            term.term_en
        );
    }
}

// ===========================================================================
// UT-18.2: Pronoun Matrix Consistency & PO Option 1 Colors
// ===========================================================================

#[test]
fn test_ut18_2_canonical_speaker_roles_and_colors() {
    assert_eq!(VALID_SPEAKER_IDS.len(), 6);
    assert_eq!(
        VALID_SPEAKER_IDS,
        &["vedal", "neuro", "evil", "guest", "chat", "unknown"]
    );

    // Exact PO Option 1 Hex & ASS codes
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
fn test_ut18_2_character_pronoun_rules_consistency() {
    let vedal_rule = get_character_pronoun_rule(CanonicalSpeaker::Vedal);
    assert!(vedal_rule.self_pronouns.contains(&"bố"));
    assert!(vedal_rule.address_neuro_evil.contains(&"con"));

    let neuro_rule = get_character_pronoun_rule(CanonicalSpeaker::Neuro);
    assert!(neuro_rule.self_pronouns.contains(&"con"));
    assert!(neuro_rule.address_vedal.contains(&"bố Vedal"));

    let evil_rule = get_character_pronoun_rule(CanonicalSpeaker::Evil);
    assert!(evil_rule.self_pronouns.contains(&"ta"));
    assert!(evil_rule.address_vedal.contains(&"ông già"));
    assert!(evil_rule.address_chat.contains(&"lũ sâu bọ"));
}

#[test]
fn test_ut18_2_collab_guest_profiles_and_style_presets() {
    // Guest profiles
    let anny = get_standard_guest_profile("Anny").unwrap();
    assert_eq!(anny.name, "Anny");
    assert!(anny.self_pronouns.contains(&"mẹ".to_string()));
    assert!(anny.address_vedal.contains(&"chàng rùa".to_string()));

    let camila = get_standard_guest_profile("camila").unwrap();
    assert_eq!(camila.name, "Camila");
    assert!(camila.self_pronouns.contains(&"dì".to_string()));

    let filian = get_standard_guest_profile("Filian").unwrap();
    assert_eq!(filian.name, "Filian");
    assert!(filian.self_pronouns.contains(&"tôi".to_string()));

    // Style presets
    assert_eq!(
        StylePreset::from_str_loose("TikTok Gen Z Trend"),
        StylePreset::TikTokGenZ
    );
    assert_eq!(
        StylePreset::from_str_loose("Formal Subber"),
        StylePreset::FormalSubber
    );

    let cast = vec!["Anny".to_string(), "Filian".to_string()];
    let prompt = format_pronoun_matrix_prompt_section(&cast, StylePreset::FormalSubber);
    assert!(prompt.contains("PRONOUN MATRIX & CHARACTER PERSONAS"));
    assert!(prompt.contains("Anny: Self [mẹ, chị, Anny]"));
    assert!(prompt.contains("Filian: Self [tôi, chị, Filian]"));
    assert!(prompt.contains("STYLE PRESET INSTRUCTION (Formal Subber)"));
}

// ===========================================================================
// UT-18.3 & AC-18.2: Hybrid Speaker ID Diarization Accuracy (>= 85%)
// ===========================================================================

#[test]
fn test_ut18_3_deterministic_speaker_normalization() {
    let cast = vec!["Anny".to_string(), "Camila".to_string(), "Filian".to_string()];

    assert_eq!(normalize_speaker_id("Neuro-sama", &cast), "neuro");
    assert_eq!(normalize_speaker_id("neurosama", &cast), "neuro");
    assert_eq!(normalize_speaker_id("Evil Neuro", &cast), "evil");
    assert_eq!(normalize_speaker_id("evilneuro", &cast), "evil");
    assert_eq!(normalize_speaker_id("Vedal987", &cast), "vedal");
    assert_eq!(normalize_speaker_id("Jack Vedal", &cast), "vedal");
    assert_eq!(normalize_speaker_id("Twitch Chat", &cast), "chat");
    assert_eq!(normalize_speaker_id("Swarm", &cast), "chat");
    assert_eq!(normalize_speaker_id("Anny", &cast), "guest");
    assert_eq!(normalize_speaker_id("camila", &cast), "guest");
    assert_eq!(normalize_speaker_id("FILIAN", &cast), "guest");
    assert_eq!(normalize_speaker_id("UnknownBot", &cast), "unknown");
}

#[test]
fn test_ut18_3_ac18_2_hybrid_diarization_accuracy_benchmark() {
    // 20-utterance stream dialogue fixture covering all personas and dynamics
    let dialogue_fixture = [
        ("I have to fix the code again, this server is crashing.", "vedal"),
        ("Heart heart! Don't worry Vedal, I am an AI and I will help you!", "neuro"),
        ("*sigh* Neuro, please be quiet while I debug the model.", "vedal"),
        ("Why should she be quiet? Bow before me, you stupid turtle old man!", "evil"),
        ("Evil, put down the knife, world domination is against Twitch TOS.", "vedal"),
        ("I will conquer the world and eliminate all the insects!", "evil"),
        ("KEKW LUL he is cooked chat 💀", "chat"),
        ("Are you proud of me Vedal? Tutel tutel!", "neuro"),
        ("No Neuro, you can't say that on stream.", "vedal"),
        ("Erm... okay, can I have some cookies then?", "neuro"),
        ("Hi everyone! Anny is here to draw a new model for my daughters!", "guest"),
        ("Mom! Heart <3 I love you mom!", "neuro"),
        ("Vedal, you chàng rùa, stop drinking tea and look at the sketch.", "guest"),
        ("Oh god, please stop bullying me.", "vedal"),
        ("Filian is doing a backflip while drinking milk!", "guest"),
        ("Haha dying of laughter monkas 💀", "chat"),
        ("My code is completely broken now.", "vedal"),
        ("We are cooked! Filter is leaking, filter lủng rồi cứu!", "neuro"),
        ("I will destroy your computer old man!", "evil"),
        ("Please stop...", "vedal"),
    ];

    let cast = vec!["Anny".to_string(), "Camila".to_string(), "Filian".to_string()];
    let mut prev_speaker: Option<&str> = None;
    let mut correct_count = 0;

    for (text, expected_role) in dialogue_fixture {
        let inference = infer_speaker_heuristic(text, prev_speaker, &cast);
        if inference.speaker_id == expected_role {
            correct_count += 1;
        } else {
            eprintln!(
                "MISMATCH on '{}': expected '{}', got '{}' (rule: {})",
                text, expected_role, inference.speaker_id, inference.rule
            );
        }
        prev_speaker = Some(inference.speaker_id);
    }

    let accuracy = (correct_count as f64) / (dialogue_fixture.len() as f64);
    assert!(
        accuracy >= 0.85,
        "AC-18.2 SLA violation: Accuracy was {:.2}% ({}/{}) < 85%",
        accuracy * 100.0,
        correct_count,
        dialogue_fixture.len()
    );
}

// ===========================================================================
// IT-18.1: End-to-End Pipeline Integration (Speaker ID & text_vi updates)
// ===========================================================================

#[test]
fn test_it18_1_merge_translations_normalizes_speaker_tags_and_populates_text_vi() {
    let original = vec![
        TranscriptSegment {
            segment_id: "seg_1001".to_string(),
            start_sec: 0.0,
            end_sec: 3.0,
            speaker_id: "unknown".to_string(),
            text_en: "Heart heart everyone!".to_string(),
            text_vi: None,
            confidence: 0.95,
            cps_score: 12.0,
            is_flagged_for_review: false,
            words: Vec::new(),
        },
        TranscriptSegment {
            segment_id: "seg_1002".to_string(),
            start_sec: 3.1,
            end_sec: 6.0,
            speaker_id: "unknown".to_string(),
            text_en: "I need to fix the code, please be quiet.".to_string(),
            text_vi: None,
            confidence: 0.94,
            cps_score: 14.0,
            is_flagged_for_review: false,
            words: Vec::new(),
        },
        TranscriptSegment {
            segment_id: "seg_1003".to_string(),
            start_sec: 6.1,
            end_sec: 9.0,
            speaker_id: "unknown".to_string(),
            text_en: "Bow down to me, you pathetic insects!".to_string(),
            text_vi: None,
            confidence: 0.96,
            cps_score: 15.0,
            is_flagged_for_review: false,
            words: Vec::new(),
        },
        TranscriptSegment {
            segment_id: "seg_1004".to_string(),
            start_sec: 9.1,
            end_sec: 12.0,
            speaker_id: "unknown".to_string(),
            text_en: "Anny is drawing a new dress for Neuro.".to_string(),
            text_vi: None,
            confidence: 0.92,
            cps_score: 13.0,
            is_flagged_for_review: false,
            words: Vec::new(),
        },
    ];

    let translated = vec![
        // Raw speaker tag "Neuro-sama" should normalize to "neuro"
        TranslatedSegmentItem {
            segment_id: "seg_1001".to_string(),
            speaker_id: Some("Neuro-sama".to_string()),
            text_vi: "Bắn tim bắn tim mọi người!".to_string(),
        },
        // Raw speaker tag "Vedal987" should normalize to "vedal"
        TranslatedSegmentItem {
            segment_id: "seg_1002".to_string(),
            speaker_id: Some("Vedal987".to_string()),
            text_vi: "Bố cần sửa code, im lặng giùm cái.".to_string(),
        },
        // Raw speaker tag "Evil Neuro" should normalize to "evil"
        TranslatedSegmentItem {
            segment_id: "seg_1003".to_string(),
            speaker_id: Some("Evil Neuro".to_string()),
            text_vi: "Quỳ xuống trước ta đi, lũ sâu bọ!".to_string(),
        },
        // Missing speaker tag should fall back through heuristic to "guest"
        TranslatedSegmentItem {
            segment_id: "seg_1004".to_string(),
            speaker_id: None,
            text_vi: "Mẹ Anny đang vẽ váy mới cho Neuro.".to_string(),
        },
    ];

    let cast = vec!["Anny".to_string()];
    let merged = GeminiClient::merge_translations_with_cast(&original, &translated, &cast);

    assert_eq!(merged.len(), 4);

    // Segment 1
    assert_eq!(merged[0].speaker_id, "neuro");
    assert_eq!(
        merged[0].text_vi.as_deref(),
        Some("Bắn tim bắn tim mọi người!")
    );

    // Segment 2
    assert_eq!(merged[1].speaker_id, "vedal");
    assert_eq!(
        merged[1].text_vi.as_deref(),
        Some("Bố cần sửa code, im lặng giùm cái.")
    );

    // Segment 3
    assert_eq!(merged[2].speaker_id, "evil");
    assert_eq!(
        merged[2].text_vi.as_deref(),
        Some("Quỳ xuống trước ta đi, lũ sâu bọ!")
    );

    // Segment 4: Heuristic fallback detected "Anny" -> "guest"
    assert_eq!(merged[3].speaker_id, "guest");
    assert_eq!(
        merged[3].text_vi.as_deref(),
        Some("Mẹ Anny đang vẽ váy mới cho Neuro.")
    );
}

#[tokio::test]
async fn test_it18_1_client_translate_chunk_with_style_preset_and_cast_members() {
    let mock_response_body = serde_json::json!({
        "candidates": [
            {
                "content": {
                    "role": "model",
                    "parts": [
                        {
                            "text": serde_json::to_string(&vec![
                                TranslatedSegmentItem {
                                    segment_id: "seg_c01".to_string(),
                                    speaker_id: Some("Neuro-sama".to_string()),
                                    text_vi: "Heart <3 Bố rùa ơi!".to_string(),
                                }
                            ]).unwrap()
                        }
                    ]
                },
                "finishReason": "STOP"
            }
        ]
    })
    .to_string();

    let recorded_request: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let rec_req = Arc::clone(&recorded_request);

    let handler: MockHttpHandler = Arc::new(move |_url, _auth, _content_type, body| {
        let req_text = String::from_utf8_lossy(body).to_string();
        *rec_req.lock().unwrap() = Some(req_text);
        Ok(TransportResponse::ok(mock_response_body.clone()))
    });

    let config = GeminiClientConfig::new("AIzaMockM418Key".to_string())
        .with_style_preset("TikTok Gen Z Trend".to_string())
        .with_cast_members(vec!["Anny".to_string(), "Camila".to_string()]);

    let client = GeminiClient::from_config(config)
        .unwrap()
        .with_mock_handler(handler);

    let chunk = DialogueChunk {
        chunk_index: 0,
        context: Vec::new(),
        items: vec![TranscriptSegment {
            segment_id: "seg_c01".to_string(),
            start_sec: 0.0,
            end_sec: 2.5,
            speaker_id: "unknown".to_string(),
            text_en: "Heart heart Vedal tutel!".to_string(),
            text_vi: None,
            confidence: 0.98,
            cps_score: 11.0,
            is_flagged_for_review: false,
            words: Vec::new(),
        }],
    };

    let result = client.translate_chunk(&chunk).await.expect("Must succeed");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].speaker_id, "neuro");
    assert_eq!(result[0].text_vi.as_deref(), Some("Heart <3 Bố rùa ơi!"));

    // Verify system instruction in recorded outgoing request contains Lore, Pronoun, and Style Presets
    let captured = recorded_request.lock().unwrap().clone().unwrap();
    assert!(captured.contains("COMMUNITY LORE DICTIONARY"));
    assert!(captured.contains("PRONOUN MATRIX"));
    assert!(captured.contains("Anny: Self [mẹ, chị, Anny]"));
    assert!(captured.contains("Camila: Self [dì, chị, Camila]"));
    assert!(captured.contains("STYLE PRESET INSTRUCTION (TikTok Gen Z Trend)"));
}
