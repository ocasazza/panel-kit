//! Gate: forbid colour literals and retired variable names in panel-kit.css.
//!
//! `--tag-<digits>: <hex>;` and `--shadow-ink: <hex>;` are the only permitted
//! colour literals outside the generated theme region (which is owned by the
//! core emitter and is skipped entirely).

use std::collections::HashSet;
use std::sync::LazyLock;

static COLOUR_SET: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| NAMED_COLOURS.iter().copied().collect());

/// Return a violation message for each illegal colour reference in `css`.
///
/// Each message is `assets/panel-kit.css:<line>: <what> in '<trimmed line>'`.
/// The generated-theme region (delimited by `CSS_THEME_REGION_BEGIN` /
/// `CSS_THEME_REGION_END` from `panel_kit_core::theme`) is exempt.
pub fn css_colour_violations(css: &str) -> Vec<String> {
    let clean = strip_comments(css);
    let generated = match (
        css.find(panel_kit_core::theme::CSS_THEME_REGION_BEGIN),
        css.find(panel_kit_core::theme::CSS_THEME_REGION_END),
    ) {
        (Some(lo), Some(hi)) => Some((lo, hi + 1)),
        _ => None,
    };

    // Declarations, not lines: a rule written on one line still carries its
    // property after the selector's brace, and a permitted declaration is
    // exempt wherever it sits.
    let bytes = clean.as_bytes();
    let mut results = Vec::new();
    let mut line_no = 1_usize;
    let mut decl_start = 0_usize;
    let mut i = 0_usize;
    while i <= bytes.len() {
        let at_end = i == bytes.len();
        let separator = !at_end && matches!(bytes[i], b';' | b'{' | b'}');
        if !separator && !at_end {
            if bytes[i] == b'\n' {
                line_no += 1;
            }
            i += 1;
            continue;
        }

        let raw = &clean[decl_start..i];
        let decl_line = line_no - raw.bytes().filter(|byte| *byte == b'\n').count();
        let spans_generated = generated
            .is_some_and(|(lo, hi)| decl_start >= lo && i + 1 <= hi);
        let decl = raw.trim();
        decl_start = i + 1;
        i += 1;

        if at_end || decl.is_empty() || spans_generated || is_permitted_declaration(decl) {
            continue;
        }
        let Some(colon) = decl.find(':') else { continue };
        let prop = decl[..colon].trim();
        if prop.is_empty() || prop.starts_with('@') {
            continue;
        }
        let value = decl[colon + 1..].trim();

        check_hex_literal(value, decl, decl_line, &mut results);
        check_colour_fns(value, decl, decl_line, &mut results);
        check_named_colour(value, decl, decl_line, &mut results);
        check_retired_vars(value, decl, decl_line, &mut results);
    }

    results
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Replace `/* ... */` comment text with spaces, preserving newlines.
fn strip_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut in_comment = false;
    let mut i = 0;
    let bytes = css.as_bytes();
    while i < bytes.len() {
        if !in_comment && i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            in_comment = true;
            out.push(' ');
            out.push(' ');
            i += 2;
        } else if in_comment && i + 1 < bytes.len() && bytes[i] == b'*' && bytes[i + 1] == b'/' {
            in_comment = false;
            out.push(' ');
            out.push(' ');
            i += 2;
        } else if in_comment {
            let c = bytes[i] as char;
            out.push(if c == '\n' { '\n' } else { ' ' });
            i += 1;
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

fn slice_at(text: &str, lo: usize, hi: usize) -> &str {
    let lo = lo.min(text.len());
    let hi = hi.min(text.len());
    &text[lo..hi]
}

/// This (cleaned) line is a `--tag-N:` or `--shadow-ink:` declaration — skip.
fn is_permitted_declaration(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.starts_with("--tag-") && trimmed.contains(':') {
        // Permit only `--tag-<digits>:`, not `--tag-ink-*` etc.
        let digits = &trimmed[6..]; // after "--tag-"
        if let Some(colon) = digits.find(':') {
            if digits[..colon].chars().all(|c| c.is_ascii_digit()) {
                return true;
            }
        }
    }
    if trimmed.starts_with("--shadow-ink") && trimmed.contains(':') {
        return true;
    }
    false
}

/// Token boundary check.
fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

// ---------------------------------------------------------------------------
// Violation checks
// ---------------------------------------------------------------------------

fn check_hex_literal(value: &str, decl: &str, line_no: usize, out: &mut Vec<String>) {
    let bytes = value.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'#' {
            let start = i;
            i += 1;
            let mut digits = 0;
            while i < bytes.len() && bytes[i].is_ascii_hexdigit() {
                digits += 1;
                i += 1;
            }
            let matched = matches!(digits, 3 | 4 | 6 | 8);
            if matched {
                let before_ok = start == 0 || !is_ident_byte(bytes[start - 1]);
                let after_ok = i >= bytes.len() || !is_ident_byte(bytes[i]);
                if before_ok && after_ok {
                    let snippet = &value[start..(start + 1 + digits).min(value.len())];
                    let snippet = snippet.trim();
                    out.push(violation(line_no, &format!("hex colour literal {snippet}"), decl));
                    return;
                }
            }
        } else {
            i += 1;
        }
    }
}

fn check_colour_fns(value: &str, decl: &str, line_no: usize, out: &mut Vec<String>) {
    for pat in [
        "rgb(", "rgba(", "hsl(", "hsla(", "hwb(", "lab(", "lch(", "oklab(", "oklch(",
    ] {
        if value.contains(pat) {
            out.push(violation(line_no, &format!("colour function {pat}"), decl));
            return;
        }
    }
    // `color(` — but not `color-mix(`, `color-scheme`, etc.
    if let Some(pos) = value.find("color(") {
        let preceded_by_ident = pos > 0 && is_ident_byte(value.as_bytes()[pos - 1]);
        if !preceded_by_ident {
            out.push(violation(line_no, "colour function color(", decl));
        }
    }
}

/// A named colour (CSS keyword) in a declaration value.
fn check_named_colour(value: &str, decl: &str, line_no: usize, out: &mut Vec<String>) {
    for token in value.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_')) {
        if token.is_empty() || token.starts_with("--") || token.starts_with('#') {
            continue;
        }
        if COLOUR_SET.contains(&token) {
            out.push(violation(
                line_no,
                &format!("named colour `{token}`"),
                decl,
            ));
        }
    }
}

