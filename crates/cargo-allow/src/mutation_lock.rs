use allow_core::{CargoAllowError, CargoAllowResult};
use fs4::fs_std::FileExt;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};

/// Cross-process lock for a single cargo-allow mutation target.
///
/// The lock is held by the returned guard until it is dropped. Lock files live
/// under the operating system temp directory rather than beside the policy,
/// so the coordination artifact cannot become a new source-tree finding. The
/// file is intentionally retained there: the operating-system lock, rather
/// than the file's presence, represents ownership, so a terminated writer
/// cannot strand future mutations behind a stale path.
#[derive(Debug)]
pub(crate) struct MutationLock {
    _file: File,
}

impl MutationLock {
    pub(crate) fn acquire(target: impl AsRef<Path>) -> CargoAllowResult<Self> {
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

fn lock_path(target: &Path) -> PathBuf {
    let absolute_target = if target.is_absolute() {
        target.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(target))
            .unwrap_or_else(|_| target.to_path_buf())
    };
    std::env::temp_dir().join("cargo-allow-locks").join(format!(
        "{}.lock",
        allow_core::stable_hash_hex(&absolute_target.to_string_lossy())
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn lock_is_released_when_guard_drops() -> Result<(), Box<dyn std::error::Error>> {
        let root = TempRoot::new("mutation-lock")?;
        let target = root.path().join("policy/allow.toml");
        let first = MutationLock::acquire(&target)?;
        let (ready_tx, ready_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let target_for_thread = target.clone();
        let worker = thread::spawn(move || -> Result<(), String> {
            ready_tx.send(()).map_err(|error| error.to_string())?;
            let second =
                MutationLock::acquire(&target_for_thread).map_err(|error| error.to_string())?;
            release_rx.recv().map_err(|error| error.to_string())?;
            drop(second);
            Ok(())
        });
        ready_rx.recv_timeout(Duration::from_secs(2))?;
        thread::sleep(Duration::from_millis(50));
        if release_tx.send(()).is_err() {
            return Err("failed to release lock worker".into());
        }
        drop(first);
        worker
            .join()
            .map_err(|_| "lock worker panicked")?
            .map_err(Box::<dyn std::error::Error>::from)?;
        let _lock = MutationLock::acquire(&target)?;
        if !lock_path(&target).exists() {
            return Err("lock path was not created".into());
        }
        Ok(())
    }

    struct TempRoot {
        path: PathBuf,
    }

    impl TempRoot {
        fn new(label: &str) -> Result<Self, Box<dyn std::error::Error>> {
            let path =
                std::env::temp_dir().join(format!("cargo-allow-{label}-{}", std::process::id()));
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
