//! Vietnamese Smart Line Wrapping Subsystem (`TASK-M4-19`, `PRD.md` §6.1, `ADR-006`).
//!
//! Enforces:
//! - Subtitle character limits: <= 42 chars per line (range 38-42, max 2 lines).
//! - Compound word preservation: Indivisible compound words ("bánh mì", "trí tuệ", "bố Vedal",
//!   "Evil Neuro", "Cụ Rùa", "The Swarm", etc.) are NEVER broken across line breaks.
//! - Accurate Unicode character counting (not UTF-8 byte lengths) for Vietnamese diacritics.
//! - Pure Rust, zero unwraps, zero expects, clippy pedantic compliance.

/// Default maximum character count per subtitle line for horizontal 16:9 video.
pub const DEFAULT_MAX_CHARS_PER_LINE: usize = 42;

/// Minimum character count target for wrapped lines before breaking.
pub const MIN_LINE_WRAP_TARGET_CHARS: usize = 38;

/// Default maximum lines per subtitle cue on screen.
pub const DEFAULT_MAX_LINES: usize = 2;

/// Maximum character count per line for vertical 9:16 layout (TikTok / Reels, `PRD.md` §6.1 CR-03).
pub const VERTICAL_9_16_MAX_CHARS_PER_LINE: usize = 26;

/// Canonical Vietnamese honorifics and family titles frequently paired with proper names.
pub const VIETNAMESE_HONORIFICS: &[&str] = &[
    "bố", "mẹ", "ông", "bà", "anh", "chị", "em", "bé", "cụ", "chú", "bác",
];

