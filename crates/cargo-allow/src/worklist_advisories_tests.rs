use std::path::PathBuf;

use allow_core::{AllowConfig, FindingKind, MatchStatus};

use super::work_items_from_policy_advisories;
use crate::worklist::test_support::{test_entry, test_finding, test_outcome};

#[test]
fn matched_outcome_for_entry_call_presence_observer() {
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-matched", FindingKind::NonRustFile);
    entry.path = Some(PathBuf::from("docs/policy.md"));
    entry.selector.glob = Some("docs/policy.md".to_string());
    cfg.allow.push(entry);
    let outcomes = vec![test_outcome(
        MatchStatus::Matched,
        Some("allow-matched"),
        Some(0),
        "matched",
    )];

    let items = work_items_from_policy_advisories(&cfg, &[], &outcomes, 0);

    match items.as_slice() {
        [item] => {
            assert_eq!(item.allow_id.as_deref(), Some("allow-matched"));
            assert_eq!(item.kind, "missing_evidence");
            assert_eq!(item.status, MatchStatus::EvidenceMissing);
        }
        other => assert_eq!(other.len(), 1),
    }

    let stale_outcomes = vec![test_outcome(
        MatchStatus::Stale,
        Some("allow-matched"),
        None,
        "stale",
    )];
    let skipped = work_items_from_policy_advisories(&cfg, &[], &stale_outcomes, 0);
    assert_eq!(skipped, Vec::new());
}

#[test]
fn source_package_name_call_presence_observer() {
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-baseline", FindingKind::Panic);
    entry.classification = "baseline_debt".to_string();
    cfg.allow.push(entry);
    let mut finding = test_finding(
        FindingKind::Panic,
        None,
        "crates/parser/src/lib.rs",
        "method_call",
    );
    finding.identity.crate_name = Some("parser".to_string());
    let outcomes = vec![test_outcome(
        MatchStatus::Matched,
        Some("allow-baseline"),
        Some(0),
        "matched",
    )];

    let items = work_items_from_policy_advisories(&cfg, &[finding], &outcomes, 0);

    match items.as_slice() {
        [item] => {
            assert_eq!(item.kind, "baseline_debt");
            assert_eq!(item.source_package.as_deref(), Some("parser"));
        }
        other => assert_eq!(other.len(), 1),
    }
}
