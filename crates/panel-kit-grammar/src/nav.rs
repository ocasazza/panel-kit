//! Vi-style graph navigation over a [`Site`] index.
//!
//! [`SiteIndex`] builds adjacency per morphism kind from a site, implements
//! [`GraphView`], and pairs with [`sort_map`] to map a [`Topos`] morphism
//! sorting into a [`SortMap`] so one keymap serves every regime.

use std::collections::{HashMap, HashSet};

use panel_kit_core::graph_nav::{GraphView, Side, SortMap};

use crate::site::{Object, Site};
use crate::topos::Topos;

/// Morphism kind assigned to untyped morphisms (kind [`None`]).
pub const UNTYPED_KIND: &str = "untyped";

/// Pre-built adjacency index for a site, keyed by morphism kind.
pub struct SiteIndex {
    objects: Vec<Object>,
    id_to_idx: HashMap<String, u32>,
    /// Forward adjacency: fwd[i] = codomains of morphisms whose domain == objects[i].id.
    fwd: Vec<Vec<u32>>,
    /// Forward adjacency per kind.
    fwd_by_kind: HashMap<String, Vec<Vec<u32>>>,
    /// Reverse adjacency: rev[i] = domains of morphisms whose codomain == objects[i].id.
    rev: Vec<Vec<u32>>,
    /// Reverse adjacency per kind.
    rev_by_kind: HashMap<String, Vec<Vec<u32>>>,
}

impl SiteIndex {
    /// Build the index from a site.
    ///
    /// # Panics
    /// If a morphism references an object id not present in the site.
    pub fn new(site: &Site) -> Self {
        let objects = site.objects.clone();
        let n = objects.len();
        let id_to_idx: HashMap<String, u32> = objects
            .iter()
            .enumerate()
            .map(|(i, o)| (o.id.clone(), i as u32))
            .collect();
        let mut fwd = vec![Vec::new(); n];
        let mut rev = vec![Vec::new(); n];
        let mut fwd_by_kind: HashMap<String, Vec<Vec<u32>>> = HashMap::new();
        let mut rev_by_kind: HashMap<String, Vec<Vec<u32>>> = HashMap::new();

        for m in &site.morphisms {
            let kind = m.kind.as_deref().unwrap_or(UNTYPED_KIND).to_owned();
            let src = id_to_idx[&m.domain];
            let tgt = id_to_idx[&m.codomain];
            fwd[src as usize].push(tgt);
            rev[tgt as usize].push(src);
            fwd_by_kind
                .entry(kind.clone())
                .or_insert_with(|| vec![Vec::new(); n])[src as usize]
                .push(tgt);
            rev_by_kind
                .entry(kind)
                .or_insert_with(|| vec![Vec::new(); n])[tgt as usize]
                .push(src);
        }

        Self {
            objects,
            id_to_idx,
            fwd,
            fwd_by_kind,
            rev,
            rev_by_kind,
        }
    }
}

impl GraphView for SiteIndex {
    type Id = String;

    fn ids(&self) -> Vec<String> {
        self.objects.iter().map(|o| o.id.clone()).collect()
    }

    fn neighbors(&self, id: &String, kinds: Option<&[String]>, side: Side) -> Vec<String> {
        let idx = match self.id_to_idx.get(id) {
            Some(&i) => i,
            None => return Vec::new(),
        };
        let indices: Vec<u32> = match kinds {
            None => match side {
                Side::Codomain => self.fwd[idx as usize].clone(),
                Side::Domain => self.rev[idx as usize].clone(),
            },
            Some(ks) => {
                let mut result = Vec::new();
                let store = match side {
                    Side::Codomain => &self.fwd_by_kind,
                    Side::Domain => &self.rev_by_kind,
                };
                for k in ks {
                    if let Some(vecs) = store.get(k) {
                        result.extend_from_slice(&vecs[idx as usize]);
                    }
                }
                result
            }
        };
        indices
            .into_iter()
            .map(|i| self.objects[i as usize].id.clone())
            .collect()
    }

