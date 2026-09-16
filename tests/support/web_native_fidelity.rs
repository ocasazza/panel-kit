#[derive(Debug, PartialEq, Eq)]
struct SemanticNode {
    path: String,
    tag: String,
    class: Option<String>,
    role: Option<String>,
    tabindex: Option<String>,
    title: Option<String>,
    aria_label: Option<String>,
    style: Option<String>,
}

/// Assert that the composable web parts preserve the controller-era native panel semantics.
pub fn assert_controller_era_panel_semantics(html: &str) {
    let actual = semantic_nodes(html);
    let expected = controller_era_panel_semantics();

    assert_eq!(
        actual, expected,
        "SSR native panel semantics drifted.\nHTML:\n{html}"
    );
}

macro_rules! node {
    ($path:expr, $tag:expr, $class:expr, $role:expr, $tabindex:expr, $title:expr, $aria_label:expr, $style:expr) => {
        SemanticNode {
            path: $path.to_string(),
            tag: $tag.to_string(),
            class: optional($class),
            role: optional($role),
            tabindex: optional($tabindex),
            title: optional($title),
            aria_label: optional($aria_label),
            style: optional($style),
        }
    };
}

fn controller_era_panel_semantics() -> Vec<SemanticNode> {
    vec![
        node!("div.chrome-probe > section.panel panel-nodes focused", "section", Some("panel panel-nodes focused"), None, None, None, None, Some("position:absolute;left:16px;top:24px;width:320px;height:180px;z-index:7;")),
        node!("div.chrome-probe > section.panel panel-nodes focused > header.panel-head", "header", Some("panel-head"), Some("toolbar"), Some("0"), Some("Nodes"), None, None),
        node!("div.chrome-probe > section.panel panel-nodes focused > header.panel-head > div.lights", "div", Some("lights"), None, None, None, None, None),
        node!("div.chrome-probe > section.panel panel-nodes focused > header.panel-head > div.lights > button.light mode", "button", Some("light mode"), None, None, Some("tiling / floating"), Some("tiling / floating"), None),
        node!("div.chrome-probe > section.panel panel-nodes focused > header.panel-head > div.lights > button.light yellow", "button", Some("light yellow"), None, None, Some("minimize"), Some("minimize"), None),
        node!("div.chrome-probe > section.panel panel-nodes focused > header.panel-head > div.lights > button.light max", "button", Some("light max"), None, None, Some("maximize / restore"), Some("maximize / restore"), None),
        node!("div.chrome-probe > section.panel panel-nodes focused > header.panel-head > span.panel-title", "span", Some("panel-title"), None, None, Some("Nodes"), None, None),
        node!("div.chrome-probe > section.panel panel-nodes focused > header.panel-head > div.panel-head-actions", "div", Some("panel-head-actions"), None, None, None, None, None),
        node!("div.chrome-probe > section.panel panel-nodes focused > div.panel-body", "div", Some("panel-body"), None, None, None, None, Some("overflow:auto;")),
        node!("div.chrome-probe > section.panel panel-nodes focused > button.resize", "button", Some("resize"), None, Some("0"), None, Some("Resize panel"), None),
    ]
}

fn semantic_nodes(html: &str) -> Vec<SemanticNode> {
    let mut nodes = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut rest = html;

    while let Some(open) = rest.find('<') {
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

        let (tag, attrs) = split_tag(tag_source);
        let class = attr(attrs, "class");
        let descriptor = node_descriptor(tag, class.as_deref());
        stack.push(descriptor);

        let node = SemanticNode {
            path: stack.join(" > "),
            tag: tag.to_string(),
            class,
            role: attr(attrs, "role"),
            tabindex: attr(attrs, "tabindex"),
            title: attr(attrs, "title"),
            aria_label: attr(attrs, "aria-label"),
            style: attr(attrs, "style"),
        };
        if is_native_panel_node(&node) {
            nodes.push(node);
        }
    }

    nodes
}

fn split_tag(source: &str) -> (&str, &str) {
    source
        .split_once(char::is_whitespace)
        .map_or((source.trim_end_matches('/'), ""), |(tag, attrs)| {
            (tag, attrs)
        })
}

fn attr(attrs: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=\"");
    let start = attrs.find(&needle)? + needle.len();
    let value = attrs[start..].split_once('"')?.0;
    Some(value.to_string())
}

fn node_descriptor(tag: &str, class: Option<&str>) -> String {
    class.map_or_else(|| tag.to_string(), |class| format!("{tag}.{class}"))
}

fn is_native_panel_node(node: &SemanticNode) -> bool {
    matches!(
        node.class.as_deref(),
        Some(
            "panel panel-nodes focused"
                | "panel-body"
                | "panel-head"
                | "panel-title"
                | "panel-head-actions"
                | "lights"
                | "light mode"
                | "light yellow"
                | "light max"
                | "resize"
        )
    )
}

fn optional(value: Option<&str>) -> Option<String> {
    value.map(ToString::to_string)
}
