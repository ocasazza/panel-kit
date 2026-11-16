//! Renderer-neutral form model: an ordered list of labeled controls.
//!
//! The model owns every field's current value; [`super::reduce`] is the only
//! writer. Backends paint a [`FormModel`] with a [`super::FormState`] and
//! translate platform input into [`super::FormInput`].

use serde::{Deserialize, Serialize};

use super::fmt_f;

/// Glyph that stands in for each character of a password value.
pub const MASK: char = '•';

/// An ordered set of labeled input fields.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FormModel {
    /// Fields in display and focus order.
    pub fields: Vec<FieldModel>,
}

impl FormModel {
    /// The field with stable `id`.
    pub fn field(&self, id: &str) -> Option<&FieldModel> {
        self.fields.iter().find(|field| field.id == id)
    }
}

/// One labeled control.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FieldModel {
    /// Stable key reported in [`super::FormAction`]s.
    pub id: String,
    /// Label in the row's key column.
    pub label: String,
    /// The control and its current value.
    pub control: ControlModel,
    /// A disabled field paints muted, takes no focus, and never changes.
    pub disabled: bool,
    /// Muted note after the control, e.g. a unit or a consequence; empty for none.
    pub note: String,
}

/// What a single-line text control accepts and how it paints its value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum TextEntry {
    /// Free text.
    Plain,
    /// Free text painted as [`MASK`] glyphs.
    Password,
    /// Free text that filters something.
    Search,
    /// An e-mail address.
    Email,
    /// A URL.
    Url,
    /// A telephone number: digits, space, and `+-()`.
    Tel,
    /// An ISO date, `YYYY-MM-DD`.
    Date,
    /// A 24-hour time, `HH:MM`.
    Time,
}

impl TextEntry {
    /// Whether typing `c` may extend a value of this entry kind.
    pub fn accepts(self, c: char) -> bool {
        match self {
            Self::Date => c.is_ascii_digit() || c == '-',
            Self::Time => c.is_ascii_digit() || c == ':',
            Self::Tel => c.is_ascii_digit() || matches!(c, ' ' | '+' | '-' | '(' | ')'),
            Self::Email | Self::Url => !c.is_control() && !c.is_whitespace(),
            Self::Plain | Self::Password | Self::Search => !c.is_control(),
        }
    }

    /// Longest value typing may produce, in characters.
    pub fn max_len(self) -> Option<usize> {
        match self {
            Self::Date => Some(10),
            Self::Time => Some(5),
            _ => None,
        }
    }
}

/// One option of a radio group or select.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ChoiceModel {
    /// Stable machine value.
    pub value: String,
    /// Label painted for the option.
    pub label: String,
}

/// The closed set of renderer-neutral input controls.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ControlModel {
    /// Single-line text entry.
    Text {
        /// Current value.
        value: String,
        /// Prompt painted while the value is empty.
        placeholder: String,
        /// Accepted characters and value presentation.
        entry: TextEntry,
    },
    /// Multi-line text entry.
    TextArea {
        /// Current value; lines separated by `\n`.
        value: String,
        /// Prompt painted while the value is empty.
        placeholder: String,
        /// Visible rows.
        rows: u16,
    },
    /// Stepped number box.
    Number {
        /// Current value, within `[min, max]`.
        value: f64,
        /// Inclusive minimum.
        min: f64,
        /// Inclusive maximum.
        max: f64,
        /// Increment of one step; values snap to `min + n * step`.
        step: f64,
    },
    /// Range slider.
    Slider {
        /// Current value, within `[min, max]`.
        value: f64,
        /// Inclusive minimum.
        min: f64,
        /// Inclusive maximum.
        max: f64,
        /// Increment of one step; values snap to `min + n * step`.
        step: f64,
    },
    /// Independent on/off box.
    Checkbox {
        /// Whether the box is checked.
        checked: bool,
    },
    /// On/off switch that takes effect immediately.
    Switch {
        /// Whether the switch is on.
        on: bool,
    },
    /// One-of-N choice with every option visible.
    Radio {
        /// Selected option value; empty for none.
        value: String,
        /// Options in display order.
        options: Vec<ChoiceModel>,
    },
    /// One-of-N choice behind a popup list.
    Select {
        /// Selected option value; empty for none.
        value: String,
        /// Prompt painted while nothing is selected.
        placeholder: String,
        /// Options in display order.
        options: Vec<ChoiceModel>,
    },
    /// Action button; it holds no value.
    Button {
        /// Button caption.
        text: String,
    },
}

