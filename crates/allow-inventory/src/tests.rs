use super::*;
use crate::filesystem::{
    ReparseClassification, existing_regular_files_with_metadata, visit_for_test,
};
use allow_core::source_tree_path_is_ignored;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn inaccessible_paths_make_inventory_partial_even_when_git_failed() {
    let options = InventoryOptions {
        ignored: Vec::new(),
        generated: Vec::new(),
        include_untracked: false,
    };
    assert_eq!(
        super::inventory_completeness(
            &options,
            false,
            &[],
            &[],
            &[PathBuf::from("blocked.rs")],
            &[],
            &[],
        ),
        InventoryCompleteness::Partial
    );
    // #4646: omissions outrank fallback provenance. A truncated filesystem
    // walk reports Partial even though git failed; the fallback source stays
    // visible in `InventorySource` and `git_error`, never in the
    // completeness value.
    assert_eq!(
        super::inventory_completeness(
            &options,
            true,
            &[],
            &[],
            &[PathBuf::from("blocked.rs")],
            &[PathBuf::from("skipped.rs")],
            &[],
        ),
        InventoryCompleteness::Partial
    );
    // #4677: a junctioned alias traversal is a recorded omission too.
    assert_eq!(
        super::inventory_completeness(
            &options,
            false,
            &[],
            &[],
            &[],
            &[],
            &[PathBuf::from("linked_src/lib.rs")],
        ),
        InventoryCompleteness::Partial
    );
    // A fallback walk with no recorded omissions keeps the fallback class.
    assert_eq!(
        super::inventory_completeness(&options, true, &[], &[], &[], &[], &[]),
        InventoryCompleteness::Fallback
    );
}

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
fn inventory_does_not_intrinsically_exclude_tool_owned_cache() {
    if !git_available() {
        eprintln!("skipped: git not available");
        return;
    }
    let root = temp_root("tool-owned-cache");
    write_file(root.join("tracked.txt"), "tracked");
    fs::create_dir_all(root.join("target/cargo-allow/cache"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("cache dir: {err}")));
    write_file(
        root.join("target/cargo-allow/cache/fact.rs"),
        "fn cached() {}",
    );
    run_git(&root, &["init"]);
    run_git(&root, &["add", "tracked.txt"]);
    let options = InventoryOptions {
        ignored: Vec::new(),
        generated: Vec::new(),
        include_untracked: false,
    };
    let first = inventory(&root, &options)
        .unwrap_or_else(|err| std::panic::panic_any(format!("first inventory: {err}")));
    assert_eq!(first.completeness, InventoryCompleteness::Complete);
    let include_untracked = inventory(
        &root,
        &InventoryOptions {
            ignored: vec![".git/**".to_string()],
            include_untracked: true,
            ..options.clone()
        },
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("untracked inventory: {err}")));
    assert_eq!(
        include_untracked.completeness,
        InventoryCompleteness::Scoped
    );
    run_git(&root, &["add", "-f", "target/cargo-allow/cache/fact.rs"]);
    let second = inventory(&root, &options)
        .unwrap_or_else(|err| std::panic::panic_any(format!("second inventory: {err}")));
    assert_eq!(second.completeness, InventoryCompleteness::Complete);
    assert!(first.files.contains(&PathBuf::from("tracked.txt")));
    assert!(
        second
            .files
            .contains(&PathBuf::from("target/cargo-allow/cache/fact.rs"))
    );
    std::fs::remove_file(root.join("target/cargo-allow/cache/fact.rs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove cache fixture: {err}")));
    let deleted = inventory(&root, &options)
        .unwrap_or_else(|err| std::panic::panic_any(format!("deleted inventory: {err}")));
    assert_eq!(deleted.completeness, InventoryCompleteness::Partial);
    assert!(
        deleted
            .deleted_tracked
            .contains(&PathBuf::from("target/cargo-allow/cache/fact.rs"))
    );
    remove_dir(&root);
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

    let ReparseClassification {
        existing,
        deleted_tracked,
        submodule_paths,
        inaccessible_paths,
        reparse_skipped_paths,
    } = existing_regular_files(
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
    assert!(inaccessible_paths.is_empty());
    assert!(reparse_skipped_paths.is_empty());
    remove_dir(&root);
    Ok(())
}

