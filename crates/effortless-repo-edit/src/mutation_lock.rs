use crate::error::{RepoEditError, RepoEditResult};
use std::fs::TryLockError;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::target_identity::canonicalize_lexically;

/// Default timeout for acquiring a mutation lock (30 seconds).
const DEFAULT_LOCK_TIMEOUT: Duration = Duration::from_secs(30);

/// Polling interval between lock attempts.
const LOCK_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Cross-process lock for a single repository mutation target.
#[derive(Debug)]
pub struct MutationLock {
    file: Option<File>,
    lock_path: Option<PathBuf>,
}

impl MutationLock {
    pub fn acquire(target: impl AsRef<Path>) -> RepoEditResult<Self> {
        Self::acquire_with_timeout(target, DEFAULT_LOCK_TIMEOUT)
    }

    /// Acquire the lock with a custom timeout, polling with `try_lock` instead
    /// of blocking indefinitely. On timeout, returns a descriptive error so the
    /// operator knows the lock is held by another process, not stuck (#2831).
    pub fn acquire_with_timeout(
        target: impl AsRef<Path>,
        timeout: Duration,
    ) -> RepoEditResult<Self> {
        let target = target.as_ref();
        let path = lock_path(target);
        Self::acquire_at_path(path, timeout)
    }

    /// Acquire the lock for a resolved MutationTarget, using its canonical
    /// fingerprint for the lock key (#2487/#2489). This ensures that path
    /// aliases (relative/absolute, dot-dot, symlinks) share one lock file.
    pub fn acquire_for_target(
        target: &crate::mutation_target::MutationTarget,
    ) -> RepoEditResult<Self> {
        let path = crate::mutation_target::lock_path_for_target(target);
        Self::acquire_at_path(path, DEFAULT_LOCK_TIMEOUT)
    }

    fn acquire_at_path(path: PathBuf, timeout: Duration) -> RepoEditResult<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                RepoEditError::new(format!(
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
                RepoEditError::new(format!(
                    "failed to open mutation lock {}: {error}",
                    path.display()
                ))
            })?;
        let start = Instant::now();
        loop {
            match file.try_lock() {
                Ok(()) => {
                    return Ok(Self {
                        file: Some(file),
                        lock_path: Some(path),
                    });
                }
                Err(TryLockError::WouldBlock) => {
                    if start.elapsed() >= timeout {
                        return Err(RepoEditError::new(format!(
                            "mutation lock held by another process; waited {}s for {}; \
                             another cargo-allow mutation (add/propose/refresh/prune/migrate/init) \
                             may be running; check for stale processes or rerun after it completes",
                            timeout.as_secs(),
                            path.display()
                        )));
                    }
                    std::thread::sleep(LOCK_POLL_INTERVAL);
                }
                Err(TryLockError::Error(error)) => {
                    return Err(RepoEditError::new(format!(
                        "failed to acquire mutation lock {}: {error}; \
                         ensure the directory is writable and not on a read-only filesystem",
                        path.display()
                    )));
                }
            }
        }
    }
}

impl Drop for MutationLock {
    /// Release the OS advisory lock (via File drop) and clean up the lock file
    /// from temp to prevent unbounded accumulation (#2781).
    fn drop(&mut self) {
        // Drop the file handle first to release the OS advisory lock.
        // On Windows, the lock must be released before the file can be deleted.
        self.file.take();
        // Best-effort cleanup: remove the lock file from temp.
        if let Some(path) = self.lock_path.take() {
            let _ = fs::remove_file(&path);
        }
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
        crate::error::stable_hash_hex(&canonical.to_string_lossy())
    ))
}
