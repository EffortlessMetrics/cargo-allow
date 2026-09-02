use super::*;
use crate::artifact_contract_support::parse_json_artifact;
use crate::{CargoAllowCli, CargoAllowCommand, HumanJsonFormat, RootArgs};
use allow_core::{AllowConfig, CargoAllowErrorKind, Span, StructuralIdentity};
use allow_policy::{BASELINE_DEBT_MAX_DAYS, render_policy};
use clap::Parser;
use serde_json::Value;
use std::fs;
use std::path::Path;

#[test]
fn clap_parses_propose_force() {
    let parsed = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "propose",
        "--write",
        "target/proposed.toml",
        "--force",
        "--summary-format",
        "json",
        "--summary-output",
        "target/propose-summary.json",
    ]))
    .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse: {err}")));

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::Propose(ProposeArgs {
            write: Some(path),
            force: true,
            summary_format: HumanJsonFormat::Json,
            summary_output: Some(summary_output),
            ..
        })) if path == Path::new("target/proposed.toml")
            && summary_output == Path::new("target/propose-summary.json")
    ));
}

#[test]
fn propose_summary_collision_rejects_before_touching_live_policy() -> Result<(), String> {
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-propose-summary-collision-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let policy_dir = root.join("policy");
    fs::create_dir_all(&policy_dir).map_err(|error| error.to_string())?;
    let policy = policy_dir.join("allow.toml");
    fs::write(&policy, render_policy(&AllowConfig::empty())).map_err(|error| error.to_string())?;
    let summary_alias = policy_dir.join(".").join("allow.toml");
    let result = cmd_propose(&ProposeArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: Some(policy),
        kind: None,
        include_untracked: false,
        expires: None,
        write: None,
        force: false,
        summary_format: HumanJsonFormat::Human,
        summary_output: Some(summary_alias),
        max: 50,
    });
    let error = match result {
        Ok(()) => return Err("summary collision was accepted".to_string()),
        Err(error) => error,
    };
    if !error.to_string().contains("--summary-output") {
        return Err(format!("unexpected collision error: {error}"));
    }
    let contents =
        fs::read_to_string(root.join("policy/allow.toml")).map_err(|error| error.to_string())?;
    if contents != render_policy(&AllowConfig::empty()) {
        return Err("summary collision changed the live policy sentinel".to_string());
    }
    fs::remove_dir_all(root).map_err(|error| error.to_string())?;
    Ok(())
}

#[test]
fn propose_without_policy_preserves_bootstrap_baseline_path() -> Result<(), String> {
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-propose-no-policy-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("src")).map_err(|error| error.to_string())?;
    fs::write(root.join("src/lib.rs"), "pub fn example() {}\n")
        .map_err(|error| error.to_string())?;
    let target = root.join("policy/allow.toml");
    let result = cmd_propose(&ProposeArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: None,
        kind: None,
        include_untracked: true,
        expires: None,
        write: Some(target.clone()),
        force: false,
        summary_format: HumanJsonFormat::Human,
        summary_output: None,
        max: 50,
    });
    result.map_err(|error| format!("bootstrap propose failed: {error}"))?;
    if !target.is_file() {
        return Err("bootstrap propose did not create a policy".to_string());
    }
    fs::remove_dir_all(root).map_err(|error| error.to_string())?;
    Ok(())
}

#[test]
fn proposal_summary_preserves_distinct_rejection_reasons() {
    let mut reason = None;
    record_unreceiptable_reason(&mut reason, "bare allow forbidden");
    record_unreceiptable_reason(&mut reason, "verified unsafe evidence required");

    assert_eq!(reason, Some(MULTIPLE_UNRECEIPTABLE_REASONS));
    assert!(reason.is_some_and(|value| value.contains("multiple policy requirements")));
}

#[test]
fn write_target_after_containment_errors_as_internal_when_missing() {
    let err = write_target_after_containment(None)
        .expect_err("a missing --write target should fail closed");

    assert_eq!(err.kind(), CargoAllowErrorKind::Internal);
    assert_eq!(err.code(), "E0008_INTERNAL");
    assert_eq!(
        err.to_string(),
        "internal error: --write target missing after containment check"
    );
}

#[test]
fn clap_rejects_invalid_propose_expiry() {
    let err = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "propose",
        "--expires",
        "not-a-date",
    ]))
    .expect_err("invalid generated baseline expiry should fail closed");

    assert!(
        err.to_string().contains("YYYY-MM-DD"),
        "unexpected parse error: {err}"
    );
}

