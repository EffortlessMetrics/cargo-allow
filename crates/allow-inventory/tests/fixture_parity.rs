//! #1783 umbrella acceptance: cross-platform fixture parity between the
//! public inventory API and `git ls-files` expectations.
//!
//! The fixture is built programmatically in a temp directory (no tempfile
//! dependency) and covers selected parity dimensions: gitignored artifacts,
//! nested `target` dirs (prune is root-only), non-UTF-8 filenames (#1841),
//! file symlinks (#1842), and submodule disclosure (#1846). Symlink and
//! submodule capability gaps are explicit and conditional; unexpected setup
//! failures are hard failures rather than passing assertions.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use allow_inventory::{
    Inventory, InventoryCompleteness, InventoryOptions, InventorySource, inventory,
};

/// Monotonic counter plus timestamp plus pid: the system clock is coarse on
/// Windows, so timestamp-only temp names collide and delete each other's
/// fixtures during cleanup.
static NEXT_ID: AtomicU64 = AtomicU64::new(0);

fn temp_root(label: &str) -> PathBuf {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!(
        "allow-inventory-parity-{label}-{}-{stamp}-{id}",
        std::process::id()
    ));
    fs::create_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("create temp root: {err}")));
    root
}

fn drop_root_best_effort(root: &Path) {
    // Submodule fixtures contain git-managed object files that can be
    // read-only on Windows; cleanup failure must not fail passing assertions,
    // so removal problems are reported instead of panicked.
    if let Err(err) = fs::remove_dir_all(root) {
        eprintln!("warning: cleanup of {}: {err}", root.display());
    }
}

fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .status()
        .is_ok_and(|status| status.success())
}

fn run_git<S: AsRef<OsStr> + std::fmt::Debug>(root: &Path, args: &[S]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("invoke git {args:?}: {err}")));
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn configure_empty_excludes_file(root: &Path) {
    let excludes = root.join(".git").join("empty-excludes");
    write_file(excludes.clone(), "");
    let excludes = excludes.to_string_lossy().into_owned();
    run_git(root, &["config", "core.excludesFile", excludes.as_str()]);
}

fn submodule_capability_unsupported(stderr: &str) -> bool {
    stderr.lines().next().is_some_and(|line| {
        line.trim() == "git: 'submodule' is not a git command. See 'git --help'."
    })
}

fn write_file(path: PathBuf, contents: &str) {
    fs::write(&path, contents)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write {}: {err}", path.display())));
}

fn run_inventory(root: &Path, options: &InventoryOptions) -> Inventory {
    inventory(root, options)
        .unwrap_or_else(|err| std::panic::panic_any(format!("inventory failed: {err}")))
}

#[cfg(unix)]
fn non_utf8_file_name() -> OsString {
    use std::os::unix::ffi::OsStringExt;
    // 0xFF is invalid UTF-8; Unix paths carry raw bytes, so the `-z` parser
    // must round-trip them losslessly (#1841).
    OsString::from_vec(b"nonutf8-\xff.rs".to_vec())
}

#[cfg(target_os = "macos")]
fn unsupported_non_utf8_path(error: &std::io::Error) -> bool {
    // Darwin reports EILSEQ (illegal byte sequence) as errno 92 when its
    // filesystem rejects a path containing invalid UTF-8 bytes.
    error.raw_os_error() == Some(92)
}

#[cfg(not(target_os = "macos"))]
fn unsupported_non_utf8_path(_error: &std::io::Error) -> bool {
    false
}

#[cfg(windows)]
fn non_utf8_file_name() -> OsString {
    use std::os::windows::ffi::OsStringExt;
    // Windows filenames are UTF-16. U+00FF survives git's UTF-8 emission and
    // the documented Windows lossy conversion unchanged.
    let wide: Vec<u16> = "nonutf8-\u{00FF}.rs".encode_utf16().collect();
    OsString::from_wide(&wide)
}

#[cfg(not(any(unix, windows)))]
fn non_utf8_file_name() -> OsString {
    OsString::from("nonutf8-fallback.rs")
}

#[cfg(unix)]
fn create_file_symlink(target: &Path, link: &Path) -> bool {
    std::os::unix::fs::symlink(target, link).is_ok()
}

