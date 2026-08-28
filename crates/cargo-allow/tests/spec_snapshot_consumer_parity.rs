//! A cargo-allow-side consumer of the shared repository snapshot identity.
//!
//! The spec walking skeleton's downstream consumers (#2217/#2219/#2220/#2221)
//! must read one shared snapshot identity rather than issuing their own Git
//! interpretation. This proves the identity is consumable from the cargo-allow
//! crate and is deterministic for the same inputs across checkout roots.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use effortless_repo_snapshot::{
    RepositoryDirtyState, RepositorySnapshotKind, RepositorySnapshotRequest, repository_snapshot,
};

fn temp_root(label: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_else(|err| panic!("clock: {err}"))
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-snapshot-consumer-{label}-{}-{unique}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap_or_else(|err| panic!("mkdir: {err}"));
    root
}

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap_or_else(|err| panic!("git {args:?}: {err}"));
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn spec_snapshot_consumer_parity_reads_one_shared_identity() {
    let root = temp_root("parity");
    git(&root, &["init", "-q"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| panic!("mkdir src: {err}"));
    fs::write(root.join("src/lib.rs"), "pub fn a() {}\n")
        .unwrap_or_else(|err| panic!("write: {err}"));
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "one"]);

    let request = RepositorySnapshotRequest::committed_head("HEAD")
        .with_selected_paths([PathBuf::from("src/lib.rs")])
        .with_dirty_state_probe(true);

    // A consumer resolves the identity twice and must get the same object.
    let first = repository_snapshot(&root, &request)
        .unwrap_or_else(|err| panic!("first snapshot: {err}"));
    let second = repository_snapshot(&root, &request)
        .unwrap_or_else(|err| panic!("second snapshot: {err}"));
    assert_eq!(first, second, "snapshot identity must be deterministic");

    assert_eq!(first.kind, RepositorySnapshotKind::CommittedHead);
    assert_eq!(first.dirty_state, RepositoryDirtyState::Clean);
    assert_eq!(first.selected_paths.len(), 1);
    assert!(first.selected_paths[0].present);
    assert!(first.head.commit.len() >= 40);
    assert!(first.selected_source_closure.starts_with("sha256:v1:"));

    // The same repository at a second checkout root yields the same identity.
    let clone_root = temp_root("parity-clone");
    let _ = fs::remove_dir_all(&clone_root);
    let cloned = Command::new("git")
        .args(["clone", "-q"])
        .arg(&root)
        .arg(&clone_root)
        .status()
        .unwrap_or_else(|err| panic!("clone: {err}"));
    assert!(cloned.success(), "git clone should succeed");
    let clone_snapshot = repository_snapshot(&clone_root, &request)
        .unwrap_or_else(|err| panic!("clone snapshot: {err}"));
    assert_eq!(
        first, clone_snapshot,
        "the shared identity must be stable across checkout roots"
    );

    let _ = fs::remove_dir_all(&root);
    let _ = fs::remove_dir_all(&clone_root);
}
