use crate::EvidenceReference;

pub(crate) struct EvidenceReferenceHumanStatus {
    pub(crate) label: &'static str,
}

pub(crate) fn evidence_reference_human_status(
    reference: &EvidenceReference<'_>,
) -> EvidenceReferenceHumanStatus {
    match reference.category {
        "present" => EvidenceReferenceHumanStatus { label: "present" },
        "missing" => EvidenceReferenceHumanStatus { label: "missing" },
        "invalid_local_path" => EvidenceReferenceHumanStatus {
            label: "invalid-local-path",
        },
        "not_local" => EvidenceReferenceHumanStatus { label: "not-local" },
        "unknown_prefix" => EvidenceReferenceHumanStatus { label: "weak" },
        "untyped" => EvidenceReferenceHumanStatus { label: "weak" },
        _ => EvidenceReferenceHumanStatus {
            label: "unknown_status",
        },
    }
}
