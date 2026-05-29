use allow_core::{AllowConfig, AllowEntry, CargoAllowError, CargoAllowResult};
use std::path::{Path, PathBuf};

use crate::scope_validation::validate_path_scope;

pub fn validate_local_evidence_references(
    root: impl AsRef<Path>,
    cfg: &AllowConfig,
) -> CargoAllowResult<()> {
    let root = root.as_ref();
    for entry in &cfg.allow {
        for evidence in &entry.evidence {
            let Some(reference) = EvidenceReference::parse(evidence) else {
                continue;
            };
            if reference.kind.is_local_file() {
                validate_path_scope(
                    &format!("{} evidence `{}`", entry.id, reference.raw),
                    reference.value.as_ref(),
                )?;
                let path = root.join(&reference.value);
                let metadata = path.metadata().map_err(|_| {
                    CargoAllowError::new(format!(
                        "{} evidence `{}` references missing local file {}",
                        entry.id,
                        reference.raw,
                        reference.value.display()
                    ))
                })?;
                if !metadata.is_file() {
                    return Err(CargoAllowError::new(format!(
                        "{} evidence `{}` must reference a local file, not a directory: {}",
                        entry.id,
                        reference.raw,
                        reference.value.display()
                    )));
                }
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceReferenceStatus {
    LocalFilePresent,
    LocalFileMissing,
    InvalidLocalPath,
    TraceabilityOnly,
    Unstructured,
}

impl EvidenceReferenceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LocalFilePresent => "local_file_present",
            Self::LocalFileMissing => "local_file_missing",
            Self::InvalidLocalPath => "invalid_local_path",
            Self::TraceabilityOnly => "traceability_only",
            Self::Unstructured => "unstructured",
        }
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
    match path.metadata() {
        Ok(metadata) if metadata.is_file() => EvidenceReferenceDiagnostic {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EvidenceKind {
    Test,
    Cargo,
    Ripr,
    UnsafeReview,
    Coverage,
    Doc,
    Spec,
    Adr,
    Issue,
    Pr,
    Unknown,
}

impl EvidenceKind {
    fn parse(prefix: &str) -> Self {
        match prefix {
            "test" => Self::Test,
            "cargo" => Self::Cargo,
            "ripr" => Self::Ripr,
            "unsafe-review" | "unsafe_review" => Self::UnsafeReview,
            "coverage" => Self::Coverage,
            "doc" => Self::Doc,
            "spec" => Self::Spec,
            "adr" => Self::Adr,
            "issue" => Self::Issue,
            "pr" => Self::Pr,
            _ => Self::Unknown,
        }
    }

    fn is_local_file(self) -> bool {
        matches!(
            self,
            Self::Ripr | Self::UnsafeReview | Self::Coverage | Self::Doc | Self::Spec | Self::Adr
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EvidenceReference<'a> {
    raw: &'a str,
    prefix: &'a str,
    kind: EvidenceKind,
    value: PathBuf,
}

impl<'a> EvidenceReference<'a> {
    fn parse(raw: &'a str) -> Option<Self> {
        let (prefix, value) = raw.split_once(':')?;
        let value = value.trim();
        if value.is_empty() {
            return Some(Self {
                raw,
                prefix: prefix.trim(),
                kind: EvidenceKind::parse(prefix.trim()),
                value: PathBuf::new(),
            });
        }
        Some(Self {
            raw,
            prefix: prefix.trim(),
            kind: EvidenceKind::parse(prefix.trim()),
            value: PathBuf::from(value),
        })
    }
}
