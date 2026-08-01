use super::test_support::test_finding_at_line;
use super::*;
use crate::artifact_contract_support::{assert_inventory_contract, parse_json_artifact};
use serde_json::Value;

#[test]
fn render_broad_add_summary_json_escapes_string_fields() {
    let mut entry = allow_entry_broad(AddBroadRequest {
        id: "allow-\\\"quoted".to_string(),
        kind: FindingKind::Panic,
        family: Some("family\\\"quoted".to_string()),
        callee: None,
        glob: "src/\\\"quoted\\\\path\n.rs".to_string(),
        owner: "owner".to_string(),
        classification: "classification".to_string(),
        reason: "reason".to_string(),
        evidence: vec!["test:json".to_string()],
        review_after: "2026-11-01".to_string(),
        expires: None,
    });
    entry.occurrence_limit = Some(2);

    let json = render_add_summary_broad_json(
        &entry,
        Some("policy/\\\"quoted\\\\output.toml"),
        true,
        &AddContext {
            inventory: allow_report::InventoryContext::source_syntax(
                "git_tracked",
                Some("H:/Code/Rust/cargo-allow"),
                Some(52),
            ),
            repo_root: Some("H:/Code/Rust/cargo-allow".to_string()),
            config_source: Some("policy/allow.toml".to_string()),
        },
    );
    let value: Value = serde_json::from_str(&json).unwrap_or_else(|err| {
        std::panic::panic_any(format!(
            "broad add summary should parse as JSON: {err}\n{json}"
        ))
    });

    assert_eq!(
        value.get("id").and_then(Value::as_str),
        Some(entry.id.as_str())
    );
    assert_eq!(
        value.get("scope").and_then(Value::as_str),
        Some(entry.path_or_glob().as_str())
    );
    assert_eq!(
        value.get("policy_output").and_then(Value::as_str),
        Some("policy/\\\"quoted\\\\output.toml")
    );
    assert_eq!(
        value.get("action").and_then(Value::as_str),
        Some("overwrite")
    );
}

#[test]
fn render_broad_add_summary_human_styles_fixed_baseline_marker() {
    let mut entry = allow_entry_broad(AddBroadRequest {
        id: "allow-broad".to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        callee: Some("unwrap".to_string()),
        glob: "src/**/*.rs".to_string(),
        owner: "owner".to_string(),
        classification: "baseline_debt".to_string(),
        reason: "reason".to_string(),
        evidence: vec!["test:broad".to_string()],
        review_after: "2026-11-01".to_string(),
        expires: None,
    });
    entry.occurrence_limit = Some(2);

    let styled = render_add_summary_broad_human(
        &entry,
        Some("policy/allow.toml"),
        allow_report::Style::ANSI,
    );

    assert_eq!(
        styled,
        "added \u{1b}[31mbroad baseline\u{1b}[0m allow-broad (kind=panic, scope=src/**/*.rs, occurrence_limit=2); policy written to policy/allow.toml\n"
    );
    assert_eq!(styled.matches('\u{1b}').count(), 2);
}

#[test]
fn render_add_summary_json_records_entry_and_selected_finding() {
    let mut finding = test_finding_at_line(
        FindingKind::Panic,
        Some("unwrap"),
        "src/lib.rs",
        "method_call",
        42,
    );
    finding.identity.crate_name = Some("parser".to_string());
    finding.identity.container = Some("parse_span".to_string());
    finding.identity.callee = Some("unwrap".to_string());
    let mut entry = allow_entry_from_finding(AddEntryRequest {
        finding: &finding,
        id: "allow-0101".to_string(),
        owner: "parser".to_string(),
        classification: "validated_invariant".to_string(),
        reason: "Parser validates the span before unwrapping.".to_string(),
        evidence: vec!["test:parser_validates_span".to_string()],
        review_after: "2026-11-01".to_string(),
        expires: Some("2027-01-01".to_string()),
    });
    entry.selector.normalized_snippet_hash = Some("fnv1a64:1234".to_string());

    let json = render_add_summary_json(
        &entry,
        &finding,
        Some("policy/allow.proposed.toml"),
        true,
        AddContext {
            inventory: allow_report::InventoryContext::source_syntax(
                "git_tracked",
                Some("H:/Code/Rust/cargo-allow"),
                Some(52),
            ),
            repo_root: Some("H:/Code/Rust/cargo-allow".to_string()),
            config_source: Some("policy/allow.toml".to_string()),
        },
    );
    let value = parse_json_artifact("add", &json, allow_report::ADD_SCHEMA_ID, "add");

    assert_inventory_contract(
        "add",
        &value,
        "git_tracked",
        Some("H:/Code/Rust/cargo-allow"),
        Some(52),
    );
    assert_eq!(
        value
            .pointer("/options/policy_output")
            .and_then(Value::as_str),
        Some("policy/allow.proposed.toml"),
        "add policy output"
    );
    assert_eq!(
        value.pointer("/options/force").and_then(Value::as_bool),
        Some(true),
        "add force"
    );
    assert_eq!(
        value.pointer("/summary/entry_id").and_then(Value::as_str),
        Some("allow-0101"),
        "add summary entry id"
    );
    assert_eq!(
        value
            .pointer("/summary/human_review_required")
            .and_then(Value::as_bool),
        Some(true),
        "add human_review_required"
    );
    assert_eq!(
        value.pointer("/allow_entry/id").and_then(Value::as_str),
        Some("allow-0101"),
        "add allow id"
    );
    assert_eq!(
        value
            .pointer("/allow_entry/evidence_count")
            .and_then(Value::as_u64),
        Some(1),
        "add evidence count"
    );
    assert_eq!(
        value
            .pointer("/selected_finding/source_package")
            .and_then(Value::as_str),
        Some("parser"),
        "add selected finding source package"
    );
}

#[test]
fn render_add_summary_human_records_inventory_context() {
    let finding = test_finding_at_line(
        FindingKind::Panic,
        Some("unwrap"),
        "src/lib.rs",
        "method_call",
        42,
    );
    let entry = allow_entry_from_finding(AddEntryRequest {
        finding: &finding,
        id: "allow-0101".to_string(),
        owner: "parser".to_string(),
        classification: "validated_invariant".to_string(),
        reason: "Parser validates the span before unwrapping.".to_string(),
        evidence: vec!["test:parser_validates_span".to_string()],
        review_after: "2026-11-01".to_string(),
        expires: Some("2027-01-01".to_string()),
    });

    let text = render_add_summary(
        &entry,
        &finding,
        Some("policy/allow.proposed.toml"),
        AddContext {
            inventory: allow_report::InventoryContext::source_syntax(
                "git_tracked",
                Some("H:/Code/Rust/cargo-allow"),
                Some(52),
            ),
            repo_root: Some("H:/Code/Rust/cargo-allow".to_string()),
            config_source: Some("policy/allow.toml".to_string()),
        },
    );

    assert!(
        text.contains("inventory: source_tree/source_syntax via git_tracked; files scanned: 52")
    );
    assert!(text.contains("source_tree_root: H:/Code/Rust/cargo-allow"));
    assert!(text.contains("matched finding: src/lib.rs:42:1"));
    assert!(text.contains("Claim boundary: scanned source-tree/source syntax only"));
}
