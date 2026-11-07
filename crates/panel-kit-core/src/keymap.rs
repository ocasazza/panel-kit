//! Keymaps as a grammar over key chords.
//!
//! A [`KeymapSpec`] is authored data: rules whose `keys` are a sequence of
//! chord tokens and whose `intent` is a string the host's
//! [`IntentVocabulary`] parses at compile time. [`Keymap::compile`] merges
//! layers (a later layer shadows an earlier rule on the same sequence, a rule
//! that is a strict prefix of another is a compile error) into a trie, and
//! [`Keymap::feed`] parses one chord at a time with a vi-style `[count]
//! sequence` shape per mode.
//!
//! Escape is not a rule: it always cancels a pending sequence and count and
//! returns to the first declared mode.
//!
//! Chord tokens: a single printable character; `<esc>` `<cr>` `<tab>`
//! `<space>` `<bs>` `<del>` `<home>` `<end>` `<pgup>` `<pgdn>` `<left>`
//! `<right>` `<up>` `<down>`; modifiers prefix a token as `<c-x>` `<a-x>`
//! `<s-x>` `<m-x>` (combinable, `<c-s-left>`); `{a-z}` captures one lowercase
//! letter and substitutes it for `$1` (then `$2`, ...) in the intent.

use std::collections::HashSet;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::reducer::WorkspaceEvent;
use crate::{FocusContext, Key, KeyChord, PanelKey};

/// Keymap spec version this module compiles.
pub const KEYMAP_SPEC_VERSION: u32 = 1;

/// One authored keymap layer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct KeymapSpec {
    /// Spec version; must equal [`KEYMAP_SPEC_VERSION`].
    pub spec_version: u32,
    /// Stable layer id, for diagnostics.
    pub id: String,
    /// Modes this layer declares, first is the initial mode. Layers merge
    /// mode lists in order; the first layer's first mode is initial.
    pub modes: Vec<String>,
    /// Rules in authoring order.
    pub rules: Vec<RuleSpec>,
}

/// One authored rule.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RuleSpec {
    /// Space-separated chord tokens (see module docs).
    pub keys: String,
    /// Intent handed to the vocabulary; `$n` is replaced by the n-th capture.
    pub intent: String,
    /// Modes the rule is active in; `None` means every declared mode.
    #[serde(default)]
    pub modes: Option<Vec<String>>,
    /// Mode entered after the rule fires; `None` keeps the current mode.
    #[serde(default)]
    pub enter: Option<String>,
    /// Human label for help surfaces (`parent`, `first node`); defaults to
    /// the intent.
    #[serde(default)]
    pub label: Option<String>,
    /// Help group (`Move`, `Marks`); ungrouped rules list last.
    #[serde(default)]
    pub group: Option<String>,
}

/// One rule as a help surface sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleInfo<'a> {
    /// Display sequence (`g g`).
    pub keys: &'a str,
    /// Intent template.
    pub intent: &'a str,
    /// Label, or the intent when none was authored.
    pub label: &'a str,
    /// Group, if authored.
    pub group: Option<&'a str>,
}

/// One rule still reachable from the pending sequence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    /// Full display sequence (`g g`).
    pub keys: String,
    /// Label, or the intent when none was authored.
    pub label: String,
}

/// Who owns the keyboard right now, for a mode line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Posture<K> {
    /// The keymap, in the named mode with any typed count.
    Keymap {
        /// Mode name.
        mode: String,
        /// Count typed so far.
        count: Option<u32>,
        /// Pending sequence, space separated.
        pending: String,
    },
    /// A focused panel's own commands; bare keys never reach the keymap.
    Panel(K),
    /// A text input; bare keys are text.
    TextInput,
}

/// Parses authored intent strings into the host's typed vocabulary.
pub trait IntentVocabulary: Sized + Clone {
    /// Parse one intent string; `Err` is the message reported at compile.
    fn parse_intent(intent: &str) -> Result<Self, String>;
}

impl IntentVocabulary for String {
    fn parse_intent(intent: &str) -> Result<Self, String> {
        Ok(intent.to_owned())
    }
}

