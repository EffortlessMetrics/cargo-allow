use allow_core::{CargoAllowError, CargoAllowResult, glob_matches, normalize_path};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct InventoryOptions {
    pub ignored: Vec<String>,
    pub generated: Vec<String>,
    pub include_untracked: bool,
}

impl Default for InventoryOptions {
    fn default() -> Self {
        Self {
            ignored: vec![".git/**".to_string(), "target/**".to_string()],
            generated: Vec::new(),
            include_untracked: false,
        }
    }
}

pub fn resolve_source_tree_root(
    explicit_root: Option<&Path>,
    start: impl AsRef<Path>,
) -> CargoAllowResult<PathBuf> {
    if let Some(root) = explicit_root {
        return canonical_dir(root);
    }
    discover_source_tree_root(start)
}

pub fn discover_source_tree_root(start: impl AsRef<Path>) -> CargoAllowResult<PathBuf> {
    let start = canonical_start_dir(start.as_ref())?;
    let mut dir = start.as_path();
    loop {
        if dir.join(".git").exists() {
            return Ok(dir.to_path_buf());
        }
        let Some(parent) = dir.parent() else {
            return Ok(start);
        };
        dir = parent;
    }
}

fn canonical_dir(path: &Path) -> CargoAllowResult<PathBuf> {
    let canonical = path
        .canonicalize()
        .map_err(|e| CargoAllowError::new(format!("failed to canonicalize root path: {e}")))?;
    if canonical.is_dir() {
        Ok(canonical)
    } else {
        Err(CargoAllowError::new(format!(
            "source tree root is not a directory: {}",
            canonical.display()
        )))
    }
}

fn canonical_start_dir(start: &Path) -> CargoAllowResult<PathBuf> {
    let canonical = start
        .canonicalize()
        .map_err(|e| CargoAllowError::new(format!("failed to canonicalize start path: {e}")))?;
    if canonical.is_file() {
        canonical
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| CargoAllowError::new("start path has no parent directory"))
    } else {
        Ok(canonical)
    }
}

pub fn inventory_files(
    root: impl AsRef<Path>,
    options: &InventoryOptions,
) -> CargoAllowResult<Vec<PathBuf>> {
    let root = root.as_ref();
    let mut files = if options.include_untracked {
        recursive_files(root)?
    } else {
        git_ls_files(root).or_else(|_| recursive_files(root))?
    };
    files.sort();
    files.dedup();
    files.retain(|path| !is_ignored(path, &options.ignored));
    Ok(files)
}

pub fn git_ls_files(root: impl AsRef<Path>) -> CargoAllowResult<Vec<PathBuf>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root.as_ref())
        .arg("ls-files")
        .output()
        .map_err(|e| CargoAllowError::new(format!("failed to invoke git ls-files: {e}")))?;
    if !output.status.success() {
        return Err(CargoAllowError::new("git ls-files failed"));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(PathBuf::from)
        .collect())
}

fn recursive_files(root: &Path) -> CargoAllowResult<Vec<PathBuf>> {
    let mut out = Vec::new();
    visit(root, root, &mut out)?;
    Ok(out)
}

fn visit(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> CargoAllowResult<()> {
    for entry in fs::read_dir(dir)
        .map_err(|e| CargoAllowError::new(format!("failed to read {}: {e}", dir.display())))?
    {
        let entry = entry
            .map_err(|e| CargoAllowError::new(format!("failed to read directory entry: {e}")))?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|e| {
            CargoAllowError::new(format!("failed to inspect {}: {e}", path.display()))
        })?;
        if file_type.is_symlink() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == ".git" || name == "target" {
            continue;
        }
        if file_type.is_dir() {
            visit(root, &path, out)?;
        } else if file_type.is_file() {
            let rel = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            out.push(rel);
        }
    }
    Ok(())
}

fn is_ignored(path: &Path, patterns: &[String]) -> bool {
    let normalized = normalize_path(path);
    patterns.iter().any(|pattern| {
        glob_matches(pattern, path)
            || pattern
                .strip_suffix("/**")
                .map(|prefix| normalized == prefix || normalized.starts_with(&format!("{prefix}/")))
                .unwrap_or(false)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_target_paths() {
        let opts = InventoryOptions::default();
        assert!(super::is_ignored(
            Path::new("target/debug/x"),
            &opts.ignored
        ));
        assert!(super::is_ignored(Path::new(".git/config"), &opts.ignored));
    }

    #[test]
    fn dot_git_ignore_does_not_swallow_dot_github() {
        let opts = InventoryOptions::default();
        assert!(!super::is_ignored(
            Path::new(".github/workflows/ci.yml"),
            &opts.ignored
        ));
        assert!(!super::is_ignored(Path::new(".gitignore"), &opts.ignored));
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
        let with_untracked = inventory_files(
            &root,
            &InventoryOptions {
                include_untracked: true,
                ..InventoryOptions::default()
            },
        )
        .unwrap_or_else(|err| {
            std::panic::panic_any(format!("untracked-inclusive inventory: {err}"))
        });

        assert!(tracked.contains(&PathBuf::from("tracked.txt")));
        assert!(!tracked.contains(&PathBuf::from("untracked.txt")));
        assert!(with_untracked.contains(&PathBuf::from("tracked.txt")));
        assert!(with_untracked.contains(&PathBuf::from("untracked.txt")));
        remove_dir(&root);
    }

    #[test]
    fn inventory_applies_custom_ignored_globs() {
        let opts = InventoryOptions {
            ignored: vec!["scripts/**".to_string()],
            ..InventoryOptions::default()
        };

        assert!(super::is_ignored(
            Path::new("scripts/release.sh"),
            &opts.ignored
        ));
        assert!(!super::is_ignored(
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

        let discovered = discover_source_tree_root(&nested).unwrap_or_else(|err| {
            std::panic::panic_any(format!("discover source tree root: {err}"))
        });

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

        let discovered = discover_source_tree_root(&nested).unwrap_or_else(|err| {
            std::panic::panic_any(format!("discover source tree root: {err}"))
        });

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

        let discovered = discover_source_tree_root(&nested).unwrap_or_else(|err| {
            std::panic::panic_any(format!("discover source tree root: {err}"))
        });

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

        let discovered = resolve_source_tree_root(Some(&explicit), &nested).unwrap_or_else(|err| {
            std::panic::panic_any(format!("resolve source tree root: {err}"))
        });

        let canonical = explicit
            .canonicalize()
            .unwrap_or_else(|err| std::panic::panic_any(format!("canonical explicit: {err}")));
        assert_eq!(discovered, canonical);
        remove_dir(&root);
    }

    #[cfg(unix)]
    #[test]
    fn recursive_inventory_skips_symlinked_directories() {
        use std::fs;
        use std::os::unix::fs::symlink;
        use std::path::PathBuf;
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "cargo-allow-symlink-test-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("real")).expect("create test fixture directory");
        fs::write(root.join("real/file.txt"), "tracked").expect("write test fixture file");
        symlink(&root, root.join("real/loop")).expect("create symlink loop");

        let files = super::recursive_files(&root).expect("recursive inventory should finish");

        assert_eq!(files, vec![PathBuf::from("real/file.txt")]);
        fs::remove_dir_all(root).expect("clean up test fixture");
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
        fs::write(&path, contents).unwrap_or_else(|err| {
            std::panic::panic_any(format!("write {}: {err}", path.display()))
        });
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
}