#[test]
fn existing_regular_files_discloses_non_not_found_metadata_errors() {
    let root = temp_root("inaccessible-metadata");
    write_file(root.join("kept.txt"), "kept");
    let ReparseClassification {
        existing,
        deleted_tracked: deleted,
        submodule_paths: submodules,
        inaccessible_paths: inaccessible,
        reparse_skipped_paths: reparse_skipped,
    } = existing_regular_files_with_metadata(
        &root,
        vec![
            PathBuf::from("kept.txt"),
            PathBuf::from("missing.txt"),
            PathBuf::from("blocked.txt"),
        ],
        |path| {
            if path.ends_with("missing.txt") {
                Err(std::io::Error::from(std::io::ErrorKind::NotFound))
            } else if path.ends_with("blocked.txt") {
                Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
            } else {
                fs::metadata(path)
            }
        },
        |_path| false,
    );

    assert_eq!(existing, vec![PathBuf::from("kept.txt")]);
    assert_eq!(deleted, vec![PathBuf::from("missing.txt")]);
    assert!(submodules.is_empty());
    assert_eq!(inaccessible, vec![PathBuf::from("blocked.txt")]);
    assert!(reparse_skipped.is_empty());
    remove_dir(&root);
}

/// A prefix that is probed and found NOT to be an alias must be cached like a
/// positive verdict. The first cut cached only aliases, so every file re-probed
/// its non-alias ancestors (O(files x depth) stats on a large tree). The other
/// probe family — one final-component check per file, for #1842 parity and the
/// dangling-reparse disclosure — is unchanged: each file is probed exactly once.
#[test]
fn reparse_prefix_probe_runs_once_per_unique_prefix_even_without_aliases() {
    use std::sync::Arc;
    use std::sync::Mutex;

    // Nonexistent root: every probe is answered by the synthetic closures, so
    // no filesystem fixture is needed.
    let root = temp_root("reparse-probe-count");
    let files = vec![
        PathBuf::from("a/one.rs"),
        PathBuf::from("a/b/two.rs"),
        PathBuf::from("a/b/c/three.rs"),
        PathBuf::from("a/four.rs"),
    ];
    let probes = Arc::new(Mutex::new(Vec::<PathBuf>::new()));
    let recorder = Arc::clone(&probes);
    let classification = existing_regular_files_with_metadata(
        &root,
        files,
        |_path| Err(std::io::Error::from(std::io::ErrorKind::NotFound)),
        move |probe| {
            recorder.lock().unwrap().push(probe.to_path_buf());
            false
        },
    );

    let probes = probes.lock().unwrap().clone();
    let file_paths: Vec<PathBuf> = [
        PathBuf::from("a/one.rs"),
        PathBuf::from("a/b/two.rs"),
        PathBuf::from("a/b/c/three.rs"),
        PathBuf::from("a/four.rs"),
    ]
    .into_iter()
    .collect();
    let mut file_probes = Vec::new();
    let mut prefix_probes = Vec::new();
    for probe in probes {
        let relative = probe.strip_prefix(&root).expect("probe under root");
        if file_paths.contains(&relative.to_path_buf()) {
            file_probes.push(relative.to_path_buf());
        } else {
            prefix_probes.push(relative.to_path_buf());
        }
    }

    // Each unique non-empty prefix (a, a/b, a/b/c) is probed exactly once —
    // negative verdicts included.
    prefix_probes.sort();
    prefix_probes.dedup();
    assert_eq!(
        prefix_probes,
        vec![PathBuf::from("a"), PathBuf::from("a/b"), PathBuf::from("a/b/c")],
        "each unique directory prefix is probed exactly once"
    );
    // The per-file final-component probes stay one per file.
    file_probes.sort();
    assert_eq!(
        file_probes,
        vec![
            PathBuf::from("a/b/c/three.rs"),
            PathBuf::from("a/b/two.rs"),
            PathBuf::from("a/four.rs"),
            PathBuf::from("a/one.rs"),
        ]
    );
    assert!(classification.reparse_skipped_paths.is_empty());
}