/// Why a spec failed to compile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeymapError {
    /// Unsupported `spec_version`.
    Version {
        /// Layer id.
        layer: String,
        /// Version found.
        found: u32,
    },
    /// A layer declared no modes.
    NoModes {
        /// Layer id.
        layer: String,
    },
    /// A chord token did not parse.
    Token {
        /// Layer id.
        layer: String,
        /// Rule's `keys`.
        keys: String,
        /// Offending token.
        token: String,
    },
    /// A rule named a mode no layer declares.
    UnknownMode {
        /// Layer id.
        layer: String,
        /// Rule's `keys`.
        keys: String,
        /// Mode name.
        mode: String,
    },
    /// The vocabulary rejected an intent.
    Intent {
        /// Layer id.
        layer: String,
        /// Rule's `keys`.
        keys: String,
        /// Vocabulary message.
        message: String,
    },
    /// One rule's sequence is a strict prefix of another's in the same mode.
    Prefix {
        /// Mode name.
        mode: String,
        /// Shorter sequence.
        prefix: String,
        /// Longer sequence.
        longer: String,
    },
}

impl fmt::Display for KeymapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Version { layer, found } => {
                write!(f, "{layer}: spec_version {found} != {KEYMAP_SPEC_VERSION}")
            }
            Self::NoModes { layer } => write!(f, "{layer}: no modes declared"),
            Self::Token { layer, keys, token } => {
                write!(f, "{layer}: rule `{keys}`: bad token `{token}`")
            }
            Self::UnknownMode { layer, keys, mode } => {
                write!(f, "{layer}: rule `{keys}`: unknown mode `{mode}`")
            }
            Self::Intent {
                layer,
                keys,
                message,
            } => write!(f, "{layer}: rule `{keys}`: {message}"),
            Self::Prefix {
                mode,
                prefix,
                longer,
            } => write!(f, "mode {mode}: `{prefix}` is a prefix of `{longer}`"),
        }
    }
}

impl std::error::Error for KeymapError {}

/// One matcher position in a rule sequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Token {
    Exact(KeyChord),
    /// `{a-z}`: one unmodified lowercase letter, captured.
    Letter,
}

impl Token {
    fn matches(self, chord: KeyChord) -> bool {
        match self {
            // A printable key already carries its case, and backends disagree
            // on whether Shift is reported alongside it.
            Self::Exact(expected) if matches!(expected.key, Key::Char(_)) => {
                expected.key == chord.key
                    && expected.alt == chord.alt
                    && expected.ctrl == chord.ctrl
                    && expected.meta == chord.meta
            }
            Self::Exact(expected) => expected == chord,
            Self::Letter => {
                !chord.alt
                    && !chord.ctrl
                    && !chord.meta
                    && matches!(chord.key, Key::Char(c) if c.is_ascii_lowercase())
            }
        }
    }
}

#[derive(Clone, Debug)]
struct Rule<I> {
    tokens: Vec<Token>,
    intent_template: String,
    /// Pre-parsed when the template has no captures.
    intent: Option<I>,
    enter: Option<usize>,
    display: String,
    label: String,
    group: Option<String>,
}

/// A compiled, layered keymap.
#[derive(Clone, Debug)]
pub struct Keymap<I> {
    modes: Vec<String>,
    /// Per mode, rules in precedence order (later layers first).
    rules: Vec<Vec<Rule<I>>>,
}

/// Parser state carried between [`Keymap::feed`] calls.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeymapState {
    /// Index into the keymap's modes.
    pub mode: usize,
    /// Accumulated count prefix.
    pub count: Option<u32>,
    /// Chords consumed by an incomplete sequence.
    pub pending: Vec<KeyChord>,
}

/// What one chord did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resolved<I> {
    /// A rule fired.
    Intent {
        /// Parsed intent with captures substituted.
        intent: I,
        /// Count prefix, if any was typed.
        count: Option<u32>,
    },
    /// The chord extended a sequence; `candidates` are the rules still open.
    Pending {
        /// Every rule still reachable.
        candidates: Vec<Candidate>,
    },
    /// Escape cancelled pending input or a count.
    Cancelled,
    /// No rule in the current mode starts with the pending sequence; the chord
    /// is the host's to handle and any pending input was dropped.
    Unbound(KeyChord),
}

