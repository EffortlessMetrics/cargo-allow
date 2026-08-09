//! Forward-looking guard against re-introducing extraction scaffolding
//! as public API (#2940 step 6 / #3332).
//!
//! Scans crate source files for three patterns that were retired.
//! Uses an allow-list seeded from current main so the check starts green;
//! only NEW violations fail.

use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn crate_src_dirs(root: &std::path::Path) -> Vec<PathBuf> {
    let crates_dir = root.join("crates");
    let mut dirs = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&crates_dir) {
        for entry in entries.flatten() {
            let src = entry.path().join("src");
            if src.is_dir() {
                dirs.push(src);
            }
        }
    }
    dirs
}

fn collect_rust_files(dir: &std::path::Path) -> Vec<(PathBuf, String)> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(collect_rust_files(&path));
            } else if let Some("rs") = path.extension().and_then(|e| e.to_str())
                && let Ok(text) = std::fs::read_to_string(&path)
            {
                files.push((path, text));
            }
        }
    }
    files
}

fn is_test_file(path: &std::path::Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.ends_with("_tests.rs") || n == "tests.rs" || n.contains("_test_"))
        .unwrap_or(false)
}

fn normalize_rel_path(path: &std::path::Path, root: &std::path::Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace(char::from(92), "/")
        .to_string()
}

/// Allow-list of existing violations seeded from main at the time this guard
/// was added (#3332). New violations not in this list fail the check.
const ALLOWED_SURFACE_MARKERS: &[&str] = &[
    "BoundarySurface",
    "StagedIndexSurface",
    "RevisionIdentitySurface",
];
const ALLOWED_DEP_ARRAY_FILES: &[&str] = &[
    "crates/intent-edit/src/boundary.rs",
    "crates/proof-engine/src/boundary.rs",
    "crates/proof-engine/src/command_adapter/boundary.rs",
    "crates/proof-engine/src/provider_api/boundary.rs",
    "crates/proof-protocol/src/boundary.rs",
];

const ALLOWED_PARITY_LOCATOR_FILES: &[&str] = &[
    "crates/effortless-repo-edit/src/lib.rs",
    "crates/effortless-repo-snapshot/src/lib.rs",
    "crates/proof-engine/src/provider_api/mod.rs",
];

#[test]
fn no_new_public_surface_marker_structs() -> Result<(), String> {
    let root = workspace_root();
    let mut violations = Vec::new();

    for src_dir in crate_src_dirs(&root) {
        for (path, text) in collect_rust_files(&src_dir) {
            if is_test_file(&path) {
                continue;
            }
            let rel = normalize_rel_path(&path, &root);
            for (i, line) in text.lines().enumerate() {
                let trimmed = line.trim();
                if trimmed.starts_with("pub struct ") && trimmed.ends_with("Surface;") {
                    let name = trimmed
                        .strip_prefix("pub struct ")
                        .and_then(|s| s.strip_suffix(";"))
                        .unwrap_or(trimmed);
                    if !ALLOWED_SURFACE_MARKERS.contains(&name) {
                        violations.push(format!("{rel}:{}: `{name}`", i + 1));
                    }
                }
            }
        }
    }

    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "New public Surface markers found (not in allow-list). Add #[cfg(test)] or remove:\n{}",
            violations.join("\n")
        ))
    }
}

#[test]
fn no_new_public_dependency_arrays() -> Result<(), String> {
    let root = workspace_root();
    let mut violations = Vec::new();

    for src_dir in crate_src_dirs(&root) {
        for (path, text) in collect_rust_files(&src_dir) {
            if is_test_file(&path) {
                continue;
            }
            let rel = normalize_rel_path(&path, &root);
            if ALLOWED_DEP_ARRAY_FILES.contains(&rel.as_str()) {
                continue;
            }
            for (i, line) in text.lines().enumerate() {
                let trimmed = line.trim();
                if trimmed.starts_with("pub const ALLOWED_")
                    || trimmed.starts_with("pub const FORBIDDEN_")
                {
                    violations.push(format!("{rel}:{}: `{trimmed}`", i + 1));
                }
            }
        }
    }

    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "New public dependency arrays found (not in allow-list):\n{}",
            violations.join("\n")
        ))
    }
}

#[test]
fn no_new_public_parity_path_locators() -> Result<(), String> {
    let root = workspace_root();
    let mut violations = Vec::new();

    for src_dir in crate_src_dirs(&root) {
        for (path, text) in collect_rust_files(&src_dir) {
            if is_test_file(&path) {
                continue;
            }
            let rel = normalize_rel_path(&path, &root);
            if ALLOWED_PARITY_LOCATOR_FILES.contains(&rel.as_str()) {
                continue;
            }
            for (i, line) in text.lines().enumerate() {
                let trimmed = line.trim();
                if trimmed.starts_with("pub use ") && trimmed.contains("parity_contract_path") {
                    violations.push(format!("{rel}:{}: `{trimmed}`", i + 1));
                }
            }
        }
    }

    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "New public parity path locators found (not in allow-list):\n{}",
            violations.join("\n")
        ))
    }
}