#[cfg(windows)]
fn create_file_symlink(target: &Path, link: &Path) -> bool {
    use std::os::windows::fs::symlink_file;
    // Creating a file symlink needs developer mode or elevation on Windows;
    // the return value is a runtime probe, not an assertion.
    symlink_file(target, link).is_ok()
}

#[cfg(not(any(unix, windows)))]
fn create_file_symlink(_target: &Path, _link: &Path) -> bool {
    false
}

struct Fixture {
    root: PathBuf,
    symlink_supported: bool,
    non_utf8_rel: Option<PathBuf>,
}

fn build_parity_fixture(label: &str) -> Fixture {
    let root = temp_root(label);
    fs::create_dir_all(root.join("src").join("target"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("src/target dir: {err}")));
    fs::create_dir_all(root.join("target"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("target dir: {err}")));
    fs::create_dir_all(root.join("build-artifacts"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("build-artifacts dir: {err}")));
    fs::create_dir_all(root.join("scratch"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("scratch dir: {err}")));

    write_file(root.join(".gitignore"), "build-artifacts/\n");
    write_file(root.join("src").join("lib.rs"), "pub fn demo() {}\n");
    write_file(
        root.join("src").join("target").join("mod.rs"),
        "pub fn target_module() {}\n",
    );
    // Staged despite living in the root `target` dir: the scope-free parity
    // run must keep it (raw `git ls-files` behavior), while the default-
    // option run must drop it through the policy glob filter.
    write_file(root.join("target").join("debug.txt"), "ignored\n");
    write_file(
        root.join("build-artifacts").join("artifact.bin"),
        "artifact\n",
    );
    // Untracked but not ignored: only the untracked-inclusive mode may see it.
    write_file(root.join("scratch").join("uncommitted.txt"), "untracked\n");

    let non_utf8_name = non_utf8_file_name();
    let non_utf8_path = root.join(&non_utf8_name);
    let non_utf8_rel = match fs::write(&non_utf8_path, b"non-utf8 content\n") {
        Ok(()) => Some(PathBuf::from(&non_utf8_name)),
        Err(err) => {
            // Some Unix filesystems, notably the macOS filesystem used by
            // the cross-platform CI lane, reject invalid UTF-8 filename
            // bytes before Git can see them. Keep this capability gap
            // explicit rather than making the entire parity fixture fail.
            if unsupported_non_utf8_path(&err) {
                eprintln!("skipped: filesystem cannot create non-UTF-8 fixture path ({err})");
                None
            } else {
                std::panic::panic_any(format!("write {non_utf8_path:?}: {err}"));
            }
        }
    };

    let symlink_supported = create_file_symlink(
        &root.join("src").join("lib.rs"),
        &root.join("link-to-lib.rs"),
    );
    if !symlink_supported {
        eprintln!(
            "skipped: file-symlink fixture entries (symlink creation unavailable on this platform)"
        );
    }

    run_git(&root, &["init"]);
    configure_empty_excludes_file(&root);
    run_git(&root, &["config", "user.email", "test@example.com"]);
    run_git(&root, &["config", "user.name", "Test"]);
    run_git(
        &root,
        &[
            "add",
            ".gitignore",
            "src/lib.rs",
            "src/target/mod.rs",
            "target/debug.txt",
        ],
    );
    if non_utf8_rel.is_some() {
        run_git(&root, &[OsStr::new("add"), non_utf8_name.as_os_str()]);
    }
    if symlink_supported {
        run_git(&root, &["add", "link-to-lib.rs"]);
    }

    Fixture {
        root,
        symlink_supported,
        non_utf8_rel,
    }
}

/// Scope-free options are required for completeness == Complete: the default
/// option set always carries ignore globs, which force Scoped by definition.
fn scope_free_options(include_untracked: bool) -> InventoryOptions {
    InventoryOptions {
        ignored: Vec::new(),
        generated: Vec::new(),
        include_untracked,
    }
}

