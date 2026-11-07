//! A vimtutor-style tour: ordered prompts, each satisfied by an intent.
//!
//! [`TourSpec`] is authored data; [`Tour::compile`] parses every expected
//! intent with the host's [`IntentVocabulary`] so a typo fails at load.
//! The host feeds every fired intent to [`Tour::observe`]; the tour only
//! advances on the one the current step expects and otherwise stays put.

use serde::{Deserialize, Serialize};

use super::{IntentVocabulary, KeymapError};

/// Tour spec version this module compiles.
pub const TOUR_SPEC_VERSION: u32 = 1;

/// One authored tour.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TourSpec {
    /// Spec version; must equal [`TOUR_SPEC_VERSION`].
    pub spec_version: u32,
    /// Stable tour id.
    pub id: String,
    /// Shown when the tour completes.
    pub done: String,
    /// Steps in order.
    pub steps: Vec<TourStepSpec>,
}

/// One authored step.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TourStepSpec {
    /// What to tell the user (`Press ] to place the cursor`).
    pub prompt: String,
    /// The intent that completes the step, in vocabulary form.
    pub expect: String,
    /// Shown after the step completes, before the next prompt.
    #[serde(default)]
    pub after: Option<String>,
}

#[derive(Clone, Debug)]
struct Step<I> {
    prompt: String,
    expect: I,
    after: Option<String>,
}

/// A compiled tour with its position.
#[derive(Clone, Debug)]
pub struct Tour<I> {
    id: String,
    done: String,
    steps: Vec<Step<I>>,
    at: usize,
}

/// What a host should show after feeding one intent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TourEvent {
    /// The step completed; show `after` (if any), then `next` (the new
    /// prompt) or nothing when the tour is finished.
    Advanced {
        /// Text to show for the completed step.
        after: Option<String>,
        /// Next prompt; `None` means the tour is done.
        next: Option<String>,
    },
    /// The intent was not the one expected; keep showing the current prompt.
    Ignored,
    /// The tour was already finished.
    Finished,
}

impl<I: IntentVocabulary + PartialEq> Tour<I> {
    /// Compile a spec; every `expect` must parse.
    pub fn compile(spec: &TourSpec) -> Result<Self, KeymapError> {
        if spec.spec_version != TOUR_SPEC_VERSION {
            return Err(KeymapError::Version {
                layer: spec.id.clone(),
                found: spec.spec_version,
            });
        }
        let steps = spec
            .steps
            .iter()
            .map(|s| {
                I::parse_intent(&s.expect)
                    .map(|expect| Step {
                        prompt: s.prompt.clone(),
                        expect,
                        after: s.after.clone(),
                    })
                    .map_err(|message| KeymapError::Intent {
                        layer: spec.id.clone(),
                        keys: s.expect.clone(),
                        message,
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            id: spec.id.clone(),
            done: spec.done.clone(),
            steps,
            at: 0,
        })
    }

    /// Tour id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Current prompt, or the completion text when finished.
    pub fn prompt(&self) -> &str {
        self.steps.get(self.at).map_or(&self.done, |s| &s.prompt)
    }

    /// Zero-based step index and total.
    pub fn progress(&self) -> (usize, usize) {
        (self.at.min(self.steps.len()), self.steps.len())
    }

    /// Whether every step is done.
    pub fn finished(&self) -> bool {
        self.at >= self.steps.len()
    }

    /// Start over.
    pub fn restart(&mut self) {
        self.at = 0;
    }

    /// Feed one fired intent.
    pub fn observe(&mut self, intent: &I) -> TourEvent {
        let Some(step) = self.steps.get(self.at) else {
            return TourEvent::Finished;
        };
        if &step.expect != intent {
            return TourEvent::Ignored;
        }
        let after = step.after.clone();
        self.at += 1;
        TourEvent::Advanced {
            after,
            next: self.steps.get(self.at).map(|s| s.prompt.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> TourSpec {
        TourSpec {
            spec_version: TOUR_SPEC_VERSION,
            id: "t".into(),
            done: "done".into(),
            steps: vec![
                TourStepSpec {
                    prompt: "press a".into(),
                    expect: "a".into(),
                    after: Some("good".into()),
                },
                TourStepSpec {
                    prompt: "press b".into(),
                    expect: "b".into(),
                    after: None,
                },
            ],
        }
    }

    #[test]
    fn advances_only_on_the_expected_intent_and_finishes() {
        let mut tour: Tour<String> = Tour::compile(&spec()).unwrap();
        assert_eq!(tour.prompt(), "press a");
        assert_eq!(tour.observe(&"b".to_string()), TourEvent::Ignored);
        assert_eq!(
            tour.observe(&"a".to_string()),
            TourEvent::Advanced {
                after: Some("good".into()),
                next: Some("press b".into())
            }
        );
        assert_eq!(tour.progress(), (1, 2));
        assert_eq!(
            tour.observe(&"b".to_string()),
            TourEvent::Advanced {
                after: None,
                next: None
            }
        );
        assert!(tour.finished());
        assert_eq!(tour.prompt(), "done");
        assert_eq!(tour.observe(&"a".to_string()), TourEvent::Finished);
        tour.restart();
        assert_eq!(tour.prompt(), "press a");
    }

    #[test]
    fn bad_expect_fails_at_compile() {
        #[derive(Clone, Debug, PartialEq)]
        struct Only;
        impl IntentVocabulary for Only {
            fn parse_intent(s: &str) -> Result<Self, String> {
                (s == "only").then_some(Only).ok_or_else(|| format!("unknown `{s}`"))
            }
        }
        let mut s = spec();
        s.steps[0].expect = "only".into();
        assert!(matches!(Tour::<Only>::compile(&s), Err(KeymapError::Intent { keys, .. }) if keys == "b"));
    }
}
