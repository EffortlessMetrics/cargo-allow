//! Source-view adapter tests (#3622): mixed-view rejection, config
//! selection law, population semantics, determinism, and the required
//! falsifier list — each fixture built before the happy path.

#![cfg(feature = "changie-adapter")]

use crate::changie_source_view::*;
use effortless_repo_snapshot::RepositorySourceView;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

struct FixtureRepo {
    root: PathBuf,
}

impl FixtureRepo {
    fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "changie-source-view-{}-{}-{}",
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

fn committed(label: &str, config: &str, fragment: &str) -> FixtureRepo {
    let repo = FixtureRepo::new(label);
    repo.write(".changie.yaml", config);
    repo.write(".changes/Fixture.yaml", fragment);
    repo.git(&["init"]);
    repo.git(&["config", "user.email", "changie-adapter@example.invalid"]);
    repo.git(&["config", "user.name", "changie-adapter test"]);
    repo.git(&["add", "--all"]);
    repo.git(&["commit", "-m", "adapter fixture"]);
    repo
}

const CONFIG: &str = "changesDir: .changes\nunreleasedDir: .\nkinds:\n  - label: Fixed\n";
const FRAGMENT: &str = "kind: Fixed\nbody: text\n";

#[test]
fn saved_worktree_analysis_is_clean_and_carries_identity() {
    let repo = committed("worktree-clean", CONFIG, FRAGMENT);
    let view = RepositorySourceView::filesystem(&repo.root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("filesystem view: {err}")));
    let result = analyze_source_view(&view, &ChangieConfigSelectionV1::DefaultNames)
        .unwrap_or_else(|err| std::panic::panic_any(format!("analyze: {err}")));
    assert_eq!(result.generation, "1.25");
    assert_eq!(result.view_kind, ChangieSourceViewKind::SavedWorktree);
    assert_eq!(result.config_selection.selected_path, ".changie.yaml");
    assert!(
        result.report.diagnostics.is_empty(),
        "{:#?}",
        result.report.diagnostics
    );
    assert_eq!(result.population.root, ".changes");
    assert_eq!(
        result
            .population
            .inspected
            .iter()
            .map(|entry| entry.repo_path.clone())
            .collect::<Vec<_>>(),
        vec![".changes/Fixture.yaml".to_string()]
    );
    assert!(result.analysis_identity.starts_with("changie.analysis.v1:"));
    assert_eq!(
        result.completeness,
        ChangieAcquisitionCompleteness::Complete
    );
    // The saved-worktree view records its type-blindness limitation.
    assert!(
        result
            .limitations
            .iter()
            .any(|limitation| limitation.contains("does not expose entry types"))
    );
}

#[test]
fn committed_view_ignores_dirty_worktree_bytes() {
    // Falsifier 1 (inverse direction): committed analysis must not read
    // dirty worktree bytes.
    let repo = committed("committed-dirty", CONFIG, FRAGMENT);
    // Dirty the worktree after the commit.
    repo.write(".changes/Fixture.yaml", "kind: Added\nbody: dirty\n");
    let view = RepositorySourceView::committed(&repo.root, "HEAD")
        .unwrap_or_else(|err| std::panic::panic_any(format!("committed view: {err}")));
    let result = analyze_source_view(&view, &ChangieConfigSelectionV1::DefaultNames)
        .unwrap_or_else(|err| std::panic::panic_any(format!("analyze: {err}")));
    assert_eq!(result.view_kind, ChangieSourceViewKind::CommittedTree);
    // Committed fragment is still the committed (clean) one.
    assert!(
        result.report.diagnostics.is_empty(),
        "committed view must analyze committed bytes: {:#?}",
        result.report.diagnostics
    );
    let identity = result
        .population
        .inspected
        .iter()
        .find(|entry| entry.repo_path == ".changes/Fixture.yaml")
        .and_then(|entry| entry.content_identity)
        .unwrap_or_else(|| std::panic::panic_any("committed fragment identity missing"));
    let committed_identity = allow_files::changie::ChangieContentIdentity::of(FRAGMENT.as_bytes());
    assert_eq!(identity, committed_identity);
}

