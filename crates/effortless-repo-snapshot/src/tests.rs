use crate::parity::{ParityContract, load_parity_contract};
use crate::protocol_adapter::repository_snapshot_v1;
use crate::{
    RepositorySnapshotRequest, StagedPathRead, read_staged_path, repository_snapshot,
    staged_repository_snapshot,
};
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
fn revision_identity_transport_contract() -> Result<(), String> {
    let root = workspace_root();
    let contract_path = root.join("tests/fixtures/repo-snapshot/parity-committed-head-v1.toml");
    let contract = load_parity_contract(&contract_path)?;

    let repo = init_git_repo("revision-parity")?;
    write_file(&repo, "src/lib.rs", "pub fn parity() {}\n")?;
    commit_all(&repo, "parity seed")?;

    let request = RepositorySnapshotRequest::committed_head("HEAD")
        .with_selected_paths([PathBuf::from("src/lib.rs")]);
    let identity = repository_snapshot(&repo, &request)
        .map_err(|err| format!("repo-snapshot repository_snapshot: {err}"))?;
    let transport = repository_snapshot_v1(&identity);
    validate_transport_contract(&transport, &contract)?;
    assert_eq!(
        contract.repo_snapshot_module,
        crate::revision_identity::RevisionIdentitySurface::MODULE_ID
    );
    Ok(())
}

