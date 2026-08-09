use crate::artifact_contract_samples::{command_artifact_samples, core_artifact_samples};
use crate::artifact_contract_support::{assert_inventory_contract, parse_json_artifact};
use crate::artifact_schema_support::schema_contracts;
use crate::diff;
use serde_json::Value;
use std::collections::BTreeSet;

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
    let ledger = diff::DiffLedgerContext::new(
        &cfg,
        &cfg,
        &[],
        &[],
        allow_report::DiffAnalysisContext::default(),
    );
    let diff_json = diff::render_diff_json_with_posture(diff_base_json, 0, &[], &ledger);
    let diff = parse_json_artifact("diff", &diff_json, allow_report::REPORT_SCHEMA_ID, "diff");
    assert_eq!(
        diff.pointer("/diff/net_posture").and_then(Value::as_str),
        Some("unchanged"),
        "diff net posture"
    );
}

#[test]
fn receipt_result_classes_preserve_status_and_error_diagnostic() {
    let failed_json = allow_report::render_receipt_with_context(
        "check",
        &[],
        true,
        allow_report::ReportContext::source_syntax("git_tracked", None, None, None),
    );
    let failed = parse_json_artifact(
        "receipt_failed",
        &failed_json,
        allow_report::RECEIPT_SCHEMA_ID,
        "check",
    );
    assert_eq!(failed.get("status").and_then(Value::as_str), Some("failed"));
    assert_eq!(failed.get("failed").and_then(Value::as_bool), Some(true));

    let error_json = allow_report::render_error_receipt(
        "invalid policy field: \"mode\"",
        allow_report::ReportContext::source_syntax("filesystem_fallback", None, None, None),
    );
    let error = parse_json_artifact(
        "receipt_error",
        &error_json,
        allow_report::RECEIPT_SCHEMA_ID,
        "check",
    );
    assert_eq!(error.get("status").and_then(Value::as_str), Some("error"));
    assert_eq!(error.get("failed").and_then(Value::as_bool), Some(true));
    assert_eq!(
        error.get("diagnostic").and_then(Value::as_str),
        Some("invalid policy field: \"mode\"")
    );
}

#[test]
fn rendered_artifact_samples_validate_against_json_schemas() -> Result<(), String> {
    let mut samples = command_artifact_samples();
    samples.extend(core_artifact_samples());
    let schemas = schema_contracts();
    let mut validated_schema_names = BTreeSet::new();

    for sample in samples {
        let contract = schemas
            .iter()
            .find(|contract| contract.name == sample.schema_name)
            .ok_or_else(|| {
                format!(
                    "sample {} references unknown schema {}",
                    sample.name, sample.schema_name
                )
            })?;
        let schema = serde_json::from_str(contract.schema)
            .map_err(|error| format!("{} schema JSON: {error}", sample.schema_name))?;
        let instance = serde_json::from_str(&sample.json)
            .map_err(|error| format!("{} sample JSON: {error}", sample.name))?;
        let validator = jsonschema::validator_for(&schema)
            .map_err(|error| format!("{} schema compilation: {error}", sample.schema_name))?;
        validator.validate(&instance).map_err(|error| {
            format!(
                "{} sample does not validate against {}: {error}",
                sample.name, sample.schema_name
            )
        })?;
        validated_schema_names.insert(contract.name);
    }

    let expected_schema_names = schemas
        .iter()
        .map(|contract| contract.name)
        .collect::<BTreeSet<_>>();
    if validated_schema_names != expected_schema_names {
        return Err(format!(
            "producer samples do not cover every schema: expected {expected_schema_names:?}, got {validated_schema_names:?}"
        ));
    }

    Ok(())
}
