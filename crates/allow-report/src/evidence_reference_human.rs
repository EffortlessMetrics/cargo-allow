use crate::EvidenceReference;

pub(crate) struct EvidenceReferenceHumanStatus {
    pub(crate) marker: &'static str,
    pub(crate) label: &'static str,
}

pub(crate) fn evidence_reference_human_status(
    reference: &EvidenceReference<'_>,
) -> EvidenceReferenceHumanStatus {
    match reference.category {
        "present" => EvidenceReferenceHumanStatus {
            marker: "ok",
            label: "present",
        },
        "missing" => EvidenceReferenceHumanStatus {
            marker: "missing",
            label: "missing",
        },
        "invalid_local_path" => EvidenceReferenceHumanStatus {
            marker: "invalid",
            label: "invalid-local-path",
        },
        "not_local" => EvidenceReferenceHumanStatus {
            marker: "info",
            label: "not-local",
        },
        "unknown_prefix" => EvidenceReferenceHumanStatus {
            marker: "weak",
            label: "weak",
        },
        "untyped" => EvidenceReferenceHumanStatus {
            marker: "weak",
            label: "weak",
        },
        _ => EvidenceReferenceHumanStatus {
            marker: "info",
            label: "unknown_status",
        },
    }
}
