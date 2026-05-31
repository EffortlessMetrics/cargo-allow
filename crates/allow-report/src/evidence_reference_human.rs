use crate::EvidenceReference;

pub(crate) struct EvidenceReferenceHumanStatus {
    pub(crate) marker: &'static str,
    pub(crate) label: &'static str,
}

pub(crate) fn evidence_reference_human_status(
    reference: &EvidenceReference<'_>,
) -> EvidenceReferenceHumanStatus {
    match reference.status {
        "local_file_present" => EvidenceReferenceHumanStatus {
            marker: "ok",
            label: "present",
        },
        "local_file_missing" => EvidenceReferenceHumanStatus {
            marker: "missing",
            label: "missing",
        },
        "invalid_local_path" => EvidenceReferenceHumanStatus {
            marker: "invalid",
            label: "invalid_local_path",
        },
        "traceability_only" => EvidenceReferenceHumanStatus {
            marker: "info",
            label: "not_local",
        },
        "unstructured" if reference.message.contains("unrecognized evidence prefix") => {
            EvidenceReferenceHumanStatus {
                marker: "weak",
                label: "unknown_prefix",
            }
        }
        "unstructured" if reference.prefix.is_some() => EvidenceReferenceHumanStatus {
            marker: "weak",
            label: "untyped",
        },
        "unstructured" => EvidenceReferenceHumanStatus {
            marker: "weak",
            label: "untyped",
        },
        _ => EvidenceReferenceHumanStatus {
            marker: "info",
            label: "unknown_status",
        },
    }
}
