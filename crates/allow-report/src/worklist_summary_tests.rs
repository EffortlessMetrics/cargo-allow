use std::collections::BTreeMap;

use crate::WorklistItem;
use crate::worklist_summary::{worklist_kind_counts, worklist_risk_count};

fn item(kind: &'static str, risk: &'static str, difficulty: &'static str) -> WorklistItem<'static> {
    WorklistItem {
        id: "work-0001",
        kind,
        exception_kind: None,
        family: None,
        owner: None,
        classification: None,
        reason: None,
        created: None,
        review_after: None,
        expires: None,
        evidence_count: None,
        selector_precision: None,
        risk,
        difficulty,
        status: "new",
        allow_id: None,
        candidate_ids: &[],
        finding_index: None,
        path: None,
        evidence_reference: None,
        source_package: None,
        message: "fixture",
        suggested_actions: &[],
        proof_commands: &[],
        ledger_id: None,
        ledger_path: None,
        lane: None,
        mode: None,
        role: None,
    }
}

#[test]
fn worklist_risk_count_call_presence_observer() {
    let items = [
        item("stale_allow", "high", "small"),
        item("new_unreceipted_finding", "medium", "small"),
        item("review_due", "high", "large"),
    ];

    assert_eq!(worklist_risk_count(&items, "high"), 2);
    assert_eq!(worklist_risk_count(&items, "medium"), 1);
    assert_eq!(worklist_risk_count(&items, "low"), 0);
}

#[test]
fn worklist_kind_counts_call_presence_observer() {
    let items = [
        item("stale_allow", "high", "small"),
        item("stale_allow", "low", "small"),
        item("review_due", "medium", "large"),
    ];

    assert_eq!(
        worklist_kind_counts(&items),
        BTreeMap::from([("review_due", 1), ("stale_allow", 2)])
    );
}

#[test]
fn worklist_kind_counts_return_value_discriminator() {
    let items = [item("ambiguous_selector", "medium", "small")];

    assert_eq!(
        worklist_kind_counts(&items),
        BTreeMap::from([("ambiguous_selector", 1)])
    );
}
