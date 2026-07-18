use super::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn temp_root(label: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_else(|err| std::panic::panic_any(format!("system clock: {err}")))
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-snapshot-{label}-{}-{unique}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("temp root: {err}")));
    root
}

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("git {args:?}: {err}")));
    if !output.status.success() {
        std::panic::panic_any(format!(
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
}

fn init_repo(label: &str) -> PathBuf {
    let root = temp_root(label);
    git(&root, &["init", "-q"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    root
}

fn write(root: &Path, rel: &str, contents: &str) {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .unwrap_or_else(|err| std::panic::panic_any(format!("mkdir: {err}")));
    }
    fs::write(&path, contents)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write {rel}: {err}")));
}

fn commit(root: &Path, message: &str) -> String {
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "--allow-empty", "-m", message]);
    head_commit(root)
}

fn head_commit(root: &Path) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("rev-parse: {err}")));
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn branch_exists(root: &Path, name: &str) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--verify", "--quiet", name])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

// --- committed identity -------------------------------------------------

#[test]
fn repository_snapshot_identity_binds_head_tree_and_selected_closure() {
    let root = init_repo("head-identity");
    write(&root, "src/lib.rs", "pub fn a() {}\n");
    let head = commit(&root, "one");

    let request = RepositorySnapshotRequest::committed_head("HEAD")
        .with_selected_paths([PathBuf::from("src/lib.rs")]);
    let snapshot = repository_snapshot(&root, &request)
        .unwrap_or_else(|err| std::panic::panic_any(format!("snapshot: {err}")));

    assert_eq!(snapshot.schema, REPOSITORY_SNAPSHOT_SCHEMA);
    assert_eq!(snapshot.kind, RepositorySnapshotKind::CommittedHead);
    assert_eq!(snapshot.head.commit, head);
    assert!(!snapshot.head.tree.is_empty());
    assert!(snapshot.base.is_none());
    assert!(snapshot.merge_base.is_none());
    assert_eq!(snapshot.dirty_state, RepositoryDirtyState::Clean);
    assert_eq!(snapshot.selected_paths.len(), 1);
    assert!(snapshot.selected_paths[0].present);
    assert!(snapshot.selected_paths[0].blob_oid.is_some());
    assert!(snapshot.root_identity.starts_with("sha256:v1:"));
    assert!(snapshot.selected_source_closure.starts_with("sha256:v1:"));
    // No absolute checkout path leaks into portable identity.
    let portable = format!("{snapshot:?}");
    assert!(!portable.contains(&root.display().to_string()));
}

#[test]
fn repository_snapshot_identity_resolves_range_base_and_merge_base() {
    let root = init_repo("range-identity");
    write(&root, "a.txt", "1\n");
    let base = commit(&root, "base");
    write(&root, "b.txt", "2\n");
    let head = commit(&root, "head");

    let request = RepositorySnapshotRequest::committed_range(&base, "HEAD");
    let snapshot = repository_snapshot(&root, &request)
        .unwrap_or_else(|err| std::panic::panic_any(format!("range snapshot: {err}")));

    assert_eq!(snapshot.kind, RepositorySnapshotKind::CommittedRange);
    assert_eq!(snapshot.head.commit, head);
    assert_eq!(
        snapshot.base.as_ref().map(|base| base.commit.clone()),
        Some(base.clone())
    );
    // Linear history: the base is the merge base.
    assert_eq!(snapshot.merge_base, Some(base));
}

#[test]
fn repository_snapshot_identity_reports_true_merge_base() {
    let root = init_repo("merge-base");
    write(&root, "a.txt", "root\n");
    let fork = commit(&root, "fork point");
    git(&root, &["checkout", "-q", "-b", "feature"]);
    write(&root, "feature.txt", "f\n");
    let feature = commit(&root, "feature work");
    let primary = if branch_exists(&root, "main") {
        "main"
    } else {
        "master"
    };
    git(&root, &["checkout", "-q", primary]);
    write(&root, "main.txt", "m\n");
    commit(&root, "main work");

    let request = RepositorySnapshotRequest::committed_range(&feature, "HEAD");
    let snapshot = repository_snapshot(&root, &request)
        .unwrap_or_else(|err| std::panic::panic_any(format!("merge-base snapshot: {err}")));
    assert_eq!(snapshot.merge_base, Some(fork));
}

#[test]
fn repository_snapshot_identity_rejects_missing_revision() {
    let root = init_repo("missing-rev");
    write(&root, "a.txt", "1\n");
    commit(&root, "one");

    let request = RepositorySnapshotRequest::committed_head("does-not-exist");
    assert!(repository_snapshot(&root, &request).is_err());

    let range = RepositorySnapshotRequest::committed_range("deadbeefdeadbeef", "HEAD");
    assert!(repository_snapshot(&root, &range).is_err());
}