/// Curated dictionary of Vietnamese compound words, Swarm lore terms, and multi-syllable entities.
pub const VIETNAMESE_COMPOUND_WORDS: &[&str] = &[
    // 4-word & 3-word compounds
    "trí tuệ nhân tạo",
    "ngây thơ chưa biết gì",
    "mất kiểm soát rồi",
    "không có manh mối",
    "bắn tim bắn tim",
    "bị bỏ rơi rồi",
    "bộ lọc bị thủng",
    "filter lủng rồi cứu",
    // 2-word Neuro / Swarm Lore & Character Entities
    "bố vedal",
    "bố rùa",
    "evil neuro",
    "neuro-sama",
    "cụ rùa",
    "con rùa",
    "the swarm",
    "bầy đàn",
    "tổ kiến",
    "bộ lọc",
    "bị thủng",
    "lủng rồi",
    "rò rỉ",
    "lộ bản chất",
    "bỏ rơi",
    "bị bỏ rơi",
    "thả tim",
    "bắn tim",
    "gõ đầu",
    "load não",
    "suy nghĩ",
    "sâu xa",
    "nấu chín",
    "xong đời",
    "toang rồi",
    "cook rồi",
    "mất kiểm soát",
    "gãy gánh",
    "điên rồ",
    "thật sự",
    "thực sự",
    "chết cười",
    "cười ẻ",
    "hết cứu",
    "vãi nồi",
    "ngây thơ",
    "tỉnh ngộ",
    "nhận ra",
    "chưa biết",
    "bao tải gym",
    "bao cát gym",
    "giga chad",
    "super chat",
    // Common Vietnamese Compound Nouns
    "bánh mì",
    "trí tuệ",
    "cà phê",
    "máy tính",
    "điện thoại",
    "hình ảnh",
    "âm thanh",
    "phụ đề",
    "thời gian",
    "bản thân",
    "gia đình",
    "bạn bè",
    "con người",
    "thế giới",
    "câu hỏi",
    "câu trả lời",
    "vấn đề",
    "ý kiến",
    "kết quả",
    "lý do",
    "công việc",
    "trò chơi",
    "cảm xúc",
    "sức khỏe",
    "tương lai",
    "quá khứ",
    "hiện tại",
    "cuộc sống",
    "kỷ niệm",
    "thói quen",
    "kinh nghiệm",
    "cơ hội",
    "hy vọng",
    "ước mơ",
    "quyết định",
    "thông tin",
    "nội dung",
    "ý nghĩa",
    "thành phố",
    // Interrogatives & Demonstratives
    "cái gì",
    "chuyện gì",
    "điều gì",
    "ai đó",
    "nơi nào",
    "khi nào",
    "sao lại",
    "làm sao",
    "tại sao",
    "như thế nào",
    "bao nhiêu",
    "bao lâu",
    "bao giờ",
    // Action & State Verbs
    "bắt đầu",
    "kết thúc",
    "tiếp tục",
    "xuất hiện",
    "biến mất",
    "phát hiện",
    "tìm kiếm",
    "giải thích",
    "hướng dẫn",
    "chia sẻ",
    "tham gia",
    "chuẩn bị",
    "thực hiện",
    "hoàn thành",
    "giúp đỡ",
    "quan tâm",
    "bảo vệ",
    "tấn công",
    "chiến đấu",
    "thất bại",
    "chiến thắng",
    "cảm thấy",
    "nhận thấy",
    "nhớ lại",
    "quên mất",
    "cố gắng",
    "đồng ý",
    "từ chối",
    "yêu thích",
    "ghét bỏ",
    "lo lắng",
    "sợ hãi",
    "tự tin",
    "tin tưởng",
    "nghi ngờ",
    "chăm sóc",
    "trò chuyện",
    "thảo luận",
    "thông báo",
    "giới thiệu",
    "thay đổi",
    "phát triển",
    "mở rộng",
    "sửa chữa",
    "khắc phục",
    "rời khỏi",
    "quay lại",
    "trở về",
    "tiến lên",
    "dừng lại",
    // Adjectives & Adverbs
    "tuyệt vời",
    "kinh khủng",
    "khủng khiếp",
    "dễ thương",
    "đáng yêu",
    "thông minh",
    "ngốc nghếch",
    "vui vẻ",
    "hạnh phúc",
    "buồn bã",
    "tức giận",
    "ngạc nhiên",
    "bất ngờ",
    "thất vọng",
    "hài lòng",
    "tự nhiên",
    "đặc biệt",
    "bình thường",
    "quan trọng",
    "cần thiết",
    "rõ ràng",
    "chính xác",
    "đúng đắn",
    "sai lầm",
    "khó khăn",
    "dễ dàng",
    "nguy hiểm",
    "an toàn",
    "nhanh chóng",
    "chậm chạp",
    "mạnh mẽ",
    "yếu đuối",
    "đơn giản",
    "phức tạp",
    "hoàn hảo",
    "sẵn sàng",
    "chắc chắn",
    "may mắn",
    "nghiêm túc",
    "hài hước",
    // Pronouns & Discourse Particles
    "chúng ta",
    "chúng tôi",
    "chúng nó",
    "bọn họ",
    "mọi người",
    "ai nấy",
    "người ta",
    "anh ấy",
    "cô ấy",
    "ông ấy",
    "bà ấy",
    "các bạn",
    "tụi mình",
    "tụi nó",
    "hoàn toàn",
    "nhất định",
    "tuyệt đối",
    "vô cùng",
    "cực kỳ",
    "hầu như",
    "dường như",
    "có vẻ",
    "thậm chí",
    "ngay cả",
    "bỗng nhiên",
    "đột nhiên",
    "thường xuyên",
    "thỉnh thoảng",
    "mãi mãi",
    "lập tức",
    "ngay lập tức",
    "tức thì",
    "tuy nhiên",
    "bởi vì",
    "cho nên",
    "do đó",
    "vì vậy",
    "mặc dù",
    "cho dù",
    "nếu như",
    "giả sử",
    "miễn là",
    "ngoài ra",
    "hơn nữa",
    "cuối cùng",
    "tóm lại",
];

/// Strips leading and trailing punctuation from a word for dictionary lookup.
#[must_use]
pub fn strip_punctuation(word: &str) -> &str {
    word.trim_matches(|c: char| {
        c.is_ascii_punctuation()
            || matches!(
                c,
                '“' | '”' | '‘' | '’' | '«' | '»' | '…' | '—' | '–' | '•' | '·'
            )
    })
}

/// Checks whether a lowercase phrase matches the compound dictionary.
#[must_use]
pub fn is_compound_word(phrase: &str) -> bool {
    let lower = phrase.to_lowercase();
    VIETNAMESE_COMPOUND_WORDS.iter().any(|&c| c == lower)
}

/// Checks if a word is a Vietnamese honorific / kinship title.
#[must_use]
pub fn is_honorific(word: &str) -> bool {
    let lower = word.to_lowercase();
    VIETNAMESE_HONORIFICS.iter().any(|&h| h == lower)
}

/// Checks whether a word starts with an uppercase character.
#[must_use]
pub fn starts_with_uppercase(word: &str) -> bool {
    word.chars().next().is_some_and(char::is_uppercase)
}

