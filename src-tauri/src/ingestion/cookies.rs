//! Cookie loading and User-Agent rotation subsystem for yt-dlp anti-bot defense.
//!
//! Complies with AC-06.4, ED-06.3, and docs/CONVENTIONS.md (zero-panic invariant).

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::error::{AppError, IngestError};

/// Supported desktop browsers for extracting session cookies via yt-dlp.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserType {
    Chrome,
    Firefox,
    Edge,
}

impl BrowserType {
    /// Return the canonical CLI identifier string for yt-dlp `--cookies-from-browser`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Chrome => "chrome",
            Self::Firefox => "firefox",
            Self::Edge => "edge",
        }
    }
}

/// Source specification for authentication cookies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case", tag = "type", content = "value")]
pub enum CookieSource {
    #[default]
    None,
    Browser(BrowserType),
    File(String),
}

/// Pool of modern Windows 10/11 desktop User-Agent strings.
///
/// These realistic desktop headers bypass Cloudflare and YouTube HTTP 403 / 429 bot challenges.
pub const DESKTOP_USER_AGENTS: &[&str] = &[
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/123.0.0.0 Safari/537.36 Edg/123.0.0.0",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:125.0) Gecko/20100101 Firefox/125.0",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36",
];

static UA_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Retrieve the full list of whitelisted desktop User-Agent strings.
#[must_use]
pub const fn get_desktop_user_agents() -> &'static [&'static str] {
    DESKTOP_USER_AGENTS
}

/// Rotate through the pool of desktop User-Agent strings in a thread-safe manner.
#[must_use]
pub fn get_rotated_desktop_user_agent() -> &'static str {
    let index = UA_COUNTER.fetch_add(1, Ordering::Relaxed) % DESKTOP_USER_AGENTS.len();
    DESKTOP_USER_AGENTS[index]
}

/// Parse a raw browser name string into a strongly-typed `BrowserType`.
#[must_use]
pub fn parse_browser_type(name: &str) -> Option<BrowserType> {
    match name.trim().to_ascii_lowercase().as_str() {
        "chrome" | "google-chrome" => Some(BrowserType::Chrome),
        "firefox" | "mozilla" => Some(BrowserType::Firefox),
        "edge" | "msedge" => Some(BrowserType::Edge),
        _ => None,
    }
}

/// Validate cookie source parameters against CLI flag injection or NUL bytes.
///
/// # Errors
/// Returns [`AppError::Ingestion`] with [`IngestError::InvalidUrl`] if a cookie file path is empty,
/// starts with `-`, or contains `\0` / control characters.
pub fn validate_cookie_source(source: &CookieSource) -> Result<(), AppError> {
    if let CookieSource::File(path) = source {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return Err(AppError::Ingestion(IngestError::InvalidUrl(
                "Cookie file path cannot be empty".to_string(),
            )));
        }
        if path.chars().next().is_some_and(char::is_whitespace) || trimmed.starts_with('-') {
            return Err(AppError::Ingestion(IngestError::InvalidUrl(
                "Cookie file path must not start with whitespace or '-'".to_string(),
            )));
        }
        if path.contains('\0') || path.chars().any(char::is_control) {
            return Err(AppError::Ingestion(IngestError::InvalidUrl(
                "Cookie file path contains control or NUL characters".to_string(),
            )));
        }
    }
    Ok(())
}

/// Validate optional custom User-Agent string against header/CLI injection
/// (control characters such as `\r`, `\n`, `\0`, `\t`, `\x1b` and leading `-`).
///
/// # Errors
/// Returns [`AppError::Ingestion`] with [`IngestError::InvalidUrl`] if `custom_ua` contains
/// control characters or starts with `-`.
pub fn validate_custom_user_agent(custom_ua: Option<&str>) -> Result<(), AppError> {
    if let Some(ua) = custom_ua {
        if ua.chars().any(char::is_control) {
            return Err(AppError::Ingestion(IngestError::InvalidUrl(
                "Custom User-Agent must not contain control or NUL characters".to_string(),
            )));
        }
        if ua.trim().starts_with('-') {
            return Err(AppError::Ingestion(IngestError::InvalidUrl(
                "Custom User-Agent must not start with '-'".to_string(),
            )));
        }
    }
    Ok(())
}