#[test]
fn staged_view_reads_staged_bytes_not_worktree() {
    // Falsifier 1: staged config with dirty-worktree fragment bytes must
    // analyze the staged bytes.
    let repo = committed("staged-dirty", CONFIG, FRAGMENT);
    // Stage a modified fragment, then dirty the worktree further.
    repo.write(
        ".changes/Fixture.yaml",
        "kind: Fixed\nbody: staged version\n",
    );
    repo.git(&["add", ".changes/Fixture.yaml"]);
    repo.write(
        ".changes/Fixture.yaml",
        "kind: Added\nbody: worktree version\n",
    );
    let view = RepositorySourceView::staged(&repo.root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("staged view: {err}")));
    let result = analyze_source_view(&view, &ChangieConfigSelectionV1::DefaultNames)
        .unwrap_or_else(|err| std::panic::panic_any(format!("analyze: {err}")));
    assert_eq!(result.view_kind, ChangieSourceViewKind::StagedIndex);
    assert!(
        result.report.diagnostics.is_empty(),
        "staged bytes are clean: {:#?}",
        result.report.diagnostics
    );
    let identity = result
        .population
        .inspected
        .iter()
        .find(|entry| entry.repo_path == ".changes/Fixture.yaml")
        .and_then(|entry| entry.content_identity)
        .unwrap_or_else(|| std::panic::panic_any("staged fragment identity missing"));
    let staged_identity = allow_files::changie::ChangieContentIdentity::of(
        "kind: Fixed\nbody: staged version\n".as_bytes(),
    );
    assert_eq!(identity, staged_identity);
}

#[test]
fn config_path_change_rediscoveries_the_population_in_the_same_view() {
    // Falsifier 2: a config whose path fields change must rediscover the
    // fragment root inside the same view — the old root is not selected.
    let repo = committed("rediscovery", CONFIG, FRAGMENT);
    // Commit a config that moves the fragment root.
    repo.write(
        ".changie.yaml",
        "changesDir: fragments\nunreleasedDir: current\nkinds:\n  - label: Fixed\n",
    );
    repo.write("fragments/current/Moved.yaml", FRAGMENT);
    repo.git(&["add", "--all"]);
    repo.git(&["commit", "-m", "move fragment root"]);
    let view = RepositorySourceView::committed(&repo.root, "HEAD")
        .unwrap_or_else(|err| std::panic::panic_any(format!("committed view: {err}")));
    let result = analyze_source_view(&view, &ChangieConfigSelectionV1::DefaultNames)
        .unwrap_or_else(|err| std::panic::panic_any(format!("analyze: {err}")));
    assert_eq!(result.population.root, "fragments/current");
    let paths: Vec<&str> = result
        .population
        .inspected
        .iter()
        .map(|entry| entry.repo_path.as_str())
        .collect();
    assert_eq!(paths, vec!["fragments/current/Moved.yaml"]);
}

#[test]
fn malformed_nearer_config_does_not_fall_through() {
    // Falsifier 3: a malformed .changie.yaml must not silently select a
    // clean .changie.yml.
    let repo = committed("malformed-fallthrough", CONFIG, FRAGMENT);
    repo.write(".changie.yaml", "changesDir: [broken\n");
    repo.write(
        ".changie.yml",
        "changesDir: .changes\nunreleasedDir: .\nkinds:\n  - label: Fixed\n",
    );
    repo.git(&["add", "--all"]);
    repo.git(&["commit", "-m", "malformed nearer config"]);
    let view = RepositorySourceView::committed(&repo.root, "HEAD")
        .unwrap_or_else(|err| std::panic::panic_any(format!("committed view: {err}")));
    let result = analyze_source_view(&view, &ChangieConfigSelectionV1::DefaultNames)
        .unwrap_or_else(|err| std::panic::panic_any(format!("analyze: {err}")));
    // The malformed nearer config is selected; the sensor reports it.
    assert_eq!(result.config_selection.selected_path, ".changie.yaml");
    assert!(result.config_selection.ambiguous);
    assert!(
        result
            .report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule.as_str() == "changie.config.malformed")
    );
}

