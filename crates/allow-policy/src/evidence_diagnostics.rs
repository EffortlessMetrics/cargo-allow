use allow_core::AllowEntry;
use std::fs;
use std::path::{Path, PathBuf};

use crate::evidence_reference::{EvidenceKind, EvidenceReference};
use crate::scope_validation::validate_path_scope;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceReferenceStatus {
    LocalFilePresent,
    LocalFileMissing,
    InvalidLocalPath,
    TraceabilityOnly,
    Unstructured,
}

impl EvidenceReferenceStatus {
    pub const ALL: &[Self] = &[
        Self::LocalFilePresent,
        Self::LocalFileMissing,
        Self::InvalidLocalPath,
        Self::TraceabilityOnly,
        Self::Unstructured,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::LocalFilePresent => "local_file_present",
            Self::LocalFileMissing => "local_file_missing",
            Self::InvalidLocalPath => "invalid_local_path",
            Self::TraceabilityOnly => "traceability_only",
            Self::Unstructured => "unstructured",
        }
    }

    pub fn is_broken_local_link(self) -> bool {
        matches!(self, Self::LocalFileMissing | Self::InvalidLocalPath)
    }

    pub fn is_weak_reference(self) -> bool {
        matches!(self, Self::Unstructured)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceReferenceDiagnostic {
    pub raw: String,
    pub prefix: Option<String>,
    pub target: Option<PathBuf>,
    pub status: EvidenceReferenceStatus,
    pub message: String,
}

pub fn evidence_reference_diagnostics(
    root: impl AsRef<Path>,
    entry: &AllowEntry,
) -> Vec<EvidenceReferenceDiagnostic> {
    let root = root.as_ref();
    entry
        .evidence
        .iter()
        .map(|evidence| evidence_reference_diagnostic(root, evidence))
        .collect()
}

fn evidence_reference_diagnostic(root: &Path, raw: &str) -> EvidenceReferenceDiagnostic {
    let Some(reference) = EvidenceReference::parse(raw) else {
        return EvidenceReferenceDiagnostic {
            raw: raw.to_string(),
            prefix: None,
            target: None,
            status: EvidenceReferenceStatus::Unstructured,
            message: "unstructured evidence string; not locally validated".to_string(),
        };
    };
    let prefix = Some(reference.prefix.to_string());
    if reference.kind == EvidenceKind::Unknown {
        return EvidenceReferenceDiagnostic {
            raw: raw.to_string(),
            prefix,
            target: Some(reference.value.clone()),
            status: EvidenceReferenceStatus::Unstructured,
            message: "unrecognized evidence prefix; not locally validated".to_string(),
        };
    }
    if !reference.kind.is_local_file() && reference.value.as_os_str().is_empty() {
        return EvidenceReferenceDiagnostic {
            raw: raw.to_string(),
            prefix,
            target: None,
            status: EvidenceReferenceStatus::Unstructured,
            message: "empty evidence reference target; not locally validated".to_string(),
        };
    }
    if !reference.kind.is_local_file() {
        return EvidenceReferenceDiagnostic {
            raw: raw.to_string(),
            prefix,
            target: Some(reference.value.clone()),
            status: EvidenceReferenceStatus::TraceabilityOnly,
            message: "traceability reference; not executed or resolved by cargo-allow".to_string(),
        };
    }
    if let Err(err) = validate_path_scope("evidence", reference.value.as_ref()) {
        return EvidenceReferenceDiagnostic {
            raw: raw.to_string(),
            prefix,
            target: Some(reference.value.clone()),
            status: EvidenceReferenceStatus::InvalidLocalPath,
            message: err.to_string(),
        };
    }
    let path = root.join(&reference.value);
    if let Some(component) = first_symlink_component(root, &reference.value) {
        return EvidenceReferenceDiagnostic {
            raw: raw.to_string(),
            prefix,
            target: Some(reference.value.clone()),
            status: EvidenceReferenceStatus::InvalidLocalPath,
            message: format!(
                "local evidence path contains symlink component {}; reference regular source-tree files",
                component.display()
            ),
        };
    }
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() => EvidenceReferenceDiagnostic {
            raw: raw.to_string(),
            prefix,
            target: Some(reference.value.clone()),
            status: EvidenceReferenceStatus::InvalidLocalPath,
            message: "local evidence path is a symlink; reference a regular source-tree file"
                .to_string(),
        },
        Ok(metadata) if metadata.file_type().is_file() => EvidenceReferenceDiagnostic {
            raw: raw.to_string(),
            prefix,
            target: Some(reference.value.clone()),
            status: EvidenceReferenceStatus::LocalFilePresent,
            message: "local evidence file exists".to_string(),
        },
        Ok(_) => EvidenceReferenceDiagnostic {
            raw: raw.to_string(),
            prefix,
            target: Some(reference.value.clone()),
            status: EvidenceReferenceStatus::InvalidLocalPath,
            message: "local evidence path exists but is not a file".to_string(),
        },
        Err(_) => EvidenceReferenceDiagnostic {
            raw: raw.to_string(),
            prefix,
            target: Some(reference.value.clone()),
            status: EvidenceReferenceStatus::LocalFileMissing,
            message: "local evidence file is missing".to_string(),
        },
    }
}

fn first_symlink_component(root: &Path, relative: &Path) -> Option<PathBuf> {
    let mut current = root.to_path_buf();
    let mut source_tree_component = PathBuf::new();
    for component in relative.components() {
        current.push(component.as_os_str());
        source_tree_component.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Some(source_tree_component);
            }
            Ok(_) => {}
            Err(_) => return None,
        }
    }
    None
}
