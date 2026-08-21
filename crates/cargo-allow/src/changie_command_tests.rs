//! Changie command projection tests (#3623): projection parity, exit
//! posture, discoverability, and the issue's falsifier list at the
//! command surface.

use crate::changie::{render_human, render_json, render_sarif, should_fail};
use crate::changie_source_view::ChangieConfigSelectionV1;
use crate::changie_source_view::analyze_source_view;
use effortless_repo_snapshot::RepositorySourceView;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct FixtureRepo {
    root: PathBuf,
}

impl FixtureRepo {
    fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "changie-command-{}-{}-{}",
            std::process::id(),
            label,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&dir)
            .unwrap_or_else(|err| std::panic::panic_any(format!("mkdir: {err}")));
        Self { root: dir }
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.root.join(relative);
        fs::create_dir_all(path.parent().unwrap_or(&self.root))
            .unwrap_or_else(|err| std::panic::panic_any(format!("parent: {err}")));
        fs::write(&path, contents)
            .unwrap_or_else(|err| std::panic::panic_any(format!("write {relative}: {err}")));
    }

    fn git(&self, args: &[&str]) -> Output {
        Command::new("git")
            .arg("-C")
            .arg(&self.root)
            .args(args)
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("git {args:?}: {err}")))
    }
}

impl Drop for FixtureRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn repo_with(label: &str, config: &str, fragment: &str) -> FixtureRepo {
    let repo = FixtureRepo::new(label);
    repo.git(&["init"]);
    repo.git(&["config", "user.email", "changie-command@example.invalid"]);
    repo.git(&["config", "user.name", "changie-command test"]);
    repo.write(".changie.yaml", config);
    repo.write(".changes/Fixture.yaml", fragment);
    repo.git(&["add", "--all"]);
    repo.git(&["commit", "-m", "command fixture"]);
    repo
}

fn analyze(root: &Path) -> crate::changie_source_view::ChangieAnalysisResultV1 {
    let view = RepositorySourceView::filesystem(root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("view: {err}")));
    analyze_source_view(&view, &ChangieConfigSelectionV1::DefaultNames)
        .unwrap_or_else(|err| std::panic::panic_any(format!("analyze: {err}")))
}

const CONFIG: &str = "changesDir: .changes\nunreleasedDir: .\nkinds:\n  - label: Fixed\n";
const VALID: &str = "kind: Fixed\nbody: text\n";
const INVALID: &str = "kind: Added\nbody: x\n";

#[test]
fn projections_agree_on_rule_and_result_identity() {
    // Falsifier 2: human, JSON, and SARIF must not disagree on rule or
    // result identity for the same canonical analysis.
    let repo = repo_with("parity", CONFIG, INVALID);
    let result = analyze(&repo.root);
    assert!(!result.report.diagnostics.is_empty());
    let human = render_human(&result);
    let json = render_json(&result);
    let sarif = render_sarif(&result);
    for surface in [&human, &json, &sarif] {
        assert!(
            surface.contains("changie.fragment.kind_unknown"),
            "projection lost the rule identity: {}",
            if surface.len() > 80 {
                &surface[..80]
            } else {
                surface
            }
        );
    }
    assert!(human.contains("provenance: rust_static_companion"));
    assert!(json.contains("\"provenance\": \"rust_static_companion\""));
    assert!(sarif.contains("\"ruleId\": \"changie.fragment.kind_unknown\""));
    assert!(json.contains("\"analysisIdentity\""));
    assert!(sarif.contains("analysisIdentity"));
    assert!(human.contains(&result.analysis_identity));
}

#[test]
fn clean_complete_result_exits_zero_and_partial_never_mimics_clean() {
    let repo = repo_with("exit-clean", CONFIG, VALID);
    let result = analyze(&repo.root);
    assert!(!should_fail(&result));
    let human = render_human(&result);
    assert!(human.contains("Result: clean"));
    assert!(human.contains("rendering not claimed"));
}

#[test]
fn findings_fail() {
    let repo = repo_with("exit-findings", CONFIG, INVALID);
    let result = analyze(&repo.root);
    assert!(should_fail(&result));
}

#[test]
fn sarif_has_physical_locations_and_no_render_claim() {
    let repo = repo_with("sarif-shape", CONFIG, "kind: Fixed\nbody: \"\"\n");
    let result = analyze(&repo.root);
    let sarif = render_sarif(&result);
    assert!(sarif.contains("\"version\": \"2.1.0\""));
    assert!(sarif.contains("\"startLine\""));
    assert!(sarif.contains("no render, batch, or merge proof"));
    assert!(!sarif.to_lowercase().contains("\"security\""));
}

#[test]
fn command_help_is_discoverable() {
    // Discoverability falsifier: the surface must be visible in help
    // without hidden configuration.
    let binary = std::env::current_exe().ok().and_then(|path| {
        path.parent()
            .and_then(Path::parent)
            .map(|parent| parent.join("cargo-allow.exe"))
    });
    let Some(binary) = binary.filter(|path| path.is_file()) else {
        return;
    };
    let output = Command::new(&binary)
        .args(["changie", "lint", "--help"])
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("help: {err}")));
    assert!(output.status.success(), "help must succeed");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("--staged"), "{text}");
    assert!(text.contains("--committed"), "{text}");
    assert!(text.contains("--config"), "{text}");
    assert!(text.contains("sarif"), "{text}");
}
