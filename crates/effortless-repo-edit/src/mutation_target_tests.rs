//! Tests for canonical mutation target identity (#2489).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};

use super::mutation_target::{
    MutationTargetOwnership, lock_path_for_target, resolve_mutation_target,
};

/// Serializes the tests that mutate the process-wide current directory.
///
/// The current directory is per-process, not per-thread, so two of these
/// tests running concurrently would resolve each other's relative spellings
/// against the wrong repository root. Because every temp repo lays out the
/// same `policy/allow.toml`, that misresolution succeeds silently and shows
/// up only as a mismatched fingerprint.
fn cwd_guard() -> CwdGuard {
    static CWD_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let lock = CWD_LOCK.get_or_init(|| Mutex::new(()));
    // A panicking test poisons the lock; the guarded state is a unit value
    // with no invariants to corrupt, so recovering keeps one failure from
    // cascading into every other test in this module.
    let guard = match lock.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    CwdGuard {
        original: std::env::current_dir().ok(),
        _lock: guard,
    }
}

/// Holds the current-directory lock and restores the entry directory on drop.
///
/// Restoring matters because each of these tests deletes the temp repo it
/// changed into; without this, every later test in the binary would inherit
/// a current directory that no longer exists.
struct CwdGuard {
    original: Option<PathBuf>,
    _lock: MutexGuard<'static, ()>,
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        if let Some(original) = &self.original {
            std::env::set_current_dir(original).ok();
        }
    }
}

fn make_temp_repo() -> Result<PathBuf, String> {
    // A monotonic counter, not just a timestamp: the system clock is coarse
    // on Windows (tens of milliseconds), so two temp repos created in the
    // same tick would otherwise collide on one directory and delete each
    // other's fixtures on cleanup.
    static NEXT_ID: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "mutation-target-test-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&dir).map_err(|e| format!("create temp repo: {e}"))?;
    Ok(dir)
}

#[test]
fn relative_and_absolute_spellings_produce_same_fingerprint() -> Result<(), String> {
    let _cwd = cwd_guard();
    let repo = make_temp_repo()?;
    let file_path = repo.join("policy/allow.toml");
    fs::create_dir_all(file_path.parent().unwrap_or(Path::new("."))).ok();
    fs::write(&file_path, "test").ok();

    // Absolute spelling.
    let target_abs = resolve_mutation_target(&file_path, &repo).map_err(|e| e.to_string())?;
    // Relative spelling (from cwd = repo).
    std::env::set_current_dir(&repo).ok();
    let target_rel = resolve_mutation_target(PathBuf::from("policy/allow.toml").as_path(), &repo)
        .map_err(|e| e.to_string())?;

    assert_eq!(
        target_abs.target_fingerprint(),
        target_rel.target_fingerprint(),
        "relative and absolute spellings should produce the same fingerprint"
    );
    assert_eq!(
        target_abs.ownership(),
        MutationTargetOwnership::SourceTreeOwned
    );
    fs::remove_dir_all(&repo).ok();
    Ok(())
}

#[test]
fn dot_dot_aliases_produce_same_fingerprint() -> Result<(), String> {
    let _cwd = cwd_guard();
    let repo = make_temp_repo()?;
    let file_path = repo.join("policy/allow.toml");
    fs::create_dir_all(file_path.parent().unwrap_or(Path::new("."))).ok();
    fs::write(&file_path, "test").ok();

    let target_a = resolve_mutation_target(&file_path, &repo).map_err(|e| e.to_string())?;
    // Use policy/../policy/allow.toml from within repo
    std::env::set_current_dir(&repo).ok();
    let dotted = PathBuf::from("policy/../policy/allow.toml");
    let target_b = resolve_mutation_target(&dotted, &repo).map_err(|e| e.to_string())?;

    assert_eq!(
        target_a.target_fingerprint(),
        target_b.target_fingerprint(),
        "dot-dot aliases should produce the same fingerprint"
    );
    fs::remove_dir_all(&repo).ok();
    Ok(())
}

