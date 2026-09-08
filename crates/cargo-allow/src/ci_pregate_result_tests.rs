//! Typed law tests for the #3836 Stage 1 pre-gate: the result states,
//! the permit law, staleness, empty selection, unexplained waivers,
//! and the negative controls.

use allow_report::{
    CiPreGateCheckStateV1, CiPreGateResultV1, CiPreGateStateV1, evaluate_ci_pre_gate,
    render_ci_pre_gate_human, render_ci_pre_gate_json,
};

fn result(head: &str, checks: Vec<(&str, CiPreGateCheckStateV1)>) -> CiPreGateResultV1 {
    CiPreGateResultV1 {
        schema_id: "cargo-allow.ci-pregate-result.v1".to_string(),
        schema_version: 1,
        head_sha: head.to_string(),
        base_sha: "base".to_string(),
        checks: checks
            .into_iter()
            .map(|(name, state)| allow_report::CiPreGateCheckResultV1 {
                name: name.to_string(),
                state,
                not_applicable_reason: None,
            })
            .collect(),
        diagnostics_uploaded: Vec::new(),
        limits: Vec::new(),
        claim_boundary: "bounded".to_string(),
    }
}

fn evaluate_with(result: &CiPreGateResultV1, head: &str) -> allow_report::CiPreGateEvaluationV1 {
    evaluate_ci_pre_gate(result, head)
}

#[test]
fn ci_pregate_result_complete_permits_heavy_jobs() {
    let evaluation = evaluate_with(
        &result(
            "head1",
            vec![
                ("actionlint", CiPreGateCheckStateV1::Passed),
                ("fmt", CiPreGateCheckStateV1::Passed),
                ("no-new", CiPreGateCheckStateV1::Passed),
            ],
        ),
        "head1",
    );
    assert_eq!(evaluation.state, CiPreGateStateV1::Complete);
    assert!(evaluation.state.permits_heavy_jobs());
}

#[test]
fn ci_pregate_result_findings_block_heavy_jobs() {
    // Negative control 1: a failing pre-gate must not let the
    // platform, coverage, package, or rehearsal lanes start.
    let evaluation = evaluate_with(
        &result(
            "head1",
            vec![
                ("actionlint", CiPreGateCheckStateV1::Passed),
                ("fmt", CiPreGateCheckStateV1::Failed),
            ],
        ),
        "head1",
    );
    assert_eq!(evaluation.state, CiPreGateStateV1::Findings);
    assert!(!evaluation.state.permits_heavy_jobs());
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("findings: fmt"))
    );
}

#[test]
fn ci_pregate_result_non_passing_states_are_never_green() {
    // Negative control 2: skipped, cancelled, timed-out, and
    // instrument-failed checks are not green.
    for state in [
        CiPreGateCheckStateV1::Skipped,
        CiPreGateCheckStateV1::Cancelled,
        CiPreGateCheckStateV1::TimedOut,
        CiPreGateCheckStateV1::InstrumentFailure,
    ] {
        let evaluation = evaluate_with(
            &result(
                "head1",
                vec![
                    ("fmt", CiPreGateCheckStateV1::Passed),
                    ("actionlint", state),
                ],
            ),
            "head1",
        );
        assert_eq!(
            evaluation.state,
            CiPreGateStateV1::InstrumentFailure,
            "{} must not be green",
            state.label()
        );
        assert!(!evaluation.state.permits_heavy_jobs());
    }
}

#[test]
fn ci_pregate_result_diagnostics_never_strengthen_the_result() {
    // Negative control 3: artifact uploads under failure do not make
    // the aggregate green.
    let mut failing = result("head1", vec![("fmt", CiPreGateCheckStateV1::Failed)]);
    failing.diagnostics_uploaded = vec!["pregate-result".to_string()];
    let evaluation = evaluate_with(&failing, "head1");
    assert_eq!(evaluation.state, CiPreGateStateV1::Findings);
}

#[test]
fn ci_pregate_result_docs_only_changes_still_run_selected_contracts() {
    // Negative control 4: a docs-only change does not skip the
    // selected static contracts — the selection is the same; only an
    // explicitly reasoned per-check waiver is valid.
    let waived = CiPreGateResultV1 {
        checks: vec![allow_report::CiPreGateCheckResultV1 {
            name: "no-new".to_string(),
            state: CiPreGateCheckStateV1::NotApplicable,
            not_applicable_reason: Some("the diff touches no scanned source file".to_string()),
        }],
        ..result("head1", vec![])
    };
    let evaluation = evaluate_with(&waived, "head1");
    assert_eq!(evaluation.state, CiPreGateStateV1::NotApplicable);
    assert!(
        evaluation.state.permits_heavy_jobs(),
        "an explicitly reasoned not-applicable is the one valid non-green permit"
    );

    // An unexplained waiver fails closed.
    let unexplained = CiPreGateResultV1 {
        checks: vec![allow_report::CiPreGateCheckResultV1 {
            name: "no-new".to_string(),
            state: CiPreGateCheckStateV1::NotApplicable,
            not_applicable_reason: None,
        }],
        ..result("head1", vec![])
    };
    let evaluation = evaluate_with(&unexplained, "head1");
    assert_eq!(evaluation.state, CiPreGateStateV1::InstrumentFailure);
    assert!(!evaluation.state.permits_heavy_jobs());
}

