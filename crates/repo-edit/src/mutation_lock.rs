use allow_core::{CargoAllowError, CargoAllowResult};
use fs4::fs_std::FileExt;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};

use crate::target_identity::canonicalize_lexically;

/// Cross-process lock for a single repository mutation target.
#[derive(Debug)]
pub struct MutationLock {
    _file: File,
}

impl MutationLock {
    pub fn acquire(target: impl AsRef<Path>) -> CargoAllowResult<Self> {
        let target = target.as_ref();
        let path = lock_path(target);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                CargoAllowError::new(format!(
                    "failed to create mutation lock directory {}: {error}",
                    parent.display()
                ))
            })?;
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|error| {
                CargoAllowError::new(format!(
                    "failed to open mutation lock {}: {error}",
                    path.display()
                ))
            })?;
        file.lock_exclusive().map_err(|error| {
            CargoAllowError::new(format!(
                "failed to acquire mutation lock {}: {error}",
                path.display()
            ))
        })?;
        Ok(Self { _file: file })
    }
}

pub(crate) fn lock_path(target: &Path) -> PathBuf {
    let absolute_target = if target.is_absolute() {
        target.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(target))
            .unwrap_or_else(|_| target.to_path_buf())
    };
    let canonical = canonicalize_lexically(&absolute_target);
    std::env::temp_dir().join("cargo-allow-locks").join(format!(
        "{}.lock",
        allow_core::stable_hash_hex(&canonical.to_string_lossy())
    ))
}