#[test]
fn missing_config_fails_closed_not_empty_clean() {
    // Falsifier 4 (config direction): no config in the view is an
    // error, never an empty clean population.
    let repo = FixtureRepo::new("no-config");
    repo.git(&["init"]);
    let view = RepositorySourceView::filesystem(&repo.root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("filesystem view: {err}")));
    let result = analyze_source_view(&view, &ChangieConfigSelectionV1::DefaultNames);
    assert!(matches!(
        result,
        Err(ChangieSourceViewError::ConfigNotFound { .. })
    ));
}

#[test]
fn deleted_fragment_stays_typed_in_the_population() {
    // Falsifier 5: a deleted tracked fragment remains visible with its
    // typed state instead of disappearing.
    let repo = committed("deleted-fragment", CONFIG, FRAGMENT);
    repo.write(".changes/Gone.yaml", FRAGMENT);
    repo.git(&["add", "--all"]);
    repo.git(&["commit", "-m", "add second fragment"]);
    // Committed view at the first commit still sees both; a staged
    // deletion must surface as typed.
    fs::remove_file(repo.root.join(".changes/Fixture.yaml")).ok();
    repo.git(&["add", "--all"]);
    let view = RepositorySourceView::staged(&repo.root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("staged view: {err}")));
    let result = analyze_source_view(&view, &ChangieConfigSelectionV1::DefaultNames)
        .unwrap_or_else(|err| std::panic::panic_any(format!("analyze: {err}")));
    let deleted = result
        .population
        .inspected
        .iter()
        .find(|entry| entry.repo_path == ".changes/Fixture.yaml")
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "deleted fragment vanished from the population: {:#?}",
                result.population
            ))
        });
    assert_eq!(deleted.state, ChangieEntryState::DeletedTracked);
    assert!(
        result
            .report
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.rule.as_str() == "changie.fragment.entry_unsupported" })
    );
}

#[test]
fn explicit_config_selection_is_exact() {
    let repo = committed("explicit-config", CONFIG, FRAGMENT);
    let view = RepositorySourceView::filesystem(&repo.root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("filesystem view: {err}")));
    let result = analyze_source_view(
        &view,
        &ChangieConfigSelectionV1::Explicit(".changie.yaml".into()),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("analyze: {err}")));
    assert_eq!(result.config_selection.selected_path, ".changie.yaml");
    // Falsifier 7: selection ignores the environment entirely — no
    // ambient CHANGIE_CONFIG_PATH exists to inherit (structural).
    let missing = analyze_source_view(
        &view,
        &ChangieConfigSelectionV1::Explicit(".changie.other.yaml".into()),
    );
    assert!(matches!(
        missing,
        Err(ChangieSourceViewError::ConfigNotFound { .. })
    ));
}

#[test]
fn traversal_escape_in_config_paths_fails_closed() {
    // Falsifier 8: config path fields cannot escape the repository root.
    let repo = FixtureRepo::new("escape");
    repo.write(
        ".changie.yaml",
        "changesDir: ../outside\nunreleasedDir: .\n",
    );
    repo.git(&["add", "--all"]);
    let view = RepositorySourceView::filesystem(&repo.root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("filesystem view: {err}")));
    let result = analyze_source_view(&view, &ChangieConfigSelectionV1::DefaultNames);
    assert!(matches!(
        result,
        Err(ChangieSourceViewError::ConfigPathUnsafe { .. })
    ));
}

