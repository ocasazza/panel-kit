use std::path::{Path, PathBuf};
use std::process::ExitCode;

use visual_parity::capture::{build_web, capture_bevy, capture_web, workspace_root};
use visual_parity::compare::Comparison;
use visual_parity::compare_files;

const USAGE: &str = "usage:
  visual-parity check [OUT_DIR]               capture web + Bevy, compare, fail on unexplained differences
  visual-parity compare WEB.png BEVY.png OUT  compare existing captures

OUT_DIR defaults to target/visual-parity. Run inside `nix develop`.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["check"] => check(&workspace_root().join("target/visual-parity")),
        ["check", out] => check(Path::new(out)),
        ["compare", web, bevy, out] => compare_files(Path::new(web), Path::new(bevy), Path::new(out))
            .map(|comparison| verdict(&comparison, Path::new(out))),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("visual-parity: {error}");
            ExitCode::FAILURE
        }
    }
}

fn check(out: &Path) -> Result<bool, String> {
    let site = out.join("web");
    std::fs::create_dir_all(&site).map_err(|error| format!("{}: {error}", site.display()))?;
    let out = std::fs::canonicalize(out).map_err(|error| format!("{}: {error}", out.display()))?;
    let (web_png, bevy_png): (PathBuf, PathBuf) = (out.join("web.png"), out.join("bevy.png"));
    eprintln!("visual-parity: capturing Bevy");
    capture_bevy(&bevy_png)?;
    eprintln!("visual-parity: building and capturing web");
    build_web(&out.join("web"))?;
    capture_web(&out.join("web"), &web_png)?;
    let comparison = compare_files(&web_png, &bevy_png, &out)?;
    Ok(verdict(&comparison, &out))
}

/// Print the summary; `true` when nothing unexplained differs.
fn verdict(comparison: &Comparison, out: &Path) -> bool {
    let (mut pass, mut fail) = (0, 0);
    for (panel, p, f) in comparison.summary() {
        println!("  {panel:<14} pass {p:4} fail {f:3}");
        pass += p;
        fail += f;
    }
    let unexplained: Vec<_> = comparison.unexplained().collect();
    println!(
        "pass {pass} fail {fail} ({} unexplained) -> {}",
        unexplained.len(),
        out.join("report.md").display()
    );
    for row in &unexplained {
        println!(
            "  UNEXPLAINED {} | {} | {}: web {} bevy {} ({})",
            row.panel, row.part, row.property, row.web, row.bevy, row.delta
        );
    }
    unexplained.is_empty()
}
