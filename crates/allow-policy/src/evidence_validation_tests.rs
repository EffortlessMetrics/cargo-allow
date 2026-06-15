use std::path::PathBuf;

use allow_core::{AllowEntry, CargoAllowError, FindingKind, Lifecycle, Selector};

use crate::evidence_diagnostics::{
    EvidenceReferenceCategory, EvidenceReferenceDiagnostic, EvidenceReferenceSource,
    EvidenceReferenceStatus, PolicyReferenceDiagnostic,
};

use super::policy_reference_validation_error;

fn entry(id: &str) -> AllowEntry {
    AllowEntry {
        id: id.to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: Some(PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "core".to_string(),
        classification: "reviewed".to_string(),
        reason: "fixture".to_string(),
        evidence: vec!["doc:missing.md".to_string()],
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle::empty(),
        selector: Selector::default(),
        last_seen: None,
    }
}

fn diagnostic(
    status: EvidenceReferenceStatus,
    message: &str,
    target: Option<PathBuf>,
) -> EvidenceReferenceDiagnostic {
    EvidenceReferenceDiagnostic {
        raw: "doc:missing.md".to_string(),
        prefix: Some("doc".to_string()),
        target,
        status,
        category: EvidenceReferenceCategory::Missing,
        message: message.to_string(),
    }
}

fn policy_reference(
    source: EvidenceReferenceSource,
    diagnostic: EvidenceReferenceDiagnostic,
) -> PolicyReferenceDiagnostic {
    PolicyReferenceDiagnostic { source, diagnostic }
}

#[test]
fn policy_reference_validation_error_call_presence_observer() {
    let entry = entry("allow-001");
    let target = PathBuf::from("docs/missing.md");
    let reference = policy_reference(
        EvidenceReferenceSource::Evidence,
        diagnostic(
            EvidenceReferenceStatus::LocalFileMissing,
            "local evidence file is missing",
            Some(target.clone()),
        ),
    );

    assert_eq!(
        policy_reference_validation_error(&entry, &reference),
        Some(CargoAllowError::new(format!(
            "allow-001 evidence `doc:missing.md` references missing local file {}",
            target.display()
        )))
    );
}

#[test]
fn reference_validation_error_match_arm_discriminator() {
    let entry = entry("allow-002");

    assert_eq!(
        policy_reference_validation_error(
            &entry,
            &policy_reference(
                EvidenceReferenceSource::Evidence,
                diagnostic(
                    EvidenceReferenceStatus::LocalFilePresent,
                    "local evidence file is present",
                    Some(PathBuf::from("docs/present.md")),
                ),
            )
        ),
        None
    );
    assert_eq!(
        policy_reference_validation_error(
            &entry,
            &policy_reference(
                EvidenceReferenceSource::Link,
                diagnostic(
                    EvidenceReferenceStatus::TraceabilityOnly,
                    "traceability evidence is not executed",
                    None,
                ),
            )
        ),
        None
    );
    assert_eq!(
        policy_reference_validation_error(
            &entry,
            &policy_reference(
                EvidenceReferenceSource::Evidence,
                diagnostic(
                    EvidenceReferenceStatus::Unstructured,
                    "unstructured evidence",
                    None,
                ),
            )
        ),
        None
    );

    let directory = PathBuf::from("docs/dir");
    assert_eq!(
        policy_reference_validation_error(
            &entry,
            &policy_reference(
                EvidenceReferenceSource::Link,
                diagnostic(
                    EvidenceReferenceStatus::InvalidLocalPath,
                    "link target is not a file",
                    Some(directory.clone()),
                ),
            )
        ),
        Some(CargoAllowError::new(format!(
            "allow-002 link `doc:missing.md` must reference a local file, not a directory: {}",
            directory.display()
        )))
    );
}
