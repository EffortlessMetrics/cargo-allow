use super::*;

fn sample_diagnostic() -> CargoAllowDiagnosticV1 {
    CargoAllowDiagnosticV1 {
        rule_id: "panic.unwrap".to_string(),
        rule_generation: 1,
        subject_key: "src/lib.rs::load::unwrap".to_string(),
        severity: DiagnosticSeverity::High,
        posture: RulePosture::Blocking,
        confidence: DiagnosticConfidence::Exact,
        result_class: DiagnosticResultClass::Finding,
        primary_location: SourceRange::file("src/lib.rs")
            .with_span(
                SourcePosition::precise(10, 5),
                SourcePosition::precise(10, 20),
            )
            .with_content_identity("sha256:v1:abc"),
        related: vec![RelatedLocation {
            role: RelatedRole::TestSubject,
            range: SourceRange::file("tests/load.rs"),
            note: Some("covering test".to_string()),
        }],
        missing_obligation: Some(MissingObligation::EvidenceSubjectMissingOrAmbiguous),
        snapshot_identity: "sha256:v1:snapshot".to_string(),
        message: "unreceipted panic.unwrap".to_string(),
        actions: vec![CargoAllowActionV1::navigate(
            "open-src-lib",
            "open src/lib.rs:10",
        )],
    }
}

#[test]
fn actionable_diagnostic_model_keeps_the_four_judgment_dimensions_independent() {
    // Severity, posture, confidence, and result class are independent fields:
    // a high-severity finding can be advisory, and a blocking rule can be
    // low-severity. Constructing such combinations must be representable.
    let advisory_high = CargoAllowDiagnosticV1 {
        severity: DiagnosticSeverity::High,
        posture: RulePosture::Advisory,
        confidence: DiagnosticConfidence::Bounded,
        result_class: DiagnosticResultClass::NotProven,
        ..sample_diagnostic()
    };
    assert_eq!(advisory_high.severity, DiagnosticSeverity::High);
    assert!(!advisory_high.posture.is_blocking());
    assert_eq!(advisory_high.confidence, DiagnosticConfidence::Bounded);

    // An instrument failure is not a repository condition, even at high severity.
    let instrument = CargoAllowDiagnosticV1 {
        severity: DiagnosticSeverity::Critical,
        result_class: DiagnosticResultClass::InstrumentFailure,
        ..sample_diagnostic()
    };
    assert!(!instrument.result_class.is_repository_condition());
    assert!(DiagnosticResultClass::Finding.is_repository_condition());
    assert!(DiagnosticResultClass::Unsupported.result_class_is_tool_side());
}

#[test]
fn actionable_diagnostic_model_fingerprint_is_deterministic_and_identity_bound() {
    let diagnostic = sample_diagnostic();
    // Deterministic across calls.
    assert_eq!(diagnostic.fingerprint(), diagnostic.fingerprint());
    assert!(diagnostic.fingerprint().starts_with("sha256:v1:"));

    // Stable across pure presentation/attribute changes (message, actions,
    // severity, posture, confidence are not identity).
    let reworded = CargoAllowDiagnosticV1 {
        message: "completely different wording".to_string(),
        actions: Vec::new(),
        severity: DiagnosticSeverity::Low,
        posture: RulePosture::Informational,
        confidence: DiagnosticConfidence::Uncertain,
        ..sample_diagnostic()
    };
    assert_eq!(
        diagnostic.fingerprint(),
        reworded.fingerprint(),
        "fingerprint must survive message/action/attribute changes"
    );
}

#[test]
fn actionable_diagnostic_model_fingerprint_changes_on_semantic_identity_change() {
    let base = sample_diagnostic();
    let base_fp = base.fingerprint();

    let mutations: Vec<(&str, CargoAllowDiagnosticV1)> = vec![
        (
            "rule",
            CargoAllowDiagnosticV1 {
                rule_id: "panic.expect".to_string(),
                ..sample_diagnostic()
            },
        ),
        (
            "generation",
            CargoAllowDiagnosticV1 {
                rule_generation: 2,
                ..sample_diagnostic()
            },
        ),
        (
            "subject",
            CargoAllowDiagnosticV1 {
                subject_key: "src/other.rs::x".to_string(),
                ..sample_diagnostic()
            },
        ),
        (
            "result class",
            CargoAllowDiagnosticV1 {
                result_class: DiagnosticResultClass::Stale,
                ..sample_diagnostic()
            },
        ),
        (
            "obligation",
            CargoAllowDiagnosticV1 {
                missing_obligation: Some(MissingObligation::ProofCommandMissingOrIncompatible),
                ..sample_diagnostic()
            },
        ),
        (
            "location path",
            CargoAllowDiagnosticV1 {
                primary_location: SourceRange::file("src/moved.rs"),
                ..sample_diagnostic()
            },
        ),
        (
            "location span",
            CargoAllowDiagnosticV1 {
                primary_location: SourceRange::file("src/lib.rs").with_span(
                    SourcePosition::precise(11, 5),
                    SourcePosition::precise(11, 20),
                ),
                ..sample_diagnostic()
            },
        ),
        (
            "snapshot",
            CargoAllowDiagnosticV1 {
                snapshot_identity: "sha256:v1:different".to_string(),
                ..sample_diagnostic()
            },
        ),
        (
            "encoding",
            CargoAllowDiagnosticV1 {
                primary_location: SourceRange {
                    encoding: SourceEncoding::Utf16,
                    ..sample_diagnostic().primary_location
                },
                ..sample_diagnostic()
            },
        ),
        (
            "position base",
            CargoAllowDiagnosticV1 {
                primary_location: SourceRange {
                    base: PositionBase::Zero,
                    ..sample_diagnostic().primary_location
                },
                ..sample_diagnostic()
            },
        ),
    ];
    for (label, mutated) in mutations {
        assert_ne!(
            base_fp,
            mutated.fingerprint(),
            "{label} change must alter the fingerprint"
        );
    }
}

