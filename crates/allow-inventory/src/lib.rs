use allow_core::{CargoAllowError, CargoAllowResult, glob_matches, normalize_path};
use cargo_metadata::{MetadataCommand, PackageId};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceMetadata {
    pub root: PathBuf,
    pub packages: Vec<WorkspacePackage>,
}

impl WorkspaceMetadata {
    pub fn target_count(&self) -> usize {
        self.packages
            .iter()
            .map(|package| package.targets.len())
            .sum()
    }

    pub fn source_roots(&self) -> Vec<PathBuf> {
        let mut roots = self
            .packages
            .iter()
            .flat_map(|package| package.source_roots.iter().cloned())
            .collect::<Vec<_>>();
        roots.sort();
        roots.dedup();
        roots
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspacePackage {
    pub name: String,
    pub manifest_path: PathBuf,
    pub source_roots: Vec<PathBuf>,
    pub targets: Vec<WorkspaceTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceTarget {
    pub name: String,
    pub kinds: Vec<String>,
    pub src_path: PathBuf,
}

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

pub fn discover_workspace_root(start: impl AsRef<Path>) -> CargoAllowResult<PathBuf> {
    if let Ok(metadata) = workspace_metadata(start.as_ref()) {
        return Ok(metadata.root);
    }
    discover_workspace_root_by_ancestor(start)
}

pub fn workspace_metadata(start: impl AsRef<Path>) -> CargoAllowResult<WorkspaceMetadata> {
    let start = metadata_start_dir(start.as_ref())?;
    let metadata = MetadataCommand::new()
        .current_dir(start)
        .no_deps()
        .exec()
        .map_err(|e| CargoAllowError::new(format!("failed to read cargo metadata: {e}")))?;
    let root = metadata.workspace_root.as_std_path().to_path_buf();
    let member_ids = metadata
        .workspace_members
        .iter()
        .cloned()
        .collect::<BTreeSet<PackageId>>();
    let mut packages = metadata
        .packages
        .into_iter()
        .filter(|package| member_ids.contains(&package.id))
        .map(|package| workspace_package(package, &root))
        .collect::<Vec<_>>();
    packages.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(WorkspaceMetadata { root, packages })
}

fn metadata_start_dir(start: &Path) -> CargoAllowResult<PathBuf> {
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

fn workspace_package(package: cargo_metadata::Package, workspace_root: &Path) -> WorkspacePackage {
    let mut source_roots = package
        .targets
        .iter()
        .filter_map(|target| target.src_path.as_std_path().parent())
        .map(|path| relative_to_workspace(path, workspace_root))
        .collect::<Vec<_>>();
    source_roots.sort();
    source_roots.dedup();

    let mut targets = package
        .targets
        .into_iter()
        .map(|target| WorkspaceTarget {
            name: target.name,
            kinds: target.kind.iter().map(ToString::to_string).collect(),
            src_path: relative_to_workspace(target.src_path.as_std_path(), workspace_root),
        })
        .collect::<Vec<_>>();
    targets.sort_by(|a, b| a.name.cmp(&b.name).then(a.src_path.cmp(&b.src_path)));

    WorkspacePackage {
        name: package.name.to_string(),
        manifest_path: relative_to_workspace(package.manifest_path.as_std_path(), workspace_root),
        source_roots,
        targets,
    }
}

fn relative_to_workspace(path: &Path, workspace_root: &Path) -> PathBuf {
    path.strip_prefix(workspace_root)
        .unwrap_or(path)
        .to_path_buf()
}

fn discover_workspace_root_by_ancestor(start: impl AsRef<Path>) -> CargoAllowResult<PathBuf> {
    let mut dir = start
        .as_ref()
        .canonicalize()
        .map_err(|e| CargoAllowError::new(format!("failed to canonicalize start path: {e}")))?;
    loop {
        if dir.join("Cargo.toml").exists() || dir.join(".git").exists() {
            return Ok(dir);
        }
        if !dir.pop() {
            return Err(CargoAllowError::new(
                "could not find Cargo.toml or .git in this directory or ancestors",
            ));
        }
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
    fn workspace_metadata_reports_member_packages_and_targets() {
        let metadata = workspace_metadata(env!("CARGO_MANIFEST_DIR"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("read workspace metadata: {err}")));

        assert!(metadata.root.join("Cargo.toml").exists());
        assert!(metadata.packages.iter().any(|package| {
            package.name == "allow-inventory"
                && package.manifest_path == Path::new("crates/allow-inventory/Cargo.toml")
                && package.targets.iter().any(|target| {
                    target.name == "allow_inventory"
                        && target.kinds.iter().any(|kind| kind == "lib")
                        && target.src_path == Path::new("crates/allow-inventory/src/lib.rs")
                })
        }));
        assert!(metadata.target_count() >= metadata.packages.len());
        assert!(
            metadata
                .source_roots()
                .iter()
                .any(|path| path == Path::new("crates/allow-inventory/src"))
        );
    }

    #[test]
    fn workspace_root_uses_cargo_metadata_from_member_directory() {
        let member_src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let root = discover_workspace_root(member_src)
            .unwrap_or_else(|err| std::panic::panic_any(format!("discover workspace root: {err}")));

        assert!(root.join("Cargo.toml").exists());
        assert!(root.join("crates/allow-inventory/Cargo.toml").exists());
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
