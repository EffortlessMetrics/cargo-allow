use allow_core::{AllowConfig, AllowEntry, MatchOutcome, MatchStatus, SimpleDate};
use std::collections::BTreeMap;

/// The canonical lifecycle and capacity projection shared by read surfaces.
///
/// This is intentionally an internal reporting model: it gives commands one
/// status precedence and keeps occurrence accounting available for the later
/// read-surface convergence slices without widening their output contracts yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LedgerReadState {
    pub status: MatchStatus,
    pub matched_count: u32,
    pub occurrence_limit: Option<u32>,
}

pub fn ledger_read_state(
    entry: &AllowEntry,
    outcomes: &[&MatchOutcome],
    today: SimpleDate,
) -> LedgerReadState {
    let status = lifecycle_status(entry, outcomes, today);
    let matched_count = outcomes
        .iter()
        .filter(|outcome| outcome.status == MatchStatus::Matched)
        .count() as u32;

    LedgerReadState {
        status,
        matched_count,
        occurrence_limit: entry.occurrence_limit,
    }
}

pub fn ledger_read_state_for_outcomes(
    entry: &AllowEntry,
    outcomes: &[MatchOutcome],
    today: SimpleDate,
) -> LedgerReadState {
    let outcome_refs = outcomes.iter().collect::<Vec<_>>();
    ledger_read_state(entry, &outcome_refs, today)
}

pub fn ledger_read_statuses<'a>(
    cfg: &'a AllowConfig,
    outcomes: &[MatchOutcome],
    today: SimpleDate,
) -> BTreeMap<&'a str, MatchStatus> {
    let mut entries_by_id = BTreeMap::new();
    for entry in &cfg.allow {
        entries_by_id.entry(entry.id.as_str()).or_insert(entry);
    }

    let mut outcomes_by_allow_id = BTreeMap::<&str, Vec<&MatchOutcome>>::new();
    for outcome in outcomes {
        if let Some(allow_id) = outcome.allow_id.as_deref() {
            outcomes_by_allow_id
                .entry(allow_id)
                .or_default()
                .push(outcome);
        }
    }

    entries_by_id
        .iter()
        .filter_map(|(allow_id, entry)| {
            let entry_outcomes = outcomes_by_allow_id.get(allow_id).map(Vec::as_slice)?;
            let status = ledger_read_state(entry, entry_outcomes, today).status;
            Some((*allow_id, status))
        })
        .collect()
}

/// The effective read status for ONE outcome row against the entry-level
/// projection from [`ledger_read_statuses`] (#4238).
///
/// A finding-level hard failure is authoritative and must never be downgraded
/// to the entry-level lifecycle annotation (#4644, #4238). The matcher
/// re-raises findings under expired broad (identity-free, unbounded) entries
/// as `New`; #4644 extends the same protection to every no-new failure
/// status, because the date-first `Expired`/`ReviewDue` projection used to
/// swallow finding-level `EvidenceMissing` (required SAFETY comment, required
/// entry evidence) and `InvalidSelector` (required or mismatched
/// `policy:<allow-id>`) rows, letting `check --mode no-new` pass exactly when
/// a mandatory requirement was unmet. Equal-strength ambiguity and
/// malformed-policy refusals keep the same protection.
///
/// Every other row keeps the entry projection, so plain `Matched` rows stay
/// annotated as `Expired`/`ReviewDue` for visibility (the L1/L6 surface).
pub fn ledger_outcome_status(
    projected_statuses: &BTreeMap<&str, MatchStatus>,
    outcome: &MatchOutcome,
) -> MatchStatus {
    if outcome.status.is_failure_in_no_new() {
        return outcome.status;
    }
    outcome
        .allow_id
        .as_deref()
        .and_then(|allow_id| projected_statuses.get(allow_id).copied())
        .unwrap_or(outcome.status)
}

pub fn ledger_project_outcomes(
    cfg: &AllowConfig,
    outcomes: &[MatchOutcome],
    today: SimpleDate,
) -> Vec<MatchOutcome> {
    let mut entries_by_id = BTreeMap::new();
    for entry in &cfg.allow {
        entries_by_id.entry(entry.id.as_str()).or_insert(entry);
    }
    outcomes
        .iter()
        .map(|outcome| {
            let mut projected = outcome.clone();
            if outcome.status == MatchStatus::Matched
                && let Some(status) = outcome
                    .allow_id
                    .as_deref()
                    .and_then(|allow_id| entries_by_id.get(allow_id).copied())
                    .and_then(|entry| entry_lifecycle_status(entry, today))
            {
                projected.status = status;
            }
            projected
        })
        .collect()
}

