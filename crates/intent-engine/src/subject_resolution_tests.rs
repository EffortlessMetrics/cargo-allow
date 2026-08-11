use crate::{
    IntentSubjectResolutionClassV1, resolve_authored_rust_subject,
};
use effortless_rust_source_index::{
    RustTestInventory, RustTestInventoryStatus, RustTestSelector, RustTestSourceRange,
    RustTestSubject, RustTestTargetIdentity, RustTestTargetKind,
};
use intent_model::{
    EvidenceSubjectId, EvidenceSubjectRegistration, EvidenceSubjectRole, SourceLocation,
};

#[test]
fn missing_and_malformed_authored_subjects_remain_distinct() {
    let missing = resolve_authored_rust_subject(&anchor("lib:demo"), &empty_inventory());
    assert_eq!(missing.class, IntentSubjectResolutionClassV1::Missing);
    assert!(missing.selector.is_some());

    let malformed = resolve_authored_rust_subject(&anchor("bench:demo"), &empty_inventory());
    assert_eq!(
        malformed.class,
        IntentSubjectResolutionClassV1::MalformedAnchor
    );
    assert!(malformed.selector.is_none());
    assert_eq!(malformed.limitations.len(), 1);
}

#[test]
fn ambiguous_authored_subject_preserves_candidates() {
    let selector = selector(RustTestTargetKind::Library, "demo", "alpha");
    let mut second = subject(selector.clone());
    second.source_path = "src/other.rs".to_string();
    let inventory = RustTestInventory {
        subjects: vec![subject(selector), second],
        status: RustTestInventoryStatus::Complete,
        diagnostics: Vec::new(),
    };

    let resolution = resolve_authored_rust_subject(&anchor("lib:demo"), &inventory);
    assert_eq!(resolution.class, IntentSubjectResolutionClassV1::Ambiguous);
    assert_eq!(resolution.candidates.len(), 2);
    assert!(resolution.source_path.is_none());
}

#[test]
fn subject_postures_are_projected_without_becoming_current() {
    for (ignored, generated, conditional, expected) in [
        (
            true,
            false,
            false,
            IntentSubjectResolutionClassV1::Ignored,
        ),
        (
            false,
            true,
            false,
            IntentSubjectResolutionClassV1::GeneratedOrParameterized,
        ),
        (
            false,
            false,
            true,
            IntentSubjectResolutionClassV1::ConditionalUnknown,
        ),
    ] {
        let mut subject = subject(selector(RustTestTargetKind::Library, "demo", "alpha"));
        subject.ignored = ignored;
        subject.generated_or_parameterized = generated;
        subject.cfg_or_feature_unknown = conditional;
        subject.limitations = vec!["structural limitation".to_string()];
        let inventory = RustTestInventory {
            subjects: vec![subject],
            status: RustTestInventoryStatus::Complete,
            diagnostics: Vec::new(),
        };

        let resolution = resolve_authored_rust_subject(&anchor("lib:demo"), &inventory);
        assert_eq!(resolution.class, expected);
        assert_eq!(resolution.source_path.as_deref(), Some("src/lib.rs"));
        assert_eq!(resolution.limitations, ["structural limitation"]);
    }
}

#[test]
fn binary_and_integration_test_targets_resolve_exactly() {
    for (target, kind, name) in [
        ("bin:runner", RustTestTargetKind::Binary, "runner"),
        (
            "test:api",
            RustTestTargetKind::IntegrationTest,
            "api",
        ),
    ] {
        let selector = selector(kind, name, "alpha");
        let inventory = RustTestInventory {
            subjects: vec![subject(selector)],
            status: RustTestInventoryStatus::Complete,
            diagnostics: Vec::new(),
        };
        let resolution = resolve_authored_rust_subject(&anchor(target), &inventory);
        assert_eq!(
            resolution.class,
            IntentSubjectResolutionClassV1::ExactCurrent
        );
    }
}

#[test]
fn resolution_class_strings_are_stable() {
    for (class, expected) in [
        (IntentSubjectResolutionClassV1::ExactCurrent, "exact_current"),
        (
            IntentSubjectResolutionClassV1::ExactBodyChanged,
            "exact_body_changed",
        ),
        (IntentSubjectResolutionClassV1::Ambiguous, "ambiguous"),
        (IntentSubjectResolutionClassV1::Missing, "missing"),
        (IntentSubjectResolutionClassV1::Ignored, "ignored"),
        (
            IntentSubjectResolutionClassV1::GeneratedOrParameterized,
            "generated_or_parameterized",
        ),
        (
            IntentSubjectResolutionClassV1::ConditionalUnknown,
            "conditional_unknown",
        ),
        (
            IntentSubjectResolutionClassV1::PartialInventory,
            "partial_inventory",
        ),
        (
            IntentSubjectResolutionClassV1::MalformedAnchor,
            "malformed_anchor",
        ),
    ] {
        assert_eq!(class.as_str(), expected);
    }
}

fn anchor(target: &str) -> EvidenceSubjectRegistration {
    EvidenceSubjectRegistration {
        id: EvidenceSubjectId("subject:demo:alpha".to_string()),
        role: EvidenceSubjectRole::ExactEvidence,
        package: "demo".to_string(),
        target: target.to_string(),
        module_path: "tests".to_string(),
        test_name: "alpha".to_string(),
        source: SourceLocation::new("src/lib.rs"),
        source_identity: "fnv1a64:current".to_string(),
    }
}

fn empty_inventory() -> RustTestInventory {
    RustTestInventory {
        subjects: Vec::new(),
        status: RustTestInventoryStatus::Complete,
        diagnostics: Vec::new(),
    }
}

fn selector(kind: RustTestTargetKind, name: &str, function: &str) -> RustTestSelector {
    RustTestSelector {
        package: "demo".to_string(),
        target: RustTestTargetIdentity {
            kind,
            name: name.to_string(),
        },
        module_path: vec!["tests".to_string()],
        function: function.to_string(),
    }
}

fn subject(selector: RustTestSelector) -> RustTestSubject {
    RustTestSubject {
        selector,
        source_path: "src/lib.rs".to_string(),
        source_range: RustTestSourceRange {
            start_line: 1,
            start_column: 1,
            end_line: 3,
            end_column: 2,
        },
        body_identity: "fnv1a64:current".to_string(),
        attributes: vec!["test".to_string()],
        generated_or_parameterized: false,
        cfg_or_feature_unknown: false,
        ignored: false,
        limitations: Vec::new(),
    }
}
