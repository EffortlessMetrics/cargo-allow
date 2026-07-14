use crate::ReportContext;
use crate::advisory_class::AdvisoryClass;
use allow_core::{AllowConfig, AllowEntry, MatchOutcome, MatchStatus};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const STATUS_COUNT_ORDER: [MatchStatus; 11] = [
    MatchStatus::Matched,
    MatchStatus::New,
    MatchStatus::Expired,
    MatchStatus::ReviewDue,
    MatchStatus::LocationDrift,
    MatchStatus::Stale,
    MatchStatus::Ambiguous,
    MatchStatus::InvalidSelector,
    MatchStatus::EvidenceMissing,
    MatchStatus::MissingRequiredField,
    MatchStatus::BaselineDebt,
];

pub(crate) const REVIEW_ITEM_STATUSES: [MatchStatus; 9] = [
    MatchStatus::New,
    MatchStatus::Expired,
    MatchStatus::ReviewDue,
    MatchStatus::LocationDrift,
    MatchStatus::Stale,
    MatchStatus::Ambiguous,
    MatchStatus::InvalidSelector,
    MatchStatus::MissingRequiredField,
    MatchStatus::EvidenceMissing,
];

pub(crate) const AUDIT_REVIEW_QUEUE_STATUSES: [MatchStatus; 9] = [
    MatchStatus::New,
    MatchStatus::Expired,
    MatchStatus::Ambiguous,
    MatchStatus::EvidenceMissing,
    MatchStatus::MissingRequiredField,
    MatchStatus::BaselineDebt,
    MatchStatus::Stale,
    MatchStatus::ReviewDue,
    MatchStatus::LocationDrift,
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
    pub(crate) policy_missing_evidence: usize,
    pub(crate) broken_evidence_links: usize,
    pub(crate) weak_evidence_references: usize,
    pub(crate) occurrence_headroom: usize,
    pub(crate) mirror_divergence: usize,
    pub(crate) review_items: usize,
}

impl ReviewSignals {
    pub(crate) fn from_summary(summary: &Summary, context: ReportContext<'_>) -> Self {
        let baseline_debt = baseline_debt_count(summary, context);
        let policy_missing_evidence = policy_missing_evidence_count(summary, context);
        let broken_evidence_links = broken_evidence_link_count(context);
        let weak_evidence_references = weak_evidence_reference_count(context);
        let occurrence_headroom = occurrence_headroom_count(context);
        let mirror_divergence = mirror_divergence_count(context);
        let review_items = review_item_count_with_baseline(
            summary,
            baseline_debt,
            policy_missing_evidence,
            broken_evidence_links,
            weak_evidence_references,
            occurrence_headroom,
            mirror_divergence,
        );
        Self {
            baseline_debt,
            policy_missing_evidence,
            broken_evidence_links,
            weak_evidence_references,
            occurrence_headroom,
            mirror_divergence,
            review_items,
        }
    }
}

