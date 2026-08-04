use super::*;
use allow_core::{CargoAllowErrorKind, MatchStatus};
use allow_report::{ReportContext, Summary};

fn outcome(status: MatchStatus) -> allow_core::MatchOutcome {
    allow_core::MatchOutcome {
        status,
        allow_id: None,
        candidate_ids: Vec::new(),
        finding_index: None,
        message: status.as_str().to_string(),
        score: 0,
    }
}

#[test]
fn validate_deny_statuses_accepts_receipt_advisory_fields() {
    let summary = Summary::from_outcomes(&[outcome(MatchStatus::ReviewDue)]);
    validate_deny_statuses(
        &["review_due".to_string(), "baseline_debt".to_string()],
        &summary,
        ReportContext::default(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("expected valid deny statuses: {err}")));
}

#[test]
fn validate_deny_statuses_rejects_unknown_status() {
    let summary = Summary::from_outcomes(&[outcome(MatchStatus::Matched)]);
    let err = validate_deny_statuses(
        &["not_a_status".to_string()],
        &summary,
        ReportContext::default(),
    )
    .expect_err("unknown deny status should fail closed");
    assert!(
        err.to_string()
            .contains("unknown --deny status `not_a_status`")
    );
    assert_eq!(err.kind(), CargoAllowErrorKind::Usage);
    assert!(err.to_string().contains("review_due"));
}

#[test]
fn validate_deny_statuses_rejects_absent_optional_advisory_class() {
    let summary = Summary::from_outcomes(&[outcome(MatchStatus::Matched)]);
    let err = validate_deny_statuses(
        &["occurrence_headroom".to_string()],
        &summary,
        ReportContext::default(),
    )
    .expect_err("absent optional advisory class should fail closed");

    assert!(
        err.to_string()
            .contains("unknown --deny status `occurrence_headroom`")
    );
    assert_eq!(err.kind(), CargoAllowErrorKind::Usage);
    assert!(
        !err.to_string().contains("occurrence_headroom,"),
        "absent optional classes should not be listed as supported: {err}"
    );
}

#[test]
fn validate_deny_statuses_accepts_present_optional_advisory_class() {
    let summary = Summary::from_outcomes(&[outcome(MatchStatus::Matched)]);
    let context = ReportContext {
        occurrence_headroom_entries: Some(2),
        ..ReportContext::default()
    };
    validate_deny_statuses(&["occurrence_headroom".to_string()], &summary, context).unwrap_or_else(
        |err| {
            std::panic::panic_any(format!(
                "present occurrence_headroom should be supported: {err}"
            ))
        },
    );
}

#[test]
fn deny_escalation_failed_when_occurrence_headroom_count_is_positive() {
    let summary = Summary::from_outcomes(&[outcome(MatchStatus::Matched)]);
    let context = ReportContext {
        occurrence_headroom_entries: Some(2),
        ..ReportContext::default()
    };
    assert!(deny_escalation_failed(
        &["occurrence_headroom".to_string()],
        &summary,
        context
    ));
}

#[test]
fn deny_escalation_failed_when_denied_advisory_count_is_positive() {
    let summary = Summary::from_outcomes(&[outcome(MatchStatus::ReviewDue)]);
    assert!(deny_escalation_failed(
        &["review_due".to_string()],
        &summary,
        ReportContext::default()
    ));
}

#[test]
fn deny_escalation_passes_when_denied_advisory_count_is_zero() {
    let summary = Summary::from_outcomes(&[outcome(MatchStatus::Matched)]);
    assert!(!deny_escalation_failed(
        &["review_due".to_string()],
        &summary,
        ReportContext::default()
    ));
}
