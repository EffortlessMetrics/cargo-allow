//! Tests for canonical mutation target identity (#2489).

use std::fs;
use std::path::{Path, PathBuf};

use super::mutation_target::{
    MutationTargetOwnership, lock_path_for_target, resolve_mutation_target,
};

fn make_temp_repo() -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join(format!(
        "mutation-target-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&dir).map_err(|e| format!("create temp repo: {e}"))?;
    Ok(dir)
}

#[test]
fn relative_and_absolute_spellings_produce_same_fingerprint() -> Result<(), String> {
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