/// Validate both cookie source and optional custom User-Agent before passing to `yt-dlp`.
///
/// # Errors
/// Returns [`AppError::Ingestion`] if either input contains unsafe characters or leading `-`.
pub fn validate_bot_defense_inputs(
    cookie_source: &CookieSource,
    custom_ua: Option<&str>,
) -> Result<(), AppError> {
    validate_cookie_source(cookie_source)?;
    validate_custom_user_agent(custom_ua)?;
    Ok(())
}

/// Build CLI arguments for cookies to pass to `yt-dlp`.
#[must_use]
pub fn build_cookie_args(source: &CookieSource) -> Vec<String> {
    match source {
        CookieSource::None => Vec::new(),
        CookieSource::Browser(browser) => {
            vec![
                "--cookies-from-browser".to_string(),
                browser.as_str().to_string(),
            ]
        }
        CookieSource::File(path) => {
            if validate_cookie_source(source).is_err() {
                return Vec::new();
            }
            vec!["--cookies".to_string(), path.trim().to_string()]
        }
    }
}

/// Build comprehensive anti-bot defense CLI arguments for `yt-dlp`.
///
/// Combines cookie source, rotated desktop User-Agent, and browser HTTP request headers.
#[must_use]
pub fn build_bot_defense_args(
    cookie_source: &CookieSource,
    custom_ua: Option<&str>,
) -> Vec<String> {
    let mut args = build_cookie_args(cookie_source);

    let ua = match custom_ua {
        Some(u) if !u.trim().is_empty() && validate_custom_user_agent(Some(u)).is_ok() => u.trim(),
        _ => get_rotated_desktop_user_agent(),
    };

    args.push("--user-agent".to_string());
    args.push(ua.to_string());

    args.push("--add-header".to_string());
    args.push("Accept-Language: en-US,en;q=0.9".to_string());

    args
}