#[test]
fn fixture_inventory_matches_git_ls_files_expectations() {
    if !git_available() {
        eprintln!("skipped: git not available");
        return;
    }
    let fixture = build_parity_fixture("tracked-parity");

    let mut expected_tracked = vec![
        PathBuf::from(".gitignore"),
        PathBuf::from("src/lib.rs"),
        PathBuf::from("src/target/mod.rs"),
        PathBuf::from("target/debug.txt"),
    ];
    if let Some(non_utf8_rel) = &fixture.non_utf8_rel {
        expected_tracked.push(non_utf8_rel.clone());
    }
    if fixture.symlink_supported {
        expected_tracked.push(PathBuf::from("link-to-lib.rs"));
    }
    // inventory() sorts its output before returning.
    expected_tracked.sort();

    let snapshot = run_inventory(&fixture.root, &scope_free_options(false));

    assert_eq!(snapshot.source, InventorySource::GitTracked);
    assert_eq!(snapshot.completeness, InventoryCompleteness::Complete);
    assert_eq!(snapshot.git_error, None);
    assert!(!snapshot.empty_git_tracked);
    assert!(snapshot.deleted_tracked.is_empty());
    assert!(snapshot.skipped_paths.is_empty());
    assert!(snapshot.submodule_paths.is_empty());
    assert_eq!(
        snapshot.files, expected_tracked,
        "git-tracked inventory must equal the tracked set exactly once per path"
    );

    let defaults = run_inventory(&fixture.root, &InventoryOptions::default());
    assert_eq!(defaults.source, InventorySource::GitTracked);
    assert_eq!(
        defaults.completeness,
        InventoryCompleteness::Scoped,
        "default ignore globs must force Scoped even over a clean tree"
    );
    // Nested `target` survives the root-anchored policy glob...
    assert!(defaults.files.contains(&PathBuf::from("src/target/mod.rs")));
    // ...while the staged root-level `target` path does not.
    assert!(!defaults.files.contains(&PathBuf::from("target/debug.txt")));
    assert!(
        !defaults
            .files
            .contains(&PathBuf::from("build-artifacts/artifact.bin"))
    );
    if fixture.symlink_supported {
        assert!(defaults.files.contains(&PathBuf::from("link-to-lib.rs")));
    }

    let untracked = run_inventory(&fixture.root, &scope_free_options(true));
    assert_eq!(
        untracked.source,
        InventorySource::FilesystemIncludeUntracked
    );
    assert!(
        untracked
            .files
            .contains(&PathBuf::from("scratch/uncommitted.txt")),
        "untracked-inclusive mode must see untracked non-ignored paths"
    );
    assert!(
        !untracked
            .files
            .contains(&PathBuf::from("build-artifacts/artifact.bin")),
        "--exclude-standard must keep gitignored artifacts excluded"
    );
    // Scope-free options disable the policy glob filter entirely, so the
    // staged root-level path shows up here too; its absence under defaults is
    // asserted above.
    assert!(untracked.files.contains(&PathBuf::from("target/debug.txt")));

    drop_root_best_effort(&fixture.root);
}