#[test]
fn repository_snapshot_identity_missing_range_base_is_rejected() {
    let root = init_repo("no-base");
    write(&root, "a.txt", "1\n");
    commit(&root, "one");
    let mut request = RepositorySnapshotRequest::committed_head("HEAD");
    request.kind = RepositorySnapshotKind::CommittedRange;
    request.base = None;
    assert!(repository_snapshot(&root, &request).is_err());
}

// --- staleness / closure stability -------------------------------------

#[test]
fn repository_snapshot_identity_stable_closure_across_unrelated_commit() {
    let root = init_repo("stable-closure");
    write(&root, "kept.rs", "pub fn kept() {}\n");
    write(&root, "other.rs", "pub fn other() {}\n");
    commit(&root, "one");

    let request = RepositorySnapshotRequest::committed_head("HEAD")
        .with_selected_paths([PathBuf::from("kept.rs")]);
    let first = repository_snapshot(&root, &request)
        .unwrap_or_else(|err| std::panic::panic_any(format!("first: {err}")));

    // A new commit that does not touch the selected file.
    write(&root, "other.rs", "pub fn other() { let _ = 1; }\n");
    commit(&root, "two");
    let second = repository_snapshot(&root, &request)
        .unwrap_or_else(|err| std::panic::panic_any(format!("second: {err}")));

    // The head (and thus the overall identity) advances...
    assert_ne!(first.head.commit, second.head.commit);
    assert_ne!(first, second);
    // ...but the selected-source closure is unchanged because the selected file
    // is byte-identical. Tree-equivalence reuse remains higher-level policy.
    assert_eq!(
        first.selected_source_closure,
        second.selected_source_closure
    );
    assert_eq!(
        first.selected_paths[0].blob_oid,
        second.selected_paths[0].blob_oid
    );
}

#[test]
fn repository_snapshot_identity_closure_changes_when_selected_file_changes() {
    let root = init_repo("changed-closure");
    write(&root, "kept.rs", "pub fn kept() {}\n");
    commit(&root, "one");
    let request = RepositorySnapshotRequest::committed_head("HEAD")
        .with_selected_paths([PathBuf::from("kept.rs")]);
    let first = repository_snapshot(&root, &request)
        .unwrap_or_else(|err| std::panic::panic_any(format!("first: {err}")));

    write(&root, "kept.rs", "pub fn kept() { let _ = 2; }\n");
    commit(&root, "two");
    let second = repository_snapshot(&root, &request)
        .unwrap_or_else(|err| std::panic::panic_any(format!("second: {err}")));

    assert_ne!(
        first.selected_source_closure,
        second.selected_source_closure
    );
    assert_ne!(
        first.selected_paths[0].blob_oid,
        second.selected_paths[0].blob_oid
    );
}

#[test]
fn repository_snapshot_identity_records_absent_selected_path() {
    let root = init_repo("absent-path");
    write(&root, "present.rs", "pub fn present() {}\n");
    commit(&root, "one");
    let request = RepositorySnapshotRequest::committed_head("HEAD")
        .with_selected_paths([PathBuf::from("present.rs"), PathBuf::from("gone.rs")]);
    let snapshot = repository_snapshot(&root, &request)
        .unwrap_or_else(|err| std::panic::panic_any(format!("snapshot: {err}")));
    // Sorted by path: gone.rs then present.rs.
    assert_eq!(snapshot.selected_paths[0].path, "gone.rs");
    assert!(!snapshot.selected_paths[0].present);
    assert!(snapshot.selected_paths[0].blob_oid.is_none());
    assert_eq!(snapshot.selected_paths[1].path, "present.rs");
    assert!(snapshot.selected_paths[1].present);
}

#[test]
fn repository_snapshot_identity_selected_path_with_spaces_is_normalized() {
    let root = init_repo("spaced-path");
    write(&root, "dir with space/file name.rs", "pub fn x() {}\n");
    commit(&root, "one");
    let request = RepositorySnapshotRequest::committed_head("HEAD")
        .with_selected_paths([PathBuf::from("dir with space/file name.rs")]);
    let snapshot = repository_snapshot(&root, &request)
        .unwrap_or_else(|err| std::panic::panic_any(format!("snapshot: {err}")));
    assert_eq!(
        snapshot.selected_paths[0].path,
        "dir with space/file name.rs"
    );
    assert!(snapshot.selected_paths[0].present);
}