#[test]
fn staged_index_contract() -> Result<(), String> {
    let root = workspace_root();
    let contract_path = root.join("tests/fixtures/repo-snapshot/parity-staged-index-v1.toml");
    let contract = load_staged_contract(&contract_path)?;

    let repo = init_git_repo("staged-parity")?;
    write_file(&repo, "src/lib.rs", "pub fn staged() {}\n")?;
    commit_all(&repo, "staged seed")?;
    write_file(&repo, "src/lib.rs", "pub fn staged() { /* staged */ }\n")?;
    git(&repo, &["add", "src/lib.rs"])?;

    let snapshot = staged_repository_snapshot(&repo)
        .map_err(|err| format!("repo-snapshot staged_repository_snapshot: {err}"))?;
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
        .map_err(|err| format!("repo-snapshot staged_repository_snapshot: {err}"))?;
    let read = read_staged_path(&snapshot, Path::new(&contract.staged_path))
        .map_err(|err| format!("repo-snapshot read_staged_path: {err}"))?;

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

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
struct SourceViewParityContract {
    scenario_id: String,
    prior_module: String,
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
    prior_module: String,
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
    prior_module: String,
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

#[test]
fn batched_tree_blob_lookup_matches_per_path_lookups() -> Result<(), String> {
    let root = init_git_repo("batched-tree-blobs")?;
    write_file(&root, "src/present.rs", "pub fn present() {}\n")?;
    write_file(&root, "src/nested/deep.rs", "pub fn deep() {}\n")?;
    fs::create_dir_all(root.join("adir")).map_err(|err| format!("mkdir adir: {err}"))?;
    write_file(&root, "adir/inner.txt", "dir entry\n")?;
    // Enough bulk paths that a 64-path chunk boundary is crossed.
    for index in 0..80 {
        write_file(&root, &format!("bulk/f{index:02}.txt"), "bulk\n")?;
    }
    commit_all(&root, "seed")?;

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("src/present.rs", root.join("link.rs"))
            .map_err(|err| format!("symlink fixture: {err}"))?;
        git(&root, &["add", "link.rs"])?;
        git(&root, &["commit", "-q", "-m", "link"])?;
    }

    let head = crate::git::resolve_commit_oid(&root, "HEAD").map_err(|err| err.to_string())?;

    let mut paths: Vec<PathBuf> = vec![
        PathBuf::from("src/present.rs"),
        PathBuf::from("src/missing.rs"),
        PathBuf::from("src/nested/deep.rs"),
        PathBuf::from("adir"),
        PathBuf::from("bulk/f00.txt"),
        PathBuf::from("bulk/f39.txt"),
        PathBuf::from("bulk/f63-missing.txt"),
    ];
    for index in 0..80 {
        paths.push(PathBuf::from(format!("bulk/f{index:02}.txt")));
    }
    paths.push(PathBuf::from("src/present.rs"));
    #[cfg(unix)]
    paths.push(PathBuf::from("link.rs"));

    let path_refs: Vec<&Path> = paths.iter().map(|path| path.as_path()).collect();
    let batched = crate::git::tree_blob_oids_at_commit(&root, &head, &path_refs)
        .map_err(|err| err.to_string())?;
    if batched.len() != paths.len() {
        return Err(format!(
            "batched lookup returned {} entries for {} paths",
            batched.len(),
            paths.len()
        ));
    }

    for (index, path) in paths.iter().enumerate() {
        let single = crate::git::tree_blob_oid_at_commit(&root, &head, path)
            .map_err(|err| err.to_string())?;
        assert_eq!(
            batched[index], single,
            "batched result must match the per-path lookup for {path:?}"
        );
    }

    assert!(batched[0].is_some(), "tracked regular file resolves");
    assert!(batched[1].is_none(), "absent path stays absent");
    assert!(
        batched[3].is_none(),
        "directory entries are not regular blobs"
    );
    assert!(batched[4].is_some() && batched[5].is_some());
    let duplicate_index = paths
        .iter()
        .rposition(|path| path == Path::new("src/present.rs"))
        .ok_or_else(|| "duplicate fixture path missing from request".to_string())?;
    assert_eq!(batched[0], batched[duplicate_index]);
    #[cfg(unix)]
    {
        let symlink_index = paths
            .iter()
            .position(|path| path == Path::new("link.rs"))
            .ok_or_else(|| "symlink fixture path missing from request".to_string())?;
        assert!(batched[symlink_index].is_none());
    }

    let _ = fs::remove_dir_all(&root);
    Ok(())
}

#[test]
fn batch_planner_enforces_count_bytes_and_long_path_boundaries() -> Result<(), String> {
    let lengths = vec![1usize; 65];
    let first =
        crate::git::tree_blob_batch_end_for_test(&lengths, 0).map_err(|err| err.to_string())?;
    let second =
        crate::git::tree_blob_batch_end_for_test(&lengths, first).map_err(|err| err.to_string())?;
    assert_eq!(first, 64);
    assert_eq!(second, 65);

    let cumulative = vec![4000usize, 4000usize];
    assert_eq!(
        crate::git::tree_blob_batch_end_for_test(&cumulative, 0).map_err(|err| err.to_string())?,
        1
    );
    let single_over_budget = vec![16384usize];
    assert_eq!(
        crate::git::tree_blob_batch_end_for_test(&single_over_budget, 0)
            .map_err(|err| err.to_string())?,
        1
    );
    let encoded_multibyte_and_path_syntax = vec![3usize, 1024usize, 2048usize];
    assert_eq!(
        crate::git::tree_blob_batch_end_for_test(&encoded_multibyte_and_path_syntax, 0)
            .map_err(|err| err.to_string())?,
        3
    );

    let requested = [b"present.rs".to_vec()];
    let requested_refs: Vec<&Vec<u8>> = requested.iter().collect();
    let mut returned = std::collections::HashSet::new();
    crate::git::validate_batch_record_path_for_test(b"present.rs", &requested_refs, &mut returned)
        .map_err(|err| err.to_string())?;
    let duplicate = crate::git::validate_batch_record_path_for_test(
        b"present.rs",
        &requested_refs,
        &mut returned,
    )
    .expect_err("duplicate returned records must fail closed");
    assert!(duplicate.to_string().contains("duplicate path"));
    let unrequested = crate::git::validate_batch_record_path_for_test(
        b"other.rs",
        &requested_refs,
        &mut returned,
    )
    .expect_err("unrequested returned records must fail closed");
    assert!(unrequested.to_string().contains("not requested"));
    Ok(())
}

#[test]
fn batched_tree_blob_lookup_error_paths_match_single_lookup() -> Result<(), String> {
    let root = init_git_repo("batched-tree-blob-errors")?;
    write_file(&root, "src/lib.rs", "pub fn demo() {}\n")?;
    commit_all(&root, "seed")?;
    let head = crate::git::resolve_commit_oid(&root, "HEAD").map_err(|err| err.to_string())?;

    // Absolute host paths are rejected by the same source-tree path
    // validation both entry points share.
    let absolute = root.join("src/lib.rs");
    let batched = crate::git::tree_blob_oids_at_commit(&root, &head, &[Path::new(&absolute)])
        .map(|mut resolved| resolved.pop().flatten());
    let single = crate::git::tree_blob_oid_at_commit(&root, &head, Path::new(&absolute));
    match (&batched, &single) {
        (Err(_), Err(_)) => {}
        _ => return Err("absolute path must be rejected by both lookups".to_string()),
    }

    // An unresolvable revision fails closed for the whole batch.
    let ok_ref = Path::new("src/lib.rs");
    assert!(crate::git::tree_blob_oids_at_commit(&root, "0000deadbeef", &[ok_ref]).is_err());

    // An empty request performs no work and returns no entries.
    let empty =
        crate::git::tree_blob_oids_at_commit(&root, &head, &[]).map_err(|err| err.to_string())?;
    assert!(empty.is_empty());

    let _ = fs::remove_dir_all(&root);
    Ok(())
}
