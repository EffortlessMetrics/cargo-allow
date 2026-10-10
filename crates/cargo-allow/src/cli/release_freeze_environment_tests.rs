//! Exercise the real freeze acquisition boundary with child-local Git overrides.

use super::compose_fixture_tests::{committed_subject_fixture, fixture_git};
use crate::repository_environment::{isolate_repository, require_isolated_fixture_test};
use allow_core::CargoAllowErrorKind;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

type TestResult = Result<(), Box<dyn std::error::Error>>;

const CHILD_CASE: &str = "CARGO_ALLOW_FREEZE_TEST_CASE";
const SELECTED_ROOT: &str = "CARGO_ALLOW_FREEZE_TEST_ROOT";
const EXPECTED_HEAD: &str = "CARGO_ALLOW_FREEZE_TEST_HEAD";
const EXPECTED_TREE: &str = "CARGO_ALLOW_FREEZE_TEST_TREE";
const CHILD_TEST: &str =
    "cli::release_freeze_command::environment_tests::subject_and_discovery_use_selected_repository";

struct Fixture(PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn child_probe(case: &str) -> TestResult {
    let root = PathBuf::from(std::env::var_os(SELECTED_ROOT).ok_or("missing selected root")?);
    if case == "discovery" {
        let discovered = super::git_root()?.canonicalize()?;
        if discovered != root.canonicalize()? {
            return Err(format!("cwd selected {root:?}, discovery returned {discovered:?}").into());
        }
        return Ok(());
    }
    if case == "outside" {
        let out_dir = std::env::current_dir()?.join("unexpected-freeze-output");
        let args = super::ReleaseFreezeArgs {
            command: super::ReleaseFreezeSubcommand::Compose(super::ReleaseFreezeComposeArgs {
                version: "0.2.0".to_string(),
                evidence: Vec::new(),
                out_dir: out_dir.clone(),
            }),
        };
        let error = super::cmd_release_freeze(&args)
            .err()
            .ok_or("outside-cwd discovery accepted a foreign repository")?;
        if error.kind() != CargoAllowErrorKind::InvalidConfig
            || !error
                .to_string()
                .contains("release-freeze requires a git worktree")
            || out_dir.exists()
        {
            return Err(format!("outside-cwd rejection changed: {error}").into());
        }
        return Ok(());
    }

    if matches!(case, "untracked-local" | "untracked-injected") {
        let configured_status = fixture_git(&root, &["status", "--porcelain"])?;
        if !configured_status.trim().is_empty() || !root.join("src/untracked.rs").is_file() {
            return Err("the selected config did not hide the untracked source control".into());
        }
    }
    let result = super::SubjectIdentity::collect(
        &mut super::FilesystemSubjectInputs { root: &root },
        "0.2.0",
    );
    if case == "clean" || case == "clean-injected" {
        let subject = result?;
        if subject.commit != std::env::var(EXPECTED_HEAD)?
            || subject.tree != std::env::var(EXPECTED_TREE)?
        {
            return Err("the collector used a foreign subject identity".into());
        }
        return Ok(());
    }
    let diagnostic = match case {
        "dirty" | "untracked-local" | "untracked-injected" => "the worktree is dirty",
        "assume-unchanged" | "skip-worktree" => "the index carries hidden state",
        other => return Err(format!("unknown child probe {other:?}").into()),
    };
    let error = result
        .err()
        .ok_or_else(|| format!("the collector accepted selected {case} state"))?;
    if error.kind() != CargoAllowErrorKind::InstrumentFailure
        || !error.to_string().contains(diagnostic)
    {
        return Err(format!("incorrect selected-{case} refusal: {error}").into());
    }
    Ok(())
}

fn require_child_probe(selected: &Path, foreign: &Path, case: &str, cwd: &Path) -> TestResult {
    // These values come from the independent fixture helper, not production acquisition.
    let head = fixture_git(selected, &["rev-parse", "HEAD"])?;
    let tree = fixture_git(selected, &["rev-parse", "HEAD^{tree}"])?;
    let foreign_head = fixture_git(foreign, &["rev-parse", "HEAD"])?;
    let git_dir = foreign.join(".git");
    let index = git_dir.join("index");
    let canary_paths = [
        index.clone(),
        git_dir.join("HEAD"),
        foreign.join("src/subject.rs"),
        foreign.join("Cargo.toml"),
        foreign.join("Cargo.lock"),
        foreign.join("policy/product-package-topology-v2.toml"),
    ];
    let canary_bytes = canary_paths
        .iter()
        .map(fs::read)
        .collect::<Result<Vec<_>, _>>()?;
    for index_only in [true, false] {
        let mut command = Command::new(std::env::current_exe()?);
        isolate_repository(&mut command);
        command
            .args(["--exact", CHILD_TEST, "--nocapture", "--color", "never"])
            .current_dir(cwd)
            .env(CHILD_CASE, case)
            .env(SELECTED_ROOT, selected)
            .env(EXPECTED_HEAD, head.trim())
            .env(EXPECTED_TREE, tree.trim())
            .env("GIT_INDEX_FILE", &index);
        if case == "untracked-injected" || case == "clean-injected" {
            command
                .env("GIT_CONFIG_COUNT", "1")
                .env("GIT_CONFIG_KEY_0", "status.showUntrackedFiles")
                .env("GIT_CONFIG_VALUE_0", "no");
        }
        if !index_only {
            command
                .env("GIT_DIR", &git_dir)
                .env("GIT_WORK_TREE", foreign)
                .env("GIT_COMMON_DIR", &git_dir)
                .env("GIT_OBJECT_DIRECTORY", git_dir.join("objects"))
                .env("GIT_ALTERNATE_OBJECT_DIRECTORIES", git_dir.join("objects"));
        }
        let output = command.output()?;
        for (path, before) in canary_paths.iter().zip(&canary_bytes) {
            if fs::read(path)? != *before {
                return Err(format!("{case} child changed foreign canary {path:?}").into());
            }
        }
        if fixture_git(foreign, &["rev-parse", "HEAD"])? != foreign_head {
            return Err(format!("{case} child changed the foreign HEAD commit").into());
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !output.status.success() || !stdout.contains(&format!("test {CHILD_TEST} ... ok")) {
            return Err(format!(
                "{case} child failed (index_only={index_only}): {stdout}\n{}",
                String::from_utf8_lossy(&output.stderr)
            )
            .into());
        }
    }
    Ok(())
}

#[test]
fn subject_and_discovery_use_selected_repository() -> TestResult {
    if let Some(case) = std::env::var_os(CHILD_CASE) {
        return child_probe(case.to_str().ok_or("non-UTF-8 child probe case")?);
    }

    let selected = Fixture(committed_subject_fixture()?);
    let foreign = Fixture(committed_subject_fixture()?);
    for (fixture, marker) in [(&selected, "selected source"), (&foreign, "foreign source")] {
        fs::create_dir(fixture.0.join("src"))?;
        fs::write(fixture.0.join("src/subject.rs"), format!("// {marker}\n"))?;
        fixture_git(&fixture.0, &["add", "src/subject.rs"])?;
        fixture_git(&fixture.0, &["commit", "-m", marker])?;
    }
    for path in [
        super::WORKSPACE_MANIFEST_PATH,
        super::CARGO_LOCK_PATH,
        super::TOPOLOGY_PATH,
    ] {
        if fs::read(selected.0.join(path))? != fs::read(foreign.0.join(path))? {
            return Err(format!("the subject-control inputs differ at {path}").into());
        }
    }
    if fixture_git(&selected.0, &["rev-parse", "HEAD^{tree}"])?
        == fixture_git(&foreign.0, &["rev-parse", "HEAD^{tree}"])?
    {
        return Err("selected and foreign trees must be distinct".into());
    }

    require_child_probe(&selected.0, &foreign.0, "clean", &selected.0)?;
    fs::write(
        selected.0.join("src/subject.rs"),
        b"// uncommitted selected source\n",
    )?;
    require_child_probe(&selected.0, &foreign.0, "dirty", &selected.0)?;
    fs::write(selected.0.join("src/subject.rs"), b"// selected source\n")?;

    // Status configuration may hide a nonignored source without changing HEAD,
    // the index flags, or the three specially checked input files.
    let untracked = selected.0.join("src/untracked.rs");
    fs::write(&untracked, b"// selected untracked source\n")?;
    fixture_git(
        &selected.0,
        &["config", "--local", "status.showUntrackedFiles", "no"],
    )?;
    require_child_probe(&selected.0, &foreign.0, "untracked-local", &selected.0)?;
    fs::remove_file(&untracked)?;
    require_child_probe(&selected.0, &foreign.0, "clean", &selected.0)?;
    fixture_git(
        &selected.0,
        &["config", "--local", "--unset", "status.showUntrackedFiles"],
    )?;
    fs::write(&untracked, b"// selected untracked source\n")?;
    require_child_probe(&selected.0, &foreign.0, "untracked-injected", &selected.0)?;
    fs::remove_file(&untracked)?;
    require_child_probe(&selected.0, &foreign.0, "clean-injected", &selected.0)?;

    // All three input bytes remain identical: only the selected index carries the hold.
    fixture_git(
        &selected.0,
        &["update-index", "--assume-unchanged", "Cargo.lock"],
    )?;
    require_child_probe(&selected.0, &foreign.0, "assume-unchanged", &selected.0)?;
    fixture_git(
        &selected.0,
        &["update-index", "--no-assume-unchanged", "Cargo.lock"],
    )?;
    fixture_git(
        &selected.0,
        &["update-index", "--skip-worktree", super::TOPOLOGY_PATH],
    )?;
    require_child_probe(&selected.0, &foreign.0, "skip-worktree", &selected.0)?;
    fixture_git(
        &selected.0,
        &["update-index", "--no-skip-worktree", super::TOPOLOGY_PATH],
    )?;

    require_child_probe(&selected.0, &foreign.0, "discovery", &selected.0)?;
    let nested = selected.0.join("nested/working-directory");
    fs::create_dir_all(&nested)?;
    require_child_probe(&selected.0, &foreign.0, "discovery", &nested)?;
    let outside_path = selected.0.with_extension("outside");
    fs::create_dir(&outside_path)?;
    let outside = Fixture(outside_path);
    require_child_probe(&selected.0, &foreign.0, "outside", &outside.0)?;
    Ok(())
}

#[test]
fn existing_freeze_fixtures_ignore_repository_environment() -> Result<(), String> {
    for name in [
        "compose_retains_rehearsal_denials_through_graph_readiness_and_replay",
        "collect_accepts_a_genuinely_clean_subject",
        "collect_rejects_an_assume_unchanged_hidden_lock",
        "collect_rejects_a_skip_worktree_hidden_topology",
        "collect_accepts_crlf_checkout_framing_of_committed_content",
        "collect_still_rejects_ordinary_dirty_subjects",
    ] {
        require_isolated_fixture_test(&format!(
            "cli::release_freeze_command::compose_fixture_tests::{name}"
        ))?;
    }
    Ok(())
}
