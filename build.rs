//! Generates theme-derived assets from the canonical core token source.
//!
//! The pre-WASM boot stylesheet, the injected stylesheet's `:root` theme
//! variables, and the `DESIGN.md` token region are generated rather than
//! hand-copied. Unknown boot placeholders fail hard: a typo must not silently
//! ship as literal text in a stylesheet.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use panel_kit_core::theme::{
    css_root_block, design_token_region, replace_generated_region, ThemeTokens,
    CSS_THEME_REGION_BEGIN, CSS_THEME_REGION_END, DESIGN_THEME_REGION_BEGIN,
    DESIGN_THEME_REGION_END,
};
use panel_kit_core::tokens;

const TEMPLATE: &str = "assets/panel-kit-boot.css.in";
const CSS_FILE: &str = "assets/panel-kit.css";
const DESIGN_FILE: &str = "DESIGN.md";

fn main() {
    println!("cargo:rerun-if-changed={TEMPLATE}");
    println!("cargo:rerun-if-changed={CSS_FILE}");
    println!("cargo:rerun-if-changed={DESIGN_FILE}");
    println!("cargo:rerun-if-changed=build.rs");

    let template =
        fs::read_to_string(TEMPLATE).unwrap_or_else(|e| panic!("cannot read {TEMPLATE}: {e}"));
    let rendered = render(&template);
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by cargo"))
        .join("panel-kit-boot.css");
    write_if_changed(&out, &rendered);

    let dark = ThemeTokens::dark();
    update_generated_region(
        CSS_FILE,
        CSS_THEME_REGION_BEGIN,
        CSS_THEME_REGION_END,
        &css_root_block(&dark),
    );
    update_generated_region(
        DESIGN_FILE,
        DESIGN_THEME_REGION_BEGIN,
        DESIGN_THEME_REGION_END,
        &design_token_region(&dark),
    );
}

/// Substitute every double-brace placeholder, returning the rendered sheet.
fn render(template: &str) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;

    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let end = after
            .find("}}")
            .unwrap_or_else(|| panic!("unterminated placeholder in {TEMPLATE}"));
        let name = after[..end].trim();

        out.push_str(&placeholder_value(name));
        rest = &after[end + 2..];
    }
    out.push_str(rest);

    out
}

fn placeholder_value(name: &str) -> String {
    if name == "mono" {
        return tokens::MONO.to_string();
    }

    tokens::by_name(name)
        .unwrap_or_else(|| {
            panic!(
                "{TEMPLATE} references unknown token `{name}`; known tokens: mono, {}",
                tokens::DARK
                    .iter()
                    .map(|t| t.name)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
        .hex
        .to_string()
}

fn update_generated_region(path: &str, begin: &str, end: &str, generated: &str) {
    let document = fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    let rendered = replace_generated_region(&document, begin, end, generated);
    write_if_changed(path, &rendered);
}

fn write_if_changed(path: impl AsRef<Path>, contents: &str) {
    let path = path.as_ref();
    if let Ok(existing) = fs::read_to_string(path) {
        if existing == contents {
            return;
        }
    }

    fs::write(path, contents).unwrap_or_else(|e| panic!("cannot write {}: {e}", path.display()));
}