fn check_retired_vars(value: &str, decl: &str, line_no: usize, out: &mut Vec<String>) {
    let mut search_from = 0;
    while let Some(pos) = value[search_from..].find("var(") {
        let abs = search_from + pos + 4; // after "var("
        let rest = &value[abs..];

        let Some(name_part) = rest.strip_prefix("--") else {
            search_from = abs;
            continue;
        };

        // Read until next non-ident, non-dash char, comma, or paren.
        let end = name_part
            .find(|c: char| !is_ident_byte(c as u8) && c != '-')
            .unwrap_or(name_part.len());
        let name = &name_part[..end].trim_end_matches('-');

        if RETIRED_NAMES.contains(name) {
            out.push(violation(
                line_no,
                &format!("retired var(--{name})"),
                decl,
            ));
            return;
        }

        search_from = abs + 2 + end;
    }
}

fn violation(line_no: usize, what: &str, raw_line: &str) -> String {
    let snippet: String = raw_line.trim().chars().take(96).collect();
    format!("assets/panel-kit.css:{line_no}: {what} in `{snippet}`")
}

// ---------------------------------------------------------------------------
// Colour-bearing properties (where a named colour is legal)
// ---------------------------------------------------------------------------

fn is_colour_bearing_decl(trimmed: &str) -> bool {
    let Some(colon) = trimmed.find(':') else { return false };
    let prop = trimmed[..colon].trim().to_lowercase();
    if prop.is_empty() {
        return false;
    }
    COLOUR_PROPS.iter().any(|&p| p == prop.as_str()) || prop_matches_prefix(&prop)
}

fn prop_matches_prefix(prop: &str) -> bool {
    for prefix in &["border-", "outline-", "column-rule-"] {
        if prop.starts_with(prefix) && prop.ends_with("-color") {
            return true;
        }
    }
    // `border-top`, `border-right`, etc. are shorthands that include colour.
    prop.starts_with("border-")
}

static COLOUR_PROPS: &[&str] = &[
    "color",
    "background",
    "background-color",
    "border",
    "border-top",
    "border-right",
    "border-bottom",
    "border-left",
    "outline",
    "box-shadow",
    "text-shadow",
    "fill",
    "stroke",
    "caret-color",
    "accent-color",
    "scrollbar-color",
    "text-decoration-color",
    "column-rule-color",
    "text-emphasis-color",
    "-webkit-text-fill-color",
    "-webkit-text-stroke-color",
];

