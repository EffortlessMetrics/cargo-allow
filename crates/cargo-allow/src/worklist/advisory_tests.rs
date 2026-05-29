use super::test_support::{test_entry, test_finding, test_outcome};
use super::*;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

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
    assert!(json.contains("\"kind\": \"broad_scope\""));
    assert!(json.contains("\"status\": \"matched\""));
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
    assert!(json.contains("\"kind\": \"baseline_debt\""));
    assert!(json.contains("\"status\": \"baseline_debt\""));
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

    let items = work_items_from_policy_advisories(&cfg, &[], &outcomes, 1);

    assert!(items.is_empty());
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

    let items = work_items_from_policy_advisories(&cfg, &[], &outcomes, 1);

    assert!(items.is_empty());
}

#[test]
fn worklist_items_report_broken_evidence_links() {
    let root = migrate_fixture_dir();
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-unsafe", FindingKind::Unsafe);
    entry.evidence = vec!["doc:docs/missing.md".to_string()];
    cfg.allow.push(entry);

    let items = work_items_from_evidence_diagnostics(&root, &cfg, 1);
    let json = render_worklist_json_with_context(&items, WorklistContext::default());

    let item = items
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one work item"));
    assert_eq!(item.kind, "broken_evidence_link");
    assert_eq!(item.exception_kind.as_deref(), Some("unsafe"));
    assert_eq!(item.risk, "high");
    assert_eq!(item.difficulty, "small");
    assert_eq!(item.status, MatchStatus::EvidenceMissing);
    assert_eq!(item.allow_id.as_deref(), Some("allow-unsafe"));
    assert_eq!(item.path.as_deref(), Some("docs/missing.md"));
    assert!(item.message.contains("local evidence file is missing"));
    assert!(json.contains("\"kind\": \"broken_evidence_link\""));
    assert!(json.contains("\"exception_kind\": \"unsafe\""));
    assert!(json.contains("\"cargo-allow explain allow-unsafe\""));
    assert!(json.contains("\"cargo-allow check --kind unsafe --mode no-new\""));
    assert!(json.contains("\"cargo-allow worklist --kind unsafe --format json\""));
    fs::remove_dir_all(root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

static NEXT_WORKLIST_FIXTURE: AtomicUsize = AtomicUsize::new(0);

fn migrate_fixture_dir() -> PathBuf {
    let id = NEXT_WORKLIST_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "cargo-allow-cli-worklist-{}-{stamp}-{id}",
        std::process::id()
    ));
    fs::create_dir_all(&dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture dir: {err}")));
    dir
}
