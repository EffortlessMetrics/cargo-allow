use super::*;
use crate::{CargoAllowCli, CargoAllowCommand};
use allow_core::{AllowEntry, FindingKind, Lifecycle, Selector, Span, StructuralIdentity};
use clap::Parser;
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
}

#[test]
fn explain_entry_json_records_context_and_live_status() {
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-json", FindingKind::NonRustFile);
    entry.family = Some("documentation".to_string());
    entry.evidence = vec!["test:explain_fixture".to_string()];
    entry.lifecycle.created = Some("2026-05-27".to_string());
    entry.lifecycle.review_after = Some("2026-11-01".to_string());
    cfg.allow.push(entry.clone());
    let mut finding = test_finding(
        FindingKind::NonRustFile,
        Some("documentation"),
        "tracked.file",
        "tracked_file",
    );
    finding.identity.crate_name = Some("allow-core".to_string());

    let json = explain_entry_json(
        Path::new("."),
        &cfg,
        &entry,
        &[finding],
        ExplainContext {
            inventory_source: "git_tracked",
            source_tree_root: Some("H:/Code/Rust/cargo-allow"),
            inventory_files: Some(47),
        },
    );

    assert!(json.contains("\"schema_version\": 1"));
    assert!(json.contains(&format!(
        "\"schema_id\": \"{}\"",
        allow_report::EXPLAIN_SCHEMA_ID
    )));
    assert!(json.contains("\"command\": \"explain\""));
    assert!(json.contains("\"claim_boundary\""));
    assert!(json.contains("\"scanner_limitations\""));
    assert!(json.contains("\"cargo_metadata_not_invoked\""));
    assert!(json.contains("\"repository_code_not_executed\""));
    assert!(json.contains("\"source\": \"git_tracked\""));
    assert!(json.contains("\"root\": \"H:/Code/Rust/cargo-allow\""));
    assert!(json.contains("\"files_scanned\": 47"));
    assert!(json.contains("\"id\": \"allow-json\""));
    assert!(json.contains("\"current_status\": \"matched\""));
    assert!(json.contains("\"current_matches\": 1"));
    assert!(json.contains("\"path\": \"tracked.file\""));
    assert!(json.contains("\"source_package\": \"allow-core\""));
    assert!(json.contains("\"status\": \"traceability_only\""));
    assert!(json.contains("\"suggested_actions\": []"));
    assert!(json.contains("\"proof_commands\": []"));
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

    assert!(text.contains("evidence references:"));
    assert!(text.contains("doc:docs/safety.md"));
    assert!(text.contains("status=local_file_present"));
    assert!(text.contains("spec:docs/missing.md"));
    assert!(text.contains("status=local_file_missing"));
    assert!(text.contains("test:file_policy_fixture"));
    assert!(text.contains("status=traceability_only"));
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

#[test]
fn explain_schema_documents_current_contract() {
    let schema = include_str!("../../../docs/schemas/explain.schema.json");

    assert!(schema.contains(allow_report::EXPLAIN_SCHEMA_ID));
    assert!(schema.contains("\"allow_entry\""));
    assert!(schema.contains("\"evidence_references\""));
    assert!(schema.contains("\"current_findings\""));
    assert!(schema.contains("\"match_outcomes\""));
    assert!(schema.contains("\"next\""));
    assert!(schema.contains("\"scanner_limitations\""));
    assert!(schema.contains("\"scanner_limitation\""));
    assert!(schema.contains("\"source_package\""));
    assert!(schema.contains("\"cargo_metadata_not_invoked\""));
    assert!(schema.contains("\"repository_code_not_executed\""));
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

fn test_entry(id: &str, kind: FindingKind) -> AllowEntry {
    AllowEntry {
        id: id.to_string(),
        kind,
        family: None,
        path: Some(PathBuf::from("tracked.file")),
        glob: None,
        owner: "owner".to_string(),
        classification: "classification".to_string(),
        reason: "reason".to_string(),
        evidence: Vec::new(),
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle::empty(),
        selector: Selector {
            ast_kind: Some("tracked_file".to_string()),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn test_finding(kind: FindingKind, family: Option<&str>, path: &str, ast_kind: &str) -> Finding {
    Finding {
        kind,
        family: family.map(str::to_string),
        path: PathBuf::from(path),
        span: Some(Span { line: 1, column: 1 }),
        identity: StructuralIdentity::new("file", ast_kind),
        message: "test finding".to_string(),
    }
}
