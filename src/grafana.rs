//! Grafana iframe embed components.
//!
//! [`GrafanaDashboard`] embeds a whole dashboard through `/d/` with Grafana's
//! chrome hidden by `kiosk`. [`GrafanaPanel`] embeds one visualization through
//! `/d-solo/`. Both expose Grafana template variables, theme, and time-range
//! query parameters and retain an always-visible path to open Grafana itself.
//!
//! The iframe is cross-origin, so panel-kit cannot inspect Grafana's DOM or
//! reliably detect an `X-Frame-Options` failure. The open-in-new-tab link is
//! therefore always rendered rather than being conditional on a detectable
//! error. Grafana must be configured with `[security] allow_embedding = true`;
//! anonymous access or a usable viewer session is also required.

use crate::loading::ProgressBar;
use dioxus::prelude::*;

/// Percent-encode a Grafana query value without adding a URL dependency.
fn encode_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' | '*' | ':' | '/' => {
                out.push(ch)
            }
            ' ' => out.push_str("%20"),
            other => {
                let mut buf = [0_u8; 4];
                for byte in other.encode_utf8(&mut buf).bytes() {
                    out.push_str(&format!("%{byte:02X}"));
                }
            }
        }
    }
    out
}

/// Build `{base}/{endpoint}/{uid}[/{slug}]` with no duplicate separator.
fn build_path(base_url: &str, dashboard_uid: &str, slug: Option<&str>, endpoint: &str) -> String {
    let base = base_url.trim_end_matches('/');
    match slug {
        Some(slug) if !slug.is_empty() => format!("{base}/{endpoint}/{dashboard_uid}/{slug}"),
        _ => format!("{base}/{endpoint}/{dashboard_uid}"),
    }
}

/// Build the query shared by iframe and open-in-Grafana URLs.
fn build_query(
    extra: &[String],
    vars: &[(String, String)],
    theme: Option<&str>,
    from: Option<&str>,
    to: Option<&str>,
) -> String {
    let mut query = format!(
        "?from={}&to={}",
        encode_value(from.unwrap_or("now-6h")),
        encode_value(to.unwrap_or("now"))
    );

    for fragment in extra {
        query.push('&');
        query.push_str(fragment);
    }
    if let Some(theme) = theme.filter(|theme| !theme.is_empty()) {
        query.push_str("&theme=");
        query.push_str(&encode_value(theme));
    }
    for (key, value) in vars {
        query.push_str("&var-");
        query.push_str(&encode_value(key));
        query.push('=');
        query.push_str(&encode_value(value));
    }

    query
}

/// Shared iframe chrome with an honest pending state and permanent escape hatch.
#[component]
fn GrafanaFrame(src: String, open_url: String, title: String) -> Element {
    let mut loaded = use_signal(|| false);

    // Dioxus reuses the iframe node when props change. Reset explicitly so a
    // re-scoped dashboard reports that its new request is pending.
    use_effect(use_reactive(&src, move |_| loaded.set(false)));

    rsx! {
        div { class: "grafana-panel",
            iframe {
                class: "grafana-frame",
                src: "{src}",
                title: "{title}",
                // Grafana visualizations are interactive and may request their
                // own scrolling or fullscreen behavior.
                allow: "fullscreen",
                // Deliberately NOT loading="lazy": a panel that is minimized,
                // docked, or scrolled out of the workspace never fetches, so
                // re-scoping it leaves the loading state up with no load event
                // ever coming. Eager costs one request the user asked for anyway.
                onload: move |_| loaded.set(true),
            }
            if !loaded() {
                div { class: "grafana-loading",
                    ProgressBar {
                        fraction: None,
                        label: "Loading Grafana…".to_string(),
                    }
                }
            }
            a {
                class: "grafana-open",
                href: "{open_url}",
                target: "_blank",
                rel: "noopener noreferrer",
                title: "Open in Grafana",
                "open ↗"
            }
        }
    }
}

