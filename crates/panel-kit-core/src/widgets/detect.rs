//! Pure string detectors shared by badge painters.
//!
//! Ported from jump-cannon's `badges.rs` (itself a port of the egui
//! graph-renderer's `modal/detect.rs` + `modal/badges.rs::status_color`).
//! Detection order matters for callers: wikilink, url, status, date, ticket,
//! then a plain value.

/// Semantic role a status string resolves to. Painters map the role onto
/// their renderer's palette.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusColor {
    /// Healthy / settled states (`active`, `done`, `ok`, `ready`, `passed`).
    Success,
    /// Failing states (`failed`, `blocked`, `broken`, `error`).
    Failure,
    /// Attention states (`needs-review`, `needs-fetch`, `in-progress`, `wip`).
    Warning,
    /// Draft-ish states (`draft`, `pending`) — the original mapping kept a
    /// fixed accent blue for these.
    Draft,
    /// Retired states (`archived`, `deprecated`, `stale`) — plain text colour.
    Neutral,
}

/// Status string → semantic colour role. Case-insensitive; `None` for
/// anything not in the mapping.
pub fn status_color(s: &str) -> Option<StatusColor> {
    match s.to_ascii_lowercase().as_str() {
        "active" | "done" | "ok" | "ready" | "passed" => Some(StatusColor::Success),
        "failed" | "blocked" | "broken" | "error" => Some(StatusColor::Failure),
        "needs-review" | "needs-fetch" | "in-progress" | "wip" => Some(StatusColor::Warning),
        "draft" | "pending" => Some(StatusColor::Draft),
        "archived" | "deprecated" | "stale" => Some(StatusColor::Neutral),
        _ => None,
    }
}

/// Parse a `[[page]]` or `[[page|alias]]` wikilink into `(page, alias)`.
pub fn parse_wikilink(s: &str) -> Option<(String, Option<String>)> {
    let s = s.trim();
    if !(s.starts_with("[[") && s.ends_with("]]") && s.len() >= 5) {
        return None;
    }
    let inner = &s[2..s.len() - 2];
    if inner.is_empty() {
        return None;
    }
    if let Some((page, alias)) = inner.split_once('|') {
        Some((page.trim().to_string(), Some(alias.trim().to_string())))
    } else {
        Some((inner.trim().to_string(), None))
    }
}

/// The host part of an `http(s)://` URL, up to the first `/`, `:`, or `?`.
pub fn host_from_url(s: &str) -> Option<String> {
    let rest = s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))?;
    let host = rest
        .split(|c: char| c == '/' || c == ':' || c == '?')
        .next()?;
    Some(host.to_string())
}

/// Whether the whole string is an `http(s)://` URL: no whitespace, under 2 KiB.
pub fn is_url(s: &str) -> bool {
    (s.starts_with("http://") || s.starts_with("https://"))
        && !s.contains(char::is_whitespace)
        && s.len() < 2048
}

/// `YYYY-MM-DD` with optional time tail. Strict on the date head.
pub fn is_iso_date(s: &str) -> bool {
    if s.len() < 10 {
        return false;
    }
    let bytes = s.as_bytes();
    bytes[..4].iter().all(u8::is_ascii_digit)
        && bytes[4] == b'-'
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[7] == b'-'
        && bytes[8..10].iter().all(u8::is_ascii_digit)
        && (s.len() == 10 || matches!(bytes[10], b'T' | b' '))
}

/// Parse a JIRA-style ticket id at the start of the string. Accepts
/// `FOO-123`, `FOO-123: subject`, `FOO-123 — subject`. Returns the bare id.
pub fn parse_ticket_id(s: &str) -> Option<String> {
    let head: &str = s.split([':', ' ']).next()?;
    if head.len() < 3 || head.len() > 24 {
        return None;
    }
    let (prefix, rest) = head.split_once('-')?;
    if prefix.is_empty() || prefix.len() > 16 {
        return None;
    }
    let mut chars = prefix.chars();
    let first = chars.next()?;
    if !first.is_ascii_uppercase() {
        return None;
    }
    if !chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()) {
        return None;
    }
    if rest.is_empty() || !rest.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(head.to_string())
}

#[cfg(test)]
mod tests {
    use super::{host_from_url, is_iso_date, is_url, parse_ticket_id, parse_wikilink, status_color, StatusColor};

    #[test]
    fn status_palette() {
        assert_eq!(status_color("active"), Some(StatusColor::Success));
        assert_eq!(status_color("DONE"), Some(StatusColor::Success));
        assert_eq!(status_color("failed"), Some(StatusColor::Failure));
        assert_eq!(status_color("needs-review"), Some(StatusColor::Warning));
        assert_eq!(status_color("draft"), Some(StatusColor::Draft));
        assert_eq!(status_color("archived"), Some(StatusColor::Neutral));
        assert_eq!(status_color("foobar"), None);
    }

    #[test]
    fn wikilink_basic() {
        assert_eq!(parse_wikilink("[[Alpha]]"), Some(("Alpha".into(), None)));
        assert_eq!(
            parse_wikilink("[[Alpha|alias]]"),
            Some(("Alpha".into(), Some("alias".into())))
        );
        assert_eq!(parse_wikilink("Alpha"), None);
        assert_eq!(parse_wikilink("[[]]"), None);
    }

    #[test]
    fn url_detect() {
        assert!(is_url("https://example.com"));
        assert!(is_url("http://luna.local:3000/d/x"));
        assert!(!is_url("example.com"));
        assert!(!is_url("not a url"));
        assert_eq!(host_from_url("http://luna.local:3000/d/x"), Some("luna.local".into()));
    }

    #[test]
    fn iso_date_detect() {
        assert!(is_iso_date("2026-04-30"));
        assert!(is_iso_date("2026-04-30T12:00:00Z"));
        assert!(!is_iso_date("April 30"));
        assert!(!is_iso_date("2026-4-1"));
    }

    #[test]
    fn ticket_id_detect() {
        assert_eq!(parse_ticket_id("ITHELP-1234"), Some("ITHELP-1234".into()));
        assert_eq!(parse_ticket_id("JIRA-42: subject"), Some("JIRA-42".into()));
        assert_eq!(parse_ticket_id("FOO-7 — body"), Some("FOO-7".into()));
        assert_eq!(parse_ticket_id("not-a-ticket"), None);
        assert_eq!(parse_ticket_id("foo-1"), None);
    }
}
