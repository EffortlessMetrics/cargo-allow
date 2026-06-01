use allow_core::AllowEntry;
use std::fs;
use std::path::{Path, PathBuf};

use crate::evidence_path::{first_symlink_component, normalize_local_evidence_path};
use crate::evidence_reference::{EvidenceKind, EvidenceReference};
use crate::source_tree_scope::validate_path_scope;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceReferenceCategory {
    Present,
    Missing,
    InvalidLocalPath,
    NotLocal,
    UnknownPrefix,
    Untyped,
}

impl EvidenceReferenceCategory {
    pub const ALL: &[Self] = &[
        Self::Present,
        Self::Missing,
        Self::InvalidLocalPath,
        Self::NotLocal,
        Self::UnknownPrefix,
        Self::Untyped,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Present => "present",
            Self::Missing => "missing",
            Self::InvalidLocalPath => "invalid_local_path",
            Self::NotLocal => "not_local",
            Self::UnknownPrefix => "unknown_prefix",
            Self::Untyped => "untyped",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceReferenceDiagnostic {
    pub raw: String,
    pub prefix: Option<String>,
    pub target: Option<PathBuf>,
    pub status: EvidenceReferenceStatus,
    pub category: EvidenceReferenceCategory,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceReferenceSource {
    Evidence,
    Link,
}

impl EvidenceReferenceSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::Evidence => "evidence",
            Self::Link => "link",
        }
    }

    pub fn message(self, message: &str) -> String {
        match self {
            Self::Evidence => message.to_string(),
            Self::Link => message.replace("evidence", "link"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyReferenceDiagnostic {
    pub source: EvidenceReferenceSource,
    pub diagnostic: EvidenceReferenceDiagnostic,
}

pub fn evidence_reference_diagnostics(
    root: impl AsRef<Path>,
    entry: &AllowEntry,
) -> Vec<EvidenceReferenceDiagnostic> {
    reference_diagnostics(root.as_ref(), &entry.evidence)
}

pub fn policy_reference_diagnostics(
    root: impl AsRef<Path>,
    entry: &AllowEntry,
) -> Vec<PolicyReferenceDiagnostic> {
    let root = root.as_ref();
    let mut diagnostics = reference_diagnostics(root, &entry.evidence)
        .into_iter()
        .map(|diagnostic| PolicyReferenceDiagnostic {
            source: EvidenceReferenceSource::Evidence,
            diagnostic,
        })
        .collect::<Vec<_>>();
    diagnostics.extend(
        reference_diagnostics(root, &entry.links)
            .into_iter()
            .map(|diagnostic| PolicyReferenceDiagnostic {
                source: EvidenceReferenceSource::Link,
                diagnostic,
            }),
    );
    diagnostics
}

fn reference_diagnostics(root: &Path, references: &[String]) -> Vec<EvidenceReferenceDiagnostic> {
    references
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
            category: EvidenceReferenceCategory::Untyped,
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
            category: EvidenceReferenceCategory::UnknownPrefix,
            message: "unrecognized evidence prefix; not locally validated".to_string(),
        };
    }
    if !reference.kind.is_local_file() && reference.value.as_os_str().is_empty() {
        return EvidenceReferenceDiagnostic {
            raw: raw.to_string(),
            prefix,
            target: None,
            status: EvidenceReferenceStatus::Unstructured,
            category: EvidenceReferenceCategory::Untyped,
            message: "empty evidence reference target; not locally validated".to_string(),
        };
    }
    if !reference.kind.is_local_file() {
        return EvidenceReferenceDiagnostic {
            raw: raw.to_string(),
            prefix,
            target: Some(reference.value.clone()),
            status: EvidenceReferenceStatus::TraceabilityOnly,
            category: EvidenceReferenceCategory::NotLocal,
            message: "traceability reference; not executed or resolved by cargo-allow".to_string(),
        };
    }
    let target = normalize_local_evidence_path(&reference.value);
    if let Err(err) = validate_path_scope("evidence", target.as_ref()) {
        return EvidenceReferenceDiagnostic {
            raw: raw.to_string(),
            prefix,
            target: Some(target),
            status: EvidenceReferenceStatus::InvalidLocalPath,
            category: EvidenceReferenceCategory::InvalidLocalPath,
            message: err.to_string(),
        };
    }
    let path = root.join(&target);
    if let Some(component) = first_symlink_component(root, &target) {
        return EvidenceReferenceDiagnostic {
            raw: raw.to_string(),
            prefix,
            target: Some(target),
            status: EvidenceReferenceStatus::InvalidLocalPath,
            category: EvidenceReferenceCategory::InvalidLocalPath,
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
            target: Some(target),
            status: EvidenceReferenceStatus::InvalidLocalPath,
            category: EvidenceReferenceCategory::InvalidLocalPath,
            message: "local evidence path is a symlink; reference a regular source-tree file"
                .to_string(),
        },
        Ok(metadata) if metadata.file_type().is_file() => EvidenceReferenceDiagnostic {
            raw: raw.to_string(),
            prefix,
            target: Some(target),
            status: EvidenceReferenceStatus::LocalFilePresent,
            category: EvidenceReferenceCategory::Present,
            message: "local evidence file exists".to_string(),
        },
        Ok(_) => EvidenceReferenceDiagnostic {
            raw: raw.to_string(),
            prefix,
            target: Some(target),
            status: EvidenceReferenceStatus::InvalidLocalPath,
            category: EvidenceReferenceCategory::InvalidLocalPath,
            message: "local evidence path exists but is not a file".to_string(),
        },
        Err(_) => EvidenceReferenceDiagnostic {
            raw: raw.to_string(),
            prefix,
            target: Some(target),
            status: EvidenceReferenceStatus::LocalFileMissing,
            category: EvidenceReferenceCategory::Missing,
            message: "local evidence file is missing".to_string(),
        },
    }
}
