//! Semantic snapshot helpers for widget SSR tests.

/// Assert a normalized semantic snapshot of every rendered widget node and text value.
pub(super) fn assert_widget_snapshot(html: &str, expected: &str) {
    let actual = widget_snapshot(html);
    let expected = expected.trim();

    assert_eq!(
        actual, expected,
        "widget SSR semantic snapshot drifted.\nHTML:\n{html}"
    );
}

/// Convert SSR HTML into a stable semantic snapshot without ignoring attributes.
fn widget_snapshot(html: &str) -> String {
    let mut lines = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut rest = html;

    while let Some(open) = rest.find('<') {
        capture_text(&mut lines, &stack, &rest[..open]);
        rest = &rest[open + 1..];

        let Some(close) = rest.find('>') else { break };
        let tag_source = &rest[..close];
        rest = &rest[close + 1..];

        if tag_source.starts_with('/') {
            stack.pop();
            continue;
        }
        if tag_source.starts_with('!') {
            continue;
        }

        let self_closing = tag_source.ends_with('/');
        let (tag, attrs) = split_tag(tag_source);
        let descriptor = node_descriptor(tag, attr(attrs, "class").as_deref());
        stack.push(descriptor);
        lines.push(format!(
            "node {}{}",
            stack.join(" > "),
            attribute_snapshot(attrs)
        ));

        if self_closing {
            stack.pop();
        }
    }

    capture_text(&mut lines, &stack, rest);
    lines.join("\n")
}

/// Record non-empty normalized text at the current semantic path.
fn capture_text(lines: &mut Vec<String>, stack: &[String], text: &str) {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return;
    }

    lines.push(format!("text {} => {normalized}", stack.join(" > ")));
}

/// Split an opening tag into its tag name and raw attribute suffix.
fn split_tag(source: &str) -> (&str, &str) {
    source
        .split_once(char::is_whitespace)
        .map_or((source.trim_end_matches('/'), ""), |(tag, attrs)| {
            (tag, attrs)
        })
}

/// Extract a double-quoted HTML attribute from an SSR tag.
fn attr(attrs: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=\"");
    let start = attrs.find(&needle)? + needle.len();
    let value = attrs[start..].split_once('"')?.0;
    Some(value.to_string())
}

/// Append every quoted attribute in rendered order so additions cannot hide.
fn attribute_snapshot(attrs: &str) -> String {
    let mut snapshot = String::new();
    let mut rest = attrs.trim().trim_end_matches('/').trim();

    while let Some((name, after_name)) = rest.split_once('=') {
        let name = name.trim();
        let after_name = after_name.trim_start();
        if !after_name.starts_with('"') {
            break;
        }

        let Some((value, after_value)) = after_name[1..].split_once('"') else {
            break;
        };
        snapshot.push_str(&format!(" {name}=\"{value}\""));
        rest = after_value.trim_start();
    }

    snapshot
}

/// Describe nodes by tag and class to make structure drift obvious.
fn node_descriptor(tag: &str, class: Option<&str>) -> String {
    class.map_or_else(|| tag.to_string(), |class| format!("{tag}.{class}"))
}
