//! Pixel metrics of the graph: every constant the layout reads, and the label
//! arithmetic built on them. All lengths are CSS px at the measured width;
//! text is mono and never scaled.

use std::borrow::Cow;

use super::super::model::Lane;

/// Row pitch: an 11px label and 3px clear of the next row.
pub const ROW: f64 = 14.0;
/// Consecutive dots closer than this merge into one cluster glyph.
pub const DOT_MERGE_PX: f64 = 6.0;
/// Mono advance of every label character.
pub const CHAR_PX: f64 = 6.6;
/// Clear space after a label before another item may share its row.
pub const LABEL_GAP: f64 = 5.0;
/// Left margin before the longest-label reserve.
pub const PAD_LEFT: f64 = 6.0;
/// Right margin before the widest-tip reserve.
pub const PAD_RIGHT: f64 = 6.0;
/// Width of one density-strip cell.
pub const STRIP_PX: f64 = 4.0;
/// Height of the axis label band under the last row.
pub const AXIS: f64 = 28.0;
/// Radius of a single dot.
pub const DOT_R: f64 = 3.0;
/// Radius of a cluster glyph.
pub const CLUSTER_R: f64 = 5.0;
/// Half of the 1.5px dot stroke.
pub const HALF_STROKE: f64 = 0.75;
/// Clear space between text and a dot's outer edge.
pub const TEXT_CLEAR: f64 = 2.0;
/// An item's id is drawn at its start too once its line is this long.
pub const START_LABEL_PX: f64 = 120.0;
/// Half of the 7px fork tip dot plus the 4px gap before its caption.
pub const FORK_NODE_PX: f64 = 7.5;
/// Fork captions longer than this many characters are cut.
pub const FORK_LABEL_MAX: usize = 16;

/// Width of `chars` label characters.
pub fn label_px_chars(chars: usize) -> f64 {
    chars as f64 * CHAR_PX
}

/// Width of `label` drawn in mono.
pub fn label_px(label: &str) -> f64 {
    label_px_chars(label.chars().count())
}

/// A fork caption as drawn: more than 16 characters become the first 15 and `…`.
pub fn truncate_label(label: &str) -> Cow<'_, str> {
    match label.char_indices().nth(FORK_LABEL_MAX) {
        None => Cow::Borrowed(label),
        Some(_) => {
            let mut cut: String = label.chars().take(FORK_LABEL_MAX - 1).collect();
            cut.push('…');
            Cow::Owned(cut)
        }
    }
}

/// Characters of [`truncate_label`]'s result, without building it.
pub fn truncated_chars(label: &str) -> usize {
    label
        .chars()
        .take(FORK_LABEL_MAX + 1)
        .count()
        .min(FORK_LABEL_MAX)
}

/// How far a label stands off a dot's centre: the outer edge of the glyph,
/// then clear space.
pub fn clearance(count: u32) -> f64 {
    if count >= 2 {
        CLUSTER_R + HALF_STROKE + TEXT_CLEAR
    } else {
        DOT_R + HALF_STROKE + TEXT_CLEAR
    }
}

/// Room at the right edge for a seven-character id, e.g. `imp-123`.
pub fn longest_label_reserve() -> f64 {
    7.0 * CHAR_PX + CLUSTER_R + HALF_STROKE + TEXT_CLEAR
}

/// Right margin: the id reserve, or the widest fork tip node when wider.
pub fn right_reserve(lanes: &[Lane]) -> f64 {
    lanes.iter().fold(longest_label_reserve(), |widest, lane| {
        widest.max(FORK_NODE_PX + label_px_chars(truncated_chars(&lane.label)) + LABEL_GAP)
    })
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::widgets::event_graph::model::LaneKind;

    fn lane(label: &str) -> Lane {
        Lane {
            id: format!("imp-1/collab/{label}"),
            item_id: "imp-1".into(),
            kind: LaneKind::Collab,
            label: label.into(),
            from: 0,
            to: 0,
            chosen: None,
            parent: None,
            agent: None,
        }
    }

    #[rstest]
    #[case("omp", "omp")]
    #[case("orchestrator", "orchestrator")]
    #[case("0123456789abcdef", "0123456789abcdef")]
    #[case("0123456789abcdefg", "0123456789abcde…")]
    #[case("imp-1/branch/feature/very-long-name", "imp-1/branch/fe…")]
    fn truncation_cuts_past_sixteen_characters(#[case] label: &str, #[case] want: &str) {
        assert_eq!(truncate_label(label), want);
    }

    #[test]
    fn truncation_counts_characters_not_bytes() {
        let long = "é".repeat(17);
        assert_eq!(truncate_label(&long), format!("{}…", "é".repeat(15)));
        assert_eq!(truncate_label(&"é".repeat(16)), "é".repeat(16));
    }

    #[rstest]
    #[case("")]
    #[case("a")]
    #[case("0123456789abcdef")]
    #[case("0123456789abcdefg")]
    #[case("a-very-long-collaborator-name")]
    fn truncated_chars_matches_the_cut_label(#[case] label: &str) {
        assert_eq!(
            truncated_chars(label),
            truncate_label(label).chars().count()
        );
    }

    #[test]
    fn longest_label_reserve_holds_a_seven_char_id() {
        assert_eq!(longest_label_reserve(), 7.0 * 6.6 + 5.0 + 0.75 + 2.0);
        assert!((longest_label_reserve() - 53.95).abs() < 1e-9);
    }

    #[test]
    fn right_reserve_grows_with_the_widest_fork_caption() {
        assert_eq!(right_reserve(&[]), longest_label_reserve());
        let lanes = [lane("orchestrator")];
        assert!((right_reserve(&lanes) - (7.5 + 12.0 * 6.6 + 5.0)).abs() < 1e-9);
        assert_eq!(right_reserve(&[lane("a")]), longest_label_reserve());
    }

    #[test]
    fn clearance_is_the_glyph_edge_plus_clear_space() {
        assert_eq!(clearance(1), 5.75);
        assert_eq!(clearance(2), 7.75);
        assert_eq!(clearance(40), 7.75);
    }

    #[test]
    fn label_px_counts_characters_not_bytes() {
        assert_eq!(label_px("imp-9"), 5.0 * 6.6);
        assert_eq!(label_px("ééé"), 3.0 * 6.6);
    }
}
