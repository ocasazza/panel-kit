use super::*;
use crate::keymap::{Keymap, KeymapSpec, KeymapState, Resolved, RuleSpec, KEYMAP_SPEC_VERSION};
use crate::{Key, KeyChord};

/// A small taxonomy: `genls` is the hierarchy (domain = child, codomain =
/// parent), `isa` is instance-of.
///
/// ```text
/// thing <-genls- animal <-genls- dog, cat
/// rex -isa-> dog
/// ```
struct Taxonomy {
    nodes: Vec<&'static str>,
    edges: Vec<(&'static str, &'static str, &'static str)>,
}

fn taxonomy() -> Taxonomy {
    Taxonomy {
        nodes: vec!["thing", "animal", "dog", "cat", "rex"],
        edges: vec![
            ("animal", "thing", "genls"),
            ("dog", "animal", "genls"),
            ("cat", "animal", "genls"),
            ("rex", "dog", "isa"),
        ],
    }
}

impl GraphView for Taxonomy {
    type Id = String;

    fn ids(&self) -> Vec<String> {
        self.nodes.iter().map(|s| (*s).to_string()).collect()
    }

    fn neighbors(&self, id: &String, kinds: Option<&[String]>, side: Side) -> Vec<String> {
        self.edges
            .iter()
            .filter(|(_, _, k)| kinds.map_or(true, |ks| ks.iter().any(|x| x == k)))
            .filter_map(|(d, c, _)| match side {
                Side::Codomain if d == id => Some((*c).to_string()),
                Side::Domain if c == id => Some((*d).to_string()),
                _ => None,
            })
            .collect()
    }

