//! Vi-style traversal over any directed, kind-typed graph.
//!
//! [`GraphView`] is the only thing a host supplies: ordered node ids and
//! neighbours by edge kind and side. [`NavVerb`] is the authored vocabulary
//! (an [`IntentVocabulary`] so keymaps bind it as data), and [`GraphCursor`]
//! reduces verbs into a current node, a jumplist, marks and search matches.
//!
//! Verbs name edge *sorts*, never concrete kinds: `step hierarchy domain`
//! reads "the hierarchy-sorted edges that point at me". A host maps its edge
//! kinds onto sorts once in [`SortMap`], so one keymap serves every graph.

use std::collections::BTreeMap;
use std::fmt;

use crate::keymap::IntentVocabulary;

/// Which end of a directed edge the cursor moves toward.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    /// Toward edge sources: nodes with an edge *into* the cursor.
    Domain,
    /// Toward edge targets: nodes the cursor has an edge *to*.
    Codomain,
}

/// Neighbour access a host provides for its graph.
pub trait GraphView {
    /// Node identity.
    type Id: Clone + Eq + Ord + fmt::Debug;

    /// Every node, in the order `cycle` walks.
    fn ids(&self) -> Vec<Self::Id>;

    /// Neighbours of `id` across edges whose kind is any of `kinds`
    /// (`None` means every kind), on `side`, in stable order.
    fn neighbors(&self, id: &Self::Id, kinds: Option<&[String]>, side: Side) -> Vec<Self::Id>;

    /// Nodes whose searchable text matches `query`; order is match rank.
    fn search(&self, query: &str) -> Vec<Self::Id>;
}

/// Edge sort to the host's concrete edge kinds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SortMap {
    kinds: BTreeMap<String, Vec<String>>,
}

impl SortMap {
    /// Build from (edge kind, sort) pairs, the shape of a topos
    /// `morphism_sorting` map.
    pub fn from_sorting<'a>(sorting: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        let mut kinds: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (kind, sort) in sorting {
            kinds.entry(sort.to_owned()).or_default().push(kind.to_owned());
        }
        Self { kinds }
    }

    /// Kinds behind `sort`; `any` is every kind.
    pub fn kinds(&self, sort: &str) -> Option<&[String]> {
        if sort == "any" {
            None
        } else {
            self.kinds.get(sort).map(Vec::as_slice)
        }
    }
}

/// One navigation intent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NavVerb {
    /// Move to the first neighbour across `sort` on `side`; a count picks the
    /// n-th.
    Step {
        /// Edge sort.
        sort: String,
        /// Direction.
        side: Side,
    },
    /// Move among the siblings that share the cursor's `side` neighbour
    /// across `sort`; `delta` is ±1 per count.
    Sibling {
        /// Edge sort.
        sort: String,
        /// Which shared neighbour defines siblinghood.
        side: Side,
        /// Direction along the sibling list.
        delta: i32,
    },
    /// Move `delta` nodes through `ids()` order, wrapping.
    Cycle {
        /// Direction.
        delta: i32,
    },
    /// Go to the first node (`gg`) or the last (`G`); a count goes to the
    /// n-th.
    First,
    /// See [`NavVerb::First`].
    Last,
    /// Set the search query from the host's prompt and go to its first match.
    Search,
    /// Move `delta` through the current matches, wrapping.
    Match {
        /// Direction.
        delta: i32,
    },
    /// Remember the cursor under `name`.
    Mark(char),
    /// Return to the mark `name`.
    Jump(char),
    /// Walk the jumplist: `-1` is back, `1` is forward.
    JumpList {
        /// Direction.
        delta: i32,
    },
}

impl IntentVocabulary for NavVerb {
    fn parse_intent(intent: &str) -> Result<Self, String> {
        let words: Vec<&str> = intent.split_whitespace().collect();
        let side = |w: &str| match w {
            "domain" => Ok(Side::Domain),
            "codomain" => Ok(Side::Codomain),
            other => Err(format!("unknown side `{other}`")),
        };
        let delta = |w: &str| match w {
            "+1" | "1" | "next" => Ok(1),
            "-1" | "prev" => Ok(-1),
            other => Err(format!("unknown delta `{other}`")),
        };
        let letter = |w: &str| {
            let mut cs = w.chars();
            match (cs.next(), cs.next()) {
                (Some(c), None) if c.is_ascii_lowercase() => Ok(c),
                _ => Err(format!("mark name must be one lowercase letter, got `{w}`")),
            }
        };
        match words.as_slice() {
            ["nav.step", sort, s] => Ok(Self::Step {
                sort: (*sort).to_owned(),
                side: side(s)?,
            }),
            ["nav.sibling", sort, s, d] => Ok(Self::Sibling {
                sort: (*sort).to_owned(),
                side: side(s)?,
                delta: delta(d)?,
            }),
            ["nav.cycle", d] => Ok(Self::Cycle { delta: delta(d)? }),
            ["nav.first"] => Ok(Self::First),
            ["nav.last"] => Ok(Self::Last),
            ["nav.search"] => Ok(Self::Search),
            ["nav.match", d] => Ok(Self::Match { delta: delta(d)? }),
            ["nav.mark", m] => Ok(Self::Mark(letter(m)?)),
            ["nav.jump", m] => Ok(Self::Jump(letter(m)?)),
            ["nav.jumplist", d] => Ok(Self::JumpList { delta: delta(d)? }),
            _ => Err(format!("unknown nav intent `{intent}`")),
        }
    }
}