// --- object format ------------------------------------------------------

#[test]
fn repository_snapshot_identity_reports_object_format() {
    let root = init_repo("object-format");
    write(&root, "a.txt", "1\n");
    commit(&root, "one");
    let format = repository_object_format(&root);
    assert!(matches!(
        format,
        RepositoryObjectFormat::Sha1 | RepositoryObjectFormat::Sha256
    ));
    assert_eq!(
        repository_object_format(temp_root("not-git")),
        RepositoryObjectFormat::Unknown
    );
}

// --- determinism across checkout roots ---------------------------------

#[test]
fn repository_snapshot_identity_is_stable_across_checkout_roots() {
    let root = init_repo("checkout-a");
    write(&root, "src/lib.rs", "pub fn a() {}\n");
    commit(&root, "one");

    let clone_root = temp_root("checkout-b");
    let _ = fs::remove_dir_all(&clone_root);
    let status = Command::new("git")
        .args(["clone", "-q"])
        .arg(&root)
        .arg(&clone_root)
        .status()
        .unwrap_or_else(|err| std::panic::panic_any(format!("clone: {err}")));
    assert!(status.success(), "git clone should succeed");

    let request = RepositorySnapshotRequest::committed_head("HEAD")
        .with_selected_paths([PathBuf::from("src/lib.rs")]);
    let original = repository_snapshot(&root, &request)
        .unwrap_or_else(|err| std::panic::panic_any(format!("original: {err}")));
    let cloned = repository_snapshot(&clone_root, &request)
        .unwrap_or_else(|err| std::panic::panic_any(format!("cloned: {err}")));

    // Same commit/tree and selected closure across two different checkout roots
    // yield the same semantic identity.
    assert_eq!(original, cloned);
}

// --- dirty-state contract ----------------------------------------------

#[test]
fn repository_snapshot_dirty_state_clean_committed_tree() {
    let root = init_repo("dirty-clean");
    write(&root, "a.txt", "1\n");
    commit(&root, "one");
    assert_eq!(resolve_dirty_state(&root), RepositoryDirtyState::Clean);
    assert!(resolve_dirty_state(&root).is_clean());
}

#[test]
fn repository_snapshot_dirty_state_tracked_modified() {
    let root = init_repo("dirty-modified");
    write(&root, "a.txt", "1\n");
    commit(&root, "one");
    write(&root, "a.txt", "changed\n");
    assert_eq!(
        resolve_dirty_state(&root),
        RepositoryDirtyState::TrackedModified
    );
}

#[test]
fn repository_snapshot_dirty_state_staged_changes() {
    let root = init_repo("dirty-staged");
    write(&root, "a.txt", "1\n");
    commit(&root, "one");
    write(&root, "a.txt", "staged\n");
    git(&root, &["add", "a.txt"]);
    assert_eq!(
        resolve_dirty_state(&root),
        RepositoryDirtyState::StagedChanges
    );
}

#[test]
fn repository_snapshot_dirty_state_untracked_present() {
    let root = init_repo("dirty-untracked");
    write(&root, "a.txt", "1\n");
    commit(&root, "one");
    write(&root, "new.txt", "untracked\n");
    assert_eq!(
        resolve_dirty_state(&root),
        RepositoryDirtyState::UntrackedPresent
    );
}

#[test]
fn repository_snapshot_dirty_state_non_git_directory() {
    let root = temp_root("dirty-non-git");
    assert_eq!(
        resolve_dirty_state(&root),
        RepositoryDirtyState::NotAGitRepository
    );
}

#[test]
fn repository_snapshot_probe_reflects_dirty_state_when_requested() {
    let root = init_repo("snapshot-dirty");
    write(&root, "a.txt", "1\n");
    commit(&root, "one");
    write(&root, "a.txt", "dirty\n");

    let probed = repository_snapshot(
        &root,
        &RepositorySnapshotRequest::committed_head("HEAD").with_dirty_state_probe(true),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("probed: {err}")));
    assert_eq!(probed.dirty_state, RepositoryDirtyState::TrackedModified);
    assert!(!probed.dirty_state.is_clean());

    // Without the probe, dirty state is reported clean and flagged as not probed.
    let unprobed = repository_snapshot(&root, &RepositorySnapshotRequest::committed_head("HEAD"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("unprobed: {err}")));
    assert_eq!(unprobed.dirty_state, RepositoryDirtyState::Clean);
    assert!(
        unprobed
            .limitations
            .iter()
            .any(|note| note == "dirty_state_not_probed")
    );
}
