//! Contextual hints: what is under the pointer or focused, and which hotkeys
//! act on it.
//!
//! Ported from jump-cannon's `hints.rs`. The registry is last-wins: each
//! publisher owns one slot keyed by its `source`, and the most recently
//! published entry is the one a status bar shows. The registry itself is
//! renderer-neutral state; renderers wrap it (the Dioxus backend keeps it in
//! a global signal and paints a hint bar over it).

/// One contextual hint. `hotkeys` are `(action label, key chord)` pairs
/// rendered as key chips.
#[derive(Clone, Debug, PartialEq)]
pub struct Hint {
    /// The publishing slot; republishing from the same source replaces it.
    pub source: &'static str,
    /// Primary text, e.g. the item under the pointer.
    pub title: String,
    /// Optional single-line body, e.g. a metadata summary.
    pub body: Option<String>,
    /// `(action label, key chord)` chips.
    pub hotkeys: Vec<(&'static str, &'static str)>,
}

/// Hotkey chip shared by every node hint.
pub const VIEW_NODE: (&str, &str) = ("View Node", "Super+V");

/// Body excerpt budget (chars) for the single-line node summary.
const EXCERPT_CHARS: usize = 96;
/// Tags shown before the summary falls back to a "+N" count.
const MAX_TAGS: usize = 3;

/// Plain node metadata the hint summary is built from — the fields of a
/// document/node record with no domain types attached.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NodeHintMeta {
    /// Display title; falls back to the path file stem, then the id.
    pub title: String,
    /// Source path; the file stem is the first title fallback.
    pub path: String,
    /// Document type, when the record carries one.
    pub doctype: Option<String>,
    /// Folder the record lives in.
    pub folder: String,
    /// Tags; the summary shows at most [`MAX_TAGS`] plus a `+N` count.
    pub tags: Vec<String>,
    /// Body text; the summary excerpt is its first non-blank line.
    pub body: String,
}

/// Last-wins per source: each publisher owns one slot, the newest entry shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HintRegistry {
    entries: Vec<Hint>,
}

impl HintRegistry {
    /// Publish `hint`, replacing any earlier entry from the same source and
    /// making it the shown one. No-op write when the shown entry is unchanged.
    pub fn publish(&mut self, hint: Hint) {
        if self.entries.last() == Some(&hint) {
            return;
        }
        self.entries.retain(|h| h.source != hint.source);
        self.entries.push(hint);
    }

    /// Drop `source`'s entry; the previously published hint (if any) shows.
    pub fn clear(&mut self, source: &'static str) {
        self.entries.retain(|h| h.source != source);
    }

    /// The currently shown hint, if any.
    pub fn shown(&self) -> Option<&Hint> {
        self.entries.last()
    }

    /// How many slots are currently held.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no slot is currently held.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Node hint from plain metadata when available, else the bare id (browser
/// graphs have no metadata endpoint).
pub fn node_hint(source: &'static str, id: &str, meta: Option<&NodeHintMeta>) -> Hint {
    let Some(m) = meta else {
        return Hint {
            source,
            title: id.to_string(),
            body: None,
            hotkeys: vec![VIEW_NODE],
        };
    };
    let mut parts: Vec<String> = Vec::new();
    if let Some(d) = m.doctype.as_deref().filter(|d| !d.is_empty()) {
        parts.push(d.to_string());
    }
    if !m.folder.is_empty() {
        parts.push(m.folder.clone());
    }
    if !m.tags.is_empty() {
        let mut tags = m
            .tags
            .iter()
            .take(MAX_TAGS)
            .map(|t| format!("#{t}"))
            .collect::<Vec<_>>()
            .join(" ");
        if m.tags.len() > MAX_TAGS {
            tags.push_str(&format!(" +{}", m.tags.len() - MAX_TAGS));
        }
        parts.push(tags);
    }
    if let Some(line) = first_line(&m.body) {
        parts.push(line);
    }
    Hint {
        source,
        title: node_title(m),
        body: (!parts.is_empty()).then(|| parts.join(" · ")),
        hotkeys: vec![VIEW_NODE],
    }
}

/// Title fallback chain: title → path file stem → id.
pub fn node_title(meta: &NodeHintMeta) -> String {
    if !meta.title.is_empty() {
        return meta.title.clone();
    }
    if !meta.path.is_empty() {
        if let Some(stem) =
            std::path::Path::new(&meta.path).file_stem().and_then(|s| s.to_str())
        {
            return stem.to_string();
        }
    }
    String::new()
}

/// First non-blank body line, cut to `EXCERPT_CHARS` with an ellipsis.
fn first_line(body: &str) -> Option<String> {
    let line = body.lines().map(str::trim).find(|l| !l.is_empty())?;
    let mut out: String = line.chars().take(EXCERPT_CHARS).collect();
    if line.chars().count() > EXCERPT_CHARS {
        out.push('…');
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::{node_hint, node_title, Hint, HintRegistry, NodeHintMeta, VIEW_NODE};

    fn meta() -> NodeHintMeta {
        NodeHintMeta {
            title: "A note".into(),
            path: "journal/2026/a-note.md".into(),
            doctype: Some("daily".into()),
            folder: "journal".into(),
            tags: vec!["work".into(), "deep".into(), "focus".into(), "extra".into()],
            body: "first line\n\nsecond line".into(),
        }
    }

    #[test]
    fn registry_is_last_wins_per_source() {
        let mut r = HintRegistry::default();
        r.publish(Hint { source: "a", title: "1".into(), body: None, hotkeys: vec![] });
        r.publish(Hint { source: "b", title: "2".into(), body: None, hotkeys: vec![] });
        assert_eq!(r.shown().unwrap().title, "2");
        // Republishing from `a` replaces its slot and becomes the shown hint.
        r.publish(Hint { source: "a", title: "3".into(), body: None, hotkeys: vec![] });
        assert_eq!(r.shown().unwrap().title, "3");
        assert_eq!(r.len(), 2);
        // Clearing the shown source falls back to the previous slot.
        r.clear("a");
        assert_eq!(r.shown().unwrap().title, "2");
        r.clear("b");
        assert!(r.shown().is_none());
    }

    #[test]
    fn summary_folds_doctype_folder_tags_and_excerpt() {
        let hint = node_hint("test", "id-1", Some(&meta()));
        assert_eq!(hint.title, "A note");
        let body = hint.body.unwrap();
        assert!(body.starts_with("daily · journal · #work #deep #focus +1 · first line"), "{body}");
        assert!(hint.hotkeys.contains(&VIEW_NODE));
    }

    #[test]
    fn title_falls_back_to_path_stem_then_empty() {
        let mut m = meta();
        m.title.clear();
        assert_eq!(node_title(&m), "a-note");
        m.path.clear();
        assert_eq!(node_title(&m), "");
    }

    #[test]
    fn bare_id_hint_when_no_meta() {
        let hint = node_hint("test", "raw-id", None);
        assert_eq!(hint.title, "raw-id");
        assert!(hint.body.is_none());
    }

    #[test]
    fn excerpt_truncates_with_ellipsis() {
        let mut m = meta();
        m.title.clear();
        m.path.clear();
        m.doctype = None;
        m.folder.clear();
        m.tags.clear();
        m.body = "x".repeat(200);
        let hint = node_hint("t", "id", Some(&m));
        let body = hint.body.unwrap();
        assert_eq!(body.chars().count(), 97);
        assert!(body.ends_with('…'));
    }
}
