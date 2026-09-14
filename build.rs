//! Generates the pre-WASM boot stylesheet from the canonical token source.
//!
//! `assets/panel-kit-boot.css` used to be hand-maintained, which meant the
//! palette existed in three places (the `:root` block, the terminal `Theme`,
//! and the boot shell) held together by discipline alone. The discipline had
//! already failed by seven near-miss values. The boot shell genuinely cannot
//! read `panel_kit::CSS` — it paints before the WASM module exists — so the
//! fix is to generate it rather than to ask people to remember.
//!
//! Every `{{name}}` in `assets/panel-kit-boot.css.in` is replaced with the
//! matching `panel_kit_core::tokens` value. An unknown placeholder is a hard
//! error: a typo must not silently ship as literal text in a stylesheet.

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::PathBuf;

use panel_kit_core::tokens;

const TEMPLATE: &str = "assets/panel-kit-boot.css.in";

fn main() {
    println!("cargo:rerun-if-changed={TEMPLATE}");
    println!("cargo:rerun-if-changed=build.rs");

    let template =
        fs::read_to_string(TEMPLATE).unwrap_or_else(|e| panic!("cannot read {TEMPLATE}: {e}"));

    let (rendered, used) = render(&template);

    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by cargo"))
        .join("panel-kit-boot.css");
    fs::write(&out, rendered).unwrap_or_else(|e| panic!("cannot write {}: {e}", out.display()));

    // Surface unused tokens as a note rather than an error: not every colour
    // belongs in a loading shell, and failing the build for that would just
    // teach people to paste placeholders in to silence it.
    let unused: Vec<&str> = tokens::DARK
        .iter()
        .map(|t| t.name)
        .filter(|n| !used.contains(*n))
        .collect();
    if !unused.is_empty() {
        println!(
            "cargo:warning=boot stylesheet does not use token(s): {}",
            unused.join(", ")
        );
    }
}

/// Substitute every `{{name}}`, returning the rendered sheet and the set of
/// token names it consumed.
fn render(template: &str) -> (String, BTreeSet<String>) {
    let mut out = String::with_capacity(template.len());
    let mut used = BTreeSet::new();
    let mut rest = template;

    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let end = after
            .find("}}")
            .unwrap_or_else(|| panic!("unterminated {{{{ placeholder in {TEMPLATE}"));
        let name = after[..end].trim();

        let value = if name == "mono" {
            tokens::MONO.to_string()
        } else {
            tokens::by_name(name)
                .unwrap_or_else(|| {
                    panic!(
                        "{TEMPLATE} references unknown token `{name}`; \
                         known tokens: mono, {}",
                        tokens::DARK
                            .iter()
                            .map(|t| t.name)
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                })
                .hex
                .to_string()
        };

        out.push_str(&value);
        used.insert(name.to_string());
        rest = &after[end + 2..];
    }
    out.push_str(rest);

    (out, used)
}
