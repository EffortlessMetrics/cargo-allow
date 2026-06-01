use allow_core::normalize_path;
use allow_policy::{EvidenceReferenceDiagnostic, EvidenceReferenceStatus};

pub(crate) fn evidence_reference_target_text(
    diagnostic: &EvidenceReferenceDiagnostic,
) -> Option<String> {
    diagnostic.target.as_ref().map(|target| {
        if diagnostic.status == EvidenceReferenceStatus::InvalidLocalPath {
            target.to_string_lossy().replace('\\', "/")
        } else {
            normalize_path(target)
        }
    })
}