#[test]
fn not_yet_existing_target_resolves_via_parent() -> Result<(), String> {
    let repo = make_temp_repo()?;
    let dir = repo.join("policy");
    fs::create_dir_all(&dir).ok();
    let missing = dir.join("new-allow.toml");

    let target = resolve_mutation_target(&missing, &repo).map_err(|e| e.to_string())?;
    assert_eq!(target.ownership(), MutationTargetOwnership::SourceTreeOwned);
    assert_eq!(target.repo_relative_display(), "policy/new-allow.toml");
    fs::remove_dir_all(&repo).ok();
    Ok(())
}

#[test]
fn out_of_tree_target_is_classified() -> Result<(), String> {
    let repo = make_temp_repo()?;
    let outside = std::env::temp_dir().join("outside-target-test.toml");
    fs::write(&outside, "test").ok();

    let target = resolve_mutation_target(&outside, &repo).map_err(|e| e.to_string())?;
    assert_eq!(
        target.ownership(),
        MutationTargetOwnership::OutsideSourceTree
    );
    fs::remove_file(&outside).ok();
    fs::remove_dir_all(&repo).ok();
    Ok(())
}

#[cfg(unix)]
#[test]
fn parent_symlink_target_is_classified_outside_source_tree() -> Result<(), String> {
    let repo = make_temp_repo()?;
    let outside = make_temp_repo()?;
    let policy_dir = repo.join("policy");
    let foreign = outside.join("allow.toml");
    fs::write(&foreign, "foreign sentinel").map_err(|e| e.to_string())?;
    std::os::unix::fs::symlink(&outside, &policy_dir).map_err(|e| e.to_string())?;

    let target = resolve_mutation_target(Path::new("policy/allow.toml"), &repo)
        .map_err(|e| e.to_string())?;
    assert_eq!(
        target.ownership(),
        MutationTargetOwnership::OutsideSourceTree,
        "a symlinked parent must not make a foreign target source-tree owned"
    );
    assert_eq!(
        target.normalized_absolute(),
        foreign.canonicalize().map_err(|e| e.to_string())?
    );
    assert_eq!(
        fs::read_to_string(&foreign).map_err(|e| e.to_string())?,
        "foreign sentinel"
    );
    fs::remove_dir_all(&repo).map_err(|e| e.to_string())?;
    fs::remove_dir_all(&outside).map_err(|e| e.to_string())?;
    Ok(())
}

#[test]
fn distinct_files_have_distinct_fingerprints() -> Result<(), String> {
    let repo = make_temp_repo()?;
    let file_a = repo.join("a.toml");
    let file_b = repo.join("b.toml");
    fs::write(&file_a, "a").ok();
    fs::write(&file_b, "b").ok();

    let target_a = resolve_mutation_target(&file_a, &repo).map_err(|e| e.to_string())?;
    let target_b = resolve_mutation_target(&file_b, &repo).map_err(|e| e.to_string())?;

    assert_ne!(
        target_a.target_fingerprint(),
        target_b.target_fingerprint(),
        "distinct files must have distinct fingerprints"
    );
    fs::remove_dir_all(&repo).ok();
    Ok(())
}

#[test]
fn repo_relative_display_excludes_absolute_path() -> Result<(), String> {
    let repo = make_temp_repo()?;
    let file_path = repo.join("policy/allow.toml");
    fs::create_dir_all(file_path.parent().unwrap_or(Path::new("."))).ok();
    fs::write(&file_path, "test").ok();

    let target = resolve_mutation_target(&file_path, &repo).map_err(|e| e.to_string())?;
    let display = target.repo_relative_display();
    assert!(
        !display.contains(':'),
        "repo_relative_display should not contain drive letters or absolute prefixes: {display}"
    );
    assert_eq!(display, "policy/allow.toml");
    fs::remove_dir_all(&repo).ok();
    Ok(())
}