impl<I: IntentVocabulary> Keymap<I> {
    /// Compile layers in precedence order (later shadows earlier).
    pub fn compile(layers: &[KeymapSpec]) -> Result<Self, KeymapError> {
        let mut modes: Vec<String> = Vec::new();
        for layer in layers {
            if layer.spec_version != KEYMAP_SPEC_VERSION {
                return Err(KeymapError::Version {
                    layer: layer.id.clone(),
                    found: layer.spec_version,
                });
            }
            if layer.modes.is_empty() {
                return Err(KeymapError::NoModes {
                    layer: layer.id.clone(),
                });
            }
            for mode in &layer.modes {
                if !modes.contains(mode) {
                    modes.push(mode.clone());
                }
            }
        }
        let mode_index = |layer: &KeymapSpec, rule: &RuleSpec, name: &str| {
            modes
                .iter()
                .position(|m| m == name)
                .ok_or_else(|| KeymapError::UnknownMode {
                    layer: layer.id.clone(),
                    keys: rule.keys.clone(),
                    mode: name.to_owned(),
                })
        };

        let mut rules: Vec<Vec<Rule<I>>> = vec![Vec::new(); modes.len()];
        for layer in layers.iter().rev() {
            for rule in layer.rules.iter().rev() {
                let tokens = parse_tokens(&rule.keys).map_err(|token| KeymapError::Token {
                    layer: layer.id.clone(),
                    keys: rule.keys.clone(),
                    token,
                })?;
                let captures = tokens.iter().filter(|t| **t == Token::Letter).count();
                let intent = if captures == 0 {
                    Some(
                        I::parse_intent(&rule.intent).map_err(|message| KeymapError::Intent {
                            layer: layer.id.clone(),
                            keys: rule.keys.clone(),
                            message,
                        })?,
                    )
                } else {
                    // Validate the template with placeholder captures so a bad
                    // verb still fails at compile.
                    let probe = substitute(&rule.intent, &vec!['a'; captures]);
                    I::parse_intent(&probe).map_err(|message| KeymapError::Intent {
                        layer: layer.id.clone(),
                        keys: rule.keys.clone(),
                        message,
                    })?;
                    None
                };
                let enter = rule
                    .enter
                    .as_deref()
                    .map(|m| mode_index(layer, rule, m))
                    .transpose()?;
                let active: Vec<usize> = match &rule.modes {
                    Some(names) => names
                        .iter()
                        .map(|m| mode_index(layer, rule, m))
                        .collect::<Result<_, _>>()?,
                    None => (0..modes.len()).collect(),
                };
                let compiled = Rule {
                    tokens,
                    intent_template: rule.intent.clone(),
                    intent,
                    enter,
                    display: rule.keys.split_whitespace().collect::<Vec<_>>().join(" "),
                    label: rule.label.clone().unwrap_or_else(|| rule.intent.clone()),
                    group: rule.group.clone(),
                };
                for mode in active {
                    rules[mode].push(compiled.clone());
                }
            }
        }

        for (mode, mode_rules) in rules.iter_mut().enumerate() {
            // Shadowing: keep the first (highest precedence) rule per sequence.
            let mut seen: HashSet<Vec<Token>> = HashSet::new();
            mode_rules.retain(|rule| seen.insert(rule.tokens.clone()));
            for a in mode_rules.iter() {
                for b in mode_rules.iter() {
                    if a.tokens.len() < b.tokens.len() && b.tokens.starts_with(&a.tokens) {
                        return Err(KeymapError::Prefix {
                            mode: modes[mode].clone(),
                            prefix: a.display.clone(),
                            longer: b.display.clone(),
                        });
                    }
                }
            }
        }

        Ok(Self { modes, rules })
    }

    /// Declared mode names, initial first.
    pub fn modes(&self) -> &[String] {
        &self.modes
    }

    /// Name of the mode `state` is in.
    pub fn mode_name(&self, state: &KeymapState) -> &str {
        &self.modes[state.mode]
    }

