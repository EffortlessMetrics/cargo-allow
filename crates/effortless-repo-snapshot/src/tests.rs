use crate::parity::{ParityContract, load_parity_contract};
use crate::protocol_adapter::repository_snapshot_v1_from_allow_diff;
use crate::{
    RepositorySnapshotRequest, StagedPathRead, read_staged_path, repository_snapshot,
    staged_repository_snapshot,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn source_tree_ignore_patterns_match_without_unchecked_slicing() -> Result<(), String> {
    let patterns = vec!["src/**".to_string(), "generated/*".to_string()];
    let cases = [
        ("src/lib.rs", true),
        ("generated/schema.rs", true),
        ("other/src/lib.rs", false),
        ("README.md", false),
    ];
    for (path, expected) in cases {
        if crate::error::source_tree_path_is_ignored(path, &patterns) != expected {
            return Err(format!("unexpected ignore result for {path}"));
        }
    }
    Ok(())
}

#[test]
fn parity_contracts_load_from_fixtures() -> Result<(), String> {
    let root = workspace_root();
    for path in crate::parity::revision_parity_contract_paths(&root) {
        let contract = load_parity_contract(&path)?;
        if contract.scenario_id.is_empty() {
            return Err(format!("empty scenario in {}", path.display()));
        }
    }
    Ok(())
}

#[test]
fn revision_identity_parity_over_allow_diff() -> Result<(), String> {
    let root = workspace_root();
    let contract_path = root.join("tests/fixtures/repo-snapshot/parity-committed-head-v1.toml");
    let contract = load_parity_contract(&contract_path)?;

    let repo = init_git_repo("revision-parity")?;
    write_file(&repo, "src/lib.rs", "pub fn parity() {}\n")?;
    commit_all(&repo, "parity seed")?;

    let request = RepositorySnapshotRequest::committed_head("HEAD")
        .with_selected_paths([PathBuf::from("src/lib.rs")]);
    let identity = repository_snapshot(&repo, &request)
        .map_err(|err| format!("allow-diff repository_snapshot: {err}"))?;
    let transport = repository_snapshot_v1_from_allow_diff(&identity);
    validate_transport_contract(&transport, &contract)?;
    assert_eq!(
        contract.repo_snapshot_module,
        crate::revision_identity::RevisionIdentitySurface::MODULE_ID
    );
    Ok(())
}

#[test]
fn staged_index_parity_over_allow_diff() -> Result<(), String> {
    let root = workspace_root();
    let contract_path = root.join("tests/fixtures/repo-snapshot/parity-staged-index-v1.toml");
    let contract = load_staged_contract(&contract_path)?;

    let repo = init_git_repo("staged-parity")?;
    write_file(&repo, "src/lib.rs", "pub fn staged() {}\n")?;
    commit_all(&repo, "staged seed")?;
    write_file(&repo, "src/lib.rs", "pub fn staged() { /* staged */ }\n")?;
    git(&repo, &["add", "src/lib.rs"])?;

    let snapshot = staged_repository_snapshot(&repo)
        .map_err(|err| format!("allow-diff staged_repository_snapshot: {err}"))?;
    validate_staged_contract(&snapshot, &contract)?;
    assert_eq!(
        contract.repo_snapshot_module,
        crate::staged_index::StagedIndexSurface::MODULE_ID
    );
    Ok(())
}

#[test]
fn staged_deletion_negative_fixture_ignores_dirty_replacement() -> Result<(), String> {
    let root = workspace_root();
    let contract_path = crate::parity::staged_deletion_parity_contract_path(&root);
    let contract = load_staged_deletion_contract(&contract_path)?;

    let repo = init_git_repo("staged-deletion-negative")?;
    write_file(&repo, &contract.staged_path, "committed\n")?;
    commit_all(&repo, "seed")?;
    git(&repo, &["rm", "--", &contract.staged_path])?;
    write_file(&repo, &contract.staged_path, "dirty replacement\n")?;

    let snapshot = staged_repository_snapshot(&repo)
        .map_err(|err| format!("allow-diff staged_repository_snapshot: {err}"))?;
    let read = read_staged_path(&snapshot, Path::new(&contract.staged_path))
        .map_err(|err| format!("allow-diff read_staged_path: {err}"))?;

    if contract.expected_read != "missing" {
        return Err(format!(
            "fixture {} has unsupported expected_read",
            contract.scenario_id
        ));
    }
    if read != StagedPathRead::Missing {
        return Err(
            "staged deletion must not fall back to dirty worktree replacement bytes".to_string(),
        );
    }
    if !contract.forbid_worktree_fallback {
        return Err("negative fixture must forbid worktree fallback".to_string());
    }
    Ok(())
}

#[test]
fn source_view_staged_parity_fixture() -> Result<(), String> {
    let root = workspace_root();
    let contract_path = crate::parity::source_view_parity_contract_path(&root);
    let contract = load_source_view_contract(&contract_path)?;

    let repo = init_git_repo("source-view-parity")?;
    write_file(&repo, &contract.staged_path, &contract.indexed_bytes)?;
    git(&repo, &["add", &contract.staged_path])?;
    write_file(&repo, &contract.staged_path, &contract.worktree_bytes)?;

    let view = crate::RepositorySourceView::staged(&repo)
        .map_err(|err| format!("repo-snapshot staged view: {err}"))?;
    let read = view
        .read_text(Path::new(&contract.staged_path))
        .map_err(|err| format!("repo-snapshot read_text: {err}"))?;
    if read != contract.indexed_bytes {
        return Err("staged source view must read indexed bytes, not worktree".to_string());
    }
    if !contract.forbid_worktree_fallback {
        return Err("source-view parity fixture must forbid worktree fallback".to_string());
    }
    Ok(())
}

#[test]
fn source_view_package_copy_matches_repo_snapshot() -> Result<(), String> {
    let root = workspace_root();
    let canonical =
        std::fs::read_to_string(root.join("crates/effortless-repo-snapshot/src/source_view.rs"))
            .map_err(|err| format!("read canonical source_view: {err}"))?;
    let packaged =
        std::fs::read_to_string(root.join("crates/cargo-allow/src/spec_system_source_view.rs"))
            .map_err(|err| format!("read packaged source_view copy: {err}"))?;
    let canonical_body = canonical
        .split_once("type RustSourceInputs")
        .map(|(_, body)| body)
        .ok_or_else(|| "canonical source_view missing body marker".to_string())?;
    let packaged_body = packaged
        .split_once("type RustSourceInputs")
        .map(|(_, body)| body)
        .ok_or_else(|| "packaged source_view missing body marker".to_string())?;
    if canonical_body.replace("\r\n", "\n") != packaged_body.replace("\r\n", "\n") {
        return Err(
            "cargo-allow spec_system_source_view.rs must match effortless-repo-snapshot source_view.rs (modulo import paths)"
                .to_string(),
        );
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
struct SourceViewParityContract {
    scenario_id: String,
    allow_diff_module: String,
    repo_snapshot_module: String,
    parity_case: String,
    move_ledger_entry: String,
    staged_path: String,
    indexed_bytes: String,
    worktree_bytes: String,
    forbid_worktree_fallback: bool,
}

fn load_source_view_contract(path: &Path) -> Result<SourceViewParityContract, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    toml::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
struct StagedDeletionParityContract {
    scenario_id: String,
    allow_diff_module: String,
    repo_snapshot_module: String,
    parity_case: String,
    move_ledger_entry: String,
    negative_case: bool,
    staged_path: String,
    expected_read: String,
    forbid_worktree_fallback: bool,
}

fn load_staged_deletion_contract(path: &Path) -> Result<StagedDeletionParityContract, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    toml::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
struct StagedParityContract {
    scenario_id: String,
    allow_diff_module: String,
    repo_snapshot_module: String,
    parity_case: String,
    move_ledger_entry: String,
    required_staged_fields: Vec<String>,
}

fn load_staged_contract(path: &Path) -> Result<StagedParityContract, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    toml::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
}

fn validate_staged_contract(
    snapshot: &crate::StagedRepositorySnapshot,
    contract: &StagedParityContract,
) -> Result<(), String> {
    for field in &contract.required_staged_fields {
        let present = match field.as_str() {
            "identity.semantic_hash" => !snapshot.identity.semantic_hash.is_empty(),
            "parent_commit" => snapshot.parent_commit.is_some(),
            other => return Err(format!("unknown staged parity field `{other}`")),
        };
        if !present {
            return Err(format!(
                "staged parity `{}` missing required field `{field}`",
                contract.scenario_id
            ));
        }
    }
    Ok(())
}

fn validate_transport_contract(
    transport: &effortless_repo_protocol::RepositorySnapshotV1,
    contract: &ParityContract,
) -> Result<(), String> {
    for field in &contract.required_transport_fields {
        if !transport_field_present(transport, field) {
            return Err(format!(
                "parity contract `{}` missing required transport field `{field}`",
                contract.scenario_id
            ));
        }
    }
    Ok(())
}

fn transport_field_present(
    transport: &effortless_repo_protocol::RepositorySnapshotV1,
    field: &str,
) -> bool {
    match field {
        "schema_id" => !transport.schema_id.is_empty(),
        "kind" => true,
        "root_identity" => !transport.root_identity.is_empty(),
        "head.commit" => !transport.head.commit.is_empty(),
        "head.tree" => !transport.head.tree.is_empty(),
        "selected_source_closure" => !transport.selected_source_closure.is_empty(),
        "dirty_state" => !transport.dirty_state.is_empty(),
        other => !other.is_empty(),
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn init_git_repo(label: &str) -> Result<PathBuf, String> {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|err| format!("system clock: {err}"))?
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "repo-snapshot-parity-{label}-{}-{unique}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).map_err(|err| format!("temp root: {err}"))?;
    git(&root, &["init", "-q"])?;
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    )?;
    git(&root, &["config", "user.name", "cargo-allow test"])?;
    Ok(root)
}

fn write_file(root: &Path, rel: &str, contents: &str) -> Result<(), String> {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("mkdir: {err}"))?;
    }
    fs::write(&path, contents).map_err(|err| format!("write {rel}: {err}"))?;
    Ok(())
}

fn commit_all(root: &Path, message: &str) -> Result<(), String> {
    git(root, &["add", "-A"])?;
    git(root, &["commit", "-q", "-m", message])
}

fn git(root: &Path, args: &[&str]) -> Result<(), String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|err| format!("git {args:?}: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}
