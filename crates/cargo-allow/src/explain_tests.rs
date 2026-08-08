use super::test_support::{test_entry, test_finding};
use super::*;
use crate::{CargoAllowCli, CargoAllowCommand, HumanJsonFormat, ProfileArg, RootArgs};
use allow_core::{FindingKind, MatchOutcome, MatchStatus};
use clap::Parser;
use serde_json::Value;
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
            format: HumanJsonFormat::Json,
            output,
            ..
        })) if id == "allow-0001"
            && config.as_deref() == Some(Path::new("policy/custom.toml"))
            && output.as_deref() == Some(Path::new("target/explain.json"))
    ));
}

#[test]
fn clap_parses_spec_system_profile_for_explain() {
    let parsed = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "explain",
        "CARGO-ALLOW-SPEC-0001",
        "--profile",
        "spec-system",
        "--format",
        "json",
        "--output",
        "target/spec-system-explain.json",
    ]))
    .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse: {err}")));

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::Explain(ExplainArgs {
            id,
            profile: Some(ProfileArg::SpecSystem),
            format: HumanJsonFormat::Json,
            output,
            ..
        })) if id == "CARGO-ALLOW-SPEC-0001"
            && output.as_deref() == Some(Path::new("target/spec-system-explain.json"))
    ));
}

#[test]
fn explain_core_summary_routes_attention_with_invocation_context() {
    let entry = test_entry("allow-explain-attention", FindingKind::Panic);
    let outcomes = vec![MatchOutcome {
        status: MatchStatus::Expired,
        allow_id: Some(entry.id.clone()),
        candidate_ids: vec![entry.id.clone()],
        finding_index: None,
        message: "expired entry".to_string(),
        score: 0,
    }];
    let inventory =
        allow_report::InventoryContext::source_syntax("git_tracked", Some("F:/repo"), Some(12))
            .with_completeness("complete");
    let summary = build_explain_summary(
        &entry,
        &outcomes,
        inventory,
        Some(Path::new("repo")),
        Some(Path::new("policy/allow.toml")),
        true,
    )
    .unwrap_or_else(|error| std::panic::panic_any(format!("explain summary: {error}")));

    assert_eq!(summary.result_class, ResultClassV1::Findings);
    assert_eq!(summary.posture, CoreCommandPostureV1::Blocking);
    assert_eq!(
        summary
            .primary_action
            .as_ref()
            .map(|action| action.args.clone()),
        Some(vec![
            "worklist".to_string(),
            "--allow-id".to_string(),
            "allow-explain-attention".to_string(),
            "--format".to_string(),
            "json".to_string(),
            "--root".to_string(),
            "repo".to_string(),
            "--config".to_string(),
            "policy/allow.toml".to_string(),
            "--include-untracked".to_string(),
        ])
    );
}

#[test]
fn explain_json_adds_schema_validated_core_summary_without_replacing_detail() -> Result<(), String>
{
    let entry = test_entry("allow-explain-json", FindingKind::Panic);
    let inventory = allow_report::InventoryContext::source_syntax("git_tracked", None, Some(4))
        .with_completeness("complete");
    let summary = build_explain_summary(&entry, &[], inventory, None, None, false)
        .map_err(|error| format!("explain summary: {error}"))?;
    let json = add_core_summary_to_explain_json(&sample_explain_json_for_contract_test(), &summary)
        .map_err(|error| format!("explain JSON projection: {error}"))?;
    let value: Value = serde_json::from_str(&json).map_err(|error| error.to_string())?;
    let schema: Value =
        serde_json::from_str(include_str!("../../../docs/schemas/explain.schema.json"))
            .map_err(|error| format!("explain schema JSON: {error}"))?;
    let validator = jsonschema::validator_for(&schema)
        .map_err(|error| format!("explain schema compilation: {error}"))?;
    validator
        .validate(&value)
        .map_err(|error| format!("explain core summary violates schema: {error}"))?;
    assert_eq!(
        value
            .pointer("/core_command_summary/operation")
            .and_then(Value::as_str),
        Some("explain")
    );
    assert!(value.get("allow_entry").is_some());
    Ok(())
}

