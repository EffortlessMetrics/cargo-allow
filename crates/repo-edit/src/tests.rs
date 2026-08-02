use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::atomic_write::sibling_tmp_path;
use crate::mutation_lock::{MutationLock, lock_path};
use crate::{
    assert_path_within_root, canonicalize_lexically, write_file, write_file_create_new_atomic,
    write_file_no_overwrite,
};

#[cfg(unix)]
use crate::write_file_create_new_atomic_with_permissions;

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
fn acquire_with_timeout_returns_descriptive_error_on_timeout() {
    let root = TempRoot::new("lock-timeout")
        .unwrap_or_else(|err| std::panic::panic_any(format!("temp dir: {err}")));
    let target = root.path().join("policy/allow.toml");
    let _first = MutationLock::acquire(&target)
        .unwrap_or_else(|err| std::panic::panic_any(format!("acquire first: {err}")));

    let result = MutationLock::acquire_with_timeout(&target, Duration::from_millis(300));

    let err = result.expect_err("should timeout when lock is held");
    let message = err.to_string();
    assert!(
        message.contains("held by another process"),
        "timeout error should mention held lock: {message}"
    );
    assert!(
        message.contains("stale processes"),
        "timeout error should suggest checking for stale processes: {message}"
    );
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

#[test]
fn write_file_reports_parent_creation_errors() -> Result<(), Box<dyn std::error::Error>> {
    let root = TempRoot::new("write-parent-error")?;
    let file_parent = root.path().join("not-a-directory");
    fs::write(&file_parent, "already a file")?;
    let output = file_parent.join("report.txt");
    fs::create_dir_all(&file_parent).expect_err("creating a directory over a file should fail");

    let err = write_file(&output, "contents").expect_err("parent creation should fail");
    let message = err.to_string();

    assert!(message.contains("failed to create"));
    assert!(message.contains(&file_parent.display().to_string()));
    assert_eq!(err.kind(), allow_core::CargoAllowErrorKind::Unknown);
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
fn write_file_create_new_atomic_never_replaces_existing_path()
-> Result<(), Box<dyn std::error::Error>> {
    let root = TempRoot::new("atomic-create-only")?;
    let output = root.path().join("hooks/pre-commit");

    write_file_create_new_atomic(&output, "first\n")?;
    let err = write_file_create_new_atomic(&output, "replacement\n")
        .expect_err("atomic create-only write should reject an existing target");

    assert!(err.to_string().contains("refusing to overwrite"));
    assert_eq!(fs::read_to_string(&output)?, "first\n");
    Ok(())
}

#[test]
fn write_file_create_new_atomic_reports_parent_creation_failure()
-> Result<(), Box<dyn std::error::Error>> {
    let root = TempRoot::new("atomic-create-parent-error")?;
    let parent = root.path().join("not-a-directory");
    fs::write(&parent, "already a file")?;
    let output = parent.join("pre-commit");

    let error = write_file_create_new_atomic(&output, "hook\n")
        .expect_err("atomic create should reject a file as the parent directory");
    if !error
        .to_string()
        .contains("failed to create parent directory")
    {
        return Err("parent creation error omitted its operation".into());
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn write_file_create_new_atomic_applies_requested_permissions()
-> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::PermissionsExt;

    let root = TempRoot::new("atomic-create-only-mode")?;
    let output = root.path().join("hooks/pre-commit");

    write_file_create_new_atomic_with_permissions(
        &output,
        "#!/bin/sh\nexit 0\n",
        Some(fs::Permissions::from_mode(0o755)),
    )?;

    let mode = fs::metadata(&output)?.permissions().mode() & 0o777;
    assert_eq!(mode, 0o755);
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
fn write_file_recoverable_after_stale_temp_from_prior_crash()
-> Result<(), Box<dyn std::error::Error>> {
    let root = TempRoot::new("stale-temp-recover")?;
    let target = root.path().join("policy/allow.toml");
    let tmp = sibling_tmp_path(&target);

    let parent = target
        .parent()
        .unwrap_or_else(|| std::panic::panic_any("test target must have a parent directory"));
    fs::create_dir_all(parent)?;
    fs::write(&tmp, "leftover from a crashed write")?;

    write_file(&target, "the real content")?;

    assert_eq!(fs::read_to_string(&target)?, "the real content");
    assert!(
        tmp.exists(),
        "an abandoned temp must not be removed blindly"
    );
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
