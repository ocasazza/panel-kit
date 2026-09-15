//! Dependency-graph guard for default-feature Cargo consumers.

use std::process::Command;

#[test]
fn plain_rust_enum_path_has_no_spec_schema_dependency() {
    let output = Command::new(env!("CARGO"))
        .args(["tree", "-p", "panel-kit-core", "--edges", "normal", "--no-default-features"])
        .output()
        .expect("cargo tree runs");

    assert!(output.status.success(), "cargo tree failed: {}", String::from_utf8_lossy(&output.stderr));

    let graph = String::from_utf8(output.stdout).expect("cargo tree emits utf-8");
    assert!(!graph.contains("serde_json"), "default normal dependency graph leaked serde_json:\n{graph}");
    assert!(!graph.contains("schemars"), "default normal dependency graph leaked schemars:\n{graph}");
}
