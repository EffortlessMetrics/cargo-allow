use allow_core::{AllowEntry, CargoAllowError};

use crate::evidence_diagnostics::{EvidenceReferenceDiagnostic, EvidenceReferenceStatus};

pub(crate) fn evidence_reference_validation_error(
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
