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
        allow_report::ReportContext::source_syntax(
            "filesystem_fallback",
            Some("fixtures/source-snapshot"),
            Some(7),
            None,
        ),
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
        allow_report::ReportContext::source_syntax(
            "git_tracked",
            Some("H:/Code/Rust/cargo-allow"),
            Some(42),
            None,
        ),
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
        allow_report::ReportContext::source_syntax(
            "git_tracked",
            Some("H:/Code/Rust/cargo-allow"),
            Some(8),
            None,
        ),
    );
    let cfg = allow_core::AllowConfig::empty();
    let diff_json =
        diff::render_diff_json_with_posture(diff_base_json, 0, &[], &[], &[], &cfg, &cfg);
    let diff = parse_json_artifact("diff", &diff_json, allow_report::REPORT_SCHEMA_ID, "diff");
    assert_eq!(
        diff.pointer("/diff/net_posture").and_then(Value::as_str),
        Some("unchanged"),
        "diff net posture"
    );
}