#[test]
fn equal_subjects_produce_equal_analysis_identities() {
    // Falsifier 9: traversal order cannot change the result identity.
    let repo = committed("determinism", CONFIG, FRAGMENT);
    repo.write(".changes/A-second.yaml", FRAGMENT);
    repo.write(".changes/B-first.yaml", FRAGMENT);
    repo.git(&["add", "--all"]);
    repo.git(&["commit", "-m", "more fragments"]);
    let first = RepositorySourceView::committed(&repo.root, "HEAD")
        .and_then(|view| {
            analyze_source_view(&view, &ChangieConfigSelectionV1::DefaultNames)
                .map_err(|err| effortless_repo_snapshot::SnapshotError::new(err.to_string()))
        })
        .unwrap_or_else(|err| std::panic::panic_any(format!("first: {err}")));
    let second = RepositorySourceView::committed(&repo.root, "HEAD")
        .and_then(|view| {
            analyze_source_view(&view, &ChangieConfigSelectionV1::DefaultNames)
                .map_err(|err| effortless_repo_snapshot::SnapshotError::new(err.to_string()))
        })
        .unwrap_or_else(|err| std::panic::panic_any(format!("second: {err}")));
    assert_eq!(first.analysis_identity, second.analysis_identity);
    assert_eq!(first.population.inspected.len(), 3);
    // Sorted regardless of Git's traversal order.
    let paths: Vec<&str> = first
        .population
        .inspected
        .iter()
        .map(|entry| entry.repo_path.as_str())
        .collect();
    assert_eq!(
        paths,
        vec![
            ".changes/A-second.yaml",
            ".changes/B-first.yaml",
            ".changes/Fixture.yaml"
        ]
    );
}

#[test]
fn adapter_does_not_reimplement_fragment_rules() {
    // Falsifier 10: the adapter delegates semantics — an invalid
    // fragment produces the sensor's rule identities, unchanged.
    let repo = committed("delegate", CONFIG, "kind: Added\nbody: x\n");
    let view = RepositorySourceView::filesystem(&repo.root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("filesystem view: {err}")));
    let result = analyze_source_view(&view, &ChangieConfigSelectionV1::DefaultNames)
        .unwrap_or_else(|err| std::panic::panic_any(format!("analyze: {err}")));
    assert!(
        result
            .report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule.as_str() == "changie.fragment.kind_unknown")
    );
}

#[test]
fn yml_only_view_selects_yml() {
    let repo = FixtureRepo::new("yml-only");
    repo.write(".changie.yml", CONFIG);
    repo.write(".changes/Fixture.yaml", FRAGMENT);
    repo.git(&["add", "--all"]);
    let view = RepositorySourceView::filesystem(&repo.root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("filesystem view: {err}")));
    let result = analyze_source_view(&view, &ChangieConfigSelectionV1::DefaultNames)
        .unwrap_or_else(|err| std::panic::panic_any(format!("analyze: {err}")));
    assert_eq!(result.config_selection.selected_path, ".changie.yml");
    assert!(!result.config_selection.ambiguous);
}

#[test]
fn nested_and_wrong_extension_entries_stay_reported() {
    let repo = committed("nested-entries", CONFIG, FRAGMENT);
    repo.write(".changes/Nested/deep.yaml", FRAGMENT);
    repo.write(".changes/Wrong.yml", FRAGMENT);
    repo.git(&["add", "--all"]);
    repo.git(&["commit", "-m", "nested and wrong extension"]);
    let view = RepositorySourceView::committed(&repo.root, "HEAD")
        .unwrap_or_else(|err| std::panic::panic_any(format!("committed view: {err}")));
    let result = analyze_source_view(&view, &ChangieConfigSelectionV1::DefaultNames)
        .unwrap_or_else(|err| std::panic::panic_any(format!("analyze: {err}")));
    // All three inspected; classification (nested, .yml) is the
    // sensor's, with rule-identified findings.
    assert_eq!(result.population.inspected.len(), 3);
    assert!(
        result.report.diagnostics.iter().any(|diagnostic| {
            diagnostic.rule.as_str() == "changie.fragment.path_not_discovered"
        })
    );
}