#[test]
fn lock_key_matches_for_same_target() -> Result<(), String> {
    let _cwd = cwd_guard();
    let repo = make_temp_repo()?;
    let file_path = repo.join("policy/allow.toml");
    fs::create_dir_all(file_path.parent().unwrap_or(Path::new("."))).ok();
    fs::write(&file_path, "test").ok();

    let target = resolve_mutation_target(&file_path, &repo).map_err(|e| e.to_string())?;
    let lock_a = lock_path_for_target(&target);

    // Resolve from a different spelling.
    std::env::set_current_dir(&repo).ok();
    let dotted = PathBuf::from("./policy/allow.toml");
    let target2 = resolve_mutation_target(&dotted, &repo).map_err(|e| e.to_string())?;
    let lock_b = lock_path_for_target(&target2);

    assert_eq!(
        lock_a, lock_b,
        "lock keys must match for the same target under different spellings"
    );
    fs::remove_dir_all(&repo).ok();
    Ok(())
}

#[test]
fn replace_recheck_accepts_regular_file_target() -> Result<(), String> {
    let repo = make_temp_repo()?;
    let file_path = repo.join("policy/allow.toml");
    fs::create_dir_all(file_path.parent().unwrap_or(Path::new("."))).ok();
    fs::write(&file_path, "test").ok();

    let target = resolve_mutation_target(&file_path, &repo).map_err(|e| e.to_string())?;
    super::mutation_target::assert_target_identity_for_replace(&target)
        .map_err(|e| format!("regular file must pass the replace recheck: {e}"))?;
    fs::remove_dir_all(&repo).ok();
    Ok(())
}

#[test]
fn replace_recheck_reports_disappeared_target() -> Result<(), String> {
    let repo = make_temp_repo()?;
    let file_path = repo.join("policy/allow.toml");
    fs::create_dir_all(file_path.parent().unwrap_or(Path::new("."))).ok();

    let target = resolve_mutation_target(&file_path, &repo).map_err(|e| e.to_string())?;
    let error = super::mutation_target::assert_target_identity_for_replace(&target)
        .expect_err("missing target must fail the replace recheck");
    let message = error.to_string();
    assert!(
        message.contains("disappeared between read and identity recheck (#2491)"),
        "disappearance diagnostic must name the failure: {message}"
    );
    fs::remove_dir_all(&repo).ok();
    Ok(())
}

#[test]
fn replace_recheck_rejects_directory_target() -> Result<(), String> {
    let repo = make_temp_repo()?;
    let directory = repo.join("policy/allow.toml");
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;

    let target = resolve_mutation_target(&directory, &repo).map_err(|e| e.to_string())?;
    let error = super::mutation_target::assert_target_identity_for_replace(&target)
        .expect_err("directory target must fail the replace recheck");
    assert!(error.to_string().contains("not a regular file"));
    fs::remove_dir_all(&repo).ok();
    Ok(())
}

#[cfg(unix)]
#[test]
fn replace_recheck_rejects_symlink_substitution() -> Result<(), String> {
    let repo = make_temp_repo()?;
    let file_path = repo.join("policy/allow.toml");
    let sibling = repo.join("outside-secrets.txt");
    fs::create_dir_all(file_path.parent().unwrap_or(Path::new("."))).ok();
    fs::write(&sibling, "not the ledger").ok();
    std::os::unix::fs::symlink(&sibling, &file_path)
        .map_err(|e| format!("create symlink fixture: {e}"))?;

    let target = resolve_mutation_target(&file_path, &repo).map_err(|e| e.to_string())?;
    let error = super::mutation_target::assert_target_leaf_identity_for_replace(&file_path)
        .expect_err("symlink target must fail the replace recheck");
    let message = error.to_string();
    assert!(
        message.contains("is a symlink; refusing to follow for atomic replace (#2491)"),
        "symlink substitution must be rejected: {message}"
    );
    assert!(
        !fs::read_to_string(&sibling).is_ok_and(|text| text == "replaced"),
        "recheck alone must not write anything"
    );
    fs::remove_dir_all(&repo).ok();
    Ok(())
}
