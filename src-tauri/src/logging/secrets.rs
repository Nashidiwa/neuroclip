//! Secret redaction and streaming sanitization module.
//!
//! Provides zero-leak redaction for API keys and auth tokens across providers:
//! - Groq (`gsk_*`)
//! - Google Gemini (`AIza*`)
//! - OpenAI (`sk-*`)
//! - Twitch OAuth & session tokens (`oauth:*`, `auth-token=*`)
//!
//! Complies strictly with `CONVENTIONS.md` §6.3 and zero-panic policies.

use std::io::Write;
use std::sync::OnceLock;
use regex::Regex;

/// Canonical secret redaction formula per `CONVENTIONS.md` §6.3.
///
/// Keys with length <= 8 are completely masked as `[REDACTED]`.
/// Keys with length > 8 retain their first 4 and last 4 characters:
/// `{prefix_4}[REDACTED]{suffix_4}`.
#[must_use]
pub fn redact_secret(key: &str) -> String {
    if key.len() <= 8 {
        return "[REDACTED]".to_string();
    }

    if key.is_char_boundary(4) && key.is_char_boundary(key.len() - 4) {
        format!("{}[REDACTED]{}", &key[..4], &key[key.len() - 4..])
    } else {
        "[REDACTED]".to_string()
    }
}

/// Static precompiled regex matcher for multi-provider secret patterns.
fn get_secret_regex() -> Option<&'static Regex> {
    static REGEX: OnceLock<Option<Regex>> = OnceLock::new();
    REGEX
        .get_or_init(|| {
            Regex::new(
                r"(?x)
                (?P<gsk>\bgsk_[a-zA-Z0-9_]{16,}\b) |
                (?P<aiza>\bAIza[0-9A-Za-z\-_]{20,}\b) |
                (?P<sk>\bsk-(?:proj-)?[a-zA-Z0-9_\-]{20,}\b) |
                (?P<oauth>\boauth:[a-zA-Z0-9]{16,}\b) |
                (?P<authtoken>auth-token[=:]\s*[a-zA-Z0-9\-_]{16,}) |
                (?P<bearer>Bearer\s+[a-zA-Z0-9\._\-]{20,})
            ",
            )
            .ok()
        })
        .as_ref()
}

/// Scan and redact all known API keys and tokens in a freeform text string.
#[must_use]
pub fn redact_secrets_in_text(input: &str) -> String {
    let Some(re) = get_secret_regex() else {
        return input.to_string();
    };

    re.replace_all(input, |caps: &regex::Captures<'_>| {
        let matched = caps.get(0).map_or("", |m| m.as_str());
        if matched.starts_with("auth-token") {
            "auth-token=[REDACTED]".to_string()
        } else if matched.starts_with("Bearer ") {
            "Bearer [REDACTED]".to_string()
        } else {
            redact_secret(matched)
        }
    })
    .into_owned()
}

/// Streaming interceptor that filters secrets before writing to the underlying sink.
pub struct RedactingWriter<W: Write> {
    inner: W,
}

impl<W: Write> RedactingWriter<W> {
    /// Construct a new `RedactingWriter` wrapping the given output writer.
    pub fn new(inner: W) -> Self {
        Self { inner }
    }

    /// Unwrap and return the inner writer.
    pub fn into_inner(self) -> W {
        self.inner
    }
}

impl<W: Write> Write for RedactingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let text = String::from_utf8_lossy(buf);
        let redacted = redact_secrets_in_text(&text);
        self.inner.write_all(redacted.as_bytes())?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ut_04_1_happy_path_secret_redaction() {
        let raw = "gsk_1234567890abcdef12345678";
        let redacted = redact_secret(raw);
        assert_eq!(redacted, "gsk_[REDACTED]5678");
    }

    #[test]
    fn test_ut_04_2_edge_case_short_key() {
        assert_eq!(redact_secret(""), "[REDACTED]");
        assert_eq!(redact_secret("1234567"), "[REDACTED]");
        assert_eq!(redact_secret("12345678"), "[REDACTED]");
        assert_eq!(redact_secret("123456789"), "1234[REDACTED]6789");
    }

    #[test]
    fn test_ut_04_3_all_provider_tokens() {
        // Groq
        let groq = "gsk_abcdef1234567890fedcba";
        let res_groq = redact_secrets_in_text(&format!("Loaded Groq key: {groq}"));
        assert!(!res_groq.contains(groq));
        assert!(res_groq.contains("gsk_[REDACTED]dcba"));

        // Gemini
        let gemini = "AIzaSyD987654321abcdef1234567890";
        let res_gemini = redact_secrets_in_text(&format!("Gemini token {gemini} initialized"));
        assert!(!res_gemini.contains(gemini));
        assert!(res_gemini.contains("AIza[REDACTED]7890"));

        // OpenAI
        let openai = "sk-proj-abcdefghijklmnopqrstuvwxyz123456";
        let res_openai = redact_secrets_in_text(&format!("OpenAI key {openai} active"));
        assert!(!res_openai.contains(openai));
        assert!(res_openai.contains("sk-p[REDACTED]3456"));

        // Twitch Auth Token
        let twitch_auth = "auth-token=1234567890abcdef1234";
        let res_twitch = redact_secrets_in_text(&format!("Twitch header: {twitch_auth}"));
        assert!(!res_twitch.contains("1234567890abcdef1234"));
        assert!(res_twitch.contains("auth-token=[REDACTED]"));

        // Twitch OAuth
        let oauth = "oauth:1234567890abcdef1234";
        let res_oauth = redact_secrets_in_text(&format!("IRC password: {oauth}"));
        assert!(!res_oauth.contains(oauth));
        assert!(res_oauth.contains("oaut[REDACTED]1234"));
    }

    #[test]
    fn test_redacting_writer_stream_sanitization() {
        let mut sink = Vec::new();
        {
            let mut writer = RedactingWriter::new(&mut sink);
            let raw_msg = b"Connecting with gsk_1234567890abcdef12345678 now\n";
            let written = writer.write(raw_msg);
            assert!(written.is_ok());
            let flush_res = writer.flush();
            assert!(flush_res.is_ok());
        }

        let output = String::from_utf8_lossy(&sink);
        assert!(!output.contains("gsk_1234567890abcdef12345678"));
        assert!(output.contains("gsk_[REDACTED]5678"));
    }
}
