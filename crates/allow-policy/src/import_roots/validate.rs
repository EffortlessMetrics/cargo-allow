use super::config::{ImportRootEntry, ImportRootsConfig};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportDiagnosticKind {
    DuplicateRootId,
    DuplicateRootPath,
    MissingRoot,
    BrokenEdge,
    UnknownRole,
}

impl ImportDiagnosticKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DuplicateRootId => "duplicate_root_id",
            Self::DuplicateRootPath => "duplicate_root_path",
            Self::MissingRoot => "missing_root",
            Self::BrokenEdge => "broken_edge",
            Self::UnknownRole => "unknown_role",
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
