use super::test_support::{test_entry, test_finding, test_outcome};
use super::*;
use std::path::PathBuf;

#[test]
fn worklist_items_report_occurrence_limit_overrun() {
    let mut cfg = AllowConfig::empty();
    cfg.allow
        .push(test_entry("allow-file", FindingKind::NonRustFile));
    let finding = test_finding(
        FindingKind::NonRustFile,
        None,
        "tracked.file",
        "tracked_file",
    );
    let outcomes = vec![test_outcome(
        MatchStatus::New,
        Some("allow-file"),
        Some(0),
        "allow-file occurrence_limit exceeded at tracked.file:1:1",
    )];

    let items = work_items_from_outcomes(&cfg, &[finding], &outcomes);

    let item = items
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one work item"));
    assert_eq!(item.kind, "occurrence_limit_exceeded");
    assert_eq!(item.exception_kind.as_deref(), Some("non_rust_file"));
    assert_eq!(item.risk, "medium");
    assert!(
        item.suggested_actions
            .iter()
            .any(|action| action.contains("baseline count"))
    );
}

#[test]
fn worklist_items_report_broad_scope_advisories() {
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-scripts", FindingKind::NonRustFile);
    entry.path = None;
    entry.glob = Some("scripts/**".to_string());
    entry.selector.glob = Some("scripts/**".to_string());
    entry.family = Some("shell_script".to_string());
    cfg.allow.push(entry);
    let outcomes = vec![MatchOutcome {
        status: MatchStatus::Matched,
        allow_id: Some("allow-scripts".to_string()),
        finding_index: Some(0),
        message: "matched".to_string(),
        score: 100,
    }];

    let items = work_items_from_policy_advisories(&cfg, &[], &outcomes, 1, false);
    let json = render_worklist_json_with_context(&items, WorklistContext::default());
    let human = render_worklist_human_with_context(&items, WorklistContext::default());

    let item = items
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one work item"));
    assert_eq!(item.kind, "broad_scope");
    assert_eq!(item.status, MatchStatus::Matched);
    assert_eq!(item.risk, "medium");
    assert_eq!(item.difficulty, "small");
    assert_eq!(item.allow_id.as_deref(), Some("allow-scripts"));
    assert_eq!(item.path.as_deref(), Some("scripts/**"));
    assert_eq!(item.exception_kind.as_deref(), Some("non_rust_file"));
    assert_eq!(item.family.as_deref(), Some("shell_script"));
    assert!(
        item.suggested_actions
            .iter()
            .any(|action| action.contains("narrower glob"))
    );
    assert!(
        item.proof_commands
            .iter()
            .any(|command| command == "cargo-allow worklist --broad-scope --format json")
    );
    assert!(
        item.proof_commands
            .iter()
            .any(|command| command == "cargo-allow worklist --allow-id allow-scripts --format json")
    );
    assert!(json.contains("\"kind\": \"broad_scope\""));
    assert!(json.contains("\"status\": \"matched\""));
    assert!(human.contains("proof: cargo-allow worklist --allow-id allow-scripts --format json"));
    assert!(human.contains("proof: cargo-allow worklist --broad-scope --format json"));
    assert!(human.contains("exception: non_rust_file.shell_script"));
}

