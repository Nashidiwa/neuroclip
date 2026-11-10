//! Cookie loading and User-Agent rotation subsystem for yt-dlp anti-bot defense.
//!
//! Complies with AC-06.4, ED-06.3, and docs/CONVENTIONS.md (zero-panic invariant).

use std::sync::atomic::{AtomicUsize, Ordering};
use serde::{Deserialize, Serialize};

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
            vec!["--cookies".to_string(), path.clone()]
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
        Some(u) if !u.trim().is_empty() => u.trim(),
        _ => get_rotated_desktop_user_agent(),
    };

    args.push("--user-agent".to_string());
    args.push(ua.to_string());

    args.push("--add-header".to_string());
    args.push("Accept-Language: en-US,en;q=0.9".to_string());

    args
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
        assert!(args.is_empty());
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
}
