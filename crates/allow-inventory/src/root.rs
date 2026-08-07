use allow_core::{CargoAllowError, CargoAllowResult};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

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
    discover_source_tree_root_with_git_env(&start, &[])
}

fn discover_source_tree_root_with_git_env(
    start: &Path,
    extra_env: &[(OsString, OsString)],
) -> CargoAllowResult<PathBuf> {
    if let Some(root) = git_source_tree_root(start, extra_env)? {
        return Ok(root);
    }
    discover_source_tree_root_by_marker(start)
}

fn discover_source_tree_root_by_marker(start: &Path) -> CargoAllowResult<PathBuf> {
    let mut dir = start;
    loop {
        if dir.join(".git").exists() {
            return Ok(dir.to_path_buf());
        }
        let Some(parent) = dir.parent() else {
            return Ok(start.to_path_buf());
        };
        dir = parent;
    }
}

fn git_source_tree_root(
    start: &Path,
    extra_env: &[(OsString, OsString)],
) -> CargoAllowResult<Option<PathBuf>> {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(start)
        .arg("rev-parse")
        .arg("--show-toplevel");
    for (key, value) in extra_env {
        command.env(key, value);
    }
    let Ok(output) = command.output() else {
        return Ok(None);
    };
    if !output.status.success() {
        return Ok(None);
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let Some(root) = stdout.lines().find(|line| !line.trim().is_empty()) else {
        return Ok(None);
    };
    canonical_dir(Path::new(root.trim())).map(Some)
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
            allow_core::normalize_path(&canonical)
        )))
    }
}

fn canonical_start_dir(start: &Path) -> CargoAllowResult<PathBuf> {
    let canonical = start.canonicalize().map_err(|e| {
        CargoAllowError::new(format!(
            "failed to canonicalize start path '{}': {e}",
            allow_core::normalize_path(start)
        ))
    })?;
    if canonical.is_file() {
        canonical
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| CargoAllowError::new("start path has no parent directory"))
    } else {
        Ok(canonical)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn canonical_dir_accepts_existing_directory_and_rejects_files()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = temp_root("canonical-dir")?;
        let marker = root.join("Cargo.toml");
        fs::write(&marker, "[workspace]\n")?;

        let canonical = canonical_dir(&root)?;
        let err = canonical_dir(&marker)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("file root should fail"));

        assert_eq!(canonical, root.canonicalize()?);
        assert!(
            err.to_string()
                .contains("source tree root is not a directory"),
            "unexpected error: {err}"
        );
        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn canonical_start_dir_uses_file_parent_and_preserves_directory_starts()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = temp_root("canonical-start")?;
        let nested = root.join("src").join("nested");
        fs::create_dir_all(&nested)?;
        let file = nested.join("lib.rs");
        fs::write(&file, "pub fn demo() {}\n")?;

        let from_file = canonical_start_dir(&file)?;
        let from_dir = canonical_start_dir(&nested)?;
        let err = canonical_start_dir(&root.join("missing"))
            .err()
            .unwrap_or_else(|| std::panic::panic_any("missing start should fail"));

        assert_eq!(from_file, nested.canonicalize()?);
        assert_eq!(from_dir, nested.canonicalize()?);
        assert!(
            err.to_string()
                .contains("failed to canonicalize start path"),
            "unexpected error: {err}"
        );
        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn git_source_tree_root_honors_git_work_tree_environment()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = temp_root("git-work-tree-env")?;
        let git_dir = root.join("repo.git");
        let work_tree = root.join("work-tree");
        let unrelated_start = root.join("runner").join("job");
        fs::create_dir_all(&work_tree)?;
        fs::create_dir_all(&unrelated_start)?;
        let status = Command::new("git")
            .arg("-C")
            .arg(&root)
            .arg("init")
            .arg("--bare")
            .arg(&git_dir)
            .status()?;
        if !status.success() {
            return Err(format!("git init --bare failed: {status}").into());
        }
        let env = vec![
            (
                OsString::from("GIT_DIR"),
                git_dir.as_os_str().to_os_string(),
            ),
            (
                OsString::from("GIT_WORK_TREE"),
                work_tree.as_os_str().to_os_string(),
            ),
        ];

        let discovered = discover_source_tree_root_with_git_env(&unrelated_start, &env)?;

        assert_eq!(discovered, work_tree.canonicalize()?);
        fs::remove_dir_all(root)?;
        Ok(())
    }

    fn temp_root(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "cargo-allow-root-{label}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&root)?;
        Ok(root)
    }
}