fn lifecycle_status(
    entry: &AllowEntry,
    outcomes: &[&MatchOutcome],
    today: SimpleDate,
) -> MatchStatus {
    if let Some(status) = entry_lifecycle_status(entry, today) {
        return status;
    }
    if has_outcome_status(outcomes, MatchStatus::Expired) {
        return MatchStatus::Expired;
    }
    if has_outcome_status(outcomes, MatchStatus::ReviewDue) {
        return MatchStatus::ReviewDue;
    }

    for status in [
        MatchStatus::New,
        MatchStatus::Ambiguous,
        MatchStatus::EvidenceMissing,
        MatchStatus::MissingRequiredField,
        MatchStatus::InvalidSelector,
        MatchStatus::Stale,
        MatchStatus::LocationDrift,
        MatchStatus::BaselineDebt,
    ] {
        if has_outcome_status(outcomes, status) {
            return status;
        }
    }
    if entry.classification == "baseline_debt" {
        return MatchStatus::BaselineDebt;
    }
    MatchStatus::Matched
}

fn entry_lifecycle_status(entry: &AllowEntry, today: SimpleDate) -> Option<MatchStatus> {
    if entry
        .lifecycle
        .expires
        .as_deref()
        .and_then(SimpleDate::parse)
        .is_some_and(|expires| expires.policy_expiry_has_passed(today))
    {
        return Some(MatchStatus::Expired);
    }
    if SimpleDate::is_due_date_str(entry.lifecycle.review_after.as_deref(), today) {
        return Some(MatchStatus::ReviewDue);
    }
    None
}