#[test]
fn actionable_diagnostic_model_action_applicability_is_coherent() {
    // A navigation action can never be automatic.
    let bad = CargoAllowActionV1 {
        applicability: ActionApplicability::Automatic,
        ..CargoAllowActionV1::navigate("nav", "go")
    };
    assert!(!bad.applicability_is_coherent());

    // A deterministic safe edit may be automatic.
    let ok = CargoAllowActionV1 {
        id: "insert-field".to_string(),
        kind: ActionKind::AutomaticSafeEdit,
        applicability: ActionApplicability::Automatic,
        preconditions: vec!["field absent".to_string()],
        mutation_scope: Some("policy/allow.toml::owner".to_string()),
        expected_effect: "insert the required owner field".to_string(),
        rollback: Some("remove the inserted owner field".to_string()),
        required_proof: Some(RequiredProof {
            command_argv: vec![
                "check".to_string(),
                "--mode".to_string(),
                "no-new".to_string(),
            ],
            description: None,
        }),
        residual_claim: vec!["human review still required".to_string()],
    };
    assert!(ok.applicability_is_coherent());
    assert!(ok.kind.mutates_source());

    let diagnostic = CargoAllowDiagnosticV1 {
        actions: vec![ok, CargoAllowActionV1::navigate("nav", "go")],
        ..sample_diagnostic()
    };
    assert!(diagnostic.actions_are_coherent());
}

#[test]
fn actionable_diagnostic_model_auto_eligibility_matches_the_non_inventive_boundary() {
    // Only deterministic, non-inventive changes may be automatic.
    for kind in [
        ActionKind::AutomaticSafeEdit,
        ActionKind::GenerateOwnedArtifact,
        ActionKind::RefreshOrReissue,
    ] {
        assert!(
            kind.may_be_automatic(),
            "{} should be auto-eligible",
            kind.as_str()
        );
        assert!(kind.mutates_source());
    }
    // Creating an exception, previewing, running a command, navigating, or any
    // external/decision action must never be automatic — even though some of
    // them do mutate source.
    for kind in [
        ActionKind::SuppressOrExemptUnderPolicy,
        ActionKind::PreviewableWorkspaceEdit,
        ActionKind::RunCargoAllowCommand,
        ActionKind::OpenOrNavigate,
        ActionKind::ChooseBetweenAuthorities,
        ActionKind::PerformExternalAction,
        ActionKind::RequestRepositoryDecision,
        ActionKind::DeferWithTypedReason,
        ActionKind::NoSafeActionKnown,
    ] {
        assert!(
            !kind.may_be_automatic(),
            "{} must not be auto-eligible",
            kind.as_str()
        );
    }
    // Suppression/exemption mutates source but is inventive, so an Automatic
    // suppression action is incoherent.
    let auto_suppress = CargoAllowActionV1 {
        kind: ActionKind::SuppressOrExemptUnderPolicy,
        applicability: ActionApplicability::Automatic,
        ..CargoAllowActionV1::navigate("suppress", "exempt under policy")
    };
    assert!(auto_suppress.kind.mutates_source());
    assert!(!auto_suppress.applicability_is_coherent());
}

#[test]
fn actionable_diagnostic_model_line_only_location_is_explicitly_degraded() {
    let line_only = SourceRange::file("src/lib.rs")
        .with_span(SourcePosition::line_only(10), SourcePosition::line_only(10));
    assert!(!line_only.is_precise());
    let precise = SourceRange::file("src/lib.rs").with_span(
        SourcePosition::precise(10, 1),
        SourcePosition::precise(10, 5),
    );
    assert!(precise.is_precise());
    // File-level location is neither precise nor a fabricated position.
    assert!(!SourceRange::file("src/lib.rs").is_precise());
}

#[test]
fn actionable_diagnostic_model_batch_carries_snapshot_and_partial_boundary() {
    let batch = CargoAllowDiagnosticBatchV1::new("sha256:v1:snapshot")
        .with_diagnostic(sample_diagnostic())
        .with_partial_data(PartialDataBoundary::partial(vec![
            "one file exceeded the read cap".to_string(),
        ]));
    assert_eq!(batch.schema, DIAGNOSTIC_KERNEL_SCHEMA);
    assert_eq!(batch.snapshot_identity, "sha256:v1:snapshot");
    assert!(!batch.partial_data.complete);
    assert_eq!(batch.diagnostic_fingerprints().len(), 1);
    assert_eq!(
        batch.diagnostic_fingerprints()[0],
        sample_diagnostic().fingerprint()
    );
}