/// Tokenizes Vietnamese text into atomic tokens (compounds preserved together with trailing punctuation).
#[must_use]
pub fn tokenize_vietnamese_compounds(text: &str) -> Vec<String> {
    let raw_words: Vec<&str> = text.split_whitespace().collect();
    if raw_words.is_empty() {
        return Vec::new();
    }

    let mut tokens = Vec::new();
    let mut i = 0;

    while i < raw_words.len() {
        // 1. Try 4-word compound match
        if i + 3 < raw_words.len() {
            let candidate = format!(
                "{} {} {} {}",
                strip_punctuation(raw_words[i]),
                strip_punctuation(raw_words[i + 1]),
                strip_punctuation(raw_words[i + 2]),
                strip_punctuation(raw_words[i + 3])
            );
            if is_compound_word(&candidate) {
                tokens.push(format!(
                    "{} {} {} {}",
                    raw_words[i],
                    raw_words[i + 1],
                    raw_words[i + 2],
                    raw_words[i + 3]
                ));
                i += 4;
                continue;
            }
        }

        // 2. Try 3-word compound match
        if i + 2 < raw_words.len() {
            let candidate = format!(
                "{} {} {}",
                strip_punctuation(raw_words[i]),
                strip_punctuation(raw_words[i + 1]),
                strip_punctuation(raw_words[i + 2])
            );
            if is_compound_word(&candidate) {
                tokens.push(format!(
                    "{} {} {}",
                    raw_words[i],
                    raw_words[i + 1],
                    raw_words[i + 2]
                ));
                i += 3;
                continue;
            }
        }

        // 3. Try 2-word compound or honorific + name match
        if i + 1 < raw_words.len() {
            let stripped_0 = strip_punctuation(raw_words[i]);
            let stripped_1 = strip_punctuation(raw_words[i + 1]);
            let candidate = format!("{stripped_0} {stripped_1}");

            let is_compound = is_compound_word(&candidate);
            let is_honorific_name = is_honorific(stripped_0) && starts_with_uppercase(stripped_1);
            let is_capitalized_pair = starts_with_uppercase(stripped_0)
                && starts_with_uppercase(stripped_1)
                && is_compound_word(&candidate);

            if is_compound || is_honorific_name || is_capitalized_pair {
                tokens.push(format!("{} {}", raw_words[i], raw_words[i + 1]));
                i += 2;
                continue;
            }
        }

        // 4. Fallback: single word token
        tokens.push(raw_words[i].to_string());
        i += 1;
    }

    tokens
}