// ---------------------------------------------------------------------------
// Static data
// ---------------------------------------------------------------------------

static NAMED_COLOURS: &[&str] = &[
    "aliceblue",
    "antiquewhite",
    "aqua",
    "aquamarine",
    "azure",
    "beige",
    "bisque",
    "black",
    "blanchedalmond",
    "blue",
    "blueviolet",
    "brown",
    "burlywood",
    "cadetblue",
    "chartreuse",
    "chocolate",
    "coral",
    "cornflowerblue",
    "cornsilk",
    "crimson",
    "cyan",
    "darkblue",
    "darkcyan",
    "darkgoldenrod",
    "darkgray",
    "darkgreen",
    "darkgrey",
    "darkkhaki",
    "darkmagenta",
    "darkolivegreen",
    "darkorange",
    "darkorchid",
    "darkred",
    "darksalmon",
    "darkseagreen",
    "darkslateblue",
    "darkslategray",
    "darkslategrey",
    "darkturquoise",
    "darkviolet",
    "deeppink",
    "deepskyblue",
    "dimgray",
    "dimgrey",
    "dodgerblue",
    "firebrick",
    "floralwhite",
    "forestgreen",
    "fuchsia",
    "gainsboro",
    "ghostwhite",
    "gold",
    "goldenrod",
    "gray",
    "green",
    "greenyellow",
    "grey",
    "honeydew",
    "hotpink",
    "indianred",
    "indigo",
    "ivory",
    "khaki",
    "lavender",
    "lavenderblush",
    "lawngreen",
    "lemonchiffon",
    "lightblue",
    "lightcoral",
    "lightcyan",
    "lightgoldenrodyellow",
    "lightgray",
    "lightgreen",
    "lightgrey",
    "lightpink",
    "lightsalmon",
    "lightseagreen",
    "lightskyblue",
    "lightslategray",
    "lightslategrey",
    "lightsteelblue",
    "lightyellow",
    "lime",
    "limegreen",
    "linen",
    "magenta",
    "maroon",
    "mediumaquamarine",
    "mediumblue",
    "mediumorchid",
    "mediumpurple",
    "mediumseagreen",
    "mediumslateblue",
    "mediumspringgreen",
    "mediumturquoise",
    "mediumvioletred",
    "midnightblue",
    "mintcream",
    "mistyrose",
    "moccasin",
    "navajowhite",
    "navy",
    "oldlace",
    "olive",
    "olivedrab",
    "orange",
    "orangered",
    "orchid",
    "palegoldenrod",
    "palegreen",
    "paleturquoise",
    "palevioletred",
    "papayawhip",
    "peachpuff",
    "peru",
    "pink",
    "plum",
    "powderblue",
    "purple",
    "rebeccapurple",
    "red",
    "rosybrown",
    "royalblue",
    "saddlebrown",
    "salmon",
    "sandybrown",
    "seagreen",
    "seashell",
    "sienna",
    "silver",
    "skyblue",
    "slateblue",
    "slategray",
    "slategrey",
    "snow",
    "springgreen",
    "steelblue",
    "tan",
    "teal",
    "thistle",
    "tomato",
    "turquoise",
    "violet",
    "wheat",
    "white",
    "whitesmoke",
    "yellow",
    "yellowgreen",
];

