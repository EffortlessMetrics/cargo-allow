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
    entry.evidence = vec!["doc:docs/policy/scripts.md".to_string()];
    cfg.allow.push(entry);
    let outcomes = vec![MatchOutcome {
        status: MatchStatus::Matched,
        allow_id: Some("allow-scripts".to_string()),
        candidate_ids: Vec::new(),
        finding_index: Some(0),
        message: "matched".to_string(),
        score: 100,
    }];

    let items = work_items_from_policy_advisories(&cfg, &[], &outcomes, 1);
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
fn worklist_broad_scope_advisories_use_exception_risk() {
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-unsafe-glob", FindingKind::Unsafe);
    entry.path = None;
    entry.glob = Some("crates/runtime/**".to_string());
    entry.selector.glob = Some("crates/runtime/**".to_string());
    entry.family = Some("unsafe_block".to_string());
    entry.evidence = vec!["unsafe-review:docs/evidence/unsafe.json".to_string()];
    cfg.allow.push(entry);
    let outcomes = vec![MatchOutcome {
        status: MatchStatus::Matched,
        allow_id: Some("allow-unsafe-glob".to_string()),
        candidate_ids: Vec::new(),
        finding_index: Some(0),
        message: "matched".to_string(),
        score: 100,
    }];

    let items = work_items_from_policy_advisories(&cfg, &[], &outcomes, 1);

    let item = items
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected broad-scope work item"));
    assert_eq!(item.kind, "broad_scope");
    assert_eq!(item.exception_kind.as_deref(), Some("unsafe"));
    assert_eq!(item.family.as_deref(), Some("unsafe_block"));
    assert_eq!(item.risk, "high");
    assert_eq!(item.difficulty, "small");
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
        candidate_ids: Vec::new(),
        finding_index: Some(0),
        message: "matched".to_string(),
        score: 100,
    }];

    let items = work_items_from_policy_advisories(&cfg, &[finding], &outcomes, 1);
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
    entry.evidence = vec!["doc:docs/README.md".to_string()];
    cfg.allow.push(entry);
    let outcomes = vec![MatchOutcome {
        status: MatchStatus::Matched,
        allow_id: Some("allow-doc".to_string()),
        candidate_ids: Vec::new(),
        finding_index: Some(0),
        message: "matched".to_string(),
        score: 100,
    }];

    let items = work_items_from_policy_advisories(&cfg, &[], &outcomes, 1);

    assert!(items.is_empty());
}

#[test]
fn worklist_policy_advisories_report_missing_evidence_by_default() {
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
        candidate_ids: Vec::new(),
        finding_index: Some(0),
        message: "matched".to_string(),
        score: 100,
    }];

    let items = work_items_from_policy_advisories(&cfg, &[finding], &outcomes, 1);

    let item = items
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected missing evidence advisory"));
    assert_eq!(items.len(), 1);
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
fn worklist_policy_advisories_specialize_high_risk_policy_missing_evidence_actions() {
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-process", FindingKind::PolicyException);
    entry.path = Some(PathBuf::from(".github/workflows/ci.yml"));
    entry.family = Some("process_spawn".to_string());
    cfg.allow.push(entry);
    let finding = test_finding(
        FindingKind::PolicyException,
        Some("process_spawn"),
        ".github/workflows/ci.yml",
        "process_spawn",
    );
    let outcomes = vec![MatchOutcome {
        status: MatchStatus::Matched,
        allow_id: Some("allow-process".to_string()),
        candidate_ids: Vec::new(),
        finding_index: Some(0),
        message: "matched".to_string(),
        score: 100,
    }];

    let items = work_items_from_policy_advisories(&cfg, &[finding], &outcomes, 1);

    let item = items
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected policy missing evidence advisory"));
    assert_eq!(item.kind, "missing_evidence");
    assert_eq!(item.exception_kind.as_deref(), Some("policy_exception"));
    assert_eq!(item.family.as_deref(), Some("process_spawn"));
    assert_eq!(item.risk, "high");
    assert!(
        item.suggested_actions
            .iter()
            .any(|action| action.contains("typed evidence"))
    );
    assert!(
        item.suggested_actions
            .iter()
            .any(|action| action.contains("policy_exception.process_spawn"))
    );
    assert!(
        item.suggested_actions
            .iter()
            .any(|action| action.contains("removed or narrowed"))
    );
    assert!(
        item.proof_commands
            .iter()
            .any(|command| command == "cargo-allow check --kind process --mode no-new")
    );
}

#[test]
fn worklist_policy_advisories_report_unsafe_missing_evidence_when_requested() {
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-unsafe", FindingKind::Unsafe);
    entry.path = Some(PathBuf::from("crates/runtime/src/ffi.rs"));
    entry.family = Some("unsafe_block".to_string());
    cfg.allow.push(entry);
    let finding = test_finding(
        FindingKind::Unsafe,
        Some("unsafe_block"),
        "crates/runtime/src/ffi.rs",
        "unsafe_block",
    );
    let outcomes = vec![MatchOutcome {
        status: MatchStatus::Matched,
        allow_id: Some("allow-unsafe".to_string()),
        candidate_ids: Vec::new(),
        finding_index: Some(0),
        message: "matched".to_string(),
        score: 100,
    }];

    let items = work_items_from_policy_advisories(&cfg, &[finding], &outcomes, 1);

    let item = items
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected unsafe missing evidence advisory"));
    assert_eq!(items.len(), 1);
    assert_eq!(item.id, "work-unsafe-missing-evidence-0001");
    assert_eq!(item.kind, "unsafe_missing_evidence");
    assert_eq!(item.status, MatchStatus::EvidenceMissing);
    assert_eq!(item.risk, "high");
    assert_eq!(item.difficulty, "medium");
    assert_eq!(item.evidence_count, Some(0));
    assert_eq!(item.allow_id.as_deref(), Some("allow-unsafe"));
    assert_eq!(item.path.as_deref(), Some("crates/runtime/src/ffi.rs"));
    assert_eq!(item.exception_kind.as_deref(), Some("unsafe"));
    assert_eq!(item.family.as_deref(), Some("unsafe_block"));
    assert!(
        item.suggested_actions
            .iter()
            .any(|action| action.contains("unsafe-review"))
    );
    assert!(
        item.proof_commands
            .iter()
            .any(|command| command == "cargo-allow worklist --missing-evidence --format json")
    );
    assert!(
        item.proof_commands
            .iter()
            .any(|command| command == "cargo-allow check --kind unsafe --mode no-new")
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
        candidate_ids: Vec::new(),
        finding_index: None,
        message: "stale".to_string(),
        score: 0,
    }];

    let items = work_items_from_policy_advisories(&cfg, &[], &outcomes, 1);

    assert!(items.is_empty());
}
