use allow_core::{AllowConfig, Finding, FindingKind, MatchOutcome, MatchStatus};
use allow_match::CheckMode;

pub(crate) fn check_outcome_fails(
    outcome: &MatchOutcome,
    status: MatchStatus,
    findings: &[Finding],
    cfg: &AllowConfig,
    mode: CheckMode,
) -> bool {
    if let Some(kind) = outcome_kind(outcome, findings, cfg)
        && !cfg
            .lane_enforcement_mode_for_kind(kind)
            .blocks_check_failure()
    {
        return false;
    }
    if status == MatchStatus::Stale && cfg.requirements.stale_entries_fail {
        return true;
    }
    mode.fails(status)
}

pub(crate) fn check_failed_for_outcomes(
    outcomes: &[MatchOutcome],
    findings: &[Finding],
    cfg: &AllowConfig,
    mode: CheckMode,
) -> bool {
    let projected_statuses = allow_report::ledger_read_statuses(
        cfg,
        outcomes,
        allow_core::SimpleDate::today_utc_approx(),
    );

    outcomes.iter().any(|outcome| {
        let status = outcome
            .allow_id
            .as_deref()
            .and_then(|allow_id| projected_statuses.get(allow_id).copied())
            .unwrap_or(outcome.status);
        check_outcome_fails(outcome, status, findings, cfg, mode)
    })
}

fn outcome_kind(
    outcome: &MatchOutcome,
    findings: &[Finding],
    cfg: &AllowConfig,
) -> Option<FindingKind> {
    if let Some(index) = outcome.finding_index
        && let Some(finding) = findings.get(index)
    {
        return Some(finding.kind);
    }
    if let Some(allow_id) = &outcome.allow_id
        && let Some(entry) = cfg.allow.iter().find(|entry| entry.id == *allow_id)
    {
        return Some(entry.kind);
    }
    None
}

#[cfg(test)]
#[path = "check_lane_posture_tests.rs"]
mod tests;
