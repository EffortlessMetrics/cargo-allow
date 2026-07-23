use super::config::{ImportRootEntry, ImportRootsConfig};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportDiagnosticKind {
    DuplicateRootId,
    DuplicateRootPath,
    MissingRoot,
    BrokenEdge,
    UnknownRole,
    InvalidRootPath,
}

impl ImportDiagnosticKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DuplicateRootId => "duplicate_root_id",
            Self::DuplicateRootPath => "duplicate_root_path",
            Self::MissingRoot => "missing_root",
            Self::BrokenEdge => "broken_edge",
            Self::UnknownRole => "unknown_role",
            Self::InvalidRootPath => "invalid_root_path",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportDiagnostic {
    pub kind: ImportDiagnosticKind,
    pub message: String,
    pub root_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedImportRootsConfig {
    pub config: ImportRootsConfig,
    pub diagnostics: Vec<ImportDiagnostic>,
    pub valid: bool,
}

pub fn validate_import_roots_config(config: ImportRootsConfig) -> ValidatedImportRootsConfig {
    let mut diagnostics = Vec::new();
    collect_duplicate_ids(&config.entries, &mut diagnostics);
    collect_duplicate_paths(&config.entries, &mut diagnostics);
    collect_invalid_paths(&config.entries, &mut diagnostics);
    let valid = diagnostics.is_empty();
    ValidatedImportRootsConfig {
        config,
        diagnostics,
        valid,
    }
}

fn collect_duplicate_ids(entries: &[ImportRootEntry], diagnostics: &mut Vec<ImportDiagnostic>) {
    let mut seen = std::collections::BTreeSet::new();
    for entry in entries {
        if !seen.insert(entry.id.clone()) {
            diagnostics.push(ImportDiagnostic {
                kind: ImportDiagnosticKind::DuplicateRootId,
                message: format!("duplicate import root id `{}`", entry.id),
                root_ids: vec![entry.id.clone()],
            });
        }
    }
}

fn collect_duplicate_paths(entries: &[ImportRootEntry], diagnostics: &mut Vec<ImportDiagnostic>) {
    let mut seen = std::collections::BTreeMap::<String, String>::new();
    for entry in entries {
        if let Some(existing) = seen.insert(entry.path.clone(), entry.id.clone()) {
            diagnostics.push(ImportDiagnostic {
                kind: ImportDiagnosticKind::DuplicateRootPath,
                message: format!(
                    "duplicate import root path `{}` for ids `{}` and `{}`",
                    entry.path, existing, entry.id
                ),
                root_ids: vec![existing, entry.id.clone()],
            });
        }
    }
}

/// #1839: validate import-root paths for source-tree-relative safety.
/// Reuses `validate_path_scope` (the same validator allow entries use in
/// `scope_validation.rs`) to reject absolute paths, drive letters, `..`
/// traversal, `.` bare-dot, and other out-of-tree escapes. Without this
/// check, `discover.rs:64` does `root.join(&entry.path)` which silently
/// replaces the base when given an absolute path — the same bug class the
/// federation layer calls out in its #2011 comment.
fn collect_invalid_paths(entries: &[ImportRootEntry], diagnostics: &mut Vec<ImportDiagnostic>) {
    for entry in entries {
        if let Err(err) =
            crate::source_tree_scope::validate_path_scope(&entry.id, Path::new(&entry.path))
        {
            diagnostics.push(ImportDiagnostic {
                kind: ImportDiagnosticKind::InvalidRootPath,
                message: err.to_string(),
                root_ids: vec![entry.id.clone()],
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import_roots::config::{ImportNodeRole, ImportRootEntry, ImportRootsConfig};

    fn root_entry(id: &str, path: &str) -> ImportRootEntry {
        ImportRootEntry {
            id: id.to_string(),
            path: path.to_string(),
            ecosystem: "rust".to_string(),
            role: ImportNodeRole::Owned,
        }
    }

    fn cfg(entries: Vec<ImportRootEntry>) -> ImportRootsConfig {
        ImportRootsConfig {
            owned: None,
            entries,
        }
    }

    #[test]
    fn validate_import_roots_rejects_traversal_path() {
        // #1839: path = "../sibling" must be rejected because discover.rs
        // does root.join(&entry.path), which escapes the source tree.
        let validated = validate_import_roots_config(cfg(vec![root_entry("evil", "../sibling")]));
        assert!(!validated.valid, "traversal path should be invalid");
        assert!(
            validated.diagnostics.iter().any(|d| {
                d.kind == ImportDiagnosticKind::InvalidRootPath
                    && d.message.contains("parent directory segments")
            }),
            "expected invalid_root_path diagnostic for traversal: {:?}",
            validated.diagnostics
        );
    }

    #[test]
    fn validate_import_roots_rejects_absolute_path() {
        let validated = validate_import_roots_config(cfg(vec![root_entry("abs", "/etc/passwd")]));
        assert!(!validated.valid);
        assert!(
            validated
                .diagnostics
                .iter()
                .any(|d| d.kind == ImportDiagnosticKind::InvalidRootPath),
            "expected invalid_root_path for absolute path: {:?}",
            validated.diagnostics
        );
    }

    #[test]
    fn validate_import_roots_accepts_source_tree_relative_path() {
        let validated = validate_import_roots_config(cfg(vec![root_entry("ok", "docs/policies")]));
        assert!(
            validated.valid,
            "source-tree-relative path should be valid: {:?}",
            validated.diagnostics
        );
    }
}
