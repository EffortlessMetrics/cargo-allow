use std::fs;
use std::path::Path;

use super::support::{
    assert_saved_json_artifact, assert_status, assert_stderr_empty, assert_stdout_empty,
    cargo_allow_command,
};

pub(crate) fn run_cargo_allow(args: &[&str]) -> std::process::Output {
    let output = cargo_allow_command()
        .args(args)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow: {err}")));
    let command = format!("cargo-allow {}", args.join(" "));
    assert_status(&command, &output, true);
    assert_stdout_empty(
        &command,
        &output,
        "should not write stdout when output files are set",
    );
    assert_stderr_empty(
        &command,
        &output,
        "should not write stderr when output files are set",
    );
    output
}

pub(crate) fn assert_source_syntax_artifact(
    path: &Path,
    expected_schema_id: &str,
    expected_command: &str,
) -> serde_json::Value {
    assert_source_syntax_artifact_with_inventory(
        path,
        expected_schema_id,
        expected_command,
        "filesystem_fallback",
    )
}

pub(crate) fn assert_source_syntax_artifact_with_inventory(
    path: &Path,
    expected_schema_id: &str,
    expected_command: &str,
    expected_source: &str,
) -> serde_json::Value {
    let value =
        assert_saved_json_artifact(path, expected_command, expected_schema_id, expected_command);
    assert_inventory(
        &value,
        allow_report::INVENTORY_SCANNER_SOURCE_SYNTAX,
        expected_source,
    );
    value
}

pub(crate) fn assert_policy_migration_artifact(
    path: &Path,
    expected_schema_id: &str,
    expected_command: &str,
) -> serde_json::Value {
    assert_policy_migration_artifact_with_inventory(
        path,
        expected_schema_id,
        expected_command,
        allow_report::INVENTORY_SOURCE_UNKNOWN,
    )
}

pub(crate) fn assert_policy_migration_artifact_with_inventory(
    path: &Path,
    expected_schema_id: &str,
    expected_command: &str,
    expected_source: &str,
) -> serde_json::Value {
    let value =
        assert_saved_json_artifact(path, expected_command, expected_schema_id, expected_command);
    assert_inventory(
        &value,
        allow_report::INVENTORY_SCANNER_POLICY_MIGRATION,
        expected_source,
    );
    value
}

fn assert_inventory(value: &serde_json::Value, expected_scanner: &str, expected_source: &str) {
    assert_eq!(
        value
            .pointer("/inventory/scanner")
            .and_then(serde_json::Value::as_str),
        Some(expected_scanner),
        "inventory scanner"
    );
    assert_eq!(
        value
            .pointer("/inventory/source")
            .and_then(serde_json::Value::as_str),
        Some(expected_source),
        "inventory source"
    );
}

pub(crate) fn assert_policy_output(path: &Path) {
    let text = fs::read_to_string(path).unwrap_or_else(|err| {
        std::panic::panic_any(format!("read policy output {}: {err}", path.display()))
    });
    assert!(
        text.contains("schema_version = \"0.1\""),
        "{} should be policy TOML",
        path.display()
    );
    assert!(
        !text.contains("\"schema_id\""),
        "{} should not contain summary JSON",
        path.display()
    );
}

pub(crate) fn path_arg(path: &Path) -> &str {
    path.to_str()
        .unwrap_or_else(|| std::panic::panic_any(format!("non-UTF-8 path: {}", path.display())))
}
