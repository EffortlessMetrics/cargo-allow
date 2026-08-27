use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

use serde_json::Value;

pub fn cargo_allow_command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
}

pub fn assert_status(command: &str, result: &Output, should_succeed: bool) {
    assert_eq!(
        result.status.success(),
        should_succeed,
        "{command} status mismatch: stdout=`{}` stderr=`{}`",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

pub fn assert_stdout_empty(command: &str, result: &Output, message: &str) {
    assert!(
        result.stdout.is_empty(),
        "{command} {message}: `{}`",
        String::from_utf8_lossy(&result.stdout)
    );
}

pub fn assert_stderr_empty(command: &str, result: &Output, message: &str) {
    assert!(
        result.stderr.is_empty(),
        "{command} {message}: `{}`",
        String::from_utf8_lossy(&result.stderr)
    );
}

pub fn assert_saved_json_artifact(
    path: &std::path::Path,
    name: &str,
    expected_schema_id: &str,
    expected_command: &str,
) -> Value {
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    let value: Value = serde_json::from_str(&contents).unwrap_or_else(|err| {
        panic!(
            "{name} saved artifact should parse as JSON: {err}\n{contents}"
        )
    });
    assert_eq!(
        value.get("schema_version").and_then(Value::as_u64),
        Some(1),
        "{name} schema_version"
    );
    assert_eq!(
        value.get("schema_id").and_then(Value::as_str),
        Some(expected_schema_id),
        "{name} schema_id"
    );
    assert_eq!(
        value.get("tool").and_then(Value::as_str),
        Some("cargo-allow"),
        "{name} tool"
    );
    assert_eq!(
        value.get("command").and_then(Value::as_str),
        Some(expected_command),
        "{name} command"
    );
    assert_json_string_array_eq(
        &value,
        "claim_boundary",
        allow_report::claim_boundary_for_schema_id(expected_schema_id),
        name,
    );
    assert_json_string_array_eq(
        &value,
        "scanner_limitations",
        allow_report::scanner_limitations_for_schema_id(expected_schema_id),
        name,
    );
    assert_eq!(
        value.pointer("/inventory/scope").and_then(Value::as_str),
        Some("source_tree"),
        "{name} inventory scope"
    );
    assert_eq!(
        value.pointer("/inventory/scanner").and_then(Value::as_str),
        Some(expected_inventory_scanner(name, expected_schema_id)),
        "{name} inventory scanner"
    );
    assert!(
        value
            .pointer("/inventory/source")
            .and_then(Value::as_str)
            .is_some_and(|source| !source.is_empty()),
        "{name} inventory source"
    );
    value
}

fn expected_inventory_scanner(name: &str, expected_schema_id: &str) -> &'static str {
    allow_report::artifact_contract_for_schema_id(expected_schema_id)
        .map(|contract| contract.inventory_scanner)
        .unwrap_or_else(|| {
            panic!(
                "{name} expected schema_id {expected_schema_id} has no registered inventory scanner"
            )
        })
}

fn assert_json_string_array_eq(value: &Value, field: &str, expected: &[&str], artifact: &str) {
    let Some(items) = value.get(field).and_then(Value::as_array) else {
        panic!("{artifact} {field} should be an array");
    };
    let actual = items
        .iter()
        .map(|item| {
            item.as_str().unwrap_or_else(|| {
                panic!("{artifact} {field} entries should be strings")
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected, "{artifact} {field}");
}

pub fn temp_root(label: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_else(|err| panic!("system clock: {err}"))
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&root)
        .unwrap_or_else(|err| panic!("create temp root: {err}"));
    root
}

pub fn remove_temp_root(root: PathBuf) {
    match fs::remove_dir_all(&root) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => panic!("remove temp root {}: {err}", root.display()),
    }
}
