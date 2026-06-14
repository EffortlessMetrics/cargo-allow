use allow_core::{CargoAllowError, CargoAllowResult};
use std::path::{Path, PathBuf};

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