    fn search(&self, query: &str) -> Vec<String> {
        self.nodes
            .iter()
            .filter(|n| n.contains(query))
            .map(|s| (*s).to_string())
            .collect()
    }
}

fn sorts() -> SortMap {
    SortMap::from_sorting([("genls", "hierarchy"), ("isa", "instance")])
}

fn verb(s: &str) -> NavVerb {
    NavVerb::parse_intent(s).unwrap()
}

fn cursor_at(id: &str) -> GraphCursor<String> {
    let mut c = GraphCursor::default();
    c.select(id.to_string());
    c
}

#[test]
fn step_follows_sorted_edges_by_side() {
    let g = taxonomy();
    let mut c = cursor_at("dog");
    assert_eq!(c.apply(&g, &sorts(), &verb("nav.step hierarchy codomain"), None), NavEffect::Moved);
    assert_eq!(c.current.as_deref(), Some("animal"));
    assert_eq!(c.apply(&g, &sorts(), &verb("nav.step hierarchy domain"), Some(2)), NavEffect::Moved);
    assert_eq!(c.current.as_deref(), Some("cat"));
    // `isa` edges are not hierarchy edges.
    assert!(matches!(c.apply(&g, &sorts(), &verb("nav.step instance domain"), None), NavEffect::NoTarget(_)));
    let mut c = cursor_at("dog");
    assert_eq!(c.apply(&g, &sorts(), &verb("nav.step instance domain"), None), NavEffect::Moved);
    assert_eq!(c.current.as_deref(), Some("rex"));
    // `any` ignores sorting.
    let mut c = cursor_at("rex");
    assert_eq!(c.apply(&g, &sorts(), &verb("nav.step any codomain"), None), NavEffect::Moved);
    assert_eq!(c.current.as_deref(), Some("dog"));
}

#[test]
fn sibling_walks_the_shared_parent_children_and_wraps() {
    let g = taxonomy();
    let mut c = cursor_at("dog");
    let next = verb("nav.sibling hierarchy codomain +1");
    assert_eq!(c.apply(&g, &sorts(), &next, None), NavEffect::Moved);
    assert_eq!(c.current.as_deref(), Some("cat"));
    assert_eq!(c.apply(&g, &sorts(), &next, None), NavEffect::Moved);
    assert_eq!(c.current.as_deref(), Some("dog"));
    let mut root = cursor_at("thing");
    assert!(matches!(root.apply(&g, &sorts(), &next, None), NavEffect::NoTarget(_)));
}

#[test]
fn cycle_first_last_and_counts() {
    let g = taxonomy();
    let mut c = GraphCursor::default();
    assert_eq!(c.apply(&g, &sorts(), &verb("nav.cycle +1"), None), NavEffect::Moved);
    assert_eq!(c.current.as_deref(), Some("thing"));
    assert_eq!(c.apply(&g, &sorts(), &verb("nav.cycle -1"), None), NavEffect::Moved);
    assert_eq!(c.current.as_deref(), Some("rex"));
    assert_eq!(c.apply(&g, &sorts(), &verb("nav.first"), None), NavEffect::Moved);
    assert_eq!(c.current.as_deref(), Some("thing"));
    assert_eq!(c.apply(&g, &sorts(), &verb("nav.last"), None), NavEffect::Moved);
    assert_eq!(c.current.as_deref(), Some("rex"));
    assert_eq!(c.apply(&g, &sorts(), &verb("nav.last"), Some(3)), NavEffect::Moved);
    assert_eq!(c.current.as_deref(), Some("dog"));
}

#[test]
fn search_then_match_next_prev() {
    let g = taxonomy();
    let mut c = cursor_at("thing");
    assert_eq!(c.apply(&g, &sorts(), &verb("nav.search"), None), NavEffect::PromptSearch);
    assert_eq!(c.set_query(&g, "a"), NavEffect::Moved);
    assert_eq!(c.matches, vec!["animal", "cat"]);
    assert_eq!(c.current.as_deref(), Some("animal"));
    c.apply(&g, &sorts(), &verb("nav.match next"), None);
    assert_eq!(c.current.as_deref(), Some("cat"));
    c.apply(&g, &sorts(), &verb("nav.match next"), None);
    assert_eq!(c.current.as_deref(), Some("animal"));
    // Leaving the match set and pressing `n` returns to the first match.
    c.select("rex".into());
    c.apply(&g, &sorts(), &verb("nav.match prev"), None);
    assert_eq!(c.current.as_deref(), Some("animal"));
    assert!(matches!(c.set_query(&g, "zzz"), NavEffect::NoTarget(_)));
}

#[test]
fn marks_and_jumplist() {
    let g = taxonomy();
    let s = sorts();
    let mut c = cursor_at("thing");
    assert_eq!(c.apply(&g, &s, &verb("nav.mark a"), None), NavEffect::Unchanged);
    c.apply(&g, &s, &verb("nav.step hierarchy domain"), None); // animal
    c.apply(&g, &s, &verb("nav.step hierarchy domain"), None); // dog
    assert_eq!(c.apply(&g, &s, &verb("nav.jump a"), None), NavEffect::Moved);
    assert_eq!(c.current.as_deref(), Some("thing"));
    assert!(matches!(c.apply(&g, &s, &verb("nav.jump b"), None), NavEffect::NoTarget(_)));

    // Back walks thing <- dog <- animal <- thing; forward returns.
    assert_eq!(c.apply(&g, &s, &verb("nav.jumplist -1"), None), NavEffect::Moved);
    assert_eq!(c.current.as_deref(), Some("dog"));
    c.apply(&g, &s, &verb("nav.jumplist -1"), Some(2));
    assert_eq!(c.current.as_deref(), Some("thing"));
    assert!(matches!(c.apply(&g, &s, &verb("nav.jumplist -1"), None), NavEffect::NoTarget(_)));
    c.apply(&g, &s, &verb("nav.jumplist +1"), Some(3));
    assert_eq!(c.current.as_deref(), Some("thing"));
    assert!(matches!(c.apply(&g, &s, &verb("nav.jumplist +1"), None), NavEffect::NoTarget(_)));

    // A new jump from the middle of the list discards forward history.
    c.apply(&g, &s, &verb("nav.jumplist -1"), None); // dog
    c.apply(&g, &s, &verb("nav.step instance domain"), None); // rex
    assert_eq!(c.jumplist(), ["thing", "animal", "dog"]);
    assert!(matches!(c.apply(&g, &s, &verb("nav.jumplist +1"), None), NavEffect::NoTarget(_)));
}

#[test]
fn vocabulary_rejects_malformed_verbs() {
    for bad in [
        "nav.step hierarchy sideways",
        "nav.sibling hierarchy codomain 2",
        "nav.mark AB",
        "nav.teleport",
        "",
    ] {
        assert!(NavVerb::parse_intent(bad).is_err(), "{bad}");
    }
}

/// The whole chain: a data keymap, the nav vocabulary, a graph.
#[test]
fn keymap_drives_cursor_end_to_end() {
    let spec = KeymapSpec {
        spec_version: KEYMAP_SPEC_VERSION,
        id: "vi-graph".into(),
        modes: vec!["normal".into()],
        rules: [
            ("h", "nav.step hierarchy codomain"),
            ("l", "nav.step hierarchy domain"),
            ("j", "nav.sibling hierarchy codomain +1"),
            ("g g", "nav.first"),
            ("m {a-z}", "nav.mark $1"),
            ("' {a-z}", "nav.jump $1"),
            ("<c-o>", "nav.jumplist -1"),
        ]
        .iter()
        .map(|(k, i)| RuleSpec {
            keys: (*k).into(),
            intent: (*i).into(),
            modes: None,
            enter: None,
            label: None,
            group: None,
        })
        .collect(),
    };
    let map: Keymap<NavVerb> = Keymap::compile(&[spec]).unwrap();
    let g = taxonomy();
    let s = sorts();
    let mut state = KeymapState::default();
    let mut c = cursor_at("rex");

    let mut press = |keys: &str, c: &mut GraphCursor<String>| {
        for ch in keys.chars() {
            let chord = KeyChord {
                key: if ch == '^' { Key::Char('o') } else { Key::Char(ch) },
                shift: false,
                alt: false,
                ctrl: ch == '^',
                meta: false,
            };
            if let Resolved::Intent { intent, count } = map.feed(&mut state, chord) {
                c.apply(&g, &s, &intent, count);
            }
        }
    };

    press("gg", &mut c);
    assert_eq!(c.current.as_deref(), Some("thing"));
    press("ma", &mut c);
    press("l", &mut c);
    assert_eq!(c.current.as_deref(), Some("animal"));
    press("2l", &mut c); // second child of animal
    assert_eq!(c.current.as_deref(), Some("cat"));
    press("j", &mut c); // sibling wraps
    assert_eq!(c.current.as_deref(), Some("dog"));
    press("'a", &mut c);
    assert_eq!(c.current.as_deref(), Some("thing"));
    press("^", &mut c);
    assert_eq!(c.current.as_deref(), Some("dog"));
}
