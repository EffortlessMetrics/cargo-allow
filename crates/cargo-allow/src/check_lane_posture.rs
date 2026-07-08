use allow_core::{AllowConfig, Finding, FindingKind, MatchOutcome};
use allow_match::CheckMode;

pub(crate) fn check_outcome_fails(
    outcome: &MatchOutcome,
    findings: &[Finding],
    cfg: &AllowConfig,
    mode: CheckMode,
) -> bool {
    if let Some(kind) = outcome_kind(outcome, findings, cfg) {
        if !cfg
            .lane_enforcement_mode_for_kind(kind)
            .blocks_check_failure()
        {
            return false;
        }
    }
    mode.fails(outcome.status)
}

pub(crate) fn check_failed_for_outcomes(
    outcomes: &[MatchOutcome],
    findings: &[Finding],
    cfg: &AllowConfig,
    mode: CheckMode,
) -> bool {
    outcomes
        .iter()
        .any(|outcome| check_outcome_fails(outcome, findings, cfg, mode))
}

fn outcome_kind(
    outcome: &MatchOutcome,
    findings: &[Finding],
    cfg: &AllowConfig,
) -> Option<FindingKind> {
    if let Some(index) = outcome.finding_index {
        if let Some(finding) = findings.get(index) {
            return Some(finding.kind);
        }
    }
    if let Some(allow_id) = &outcome.allow_id {
        if let Some(entry) = cfg.allow.iter().find(|entry| entry.id == *allow_id) {
            return Some(entry.kind);
        }
    }
    None
}

#[cfg(test)]
#[path = "check_lane_posture_tests.rs"]
mod tests;