#[test]
fn submodule_fixture_reports_partial_completeness_with_disclosure() {
    if !git_available() {
        eprintln!("skipped: git not available");
        return;
    }
    let parent = temp_root("parity-submodule-parent");
    let child = temp_root("parity-submodule-child");

    write_file(child.join("inner-lib.rs"), "pub fn inner() {}\n");
    run_git(&child, &["init"]);
    configure_empty_excludes_file(&child);
    run_git(&child, &["config", "user.email", "test@example.com"]);
    run_git(&child, &["config", "user.name", "Test"]);
    run_git(&child, &["add", "inner-lib.rs"]);
    run_git(&child, &["commit", "-m", "seed"]);

    write_file(parent.join("README.md"), "# parent\n");
    run_git(&parent, &["init"]);
    configure_empty_excludes_file(&parent);
    run_git(&parent, &["config", "user.email", "test@example.com"]);
    run_git(&parent, &["config", "user.name", "Test"]);
    run_git(&parent, &["add", "README.md"]);
    run_git(&parent, &["commit", "-m", "seed"]);

    // Local-path submodule adds require the file protocol opt-in since the
    // git 2.38.1 security release.
    // Avoid the Windows `\\?\` canonical path prefix: Git treats it as a
    // different URL and reports a misleading repository-not-found error.
    let child_url = child.to_string_lossy().replace('\\', "/");
    let added = Command::new("git")
        .arg("-c")
        .arg("protocol.file.allow=always")
        .arg("-C")
        .arg(&parent)
        .arg("submodule")
        .arg("add")
        .arg(&child_url)
        .arg("nested-sub")
        .output();
    match added {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                if submodule_capability_unsupported(&stderr) {
                    eprintln!(
                        "skipped: submodule capability unavailable: {}",
                        stderr.trim()
                    );
                    drop_root_best_effort(&parent);
                    drop_root_best_effort(&child);
                    return;
                }
                std::panic::panic_any(format!(
                    "git submodule add failed unexpectedly: {}",
                    stderr.trim()
                ));
            }
        }
        Err(error) => {
            std::panic::panic_any(format!("spawn git submodule add unexpectedly: {error}"))
        }
    }

    let snapshot = run_inventory(&parent, &scope_free_options(false));

    assert_eq!(snapshot.source, InventorySource::GitTracked);
    assert_eq!(
        snapshot.completeness,
        InventoryCompleteness::Partial,
        "a checked-out submodule must degrade completeness to Partial"
    );
    assert_eq!(
        snapshot.submodule_paths,
        vec![PathBuf::from("nested-sub")],
        "the submodule gitlink must be disclosed (#1846)"
    );
    assert!(snapshot.files.contains(&PathBuf::from(".gitmodules")));
    assert!(snapshot.files.contains(&PathBuf::from("README.md")));
    // Submodule contents are never scanned.
    assert!(
        !snapshot
            .files
            .iter()
            .any(|path| path.starts_with("nested-sub")),
        "submodule contents must stay out of the inventoried file set"
    );

    drop_root_best_effort(&parent);
    drop_root_best_effort(&child);
}

#[test]
fn submodule_skip_requires_a_known_unsupported_capability() {
    assert!(submodule_capability_unsupported(
        "git: 'submodule' is not a git command. See 'git --help'."
    ));
    assert!(!submodule_capability_unsupported(
        "git: 'submodule' is not a git command"
    ));
    assert!(!submodule_capability_unsupported(
        "fatal: unknown subcommand: submodule"
    ));
    assert!(!submodule_capability_unsupported(
        "submodule is not supported"
    ));
    assert!(!submodule_capability_unsupported(
        "fatal: repository setup failed"
    ));
}

// ---------------------------------------------------------------------------
// #4677: junctioned source directories. git-for-Windows traverses junctions,
// so `git ls-files --others` can emit the same real file under a junctioned
// alias spelling on top of its ordinary spelling. The inventory must skip and
// disclose the alias traversal instead of double-counting the target tree.
// Junction creation is Windows-only (`mklink /J`); every test degrades to a
// reported skip when the filesystem or git cannot produce the fixture.
// ---------------------------------------------------------------------------

#[cfg(windows)]
fn create_dir_junction(target: &Path, link: &Path) -> bool {
    let output = Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .output();
    match output {
        Ok(output) if output.status.success() => true,
        Ok(output) => {
            eprintln!(
                "skipped: junction creation failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
            false
        }
        Err(err) => {
            eprintln!("skipped: junction spawn failed: {err}");
            false
        }
    }
}

/// Build the #4677 fixture: a tracked real tree plus an untracked sibling
/// directory that is also reachable through a `linked_src` junction. git's
/// untracked listing sees the target directory twice — once under each
/// spelling.
#[cfg(windows)]
struct JunctionFixture {
    root: PathBuf,
}

#[cfg(windows)]
fn build_junction_fixture(label: &str) -> JunctionFixture {
    let root = temp_root(label);
    std::fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("src dir: {err}")));
    std::fs::create_dir_all(root.join("real"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("real dir: {err}")));
    write_file(root.join("src").join("lib.rs"), "pub fn demo() {}\n");
    write_file(root.join("real").join("lib.rs"), "pub fn extra() {}\n");
    run_git(&root, &["init"]);
    configure_empty_excludes_file(&root);
    run_git(&root, &["config", "user.email", "test@example.com"]);
    run_git(&root, &["config", "user.name", "Test"]);
    run_git(&root, &["add", "src/lib.rs"]);
    let linked = !create_dir_junction(&root.join("real"), &root.join("linked_src"));
    if linked {
        eprintln!("skipped: junction fixture unavailable on this filesystem");
    }
    JunctionFixture { root }
}