/// Embed a complete Grafana dashboard via `/d/` with `kiosk` enabled.
#[component]
pub fn GrafanaDashboard(
    /// Grafana base URL, with or without a trailing slash.
    base_url: String,
    /// Stable dashboard UID from Grafana's dashboard URL.
    dashboard_uid: String,
    /// Optional human-readable dashboard slug.
    #[props(default)]
    slug: Option<String>,
    /// Template variables emitted as `var-<key>=<value>` query parameters.
    #[props(default)]
    vars: Vec<(String, String)>,
    /// Optional Grafana theme (`light`, `dark`, or `system`).
    #[props(default)]
    theme: Option<String>,
    /// Grafana time-range start. Defaults to `now-6h`.
    #[props(default)]
    from: Option<String>,
    /// Grafana time-range end. Defaults to `now`.
    #[props(default)]
    to: Option<String>,
    /// Accessible iframe title.
    #[props(default = "Grafana dashboard".to_string())]
    title: String,
) -> Element {
    let path = build_path(&base_url, &dashboard_uid, slug.as_deref(), "d");
    let query = |extra: &[String]| {
        build_query(
            extra,
            &vars,
            theme.as_deref(),
            from.as_deref(),
            to.as_deref(),
        )
    };

    rsx! {
        GrafanaFrame {
            src: format!("{path}{}", query(&["kiosk".to_string()])),
            open_url: format!("{path}{}", query(&[])),
            title,
        }
    }
}

/// Embed one Grafana visualization via `/d-solo/`.
#[component]
pub fn GrafanaPanel(
    /// Grafana base URL, with or without a trailing slash.
    base_url: String,
    /// Stable dashboard UID from Grafana's dashboard URL.
    dashboard_uid: String,
    /// Numeric panel ID from the dashboard JSON.
    panel_id: u32,
    /// Optional human-readable dashboard slug.
    #[props(default)]
    slug: Option<String>,
    /// Template variables emitted as `var-<key>=<value>` query parameters.
    #[props(default)]
    vars: Vec<(String, String)>,
    /// Optional Grafana theme (`light`, `dark`, or `system`).
    #[props(default)]
    theme: Option<String>,
    /// Grafana time-range start. Defaults to `now-6h`.
    #[props(default)]
    from: Option<String>,
    /// Grafana time-range end. Defaults to `now`.
    #[props(default)]
    to: Option<String>,
    /// Accessible iframe title.
    #[props(default = "Grafana panel".to_string())]
    title: String,
) -> Element {
    let query = |extra: &[String]| {
        build_query(
            extra,
            &vars,
            theme.as_deref(),
            from.as_deref(),
            to.as_deref(),
        )
    };
    let solo = build_path(&base_url, &dashboard_uid, slug.as_deref(), "d-solo");
    // The escape hatch opens the panel in its dashboard, not another solo frame.
    let dashboard = build_path(&base_url, &dashboard_uid, slug.as_deref(), "d");

    rsx! {
        GrafanaFrame {
            src: format!("{solo}{}", query(&[format!("panelId={panel_id}")])),
            open_url: format!(
                "{dashboard}{}",
                query(&[format!("viewPanel={panel_id}")]),
            ),
            title,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn variables(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect()
    }

    #[test]
    fn panel_url_uses_d_solo_and_panel_id() {
        let path = build_path("https://g.example.com", "uid1", Some("overview"), "d-solo");
        let query = build_query(&["panelId=8".to_string()], &[], None, None, None);
        assert_eq!(
            format!("{path}{query}"),
            "https://g.example.com/d-solo/uid1/overview?from=now-6h&to=now&panelId=8"
        );
    }

    #[test]
    fn dashboard_url_strips_base_slash_and_enables_kiosk() {
        let path = build_path("https://g.example.com/", "uid1", Some("overview"), "d");
        let query = build_query(&["kiosk".to_string()], &[], None, None, None);
        assert_eq!(
            format!("{path}{query}"),
            "https://g.example.com/d/uid1/overview?from=now-6h&to=now&kiosk"
        );
    }

    #[test]
    fn query_includes_theme_variables_and_time_range() {
        let query = build_query(
            &["panelId=3".to_string()],
            &variables(&[("namespace", "prod"), ("pod", "a b")]),
            Some("dark"),
            Some("now-24h"),
            Some("now-1h"),
        );
        assert_eq!(
            query,
            "?from=now-24h&to=now-1h&panelId=3&theme=dark&var-namespace=prod&var-pod=a%20b"
        );
    }

    #[test]
    fn encoding_protects_query_boundaries() {
        assert_eq!(encode_value("a b&c=d"), "a%20b%26c%3Dd");
        assert_eq!(encode_value("plain-Value_1.0"), "plain-Value_1.0");
    }
}
