//! Integration test for the `cargo-allow tool` subcommand (#2795).
//!
//! The `tool` command had zero binary-level integration coverage. This test
//! exercises the `identity` subcommand end-to-end and asserts the rendered
//! output shape.

use serde_json::Value;
use std::process::Command;

fn cargo_allow_command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
}

#[test]
fn tool_identity_json_reports_schema_and_digest() {
    let output = cargo_allow_command()
        .arg("tool")
        .arg("identity")
        .arg("--format")
        .arg("json")
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("cargo-allow tool identity: {err}")));

    assert!(
        output.status.success(),
        "tool identity should succeed: stderr=`{}`",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|err| std::panic::panic_any(format!("tool identity JSON parses: {err}")));

    assert_eq!(
        json.pointer("/schema_id").and_then(Value::as_str),
        Some("cargo-allow.tool-identity.v1"),
        "schema_id should be cargo-allow.tool-identity.v1: {json}"
    );
    assert_eq!(
        json.pointer("/schema_version").and_then(Value::as_u64),
        Some(1),
        "schema_version should be 1: {json}"
    );
    assert_eq!(
        json.pointer("/reported_version").and_then(Value::as_str),
        Some(env!("CARGO_PKG_VERSION")),
        "reported_version should match CARGO_PKG_VERSION: {json}"
    );
    // The digest is the SHA-256 of the running binary, prefixed with "sha256:v1:".
    let digest = json
        .pointer("/executable_digest")
        .and_then(Value::as_str)
        .unwrap_or_else(|| {
            std::panic::panic_any(format!("executable_digest field missing: {json}"))
        });
    assert!(
        digest.starts_with("sha256:v1:"),
        "digest should be prefixed with sha256:v1:: {digest}"
    );
    // Capability generations should be present.
    assert_eq!(
        json.pointer("/command_api_generation")
            .and_then(Value::as_str),
        Some("cargo-allow.command-api.v1"),
        "command_api_generation: {json}"
    );
    let schemas = json
        .pointer("/supported_schema_generations")
        .and_then(Value::as_array)
        .unwrap_or_else(|| {
            std::panic::panic_any(format!("supported_schema_generations missing: {json}"))
        });
    assert!(
        !schemas.is_empty(),
        "supported_schema_generations should be non-empty: {json}"
    );
}

#[test]
fn tool_identity_human_is_non_empty() {
    let output = cargo_allow_command()
        .arg("tool")
        .arg("identity")
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("cargo-allow tool identity: {err}")));

    assert!(
        output.status.success(),
        "tool identity should succeed: stderr=`{}`",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("cargo-allow"),
        "human output should mention cargo-allow: {stdout}"
    );
    assert!(
        stdout.contains("cargo-allow.tool-identity.v1"),
        "human output should mention the schema_id: {stdout}"
    );
}