/// Validate inputs and build comprehensive anti-bot defense CLI arguments for `yt-dlp`.
///
/// # Errors
/// Returns [`AppError::Ingestion`] if `cookie_source` or `custom_ua` fails validation.
pub fn try_build_bot_defense_args(
    cookie_source: &CookieSource,
    custom_ua: Option<&str>,
) -> Result<Vec<String>, AppError> {
    validate_bot_defense_inputs(cookie_source, custom_ua)?;
    Ok(build_bot_defense_args(cookie_source, custom_ua))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_browser_type_cli_strings() {
        assert_eq!(BrowserType::Chrome.as_str(), "chrome");
        assert_eq!(BrowserType::Firefox.as_str(), "firefox");
        assert_eq!(BrowserType::Edge.as_str(), "edge");
    }

    #[test]
    fn test_parse_browser_type() {
        assert_eq!(parse_browser_type("chrome"), Some(BrowserType::Chrome));
        assert_eq!(parse_browser_type("CHROME"), Some(BrowserType::Chrome));
        assert_eq!(parse_browser_type("firefox"), Some(BrowserType::Firefox));
        assert_eq!(parse_browser_type("edge"), Some(BrowserType::Edge));
        assert_eq!(parse_browser_type("safari"), None);
        assert_eq!(parse_browser_type(""), None);
    }

    #[test]
    fn test_build_cookie_args_none() {
        let args = build_cookie_args(&CookieSource::None);
        assert_eq!(args, Vec::<String>::new());
    }

    #[test]
    fn test_build_cookie_args_browser() {
        let args = build_cookie_args(&CookieSource::Browser(BrowserType::Chrome));
        assert_eq!(args, vec!["--cookies-from-browser", "chrome"]);

        let edge_args = build_cookie_args(&CookieSource::Browser(BrowserType::Edge));
        assert_eq!(edge_args, vec!["--cookies-from-browser", "edge"]);
    }

    #[test]
    fn test_build_cookie_args_file() {
        let path = "C:\\secrets\\cookies.txt".to_string();
        let args = build_cookie_args(&CookieSource::File(path.clone()));
        assert_eq!(args, vec!["--cookies", path.as_str()]);
    }

    #[test]
    fn test_user_agent_rotation() {
        let ua1 = get_rotated_desktop_user_agent();
        let ua2 = get_rotated_desktop_user_agent();
        assert!(ua1.contains("Windows NT 10.0"));
        assert!(ua2.contains("Windows NT 10.0"));
    }

    #[test]
    fn test_build_bot_defense_args() {
        let args = build_bot_defense_args(
            &CookieSource::Browser(BrowserType::Firefox),
            Some("CustomUA/1.0"),
        );
        assert_eq!(args[0], "--cookies-from-browser");
        assert_eq!(args[1], "firefox");
        assert_eq!(args[2], "--user-agent");
        assert_eq!(args[3], "CustomUA/1.0");
        assert_eq!(args[4], "--add-header");
        assert_eq!(args[5], "Accept-Language: en-US,en;q=0.9");
    }

    #[test]
    fn test_validate_bot_defense_inputs_rejects_injection_and_nul() {
        assert!(
            validate_bot_defense_inputs(&CookieSource::File("-evil.txt".to_string()), None)
                .is_err()
        );
        assert!(
            validate_bot_defense_inputs(&CookieSource::File("--exec=calc".to_string()), None)
                .is_err()
        );
        assert!(validate_bot_defense_inputs(
            &CookieSource::File("C:\\cookies\0.txt".to_string()),
            None
        )
        .is_err());
        assert!(validate_bot_defense_inputs(&CookieSource::File("   ".to_string()), None).is_err());

        assert!(validate_bot_defense_inputs(
            &CookieSource::None,
            Some("Mozilla/5.0\r\n--exec=calc")
        )
        .is_err());
        assert!(
            validate_bot_defense_inputs(&CookieSource::None, Some("Mozilla/5.0\nEvil")).is_err()
        );
        assert!(
            validate_bot_defense_inputs(&CookieSource::None, Some("Mozilla/5.0\0Evil")).is_err()
        );
        assert!(validate_custom_user_agent(Some("Mozilla/5.0\x1b[31mEvil")).is_err());
        assert!(validate_custom_user_agent(Some("Mozilla/5.0\tEvil")).is_err());
        assert!(validate_custom_user_agent(Some("-evil-flag")).is_err());
        assert!(validate_custom_user_agent(Some("  --exec=calc")).is_err());
        assert!(validate_custom_user_agent(Some("   ")).is_ok());
        assert!(validate_custom_user_agent(Some("Mozilla/5.0 Custom")).is_ok());

        // Infallible builders also neutralize unsafe inputs cleanly
        assert_eq!(
            build_cookie_args(&CookieSource::File("-evil.txt".to_string())),
            Vec::<String>::new()
        );
        let safe_args = build_bot_defense_args(&CookieSource::None, Some("BadUA\r\nInjected"));
        let ua_idx = safe_args.iter().position(|a| a == "--user-agent").unwrap();
        assert!(!safe_args[ua_idx + 1].contains('\r'));
        assert!(!safe_args[ua_idx + 1].contains('\n'));

        let safe_dash_args = build_bot_defense_args(&CookieSource::None, Some("--exec=calc"));
        let dash_ua_idx = safe_dash_args
            .iter()
            .position(|a| a == "--user-agent")
            .unwrap();
        assert!(!safe_dash_args[dash_ua_idx + 1].starts_with('-'));

        let safe_ctrl_args = build_bot_defense_args(&CookieSource::None, Some("Bad\x1bUA\tTab"));
        let ctrl_ua_idx = safe_ctrl_args
            .iter()
            .position(|a| a == "--user-agent")
            .unwrap();
        assert!(!safe_ctrl_args[ctrl_ua_idx + 1]
            .chars()
            .any(char::is_control));
    }
}
