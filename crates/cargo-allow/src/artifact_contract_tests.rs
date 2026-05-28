use crate::artifact_contract_support::{assert_inventory_contract, parse_json_artifact};
use crate::diff;
use serde_json::Value;

#[test]
fn core_json_artifact_renderers_emit_parseable_v1_contracts() {
    let report_json = allow_report::render_json_with_context(
        "audit",
        &[],
        &[],
        false,
        allow_report::ReportContext {
            inventory_source: "filesystem_fallback",
            source_tree_root: Some("fixtures/source-snapshot"),
            inventory_files: Some(7),
            ..allow_report::ReportContext::default()
        },
    );
    let report = parse_json_artifact(
        "report",
        &report_json,
        allow_report::REPORT_SCHEMA_ID,
        "audit",
    );
    assert_inventory_contract(
        "report",
        &report,
        "filesystem_fallback",
        Some("fixtures/source-snapshot"),
        Some(7),
    );

    let receipt_json = allow_report::render_receipt_with_context(
        "check",
        &[],
        false,
        allow_report::ReportContext {
            inventory_source: "git_tracked",
            source_tree_root: Some("H:/Code/Rust/cargo-allow"),
            inventory_files: Some(42),
            ..allow_report::ReportContext::default()
        },
    );
    let receipt = parse_json_artifact(
        "receipt",
        &receipt_json,
        allow_report::RECEIPT_SCHEMA_ID,
        "check",
    );
    assert_inventory_contract(
        "receipt",
        &receipt,
        "git_tracked",
        Some("H:/Code/Rust/cargo-allow"),
        Some(42),
    );

    let diff_base_json = allow_report::render_json_with_context(
        "diff",
        &[],
        &[],
        false,
        allow_report::ReportContext {
            inventory_source: "git_tracked",
            source_tree_root: Some("H:/Code/Rust/cargo-allow"),
            inventory_files: Some(8),
            ..allow_report::ReportContext::default()
        },
    );
    let diff_json = diff::render_diff_json_with_posture(diff_base_json, &[], &[], &[]);
    let diff = parse_json_artifact("diff", &diff_json, allow_report::REPORT_SCHEMA_ID, "diff");
    assert_eq!(
        diff.pointer("/diff/net_posture").and_then(Value::as_str),
        Some("unchanged"),
        "diff net posture"
    );
}
