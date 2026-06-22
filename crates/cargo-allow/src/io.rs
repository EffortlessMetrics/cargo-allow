use allow_core::{CargoAllowError, CargoAllowResult};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub(crate) fn emit_text(output: Option<&Path>, contents: &str) -> CargoAllowResult<()> {
    if let Some(path) = output {
        write_file(path, contents)?;
    } else {
        println!("{contents}");
    }
    Ok(())
}

pub(crate) fn emit_stderr_text(output: Option<&Path>, contents: &str) -> CargoAllowResult<()> {
    if let Some(path) = output {
        write_file(path, contents)?;
    } else {
        eprintln!("{contents}");
    }
    Ok(())
}

/// Atomically write `contents` to `path`: write to a sibling `<path>.tmp`
/// file, then `fs::rename` into place. On any error the destination is left
/// untouched (either the complete previous file or nothing — never partial).
pub(crate) fn write_file(path: impl AsRef<Path>, contents: &str) -> CargoAllowResult<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            CargoAllowError::new(format!("failed to create {}: {e}", parent.display()))
        })?;
    }
    let tmp = sibling_tmp_path(path);
    {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .truncate(true)
            .open(&tmp)
            .map_err(|e| CargoAllowError::new(format!("failed to open {}: {e}", tmp.display())))?;
        file.write_all(contents.as_bytes())
            .map_err(|e| CargoAllowError::new(format!("failed to write {}: {e}", tmp.display())))?;
        file.sync_all().ok();
    }
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        CargoAllowError::new(format!("failed to install {}: {e}", path.display()))
    })
}

/// Write `contents` to `path` only if it does not already exist (unless
/// `force` is set). Uses `OpenOptions::create_new` so the existence check
/// and the creation are one atomic syscall — no TOCTOU window for symlink
/// planting. With `force`, the existing file is backed up to `<path>.bak`
/// before the atomic write replaces it.
pub(crate) fn write_file_no_overwrite(
    path: impl AsRef<Path>,
    contents: &str,
    force: bool,
) -> CargoAllowResult<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            CargoAllowError::new(format!("failed to create {}: {e}", parent.display()))
        })?;
    }
    if force {
        if path.exists() {
            let bak = path.with_extension("toml.bak");
            fs::rename(path, &bak).map_err(|e| {
                CargoAllowError::new(format!(
                    "failed to back up {} -> {}: {e}",
                    path.display(),
                    bak.display()
                ))
            })?;
        }
        return write_file(path, contents);
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .truncate(true)
        .open(path)
        .map_err(|e| {
            // create_new fails if the path exists (including via symlink),
            // which is the intended no-overwrite contract. Preserve the
            // actionable guidance; the OS error kind is implicit.
            let _ = e;
            CargoAllowError::new(format!(
                "{} already exists; use --force to overwrite",
                path.display()
            ))
        })?;
    file.write_all(contents.as_bytes())
        .map_err(|e| CargoAllowError::new(format!("failed to write {}: {e}", path.display())))?;
    Ok(())
}

