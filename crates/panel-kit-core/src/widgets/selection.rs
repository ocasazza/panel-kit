//! One-of-N selection models: the selectable card and clickable neighbour
//! pills.
//!
//! Ported from jump-cannon's `selection_card.rs` (the shared card/chip
//! primitives for engine and importer pickers) and the inspector's neighbour
//! pill row (`panels/inspector.rs`). Models only — renderers paint them and
//! report interaction through their own channels.

use crate::badge::Rgb;

/// Data for one selectable card in a one-of-N picker: head (name, subtitle,
/// chip strip), optional description, optional disabled-reason, and the
/// state flags a renderer paints borders from.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SelectionCard {
    /// Stable identifier regression selectors and `data-source-id` key on.
    pub source_id: String,
    /// Optional kind tag (e.g. "physics" / "static" / "github"), surfaced in
    /// `data-kind` and used for per-kind chip colouring.
    pub kind: String,
    /// CSS class chain, already including any state class (`active` /
    /// `default` / `ghost` / `viewing`) — the host builds the string.
    pub class: String,
    /// Name shown in the card head.
    pub name: String,
    /// Id rendered under the name; empty skips the row.
    pub subtitle: String,
    /// Small pill chips in the head, as `(class suffix, label)` pairs — the
    /// suffix is appended to the chip class so hosts can define per-kind
    /// colour rules in CSS without touching Rust.
    pub chips: Vec<(String, String)>,
    /// Description paragraph; empty skips it.
    pub description: String,
    /// Disabled-reason text (e.g. "node host not configured"); empty skips it.
    pub disabled_reason: String,
    /// `true` for the default / currently-active card: a primary-colour
    /// border + outline. Independent of the action button state.
    pub is_default: bool,
    /// `true` for the currently-viewed card: pairs with a yellow-tone border
    /// and `aria-pressed`.
    pub is_viewing: bool,
    /// `true` for the kept-but-unadvertised remote selection: still
    /// selectable, dashed border.
    pub is_ghost: bool,
    /// Disables the card and its action button.
    pub disabled: bool,
    /// Native tooltip and disabled-card body text.
    pub title: String,
}

/// Link direction of a neighbour relative to the focused object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PillDirection {
    /// Only incoming links.
    In,
    /// Only outgoing links.
    Out,
    /// Links in both directions.
    Both,
}

impl PillDirection {
    /// One-glyph marker prefixed to the pill label.
    pub fn glyph(self) -> &'static str {
        match self {
            PillDirection::In => "←",
            PillDirection::Out => "→",
            PillDirection::Both => "↔",
        }
    }

    /// Hover tip describing the direction.
    pub fn tip(self) -> &'static str {
        match self {
            PillDirection::In => "incoming link",
            PillDirection::Out => "outgoing link",
            PillDirection::Both => "links both ways",
        }
    }
}

/// One clickable pill's precomputed data (owned, so renderers don't borrow
/// the graph while painting).
#[derive(Clone, Debug, PartialEq)]
pub struct Pill {
    /// The full id body clicks select.
    pub id: String,
    /// Single-line label (direction glyph + truncated id).
    pub label: String,
    /// Optional colour override (e.g. a community swatch).
    pub tint: Option<Rgb>,
    /// Hover tip: the full id plus the direction, when known.
    pub tip: String,
}

/// Truncate an id to a single-line pill label — basename of path-like ids,
/// 24-char ellipsis.
pub fn short_id_for_pill(id: &str) -> String {
    let basename = id.rsplit('/').next().unwrap_or(id);
    const MAX: usize = 24;
    if basename.chars().count() <= MAX {
        basename.to_string()
    } else {
        let head: String = basename.chars().take(MAX - 1).collect();
        format!("{head}…")
    }
}

/// Walk the packed `[src, tgt]` edge list and collect `idx`'s unique
/// neighbours with direction, sorted ascending by index — port of the egui
/// `neighbor_set` (which sorted the same way, sans direction).
pub fn neighbor_set(idx: u32, edges: &[u32]) -> Vec<(u32, PillDirection)> {
    let mut map = std::collections::BTreeMap::new(); // index -> (out, in)
    for chunk in edges.chunks_exact(2) {
        let (s, t) = (chunk[0], chunk[1]);
        if s == idx && t != idx {
            map.entry(t).or_insert((false, false)).0 = true;
        } else if t == idx && s != idx {
            map.entry(s).or_insert((false, false)).1 = true;
        }
    }
    map.into_iter()
        .map(|(i, (out, inn))| {
            let dir = match (out, inn) {
                (true, true) => PillDirection::Both,
                (true, false) => PillDirection::Out,
                _ => PillDirection::In,
            };
            (i, dir)
        })
        .collect()
}

/// Fold `(index, direction)` pairs plus the id table into capped pills and
/// the count hidden past the cap. Same per-frame widget cap as the egui
/// clickable list: 200 pills. `tint_of` supplies the per-index colour
/// override so any palette policy can feed the pills.
pub fn build_pills(
    items: &[(u32, Option<PillDirection>)],
    ids: &[String],
    tint_of: impl Fn(u32) -> Option<Rgb>,
) -> (Vec<Pill>, usize) {
    const MAX: usize = 200;
    let truncated = items.len().saturating_sub(MAX);
    let pills = items
        .iter()
        .take(MAX)
        .map(|&(i, dir)| {
            let id = ids.get(i as usize).cloned().unwrap_or_else(|| "?".into());
            let short = short_id_for_pill(&id);
            let label = match dir {
                Some(d) => format!("{} {}", d.glyph(), short),
                None => short,
            };
            let tip = match dir {
                Some(d) => format!("{} — {}", id, d.tip()),
                None => id.clone(),
            };
            Pill { id, label, tint: tint_of(i), tip }
        })
        .collect();
    (pills, truncated)
}

#[cfg(test)]
mod tests {
    use super::{build_pills, neighbor_set, short_id_for_pill, PillDirection};

    #[test]
    fn pill_labels_use_basename_and_ellipsis() {
        assert_eq!(short_id_for_pill("journal/2026/a-note.md"), "a-note.md");
        assert_eq!(short_id_for_pill("plain"), "plain");
        let long = format!("{}x", "n".repeat(30));
        let short = short_id_for_pill(&long);
        assert_eq!(short.chars().count(), 24);
        assert!(short.ends_with('…'));
    }

    #[test]
    fn neighbour_walk_folds_directions_and_sorts() {
        let edges = [1u32, 0, 0, 2, 2, 0, 0, 3, 3, 0];
        assert_eq!(
            neighbor_set(0, &edges),
            vec![
                (1, PillDirection::In),
                (2, PillDirection::Both),
                (3, PillDirection::Both),
            ]
        );
        // Self-loops are ignored.
        assert!(neighbor_set(0, &[0, 0]).is_empty());
    }

    #[test]
    fn pill_lists_cap_and_report_the_overflow() {
        let items: Vec<(u32, Option<PillDirection>)> =
            (0..250u32).map(|i| (i, Some(PillDirection::In))).collect();
        let ids: Vec<String> = (0..250u32).map(|i| format!("node-{i}")).collect();
        let (pills, truncated) = build_pills(&items, &ids, |_| None);
        assert_eq!(pills.len(), 200);
        assert_eq!(truncated, 50);
        assert_eq!(pills[0].label, "← node-0");
        assert_eq!(pills[0].tip, "node-0 — incoming link");
        // Unknown indices fall back to `?`.
        let (pills, _) = build_pills(&[(9u32, None)], &[], |_| None);
        assert_eq!(pills[0].label, "?");
        assert_eq!(pills[0].tip, "?");
    }
}
