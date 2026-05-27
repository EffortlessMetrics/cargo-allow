use super::*;
use crate::{CargoAllowCli, CargoAllowCommand};
use allow_core::{Span, StructuralIdentity};
use clap::Parser;
use serde_json::Value;

#[test]
fn clap_parses_add_from_finding() {
    let parsed = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "add",
        "--kind",
        "panic",
        "--path",
        "src/lib.rs",
        "--line",
        "42",
        "--owner",
        "parser",
        "--reason",
        "validated invariant",
        "--evidence",
        "test:parser_invariant",
        "--write",
        "policy/allow.proposed.toml",
        "--force",
        "--summary-format",
        "json",
        "--summary-output",
        "target/add-summary.json",
    ]))
    .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse: {err}")));

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::Add(AddArgs {
            kind,
            path,
            line: 42,
            owner,
            reason,
            evidence,
            write: Some(write),
            force: true,
            summary_format: AddSummaryFormat::Json,
            summary_output: Some(summary_output),
            ..
        })) if kind == "panic"
            && path == Path::new("src/lib.rs")
            && owner == "parser"
            && reason == "validated invariant"
            && evidence == vec!["test:parser_invariant".to_string()]
            && write == Path::new("policy/allow.proposed.toml")
            && summary_output == Path::new("target/add-summary.json")
    ));
}

#[test]
fn select_add_finding_picks_nearest_path_and_kind() {
    let findings = vec![
        test_finding_at_line(
            FindingKind::Panic,
            Some("unwrap"),
            "src/lib.rs",
            "method_call",
            10,
        ),
        test_finding_at_line(
            FindingKind::Panic,
            Some("expect"),
            "src/lib.rs",
            "method_call",
            40,
        ),
        test_finding_at_line(
            FindingKind::Unsafe,
            Some("unsafe_fn"),
            "src/lib.rs",
            "unsafe_fn",
            39,
        ),
    ];
    let kind = parse_kind_filter("panic")
        .unwrap_or_else(|err| std::panic::panic_any(format!("kind should parse: {err}")));

    let (_index, selected) = select_add_finding(&findings, kind, Path::new("src/lib.rs"), 39)
        .unwrap_or_else(|err| std::panic::panic_any(format!("finding should select: {err}")));

    assert_eq!(selected.family.as_deref(), Some("expect"));
    assert_eq!(selected.span.as_ref().map(|span| span.line), Some(40));
}

#[test]
fn select_add_finding_fails_closed_on_equal_nearest_findings() {
    let findings = vec![
        test_finding_at_line(
            FindingKind::Panic,
            Some("unwrap"),
            "src/lib.rs",
            "method_call",
            40,
        ),
        test_finding_at_line(
            FindingKind::Panic,
            Some("expect"),
            "src/lib.rs",
            "method_call",
            42,
        ),
    ];
    let kind = parse_kind_filter("panic")
        .unwrap_or_else(|err| std::panic::panic_any(format!("kind should parse: {err}")));

    let err = select_add_finding(&findings, kind, Path::new("src/lib.rs"), 41)
        .expect_err("equally near findings should be ambiguous");

    assert!(err.to_string().contains("ambiguous add request"));
}

#[test]
fn ensure_addable_outcome_rejects_already_matched_findings() {
    assert!(ensure_addable_outcome(MatchStatus::New).is_ok());

    let err = ensure_addable_outcome(MatchStatus::Matched)
        .expect_err("matched finding should not be addable");

    assert!(err.to_string().contains("already receipted"));
}