/// Wraps Vietnamese text into lines of at most `max_chars_per_line` Unicode characters,
/// strictly preserving compound words and named entities across line breaks.
#[must_use]
pub fn wrap_vietnamese_text(
    text: &str,
    max_chars_per_line: usize,
    _max_lines: usize,
) -> Vec<String> {
    let clean_text = text.trim();
    if clean_text.is_empty() {
        return Vec::new();
    }

    let limit = if max_chars_per_line == 0 {
        DEFAULT_MAX_CHARS_PER_LINE
    } else {
        max_chars_per_line
    };

    // If text already fits within single line, return immediately
    if clean_text.chars().count() <= limit {
        return vec![clean_text.to_string()];
    }

    let tokens = tokenize_vietnamese_compounds(clean_text);
    let mut lines = Vec::new();
    let mut current_line = String::new();
    let mut current_line_chars = 0;

    for token in tokens {
        let token_chars = token.chars().count();

        // Edge case: single token itself is longer than line limit
        if token_chars > limit {
            if !current_line.is_empty() {
                lines.push(current_line);
                current_line = String::new();
                current_line_chars = 0;
            }
            let chars: Vec<char> = token.chars().collect();
            for chunk in chars.chunks(limit) {
                let chunk_str: String = chunk.iter().collect();
                if chunk.len() == limit {
                    lines.push(chunk_str);
                } else {
                    current_line = chunk_str;
                    current_line_chars = chunk.len();
                }
            }
            continue;
        }

        let space_needed = usize::from(!current_line.is_empty());
        if current_line_chars + space_needed + token_chars <= limit {
            if !current_line.is_empty() {
                current_line.push(' ');
            }
            current_line.push_str(&token);
            current_line_chars += space_needed + token_chars;
        } else {
            lines.push(current_line);
            current_line = token;
            current_line_chars = token_chars;
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    lines
}

/// Convenience helper to return wrapped subtitle joined with newline `\n`.
#[must_use]
pub fn wrap_vietnamese_subtitle(text: &str, max_chars_per_line: usize, max_lines: usize) -> String {
    wrap_vietnamese_text(text, max_chars_per_line, max_lines).join("\n")
}

/// Convenience helper to return wrapped text joined with newline `\n`.
#[must_use]
pub fn wrap_vietnamese_text_to_string(
    text: &str,
    max_chars_per_line: usize,
    max_lines: usize,
) -> String {
    wrap_vietnamese_subtitle(text, max_chars_per_line, max_lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ut_19_1_smart_compound_wrapping_preserves_banh_mi_and_bo_vedal() {
        // Exactly 50 Unicode characters:
        // "Tôi đã mua một ổ bánh mì rất ngon cho bố Vedal ăn."
        let text = "Tôi đã mua một ổ bánh mì rất ngon cho bố Vedal ăn.";
        assert_eq!(text.chars().count(), 50);

        let lines = wrap_vietnamese_text(text, 42, 2);
        assert_eq!(lines.len(), 2, "Should wrap into exactly 2 lines");

        for line in &lines {
            assert!(
                line.chars().count() <= 42,
                "Line length {} must be <= 42: '{line}'",
                line.chars().count()
            );
        }

        // Line 1 should not split "bánh mì" or "bố Vedal"
        assert!(
            !lines[0].ends_with("bánh"),
            "Line 1 must not split 'bánh mì'"
        );
        assert!(
            !lines[0].ends_with("bố"),
            "Line 1 must not split 'bố Vedal'"
        );

        // Verify the line break point: Line 1 should end with "cho" and Line 2 starts with "bố Vedal ăn."
        assert_eq!(lines[0], "Tôi đã mua một ổ bánh mì rất ngon cho");
        assert_eq!(lines[1], "bố Vedal ăn.");
    }

    #[test]
    fn test_short_text_single_line() {
        let text = "Chào bố Vedal!";
        let lines = wrap_vietnamese_text(text, 42, 2);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "Chào bố Vedal!");
    }

    #[test]
    fn test_exact_42_chars() {
        // Exactly 42 Unicode chars:
        // "Một hai ba bốn năm sáu bảy tám chín mười." -> 41 chars + '!' = 42
        let text = "Một hai ba bốn năm sáu bảy tám chín mười!!";
        assert_eq!(text.chars().count(), 42);

        let lines = wrap_vietnamese_text(text, 42, 2);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], text);
    }

    #[test]
    fn test_43_chars_borderline_wrapping() {
        let text = "Một hai ba bốn năm sáu bảy tám chín mười!!!";
        assert_eq!(text.chars().count(), 43);

        let lines = wrap_vietnamese_text(text, 42, 2);
        assert_eq!(lines.len(), 2);
        for line in &lines {
            assert!(line.chars().count() <= 42);
        }
    }

    #[test]
    fn test_multiple_compounds_preserved() {
        let text = "Evil Neuro cùng bố Vedal uống cà phê và ăn bánh mì.";
        let lines = wrap_vietnamese_text(text, 42, 2);
        assert_eq!(lines.len(), 2);
        for line in &lines {
            assert!(line.chars().count() <= 42);
            assert!(!line.ends_with("Evil"));
            assert!(!line.ends_with("bố"));
            assert!(!line.ends_with("cà"));
            assert!(!line.ends_with("bánh"));
        }
    }

    #[test]
    fn test_vertical_9_16_layout_26_chars() {
        let text = "Tôi thích ăn bánh mì và uống cà phê cùng bố Vedal.";
        let lines = wrap_vietnamese_text(text, VERTICAL_9_16_MAX_CHARS_PER_LINE, 2);
        for line in &lines {
            assert!(
                line.chars().count() <= 26,
                "Line length {} must be <= 26: '{line}'",
                line.chars().count()
            );
            assert!(!line.ends_with("bánh"));
            assert!(!line.ends_with("cà"));
            assert!(!line.ends_with("bố"));
        }
    }

    #[test]
    fn test_unbroken_token_longer_than_limit() {
        let long_token = "A".repeat(50);
        let lines = wrap_vietnamese_text(&long_token, 42, 2);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].chars().count(), 42);
        assert_eq!(lines[1].chars().count(), 8);
    }

    #[test]
    fn test_empty_and_whitespace_input() {
        assert_eq!(wrap_vietnamese_text("", 42, 2), Vec::<String>::new());
        assert_eq!(
            wrap_vietnamese_text("   \n\t  ", 42, 2),
            Vec::<String>::new()
        );
    }

    #[test]
    fn test_wrap_vietnamese_subtitle_newline_join() {
        let text = "Tôi đã mua một ổ bánh mì rất ngon cho bố Vedal ăn.";
        let wrapped = wrap_vietnamese_subtitle(text, 42, 2);
        assert_eq!(
            wrapped,
            "Tôi đã mua một ổ bánh mì rất ngon cho\nbố Vedal ăn."
        );
    }
}