/// What the host must do after a verb, beyond reading [`GraphCursor::current`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NavEffect {
    /// The cursor moved.
    Moved,
    /// The verb had no target; `reason` is what to echo.
    NoTarget(NoTargetReason),
    /// `nav.search` was fired; the host prompts, then calls
    /// [`GraphCursor::set_query`].
    PromptSearch,
    /// The cursor stayed put and no prompt is needed (a mark was set).
    Unchanged,
}

/// Why a verb did not move the cursor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoTargetReason {
    /// No cursor yet (nothing selected, or the graph is empty).
    NoCursor,
    /// The cursor has no neighbour across `sort` on `side`, or fewer than
    /// the count asked for.
    NoNeighbor {
        /// Edge sort.
        sort: String,
        /// Direction.
        side: Side,
    },
    /// The cursor has no `side` neighbour across `sort` to share siblings under.
    NoSiblings {
        /// Edge sort.
        sort: String,
        /// Direction.
        side: Side,
    },
    /// `ids()` is empty or shorter than the count.
    OutOfRange,
    /// Nothing matched the query.
    NoMatches,
    /// The mark was never set.
    MarkUnset(char),
    /// Jumplist end reached in that direction.
    JumplistEnd,
}

impl fmt::Display for NoTargetReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let side = |s: &Side| match s {
            Side::Domain => "incoming",
            Side::Codomain => "outgoing",
        };
        match self {
            Self::NoCursor => write!(f, "no cursor: select a node first"),
            Self::NoNeighbor { sort, side: s } if sort == "any" => {
                write!(f, "no {} edges", side(s))
            }
            Self::NoNeighbor { sort, side: s } => write!(f, "no {} {sort} edges", side(s)),
            Self::NoSiblings { sort, side: s } if sort == "any" => {
                write!(f, "no {} edge to share siblings under", side(s))
            }
            Self::NoSiblings { sort, side: s } => {
                write!(f, "no {} {sort} edge to share siblings under", side(s))
            }
            Self::OutOfRange => write!(f, "no node at that position"),
            Self::NoMatches => write!(f, "no matches"),
            Self::MarkUnset(m) => write!(f, "mark {m} not set"),
            Self::JumplistEnd => write!(f, "end of jumplist"),
        }
    }
}

/// Cursor state for one graph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphCursor<Id> {
    /// Current node, `None` when the graph is empty.
    pub current: Option<Id>,
    /// Current search query.
    pub query: String,
    /// Matches for `query`, in rank order.
    pub matches: Vec<Id>,
    marks: BTreeMap<char, Id>,
    jumps: Vec<Id>,
    /// Index into `jumps` of the current position (== len when at the tip).
    jump_at: usize,
}

impl<Id: Clone + Eq + Ord + fmt::Debug> Default for GraphCursor<Id> {
    fn default() -> Self {
        Self {
            current: None,
            query: String::new(),
            matches: Vec::new(),
            marks: BTreeMap::new(),
            jumps: Vec::new(),
            jump_at: 0,
        }
    }
}

impl<Id: Clone + Eq + Ord + fmt::Debug> GraphCursor<Id> {
    /// Marks set so far, by name.
    pub fn marks(&self) -> &BTreeMap<char, Id> {
        &self.marks
    }

    /// Jumplist, oldest first; the live cursor is appended only once
    /// `nav.jumplist` has walked back from it.
    pub fn jumplist(&self) -> &[Id] {
        &self.jumps
    }

    /// Move the cursor by host action (a click), recording the jump.
    pub fn select(&mut self, id: Id) {
        self.push_jump(id);
    }

    /// Set the search query, recompute matches, and go to the first.
    pub fn set_query<G: GraphView<Id = Id>>(&mut self, graph: &G, query: &str) -> NavEffect {
        self.query = query.to_owned();
        self.matches = graph.search(query);
        match self.matches.first().cloned() {
            Some(id) => {
                self.push_jump(id);
                NavEffect::Moved
            }
            None => NavEffect::NoTarget(NoTargetReason::NoMatches),
        }
    }

