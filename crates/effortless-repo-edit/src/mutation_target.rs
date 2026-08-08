//! Canonical mutation target identity and resolver (#2489).
//!
//! Provides one typed target-resolution authority for mutation locking. Equal
//! supported aliases (relative, absolute, `.`/`..`, verbatim-prefix) produce
//! the same canonical identity and lock key.

use std::path::{Path, PathBuf};

use allow_core::{CargoAllowError, CargoAllowResult};

use crate::containment::strip_verbatim_prefix;
use crate::target_identity::canonicalize_lexically;

/// The canonical identity of a mutation target (#2489).
///
/// Equal supported aliases produce the same
/// [`target_fingerprint`](MutationTarget::target_fingerprint), which is
/// used as the mutation lock key. The `repo_relative_display` is safe for
/// receipts; absolute paths are kept private.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutationTarget {
    /// Lexically normalized absolute path (private — not for receipts).
    normalized_absolute: PathBuf,
    /// Repository-relative display path (safe for receipts), or the absolute
    /// path if the target is outside the source tree root.
    repo_relative_display: String,
    /// Stable fingerprint for lock-key derivation.
    fingerprint: String,
    /// Ownership/result class.
    ownership: MutationTargetOwnership,
}

/// Result of resolving a mutation target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationTargetOwnership {
    /// Target is inside the source tree root.
    SourceTreeOwned,
    /// Target is outside the source tree root.
    OutsideSourceTree,
}

impl MutationTarget {
    /// The repository-relative display path (safe for receipts).
    pub fn repo_relative_display(&self) -> &str {
        &self.repo_relative_display
    }

    /// The stable fingerprint used for lock-key derivation.
    pub fn target_fingerprint(&self) -> &str {
        &self.fingerprint
    }

    /// The ownership class.
    pub fn ownership(&self) -> MutationTargetOwnership {
        self.ownership
    }

    /// The normalized absolute path (for filesystem operations).
    pub fn normalized_absolute(&self) -> &Path {
        &self.normalized_absolute
    }
}

/// Resolve a mutation target against a source tree root (#2489).
///
/// Normalizes `.`/`..`, strips verbatim prefixes, and canonicalizes using
/// the nearest existing parent for not-yet-created files. Equal supported
/// aliases produce the same fingerprint.
pub fn resolve_mutation_target(
    requested: &Path,
    source_tree_root: &Path,
) -> CargoAllowResult<MutationTarget> {
    // Step 1: Strip verbatim prefix and lexical normalization.
    let stripped = strip_verbatim_prefix(requested);
    let lexical = canonicalize_lexically(&stripped);

    // Step 2: Resolve to absolute.
    let absolute = if lexical.is_absolute() {
        lexical
    } else {
        std::env::current_dir()
            .map_err(|e| CargoAllowError::new(format!("failed to get current dir: {e}")))?
            .join(&lexical)
    };

    // Step 3: Canonicalize the source tree root for ownership comparison.
    let canonical_root = source_tree_root.canonicalize().map_err(|e| {
        CargoAllowError::new(format!(
            "failed to canonicalize source tree root {}: {e}",
            source_tree_root.display()
        ))
    })?;

    // Step 4: Resolve the target using nearest-existing-parent strategy.
    let resolved = resolve_nearest_existing(&absolute)?;

    // Step 5: Determine ownership.
    let ownership = if resolved.starts_with(&canonical_root) {
        MutationTargetOwnership::SourceTreeOwned
    } else {
        MutationTargetOwnership::OutsideSourceTree
    };

    // Step 6: Compute repo-relative display.
    let repo_relative_display = if ownership == MutationTargetOwnership::SourceTreeOwned {
        resolved
            .strip_prefix(&canonical_root)
            .map(|rel| rel.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| resolved.to_string_lossy().replace('\\', "/"))
    } else {
        strip_verbatim_prefix(&resolved)
            .to_string_lossy()
            .replace('\\', "/")
    };

    // Step 7: Compute stable fingerprint from the resolved canonical path.
    let fingerprint = allow_core::stable_hash_hex(&resolved.to_string_lossy());

    Ok(MutationTarget {
        normalized_absolute: resolved,
        repo_relative_display,
        fingerprint,
        ownership,
    })
}

/// Resolve a path using the nearest-existing-parent strategy.
///
/// For existing files: `std::fs::canonicalize` gives the true filesystem
/// identity (resolving symlinks).
/// For non-existing files: canonicalize the nearest existing parent, then
/// append the remaining components.
fn resolve_nearest_existing(path: &Path) -> CargoAllowResult<PathBuf> {
    if path.exists() {
        return path.canonicalize().map_err(|e| {
            CargoAllowError::new(format!(
                "failed to canonicalize target {}: {e}",
                path.display()
            ))
        });
    }
    // Walk up to find the nearest existing ancestor.
    let mut existing_parent = path.to_path_buf();
    let mut remaining: Vec<std::ffi::OsString> = Vec::new();
    while !existing_parent.exists() {
        let file_name = existing_parent.file_name().map(std::ffi::OsStr::to_owned);
        let parent = existing_parent
            .parent()
            .map(|p| p.to_path_buf())
            .ok_or_else(|| {
                CargoAllowError::new(format!("path has no parent: {}", path.display()))
            })?;
        if let Some(name) = file_name {
            remaining.push(name);
        }
        existing_parent = parent;
        if existing_parent.as_os_str().is_empty() || existing_parent == Path::new("/") {
            return Err(CargoAllowError::new(format!(
                "no existing parent directory found for {}",
                path.display()
            )));
        }
    }
    let canonical_parent = existing_parent.canonicalize().map_err(|e| {
        CargoAllowError::new(format!(
            "failed to canonicalize parent {}: {e}",
            existing_parent.display()
        ))
    })?;
    // Append remaining components in reverse order.
    let mut result = canonical_parent;
    for name in remaining.into_iter().rev() {
        result.push(name);
    }
    Ok(result)
}

/// Derive the mutation lock key from a resolved target fingerprint (#2489).
///
/// This replaces the old `lock_path` function that hashed raw lexical path
/// text. Equal targets (by canonical identity) share one lock key.
pub fn lock_path_for_target(target: &MutationTarget) -> PathBuf {
    std::env::temp_dir()
        .join("cargo-allow-locks")
        .join(format!("{}.lock", target.target_fingerprint()))
}
