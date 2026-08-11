use super::*;
use crate::filesystem::visit_for_test;
use allow_core::source_tree_path_is_ignored;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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
    if !git_available() {
        eprintln!("skipped: git not available");
        return;
    }
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
    assert_eq!(
        tracked_inventory.completeness,
        InventoryCompleteness::Scoped
    );
    assert!(!tracked_inventory.empty_git_tracked);
    assert!(with_untracked.contains(&PathBuf::from("tracked.txt")));
    assert!(with_untracked.contains(&PathBuf::from("untracked.txt")));
    remove_dir(&root);
}

#[test]
fn inventory_without_scope_reports_complete() {
    if !git_available() {
        eprintln!("skipped: git not available");
        return;
    }
    let root = temp_root("complete-inventory");
    write_file(root.join("tracked.txt"), "tracked");
    run_git(&root, &["init"]);
    run_git(&root, &["add", "tracked.txt"]);

    let inventory = inventory(
        &root,
        &InventoryOptions {
            ignored: Vec::new(),
            generated: Vec::new(),
            include_untracked: false,
        },
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("complete inventory: {err}")));

    assert_eq!(inventory.completeness, InventoryCompleteness::Complete);
    remove_dir(&root);
}

#[test]
fn git_tracked_inventory_reports_empty_tracked_set() {
    if !git_available() {
        eprintln!("skipped: git not available");
        return;
    }
    let root = temp_root("empty-git-tracked");
    write_file(root.join("untracked.txt"), "untracked");
    run_git(&root, &["init"]);

    let tracked_inventory = inventory(&root, &InventoryOptions::default())
        .unwrap_or_else(|err| std::panic::panic_any(format!("tracked inventory: {err}")));

    assert_eq!(tracked_inventory.source, InventorySource::GitTracked);
    assert_eq!(
        tracked_inventory.completeness,
        InventoryCompleteness::Scoped
    );
    assert!(tracked_inventory.files.is_empty());
    assert!(tracked_inventory.empty_git_tracked);
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
    if !git_available() {
        eprintln!("skipped: git not available");
        return;
    }
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
    assert_eq!(
        tracked_inventory.completeness,
        InventoryCompleteness::Partial
    );
    assert!(tracked_inventory.files.contains(&PathBuf::from("kept.txt")));
    // The deleted file is excluded from the scanned set...
    assert!(
        !tracked_inventory
            .files
            .contains(&PathBuf::from("deleted.txt"))
    );
    // ...but it must be disclosed as a deleted-tracked inventory gap, not
    // silently dropped (#2048).
    assert_eq!(
        tracked_inventory.deleted_tracked,
        vec![PathBuf::from("deleted.txt")],
        "deleted-tracked files must be reported as an inventory diagnostic"
    );
    remove_dir(&root);
}

#[test]
fn existing_regular_files_call_presence_observer() -> Result<(), Box<dyn std::error::Error>> {
    let root = temp_root("existing-regular-files");
    write_file(root.join("kept.txt"), "kept");
    write_file(root.join("also-kept.txt"), "also kept");
    fs::create_dir_all(root.join("directory"))?;

    let (existing, deleted_tracked, submodule_paths) = existing_regular_files(
        &root,
        vec![
            PathBuf::from("kept.txt"),
            PathBuf::from("missing.txt"),
            PathBuf::from("also-kept.txt"),
            PathBuf::from("directory"),
        ],
    );

    assert_eq!(
        existing,
        vec![PathBuf::from("kept.txt"), PathBuf::from("also-kept.txt")]
    );
    // A missing file is recorded as deleted-tracked, not silently dropped (#2048).
    assert_eq!(deleted_tracked, vec![PathBuf::from("missing.txt")]);
    // A tracked path that is a directory is a submodule candidate (#1846).
    assert_eq!(submodule_paths, vec![PathBuf::from("directory")]);
    remove_dir(&root);
    Ok(())
}