#[test]
fn clap_rejects_long_propose_expiry() {
    let expires = SimpleDate::today_utc_approx()
        .add_days(BASELINE_DEBT_MAX_DAYS + 1)
        .to_string();
    let err =
        CargoAllowCli::try_parse_from(argv(vec!["cargo-allow", "propose", "--expires", &expires]))
            .expect_err("long generated baseline expiry should fail closed");

    assert!(
        err.to_string()
            .contains(&format!("within {BASELINE_DEBT_MAX_DAYS} days")),
        "unexpected parse error: {err}"
    );
}

#[test]
fn clap_accepts_maximum_propose_expiry_window() {
    let expires = SimpleDate::today_utc_approx()
        .add_days(BASELINE_DEBT_MAX_DAYS)
        .to_string();
    let parsed =
        CargoAllowCli::try_parse_from(argv(vec!["cargo-allow", "propose", "--expires", &expires]))
            .unwrap_or_else(|err| {
                std::panic::panic_any(format!(
                    "maximum generated baseline expiry should parse: {err}"
                ))
            });

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::Propose(ProposeArgs {
            expires: Some(parsed_expires),
            ..
        })) if parsed_expires == expires
    ));
}

#[test]
fn propose_summary_reports_generated_baseline_boundary() {
    let text = render_propose_summary(
        super::propose_render::ProposeCounts {
            findings_scanned: 12,
            proposed_entries: 3,
            unsafe_proposed_entries: 1,
            truncated_new_findings: 0,
            unreceiptable_new_findings: 0,
            unreceiptable_reason: None,
        },
        "2026-08-01",
        Some(Path::new("policy/allow.proposed.toml")),
        ProposeContext {
            inventory: allow_report::InventoryContext::source_syntax(
                "git_tracked",
                Some("H:/Code/Rust/cargo-allow"),
                Some(51),
            ),
            kind_filter: Some("panic"),
            mutation_receipt: allow_report::MutationReceipt {
                operation: "propose",
                tool_version: "0.1.10",
                repo_root: Some("H:/Code/Rust/cargo-allow"),
                config_source: Some("policy/allow.toml"),
                ledger_ids: Vec::new(),
                changed_allow_ids: Vec::new(),
                before_fingerprints: Vec::new(),
                after_fingerprints: Vec::new(),
                result: "stdout",
                next_commands: Vec::new(),
            },
        },
    );

    assert!(
        text.contains("inventory: source_tree/source_syntax via git_tracked; files scanned: 51")
    );
    assert!(text.contains("source_tree_root: H:/Code/Rust/cargo-allow"));
    assert!(text.contains("kind filter: panic"));
    assert!(text.contains("findings scanned: 12"));
    assert!(text.contains("baseline_debt entries proposed: 3"));
    assert!(text.contains("unsafe baseline_debt entries proposed: 1"));
    assert!(text.contains("owner: unowned"));
    assert!(text.contains("classification: baseline_debt"));
    assert!(text.contains("output: policy/allow.proposed.toml"));
    assert!(text.contains("generated debt still requires human review"));
    assert!(text.contains("Claim boundary: scanned source-tree/source syntax only"));
}

#[test]
fn default_baseline_expiry_is_relative_to_current_date() {
    let before = SimpleDate::today_utc_approx().add_days(BASELINE_DEBT_DEFAULT_DAYS);
    let expires = default_baseline_expiry();
    let after = SimpleDate::today_utc_approx().add_days(BASELINE_DEBT_DEFAULT_DAYS);
    let parsed = SimpleDate::parse(&expires)
        .unwrap_or_else(|| std::panic::panic_any("default expiry should be a valid date"));

    assert!(
        before <= parsed && parsed <= after,
        "default baseline expiry should stay relative to the current UTC date"
    );
}