impl ControlModel {
    /// Stable normalized name of this control variant.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Text { .. } => "text",
            Self::TextArea { .. } => "text_area",
            Self::Number { .. } => "number",
            Self::Slider { .. } => "slider",
            Self::Checkbox { .. } => "checkbox",
            Self::Switch { .. } => "switch",
            Self::Radio { .. } => "radio",
            Self::Select { .. } => "select",
            Self::Button { .. } => "button",
        }
    }

    /// The control's current value; `None` for a button.
    pub fn value(&self) -> Option<FieldValue> {
        Some(match self {
            Self::Text { value, .. } | Self::TextArea { value, .. } => {
                FieldValue::Text(value.clone())
            }
            Self::Number { value, .. } | Self::Slider { value, .. } => FieldValue::Number(*value),
            Self::Checkbox { checked } => FieldValue::Bool(*checked),
            Self::Switch { on } => FieldValue::Bool(*on),
            Self::Radio { value, .. } | Self::Select { value, .. } => {
                FieldValue::Choice(value.clone())
            }
            Self::Button { .. } => return None,
        })
    }

    /// The options of a radio group or select; empty for every other control.
    pub fn options(&self) -> &[ChoiceModel] {
        match self {
            Self::Radio { options, .. } | Self::Select { options, .. } => options,
            _ => &[],
        }
    }

    /// Index of the selected option of a radio group or select.
    pub fn selected_index(&self) -> Option<usize> {
        match self {
            Self::Radio { value, options } | Self::Select { value, options, .. } => {
                options.iter().position(|option| option.value == *value)
            }
            _ => None,
        }
    }

    /// Fill fraction (`0.0..=1.0`) of a slider or number within its range.
    pub fn fraction(&self) -> Option<f64> {
        match self {
            Self::Number { value, min, max, .. } | Self::Slider { value, min, max, .. } => {
                Some(if max > min {
                    ((value - min) / (max - min)).clamp(0.0, 1.0)
                } else {
                    0.0
                })
            }
            _ => None,
        }
    }

    /// The text a cell or canvas renderer paints as the control's value:
    /// masked passwords, formatted numbers, the selected option's label.
    /// Empty when the control shows its placeholder or holds no text.
    pub fn display(&self) -> String {
        match self {
            Self::Text { value, entry: TextEntry::Password, .. } => {
                value.chars().map(|_| MASK).collect()
            }
            Self::Text { value, .. } | Self::TextArea { value, .. } => value.clone(),
            Self::Number { value, .. } | Self::Slider { value, .. } => fmt_f(*value),
            Self::Radio { .. } | Self::Select { .. } => self
                .selected_index()
                .map(|index| self.options()[index].label.clone())
                .unwrap_or_default(),
            Self::Button { text } => text.clone(),
            Self::Checkbox { .. } | Self::Switch { .. } => String::new(),
        }
    }

    /// The prompt painted while a text control or select is empty.
    pub fn placeholder(&self) -> &str {
        match self {
            Self::Text { placeholder, .. }
            | Self::TextArea { placeholder, .. }
            | Self::Select { placeholder, .. } => placeholder,
            _ => "",
        }
    }
}

/// A field value reported to the host.
#[derive(Clone, Debug, PartialEq)]
pub enum FieldValue {
    /// Text and text-area values.
    Text(String),
    /// Number and slider values.
    Number(f64),
    /// Checkbox and switch values.
    Bool(bool),
    /// The selected option value of a radio group or select.
    Choice(String),
}