#[test]
fn allow_entry_from_finding_uses_structural_selector_and_review_metadata() {
    let mut finding = test_finding_at_line(
        FindingKind::Panic,
        Some("unwrap"),
        "src/lib.rs",
        "method_call",
        42,
    );
    finding.identity.container = Some("parse_span".to_string());
    finding.identity.callee = Some("unwrap".to_string());
    finding.identity.normalized_snippet_hash = Some("fnv1a64:1234".to_string());

    let entry = allow_entry_from_finding(AddEntryRequest {
        finding: &finding,
        id: "allow-0099".to_string(),
        owner: "parser".to_string(),
        classification: "validated_invariant".to_string(),
        reason: "Parser validates the span before unwrapping.".to_string(),
        evidence: vec!["test:parser_validates_span".to_string()],
        review_after: "2026-11-01".to_string(),
        expires: None,
    });

    assert_eq!(entry.id, "allow-0099");
    assert_eq!(entry.owner, "parser");
    assert_eq!(entry.selector.container.as_deref(), Some("parse_span"));
    assert_eq!(entry.selector.callee.as_deref(), Some("unwrap"));
    assert_eq!(
        entry.selector.normalized_snippet_hash.as_deref(),
        Some("fnv1a64:1234")
    );
    assert_eq!(entry.last_seen.as_ref().map(|last| last.line), Some(42));
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
        Some(Path::new("policy/allow.proposed.toml")),
        true,
        AddContext {
            inventory_source: "git_tracked",
            source_tree_root: Some("H:/Code/Rust/cargo-allow"),
            inventory_files: Some(52),
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
fn add_schema_documents_current_contract() {
    let schema = include_str!("../../../docs/schemas/add.schema.json");

    assert!(schema.contains(allow_report::ADD_SCHEMA_ID));
    assert!(schema.contains("\"options\""));
    assert!(schema.contains("\"policy_output\""));
    assert!(schema.contains("\"allow_entry\""));
    assert!(schema.contains("\"selected_finding\""));
    assert!(schema.contains("\"human_review_required\""));
    assert!(schema.contains("\"scanner_limitations\""));
    assert!(schema.contains("\"scanner_limitation\""));
    assert!(schema.contains("\"cargo_metadata_not_invoked\""));
    assert!(schema.contains("\"repository_code_not_executed\""));
}

fn parse_json_artifact(
    name: &str,
    json: &str,
    expected_schema: &str,
    expected_command: &str,
) -> Value {
    let value: Value = serde_json::from_str(json)
        .unwrap_or_else(|err| std::panic::panic_any(format!("{name} json: {err}\n{json}")));
    assert_eq!(
        value.pointer("/schema_id").and_then(Value::as_str),
        Some(expected_schema),
        "{name} schema id"
    );
    assert_eq!(
        value.pointer("/command").and_then(Value::as_str),
        Some(expected_command),
        "{name} command"
    );
    value
}

fn assert_inventory_contract(
    name: &str,
    value: &Value,
    expected_source: &str,
    expected_root: Option<&str>,
    expected_files: Option<u64>,
) {
    assert_eq!(
        value.pointer("/inventory/scope").and_then(Value::as_str),
        Some("source_tree"),
        "{name} inventory scope"
    );
    assert_eq!(
        value.pointer("/inventory/scanner").and_then(Value::as_str),
        Some("source_syntax"),
        "{name} inventory scanner"
    );
    assert_eq!(
        value.pointer("/inventory/source").and_then(Value::as_str),
        Some(expected_source),
        "{name} inventory source"
    );
    assert_eq!(
        value.pointer("/inventory/root").and_then(Value::as_str),
        expected_root,
        "{name} inventory root"
    );
    assert_eq!(
        value
            .pointer("/inventory/files_scanned")
            .and_then(Value::as_u64),
        expected_files,
        "{name} inventory files"
    );
}

fn test_finding_at_line(
    kind: FindingKind,
    family: Option<&str>,
    path: &str,
    ast_kind: &str,
    line: u32,
) -> Finding {
    Finding {
        kind,
        family: family.map(str::to_string),
        path: PathBuf::from(path),
        span: Some(Span { line, column: 1 }),
        identity: StructuralIdentity::new("file", ast_kind),
        message: "test finding".to_string(),
    }
}

fn argv(items: Vec<&str>) -> Vec<String> {
    items.into_iter().map(String::from).collect()
}
