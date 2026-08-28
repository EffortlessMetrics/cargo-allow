use std::fs;
use std::path::Path;

use super::support::{
    assert_saved_json_artifact, assert_status, assert_stderr_empty, assert_stdout_empty,
    cargo_allow_command,
};

const FORBIDDEN_PROOF_COMMAND_TOOL_TOKENS: &[&str] = &[
    "cargo",
    "rustc",
    "clippy",
    "clippy-driver",
    "cargo-clippy",
    "cargo-deny",
    "cargo-vet",
    "cargo-geiger",
    "ripr",
    "unsafe-review",
    "cargo-llvm-cov",
    "llvm-cov",
    "grcov",
    "tarpaulin",
    "cargo-tarpaulin",
];

pub(crate) fn run_cargo_allow(args: &[&str]) -> std::process::Output {
    run_cargo_allow_expect_status(args, true)
}

pub(crate) fn run_cargo_allow_expect_status(
    args: &[&str],
    should_succeed: bool,
) -> std::process::Output {
    let output = cargo_allow_command()
        .args(args)
        .output()
        .unwrap_or_else(|err| panic!("run cargo-allow: {err}"));
    let command = format!("cargo-allow {}", args.join(" "));
    assert_status(&command, &output, should_succeed);
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
        panic!("read policy output {}: {err}", path.display())
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

pub(crate) fn assert_proof_commands_stay_cargo_allow(value: &serde_json::Value, pointer: &str) {
    let commands = value
        .pointer(pointer)
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| {
            panic!("{pointer} should point to proof_commands array")
        });
    assert!(
        !commands.is_empty(),
        "{pointer} should include proof commands for the saved artifact fixture"
    );

    for command in commands {
        let command = command.as_str().unwrap_or_else(|| {
            panic!("{pointer} proof command should be a string")
        });
        assert!(
            command.starts_with("cargo-allow "),
            "{pointer} proof command should stay within cargo-allow: {command}"
        );
        for token in command.split_ascii_whitespace() {
            assert!(
                !FORBIDDEN_PROOF_COMMAND_TOOL_TOKENS
                    .iter()
                    .any(|forbidden| forbidden_tool_token_matches(token, forbidden)),
                "{pointer} proof command should not invoke adjacent build/evidence tooling: {command}"
            );
        }
    }
}

fn forbidden_tool_token_matches(token: &str, forbidden: &str) -> bool {
    token == forbidden || token.strip_suffix(".exe") == Some(forbidden)
}

pub(crate) fn path_arg(path: &Path) -> &str {
    path.to_str()
        .unwrap_or_else(|| panic!("non-UTF-8 path: {}", path.display()))
}
