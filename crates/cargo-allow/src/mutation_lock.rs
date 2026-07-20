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

/// Compute a deterministic lock file path for a mutation target.
///
/// The target path is canonicalized (lexically — `.`/`..` folded, verbatim
/// prefix stripped) before hashing so that alias-convergent spellings of the
/// same file acquire the same lock. This prevents lost-update races where
/// two processes address the same policy file through different path
/// representations (#2487-#2491).
///
/// Full filesystem canonicalization (`std::fs::canonicalize`) is avoided
/// because it requires the file to exist at lock acquisition time, and
/// `init` creates the file as part of the mutation.
fn lock_path(target: &Path) -> PathBuf {
    let absolute_target = if target.is_absolute() {
        target.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(target))
            .unwrap_or_else(|_| target.to_path_buf())
    };
    // Lexical canonicalization: strip verbatim prefix, normalize . and ..
    let canonical = canonicalize_lexically(&absolute_target);
    std::env::temp_dir().join("cargo-allow-locks").join(format!(
        "{}.lock",
        allow_core::stable_hash_hex(&canonical.to_string_lossy())
    ))
}

/// Lexically normalize a path by stripping the Windows verbatim prefix and
/// folding `.`/`..` components. This is NOT a filesystem canonicalize — it
/// does not resolve symlinks. It ensures that `policy/allow.toml` and
/// `policy/../policy/allow.toml` produce the same string.
fn canonicalize_lexically(path: &Path) -> PathBuf {
    use std::path::Component;

    // Strip verbatim prefix if present (Windows \\?\)
    let stripped = crate::policy_config::strip_verbatim_prefix(path);

    let mut components = Vec::new();
    for component in stripped.components() {
        match component {
            Component::CurDir => { /* skip . */ }
            Component::ParentDir => {
                // Pop the last Normal component if one exists; don't pop
                // past a RootDir or Prefix.
                match components.last() {
                    Some(Component::Normal(_)) => {
                        components.pop();
                    }
                    _ => {
                        // Leading .. or .. past root — keep it
                        components.push(component);
                    }
                }
            }
            other => {
                components.push(other);
            }
        }
    }

    // Rebuild from components
    let mut result = PathBuf::new();
    for component in &components {
        result.push(component.as_os_str());
    }
    if result.as_os_str().is_empty() {
        stripped
    } else {
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn alias_convergent_paths_acquire_the_same_lock() {
        // #2487: policy/allow.toml and policy/../policy/allow.toml must
        // produce the same lock path so two processes can't bypass each
        // other's lock by spelling the path differently.
        let root = TempRoot::new("alias-lock")
            .unwrap_or_else(|err| std::panic::panic_any(format!("temp dir: {err}")));
        let direct = root.path().join("policy/allow.toml");
        let aliased = root.path().join("policy/../policy/allow.toml");

        let direct_lock = lock_path(&direct);
        let aliased_lock = lock_path(&aliased);

        assert_eq!(
            direct_lock, aliased_lock,
            "alias-convergent paths must produce the same lock file"
        );
    }

    #[test]
    fn dot_slash_normalization_produces_same_lock() {
        let root = TempRoot::new("dot-lock")
            .unwrap_or_else(|err| std::panic::panic_any(format!("temp dir: {err}")));
        let direct = root.path().join("policy/allow.toml");
        let dotted = root.path().join("./policy/./allow.toml");

        assert_eq!(
            lock_path(&direct),
            lock_path(&dotted),
            "./ normalization must produce the same lock file"
        );
    }

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