pub(crate) fn render_count_fields_with_policy_context(
    summary: &Summary,
    policy_baseline_debt: Option<usize>,
    policy_missing_evidence: Option<usize>,
    broken_evidence_links: Option<usize>,
    weak_evidence_references: Option<usize>,
    indent: &str,
) -> String {
    let include_policy_baseline_debt =
        policy_baseline_debt.filter(|count| *count > summary.count(MatchStatus::BaselineDebt));
    let include_policy_missing_evidence = policy_missing_evidence
        .filter(|count| *count > summary.count(MatchStatus::EvidenceMissing));
    let include_broken_evidence_links = broken_evidence_links.filter(|count| *count > 0);
    let include_weak_evidence_references = weak_evidence_references.filter(|count| *count > 0);
    let optional_fields = [
        ("policy_baseline_debt", include_policy_baseline_debt),
        ("policy_missing_evidence", include_policy_missing_evidence),
        ("broken_evidence_links", include_broken_evidence_links),
        ("weak_evidence_references", include_weak_evidence_references),
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

pub(crate) fn render_advisory_count_fields(
    summary: &Summary,
    context: ReportContext<'_>,
    indent: &str,
) -> String {
    let fields = AdvisoryClass::receipt_fields(summary, context);
    fields
        .iter()
        .enumerate()
        .map(|(idx, (class, value))| {
            let comma = if idx + 1 == fields.len() { "" } else { "," };
            format!("{indent}\"{}\": {value}{comma}\n", class.field_name())
        })
        .collect()
}

pub(crate) fn review_item_count_with_baseline(
    summary: &Summary,
    baseline_debt: usize,
    policy_missing_evidence: usize,
    broken_evidence_links: usize,
    weak_evidence_references: usize,
    occurrence_headroom: usize,
    mirror_divergence: usize,
) -> usize {
    let policy_missing_evidence_extra =
        policy_missing_evidence.saturating_sub(summary.count(MatchStatus::EvidenceMissing));
    REVIEW_ITEM_STATUSES
        .iter()
        .map(|status| summary.count(*status))
        .sum::<usize>()
        + baseline_debt
        + policy_missing_evidence_extra
        + broken_evidence_links
        + weak_evidence_references
        + occurrence_headroom
        + mirror_divergence
}

pub(crate) fn baseline_debt_count(summary: &Summary, context: ReportContext<'_>) -> usize {
    context
        .baseline_debt_entries
        .unwrap_or_else(|| summary.count(MatchStatus::BaselineDebt))
}

pub(crate) fn broken_evidence_link_count(context: ReportContext<'_>) -> usize {
    context.broken_evidence_links.unwrap_or(0)
}

pub(crate) fn weak_evidence_reference_count(context: ReportContext<'_>) -> usize {
    context.weak_evidence_references.unwrap_or(0)
}

pub(crate) fn occurrence_headroom_count(context: ReportContext<'_>) -> usize {
    context.occurrence_headroom_entries.unwrap_or(0)
}

pub(crate) fn mirror_divergence_count(context: ReportContext<'_>) -> usize {
    context.mirror_divergence_entries.unwrap_or(0)
}

pub fn matched_occurrence_counts(outcomes: &[MatchOutcome]) -> BTreeMap<String, u32> {
    let mut counts = BTreeMap::new();
    for outcome in outcomes {
        if outcome.status != MatchStatus::Matched {
            continue;
        }
        if let Some(allow_id) = &outcome.allow_id {
            *counts.entry(allow_id.clone()).or_insert(0) += 1;
        }
    }
    counts
}

pub fn occurrence_headroom_for_entry(entry: &AllowEntry, matched_count: u32) -> Option<u32> {
    let limit = entry.occurrence_limit?;
    (matched_count > 0 && matched_count < limit).then_some(limit - matched_count)
}

pub fn occurrence_headroom_entries(cfg: &AllowConfig, outcomes: &[MatchOutcome]) -> usize {
    let counts = matched_occurrence_counts(outcomes);
    cfg.allow
        .iter()
        .filter(|entry| {
            counts
                .get(&entry.id)
                .copied()
                .and_then(|count| occurrence_headroom_for_entry(entry, count))
                .is_some()
        })
        .count()
}

pub(crate) fn policy_missing_evidence_count(
    summary: &Summary,
    context: ReportContext<'_>,
) -> usize {
    context
        .policy_missing_evidence_entries
        .unwrap_or_else(|| summary.count(MatchStatus::EvidenceMissing))
}

pub fn policy_baseline_debt_entries(cfg: &AllowConfig) -> usize {
    cfg.allow
        .iter()
        .filter(|entry| entry.classification == "baseline_debt")
        .count()
}

pub fn policy_missing_evidence_entries(cfg: &AllowConfig) -> usize {
    cfg.allow
        .iter()
        .filter(|entry| entry.evidence.is_empty())
        .count()
}

pub fn matched_policy_missing_evidence_entries(
    cfg: &AllowConfig,
    outcomes: &[MatchOutcome],
) -> usize {
    let matched_allow_ids = outcomes
        .iter()
        .filter(|outcome| outcome.status == MatchStatus::Matched)
        .filter_map(|outcome| outcome.allow_id.as_deref())
        .collect::<BTreeSet<_>>();
    cfg.allow
        .iter()
        .filter(|entry| entry.classification != "baseline_debt")
        .filter(|entry| entry.evidence.is_empty())
        .filter(|entry| matched_allow_ids.contains(entry.id.as_str()))
        .count()
}

pub(crate) fn audit_review_queue(outcomes: &[MatchOutcome]) -> Vec<&MatchOutcome> {
    outcomes
        .iter()
        .filter(|outcome| outcome.status != MatchStatus::Matched)
        .take(20)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{AllowEntry, FindingKind, Lifecycle, Selector};
    use std::path::PathBuf;

    fn outcome(status: MatchStatus, allow_id: Option<&str>) -> MatchOutcome {
        MatchOutcome {
            status,
            allow_id: allow_id.map(str::to_string),
            candidate_ids: Vec::new(),
            finding_index: None,
            message: status.as_str().to_string(),
            score: 0,
        }
    }

    fn entry(id: &str, classification: &str, evidence: &[&str]) -> AllowEntry {
        AllowEntry {
            id: id.to_string(),
            kind: FindingKind::PolicyException,
            family: None,
            path: Some(PathBuf::from("src/lib.rs")),
            glob: None,
            owner: "owner".to_string(),
            classification: classification.to_string(),
            reason: "reason".to_string(),
            evidence: evidence.iter().map(|item| (*item).to_string()).collect(),
            links: Vec::new(),
            occurrence_limit: None,
            lifecycle: Lifecycle::empty(),
            selector: Selector::default(),
            last_seen: None,
        }
    }

    #[test]
    fn summary_counts_each_match_status_and_defaults_missing_statuses_to_zero() {
        let outcomes = vec![
            outcome(MatchStatus::Matched, Some("matched")),
            outcome(MatchStatus::Matched, Some("matched-again")),
            outcome(MatchStatus::New, Some("new")),
            outcome(MatchStatus::Expired, Some("expired")),
            outcome(MatchStatus::ReviewDue, Some("review")),
            outcome(MatchStatus::Stale, Some("stale")),
            outcome(MatchStatus::Ambiguous, Some("ambiguous")),
            outcome(MatchStatus::InvalidSelector, Some("invalid")),
            outcome(MatchStatus::EvidenceMissing, Some("evidence")),
            outcome(MatchStatus::MissingRequiredField, Some("missing")),
        ];

        let summary = Summary::from_outcomes(&outcomes);

        assert_eq!(summary.total, 10);
        assert_eq!(summary.count(MatchStatus::Matched), 2);
        assert_eq!(summary.count(MatchStatus::New), 1);
        assert_eq!(summary.count(MatchStatus::Expired), 1);
        assert_eq!(summary.count(MatchStatus::ReviewDue), 1);
        assert_eq!(summary.count(MatchStatus::Stale), 1);
        assert_eq!(summary.count(MatchStatus::Ambiguous), 1);
        assert_eq!(summary.count(MatchStatus::InvalidSelector), 1);
        assert_eq!(summary.count(MatchStatus::EvidenceMissing), 1);
        assert_eq!(summary.count(MatchStatus::MissingRequiredField), 1);
        assert_eq!(summary.count(MatchStatus::BaselineDebt), 0);
    }

    #[test]
    fn review_signals_combine_summary_counts_and_policy_context() {
        let summary = Summary::from_outcomes(&[
            outcome(MatchStatus::New, Some("new")),
            outcome(MatchStatus::Expired, Some("expired")),
            outcome(MatchStatus::EvidenceMissing, Some("missing-evidence")),
            outcome(MatchStatus::BaselineDebt, Some("baseline")),
        ]);
        let mut context = ReportContext::source_syntax("git_tracked", None, None, Some(3));
        context.policy_missing_evidence_entries = Some(5);
        context.broken_evidence_links = Some(2);
        context.weak_evidence_references = Some(1);

        let signals = ReviewSignals::from_summary(&summary, context);

        assert_eq!(
            signals,
            ReviewSignals {
                baseline_debt: 3,
                policy_missing_evidence: 5,
                broken_evidence_links: 2,
                weak_evidence_references: 1,
                occurrence_headroom: 0,
                mirror_divergence: 0,
                review_items: 13,
            }
        );
        assert_eq!(
            review_item_count_with_baseline(&summary, 3, 5, 2, 1, 0, 0),
            13
        );
    }

    #[test]
    fn render_count_fields_lists_statuses_and_policy_context_excess() {
        let summary = Summary::from_outcomes(&[
            outcome(MatchStatus::Matched, Some("matched")),
            outcome(MatchStatus::EvidenceMissing, Some("missing-evidence")),
            outcome(MatchStatus::BaselineDebt, Some("baseline")),
        ]);

        let rendered = render_count_fields_with_policy_context(
            &summary,
            Some(3),
            Some(4),
            Some(2),
            Some(1),
            "  ",
        );

        assert!(rendered.contains("  \"matched\": 1,"));
        assert!(rendered.contains("  \"new\": 0,"));
        assert!(rendered.contains("  \"evidence_missing\": 1,"));
        assert!(rendered.contains("  \"baseline_debt\": 1,"));
        assert!(rendered.contains("  \"policy_baseline_debt\": 3,"));
        assert!(rendered.contains("  \"policy_missing_evidence\": 4,"));
        assert!(rendered.contains("  \"broken_evidence_links\": 2,"));
        assert!(rendered.contains("  \"weak_evidence_references\": 1\n"));

        let without_excess = render_count_fields_with_policy_context(
            &summary,
            Some(1),
            Some(1),
            Some(0),
            Some(0),
            "",
        );
        assert!(!without_excess.contains("policy_baseline_debt"));
        assert!(!without_excess.contains("policy_missing_evidence"));
        assert!(!without_excess.contains("broken_evidence_links"));
        assert!(!without_excess.contains("weak_evidence_references"));
        assert!(without_excess.ends_with("\"baseline_debt\": 1\n"));
    }

    #[test]
    fn context_count_helpers_use_context_override_or_summary_fallback() {
        let summary = Summary::from_outcomes(&[
            outcome(MatchStatus::EvidenceMissing, Some("missing-evidence")),
            outcome(MatchStatus::BaselineDebt, Some("baseline")),
        ]);
        let mut context = ReportContext::default();

        assert_eq!(baseline_debt_count(&summary, context), 1);
        assert_eq!(policy_missing_evidence_count(&summary, context), 1);
        assert_eq!(broken_evidence_link_count(context), 0);
        assert_eq!(weak_evidence_reference_count(context), 0);

        context.baseline_debt_entries = Some(4);
        context.policy_missing_evidence_entries = Some(5);
        context.broken_evidence_links = Some(2);
        context.weak_evidence_references = Some(3);

        assert_eq!(baseline_debt_count(&summary, context), 4);
        assert_eq!(policy_missing_evidence_count(&summary, context), 5);
        assert_eq!(broken_evidence_link_count(context), 2);
        assert_eq!(weak_evidence_reference_count(context), 3);
    }

    #[test]
    fn policy_entry_helpers_count_baseline_and_matched_missing_evidence() {
        let mut cfg = AllowConfig::empty();
        cfg.allow.push(entry("matched-missing", "reviewed", &[]));
        cfg.allow
            .push(entry("matched-evidenced", "reviewed", &["test:covered"]));
        cfg.allow.push(entry("stale-missing", "reviewed", &[]));
        cfg.allow
            .push(entry("baseline-missing", "baseline_debt", &[]));
        let outcomes = vec![
            outcome(MatchStatus::Matched, Some("matched-missing")),
            outcome(MatchStatus::Matched, Some("matched-evidenced")),
            outcome(MatchStatus::Stale, Some("stale-missing")),
            outcome(MatchStatus::Matched, Some("baseline-missing")),
            outcome(MatchStatus::Matched, None),
        ];

        assert_eq!(policy_baseline_debt_entries(&cfg), 1);
        assert_eq!(policy_missing_evidence_entries(&cfg), 3);
        assert_eq!(matched_policy_missing_evidence_entries(&cfg, &outcomes), 1);
    }

    #[test]
    fn advisory_count_fields_include_optional_evidence_signals_when_nonzero() {
        let outcomes = vec![
            outcome(MatchStatus::Matched, Some("matched")),
            outcome(MatchStatus::New, Some("new")),
            outcome(MatchStatus::Expired, Some("expired")),
            outcome(MatchStatus::ReviewDue, Some("review")),
            outcome(MatchStatus::Stale, Some("stale")),
            outcome(MatchStatus::Ambiguous, Some("ambiguous")),
            outcome(MatchStatus::InvalidSelector, Some("invalid")),
            outcome(MatchStatus::MissingRequiredField, Some("missing")),
            outcome(MatchStatus::EvidenceMissing, Some("evidence")),
            outcome(MatchStatus::BaselineDebt, Some("baseline")),
        ];
        let summary = Summary::from_outcomes(&outcomes);
        let mut context = ReportContext::source_syntax("git_tracked", None, None, Some(5));
        context.policy_missing_evidence_entries = Some(4);
        context.broken_evidence_links = Some(2);
        context.weak_evidence_references = Some(3);

        let fields = render_advisory_count_fields(&summary, context, "    ");

        for expected in [
            "\"review_items\": 21,",
            "\"new\": 1,",
            "\"expired\": 1,",
            "\"review_due\": 1,",
            "\"stale\": 1,",
            "\"ambiguous\": 1,",
            "\"invalid_selector\": 1,",
            "\"missing_required_field\": 1,",
            "\"evidence_missing\": 1,",
            "\"baseline_debt\": 5,",
            "\"policy_missing_evidence\": 4,",
            "\"broken_evidence_links\": 2,",
            "\"weak_evidence_references\": 3",
        ] {
            assert!(fields.contains(expected), "{expected}\n{fields}");
        }
        assert!(!fields.contains("\"weak_evidence_references\": 3,"));
    }

    #[test]
    fn audit_review_queue_keeps_first_twenty_non_matched_outcomes() {
        let mut outcomes = vec![outcome(MatchStatus::Matched, Some("matched"))];
        outcomes.extend((0..25).map(|index| {
            let id = format!("new-{index}");
            outcome(MatchStatus::New, Some(&id))
        }));

        let queue = audit_review_queue(&outcomes);

        assert_eq!(queue.len(), 20);
        assert!(queue.iter().all(|item| item.status != MatchStatus::Matched));
        assert_eq!(queue[0].allow_id.as_deref(), Some("new-0"));
        assert_eq!(queue[19].allow_id.as_deref(), Some("new-19"));
    }

    #[test]
    fn occurrence_headroom_entries_count_matched_entries_below_limit() {
        let mut cfg = AllowConfig::empty();
        let mut capped = entry("allow-capped", "reviewed", &["test:capped"]);
        capped.occurrence_limit = Some(5);
        let mut at_limit = entry("allow-full", "reviewed", &["test:full"]);
        at_limit.occurrence_limit = Some(2);
        let mut stale = entry("allow-stale", "reviewed", &["test:stale"]);
        stale.occurrence_limit = Some(3);
        cfg.allow.push(capped.clone());
        cfg.allow.push(at_limit.clone());
        cfg.allow.push(stale.clone());
        let outcomes = vec![
            outcome(MatchStatus::Matched, Some("allow-capped")),
            outcome(MatchStatus::Matched, Some("allow-capped")),
            outcome(MatchStatus::Matched, Some("allow-full")),
            outcome(MatchStatus::Matched, Some("allow-full")),
            outcome(MatchStatus::Stale, Some("allow-stale")),
        ];

        assert_eq!(occurrence_headroom_entries(&cfg, &outcomes), 1);
        assert_eq!(
            occurrence_headroom_for_entry(&capped, 2),
            Some(3),
            "headroom should report the remaining capacity below the limit"
        );
        assert_eq!(
            occurrence_headroom_for_entry(&at_limit, 2),
            None,
            "at-limit entries should not report headroom"
        );
        assert_eq!(
            occurrence_headroom_for_entry(&stale, 0),
            None,
            "unused entries should not report headroom"
        );
    }
}