    /// Display sequences of every rule in `mode`, for help surfaces.
    pub fn rules_in(&self, mode: usize) -> impl Iterator<Item = RuleInfo<'_>> {
        self.rules[mode].iter().map(|r| RuleInfo {
            keys: &r.display,
            intent: &r.intent_template,
            label: &r.label,
            group: r.group.as_deref(),
        })
    }

    /// Rules in `mode` whose sequence starts with `pending`, as candidates.
    pub fn candidates(&self, state: &KeymapState) -> Vec<Candidate> {
        self.rules[state.mode]
            .iter()
            .filter(|r| {
                r.tokens.len() > state.pending.len()
                    && r.tokens.iter().zip(&state.pending).all(|(t, c)| t.matches(*c))
            })
            .map(|r| Candidate {
                keys: r.display.clone(),
                label: r.label.clone(),
            })
            .collect()
    }

    /// Mode-line posture for `state` under `focus`: the keymap only owns bare
    /// keys when no text input or panel does.
    pub fn posture<K: PanelKey>(&self, state: &KeymapState, focus: &FocusContext<K>) -> Posture<K> {
        match focus {
            FocusContext::TextInput => Posture::TextInput,
            FocusContext::Panel(key) => Posture::Panel(*key),
            FocusContext::Workspace => Posture::Keymap {
                mode: self.modes[state.mode].clone(),
                count: state.count,
                pending: state
                    .pending
                    .iter()
                    .map(|c| chord_display(*c))
                    .collect::<Vec<_>>()
                    .join(" "),
            },
        }
    }

    /// Parse one chord.
    pub fn feed(&self, state: &mut KeymapState, chord: KeyChord) -> Resolved<I> {
        if chord.key == Key::Escape && chord_is_bare(chord) {
            let had_input = !state.pending.is_empty() || state.count.is_some();
            state.pending.clear();
            state.count = None;
            state.mode = 0;
            if had_input {
                return Resolved::Cancelled;
            }
        }

        if state.pending.is_empty() {
            if let Key::Char(d) = chord.key {
                if chord_is_bare(chord) && d.is_ascii_digit() {
                    let digit = d as u32 - '0' as u32;
                    let starts_rule = self.rules[state.mode]
                        .iter()
                        .any(|r| r.tokens[0].matches(chord));
                    if !starts_rule && (state.count.is_some() || digit != 0) {
                        let count = state.count.unwrap_or(0).saturating_mul(10).saturating_add(digit);
                        state.count = Some(count);
                        return Resolved::Pending {
                            candidates: Vec::new(),
                        };
                    }
                }
            }
        }

        state.pending.push(chord);
        let depth = state.pending.len();
        let open: Vec<&Rule<I>> = self.rules[state.mode]
            .iter()
            .filter(|r| {
                r.tokens.len() >= depth
                    && r
                        .tokens
                        .iter()
                        .zip(&state.pending)
                        .all(|(t, c)| t.matches(*c))
            })
            .collect();

        if let Some(rule) = open.iter().find(|r| r.tokens.len() == depth) {
            let captures: Vec<char> = rule
                .tokens
                .iter()
                .zip(&state.pending)
                .filter(|(t, _)| **t == Token::Letter)
                .filter_map(|(_, c)| match c.key {
                    Key::Char(ch) => Some(ch),
                    _ => None,
                })
                .collect();
            let intent = match &rule.intent {
                Some(intent) => intent.clone(),
                None => I::parse_intent(&substitute(&rule.intent_template, &captures))
                    .expect("template validated at compile"),
            };
            let count = state.count.take();
            state.pending.clear();
            if let Some(mode) = rule.enter {
                state.mode = mode;
            }
            return Resolved::Intent { intent, count };
        }

        if open.is_empty() {
            state.pending.clear();
            state.count = None;
            return Resolved::Unbound(chord);
        }

        Resolved::Pending {
            candidates: open
                .iter()
                .map(|r| Candidate {
                    keys: r.display.clone(),
                    label: r.label.clone(),
                })
                .collect(),
        }
    }
}

/// Keymap plus parser state, routing each chord to either an application
/// intent or the workspace reducer.
///
/// Bare keys with [`FocusContext::TextInput`] focus bypass the keymap so an
/// editor keeps its characters; everything the keymap leaves unbound becomes
/// a [`WorkspaceEvent::Key`] for `reduce` and its `command_for` table.
#[derive(Clone, Debug)]
pub struct InputRouter<I> {
    keymap: Keymap<I>,
    state: KeymapState,
}

/// Where one chord went.
#[derive(Clone, Debug, PartialEq)]
pub enum Routed<K: PanelKey, I> {
    /// Hand to the workspace reducer.
    Workspace(WorkspaceEvent<K>),
    /// An application intent fired.
    Intent {
        /// Parsed intent.
        intent: I,
        /// Count prefix, if typed.
        count: Option<u32>,
    },
    /// A sequence is in progress; show `candidates`.
    Pending {
        /// Every rule still reachable.
        candidates: Vec<Candidate>,
    },
    /// Escape cancelled pending input.
    Cancelled,
}

impl<I: IntentVocabulary> InputRouter<I> {
    /// Wrap a compiled keymap.
    pub fn new(keymap: Keymap<I>) -> Self {
        Self {
            keymap,
            state: KeymapState::default(),
        }
    }

    /// Compile `layers` (see [`Keymap::compile`]) and wrap them.
    pub fn compile(layers: &[KeymapSpec]) -> Result<Self, KeymapError> {
        Keymap::compile(layers).map(Self::new)
    }

    /// The compiled keymap.
    pub fn keymap(&self) -> &Keymap<I> {
        &self.keymap
    }

