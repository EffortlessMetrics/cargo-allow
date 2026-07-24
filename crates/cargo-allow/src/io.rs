//! Text emission and atomic write shims (#2602-B).
//!
//! Canonical single-target atomic write lives in `repo-edit::atomic_write`.

use allow_core::CargoAllowResult;
use std::path::Path;

pub(crate) use repo_edit::{write_file, write_file_no_overwrite};

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
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