#[test]
fn render_propose_summary_json_records_generated_baseline_boundary() {
    let json = sample_propose_json_for_contract_test();
    let value = parse_json_artifact("propose", &json, allow_report::PROPOSE_SCHEMA_ID, "propose");

    assert_eq!(
        value.pointer("/inventory/source").and_then(Value::as_str),
        Some("git_tracked")
    );
    assert_eq!(
        value.pointer("/inventory/root").and_then(Value::as_str),
        Some("H:/Code/Rust/cargo-allow")
    );
    assert_eq!(
        value
            .pointer("/inventory/files_scanned")
            .and_then(Value::as_u64),
        Some(51)
    );
    assert_eq!(
        value.pointer("/options/kind").and_then(Value::as_str),
        Some("panic")
    );
    assert_eq!(
        value.pointer("/options/expires").and_then(Value::as_str),
        Some("2026-08-01")
    );
    assert_eq!(
        value
            .pointer("/options/policy_output")
            .and_then(Value::as_str),
        Some("policy/allow.proposed.toml")
    );
    assert_eq!(
        value.pointer("/options/force").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        value
            .pointer("/summary/findings_scanned")
            .and_then(Value::as_u64),
        Some(12)
    );
    assert_eq!(
        value
            .pointer("/summary/baseline_debt_entries_proposed")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        value
            .pointer("/summary/unsafe_baseline_debt_entries_proposed")
            .and_then(Value::as_u64),
        Some(1)
    );
    let queues = value
        .pointer("/follow_up_queues")
        .and_then(Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("propose should emit follow-up queues"));
    assert_eq!(queues.len(), 2);
    assert_eq!(
        queues[0].pointer("/signal").and_then(Value::as_str),
        Some("baseline_debt_entries_proposed")
    );
    assert_eq!(
        queues[0].pointer("/route_kind").and_then(Value::as_str),
        Some("worklist_filter")
    );
    assert_eq!(
        queues[0].pointer("/item_kind").and_then(Value::as_str),
        Some("baseline_debt")
    );
    assert_eq!(
        queues[0]
            .pointer("/worklist_filter")
            .and_then(Value::as_str),
        Some("baseline_debt")
    );
    assert_eq!(queues[0].pointer("/count").and_then(Value::as_u64), Some(3));
    assert_eq!(
        queues[0].pointer("/command").and_then(Value::as_str),
        Some("cargo-allow worklist --baseline-debt --format json")
    );
    assert_eq!(
        queues[1].pointer("/signal").and_then(Value::as_str),
        Some("unsafe_baseline_debt_entries_proposed")
    );
    assert_eq!(
        queues[1].pointer("/route_kind").and_then(Value::as_str),
        Some("worklist_item_kind")
    );
    assert_eq!(
        queues[1].pointer("/item_kind").and_then(Value::as_str),
        Some("weak_evidence_reference")
    );
    assert_eq!(queues[1].pointer("/count").and_then(Value::as_u64), Some(1));
    assert_eq!(
        queues[1].pointer("/command").and_then(Value::as_str),
        Some(
            "cargo-allow worklist --item-kind weak_evidence_reference --kind unsafe --format json"
        )
    );
    assert_eq!(
        value
            .pointer("/generated_entry_defaults/owner")
            .and_then(Value::as_str),
        Some("unowned")
    );
    assert_eq!(
        value
            .pointer("/generated_entry_defaults/classification")
            .and_then(Value::as_str),
        Some("baseline_debt")
    );
    assert_eq!(
        value
            .pointer("/generated_entry_defaults/reason")
            .and_then(Value::as_str),
        Some("Generated by cargo-allow propose; requires human review.")
    );
    assert_eq!(
        value
            .pointer("/generated_entry_defaults/expires")
            .and_then(Value::as_str),
        Some("2026-08-01")
    );
}

#[test]
fn proposed_baseline_entry_uses_current_created_date() {
    let before = SimpleDate::today_utc_approx();
    let entry = entry_from_finding(
        &Finding {
            kind: FindingKind::Panic,
            family: Some("unwrap".to_string()),
            path: "src/lib.rs".into(),
            span: Some(Span { line: 1, column: 1 }),
            identity: StructuralIdentity::new("rust", "method_call"),
            message: "test finding".to_string(),
            ledger: None,
        },
        1,
        "2026-08-01",
    );
    let after = SimpleDate::today_utc_approx();
    let created = entry
        .lifecycle
        .created
        .as_deref()
        .and_then(SimpleDate::parse)
        .unwrap_or_else(|| std::panic::panic_any("entry should have a valid created date"));

    assert!(
        before <= created && created <= after,
        "generated baseline entries should use the current UTC date"
    );
}

fn argv(items: Vec<&str>) -> Vec<String> {
    items.into_iter().map(String::from).collect()
}
