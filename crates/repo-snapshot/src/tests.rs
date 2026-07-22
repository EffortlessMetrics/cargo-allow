use crate::parity::{ParityContract, load_parity_contract};
use crate::protocol_adapter::repository_snapshot_v1_from_allow_diff;
use allow_diff::{RepositorySnapshotRequest, repository_snapshot, staged_repository_snapshot};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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
    snapshot: &allow_diff::StagedRepositorySnapshot,
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
    transport: &repo_protocol::RepositorySnapshotV1,
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

fn transport_field_present(transport: &repo_protocol::RepositorySnapshotV1, field: &str) -> bool {
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
