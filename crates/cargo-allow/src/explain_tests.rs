use super::test_support::{test_entry, test_finding};
use super::*;
use crate::{CargoAllowCli, CargoAllowCommand};
use allow_core::FindingKind;
use clap::Parser;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

fn argv(items: Vec<&str>) -> Vec<String> {
    items.into_iter().map(String::from).collect()
}

#[test]
fn clap_parses_explain_id_and_config() {
    let parsed = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "explain",
        "allow-0001",
        "--config",
        "policy/custom.toml",
        "--include-untracked",
        "--format",
        "json",
        "--output",
        "target/explain.json",
    ]))
    .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse: {err}")));

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::Explain(ExplainArgs {
            id,
            config,
            include_untracked: true,
            format: ExplainFormat::Json,
            output,
            ..
        })) if id == "allow-0001"
            && config.as_deref() == Some(Path::new("policy/custom.toml"))
            && output.as_deref() == Some(Path::new("target/explain.json"))
    ));
}

#[test]
fn explain_entry_text_reports_live_match_status() {
    let mut cfg = AllowConfig::empty();
    let entry = test_entry("allow-file", FindingKind::NonRustFile);
    cfg.allow.push(entry.clone());
    let mut finding = test_finding(
        FindingKind::NonRustFile,
        None,
        "tracked.file",
        "tracked_file",
    );
    finding.identity.crate_name = Some("fixture-package".to_string());
    let findings = vec![finding];

    let text = explain_entry_text(Path::new("."), &cfg, &entry, &findings);

    assert!(text.contains("current_status: matched"));
    assert!(text.contains("current_matches: 1"));
    assert!(text.contains("match_outcomes: matched=1"));
    assert!(text.contains("matched: tracked.file:1:1"));
    assert!(text.contains("source_package=fixture-package"));
    assert!(text.contains("Claim boundary: scanned source-tree/source syntax only"));
    assert!(text.contains("did not invoke Cargo metadata"));
    assert!(text.contains("external evidence tools"));
}

#[test]
fn explain_entry_text_reports_baseline_debt_next_actions() {
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-baseline", FindingKind::Panic);
    entry.classification = "baseline_debt".to_string();
    entry.family = Some("unwrap".to_string());
    cfg.allow.push(entry.clone());
    let finding = test_finding(
        FindingKind::Panic,
        Some("unwrap"),
        "tracked.file",
        "tracked_file",
    );

    let text = explain_entry_text(Path::new("."), &cfg, &entry, &[finding]);

    assert!(text.contains("current_status: matched"));
    assert!(text.contains("baseline_debt and still needs human review"));
    assert!(text.contains("next:"));
    assert!(text.contains("action: replace generated baseline debt"));
    assert!(text.contains("proof: cargo-allow explain allow-baseline"));
    assert!(text.contains("proof: cargo-allow check --kind panic --mode no-new"));
}

#[test]
fn explain_entry_text_reports_evidence_reference_status() {
    let root = migrate_fixture_dir();
    fs::create_dir_all(root.join("docs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create docs dir: {err}")));
    fs::write(root.join("docs/safety.md"), "review notes")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write evidence: {err}")));
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-file", FindingKind::NonRustFile);
    entry.evidence = vec![
        "doc:docs/safety.md".to_string(),
        "spec:docs/missing.md".to_string(),
        "test:file_policy_fixture".to_string(),
    ];
    cfg.allow.push(entry.clone());

    let text = explain_entry_text(&root, &cfg, &entry, &[]);

    assert!(text.contains("evidence diagnostics:"));
    assert!(text.contains("doc:docs/safety.md"));
    assert!(text.contains("[ok] present: doc:docs/safety.md"));
    assert!(text.contains("spec:docs/missing.md"));
    assert!(text.contains("[missing] missing: spec:docs/missing.md"));
    assert!(text.contains("test:file_policy_fixture"));
    assert!(text.contains("[info] not-local: test:file_policy_fixture"));
    fs::remove_dir_all(root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn explain_entry_text_reports_local_evidence_outside_source_tree_inventory_as_missing() {
    let root = migrate_fixture_dir();
    fs::create_dir_all(root.join("docs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create docs dir: {err}")));
    fs::write(root.join("docs/untracked.md"), "review notes")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write evidence: {err}")));
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-file", FindingKind::NonRustFile);
    entry.evidence = vec!["doc:docs/untracked.md".to_string()];
    cfg.allow.push(entry.clone());
    let source_tree_files = BTreeSet::new();

    let text = explain_entry_text_with_source_tree_files(
        &root,
        &cfg,
        &entry,
        &[],
        Some(&source_tree_files),
    );

    assert!(text.contains("[missing] missing: doc:docs/untracked.md"));
    assert!(text.contains("not in the default source-tree inventory"));
    fs::remove_dir_all(root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn explain_entry_text_reports_weak_evidence_next_actions() {
    let root = migrate_fixture_dir();
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-weak-evidence", FindingKind::NonRustFile);
    entry.evidence = vec![
        "spreadsheet:manual-review".to_string(),
        "TODO add reviewed evidence".to_string(),
    ];
    cfg.allow.push(entry.clone());
    let finding = test_finding(
        FindingKind::NonRustFile,
        None,
        "tracked.file",
        "tracked_file",
    );

    let text = explain_entry_text(&root, &cfg, &entry, &[finding]);

    assert!(text.contains("current_status: matched"));
    assert!(text.contains("[weak] weak: spreadsheet:manual-review"));
    assert!(text.contains("unrecognized evidence prefix"));
    assert!(text.contains("[weak] weak: TODO add reviewed evidence"));
    assert!(text.contains("unstructured evidence string"));
    assert!(text.contains("action: replace the weak evidence string"));
    assert!(
        text.contains("proof: cargo-allow worklist --allow-id allow-weak-evidence --format json")
    );
    fs::remove_dir_all(root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn explain_entry_text_reports_stale_entry() {
    let mut cfg = AllowConfig::empty();
    let entry = test_entry("allow-file", FindingKind::NonRustFile);
    cfg.allow.push(entry.clone());

    let text = explain_entry_text(Path::new("."), &cfg, &entry, &[]);

    assert!(text.contains("current_status: stale"));
    assert!(text.contains("current_matches: 0"));
    assert!(text.contains("match_outcomes: stale=1"));
    assert!(text.contains("allow-file is stale"));
    assert!(text.contains("next:"));
    assert!(text.contains("action: remove the stale allow entry"));
    assert!(text.contains("proof: cargo-allow explain allow-file"));
}

#[test]
fn explain_entry_text_reports_occurrence_limit_exceeded() {
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-file", FindingKind::NonRustFile);
    entry.occurrence_limit = Some(1);
    cfg.allow.push(entry.clone());
    let finding = test_finding(
        FindingKind::NonRustFile,
        None,
        "tracked.file",
        "tracked_file",
    );
    let findings = vec![finding.clone(), finding];

    let text = explain_entry_text(Path::new("."), &cfg, &entry, &findings);

    assert!(text.contains("occurrence_limit: 1"));
    assert!(text.contains("current_status: new"));
    assert!(text.contains("current_matches: 2"));
    assert!(text.contains("match_outcomes: matched=1, new=1"));
    assert!(text.contains("occurrence_limit exceeded"));
}

static NEXT_EXPLAIN_FIXTURE: AtomicUsize = AtomicUsize::new(0);

fn migrate_fixture_dir() -> PathBuf {
    let id = NEXT_EXPLAIN_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "cargo-allow-cli-explain-{}-{stamp}-{id}",
        std::process::id()
    ));
    fs::create_dir_all(&dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture dir: {err}")));
    dir
}