#[cfg(windows)]
#[test]
fn junctioned_source_dir_is_skipped_and_disclosed() {
    if !git_available() {
        eprintln!("skipped: git not available");
        return;
    }
    let fixture = build_junction_fixture("junction-alias");
    if !fixture.root.join("linked_src").exists() {
        drop_root_best_effort(&fixture.root);
        return;
    }

    let snapshot = run_inventory(&fixture.root, &scope_free_options(true));

    assert_eq!(snapshot.source, InventorySource::FilesystemIncludeUntracked);
    assert_eq!(
        snapshot.completeness,
        InventoryCompleteness::Partial,
        "an alias traversal is a recorded omission and must degrade completeness"
    );
    assert_eq!(
        snapshot.reparse_skipped_paths,
        vec![PathBuf::from("linked_src/lib.rs")],
        "the junction spelling must be disclosed, not scanned (#4677)"
    );
    assert!(
        !snapshot
            .files
            .iter()
            .any(|path| path.starts_with("linked_src")),
        "alias spellings must stay out of the inventoried file set"
    );
    assert_eq!(
        snapshot.files,
        vec![PathBuf::from("real/lib.rs"), PathBuf::from("src/lib.rs"),],
        "each real file must be inventoried exactly once, never double-counted \
         through the junction spelling"
    );

    drop_root_best_effort(&fixture.root);
}

/// The junction cycle must terminate (never hang) and must never emit
/// phantom alias paths. git-for-Windows versions differ on whether they
/// traverse a cycle — 2.54 refuses with a warning and no paths — so the
/// partial/disclosure assertions apply only when git actually emitted cycle
/// spellings; the absence of phantom paths is asserted unconditionally.
#[cfg(windows)]
#[test]
fn junction_cycle_is_bounded_and_disclosed() {
    if !git_available() {
        eprintln!("skipped: git not available");
        return;
    }
    let fixture = build_junction_fixture("junction-cycle");
    if !fixture.root.join("linked_src").exists() {
        drop_root_best_effort(&fixture.root);
        return;
    }
    // mklink /J does not require an existing target, so the pair is created
    // out of order to close a true two-junction cycle (a -> b -> a).
    if !create_dir_junction(&fixture.root.join("cycle_b"), &fixture.root.join("cycle_a"))
        || !create_dir_junction(&fixture.root.join("cycle_a"), &fixture.root.join("cycle_b"))
    {
        drop_root_best_effort(&fixture.root);
        return;
    }
    // Close the cycle: cycle_b's target now contains cycle_a's spelling.
    let cycle_probe = Command::new("git")
        .arg("-C")
        .arg(&fixture.root)
        .args(["ls-files", "--others", "--exclude-standard", "-z"])
        .output()
        .map(|output| {
            parse_git_ls_files_output(&output.stdout)
                .into_iter()
                .filter(|path| path.starts_with("cycle_a") || path.starts_with("cycle_b"))
                .collect::<Vec<_>>()
        });
    let emitted_cycle_paths = cycle_probe.unwrap_or_default();

    let snapshot = run_inventory(&fixture.root, &scope_free_options(true));

    assert!(
        !snapshot
            .files
            .iter()
            .any(|path| path.starts_with("cycle_a")),
        "cycle_a spellings must never be inventoried as phantom coverage"
    );
    assert!(
        !snapshot
            .files
            .iter()
            .any(|path| path.starts_with("cycle_b")),
        "cycle_b spellings must never be inventoried as phantom coverage"
    );
    assert!(
        !snapshot
            .reparse_skipped_paths
            .iter()
            .any(|path| path.starts_with("cycle_a") || path.starts_with("cycle_b"))
            && snapshot
                .files
                .iter()
                .all(|path| !path.starts_with("cycle_a") && !path.starts_with("cycle_b")),
        "cycle junctions must be absent from both scanned files and disclosures \
         when git never emitted them"
    );
    if emitted_cycle_paths.is_empty() {
        eprintln!(
            "note: this git refuses to traverse the junction cycle; bounded-run \
             and phantom-absence assertions still hold"
        );
    } else {
        assert_eq!(
            snapshot.completeness,
            InventoryCompleteness::Partial,
            "any emitted cycle spelling is a recorded omission"
        );
        for path in &emitted_cycle_paths {
            assert!(
                snapshot.reparse_skipped_paths.contains(path),
                "every emitted cycle spelling must be disclosed: {path:?}"
            );
            assert!(
                !snapshot.files.contains(path),
                "no emitted cycle spelling may reach the file set: {path:?}"
            );
        }
    }

    drop_root_best_effort(&fixture.root);
}