    /// Current parser state.
    pub fn state(&self) -> &KeymapState {
        &self.state
    }

    /// Route one chord.
    pub fn route<K: PanelKey>(&mut self, chord: KeyChord, focus: FocusContext<K>) -> Routed<K, I> {
        if matches!(focus, FocusContext::TextInput) && chord_is_bare(chord) {
            return Routed::Workspace(WorkspaceEvent::Key { chord, focus });
        }
        match self.keymap.feed(&mut self.state, chord) {
            Resolved::Intent { intent, count } => Routed::Intent { intent, count },
            Resolved::Pending { candidates } => Routed::Pending { candidates },
            Resolved::Cancelled => Routed::Cancelled,
            Resolved::Unbound(chord) => Routed::Workspace(WorkspaceEvent::Key { chord, focus }),
        }
    }
}

/// Token form of a chord, the inverse of [`parse_token`].
pub fn chord_display(chord: KeyChord) -> String {
    let name = match chord.key {
        Key::Char(' ') => "space".to_owned(),
        Key::Char(c) => c.to_string(),
        Key::Left => "left".into(),
        Key::Right => "right".into(),
        Key::Up => "up".into(),
        Key::Down => "down".into(),
        Key::Enter => "cr".into(),
        Key::Escape => "esc".into(),
        Key::Tab => "tab".into(),
        Key::Backspace => "bs".into(),
        Key::Delete => "del".into(),
        Key::Home => "home".into(),
        Key::End => "end".into(),
        Key::PageUp => "pgup".into(),
        Key::PageDown => "pgdn".into(),
    };
    let mut mods = String::new();
    if chord.ctrl { mods.push_str("c-"); }
    if chord.alt { mods.push_str("a-"); }
    if chord.meta { mods.push_str("m-"); }
    if chord.shift && !matches!(chord.key, Key::Char(_)) { mods.push_str("s-"); }
    let named = !matches!(chord.key, Key::Char(c) if c != ' ');
    if mods.is_empty() && !named { name } else { format!("<{mods}{name}>") }
}

fn chord_is_bare(chord: KeyChord) -> bool {
    !chord.shift && !chord.alt && !chord.ctrl && !chord.meta
}

fn substitute(template: &str, captures: &[char]) -> String {
    let mut out = template.to_owned();
    for (i, c) in captures.iter().enumerate() {
        out = out.replace(&format!("${}", i + 1), &c.to_string());
    }
    out
}

fn parse_tokens(keys: &str) -> Result<Vec<Token>, String> {
    let tokens: Vec<Token> = keys
        .split_whitespace()
        .map(parse_token)
        .collect::<Result<_, _>>()?;
    if tokens.is_empty() {
        return Err(String::new());
    }
    Ok(tokens)
}

fn parse_token(token: &str) -> Result<Token, String> {
    if token == "{a-z}" {
        return Ok(Token::Letter);
    }
    let mut chars = token.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        return Ok(Token::Exact(bare(Key::Char(c))));
    }
    let inner = token
        .strip_prefix('<')
        .and_then(|t| t.strip_suffix('>'))
        .ok_or_else(|| token.to_owned())?;
    let mut chord = bare(Key::Escape);
    let mut rest = inner;
    loop {
        let Some((modifier, tail)) = rest.split_once('-') else {
            break;
        };
        match modifier {
            "c" => chord.ctrl = true,
            "a" => chord.alt = true,
            "s" => chord.shift = true,
            "m" => chord.meta = true,
            _ => break,
        }
        rest = tail;
    }
    chord.key = match rest {
        "esc" => Key::Escape,
        "cr" => Key::Enter,
        "tab" => Key::Tab,
        "space" => Key::Char(' '),
        "bs" => Key::Backspace,
        "del" => Key::Delete,
        "home" => Key::Home,
        "end" => Key::End,
        "pgup" => Key::PageUp,
        "pgdn" => Key::PageDown,
        "left" => Key::Left,
        "right" => Key::Right,
        "up" => Key::Up,
        "down" => Key::Down,
        other => {
            let mut cs = other.chars();
            match (cs.next(), cs.next()) {
                (Some(c), None) => Key::Char(c),
                _ => return Err(token.to_owned()),
            }
        }
    };
    Ok(Token::Exact(chord))
}

fn bare(key: Key) -> KeyChord {
    KeyChord {
        key,
        shift: false,
        alt: false,
        ctrl: false,
        meta: false,
    }
}

pub mod tour;

#[cfg(test)]
mod tests;
