use super::*;
use crate::artifact_contract_support::parse_json_artifact;
use crate::{CargoAllowCli, CargoAllowCommand, RootArgs};
use clap::Parser;
use serde_json::Value;
use std::path::Path;

fn argv(items: Vec<&str>) -> Vec<String> {
    items.into_iter().map(String::from).collect()
}

#[test]
fn clap_parses_doctor_json_output() {
    let parsed = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "doctor",
        "--root",
        ".",
        "--config",
        "policy/custom.toml",
        "--format",
        "json",
        "--output",
        "target/doctor.json",
    ]))
    .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse: {err}")));

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::Doctor(DoctorArgs {
            root: RootArgs { root: Some(root) },
            config: Some(config),
            format: DoctorFormat::Json,
            output: Some(output),
        })) if root == Path::new(".")
            && config == Path::new("policy/custom.toml")
            && output == Path::new("target/doctor.json")
    ));
}

#[test]
fn render_doctor_json_records_setup_context() {
    let json = allow_report::render_doctor_json(allow_report::DoctorReport {
        source_tree_root: "H:/Code/Rust/cargo-allow",
        root_discovery: "nearest_git_root",
        config_path: Some("H:/Code/Rust/cargo-allow/policy/allow.toml"),
        inventory_source: "git_tracked",
        files_scanned: 50,
    });
    let value = parse_json_artifact("doctor", &json, allow_report::DOCTOR_SCHEMA_ID, "doctor");

    assert_eq!(
        value.pointer("/root/path").and_then(Value::as_str),
        Some("H:/Code/Rust/cargo-allow")
    );
    assert_eq!(
        value.pointer("/root/discovery").and_then(Value::as_str),
        Some("nearest_git_root")
    );
    assert_eq!(
        value.pointer("/config/found").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        value.pointer("/config/path").and_then(Value::as_str),
        Some("H:/Code/Rust/cargo-allow/policy/allow.toml")
    );
    assert_eq!(
        value.pointer("/inventory/scope").and_then(Value::as_str),
        Some("source_tree")
    );
    assert_eq!(
        value.pointer("/inventory/scanner").and_then(Value::as_str),
        Some("source_syntax")
    );
    assert_eq!(
        value.pointer("/inventory/source").and_then(Value::as_str),
        Some("git_tracked")
    );
    assert_eq!(
        value
            .pointer("/inventory/files_scanned")
            .and_then(Value::as_u64),
        Some(50)
    );
}