    fn search(&self, query: &str) -> Vec<String> {
        let q = query.to_lowercase();
        if q.is_empty() {
            return Vec::new();
        }
        let mut seen = HashSet::new();
        let mut result = Vec::new();

        // Title matches first.
        for (i, obj) in self.objects.iter().enumerate() {
            if obj.title.to_lowercase().contains(&q) {
                seen.insert(i as u32);
                result.push(obj.id.clone());
            }
        }
        // Id matches second.
        for (i, obj) in self.objects.iter().enumerate() {
            if !seen.contains(&(i as u32)) && obj.id.to_lowercase().contains(&q) {
                seen.insert(i as u32);
                result.push(obj.id.clone());
            }
        }
        // Tag matches last.
        for (i, obj) in self.objects.iter().enumerate() {
            if !seen.contains(&(i as u32))
                && obj
                    .tags
                    .iter()
                    .any(|t| t.to_lowercase().contains(&q))
            {
                seen.insert(i as u32);
                result.push(obj.id.clone());
            }
        }
        result
    }
}

/// Build a [`SortMap`] from a topos morphism sorting.
pub fn sort_map(topos: &Topos) -> SortMap {
    SortMap::from_sorting(
        topos
            .morphism_sorting
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str())),
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use panel_kit_core::graph_nav::{GraphCursor, NavEffect, NavVerb};

    use super::*;
    use crate::site::Morphism;

    /// Five objects with distinct ids, titles, and tags.
    fn test_site() -> Site {
        let objects = vec![
            Object {
                id: "a".into(),
                title: "First".into(),
                kind: "X".into(),
                tags: vec!["alpha".into()],
                fields: BTreeMap::new(),
            },
            Object {
                id: "b".into(),
                title: "Second".into(),
                kind: "X".into(),
                tags: vec!["beta".into()],
                fields: BTreeMap::new(),
            },
            Object {
                id: "c".into(),
                title: "Third".into(),
                kind: "Y".into(),
                tags: vec!["gamma".into()],
                fields: BTreeMap::new(),
            },
            Object {
                id: "d".into(),
                title: "Fourth".into(),
                kind: "Y".into(),
                tags: vec!["delta".into()],
                fields: BTreeMap::new(),
            },
            Object {
                id: "e".into(),
                title: "Fifth".into(),
                kind: "X".into(),
                tags: vec![],
                fields: BTreeMap::new(),
            },
        ];
        let morphisms = vec![
            Morphism {
                domain: "a".into(),
                codomain: "b".into(),
                kind: Some("fwd".into()),
            },
            Morphism {
                domain: "b".into(),
                codomain: "c".into(),
                kind: Some("fwd".into()),
            },
            Morphism {
                domain: "c".into(),
                codomain: "d".into(),
                kind: Some("rev".into()),
            },
            Morphism {
                domain: "d".into(),
                codomain: "e".into(),
                kind: None,
            },
            Morphism {
                domain: "b".into(),
                codomain: "a".into(),
                kind: Some("fwd".into()),
            },
            Morphism {
                domain: "a".into(),
                codomain: "e".into(),
                kind: Some("rev".into()),
            },
        ];
        Site {
            objects,
            morphisms,
        }
    }

    fn index() -> SiteIndex {
        SiteIndex::new(&test_site())
    }

    #[test]
    fn neighbors_codomain_all_kinds() {
        let idx = index();
        let result = idx.neighbors(&"a".into(), None, Side::Codomain);
        assert_eq!(result, vec!["b", "e"]); // morphisms in source order: a→b, a→e
    }

    #[test]
    fn neighbors_domain_all_kinds() {
        let idx = index();
        let result = idx.neighbors(&"e".into(), None, Side::Domain);
        assert_eq!(result, vec!["d", "a"]); // d→e (untyped, #4) then a→e (rev, #6)
    }

    #[test]
    fn neighbors_filtered_by_kind() {
        let idx = index();
        let result = idx.neighbors(
            &"b".into(),
            Some(&["fwd".into()]),
            Side::Codomain,
        );
        // b→c (fwd), b→a (fwd); source order preserved
        assert_eq!(result, vec!["c", "a"]);
    }

    #[test]
    fn neighbors_untyped_kind() {
        let idx = index();
        let result = idx.neighbors(
            &"e".into(),
            Some(&["untyped".into()]),
            Side::Domain,
        );
        assert_eq!(result, vec!["d"]);
    }

    #[test]
    fn neighbors_unknown_id_returns_empty() {
        let idx = index();
        let result = idx.neighbors(&"z".into(), None, Side::Codomain);
        assert!(result.is_empty());
    }

    #[test]
    fn neighbors_empty_kinds_returns_empty() {
        let idx = index();
        let result = idx.neighbors(&"a".into(), Some(&[]), Side::Codomain);
        assert!(result.is_empty());
    }

    #[test]
    fn search_title_match() {
        let idx = index();
        let result = idx.search("First");
        assert_eq!(result, vec!["a"]);
    }

    #[test]
    fn search_title_case_insensitive() {
        let idx = index();
        let result = idx.search("first");
        assert_eq!(result, vec!["a"]);
    }

    #[test]
    fn search_id_match() {
        let idx = index();
        let result = idx.search("c");
        assert_eq!(result, vec!["b", "c"]); // title "Second" (b) outranks id "c"
    }

    #[test]
    fn search_tag_match() {
        let idx = index();
        let result = idx.search("gamma");
        // "gamma" is only in c's tags; no title contains it
        assert_eq!(result, vec!["c"]);
    }

    #[test]
    fn search_ranking_title_then_id_then_tag() {
        let idx = index();
        // "f" matches title "First" (a), title "Fourth" (d), title "Fifth" (e),
        // and also id "f" doesn't match any (ids are a,b,c,d,e)
        let result = idx.search("f");
        // Title matches: a, d, e in object order. No id matches. No tag matches.
        assert_eq!(result, vec!["a", "d", "e"]);
    }

    #[test]
    fn search_ranks_title_then_id_then_tag() {
        // "e": title "Second" (b); id "e"; tag "delta" (d); "beta" on b is already ranked.
        assert_eq!(index().search("e"), vec!["b", "e", "d"]);
    }

    #[test]
    fn search_empty_query_returns_empty() {
        assert!(index().search("").is_empty());
    }

    fn minimal_topos() -> Topos {
        let json = format!(
            r##"{{"spec_version":1,"id":"nav","regime":"Nav","object_sorting":{{}},"morphism_sorting":{{"fwd":"forward","rev":"reverse"}},"stalks":{{"objects":{{}},"morphisms":{{}},"stages":{{"chain":"","branched":"","ring":"","sheet":"","tube":"","membrane":""}}}},"sheaves":{{}},"workspace":{ws}}}"##,
            ws = include_str!("../testdata/inline-workspace.json"),
        );
        Topos::from_json_str(&json).unwrap()
    }

    #[test]
    fn sort_map_groups_kinds_by_sort() {
        let sm = sort_map(&minimal_topos());
        assert_eq!(sm.kinds("any"), None);
        assert_eq!(sm.kinds("forward"), Some(&["fwd".to_string()][..]));
        assert_eq!(sm.kinds("reverse"), Some(&["rev".to_string()][..]));
    }

    #[test]
    fn cursor_steps_domain_through_the_topos_sort_map() {
        let idx = index();
        let sm = sort_map(&minimal_topos());
        let mut cursor = GraphCursor::default();
        cursor.select("a".to_string());
        // b→a is the only forward edge into a.
        let verb = NavVerb::Step { sort: "forward".into(), side: Side::Domain };
        assert_eq!(cursor.apply(&idx, &sm, &verb, None), NavEffect::Moved);
        assert_eq!(cursor.current.as_deref(), Some("b"));
    }

    #[test]
    fn untyped_morphisms_are_reachable_through_any() {
        let idx = index();
        let sm = sort_map(&minimal_topos());
        let mut cursor = GraphCursor::default();
        cursor.select("e".to_string());
        // Domains of e: d (untyped, morphism 4) then a (rev, morphism 6).
        let verb = NavVerb::Step { sort: "any".into(), side: Side::Domain };
        assert_eq!(cursor.apply(&idx, &sm, &verb, None), NavEffect::Moved);
        assert_eq!(cursor.current.as_deref(), Some("d"));
    }
}