use super::test_support::test_entry;
use super::*;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn worklist_items_report_broken_evidence_links() {
    let root = migrate_fixture_dir();
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-unsafe", FindingKind::Unsafe);
    entry.evidence = vec!["doc:docs/missing.md".to_string()];
    cfg.allow.push(entry);

    let items = work_items_from_evidence_diagnostics(&root, &cfg, 1);
    let json = render_worklist_json_with_context(&items, WorklistContext::default());
    let human = render_worklist_human_with_context(&items, WorklistContext::default());

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
    let reference = item
        .evidence_reference
        .as_ref()
        .unwrap_or_else(|| std::panic::panic_any("broken evidence item should carry reference"));
    assert_eq!(reference.raw, "doc:docs/missing.md");
    assert_eq!(reference.prefix.as_deref(), Some("doc"));
    assert_eq!(reference.target.as_deref(), Some("docs/missing.md"));
    assert_eq!(reference.status, "local_file_missing");
    assert_eq!(reference.category, "missing");
    assert!(reference.message.contains("local evidence file is missing"));
    assert!(item.message.contains("local evidence file is missing"));
    assert!(json.contains("\"kind\": \"broken_evidence_link\""));
    assert!(json.contains("\"evidence_reference\""));
    assert!(json.contains("\"raw\": \"doc:docs/missing.md\""));
    assert!(json.contains("\"category\": \"missing\""));
    assert!(json.contains("\"exception_kind\": \"unsafe\""));
    assert!(json.contains("\"cargo-allow explain allow-unsafe\""));
    assert!(json.contains("\"cargo-allow worklist --allow-id allow-unsafe --format json\""));
    assert!(json.contains("\"cargo-allow list --broken-evidence --format json\""));
    assert!(json.contains("\"cargo-allow check --kind unsafe --mode no-new\""));
    assert!(json.contains("\"cargo-allow worklist --kind unsafe --format json\""));
    assert!(human.contains("proof: cargo-allow list --broken-evidence --format json"));
    fs::remove_dir_all(root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn worklist_items_report_broken_local_traceability_links() {
    let root = migrate_fixture_dir();
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-linked-doc", FindingKind::NonRustFile);
    entry.links = vec!["doc:docs/missing-link.md".to_string()];
    cfg.allow.push(entry);

    let items = work_items_from_evidence_diagnostics(&root, &cfg, 1);

    let item = items
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one work item"));
    assert_eq!(item.kind, "broken_evidence_link");
    assert_eq!(item.exception_kind.as_deref(), Some("non_rust_file"));
    assert_eq!(item.risk, "medium");
    assert_eq!(item.difficulty, "small");
    assert_eq!(item.status, MatchStatus::EvidenceMissing);
    assert_eq!(item.allow_id.as_deref(), Some("allow-linked-doc"));
    assert_eq!(item.path.as_deref(), Some("docs/missing-link.md"));
    assert!(item.message.contains("allow-linked-doc link"));
    assert!(item.message.contains("local link file is missing"));
    let reference = item
        .evidence_reference
        .as_ref()
        .unwrap_or_else(|| std::panic::panic_any("broken link item should carry reference"));
    assert_eq!(reference.raw, "doc:docs/missing-link.md");
    assert_eq!(reference.status, "local_file_missing");
    assert_eq!(reference.category, "missing");
    assert!(reference.message.contains("local link file is missing"));
    assert!(
        item.suggested_actions
            .iter()
            .any(|action| action.contains("local traceability file"))
    );
    assert!(
        item.proof_commands
            .iter()
            .any(|command| command == "cargo-allow explain allow-linked-doc")
    );
    fs::remove_dir_all(root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn worklist_items_rank_broken_policy_exception_evidence_by_exception_risk() {
    let root = migrate_fixture_dir();
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-network", FindingKind::PolicyException);
    entry.family = Some("network_destination".to_string());
    entry.evidence = vec!["doc:docs/missing-network-policy.md".to_string()];
    cfg.allow.push(entry);

    let items = work_items_from_evidence_diagnostics(&root, &cfg, 1);

    let item = items
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one work item"));
    assert_eq!(item.kind, "broken_evidence_link");
    assert_eq!(item.exception_kind.as_deref(), Some("policy_exception"));
    assert_eq!(item.family.as_deref(), Some("network_destination"));
    assert_eq!(item.risk, "high");
    assert_eq!(item.difficulty, "small");
    assert_eq!(item.status, MatchStatus::EvidenceMissing);
    assert_eq!(item.allow_id.as_deref(), Some("allow-network"));
    assert_eq!(item.path.as_deref(), Some("docs/missing-network-policy.md"));
    fs::remove_dir_all(root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn worklist_items_report_weak_traceability_links() {
    let root = migrate_fixture_dir();
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-weak-link", FindingKind::Panic);
    entry.links = vec!["spreadsheet:manual-review".to_string()];
    cfg.allow.push(entry);

    let items = work_items_from_evidence_diagnostics(&root, &cfg, 1);

    let item = items
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one work item"));
    assert_eq!(item.kind, "weak_evidence_reference");
    assert_eq!(item.exception_kind.as_deref(), Some("panic"));
    assert_eq!(item.risk, "medium");
    assert_eq!(item.allow_id.as_deref(), Some("allow-weak-link"));
    assert!(item.message.contains("allow-weak-link link"));
    assert!(item.message.contains("unrecognized link prefix"));
    let reference = item
        .evidence_reference
        .as_ref()
        .unwrap_or_else(|| std::panic::panic_any("weak link item should carry reference"));
    assert_eq!(reference.raw, "spreadsheet:manual-review");
    assert_eq!(reference.category, "unknown_prefix");
    assert!(reference.message.contains("unrecognized link prefix"));
    assert!(
        item.suggested_actions
            .iter()
            .any(|action| action.contains("typed traceability reference"))
    );
    fs::remove_dir_all(root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn worklist_items_specialize_high_risk_policy_weak_link_actions() {
    let root = migrate_fixture_dir();
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-network-weak-link", FindingKind::PolicyException);
    entry.family = Some("network_destination".to_string());
    entry.links = vec!["spreadsheet:manual-review".to_string()];
    cfg.allow.push(entry);

    let items = work_items_from_evidence_diagnostics(&root, &cfg, 1);

    let item = items
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one weak link work item"));
    assert_eq!(item.kind, "weak_evidence_reference");
    assert_eq!(item.exception_kind.as_deref(), Some("policy_exception"));
    assert_eq!(item.family.as_deref(), Some("network_destination"));
    assert_eq!(item.risk, "high");
    assert_eq!(item.allow_id.as_deref(), Some("allow-network-weak-link"));
    assert!(
        item.suggested_actions
            .iter()
            .any(|action| action.contains("policy_exception.network_destination"))
    );
    assert!(
        item.suggested_actions
            .iter()
            .any(|action| action.contains("removed or narrowed"))
    );
    fs::remove_dir_all(root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn worklist_items_report_local_evidence_outside_source_tree_inventory() {
    let root = migrate_fixture_dir();
    fs::create_dir_all(root.join("docs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create docs dir: {err}")));
    fs::write(root.join("docs/untracked.md"), "evidence")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write evidence: {err}")));
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-unsafe", FindingKind::Unsafe);
    entry.evidence = vec!["doc:docs/untracked.md".to_string()];
    cfg.allow.push(entry);
    let source_tree_files = BTreeSet::new();

    let items = work_items_from_evidence_diagnostics_with_source_tree_files(
        &root,
        &cfg,
        1,
        Some(&source_tree_files),
    );
    let human = render_worklist_human_with_context(&items, WorklistContext::default());

    let item = items
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one work item"));
    assert_eq!(item.kind, "broken_evidence_link");
    assert_eq!(item.path.as_deref(), Some("docs/untracked.md"));
    let reference = item
        .evidence_reference
        .as_ref()
        .unwrap_or_else(|| std::panic::panic_any("broken evidence item should carry reference"));
    assert_eq!(reference.status, "local_file_missing");
    assert_eq!(reference.category, "missing");
    assert!(reference.message.contains("default source-tree inventory"));
    assert!(
        item.suggested_actions
            .iter()
            .any(|action| action.contains("commit the referenced evidence file")),
        "worklist should explain how to make evidence part of repository policy"
    );
    assert!(
        item.suggested_actions
            .iter()
            .any(|action| action.contains("--include-untracked")),
        "worklist should explain the local receipt review opt-in"
    );
    assert!(
        item.proof_commands
            .iter()
            .any(|command| command == "cargo-allow explain allow-unsafe --include-untracked")
    );
    assert!(
        item.proof_commands
            .iter()
            .any(|command| command == "cargo-allow check --include-untracked --mode no-new")
    );
    assert!(human.contains("proof: cargo-allow explain allow-unsafe --include-untracked"));
    assert!(human.contains("proof: cargo-allow check --include-untracked --mode no-new"));
    fs::remove_dir_all(root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn worklist_items_report_invalid_local_evidence_paths() {
    let root = migrate_fixture_dir();
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-doc", FindingKind::NonRustFile);
    entry.evidence = vec!["doc:../outside.md".to_string()];
    cfg.allow.push(entry);

    let items = work_items_from_evidence_diagnostics(&root, &cfg, 1);

    let item = items
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one work item"));
    assert_eq!(item.kind, "broken_evidence_link");
    assert_eq!(item.exception_kind.as_deref(), Some("non_rust_file"));
    assert_eq!(item.risk, "medium");
    assert_eq!(item.status, MatchStatus::EvidenceMissing);
    assert_eq!(item.allow_id.as_deref(), Some("allow-doc"));
    assert_eq!(item.path.as_deref(), Some("../outside.md"));
    assert!(item.message.contains("parent directory"));
    assert!(
        item.suggested_actions
            .iter()
            .any(|action| action.contains("source-tree-relative path"))
    );
    fs::remove_dir_all(root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn worklist_path_filter_does_not_normalize_invalid_evidence_paths() {
    let root = migrate_fixture_dir();
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-doc", FindingKind::NonRustFile);
    entry.evidence = vec!["doc:docs/../src/lib.rs".to_string()];
    cfg.allow.push(entry);

    let items = work_items_from_evidence_diagnostics(&root, &cfg, 1);
    let normalized_filter = filter_work_items(
        items.clone(),
        WorklistFilters {
            path: Some("src/lib.rs"),
            ..WorklistFilters::default()
        },
    );
    let exact_invalid_filter = filter_work_items(
        items,
        WorklistFilters {
            path: Some("docs/../src/lib.rs"),
            ..WorklistFilters::default()
        },
    );

    assert!(
        normalized_filter.is_empty(),
        "invalid local evidence targets must not be routed as their normalized source-tree path"
    );
    assert_eq!(
        exact_invalid_filter.len(),
        1,
        "operators can still filter by the exact invalid evidence target text"
    );
    fs::remove_dir_all(root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn worklist_items_report_weak_evidence_references() {
    let root = migrate_fixture_dir();
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-weak-evidence", FindingKind::Unsafe);
    entry.evidence = vec!["spreadsheet:manual-review".to_string()];
    cfg.allow.push(entry);

    let items = work_items_from_evidence_diagnostics(&root, &cfg, 1);
    let json = render_worklist_json_with_context(&items, WorklistContext::default());

    let item = items
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one work item"));
    assert_eq!(item.kind, "weak_evidence_reference");
    assert_eq!(item.exception_kind.as_deref(), Some("unsafe"));
    assert_eq!(item.risk, "high");
    assert_eq!(item.difficulty, "small");
    assert_eq!(item.status, MatchStatus::EvidenceMissing);
    assert_eq!(item.allow_id.as_deref(), Some("allow-weak-evidence"));
    assert_eq!(
        item.path, None,
        "weak evidence targets are not source-tree paths"
    );
    let reference = item
        .evidence_reference
        .as_ref()
        .unwrap_or_else(|| std::panic::panic_any("weak evidence item should carry reference"));
    assert_eq!(reference.raw, "spreadsheet:manual-review");
    assert_eq!(reference.prefix.as_deref(), Some("spreadsheet"));
    assert_eq!(reference.target.as_deref(), Some("manual-review"));
    assert_eq!(reference.status, "unstructured");
    assert_eq!(reference.category, "unknown_prefix");
    assert!(reference.message.contains("unrecognized evidence prefix"));
    assert!(item.message.contains("unrecognized evidence prefix"));
    assert!(
        item.suggested_actions
            .iter()
            .any(|action| action.contains("unsafe-review"))
    );
    assert!(
        item.suggested_actions
            .iter()
            .any(|action| action.contains("boundary evidence"))
    );
    assert!(
        item.suggested_actions
            .iter()
            .any(|action| action.contains("reviewed unsafe boundary"))
    );
    assert!(json.contains("\"kind\": \"weak_evidence_reference\""));
    assert!(json.contains("\"evidence_reference\""));
    assert!(json.contains("\"target\": \"manual-review\""));
    assert!(json.contains("\"category\": \"unknown_prefix\""));
    assert!(json.contains("\"cargo-allow explain allow-weak-evidence\""));
    assert!(json.contains("\"cargo-allow worklist --weak-evidence --format json\""));
    assert!(json.contains("\"cargo-allow list --weak-evidence --format json\""));
    fs::remove_dir_all(root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn worklist_items_route_migrated_unsafe_todo_evidence() {
    let root = migrate_fixture_dir();
    let policy = root.join("unsafe-allowlist.toml");
    fs::write(
        &policy,
        r#"schema_version = 1
policy = "unsafe-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
path = "src/lib.rs"
family = "unsafe_fn"

[allow.selector]
kind = "unsafe-fn"
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write unsafe policy: {err}")));
    let cfg = allow_policy_legacy::load_legacy_or_canonical(&policy).unwrap_or_else(|err| {
        std::panic::panic_any(format!("legacy unsafe policy should migrate: {err}"))
    });

    let items = work_items_from_evidence_diagnostics(&root, &cfg, 1);

    let item = items
        .iter()
        .find(|item| item.kind == "weak_evidence_reference")
        .unwrap_or_else(|| {
            std::panic::panic_any("migrated unsafe TODO evidence should route as weak evidence")
        });
    assert_eq!(item.exception_kind.as_deref(), Some("unsafe"));
    assert_eq!(item.family.as_deref(), Some("unsafe_fn"));
    assert_eq!(item.risk, "high");
    assert_eq!(item.allow_id.as_deref(), Some("legacy-unsafe-0000"));
    let reference = item.evidence_reference.as_ref().unwrap_or_else(|| {
        std::panic::panic_any("migrated weak evidence item should carry reference details")
    });
    assert!(reference.raw.contains("TODO: add unsafe-review"));
    assert_eq!(reference.category, "unknown_prefix");
    assert!(
        item.suggested_actions
            .iter()
            .any(|action| action.contains("unsafe-review"))
    );
    assert!(
        item.suggested_actions
            .iter()
            .any(|action| action.contains("boundary evidence"))
    );
    assert!(
        item.proof_commands
            .iter()
            .any(|command| command == "cargo-allow check --kind unsafe --mode no-new")
    );
    fs::remove_dir_all(root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn worklist_items_specialize_high_risk_policy_weak_evidence_actions() {
    let root = migrate_fixture_dir();
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-process-weak", FindingKind::PolicyException);
    entry.family = Some("process_spawn".to_string());
    entry.evidence = vec![
        "legacy-policy:proc-cargo-install-cargo-deny".to_string(),
        "binary:cargo".to_string(),
    ];
    cfg.allow.push(entry);

    let items = work_items_from_evidence_diagnostics(&root, &cfg, 1);

    let item = items
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one weak evidence work item"));
    assert_eq!(item.kind, "weak_evidence_reference");
    assert_eq!(item.exception_kind.as_deref(), Some("policy_exception"));
    assert_eq!(item.family.as_deref(), Some("process_spawn"));
    assert_eq!(item.risk, "high");
    assert_eq!(item.allow_id.as_deref(), Some("allow-process-weak"));
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
    fs::remove_dir_all(root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn worklist_items_report_empty_typed_evidence_references() {
    let root = migrate_fixture_dir();
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-empty-evidence", FindingKind::Panic);
    entry.evidence = vec!["test:".to_string()];
    cfg.allow.push(entry);

    let items = work_items_from_evidence_diagnostics(&root, &cfg, 1);

    let item = items
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one work item"));
    assert_eq!(item.kind, "weak_evidence_reference");
    assert_eq!(item.exception_kind.as_deref(), Some("panic"));
    assert_eq!(item.risk, "medium");
    assert_eq!(item.status, MatchStatus::EvidenceMissing);
    assert_eq!(item.allow_id.as_deref(), Some("allow-empty-evidence"));
    assert_eq!(item.path, None);
    let reference = item.evidence_reference.as_ref().unwrap_or_else(|| {
        std::panic::panic_any("empty typed evidence item should carry reference")
    });
    assert_eq!(reference.category, "untyped");
    assert!(item.message.contains("empty evidence reference target"));
    assert!(
        item.suggested_actions
            .iter()
            .any(|action| action.contains("typed evidence reference"))
    );
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
