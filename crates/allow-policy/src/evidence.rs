use allow_core::{AllowConfig, AllowEntry, CargoAllowError, CargoAllowResult};
use std::path::Path;

use crate::evidence_diagnostics::{
    EvidenceReferenceDiagnostic, EvidenceReferenceStatus, evidence_reference_diagnostics,
};

pub fn validate_local_evidence_references(
    root: impl AsRef<Path>,
    cfg: &AllowConfig,
) -> CargoAllowResult<()> {
    let root = root.as_ref();
    for entry in &cfg.allow {
        for diagnostic in evidence_reference_diagnostics(root, entry) {
            if let Some(error) = evidence_reference_validation_error(entry, &diagnostic) {
                return Err(error);
            }
        }
    }
    Ok(())
}

pub fn broken_evidence_link_count(root: impl AsRef<Path>, cfg: &AllowConfig) -> usize {
    let root = root.as_ref();
    cfg.allow
        .iter()
        .flat_map(|entry| evidence_reference_diagnostics(root, entry))
        .filter(|diagnostic| diagnostic.status.is_broken_local_link())
        .count()
}

pub fn weak_evidence_reference_count(root: impl AsRef<Path>, cfg: &AllowConfig) -> usize {
    let root = root.as_ref();
    cfg.allow
        .iter()
        .flat_map(|entry| evidence_reference_diagnostics(root, entry))
        .filter(|diagnostic| diagnostic.status.is_weak_reference())
        .count()
}

fn evidence_reference_validation_error(
    entry: &AllowEntry,
    diagnostic: &EvidenceReferenceDiagnostic,
) -> Option<CargoAllowError> {
    match diagnostic.status {
        EvidenceReferenceStatus::LocalFilePresent
        | EvidenceReferenceStatus::TraceabilityOnly
        | EvidenceReferenceStatus::Unstructured => None,
        EvidenceReferenceStatus::LocalFileMissing => {
            let target = diagnostic.target.as_ref()?;
            Some(CargoAllowError::new(format!(
                "{} evidence `{}` references missing local file {}",
                entry.id,
                diagnostic.raw,
                target.display()
            )))
        }
        EvidenceReferenceStatus::InvalidLocalPath if diagnostic.message.contains("not a file") => {
            let target = diagnostic.target.as_ref()?;
            Some(CargoAllowError::new(format!(
                "{} evidence `{}` must reference a local file, not a directory: {}",
                entry.id,
                diagnostic.raw,
                target.display()
            )))
        }
        EvidenceReferenceStatus::InvalidLocalPath => Some(CargoAllowError::new(format!(
            "{} evidence `{}` {}",
            entry.id,
            diagnostic.raw,
            diagnostic
                .message
                .strip_prefix("evidence ")
                .unwrap_or(&diagnostic.message)
        ))),
    }
}
