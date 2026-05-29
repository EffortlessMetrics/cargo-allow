use crate::ReportContext;
use allow_core::{AllowConfig, MatchOutcome, MatchStatus};
use std::collections::BTreeMap;

pub(crate) const STATUS_COUNT_ORDER: [MatchStatus; 10] = [
    MatchStatus::Matched,
    MatchStatus::New,
    MatchStatus::Expired,
    MatchStatus::ReviewDue,
    MatchStatus::Stale,
    MatchStatus::Ambiguous,
    MatchStatus::InvalidSelector,
    MatchStatus::EvidenceMissing,
    MatchStatus::MissingRequiredField,
    MatchStatus::BaselineDebt,
];

pub(crate) const REVIEW_ITEM_STATUSES: [MatchStatus; 8] = [
    MatchStatus::New,
    MatchStatus::Expired,
    MatchStatus::ReviewDue,
    MatchStatus::Stale,
    MatchStatus::Ambiguous,
    MatchStatus::InvalidSelector,
    MatchStatus::MissingRequiredField,
    MatchStatus::EvidenceMissing,
];

pub(crate) const AUDIT_REVIEW_QUEUE_STATUSES: [MatchStatus; 8] = [
    MatchStatus::New,
    MatchStatus::Expired,
    MatchStatus::Ambiguous,
    MatchStatus::EvidenceMissing,
    MatchStatus::MissingRequiredField,
    MatchStatus::BaselineDebt,
    MatchStatus::Stale,
    MatchStatus::ReviewDue,
];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Summary {
    pub total: usize,
    pub by_status: BTreeMap<MatchStatus, usize>,
}

impl Summary {
    pub fn from_outcomes(outcomes: &[MatchOutcome]) -> Self {
        let mut summary = Self {
            total: outcomes.len(),
            by_status: BTreeMap::new(),
        };
        for outcome in outcomes {
            *summary.by_status.entry(outcome.status).or_insert(0) += 1;
        }
        summary
    }

    pub fn count(&self, status: MatchStatus) -> usize {
        *self.by_status.get(&status).unwrap_or(&0)
    }
}

pub(crate) fn render_counts_fields(summary: &Summary, indent: &str) -> String {
    render_counts_fields_with_policy_baseline(summary, None, indent)
}

pub(crate) fn render_counts_fields_with_policy_baseline(
    summary: &Summary,
    policy_baseline_debt: Option<usize>,
    indent: &str,
) -> String {
    let include_policy_baseline_debt =
        policy_baseline_debt.filter(|count| *count > summary.count(MatchStatus::BaselineDebt));
    let mut out = STATUS_COUNT_ORDER
        .iter()
        .enumerate()
        .map(|(idx, status)| {
            let comma =
                if idx + 1 == STATUS_COUNT_ORDER.len() && include_policy_baseline_debt.is_none() {
                    ""
                } else {
                    ","
                };
            format!(
                "{indent}\"{}\": {}{comma}\n",
                status.as_str(),
                summary.count(*status)
            )
        })
        .collect::<String>();
    if let Some(policy_baseline_debt) = include_policy_baseline_debt {
        out.push_str(&format!(
            "{indent}\"policy_baseline_debt\": {policy_baseline_debt}\n"
        ));
    }
    out
}

pub(crate) fn review_item_count_with_baseline(summary: &Summary, baseline_debt: usize) -> usize {
    REVIEW_ITEM_STATUSES
        .iter()
        .map(|status| summary.count(*status))
        .sum::<usize>()
        + baseline_debt
}

pub(crate) fn baseline_debt_count(summary: &Summary, context: ReportContext<'_>) -> usize {
    context
        .baseline_debt_entries
        .unwrap_or_else(|| summary.count(MatchStatus::BaselineDebt))
}

pub fn policy_baseline_debt_entries(cfg: &AllowConfig) -> usize {
    cfg.allow
        .iter()
        .filter(|entry| entry.classification == "baseline_debt")
        .count()
}

pub(crate) fn audit_review_queue(outcomes: &[MatchOutcome]) -> Vec<&MatchOutcome> {
    outcomes
        .iter()
        .filter(|outcome| outcome.status != MatchStatus::Matched)
        .take(20)
        .collect()
}
