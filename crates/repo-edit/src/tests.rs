use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::mutation_lock::{MutationLock, lock_path};
use crate::{assert_path_within_root, canonicalize_lexically};

#[test]
fn alias_convergent_paths_acquire_the_same_lock() {
    let root = TempRoot::new("alias-lock")
        .unwrap_or_else(|err| std::panic::panic_any(format!("temp dir: {err}")));
    let direct = root.path().join("policy/allow.toml");
    let aliased = root.path().join("policy/../policy/allow.toml");
    assert_eq!(
        lock_path(&direct),
        lock_path(&aliased),
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

#[test]
fn lexical_canonicalization_matches_lock_identity() {
    let root = TempRoot::new("canonicalize")
        .unwrap_or_else(|err| std::panic::panic_any(format!("temp dir: {err}")));
    let direct = root.path().join("policy/allow.toml");
    let aliased = root.path().join("policy/../policy/allow.toml");
    assert_eq!(
        canonicalize_lexically(&direct),
        canonicalize_lexically(&aliased)
    );
}

#[test]
fn rejects_parent_escape_outside_root() -> Result<(), Box<dyn std::error::Error>> {
    let root = TempRoot::new("containment")?;
    if assert_path_within_root(root.path(), Path::new("../outside.txt")).is_ok() {
        return Err("parent escape should fail".into());
    }
    Ok(())
}

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn new(label: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let path = std::env::temp_dir().join(format!("repo-edit-{label}-{}", std::process::id()));
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
