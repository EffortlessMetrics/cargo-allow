use super::*;
use allow_core::source_tree_path_is_ignored;

#[test]
fn ignores_target_paths() {
    let opts = InventoryOptions::default();
    assert!(source_tree_path_is_ignored(
        Path::new("target/debug/x"),
        &opts.ignored
    ));
    assert!(source_tree_path_is_ignored(
        Path::new(".git/config"),
        &opts.ignored
    ));
}

#[test]
fn dot_git_ignore_does_not_swallow_dot_github() {
    let opts = InventoryOptions::default();
    assert!(!source_tree_path_is_ignored(
        Path::new(".github/workflows/ci.yml"),
        &opts.ignored
    ));
    assert!(!source_tree_path_is_ignored(
        Path::new(".gitignore"),
        &opts.ignored
    ));
}

#[test]
fn inventory_defaults_to_git_tracked_and_can_include_untracked() {
    let root = temp_root("include-untracked");
    write_file(root.join("tracked.txt"), "tracked");
    write_file(root.join("untracked.txt"), "untracked");
    run_git(&root, &["init"]);
    run_git(&root, &["add", "tracked.txt"]);

    let tracked = inventory_files(&root, &InventoryOptions::default())
        .unwrap_or_else(|err| std::panic::panic_any(format!("tracked inventory: {err}")));
    let tracked_inventory = inventory(&root, &InventoryOptions::default())
        .unwrap_or_else(|err| std::panic::panic_any(format!("tracked inventory: {err}")));
    let with_untracked = inventory_files(
        &root,
        &InventoryOptions {
            include_untracked: true,
            ..InventoryOptions::default()
        },
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("untracked-inclusive inventory: {err}")));

    assert!(tracked.contains(&PathBuf::from("tracked.txt")));
    assert!(!tracked.contains(&PathBuf::from("untracked.txt")));
    assert_eq!(tracked_inventory.source, InventorySource::GitTracked);
    assert!(with_untracked.contains(&PathBuf::from("tracked.txt")));
    assert!(with_untracked.contains(&PathBuf::from("untracked.txt")));
    remove_dir(&root);
}

#[test]
fn git_ls_files_z_parser_preserves_newlines_inside_paths() {
    let files = super::parse_git_ls_files_z(b"src/lib.rs\0fixtures/line\nbreak.rs\0");

    assert_eq!(
        files,
        vec![
            PathBuf::from("src/lib.rs"),
            PathBuf::from("fixtures/line\nbreak.rs")
        ]
    );
}

#[test]
fn git_tracked_inventory_skips_deleted_worktree_files() {
    let root = temp_root("deleted-tracked-file");
    write_file(root.join("kept.txt"), "kept");
    write_file(root.join("deleted.txt"), "deleted");
    run_git(&root, &["init"]);
    run_git(&root, &["add", "kept.txt", "deleted.txt"]);
    fs::remove_file(root.join("deleted.txt"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("delete tracked file: {err}")));

    let tracked_inventory = inventory(&root, &InventoryOptions::default())
        .unwrap_or_else(|err| std::panic::panic_any(format!("tracked inventory: {err}")));

    assert_eq!(tracked_inventory.source, InventorySource::GitTracked);
    assert!(tracked_inventory.files.contains(&PathBuf::from("kept.txt")));
    assert!(
        !tracked_inventory
            .files
            .contains(&PathBuf::from("deleted.txt"))
    );
    remove_dir(&root);
}

#[test]
fn inventory_reports_filesystem_fallback_source_without_git() {
    let root = temp_root("filesystem-source");
    write_file(root.join("tracked.txt"), "tracked");

    let snapshot = inventory(&root, &InventoryOptions::default())
        .unwrap_or_else(|err| std::panic::panic_any(format!("snapshot inventory: {err}")));

    assert_eq!(snapshot.source, InventorySource::FilesystemFallback);
    assert!(snapshot.files.contains(&PathBuf::from("tracked.txt")));
    remove_dir(&root);
}