/// #4677: a directory reparse point in a repo-relative prefix makes the whole
/// path an alias traversal. The classification must skip it (with disclosure)
/// instead of scanning the target tree a second time under the alias spelling.
/// Runs on every platform through the metadata seam, so no junction support is
/// required.
#[test]
fn alias_directory_prefix_is_skipped_and_disclosed() {
    let root = temp_root("reparse-alias-prefix");
    fs::create_dir_all(root.join("real"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create real dir: {err}")));
    write_file(root.join("real/lib.rs"), "pub fn demo() {}\n");
    // Model the #4677 constraint: lstat of `linked_src` is a reparse point
    // resolving to a directory, while the deeper `linked_src/lib.rs` lstat is
    // an ordinary path (ancestor reparse points are invisible to lstat).
    let ReparseClassification {
        existing,
        deleted_tracked: deleted,
        submodule_paths: submodules,
        inaccessible_paths: inaccessible,
        reparse_skipped_paths: reparse_skipped,
    } = existing_regular_files_with_metadata(
        &root,
        vec![
            PathBuf::from("real/lib.rs"),
            PathBuf::from("linked_src/lib.rs"),
        ],
        |path| {
            if path.ends_with("linked_src/lib.rs") {
                // Follow-the-link stat resolves the alias to the target
                // file — exactly why trusting it alone double-counts.
                Ok(
                    fs::metadata(root.join("real/lib.rs")).unwrap_or_else(|err| {
                        std::panic::panic_any(format!("alias target stat: {err}"))
                    }),
                )
            } else if path.ends_with("linked_src") {
                // The prefix probe follows the link and sees a directory.
                Ok(fs::metadata(root.join("real"))
                    .unwrap_or_else(|err| std::panic::panic_any(format!("alias dir stat: {err}"))))
            } else {
                fs::metadata(path)
            }
        },
        |path| path.ends_with("linked_src"),
    );

    assert_eq!(existing, vec![PathBuf::from("real/lib.rs")]);
    assert_eq!(reparse_skipped, vec![PathBuf::from("linked_src/lib.rs")]);
    assert!(deleted.is_empty());
    assert!(submodules.is_empty());
    assert!(inaccessible.is_empty());
    remove_dir(&root);
}

/// #1846 regression guard: the reparse classification must not swallow the
/// submodule disclosure. A git-tracked path that is a plain directory (no
/// reparse point) stays a `Submodule` candidate.
#[test]
fn plain_directory_stays_submodule_not_reparse_skip() -> Result<(), Box<dyn std::error::Error>> {
    let root = temp_root("reparse-plain-dir");
    fs::create_dir_all(root.join("checked-out-sub"))?;
    let ReparseClassification {
        existing,
        deleted_tracked: deleted,
        submodule_paths: submodules,
        inaccessible_paths: inaccessible,
        reparse_skipped_paths: reparse_skipped,
    } = existing_regular_files_with_metadata(
        &root,
        vec![PathBuf::from("checked-out-sub")],
        |path: &Path| fs::metadata(path),
        |_path| false,
    );

    assert!(existing.is_empty());
    assert_eq!(submodules, vec![PathBuf::from("checked-out-sub")]);
    assert!(reparse_skipped.is_empty());
    assert!(deleted.is_empty());
    assert!(inaccessible.is_empty());
    remove_dir(&root);
    Ok(())
}

/// #4677: a reparse point as the final component resolving to a directory is
/// an alias traversal — it must be skipped and disclosed, not classified as a
/// submodule (today's wrong bucket).
#[test]
fn reparse_final_component_resolving_to_dir_is_skipped_not_submodule()
-> Result<(), Box<dyn std::error::Error>> {
    let root = temp_root("reparse-final-dir");
    fs::create_dir_all(root.join("real"))?;
    let ReparseClassification {
        existing,
        deleted_tracked: deleted,
        submodule_paths: submodules,
        inaccessible_paths: inaccessible,
        reparse_skipped_paths: reparse_skipped,
    } = existing_regular_files_with_metadata(
        &root,
        vec![PathBuf::from("junction_dir")],
        |path: &Path| fs::metadata(path),
        |path| path.ends_with("junction_dir"),
    );

    assert!(existing.is_empty());
    assert!(submodules.is_empty());
    assert_eq!(reparse_skipped, vec![PathBuf::from("junction_dir")]);
    assert!(deleted.is_empty());
    assert!(inaccessible.is_empty());
    remove_dir(&root);
    Ok(())
}

/// #4677: a dangling reparse point as the final component is neither a
/// deleted tracked file nor scannable content — skip and disclose instead of
/// today's wrong `Deleted` classification.
#[test]
fn dangling_reparse_final_component_is_skipped_not_deleted() {
    let root = temp_root("reparse-dangling");
    write_file(root.join("kept.txt"), "kept");
    let ReparseClassification {
        existing,
        deleted_tracked: deleted,
        submodule_paths: submodules,
        inaccessible_paths: inaccessible,
        reparse_skipped_paths: reparse_skipped,
    } = existing_regular_files_with_metadata(
        &root,
        vec![PathBuf::from("kept.txt"), PathBuf::from("broken-link")],
        |path| {
            if path.ends_with("broken-link") {
                Err(std::io::Error::from(std::io::ErrorKind::NotFound))
            } else {
                fs::metadata(path)
            }
        },
        |path| path.ends_with("broken-link"),
    );

    assert_eq!(existing, vec![PathBuf::from("kept.txt")]);
    assert_eq!(reparse_skipped, vec![PathBuf::from("broken-link")]);
    assert!(deleted.is_empty());
    assert!(submodules.is_empty());
    assert!(inaccessible.is_empty());
    remove_dir(&root);
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

/// #4646: the completeness classification must survive a truncated fallback
/// walk. A file hidden below the depth cap is recorded as a skipped path and
/// the whole inventory degrades to Partial even though git is unavailable —
/// the fallback source alone must not read as complete coverage.
#[test]
fn depth_capped_fallback_inventory_reports_partial_with_skip_disclosure()
-> Result<(), Box<dyn std::error::Error>> {
    use super::filesystem::INVENTORY_MAX_DEPTH;

    let root = temp_root("depth-capped-fallback");
    let mut deep = root.clone();
    for index in 0..=INVENTORY_MAX_DEPTH {
        deep = deep.join(format!("d{index}"));
    }
    fs::create_dir_all(&deep)?;
    write_file(deep.join("hidden.rs"), "fn hidden() {}\n");
    write_file(root.join("visible.txt"), "visible\n");

    let snapshot = inventory(&root, &InventoryOptions::default())
        .unwrap_or_else(|err| std::panic::panic_any(format!("depth-capped inventory: {err}")));

    assert_eq!(snapshot.source, InventorySource::FilesystemFallback);
    assert_eq!(
        snapshot.completeness,
        InventoryCompleteness::Partial,
        "a depth-truncated fallback walk must not classify as bare Fallback"
    );
    assert!(snapshot.git_error.is_some());
    assert_eq!(snapshot.skipped_paths.len(), 1);
    assert!(
        snapshot.skipped_paths.iter().any(|path| {
            path.to_string_lossy()
                .contains(".cargo-allow-inventory-depth-limit-")
        }),
        "expected the depth-limit marker in skipped={:?}",
        snapshot.skipped_paths
    );
    assert!(snapshot.files.contains(&PathBuf::from("visible.txt")));
    assert!(
        !snapshot
            .files
            .iter()
            .any(|path| path.file_name().is_some_and(|name| name == "hidden.rs"))
    );
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

/// #4646 review: the walker records skips before the configured ignore globs
/// are applied, so a skip inside a policy-ignored subtree omits no selected
/// input. The skip must be filtered out as scope — not classify the inventory
/// Partial — while the same skip outside the ignored scope stays fail-closed.
#[test]
fn ignored_subtree_skips_are_scope_not_omissions() -> Result<(), Box<dyn std::error::Error>> {
    use super::filesystem::INVENTORY_MAX_DEPTH;

    let root = temp_root("ignored-subtree-skip");
    let mut deep = root.join("vendor").join("private");
    for index in 0..=INVENTORY_MAX_DEPTH {
        deep = deep.join(format!("d{index}"));
    }
    fs::create_dir_all(&deep)?;
    write_file(deep.join("hidden.rs"), "fn hidden() {}\n");
    write_file(root.join("visible.txt"), "visible\n");

    // Without the ignore: the capped vendor walk is a recorded omission and
    // the inventory degrades to Partial.
    let unscoped = inventory(&root, &InventoryOptions::default())
        .unwrap_or_else(|err| std::panic::panic_any(format!("unscoped inventory: {err}")));
    assert_eq!(unscoped.completeness, InventoryCompleteness::Partial);
    assert_eq!(unscoped.skipped_paths.len(), 1);

    // With the ignore: the same skip is scope, not an omission — the
    // inventory keeps fallback provenance and loses the Partial marker.
    let mut scoped_options = InventoryOptions::default();
    scoped_options.ignored.push("vendor/**".to_string());
    let scoped = inventory(&root, &scoped_options)
        .unwrap_or_else(|err| std::panic::panic_any(format!("scoped inventory: {err}")));
    assert_eq!(scoped.source, InventorySource::FilesystemFallback);
    assert_eq!(
        scoped.completeness,
        InventoryCompleteness::Fallback,
        "an ignored-subtree skip must not classify the inventory Partial"
    );
    assert!(
        scoped.skipped_paths.is_empty(),
        "ignored-subtree skips must be filtered from skipped_paths, got {:?}",
        scoped.skipped_paths
    );
    assert!(scoped.files.contains(&PathBuf::from("visible.txt")));

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
