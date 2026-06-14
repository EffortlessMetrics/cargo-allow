use allow_core::{CargoAllowError, CargoAllowResult};
use std::fs;
use std::path::Path;

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

pub(crate) fn write_file(path: impl AsRef<Path>, contents: &str) -> CargoAllowResult<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            CargoAllowError::new(format!("failed to create {}: {e}", parent.display()))
        })?;
    }
    fs::write(path, contents)
        .map_err(|e| CargoAllowError::new(format!("failed to write {}: {e}", path.display())))
}

pub(crate) fn write_file_no_overwrite(
    path: impl AsRef<Path>,
    contents: &str,
    force: bool,
) -> CargoAllowResult<()> {
    let path = path.as_ref();
    if path.exists() && !force {
        return Err(CargoAllowError::new(format!(
            "{} already exists; use --force to overwrite",
            path.display()
        )));
    }
    write_file(path, contents)
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
        let source_error =
            fs::write(&output, "contents").expect_err("writing to a directory should fail");

        let err = write_file(&output, "contents").expect_err("writing to a directory should fail");
        let message = err.to_string();

        assert!(message.contains("failed to write"));
        assert!(message.contains(&output.display().to_string()));
        assert_eq!(
            err,
            CargoAllowError::new(format!(
                "failed to write {}: {}",
                output.display(),
                source_error
            ))
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