#[test]
fn inventory_applies_custom_ignored_globs() {
    let opts = InventoryOptions {
        ignored: vec!["scripts/**".to_string()],
        ..InventoryOptions::default()
    };

    assert!(source_tree_path_is_ignored(
        Path::new("scripts/release.sh"),
        &opts.ignored
    ));
    assert!(!source_tree_path_is_ignored(
        Path::new("tools/release.sh"),
        &opts.ignored
    ));
}

#[test]
fn source_tree_root_uses_nearest_git_root_without_cargo_manifest() {
    let root = temp_root("git-root-no-cargo");
    let nested = root.join("src").join("nested");
    fs::create_dir_all(&nested)
        .unwrap_or_else(|err| std::panic::panic_any(format!("nested dir: {err}")));
    run_git(&root, &["init"]);

    let discovered = discover_source_tree_root(&nested)
        .unwrap_or_else(|err| std::panic::panic_any(format!("discover source tree root: {err}")));

    let canonical = root
        .canonicalize()
        .unwrap_or_else(|err| std::panic::panic_any(format!("canonical root: {err}")));
    assert_eq!(discovered, canonical);
    remove_dir(&root);
}

#[test]
fn source_tree_root_ignores_broken_cargo_manifest() {
    let root = temp_root("broken-cargo");
    let nested = root.join("crates").join("demo");
    fs::create_dir_all(&nested)
        .unwrap_or_else(|err| std::panic::panic_any(format!("nested dir: {err}")));
    write_file(root.join("Cargo.toml"), "this is not toml");
    run_git(&root, &["init"]);

    let discovered = discover_source_tree_root(&nested)
        .unwrap_or_else(|err| std::panic::panic_any(format!("discover source tree root: {err}")));

    let canonical = root
        .canonicalize()
        .unwrap_or_else(|err| std::panic::panic_any(format!("canonical root: {err}")));
    assert_eq!(discovered, canonical);
    remove_dir(&root);
}

#[test]
fn source_tree_root_falls_back_to_start_without_git() {
    let root = temp_root("snapshot-no-git");
    let nested = root.join("snapshot").join("src");
    fs::create_dir_all(&nested)
        .unwrap_or_else(|err| std::panic::panic_any(format!("nested dir: {err}")));

    let discovered = discover_source_tree_root(&nested)
        .unwrap_or_else(|err| std::panic::panic_any(format!("discover source tree root: {err}")));

    let canonical = nested
        .canonicalize()
        .unwrap_or_else(|err| std::panic::panic_any(format!("canonical nested: {err}")));
    assert_eq!(discovered, canonical);
    remove_dir(&root);
}

#[test]
fn explicit_source_tree_root_wins_over_git_ancestor() {
    let root = temp_root("explicit-root");
    let explicit = root.join("snapshot");
    let nested = explicit.join("src");
    fs::create_dir_all(&nested)
        .unwrap_or_else(|err| std::panic::panic_any(format!("nested dir: {err}")));
    run_git(&root, &["init"]);

    let discovered = resolve_source_tree_root(Some(&explicit), &nested)
        .unwrap_or_else(|err| std::panic::panic_any(format!("resolve source tree root: {err}")));

    let canonical = explicit
        .canonicalize()
        .unwrap_or_else(|err| std::panic::panic_any(format!("canonical explicit: {err}")));
    assert_eq!(discovered, canonical);
    remove_dir(&root);
}

#[cfg(unix)]
#[test]
fn recursive_inventory_skips_symlinked_directories() -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-symlink-test-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(root.join("real"))?;
    fs::write(root.join("real/file.txt"), "tracked")?;
    symlink(&root, root.join("real/loop"))?;

    let files = super::recursive_files(&root)?;

    assert_eq!(files, vec![PathBuf::from("real/file.txt")]);
    fs::remove_dir_all(root)?;
    Ok(())
}

fn temp_root(label: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_else(|err| std::panic::panic_any(format!("system clock: {err}")))
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("create temp root: {err}")));
    root
}

fn write_file(path: PathBuf, contents: &str) {
    fs::write(&path, contents)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write {}: {err}", path.display())));
}

fn run_git(root: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .status()
        .unwrap_or_else(|err| std::panic::panic_any(format!("invoke git: {err}")));
    assert!(status.success(), "git command failed: {args:?}");
}

fn remove_dir(path: &Path) {
    fs::remove_dir_all(path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove temp root: {err}")));
}