static RETIRED_NAMES: &[&str] = &[
    "bg",
    "panel",
    "fg",
    "dim",
    "line",
    "line2",
    "inv-bg",
    "inv-fg",
    "accent",
    "red",
    "yellow",
    "green",
    "blue",
    "pink",
    "badge-info",
    "focus-ring",
    "border",
    "surface-2",
];

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_css_has_zero_violations() {
        let violations = css_colour_violations(crate::CSS);
        assert!(
            violations.is_empty(),
            "{} panel-kit.css colour violations:\n{}",
            violations.len(),
            violations.join("\n"),
        );
    }

    #[test]
    fn hex_in_regular_rule_is_flagged() {
        let css = "/* gen region */\n/* BEGIN GENERATED THEME TOKENS */\n\na{}\n/* END GENERATED THEME TOKENS */\n.x { color: #fff; }\n";
        let v = css_colour_violations(css);
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(v[0].starts_with("assets/panel-kit.css:6: hex"), "{v:?}");
    }

    #[test]
    fn rgb_is_flagged() {
        let css = "/* BEGIN GENERATED THEME TOKENS */\n\n/* END GENERATED THEME TOKENS */\n.x { color: rgb(0,0,0); }\n";
        let v = css_colour_violations(css);
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(v[0].starts_with("assets/panel-kit.css:4: colour function rgb("), "{v:?}");
    }

    #[test]
    fn hsl_is_flagged() {
        let css = "/* BEGIN GENERATED THEME TOKENS */\n\n/* END GENERATED THEME TOKENS */\n.x { color: hsl(0,0%,0%); }\n";
        let v = css_colour_violations(css);
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(v[0].starts_with("assets/panel-kit.css:4: colour function hsl("), "{v:?}");
    }

    #[test]
    fn named_colour_is_flagged() {
        let css = "/* BEGIN GENERATED THEME TOKENS */\n\n/* END GENERATED THEME TOKENS */\n.x { background: white; }\n";
        let v = css_colour_violations(css);
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(v[0].contains("named colour `white`"), "{v:?}");
    }

    #[test]
    fn retired_var_is_flagged() {
        let css = "/* BEGIN GENERATED THEME TOKENS */\n\n/* END GENERATED THEME TOKENS */\n.x { color: var(--bg); }\n";
        let v = css_colour_violations(css);
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(v[0].starts_with("assets/panel-kit.css:4"), "{v:?}");
        assert!(v[0].contains("retired var(--bg)"), "{v:?}");
    }

    #[test]
    fn retired_var_fallback_is_flagged() {
        let css = "/* BEGIN GENERATED THEME TOKENS */\n\n/* END GENERATED THEME TOKENS */\n.x { outline: 1px solid var(--border, #2a2a2a); }\n";
        let v = css_colour_violations(css);
        // Both are violations of the same declaration: the retired name and
        // the literal fallback it carries.
        assert_eq!(v.len(), 2, "{v:?}");
        assert!(v.iter().any(|m| m.contains("retired var(--border)")), "{v:?}");
        assert!(v.iter().any(|m| m.contains("hex colour literal #2a2a2a")), "{v:?}");
    }

    #[test]
    fn literal_inside_generated_region_is_not_flagged() {
        let css = "/* BEGIN GENERATED THEME TOKENS */\n.x { color: #fff; }\n/* END GENERATED THEME TOKENS */\n";
        let v = css_colour_violations(css);
        assert!(v.is_empty(), "should skip generated region, got {v:?}");
    }

    #[test]
    fn literal_inside_comment_is_not_flagged() {
        let css = "/* BEGIN GENERATED THEME TOKENS */\n\n/* END GENERATED THEME TOKENS */\n/* this is a #fff comment */\n.x { color: var(--color-text); }\n";
        let v = css_colour_violations(css);
        assert!(v.is_empty(), "comment should be skipped, got {v:?}");
    }

    #[test]
    fn tag_slot_declaration_is_exempt() {
        let css = "/* BEGIN GENERATED THEME TOKENS */\n\n/* END GENERATED THEME TOKENS */\n:root { --tag-3: #36ae0e; }\n";
        let v = css_colour_violations(css);
        assert!(v.is_empty(), "tag slot should be exempt, got {v:?}");
    }

    #[test]
    fn shadow_ink_declaration_is_exempt() {
        let css = "/* BEGIN GENERATED THEME TOKENS */\n\n/* END GENERATED THEME TOKENS */\n:root { --shadow-ink: #000; }\n";
        let v = css_colour_violations(css);
        assert!(v.is_empty(), "shadow-ink should be exempt, got {v:?}");
    }

    #[test]
    fn planted_literal_on_real_css_fails() {
        let planted = format!("{}.x {{ color: #fff; }}\n", crate::CSS);
        let v = css_colour_violations(&planted);
        assert_eq!(v.len(), 1, "{v:?}");
        // The stylesheet ends with a newline, so the planted rule is the next line.
        let expected_line = crate::CSS.lines().count() + 1;
        assert!(
            v[0].starts_with(&format!("assets/panel-kit.css:{expected_line}:")),
            "{v:?}",
        );
    }
}