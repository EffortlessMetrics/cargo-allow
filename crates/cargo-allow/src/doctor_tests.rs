use super::*;
use crate::artifact_contract_support::parse_json_artifact;
use crate::{CargoAllowCli, CargoAllowCommand, RootArgs};
use clap::Parser;
use serde_json::Value;
use std::fs;
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
        config_schema_version: Some("0.1"),
        config_valid: Some(true),
        config_diagnostic: None,
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
        value
            .pointer("/config/schema_version")
            .and_then(Value::as_str),
        Some("0.1")
    );
    assert_eq!(
        value.pointer("/config/valid").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(value.pointer("/config/diagnostic"), Some(&Value::Null));
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
        value.pointer("/inventory/root").and_then(Value::as_str),
        Some("H:/Code/Rust/cargo-allow")
    );
    assert_eq!(
        value
            .pointer("/inventory/files_scanned")
            .and_then(Value::as_u64),
        Some(50)
    );
}

#[test]
fn doctor_config_status_reports_invalid_policy_without_failing() {
    let root = doctor_fixture_dir();
    let policy = root.join("allow.toml");
    fs::write(
        &policy,
        r#"
schema_version = ""
policy = "cargo-allow"
owner = "core/policy"
status = "active"
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write invalid policy: {err}")));

    let policy = load_doctor_policy(Some(&policy));
    let (valid, diagnostic) = config_status(&root, policy.as_ref());

    assert_eq!(valid, Some(false));
    assert!(
        diagnostic
            .is_some_and(|message| message.contains("policy schema_version must not be empty"))
    );
    remove_doctor_fixture_dir(root);
}

#[test]
fn doctor_inventory_respects_policy_ignored_globs() {
    let root = doctor_fixture_dir();
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::create_dir_all(root.join("ignored"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create ignored dir: {err}")));
    fs::write(root.join("kept.rs"), "fn kept() {}\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write kept source: {err}")));
    fs::write(root.join("ignored/skipped.rs"), "fn skipped() {}\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write ignored source: {err}")));
    let policy = root.join("policy/allow.toml");
    fs::write(
        &policy,
        r#"
policy = "cargo-allow"

[workspace]
ignored = ["policy/**", "ignored/**"]
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
    let output = root.join("doctor.json");

    cmd_doctor(&DoctorArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: Some(policy),
        format: DoctorFormat::Json,
        output: Some(output.clone()),
    })
    .unwrap_or_else(|err| std::panic::panic_any(format!("doctor should pass: {err}")));

    let json = fs::read_to_string(&output)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read doctor output: {err}")));
    let value = parse_json_artifact("doctor", &json, allow_report::DOCTOR_SCHEMA_ID, "doctor");
    assert_eq!(
        value
            .pointer("/inventory/files_scanned")
            .and_then(Value::as_u64),
        Some(1),
        "doctor should use policy ignored globs for source-tree inventory"
    );
    assert_eq!(
        value.pointer("/config/valid").and_then(Value::as_bool),
        Some(true),
        "policy should remain valid"
    );
    remove_doctor_fixture_dir(root);
}

fn doctor_fixture_dir() -> std::path::PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let dir =
        std::env::temp_dir().join(format!("cargo-allow-doctor-{}-{stamp}", std::process::id()));
    remove_doctor_fixture_dir(dir.clone());
    fs::create_dir_all(&dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("create doctor fixture: {err}")));
    dir
}

fn remove_doctor_fixture_dir(path: std::path::PathBuf) {
    match fs::remove_dir_all(&path) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => {
            std::panic::panic_any(format!("remove doctor fixture {}: {err}", path.display()))
        }
    }
}
