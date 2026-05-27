use super::*;
use crate::{CargoAllowCli, CargoAllowCommand, RootArgs};
use clap::Parser;
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
    let json = render_doctor_json(DoctorFacts {
        source_tree_root: "H:/Code/Rust/cargo-allow",
        root_discovery: "nearest_git_root",
        config_path: Some("H:/Code/Rust/cargo-allow/policy/allow.toml"),
        inventory_source: "git_tracked",
        files_scanned: 50,
    });

    assert!(json.contains("\"schema_version\": 1"));
    assert!(json.contains(&format!(
        "\"schema_id\": \"{}\"",
        allow_report::DOCTOR_SCHEMA_ID
    )));
    assert!(json.contains("\"command\": \"doctor\""));
    assert!(json.contains("\"claim_boundary\""));
    assert!(json.contains("\"scanner_limitations\""));
    assert!(json.contains("\"path\": \"H:/Code/Rust/cargo-allow\""));
    assert!(json.contains("\"discovery\": \"nearest_git_root\""));
    assert!(json.contains("\"found\": true"));
    assert!(json.contains("policy/allow.toml"));
    assert!(json.contains("\"source\": \"git_tracked\""));
    assert!(json.contains("\"files_scanned\": 50"));
    assert!(json.contains("\"repository_code_not_executed\""));
}

#[test]
fn doctor_schema_documents_current_contract() {
    let schema = include_str!("../../../docs/schemas/doctor.schema.json");

    assert!(schema.contains(allow_report::DOCTOR_SCHEMA_ID));
    assert!(schema.contains("\"root\""));
    assert!(schema.contains("\"discovery\""));
    assert!(schema.contains("\"config\""));
    assert!(schema.contains("\"inventory\""));
    assert!(schema.contains("\"files_scanned\""));
    assert!(schema.contains("\"scanner_limitations\""));
    assert!(schema.contains("\"scanner_limitation\""));
    assert!(schema.contains("\"cargo_metadata_not_invoked\""));
    assert!(schema.contains("\"repository_code_not_executed\""));
}
