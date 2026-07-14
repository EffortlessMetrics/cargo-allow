use allow_core::{AllowEntry, Finding, MatchOutcome, MatchStatus, SimpleDate, normalize_path};

pub(crate) fn finding_location_text(finding: &Finding) -> String {
    match &finding.span {
        Some(span) => format!(
            "{}:{}:{}",
            normalize_path(&finding.path),
            span.line,
            span.column
        ),
        None => normalize_path(&finding.path),
    }
}

pub(crate) fn explain_report_status(entry: &AllowEntry, outcomes: &[MatchOutcome]) -> MatchStatus {
    crate::ledger_read_state_for_outcomes(entry, outcomes, SimpleDate::today_utc_approx()).status
}