#[test]
fn explain_spec_system_profile_json_reports_one_artifact() {
    let root = spec_system_fixture_dir();
    write_spec_system_fixture(&root);
    let output = root.join("spec-system-explain.json");

    let result = cmd_explain(&ExplainArgs {
        id: "CARGO-ALLOW-SPEC-0001".to_string(),
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: None,
        profile: Some(ProfileArg::SpecSystem),
        include_untracked: false,
        format: HumanJsonFormat::Json,
        output: Some(output.clone()),
    });

    assert!(
        result.is_ok(),
        "spec-system explain should pass: {:?}",
        result.err()
    );
    let json = fs::read_to_string(&output)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read explain JSON: {err}")));
    let value = serde_json::from_str::<Value>(&json)
        .unwrap_or_else(|err| std::panic::panic_any(format!("parse explain JSON: {err}\n{json}")));
    fs::remove_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));

    assert_eq!(
        value.get("schema_id").and_then(Value::as_str),
        Some(allow_report::SPEC_SYSTEM_SCHEMA_ID)
    );
    assert_eq!(
        value.get("command").and_then(Value::as_str),
        Some("explain")
    );
    assert_eq!(
        value.get("profile").and_then(Value::as_str),
        Some("spec-system")
    );
    assert_eq!(
        value.get("explained_artifact_id").and_then(Value::as_str),
        Some("CARGO-ALLOW-SPEC-0001")
    );
    assert_eq!(
        value.pointer("/summary/artifacts").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        value.pointer("/artifacts/0/id").and_then(Value::as_str),
        Some("CARGO-ALLOW-SPEC-0001")
    );
    let links = value
        .get("links")
        .and_then(Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("spec-system explain should include links"));
    assert!(
        links.iter().any(|link| {
            link.get("source_id").and_then(Value::as_str) == Some("CARGO-ALLOW-SPEC-0001")
                && link.get("field").and_then(Value::as_str) == Some("linked_proposal")
        }),
        "spec-system explain should include outgoing links: {json}"
    );
    assert!(
        links.iter().any(|link| {
            link.get("target").and_then(Value::as_str) == Some("CARGO-ALLOW-SPEC-0001")
                && link.get("source_id").and_then(Value::as_str) == Some("CARGO-ALLOW-SUPPORT-0001")
        }),
        "spec-system explain should include incoming links: {json}"
    );
    let proof_commands = value
        .get("proof_commands")
        .and_then(Value::as_array)
        .unwrap_or_else(|| {
            std::panic::panic_any("spec-system explain should include proof commands")
        });
    assert!(proof_commands.iter().any(|command| {
        command.as_str() == Some("cargo-allow explain CARGO-ALLOW-SPEC-0001 --profile spec-system")
    }));
    let claim_boundary = value
        .get("claim_boundary")
        .and_then(Value::as_array)
        .unwrap_or_else(|| {
            std::panic::panic_any("spec-system explain should include claim boundary")
        });
    assert!(
        claim_boundary
            .iter()
            .any(|flag| { flag.as_str() == Some("proof_commands_not_executed") })
    );
}

#[test]
fn explain_spec_system_profile_human_reports_artifact_links_and_boundary() {
    let root = spec_system_fixture_dir();
    write_spec_system_fixture(&root);
    let output = root.join("spec-system-explain.md");

    let result = cmd_explain(&ExplainArgs {
        id: "CARGO-ALLOW-SPEC-0001".to_string(),
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: None,
        profile: Some(ProfileArg::SpecSystem),
        include_untracked: false,
        format: HumanJsonFormat::Human,
        output: Some(output.clone()),
    });

    assert!(
        result.is_ok(),
        "spec-system explain should pass: {:?}",
        result.err()
    );
    let text = fs::read_to_string(&output)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read explain text: {err}")));
    fs::remove_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));

    assert!(text.contains("# cargo-allow explain CARGO-ALLOW-SPEC-0001 --profile spec-system"));
    assert!(text.contains("## Artifact"));
    assert!(text.contains("## Outgoing Links"));
    assert!(text.contains("## Incoming Links"));
    assert!(text.contains("CARGO-ALLOW-SUPPORT-0001"));
    assert!(text.contains("## Proof Commands"));
    assert!(text.contains("proof commands"));
    assert!(text.contains("did not execute proof commands"));
}

#[test]
fn explain_spec_system_profile_rejects_include_untracked() {
    let root = spec_system_fixture_dir();
    write_spec_system_fixture(&root);

    let result = cmd_explain(&ExplainArgs {
        id: "CARGO-ALLOW-SPEC-0001".to_string(),
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: None,
        profile: Some(ProfileArg::SpecSystem),
        include_untracked: true,
        format: HumanJsonFormat::Human,
        output: None,
    });

    fs::remove_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(
        err.to_string()
            .contains("--include-untracked is not supported with --profile spec-system")
    );
}

#[test]
fn explain_spec_system_profile_rejects_unknown_artifact() {
    let root = spec_system_fixture_dir();
    write_spec_system_fixture(&root);

    let result = cmd_explain(&ExplainArgs {
        id: "CARGO-ALLOW-SPEC-9999".to_string(),
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: None,
        profile: Some(ProfileArg::SpecSystem),
        include_untracked: false,
        format: HumanJsonFormat::Human,
        output: None,
    });

    fs::remove_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(
        err.to_string()
            .contains("no spec-system artifact `CARGO-ALLOW-SPEC-9999`")
    );
}

