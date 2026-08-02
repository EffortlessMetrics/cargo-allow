//! Single-target atomic create/replace for repository mutation (#2602-B).

use allow_core::{CargoAllowError, CargoAllowResult};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

/// Atomically write `contents` to `path`: write to a sibling temp file, then
/// `fs::rename` into place. On any error the destination is left untouched.
pub fn write_file(path: impl AsRef<Path>, contents: &str) -> CargoAllowResult<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            CargoAllowError::new(format!("failed to create {}: {e}", parent.display()))
        })?;
    }
    let existing_permissions = match fs::metadata(path) {
        Ok(metadata) => Some(metadata.permissions()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(CargoAllowError::new(format!(
                "failed to inspect {}: {error}",
                path.display()
            )));
        }
    };
    let (tmp, mut file) = create_unique_temp(path)?;
    if let Err(e) = file.write_all(contents.as_bytes()) {
        let _ = fs::remove_file(&tmp);
        return Err(CargoAllowError::new(format!(
            "failed to write {}: {e}",
            tmp.display()
        )));
    }
    if let Some(permissions) = existing_permissions
        && let Err(error) = fs::set_permissions(&tmp, permissions)
    {
        let _ = fs::remove_file(&tmp);
        return Err(CargoAllowError::new(format!(
            "failed to preserve permissions for {}: {error}",
            path.display()
        )));
    }
    if let Err(error) = file.flush() {
        let _ = fs::remove_file(&tmp);
        return Err(CargoAllowError::new(format!(
            "failed to flush {}: {error}",
            tmp.display()
        )));
    }
    if let Err(error) = file.sync_all() {
        let _ = fs::remove_file(&tmp);
        return Err(CargoAllowError::new(format!(
            "failed to sync {}: {error}",
            tmp.display()
        )));
    }
    if let Err(error) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(CargoAllowError::new(format!(
            "failed to install {}: {error}",
            path.display()
        )));
    }
    sync_parent_directory(path)
}

/// Write `contents` to `path` only if it does not already exist (unless
/// `force` is set). With `force`, the existing file is backed up before
/// replacement.
pub fn write_file_no_overwrite(
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
        let bak = path.with_extension("toml.bak");
        let had_backup = path.exists();
        if path.exists() {
            fs::rename(path, &bak).map_err(|e| {
                CargoAllowError::new(format!(
                    "failed to back up {} -> {}: {e}",
                    path.display(),
                    bak.display()
                ))
            })?;
        }
        return match write_file(path, contents) {
            Ok(()) => Ok(()),
            Err(error) if had_backup => {
                if let Err(restore_error) = fs::rename(&bak, path) {
                    return Err(CargoAllowError::new(format!(
                        "{error}; failed to restore {} from {}: {restore_error}",
                        path.display(),
                        bak.display()
                    )));
                }
                sync_parent_directory(path)?;
                Err(error)
            }
            Err(error) => Err(error),
        };
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .truncate(true)
        .open(path)
        .map_err(|e| {
            let _ = e;
            CargoAllowError::new(format!(
                "{} already exists; use --force to overwrite",
                path.display()
            ))
        })?;
    if let Err(e) = file.write_all(contents.as_bytes()) {
        let _ = fs::remove_file(path);
        return Err(CargoAllowError::new(format!(
            "failed to write {}: {e}",
            path.display()
        )));
    }
    file.flush().map_err(|error| {
        let _ = fs::remove_file(path);
        CargoAllowError::new(format!("failed to flush {}: {error}", path.display()))
    })?;
    file.sync_all().map_err(|error| {
        let _ = fs::remove_file(path);
        CargoAllowError::new(format!("failed to sync {}: {error}", path.display()))
    })?;
    sync_parent_directory(path)
}

/// Atomically create `contents` at `path`, failing if the destination already
/// exists. The sibling temporary file is flushed and synced before an atomic
/// hard-link install, so an interrupted write cannot expose partial hook or
/// configuration contents.
pub fn write_file_create_new_atomic(
    path: impl AsRef<Path>,
    contents: &str,
) -> CargoAllowResult<()> {
    write_file_create_new_atomic_with_permissions(path, contents, None)
}

/// Atomic create-only write with optional destination permissions applied to
/// the temporary file before it is installed.
pub fn write_file_create_new_atomic_with_permissions(
    path: impl AsRef<Path>,
    contents: &str,
    permissions: Option<std::fs::Permissions>,
) -> CargoAllowResult<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            CargoAllowError::new(format!(
                "failed to create parent directory {}: {error}",
                parent.display()
            ))
        })?;
    }
    if fs::metadata(path).is_ok() {
        return Err(CargoAllowError::new(format!(
            "{} already exists; refusing to overwrite",
            path.display()
        )));
    }

    let (tmp, mut file) = create_unique_temp(path)?;
    if let Err(error) = file.write_all(contents.as_bytes()) {
        let _ = fs::remove_file(&tmp);
        return Err(CargoAllowError::new(format!(
            "failed to write {}: {error}",
            tmp.display()
        )));
    }
    if let Err(error) = file.flush() {
        let _ = fs::remove_file(&tmp);
        return Err(CargoAllowError::new(format!(
            "failed to flush {}: {error}",
            tmp.display()
        )));
    }
    if let Some(permissions) = permissions
        && let Err(error) = fs::set_permissions(&tmp, permissions)
    {
        let _ = fs::remove_file(&tmp);
        return Err(CargoAllowError::new(format!(
            "failed to set permissions for {}: {error}",
            tmp.display()
        )));
    }
    if let Err(error) = file.sync_all() {
        let _ = fs::remove_file(&tmp);
        return Err(CargoAllowError::new(format!(
            "failed to sync {}: {error}",
            tmp.display()
        )));
    }
    if let Err(error) = fs::hard_link(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(CargoAllowError::new(format!(
            "failed to install {} without overwrite: {error}; create-only installation requires hard-link support on this filesystem",
            path.display()
        )));
    }
    let _ = fs::remove_file(&tmp);
    sync_parent_directory(path)
}

fn create_unique_temp(path: &Path) -> CargoAllowResult<(PathBuf, std::fs::File)> {
    let base_name = path
        .file_name()
        .map(|value| value.to_os_string())
        .unwrap_or_default();
    for _ in 0..1024 {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let mut name = base_name.clone();
        name.push(format!(".tmp-{}-{id}", std::process::id()));
        let candidate = path.with_file_name(name);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => return Ok((candidate, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(CargoAllowError::new(format!(
                    "failed to open {}: {error}",
                    candidate.display()
                )));
            }
        }
    }
    Err(CargoAllowError::new(format!(
        "failed to allocate a unique temporary file beside {}",
        path.display()
    )))
}

#[cfg(unix)]
fn sync_parent_directory(path: &Path) -> CargoAllowResult<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let directory = OpenOptions::new()
        .read(true)
        .open(parent)
        .map_err(|error| {
            CargoAllowError::new(format!(
                "failed to open parent directory {} for sync: {error}",
                parent.display()
            ))
        })?;
    directory.sync_all().map_err(|error| {
        CargoAllowError::new(format!(
            "failed to sync parent directory {}: {error}",
            parent.display()
        ))
    })
}

#[cfg(not(unix))]
fn sync_parent_directory(_path: &Path) -> CargoAllowResult<()> {
    Ok(())
}

#[cfg(test)]
pub(crate) fn sibling_tmp_path(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|s| s.to_os_string())
        .unwrap_or_default();
    name.push(".tmp");
    path.with_file_name(name)
}