#[test]
fn visit_call_presence_observer() -> Result<(), Box<dyn std::error::Error>> {
    let root = temp_root("visit-source-tree");
    fs::create_dir_all(root.join("src"))?;
    fs::create_dir_all(root.join(".git"))?;
    fs::create_dir_all(root.join("target"))?;
    fs::create_dir_all(root.join("src").join("target"))?;
    fs::create_dir_all(root.join("empty"))?;
    write_file(root.join("README.md"), "readme");
    write_file(root.join("src").join("lib.rs"), "pub fn demo() {}\n");
    write_file(root.join(".git").join("config"), "ignored");
    write_file(root.join("target").join("debug.txt"), "ignored");
    write_file(
        root.join("src").join("target").join("mod.rs"),
        "pub fn target_module() {}\n",
    );
    let mut files = Vec::new();

    visit_for_test(&root, &root, &mut files)?;
    files.sort();

    assert_eq!(
        files,
        vec![
            PathBuf::from("README.md"),
            PathBuf::from("src/lib.rs"),
            PathBuf::from("src/target/mod.rs"),
        ]
    );
    remove_dir(&root);
    Ok(())
}

#[test]
fn nested_target_is_inventoried_in_all_sources() -> Result<(), Box<dyn std::error::Error>> {
    if !git_available() {
        eprintln!("skipped: git not available");
        return Ok(());
    }
    let git_root = temp_root("nested-target-git");
    fs::create_dir_all(git_root.join("src").join("target"))?;
    fs::create_dir_all(git_root.join("target"))?;
    write_file(
        git_root.join("src").join("target").join("mod.rs"),
        "pub fn target_module() {}\n",
    );
    write_file(git_root.join("target").join("debug.txt"), "ignored\n");
    run_git(&git_root, &["init"]);
    run_git(&git_root, &["add", "src/target/mod.rs"]);

    let tracked = inventory(&git_root, &InventoryOptions::default())?;
    let with_untracked = inventory(
        &git_root,
        &InventoryOptions {
            include_untracked: true,
            ..InventoryOptions::default()
        },
    )?;
    let nested = Path::new("src/target/mod.rs");
    assert_eq!(tracked.source, InventorySource::GitTracked);
    assert!(tracked.files.contains(&nested.to_path_buf()));
    assert!(with_untracked.files.contains(&nested.to_path_buf()));
    assert!(!tracked.files.contains(&PathBuf::from("target/debug.txt")));
    assert!(
        !with_untracked
            .files
            .contains(&PathBuf::from("target/debug.txt"))
    );
    remove_dir(&git_root);

    let fallback_root = temp_root("nested-target-fallback");
    fs::create_dir_all(fallback_root.join("src").join("target"))?;
    fs::create_dir_all(fallback_root.join("target"))?;
    write_file(
        fallback_root.join("src").join("target").join("mod.rs"),
        "pub fn target_module() {}\n",
    );
    write_file(fallback_root.join("target").join("debug.txt"), "ignored\n");
    let fallback = inventory(&fallback_root, &InventoryOptions::default())?;
    assert_eq!(fallback.source, InventorySource::FilesystemFallback);
    assert!(fallback.files.contains(&nested.to_path_buf()));
    assert!(!fallback.files.contains(&PathBuf::from("target/debug.txt")));
    remove_dir(&fallback_root);
    Ok(())
}

#[test]
fn visit_return_value_discriminator() -> Result<(), Box<dyn std::error::Error>> {
    let root = temp_root("visit-return");
    fs::create_dir_all(root.join("src"))?;
    write_file(root.join("src").join("lib.rs"), "pub fn demo() {}\n");
    let mut files = Vec::new();

    let result = visit_for_test(&root, &root, &mut files);

    assert!(result.is_ok());
    assert_eq!(files, vec![PathBuf::from("src/lib.rs")]);
    remove_dir(&root);
    Ok(())
}