    /// Apply one verb with an optional count.
    pub fn apply<G: GraphView<Id = Id>>(
        &mut self,
        graph: &G,
        sorts: &SortMap,
        verb: &NavVerb,
        count: Option<u32>,
    ) -> NavEffect {
        let n = count.unwrap_or(1).max(1) as usize;
        let (target, reason) = match verb {
            NavVerb::Step { sort, side } => {
                let Some(cur) = &self.current else {
                    return NavEffect::NoTarget(NoTargetReason::NoCursor);
                };
                (
                    graph
                        .neighbors(cur, sorts.kinds(sort), *side)
                        .into_iter()
                        .nth(n - 1),
                    NoTargetReason::NoNeighbor {
                        sort: sort.clone(),
                        side: *side,
                    },
                )
            }
            NavVerb::Sibling { sort, side, delta } => {
                let Some(cur) = &self.current else {
                    return NavEffect::NoTarget(NoTargetReason::NoCursor);
                };
                let kinds = sorts.kinds(sort);
                let parent = graph.neighbors(cur, kinds, *side).into_iter().next();
                let Some(parent) = parent else {
                    return NavEffect::NoTarget(NoTargetReason::NoSiblings {
                        sort: sort.clone(),
                        side: *side,
                    });
                };
                let siblings = graph.neighbors(&parent, kinds, side.flip());
                (
                    step_in(&siblings, cur, *delta * n as i32),
                    NoTargetReason::NoSiblings {
                        sort: sort.clone(),
                        side: *side,
                    },
                )
            }
            NavVerb::Cycle { delta } => {
                let ids = graph.ids();
                (
                    match &self.current {
                        Some(cur) => step_in(&ids, cur, *delta * n as i32),
                        None => ids.first().cloned(),
                    },
                    NoTargetReason::OutOfRange,
                )
            }
            NavVerb::First => (graph.ids().into_iter().nth(n - 1), NoTargetReason::OutOfRange),
            NavVerb::Last => {
                let ids = graph.ids();
                (
                    if count.is_some() {
                        ids.into_iter().nth(n - 1)
                    } else {
                        ids.last().cloned()
                    },
                    NoTargetReason::OutOfRange,
                )
            }
            NavVerb::Search => return NavEffect::PromptSearch,
            NavVerb::Match { delta } => (
                match &self.current {
                    Some(cur) if self.matches.contains(cur) => {
                        step_in(&self.matches, cur, *delta * n as i32)
                    }
                    _ => self.matches.first().cloned(),
                },
                NoTargetReason::NoMatches,
            ),
            NavVerb::Mark(name) => {
                let Some(cur) = self.current.clone() else {
                    return NavEffect::NoTarget(NoTargetReason::NoCursor);
                };
                self.marks.insert(*name, cur);
                return NavEffect::Unchanged;
            }
            NavVerb::Jump(name) => (self.marks.get(name).cloned(), NoTargetReason::MarkUnset(*name)),
            NavVerb::JumpList { delta } => {
                // Materialise the tip so forward can return to it.
                if self.jump_at == self.jumps.len() {
                    if let Some(cur) = &self.current {
                        self.jumps.push(cur.clone());
                    }
                }
                let at = self.jump_at as i64 + *delta as i64 * n as i64;
                if at < 0 || at as usize >= self.jumps.len() {
                    return NavEffect::NoTarget(NoTargetReason::JumplistEnd);
                }
                self.jump_at = at as usize;
                self.current = Some(self.jumps[self.jump_at].clone());
                return NavEffect::Moved;
            }
        };
        match target {
            Some(id) => {
                self.push_jump(id);
                NavEffect::Moved
            }
            None => NavEffect::NoTarget(reason),
        }
    }

    fn push_jump(&mut self, id: Id) {
        if self.current.as_ref() == Some(&id) {
            return;
        }
        self.jumps.truncate(self.jump_at.min(self.jumps.len()));
        if let Some(prev) = self.current.take() {
            self.jumps.push(prev);
        }
        self.jump_at = self.jumps.len();
        self.current = Some(id);
    }
}

impl Side {
    /// The opposite end.
    pub fn flip(self) -> Self {
        match self {
            Self::Domain => Self::Codomain,
            Self::Codomain => Self::Domain,
        }
    }
}

/// `delta` positions from `cur` within `list`, wrapping; `None` if `cur` is
/// absent or the list is empty.
fn step_in<Id: PartialEq + Clone>(list: &[Id], cur: &Id, delta: i32) -> Option<Id> {
    let len = list.len() as i64;
    if len == 0 {
        return None;
    }
    let at = list.iter().position(|x| x == cur)? as i64;
    let next = (at + delta as i64).rem_euclid(len) as usize;
    Some(list[next].clone())
}

#[cfg(test)]
mod tests;