/// Regression guard: tracked mode never traverses junctions (`git ls-files`
/// without `--others` lists only cached paths), so the same fixture must stay
/// Complete with zero reparse disclosures.
#[cfg(windows)]
#[test]
fn tracked_mode_ignores_junction_spelling() {
    if !git_available() {
        eprintln!("skipped: git not available");
        return;
    }
    let fixture = build_junction_fixture("junction-tracked");
    if !fixture.root.join("linked_src").exists() {
        drop_root_best_effort(&fixture.root);
        return;
    }

    let snapshot = run_inventory(&fixture.root, &scope_free_options(false));

    assert_eq!(snapshot.source, InventorySource::GitTracked);
    assert_eq!(
        snapshot.completeness,
        InventoryCompleteness::Complete,
        "tracked mode sees only cached paths; a junctioned untracked sibling \
         must not degrade it"
    );
    assert!(snapshot.reparse_skipped_paths.is_empty());
    assert!(
        !snapshot
            .files
            .iter()
            .any(|path| path.starts_with("linked_src")),
        "tracked mode must never emit the junction spelling"
    );
    assert_eq!(snapshot.files, vec![PathBuf::from("src/lib.rs")]);

    drop_root_best_effort(&fixture.root);
}

/// The filesystem fallback walk must keep refusing to recurse into directory
/// junctions (#1842): no alias paths, no hang, real files inventoried once.
#[cfg(windows)]
#[test]
fn filesystem_fallback_walk_skips_directory_junctions() {
    let root = temp_root("junction-fallback");
    std::fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("src dir: {err}")));
    std::fs::create_dir_all(root.join("real"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("real dir: {err}")));
    write_file(root.join("real").join("lib.rs"), "pub fn demo() {}\n");
    write_file(root.join("src").join("lib.rs"), "pub fn demo() {}\n");
    if !create_dir_junction(&root.join("real"), &root.join("linked_src")) {
        drop_root_best_effort(&root);
        return;
    }

    let snapshot = run_inventory(&root, &scope_free_options(true));

    // Outside any repository, include-untracked mode falls back to the raw
    // filesystem walk; the fallback is visible in `git_error` and the
    // `Fallback` completeness, while the source label stays untracked mode.
    assert!(snapshot.git_error.is_some());
    assert_eq!(
        snapshot.completeness,
        InventoryCompleteness::Fallback,
        "the walk refuses to recurse into junctions, so no omissions are recorded"
    );
    assert!(
        !snapshot
            .files
            .iter()
            .any(|path| path.starts_with("linked_src")),
        "the fallback walk must not recurse into junctioned directories"
    );
    assert!(snapshot.reparse_skipped_paths.is_empty());
    assert_eq!(
        snapshot.files,
        vec![PathBuf::from("real/lib.rs"), PathBuf::from("src/lib.rs"),],
        "real files stay inventoried exactly once through the walk"
    );

    drop_root_best_effort(&root);
}

/// Local `-z` split for the cycle probe: `parse_git_ls_files_z` is
/// crate-private, so integration tests keep their own splitter.
#[cfg(windows)]
fn parse_git_ls_files_output(stdout: &[u8]) -> Vec<PathBuf> {
    stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| PathBuf::from(String::from_utf8_lossy(path).into_owned()))
        .collect()
}