#[test]
fn recursive_files_reports_missing_root() {
    let root = temp_root("missing-recursive-root");
    let missing = root.join("missing");

    let error = recursive_files(&missing).err().map(|err| err.to_string());

    assert!(
        error
            .as_deref()
            .is_some_and(|text| text.contains("failed to read"))
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
    assert_eq!(snapshot.completeness, InventoryCompleteness::Fallback);
    assert!(snapshot.files.contains(&PathBuf::from("tracked.txt")));
    remove_dir(&root);
}

#[test]
fn include_untracked_fallback_preserves_git_error() {
    let root = temp_root("include-untracked-fallback");
    write_file(root.join("untracked.txt"), "untracked");

    let snapshot = inventory(
        &root,
        &InventoryOptions {
            include_untracked: true,
            ..InventoryOptions::default()
        },
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("snapshot inventory: {err}")));

    assert_eq!(snapshot.source, InventorySource::FilesystemIncludeUntracked);
    assert_eq!(snapshot.completeness, InventoryCompleteness::Fallback);
    assert!(snapshot.files.contains(&PathBuf::from("untracked.txt")));
    assert!(
        snapshot
            .git_error
            .as_deref()
            .is_some_and(|error| !error.is_empty())
    );
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
    if !git_available() {
        eprintln!("skipped: git not available");
        return;
    }
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
fn source_tree_root_accepts_gitfile_worktree_marker_without_cargo_manifest() {
    let root = temp_root("gitfile-root-no-cargo");
    let nested = root.join("src").join("nested");
    fs::create_dir_all(&nested)
        .unwrap_or_else(|err| std::panic::panic_any(format!("nested dir: {err}")));
    write_file(root.join(".git"), "gitdir: ../.git/worktrees/cargo-allow\n");

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
    if !git_available() {
        eprintln!("skipped: git not available");
        return;
    }
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

    let (files, _skipped) = super::recursive_files(&root)?;

    assert_eq!(files, vec![PathBuf::from("real/file.txt")]);
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn recursive_inventory_stops_at_depth_limit() -> Result<(), Box<dyn std::error::Error>> {
    use super::filesystem::{INVENTORY_MAX_DEPTH, visit_for_test_with_depth};

    let root = temp_root("depth-limit");
    let mut deep = root.clone();
    for index in 0..=INVENTORY_MAX_DEPTH {
        deep = deep.join(format!("d{index}"));
    }
    fs::create_dir_all(&deep)?;
    write_file(deep.join("too-deep.txt"), "hidden-by-depth-cap\n");
    write_file(root.join("shallow.txt"), "visible\n");

    let (files, skipped) = super::recursive_files(&root)?;

    assert!(files.contains(&PathBuf::from("shallow.txt")));
    assert!(
        !files
            .iter()
            .any(|path| { path.file_name().is_some_and(|name| name == "too-deep.txt") })
    );
    assert!(
        skipped.iter().any(|path| {
            path.to_string_lossy()
                .contains(".cargo-allow-inventory-depth-limit-")
        }),
        "expected depth-limit diagnostic in skipped={skipped:?}"
    );

    // Depth already beyond the cap skips without walking children.
    let mut out = Vec::new();
    let mut local_skipped = Vec::new();
    visit_for_test_with_depth(
        &root,
        &root,
        INVENTORY_MAX_DEPTH + 1,
        &mut out,
        &mut local_skipped,
    )?;
    assert!(out.is_empty());
    assert_eq!(local_skipped.len(), 1);
    remove_dir(&root);
    Ok(())
}

#[test]
fn recursive_inventory_stops_at_entry_limit() -> Result<(), Box<dyn std::error::Error>> {
    use super::filesystem::{INVENTORY_MAX_ENTRIES, visit_for_test_with_depth};

    // Avoid creating 250k files: start the walk as if the entry budget is
    // already exhausted and assert the synthetic skip diagnostic.
    let root = temp_root("entry-limit");
    write_file(root.join("never-reached.txt"), "x\n");
    let mut out = Vec::with_capacity(INVENTORY_MAX_ENTRIES);
    out.resize(INVENTORY_MAX_ENTRIES, PathBuf::from("seed"));
    let mut skipped = Vec::new();
    visit_for_test_with_depth(&root, &root, 0, &mut out, &mut skipped)?;
    assert_eq!(out.len(), INVENTORY_MAX_ENTRIES);
    assert!(
        skipped.iter().any(|path| {
            path.to_string_lossy()
                .contains(".cargo-allow-inventory-entry-limit-")
        }),
        "expected entry-limit diagnostic in skipped={skipped:?}"
    );
    remove_dir(&root);
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

/// Whether the system `git` binary is available (#1908).
///
/// Git-spawning tests call this at the top and return early (with a skip
/// message) when git is absent, so they do not hard-panic in gitless
/// sandboxes. Pure-parser tests do not call git and remain always-on.
fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .status()
        .is_ok_and(|s| s.success())
}

fn remove_dir(path: &Path) {
    fs::remove_dir_all(path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove temp root: {err}")));
}

#[test]
fn git_ls_files_z_parser_ignores_trailing_empty_record() {
    let parsed = parse_git_ls_files_z(b"src/lib.rs\0README.md\0\0");

    assert_eq!(
        parsed,
        vec![PathBuf::from("src/lib.rs"), PathBuf::from("README.md")]
    );
}

#[test]
fn inventory_include_untracked_uses_filesystem_and_applies_default_ignores() {
    let root = temp_root("include-untracked");
    write_file(root.join("tracked.txt"), "tracked");
    fs::create_dir_all(root.join("target"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("target dir: {err}")));
    fs::create_dir_all(root.join(".git"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("git dir: {err}")));
    write_file(root.join("target").join("ignored.txt"), "ignored");
    write_file(root.join(".git").join("ignored.txt"), "ignored");

    let options = InventoryOptions {
        include_untracked: true,
        ..InventoryOptions::default()
    };
    let snapshot = inventory(&root, &options)
        .unwrap_or_else(|err| std::panic::panic_any(format!("include untracked inventory: {err}")));

    assert_eq!(snapshot.source, InventorySource::FilesystemIncludeUntracked);
    assert_eq!(snapshot.files, vec![PathBuf::from("tracked.txt")]);
    remove_dir(&root);
}

#[test]
fn inventory_sorts_and_deduplicates_git_tracked_files() {
    let root = temp_root("sorted-git");
    write_file(root.join("b.txt"), "b");
    write_file(root.join("a.txt"), "a");
    run_git(&root, &["init"]);
    run_git(&root, &["add", "b.txt", "a.txt"]);

    let snapshot = inventory(&root, &InventoryOptions::default())
        .unwrap_or_else(|err| std::panic::panic_any(format!("git inventory: {err}")));

    assert_eq!(snapshot.source, InventorySource::GitTracked);
    assert_eq!(
        snapshot.files,
        vec![PathBuf::from("a.txt"), PathBuf::from("b.txt")]
    );
    remove_dir(&root);
}

#[test]
fn source_tree_root_accepts_start_file_by_using_parent_directory() {
    let root = temp_root("start-file");
    let nested = root.join("src");
    fs::create_dir_all(&nested)
        .unwrap_or_else(|err| std::panic::panic_any(format!("nested dir: {err}")));
    let file = nested.join("lib.rs");
    write_file(file.clone(), "fn main() {}\n");
    run_git(&root, &["init"]);

    let discovered = discover_source_tree_root(&file)
        .unwrap_or_else(|err| std::panic::panic_any(format!("discover source tree root: {err}")));

    let canonical = root
        .canonicalize()
        .unwrap_or_else(|err| std::panic::panic_any(format!("canonical root: {err}")));
    assert_eq!(discovered, canonical);
    remove_dir(&root);
}

#[test]
fn explicit_source_tree_root_must_be_directory() {
    let root = temp_root("explicit-file");
    let file = root.join("Cargo.toml");
    write_file(file.clone(), "[workspace]\n");

    let err = resolve_source_tree_root(Some(&file), &root)
        .expect_err("file should not resolve as source tree root");

    assert!(
        err.to_string()
            .contains("source tree root is not a directory"),
        "unexpected error: {err}"
    );
    remove_dir(&root);
}
