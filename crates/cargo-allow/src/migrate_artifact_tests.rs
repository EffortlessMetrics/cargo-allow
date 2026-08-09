use super::*;
use crate::artifact_contract_support::{assert_inventory_contract, parse_json_artifact};
use allow_core::{AllowEntry, Lifecycle, Selector};
use serde_json::Value;

#[test]
fn render_migrate_summary_json_records_policy_migration_context() {
    let mut cfg = AllowConfig::empty();
    let mut baseline = test_entry("allow-baseline", FindingKind::Panic);
    baseline.classification = "baseline_debt".to_string();
    let mut unsafe_entry = test_entry("allow-unsafe", FindingKind::Unsafe);
    unsafe_entry.evidence = vec![
        "doc:docs/missing-unsafe-evidence.md".to_string(),
        "TODO: add unsafe-review or boundary-test evidence".to_string(),
    ];
    unsafe_entry.links = vec!["legacy-policy:allow-unsafe".to_string()];
    let mut lint_entry = test_entry("allow-lint", FindingKind::LintException);
    lint_entry.evidence = vec!["TODO: replace with typed lint evidence".to_string()];
    lint_entry.links = vec!["legacy-policy:allow-lint".to_string()];
    cfg.allow.push(baseline);
    cfg.allow.push(unsafe_entry);
    cfg.allow.push(lint_entry);
    let context = MigrateContext {
        inventory_source: "git_tracked".to_string(),
        source_tree_root: Some("H:/Code/Rust/cargo-allow".to_string()),
        inventory_files: Some(53),
        inventory_completeness: Some("complete".to_string()),
        repository_identity: Some("test".to_string()),
        input_kind: "repo_policy".to_string(),
        input_path: "policy".to_string(),
        legacy_source_files: Vec::new(),
        legacy_compat_kinds: Vec::new(),
        baseline_debt_projection: allow_report::MigrateBaselineDebtProjection::default_projection(),
    };

    let json = render_migrate_summary_json(&cfg, &context, Path::new("policy/allow.toml"), true);
    let value = parse_json_artifact("migrate", &json, allow_report::MIGRATE_SCHEMA_ID, "migrate");

    assert_inventory_contract(
        "migrate",
        &value,
        "git_tracked",
        Some("H:/Code/Rust/cargo-allow"),
        Some(53),
    );
    assert_eq!(
        value.pointer("/inventory/scanner").and_then(Value::as_str),
        Some("policy_migration"),
        "migrate scanner"
    );
    assert_eq!(
        value.pointer("/input/kind").and_then(Value::as_str),
        Some("repo_policy"),
        "migrate input kind"
    );
    assert_eq!(
        value.pointer("/output/path").and_then(Value::as_str),
        Some("policy/allow.toml"),
        "migrate output path"
    );
    assert_eq!(
        value.pointer("/output/force").and_then(Value::as_bool),
        Some(true),
        "migrate force"
    );
    assert_eq!(
        value
            .pointer("/summary/allow_entries")
            .and_then(Value::as_u64),
        Some(3),
        "migrate allow entries"
    );
    assert_eq!(
        value
            .pointer("/summary/baseline_debt")
            .and_then(Value::as_u64),
        Some(1),
        "migrate baseline debt"
    );
    assert_eq!(
        value
            .pointer("/summary/unsafe_entries")
            .and_then(Value::as_u64),
        Some(1),
        "migrate unsafe entries"
    );
    assert_eq!(
        value
            .pointer("/summary/lint_exception_entries")
            .and_then(Value::as_u64),
        Some(1),
        "migrate lint exception entries"
    );
    assert_eq!(
        value
            .pointer("/summary/entries_with_evidence")
            .and_then(Value::as_u64),
        Some(2),
        "migrate evidence entries"
    );
    assert_eq!(
        value
            .pointer("/summary/evidence_entries")
            .and_then(Value::as_u64),
        Some(3),
        "migrate evidence reference entries"
    );
    assert_eq!(
        value
            .pointer("/summary/entries_with_links")
            .and_then(Value::as_u64),
        Some(2),
        "migrate link-bearing entries"
    );
    assert_eq!(
        value
            .pointer("/summary/link_entries")
            .and_then(Value::as_u64),
        Some(2),
        "migrate traceability link entries"
    );
    assert_eq!(
        value
            .pointer("/summary/broken_evidence_links")
            .and_then(Value::as_u64),
        Some(1),
        "migrate broken evidence links"
    );
    assert_eq!(
        value
            .pointer("/summary/unsafe_broken_evidence_links")
            .and_then(Value::as_u64),
        Some(1),
        "migrate unsafe broken evidence links"
    );
    assert_eq!(
        value
            .pointer("/summary/weak_evidence_references")
            .and_then(Value::as_u64),
        Some(2),
        "migrate weak evidence references"
    );
    assert_eq!(
        value
            .pointer("/summary/unsafe_weak_evidence_references")
            .and_then(Value::as_u64),
        Some(1),
        "migrate unsafe weak evidence references"
    );
    assert!(
        value.pointer("/closeout/preserved/allow_entries").is_some(),
        "migrate JSON should emit closeout preserved counts"
    );
    let follow_up_queues = value
        .pointer("/follow_up_queues")
        .and_then(Value::as_array)
        .unwrap_or_else(|| {
            std::panic::panic_any("migrate JSON should route baseline-debt follow-up queues")
        });
    let [baseline_debt] = follow_up_queues.as_slice() else {
        std::panic::panic_any(format!(
            "expected one migrate follow-up queue, got {}",
            follow_up_queues.len()
        ));
    };
    assert_eq!(
        baseline_debt.get("signal").and_then(Value::as_str),
        Some("baseline_debt"),
        "migrate baseline-debt queue signal"
    );
    assert_eq!(
        baseline_debt.get("route_kind").and_then(Value::as_str),
        Some("worklist_item_kind"),
        "migrate baseline-debt queue route kind"
    );
    assert_eq!(
        baseline_debt.get("item_kind").and_then(Value::as_str),
        Some("baseline_debt"),
        "migrate baseline-debt queue item kind"
    );
    assert_eq!(
        baseline_debt.get("count").and_then(Value::as_u64),
        Some(1),
        "migrate baseline-debt queue count"
    );
    assert_eq!(
        baseline_debt.get("command").and_then(Value::as_str),
        Some("cargo-allow worklist --item-kind baseline_debt --format json"),
        "migrate baseline-debt queue command"
    );
    let queues = value
        .pointer("/evidence_repair_queues")
        .and_then(Value::as_array)
        .unwrap_or_else(|| {
            std::panic::panic_any("migrate JSON should route evidence repair queues")
        });
    let [broken, weak] = queues.as_slice() else {
        std::panic::panic_any(format!(
            "expected two migrate evidence repair queues, got {}",
            queues.len()
        ));
    };
    assert_eq!(
        broken.get("item_kind").and_then(Value::as_str),
        Some("broken_evidence_link"),
        "migrate broken evidence queue kind"
    );
    assert_eq!(
        broken.get("count").and_then(Value::as_u64),
        Some(1),
        "migrate broken evidence queue count"
    );
    assert_eq!(
        broken.get("unsafe_count").and_then(Value::as_u64),
        Some(1),
        "migrate broken evidence unsafe count"
    );
    assert_eq!(
        broken.get("command").and_then(Value::as_str),
        Some("cargo-allow worklist --item-kind broken_evidence_link --format json"),
        "migrate broken evidence queue command"
    );
    assert_eq!(
        broken.get("unsafe_command").and_then(Value::as_str),
        Some("cargo-allow worklist --item-kind broken_evidence_link --kind unsafe --format json"),
        "migrate broken evidence unsafe queue command"
    );
    assert_eq!(
        weak.get("item_kind").and_then(Value::as_str),
        Some("weak_evidence_reference"),
        "migrate weak evidence queue kind"
    );
    assert_eq!(
        weak.get("count").and_then(Value::as_u64),
        Some(2),
        "migrate weak evidence queue count"
    );
    assert_eq!(
        weak.get("unsafe_count").and_then(Value::as_u64),
        Some(1),
        "migrate weak evidence unsafe count"
    );
    assert_eq!(
        weak.get("command").and_then(Value::as_str),
        Some("cargo-allow worklist --item-kind weak_evidence_reference --format json"),
        "migrate weak evidence queue command"
    );
    assert_eq!(
        weak.get("unsafe_command").and_then(Value::as_str),
        Some(
            "cargo-allow worklist --item-kind weak_evidence_reference --kind unsafe --format json"
        ),
        "migrate weak evidence unsafe queue command"
    );
}

fn test_entry(id: &str, kind: FindingKind) -> AllowEntry {
    AllowEntry {
        id: id.to_string(),
        kind,
        family: None,
        path: Some("src/lib.rs".into()),
        glob: None,
        owner: "team".to_string(),
        classification: "reviewed".to_string(),
        reason: "test".to_string(),
        evidence: Vec::new(),
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle::empty(),
        selector: Selector::default(),
        last_seen: None,
    }
}
