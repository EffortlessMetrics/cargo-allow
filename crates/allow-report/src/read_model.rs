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

fn lifecycle_status(
    entry: &AllowEntry,
    outcomes: &[&MatchOutcome],
    today: SimpleDate,
) -> MatchStatus {
    if SimpleDate::has_passed_date_str(entry.lifecycle.expires.as_deref(), today)
        || has_outcome_status(outcomes, MatchStatus::Expired)
    {
        return MatchStatus::Expired;
    }
    if SimpleDate::is_due_date_str(entry.lifecycle.review_after.as_deref(), today)
        || has_outcome_status(outcomes, MatchStatus::ReviewDue)
    {
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