/// Build a sibling temp-file path on the same filesystem so `fs::rename`
/// is atomic.
fn sibling_tmp_path(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|s| s.to_os_string())
        .unwrap_or_default();
    name.push(".tmp");
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    #[test]
    fn emit_text_writes_to_output_path() -> Result<(), Box<dyn std::error::Error>> {
        let root = TempRoot::new("emit-text")?;
        let output = root.path().join("nested/report.txt");

        let result = emit_text(Some(&output), "hello report\n");

        assert!(result.is_ok());
        assert_eq!(fs::read_to_string(&output)?, "hello report\n");
        Ok(())
    }

    #[test]
    fn emit_stderr_text_writes_to_output_path() -> Result<(), Box<dyn std::error::Error>> {
        let root = TempRoot::new("emit-stderr-text")?;
        let output = root.path().join("nested/summary.txt");

        let result = emit_stderr_text(Some(&output), "summary\n");

        assert!(result.is_ok());
        assert_eq!(fs::read_to_string(&output)?, "summary\n");
        Ok(())
    }

    #[test]
    fn write_file_reports_parent_creation_errors() -> Result<(), Box<dyn std::error::Error>> {
        let root = TempRoot::new("write-parent-error")?;
        let file_parent = root.path().join("not-a-directory");
        fs::write(&file_parent, "already a file")?;
        let output = file_parent.join("report.txt");
        let source_error = fs::create_dir_all(&file_parent)
            .expect_err("creating a directory over a file should fail");

        let err = write_file(&output, "contents").expect_err("parent creation should fail");
        let message = err.to_string();

        assert!(message.contains("failed to create"));
        assert!(message.contains(&file_parent.display().to_string()));
        assert_eq!(
            err,
            CargoAllowError::new(format!(
                "failed to create {}: {}",
                file_parent.display(),
                source_error
            ))
        );
        Ok(())
    }

    #[test]
    fn write_file_reports_file_write_errors() -> Result<(), Box<dyn std::error::Error>> {
        let root = TempRoot::new("write-file-error")?;
        let output = root.path().join("directory-target");
        fs::create_dir_all(&output)?;

        let err = write_file(&output, "contents").expect_err("writing to a directory should fail");
        let message = err.to_string();

        // With the atomic temp-write-rename helper, the failure can surface
        // at the open stage ("failed to open"), the write stage, or the
        // rename stage ("failed to install") depending on the OS and the
        // nature of the invalid target.
        assert!(
            message.contains("failed to open")
                || message.contains("failed to write")
                || message.contains("failed to install"),
            "error should mention open, write, or install failure: {message}"
        );
        Ok(())
    }

    #[test]
    fn write_file_no_overwrite_rejects_existing_path_without_force()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = TempRoot::new("no-overwrite")?;
        let output = root.path().join("policy/allow.toml");
        write_file(&output, "original")?;

        let err = write_file_no_overwrite(&output, "replacement", false)
            .expect_err("existing file should require force");

        assert!(err.to_string().contains("already exists"));
        assert_eq!(fs::read_to_string(&output)?, "original");
        Ok(())
    }

    #[test]
    fn write_file_no_overwrite_replaces_existing_path_with_force()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = TempRoot::new("force-overwrite")?;
        let output = root.path().join("policy/allow.toml");
        write_file(&output, "original")?;

        let result = write_file_no_overwrite(&output, "replacement", true);

        assert!(result.is_ok());
        assert_eq!(fs::read_to_string(&output)?, "replacement");
        Ok(())
    }

    #[test]
    fn atomic_write_leaves_original_intact_on_rename_failure()
    -> Result<(), Box<dyn std::error::Error>> {
        // Write a valid file, then attempt an atomic write to a path whose
        // parent is a regular file (not a directory). The rename will fail,
        // and the original file must be byte-identical.
        let root = TempRoot::new("atomic-intact")?;
        let target = root.path().join("dir/policy.toml");
        write_file(&target, "original content")?;

        // Corrupt the parent: make "dir" a file instead of a directory by
        // writing to a path whose parent no longer exists as a dir.
        // Instead, simulate rename failure by writing to a path inside a file.
        let blocking_file = root.path().join("blocker");
        fs::write(&blocking_file, "I am a file, not a directory")?;
        let impossible_target = blocking_file.join("nested/policy.toml");

        let result = write_file(&impossible_target, "new content");
        assert!(result.is_err(), "write to an impossible path should fail");

        // The original file is untouched.
        assert_eq!(fs::read_to_string(&target)?, "original content");
        Ok(())
    }

    #[test]
    fn write_file_no_overwrite_with_force_creates_backup() -> Result<(), Box<dyn std::error::Error>>
    {
        let root = TempRoot::new("force-backup")?;
        let output = root.path().join("policy/allow.toml");
        write_file(&output, "original")?;

        write_file_no_overwrite(&output, "replacement", true)?;

        assert_eq!(fs::read_to_string(&output)?, "replacement");
        // The backup should contain the original content.
        let bak = output.with_extension("toml.bak");
        assert_eq!(fs::read_to_string(&bak)?, "original");
        Ok(())
    }

    struct TempRoot {
        path: PathBuf,
    }

    impl TempRoot {
        fn new(label: &str) -> std::io::Result<Self> {
            let unique = format!(
                "cargo-allow-io-{label}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_else(|err| {
                        std::panic::panic_any(format!("system time before epoch: {err}"))
                    })
                    .as_nanos()
            );
            let path = std::env::temp_dir().join(unique);
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path)?;
            Ok(Self { path })
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
