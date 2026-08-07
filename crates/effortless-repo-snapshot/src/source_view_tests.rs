use crate::RepositorySourceView;
use allow_inventory::InventorySource;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn test_repo(label: &str) -> Result<PathBuf, String> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "repo-snapshot-source-view-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    git(&root, &["init", "-q"])?;
    git(&root, &["config", "user.name", "Cargo Allow"])?;
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    )?;
    Ok(root)
}

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        String::from_utf8(output.stdout).map_err(|error| error.to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
}

#[test]
fn staged_view_reads_indexed_bytes_and_inventory() -> Result<(), String> {
    let root = test_repo("indexed")?;
    fs::write(root.join("value.txt"), "staged\n").map_err(|error| error.to_string())?;
    git(&root, &["add", "value.txt"])?;
    fs::write(root.join("value.txt"), "worktree\n").map_err(|error| error.to_string())?;

    let view = RepositorySourceView::staged(&root).map_err(|error| error.to_string())?;
    assert_eq!(
        view.inventory().source,
        InventorySource::GitIndexStagedCandidate
    );
    let staged_text = view
        .read_text(Path::new("value.txt"))
        .map_err(|error| error.to_string())?;
    assert_eq!(staged_text, "staged\n");
    assert!(view.source_identity().is_some());
    let _ = fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn staged_view_does_not_fall_back_for_deleted_candidate_path() -> Result<(), String> {
    let root = test_repo("deleted")?;
    fs::write(root.join("value.txt"), "base\n").map_err(|error| error.to_string())?;
    git(&root, &["add", "value.txt"])?;
    git(&root, &["commit", "-qm", "base"])?;
    git(&root, &["rm", "-q", "value.txt"])?;
    fs::write(root.join("value.txt"), "dirty worktree\n").map_err(|error| error.to_string())?;

    let view = RepositorySourceView::staged(&root).map_err(|error| error.to_string())?;
    let error = view
        .read_text(Path::new("value.txt"))
        .err()
        .ok_or_else(|| "deleted staged path unexpectedly read from worktree".to_string())?;
    if !error.to_string().contains("absent from the candidate") {
        return Err("unexpected error for deleted staged path".to_string());
    }
    let _ = fs::remove_dir_all(root);
    Ok(())
}
