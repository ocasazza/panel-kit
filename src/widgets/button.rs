//! The content-area button: a square, 1px-stroked caps label that inverts on
//! hover and takes the accent edge while latched. Panel-header controls inset
//! into the border row use [`crate::PanelHeaderButton`] instead.

use dioxus::prelude::*;

/// A square button. `pressed` makes it a toggle (`aria-pressed`, accent edge
/// while true); `compact` is the tight variant for controls inside a row.
/// Other `<button>` attributes (`title`, `disabled`, `aria-*`, `data-*`) pass
/// through.
#[component]
pub fn Button(
    /// Whether a toggle is latched; `None` for a plain action.
    #[props(default)]
    pressed: Option<bool>,
    /// The tight variant for controls inside a row or menu.
    #[props(default)]
    compact: bool,
    #[props(extends = GlobalAttributes, extends = button)] attributes: Vec<Attribute>,
    /// Called on activation.
    on_press: EventHandler<MouseEvent>,
    children: Element,
) -> Element {
    rsx! {
        button {
            r#type: "button",
            class: if compact { "pk-btn compact" } else { "pk-btn" },
            aria_pressed: pressed.map(|on| on.to_string()),
            onclick: move |e| on_press.call(e),
            ..attributes,
            {children}
        }
    }
}
