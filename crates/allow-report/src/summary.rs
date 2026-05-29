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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReviewSignals {
    pub(crate) baseline_debt: usize,
    pub(crate) broken_evidence_links: usize,
    pub(crate) review_items: usize,
}

impl ReviewSignals {
    pub(crate) fn from_summary(summary: &Summary, context: ReportContext<'_>) -> Self {
        let baseline_debt = baseline_debt_count(summary, context);
        let broken_evidence_links = broken_evidence_link_count(context);
        let review_items =
            review_item_count_with_baseline(summary, baseline_debt, broken_evidence_links);
        Self {
            baseline_debt,
            broken_evidence_links,
            review_items,
        }
    }
}

pub(crate) fn render_counts_fields_with_policy_baseline(
    summary: &Summary,
    policy_baseline_debt: Option<usize>,
    broken_evidence_links: Option<usize>,
    indent: &str,
) -> String {
    let include_policy_baseline_debt =
        policy_baseline_debt.filter(|count| *count > summary.count(MatchStatus::BaselineDebt));
    let include_broken_evidence_links = broken_evidence_links.filter(|count| *count > 0);
    let optional_fields = [
        ("policy_baseline_debt", include_policy_baseline_debt),
        ("broken_evidence_links", include_broken_evidence_links),
    ]
    .into_iter()
    .filter_map(|(name, value)| value.map(|value| (name, value)))
    .collect::<Vec<_>>();
    let mut out = STATUS_COUNT_ORDER
        .iter()
        .enumerate()
        .map(|(idx, status)| {
            let comma = if idx + 1 == STATUS_COUNT_ORDER.len() && optional_fields.is_empty() {
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
    for (index, (name, value)) in optional_fields.iter().enumerate() {
        let comma = if index + 1 == optional_fields.len() {
            ""
        } else {
            ","
        };
        out.push_str(&format!("{indent}\"{name}\": {value}{comma}\n"));
    }
    out
}

pub(crate) fn review_item_count_with_baseline(
    summary: &Summary,
    baseline_debt: usize,
    broken_evidence_links: usize,
) -> usize {
    REVIEW_ITEM_STATUSES
        .iter()
        .map(|status| summary.count(*status))
        .sum::<usize>()
        + baseline_debt
        + broken_evidence_links
}

pub(crate) fn baseline_debt_count(summary: &Summary, context: ReportContext<'_>) -> usize {
    context
        .baseline_debt_entries
        .unwrap_or_else(|| summary.count(MatchStatus::BaselineDebt))
}

pub(crate) fn broken_evidence_link_count(context: ReportContext<'_>) -> usize {
    context.broken_evidence_links.unwrap_or(0)
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
