use std::collections::BTreeMap;

pub(super) fn extract_generated_region<'a>(document: &'a str, begin: &str, end: &str) -> &'a str {
    let start = document
        .find(begin)
        .unwrap_or_else(|| panic!("missing generated-region begin marker {begin:?}"));
    let after_begin = &document[start + begin.len()..];
    let end_offset = after_begin
        .find(end)
        .unwrap_or_else(|| panic!("missing generated-region end marker {end:?}"));
    after_begin[..end_offset].trim_matches('\n')
}

pub(super) fn parse_css_custom_properties(block: &str) -> BTreeMap<String, String> {
    block
        .lines()
        .filter_map(|line| line.trim().strip_prefix("--"))
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            let value = value.split(';').next()?.trim().trim_matches('"');
            Some((name.to_string(), value.to_string()))
        })
        .collect()
}

pub(super) fn parse_design_token_region(region: &str) -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();
    let mut section = "";
    let mut child = "";

    for line in region.lines() {
        let indent = line.chars().take_while(|c| *c == ' ').count();
        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }

        if indent == 0 {
            section = trimmed.trim_end_matches(':');
            child = "";
            continue;
        }

        if indent == 2 && trimmed.ends_with(':') {
            child = trimmed.trim_end_matches(':');
            continue;
        }

        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        let path = if child.is_empty() {
            format!("{section}.{key}")
        } else {
            format!("{section}.{child}.{key}")
        };
        values.insert(path, clean_design_value(value));
    }

    values
}

pub(super) fn px(value: f64) -> String {
    format!("{}px", number(value))
}

pub(super) fn rem(value: f64) -> String {
    format!("{}rem", number(value))
}

pub(super) fn em(value: f64) -> String {
    format!("{}em", number(value))
}

pub(super) fn number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        trimmed_fraction(value)
    }
}

fn clean_design_value(value: &str) -> String {
    value.trim().trim_matches('"').replace("\\\"", "\"")
}

fn trimmed_fraction(value: f64) -> String {
    let mut text = format!("{value:.4}");
    while text.ends_with('0') {
        text.pop();
    }
    text
}