fn has_outcome_status(outcomes: &[&MatchOutcome], status: MatchStatus) -> bool {
    outcomes.iter().any(|outcome| outcome.status == status)
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{FindingKind, Lifecycle, Selector};
    use std::path::PathBuf;

    #[test]
    fn projection_preserves_matched_count_and_occurrence_limit() {
        let entry = test_entry(Some(3));
        let matched = test_outcome(MatchStatus::Matched);
        let stale = test_outcome(MatchStatus::Stale);

        let state = ledger_read_state(
            &entry,
            &[&matched, &stale],
            SimpleDate {
                year: 2026,
                month: 7,
                day: 14,
            },
        );

        assert_eq!(state.status, MatchStatus::Stale);
        assert_eq!(state.matched_count, 1);
        assert_eq!(state.occurrence_limit, Some(3));
    }

    #[test]
    fn projected_outcomes_replace_lifecycle_status_without_match_detail_loss() {
        let mut entry = test_entry(None);
        entry.lifecycle.expires = Some("2020-01-01".to_string());
        let mut cfg = AllowConfig::empty();
        cfg.allow.push(entry);
        let mut matched = test_outcome(MatchStatus::Matched);
        matched.candidate_ids = vec!["candidate-a".to_string()];
        matched.finding_index = Some(4);
        matched.message = "matched detail".to_string();

        let mut new = test_outcome(MatchStatus::New);
        new.message = "new occurrence".to_string();
        let projected = ledger_project_outcomes(
            &cfg,
            &[matched.clone(), new],
            SimpleDate {
                year: 2026,
                month: 7,
                day: 14,
            },
        );

        assert_eq!(projected[0].status, MatchStatus::Expired);
        assert_eq!(projected[0].candidate_ids, matched.candidate_ids);
        assert_eq!(projected[0].finding_index, matched.finding_index);
        assert_eq!(projected[0].message, matched.message);
        assert_eq!(projected[1].status, MatchStatus::New);
    }

    #[test]
    fn projected_outcomes_preserve_dedicated_baseline_debt_accounting() {
        let mut entry = test_entry(None);
        entry.classification = "baseline_debt".to_string();
        let mut cfg = AllowConfig::empty();
        cfg.allow.push(entry);
        let matched = test_outcome(MatchStatus::Matched);
        let today = SimpleDate {
            year: 2026,
            month: 7,
            day: 14,
        };

        assert_eq!(
            ledger_read_statuses(&cfg, std::slice::from_ref(&matched), today).get("allow-test"),
            Some(&MatchStatus::BaselineDebt)
        );

        let projected = ledger_project_outcomes(&cfg, &[matched], today);

        assert_eq!(projected[0].status, MatchStatus::Matched);
    }

    #[test]
    fn outcome_status_keeps_reraised_new_over_entry_expiry_annotation() {
        // #4238 gate-masking repair: under one expired entry, the re-raised
        // broad-expiry finding row must read as `New` (no-new blocking) while
        // the sibling plain `Matched` row keeps the date-first `Expired`
        // annotation surface.
        let mut entry = test_entry(None);
        entry.lifecycle.expires = Some("2020-01-01".to_string());
        let mut cfg = AllowConfig::empty();
        cfg.allow.push(entry);
        let mut reraised = test_outcome(MatchStatus::New);
        reraised.finding_index = Some(0);
        let matched = test_outcome(MatchStatus::Matched);
        let today = SimpleDate {
            year: 2026,
            month: 7,
            day: 14,
        };

        let statuses = ledger_read_statuses(&cfg, &[reraised.clone(), matched], today);
        assert_eq!(
            statuses.get("allow-test"),
            Some(&MatchStatus::Expired),
            "the entry-level projection stays date-first"
        );
        assert_eq!(
            ledger_outcome_status(&statuses, &reraised),
            MatchStatus::New,
            "the re-raised finding row must not be downgraded"
        );

        let plain_matched = test_outcome(MatchStatus::Matched);
        let matched_statuses =
            ledger_read_statuses(&cfg, std::slice::from_ref(&plain_matched), today);
        assert_eq!(
            ledger_outcome_status(&matched_statuses, &plain_matched),
            MatchStatus::Expired,
            "a plain Matched row keeps the entry expiry annotation"
        );
    }

    #[test]
    fn outcome_status_keeps_hard_requirement_failures_over_lifecycle_annotation() {
        // #4644: a date-first entry projection must not swallow finding-level
        // mandatory requirement failures. Under one overdue entry the
        // `EvidenceMissing` (required SAFETY comment or entry evidence) and
        // `InvalidSelector` (required or mismatched `policy:<allow-id>`)
        // rows keep their blocking statuses through the gate projection,
        // equal-strength `Ambiguous` keeps its own status, and a plain
        // `Matched` row keeps the entry expiry annotation for visibility.
        let mut entry = test_entry(None);
        entry.lifecycle.expires = Some("2020-01-01".to_string());
        entry.lifecycle.review_after = Some("2019-06-01".to_string());
        let mut cfg = AllowConfig::empty();
        cfg.allow.push(entry);
        let hard_failures = [
            MatchStatus::EvidenceMissing,
            MatchStatus::InvalidSelector,
            MatchStatus::MissingRequiredField,
            MatchStatus::Ambiguous,
        ];
        let mut outcomes = hard_failures
            .iter()
            .enumerate()
            .map(|(index, status)| {
                let mut outcome = test_outcome(*status);
                outcome.finding_index = Some(index);
                outcome
            })
            .collect::<Vec<_>>();
        outcomes.push(test_outcome(MatchStatus::Matched));
        let today = SimpleDate {
            year: 2026,
            month: 7,
            day: 14,
        };

        let statuses = ledger_read_statuses(&cfg, &outcomes, today);
        assert_eq!(
            statuses.get("allow-test"),
            Some(&MatchStatus::Expired),
            "the entry-level projection stays date-first"
        );
        for outcome in &outcomes {
            let projected = ledger_outcome_status(&statuses, outcome);
            if outcome.status == MatchStatus::Matched {
                assert_eq!(
                    projected,
                    MatchStatus::Expired,
                    "a plain Matched row keeps the entry expiry annotation"
                );
            } else {
                assert_eq!(
                    projected,
                    outcome.status,
                    "the {} row must not be downgraded to the date-first annotation",
                    outcome.status.as_str()
                );
            }
        }

        let projected = ledger_project_outcomes(&cfg, &outcomes, today);
        for (input, projected_row) in outcomes.iter().zip(projected.iter()) {
            if input.status == MatchStatus::Matched {
                assert_eq!(
                    projected_row.status,
                    MatchStatus::Expired,
                    "the plain Matched row keeps the entry expiry annotation"
                );
            } else {
                assert_eq!(
                    projected_row.status,
                    input.status,
                    "the {} row must survive the report projection unchanged",
                    input.status.as_str()
                );
            }
        }
    }

    fn test_entry(occurrence_limit: Option<u32>) -> AllowEntry {
        AllowEntry {
            id: "allow-test".to_string(),
            kind: FindingKind::Panic,
            family: Some("unwrap".to_string()),
            path: Some(PathBuf::from("src/lib.rs")),
            glob: None,
            owner: "owner".to_string(),
            classification: "reviewed_exception".to_string(),
            reason: "test policy entry".to_string(),
            evidence: Vec::new(),
            links: Vec::new(),
            occurrence_limit,
            lifecycle: Lifecycle::empty(),
            selector: Selector::default(),
            last_seen: None,
        }
    }

    fn test_outcome(status: MatchStatus) -> MatchOutcome {
        MatchOutcome {
            status,
            allow_id: Some("allow-test".to_string()),
            candidate_ids: Vec::new(),
            finding_index: None,
            message: "test outcome".to_string(),
            score: 100,
        }
    }
}