#[test]
fn worklist_items_report_matched_baseline_debt_advisories() {
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-baseline", FindingKind::Panic);
    entry.classification = "baseline_debt".to_string();
    entry.family = Some("unwrap".to_string());
    cfg.allow.push(entry);
    let mut finding = test_finding(
        FindingKind::Panic,
        Some("unwrap"),
        "crates/parser/src/lib.rs",
        "method_call",
    );
    finding.identity.crate_name = Some("parser".to_string());
    let outcomes = vec![MatchOutcome {
        status: MatchStatus::Matched,
        allow_id: Some("allow-baseline".to_string()),
        finding_index: Some(0),
        message: "matched".to_string(),
        score: 100,
    }];

    let items = work_items_from_policy_advisories(&cfg, &[finding], &outcomes, 1, false);
    let json = render_worklist_json_with_context(&items, WorklistContext::default());
    let human = render_worklist_human_with_context(&items, WorklistContext::default());

    let item = items
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one work item"));
    assert_eq!(item.kind, "baseline_debt");
    assert_eq!(item.status, MatchStatus::BaselineDebt);
    assert_eq!(item.risk, "medium");
    assert_eq!(item.difficulty, "medium");
    assert_eq!(item.allow_id.as_deref(), Some("allow-baseline"));
    assert_eq!(item.finding_index, Some(0));
    assert_eq!(item.exception_kind.as_deref(), Some("panic"));
    assert_eq!(item.family.as_deref(), Some("unwrap"));
    assert_eq!(item.source_package.as_deref(), Some("parser"));
    assert!(item.message.contains("still needs human review"));
    assert!(
        item.suggested_actions
            .iter()
            .any(|action| action.contains("reviewed allow entry"))
    );
    assert!(
        item.proof_commands
            .iter()
            .any(|command| command == "cargo-allow worklist --baseline-debt --format json")
    );
    assert!(
        item.proof_commands.iter().any(
            |command| command == "cargo-allow worklist --allow-id allow-baseline --format json"
        )
    );
    assert!(json.contains("\"kind\": \"baseline_debt\""));
    assert!(json.contains("\"status\": \"baseline_debt\""));
    assert!(human.contains("proof: cargo-allow worklist --allow-id allow-baseline --format json"));
    assert!(human.contains("proof: cargo-allow worklist --baseline-debt --format json"));
    assert!(human.contains("source package: parser"));
    assert!(human.contains("exception: panic.unwrap"));
}

#[test]
fn worklist_policy_advisories_ignore_exact_selector_globs() {
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-doc", FindingKind::NonRustFile);
    entry.selector.glob = Some("docs/README.md".to_string());
    cfg.allow.push(entry);
    let outcomes = vec![MatchOutcome {
        status: MatchStatus::Matched,
        allow_id: Some("allow-doc".to_string()),
        finding_index: Some(0),
        message: "matched".to_string(),
        score: 100,
    }];

    let items = work_items_from_policy_advisories(&cfg, &[], &outcomes, 1, false);

    assert!(items.is_empty());
}

#[test]
fn worklist_policy_advisories_report_missing_evidence_when_requested() {
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-doc", FindingKind::NonRustFile);
    entry.path = Some(PathBuf::from("docs/policy.md"));
    entry.selector.glob = Some("docs/policy.md".to_string());
    entry.family = Some("documentation".to_string());
    cfg.allow.push(entry);
    let finding = test_finding(
        FindingKind::NonRustFile,
        Some("documentation"),
        "docs/policy.md",
        "tracked_file",
    );
    let outcomes = vec![MatchOutcome {
        status: MatchStatus::Matched,
        allow_id: Some("allow-doc".to_string()),
        finding_index: Some(0),
        message: "matched".to_string(),
        score: 100,
    }];

    let default_items = work_items_from_policy_advisories(
        &cfg,
        std::slice::from_ref(&finding),
        &outcomes,
        1,
        false,
    );
    let requested_items = work_items_from_policy_advisories(&cfg, &[finding], &outcomes, 1, true);

    assert!(default_items.is_empty());
    let item = requested_items
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected missing evidence advisory"));
    assert_eq!(requested_items.len(), 1);
    assert_eq!(item.kind, "missing_evidence");
    assert_eq!(item.status, MatchStatus::EvidenceMissing);
    assert_eq!(item.evidence_count, Some(0));
    assert_eq!(item.allow_id.as_deref(), Some("allow-doc"));
    assert_eq!(item.path.as_deref(), Some("docs/policy.md"));
    assert_eq!(item.exception_kind.as_deref(), Some("non_rust_file"));
    assert_eq!(item.family.as_deref(), Some("documentation"));
    assert!(item.message.contains("has no evidence references"));
    assert!(
        item.proof_commands
            .iter()
            .any(|command| command == "cargo-allow worklist --allow-id allow-doc --format json")
    );
}

#[test]
fn worklist_policy_advisories_ignore_unmatched_broad_scopes() {
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-scripts", FindingKind::NonRustFile);
    entry.glob = Some("scripts/**".to_string());
    cfg.allow.push(entry);
    let outcomes = vec![MatchOutcome {
        status: MatchStatus::Stale,
        allow_id: Some("allow-scripts".to_string()),
        finding_index: None,
        message: "stale".to_string(),
        score: 0,
    }];

    let items = work_items_from_policy_advisories(&cfg, &[], &outcomes, 1, false);

    assert!(items.is_empty());
}