#[test]
fn missing_explain_entry_is_a_usage_error() {
    let err = super::missing_allow_entry_error("allow-missing");

    assert_eq!(err.kind(), allow_core::CargoAllowErrorKind::Usage);
    assert!(err.to_string().contains("no allow entry `allow-missing`"));
    assert!(err.to_string().contains("cargo-allow list"));
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
fn explain_entry_text_reports_empty_evidence_next_actions() {
    let mut cfg = AllowConfig::empty();
    let entry = test_entry("allow-file", FindingKind::NonRustFile);
    cfg.allow.push(entry.clone());
    let finding = test_finding(
        FindingKind::NonRustFile,
        None,
        "tracked.file",
        "tracked_file",
    );

    let text = explain_entry_text(Path::new("."), &cfg, &entry, &[finding]);

    assert!(text.contains("current_status: matched"));
    assert!(text.contains("evidence: none"));
    assert!(text.contains("next:"));
    assert!(text.contains("action: add evidence that supports the exception reason"));
    assert!(text.contains("proof: cargo-allow worklist --missing-evidence --format json"));
    assert!(text.contains("proof: cargo-allow check --kind non-rust --mode no-new"));
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

    assert!(text.contains("current_status: baseline_debt"));
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
    assert!(text.contains("present: doc:docs/safety.md (status=local_file_present"));
    assert!(text.contains("spec:docs/missing.md"));
    assert!(text.contains("missing: spec:docs/missing.md (status=local_file_missing"));
    assert!(text.contains("test:file_policy_fixture"));
    assert!(text.contains("not-local: test:file_policy_fixture (status=traceability_only"));
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

    let finding = test_finding(
        FindingKind::NonRustFile,
        None,
        "tracked.file",
        "tracked_file",
    );

    let text = explain_entry_text_with_source_tree_files(
        &root,
        &cfg,
        &entry,
        &[finding],
        Some(&source_tree_files),
        allow_report::Style::PLAIN,
    );

    assert!(text.contains("missing: doc:docs/untracked.md (status=local_file_missing"));
    assert!(text.contains("not in the default source-tree inventory"));
    assert!(text.contains("action: commit the referenced evidence file"));
    assert!(text.contains("action: or rerun with --include-untracked"));
    assert!(text.contains("proof: cargo-allow explain allow-file --include-untracked"));
    assert!(text.contains("proof: cargo-allow check --include-untracked --mode no-new"));
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
    assert!(text.contains("weak: spreadsheet:manual-review (status=unstructured"));
    assert!(text.contains("unrecognized evidence prefix"));
    assert!(text.contains("weak: TODO add reviewed evidence (status=unstructured"));
    assert!(text.contains("unstructured evidence string"));
    assert!(text.contains("action: replace the weak evidence string"));
    assert!(
        text.contains("proof: cargo-allow worklist --allow-id allow-weak-evidence --format json")
    );
    fs::remove_dir_all(root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn explain_entry_text_specializes_high_risk_policy_weak_evidence_actions() {
    let root = migrate_fixture_dir();
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-process-weak", FindingKind::PolicyException);
    entry.family = Some("process_spawn".to_string());
    entry.path = Some(PathBuf::from(".github/workflows/ci.yml"));
    entry.selector.ast_kind = Some("process_spawn".to_string());
    entry.evidence = vec![
        "legacy-policy:proc-cargo-install-cargo-deny".to_string(),
        "binary:cargo".to_string(),
    ];
    cfg.allow.push(entry.clone());
    let finding = test_finding(
        FindingKind::PolicyException,
        Some("process_spawn"),
        ".github/workflows/ci.yml",
        "process_spawn",
    );

    let text = explain_entry_text(&root, &cfg, &entry, &[finding]);

    assert!(text.contains("current_status: matched"));
    assert!(text.contains("weak: binary:cargo (status=unstructured"));
    assert!(text.contains(
        "action: replace weak evidence with typed evidence for policy_exception.process_spawn"
    ));
    assert!(text.contains("action: keep custom legacy facts only as supporting context"));
    fs::remove_dir_all(root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn explain_entry_text_reports_weak_link_next_actions() {
    let root = migrate_fixture_dir();
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-weak-link", FindingKind::NonRustFile);
    entry.links = vec!["spreadsheet:manual-review".to_string()];
    cfg.allow.push(entry.clone());
    let finding = test_finding(
        FindingKind::NonRustFile,
        None,
        "tracked.file",
        "tracked_file",
    );

    let text = explain_entry_text(&root, &cfg, &entry, &[finding]);

    assert!(text.contains("link diagnostics:"));
    assert!(text.contains("weak: spreadsheet:manual-review (status=unstructured"));
    assert!(text.contains("message: unrecognized link prefix"));
    assert!(text.contains("action: replace the weak link string"));
    assert!(text.contains("proof: cargo-allow worklist --allow-id allow-weak-link --format json"));
    fs::remove_dir_all(root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn explain_entry_text_specializes_high_risk_policy_weak_link_actions() {
    let root = migrate_fixture_dir();
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-network-weak-link", FindingKind::PolicyException);
    entry.family = Some("network_destination".to_string());
    entry.path = Some(PathBuf::from("policy/network-allowlist.toml"));
    entry.selector.ast_kind = Some("network_destination".to_string());
    entry.links = vec!["spreadsheet:manual-review".to_string()];
    cfg.allow.push(entry.clone());
    let finding = test_finding(
        FindingKind::PolicyException,
        Some("network_destination"),
        "policy/network-allowlist.toml",
        "network_destination",
    );

    let text = explain_entry_text(&root, &cfg, &entry, &[finding]);

    assert!(text.contains("current_status: matched"));
    assert!(text.contains("weak: spreadsheet:manual-review (status=unstructured"));
    assert!(text.contains(
        "action: replace weak traceability with typed traceability for policy_exception.network_destination"
    ));
    assert!(text.contains("action: keep custom legacy notes only as supporting context"));
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

fn spec_system_fixture_dir() -> PathBuf {
    let dir = migrate_fixture_dir();
    fs::create_dir_all(&dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture dir: {err}")));
    dir
}

fn write_spec_system_fixture(root: &Path) {
    write_explain_fixture_file(root, "policy/spec-system.toml", spec_system_config());
    write_explain_fixture_file(
        root,
        "policy/doc-artifacts.toml",
        spec_system_doc_artifacts(),
    );
    write_explain_fixture_file(
        root,
        "docs/proposals/CARGO-ALLOW-PROP-0001-example.md",
        "CARGO-ALLOW-PROP-0001\n",
    );
    write_explain_fixture_file(
        root,
        "docs/specs/CARGO-ALLOW-SPEC-0001-example.md",
        "CARGO-ALLOW-SPEC-0001\n",
    );
    write_explain_fixture_file(
        root,
        "docs/status/SUPPORT_TIERS.md",
        spec_system_support_tiers(),
    );
}

fn write_explain_fixture_file(root: &Path, relative_path: &str, contents: &str) {
    let path = root.join(relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .unwrap_or_else(|err| std::panic::panic_any(format!("fixture parent: {err}")));
    }
    fs::write(&path, contents)
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture file: {err}")));
}

fn spec_system_config() -> &'static str {
    r#"
schema_version = "1.0"
profile = "spec-system"
mode = "blocking"

[roots]
proposals = "docs/proposals"
specs = "docs/specs"
adrs = "docs/adr"
plans = "plans"
goals = ".codex/goals"
support_tiers = "docs/status/SUPPORT_TIERS.md"
artifact_ledger = "policy/doc-artifacts.toml"

[requirements]
ledger_required = true
templates_required = false
support_tiers_required = true
active_goal_required = false
closeout_required_for_done_items = false
"#
}

fn spec_system_doc_artifacts() -> &'static str {
    r#"
schema_version = "1.0"
policy = "cargo-allow-doc-artifacts"
owner = "repo-infra"
status = "advisory"

[[artifact]]
id = "CARGO-ALLOW-PROP-0001"
kind = "proposal"
path = "docs/proposals/CARGO-ALLOW-PROP-0001-example.md"
status = "accepted"
owner = "repo-infra"
created = "2026-06-12"

[[artifact]]
id = "CARGO-ALLOW-SPEC-0001"
kind = "spec"
path = "docs/specs/CARGO-ALLOW-SPEC-0001-example.md"
status = "accepted"
owner = "repo-infra"
created = "2026-06-12"
linked_proposal = "CARGO-ALLOW-PROP-0001"

[[artifact]]
id = "CARGO-ALLOW-SUPPORT-0001"
kind = "support_tier"
path = "docs/status/SUPPORT_TIERS.md"
status = "active"
owner = "repo-infra"
created = "2026-06-12"
linked_proposal = "CARGO-ALLOW-PROP-0001"
linked_spec = "CARGO-ALLOW-SPEC-0001"
"#
}

fn spec_system_support_tiers() -> &'static str {
    r#"
# Support Tiers

CARGO-ALLOW-SUPPORT-0001

| Surface | Tier | Claim | Proof command | Notes |
| --- | --- | --- | --- | --- |
| Source exception ledger | Stable | Source-tree findings are checked against policy. | cargo-allow check --mode no-new | Source-tree only. |
| Spec-system profile | Advisory | The repo carries graph artifacts. | cargo-allow check --profile spec-system --mode audit | Structural only. |
"#
}