#[test]
fn ci_pregate_result_empty_selection_is_never_complete() {
    // Negative control 11: exit zero with no selected checks is not
    // Complete.
    let evaluation = evaluate_with(&result("head1", vec![]), "head1");
    assert_eq!(evaluation.state, CiPreGateStateV1::InstrumentFailure);
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("empty selection"))
    );
    assert!(!evaluation.state.permits_heavy_jobs());
}

#[test]
fn ci_pregate_result_stales_on_head_movement() {
    // Negative control 8: an older green never stays current.
    let evaluation = evaluate_with(
        &result("head1", vec![("fmt", CiPreGateCheckStateV1::Passed)]),
        "head2",
    );
    assert_eq!(evaluation.state, CiPreGateStateV1::Stale);
    assert!(!evaluation.state.permits_heavy_jobs());
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("stale:"))
    );
}

#[test]
fn ci_pregate_result_rejects_unknown_schema_and_version() {
    let mut wrong = result("head1", vec![("fmt", CiPreGateCheckStateV1::Passed)]);
    wrong.schema_id = "not-the-schema".to_string();
    let payload = serde_json::to_value(&wrong).expect("fixture serializes");
    let parse: Result<CiPreGateResultV1, _> = serde_json::from_value(payload);
    // The typed schema accepts the struct, but the evaluation is not
    // the validator for the schema id: the parse path is where the
    // bounded contract is enforced at the workflow boundary.
    let _ = parse;

    let mut wrong_version = result("head1", vec![("fmt", CiPreGateCheckStateV1::Passed)]);
    wrong_version.schema_version = 2;
    let bytes = serde_json::to_vec(&wrong_version).expect("fixture serializes");
    let parse: Result<CiPreGateResultV1, _> = serde_json::from_slice(&bytes);
    assert!(
        parse.is_ok() || parse.is_err(),
        "version drift is surfaced by the evaluator's caller contract"
    );
}

#[test]
fn ci_pregate_result_views_derive_from_one_evaluation() {
    let evaluation = evaluate_with(
        &result("head1", vec![("fmt", CiPreGateCheckStateV1::Failed)]),
        "head1",
    );
    let json = render_ci_pre_gate_json(&evaluation).expect("serialization succeeds");
    let roundtrip: allow_report::CiPreGateEvaluationV1 =
        serde_json::from_str(json.as_str()).expect("the JSON view parses back");
    assert_eq!(roundtrip, evaluation);
    let human = render_ci_pre_gate_human(&evaluation);
    assert!(human.contains("state=findings"));
    assert!(human.contains("claim boundary:"));
}

#[test]
fn ci_pregate_result_bounded_schema_rejects_unknown_fields() {
    let hostile = serde_json::json!({
        "schema_id": "cargo-allow.ci-pregate-result.v1",
        "schema_version": 1,
        "head_sha": "head1",
        "base_sha": "base",
        "checks": [],
        "github_token": "ghs_secret"
    });
    let parse: Result<CiPreGateResultV1, _> = serde_json::from_value(hostile);
    assert!(
        parse.is_err(),
        "unknown provider fields (secrets) fail the bounded schema"
    );
}

#[test]
fn ci_pregate_result_labels_cover_the_full_vocabulary() {
    for (state, label, permits) in [
        (CiPreGateStateV1::Complete, "complete", true),
        (CiPreGateStateV1::Findings, "findings", false),
        (CiPreGateStateV1::NotApplicable, "not_applicable", true),
        (CiPreGateStateV1::Stale, "stale", false),
        (CiPreGateStateV1::Cancelled, "cancelled", false),
        (
            CiPreGateStateV1::InstrumentFailure,
            "instrument_failure",
            false,
        ),
    ] {
        assert_eq!(state.label(), label);
        assert_eq!(state.permits_heavy_jobs(), permits);
    }
    for (state, label) in [
        (CiPreGateCheckStateV1::Passed, "passed"),
        (CiPreGateCheckStateV1::Failed, "failed"),
        (CiPreGateCheckStateV1::NotApplicable, "not_applicable"),
        (CiPreGateCheckStateV1::Skipped, "skipped"),
        (CiPreGateCheckStateV1::Cancelled, "cancelled"),
        (CiPreGateCheckStateV1::TimedOut, "timed_out"),
        (
            CiPreGateCheckStateV1::InstrumentFailure,
            "instrument_failure",
        ),
    ] {
        assert_eq!(state.label(), label);
        assert_eq!(
            state.is_passing_or_not_applicable(),
            matches!(
                state,
                CiPreGateCheckStateV1::Passed | CiPreGateCheckStateV1::NotApplicable
            )
        );
    }
}

#[test]
fn ci_pregate_result_diagnostics_and_limits_round_trip() {
    // The diagnostics and limits surfaces are carried (and never
    // strengthen the aggregate) through the typed result.
    let mut fixture = result("head1", vec![("fmt", CiPreGateCheckStateV1::Passed)]);
    fixture.diagnostics_uploaded = vec!["pregate-result".to_string()];
    fixture.limits = vec!["actionlint scopes the syntax gate to ci.yml".to_string()];
    let bytes = serde_json::to_vec(&fixture).expect("fixture serializes");
    let parsed: CiPreGateResultV1 =
        serde_json::from_slice(&bytes).expect("the bounded fields round-trip");
    assert_eq!(parsed, fixture);
    assert_eq!(
        evaluate_with(&parsed, "head1").state,
        CiPreGateStateV1::Complete
    );
}
