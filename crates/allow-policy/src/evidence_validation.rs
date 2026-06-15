use allow_core::{AllowEntry, CargoAllowError};

use crate::evidence_diagnostics::{
    EvidenceReferenceDiagnostic, EvidenceReferenceSource, EvidenceReferenceStatus,
    PolicyReferenceDiagnostic,
};

pub(crate) fn policy_reference_validation_error(
    entry: &AllowEntry,
    reference: &PolicyReferenceDiagnostic,
) -> Option<CargoAllowError> {
    reference_validation_error(entry, reference.source, &reference.diagnostic)
}

fn reference_validation_error(
    entry: &AllowEntry,
    source: EvidenceReferenceSource,
    diagnostic: &EvidenceReferenceDiagnostic,
) -> Option<CargoAllowError> {
    let label = source.label();
    match diagnostic.status {
        EvidenceReferenceStatus::LocalFilePresent
        | EvidenceReferenceStatus::TraceabilityOnly
        | EvidenceReferenceStatus::Unstructured => None,
        EvidenceReferenceStatus::LocalFileMissing => {
            let target = diagnostic.target.as_ref()?;
            Some(CargoAllowError::new(format!(
                "{} {label} `{}` references missing local file {}",
                entry.id,
                diagnostic.raw,
                target.display()
            )))
        }
        EvidenceReferenceStatus::InvalidLocalPath if diagnostic.message.contains("not a file") => {
            let target = diagnostic.target.as_ref()?;
            Some(CargoAllowError::new(format!(
                "{} {label} `{}` must reference a local file, not a directory: {}",
                entry.id,
                diagnostic.raw,
                target.display()
            )))
        }
        EvidenceReferenceStatus::InvalidLocalPath => Some(CargoAllowError::new(format!(
            "{} {label} `{}` {}",
            entry.id,
            diagnostic.raw,
            source
                .message(&diagnostic.message)
                .strip_prefix(&format!("{label} "))
                .map(str::to_string)
                .unwrap_or_else(|| source.message(&diagnostic.message))
        ))),
    }
}

#[cfg(test)]
#[path = "evidence_validation_tests.rs"]
mod tests;
