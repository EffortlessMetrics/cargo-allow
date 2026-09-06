//! Process-level conformance for the discovery-only provider descriptor (#2567).

use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

type TestResult = Result<(), Box<dyn std::error::Error>>;

struct Fixture(PathBuf);

impl Fixture {
    fn new(label: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let path = std::env::temp_dir().join(format!(
            "cargo-allow-provider-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn command(root: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-allow"));
    command.current_dir(root).arg("capabilities");
    command
}

fn descriptor(root: &Path) -> Result<Value, Box<dyn std::error::Error>> {
    let output = command(root)
        .args(["--provider-contract", "--format", "json"])
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "descriptor failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    // from_slice rejects mixed human/machine output and a second JSON value.
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn validator() -> Result<jsonschema::Validator, Box<dyn std::error::Error>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/schemas/cargo-allow-provider-contract-v1.schema.json");
    let schema: Value = serde_json::from_slice(&fs::read(path)?)?;
    Ok(jsonschema::validator_for(&schema)?)
}

#[test]
fn provider_descriptor_stdout_conforms_without_repository_or_policy() -> TestResult {
    let fixture = Fixture::new("stdout")?;
    // No Git repository exists here; malformed ambient policy must not be read.
    fs::create_dir(fixture.0.join("policy"))?;
    let policy = fixture.0.join("policy/allow.toml");
    fs::write(&policy, "not valid TOML [[[")?;
    let instance = descriptor(&fixture.0)?;
    let validator = validator()?;
    if !validator.is_valid(&instance) {
        let errors: Vec<_> = validator
            .iter_errors(&instance)
            .map(|error| error.to_string())
            .collect();
        return Err(format!("descriptor does not conform: {errors:?}").into());
    }
    if fs::read_to_string(policy)? != "not valid TOML [[[" {
        return Err("descriptor changed the ambient policy".into());
    }
    Ok(())
}

#[test]
fn provider_descriptor_schema_rejects_mutated_real_output() -> TestResult {
    let fixture = Fixture::new("schema-controls")?;
    let instance = descriptor(&fixture.0)?;
    let validator = validator()?;
    if !validator.is_valid(&instance) {
        return Err("negative controls require a valid producer baseline".into());
    }
    let fields = instance.as_object().ok_or("descriptor must be an object")?;
    for field in fields.keys() {
        let mut missing = instance.clone();
        missing
            .as_object_mut()
            .ok_or("descriptor must remain an object")?
            .remove(field);
        if validator.is_valid(&missing) {
            return Err(format!("schema accepted missing field {field}").into());
        }
        let mut null = instance.clone();
        null[field] = Value::Null;
        if validator.is_valid(&null) {
            return Err(format!("schema accepted null field {field}").into());
        }
    }
    for (field, invalid) in [
        ("schema_id", json!("cargo-allow.sensor-capabilities.v1")),
        ("schema_version", json!(2)),
        ("schema_version", json!("1")),
        ("provider_id", json!("proof.other.v1")),
        ("product_name", json!("cargo-proof")),
        ("access_posture", json!("read_write")),
        ("snapshot_bound", json!(false)),
        ("environment_variable", json!("OTHER_BIN")),
        ("config_relative_path", json!("../policy.toml")),
        ("discovery_order", json!([])),
        (
            "discovery_order",
            json!(["path_lookup", "compatibility_config", "explicit_environment"]),
        ),
        ("forbidden_path_prefixes", json!(["target/"])),
        ("forbidden_path_prefixes", json!(["target/", "target/"])),
        ("forbidden_path_prefixes", json!(["target/", "other/"])),
        ("required_capabilities", json!([])),
        ("required_capabilities", json!(["cargo-allow.check.no-new"])),
        (
            "required_capabilities",
            json!(["cargo-allow.check.no-new", "cargo-allow.check.no-new"]),
        ),
        (
            "required_capabilities",
            json!(["cargo-allow.check.no-new", "unknown.capability"]),
        ),
        ("unexpected_field", json!(true)),
        ("request_schema", json!("cargo-allow.analysis-request.v1")),
        ("receipt_schema", json!("repo.analysis-receipt.v1")),
    ] {
        let mut invalid_instance = instance.clone();
        invalid_instance[field] = invalid;
        if validator.is_valid(&invalid_instance) {
            return Err(format!("schema accepted invalid {field}: {invalid_instance}").into());
        }
    }
    Ok(())
}

#[test]
fn provider_descriptor_rejects_each_catalog_option_without_writing() -> TestResult {
    let fixture = Fixture::new("option-controls")?;
    let output_path = fixture.0.join("descriptor.json");
    fs::write(&output_path, "existing output")?;
    for (option, value) in [
        ("--root", "."),
        ("--config", "missing-policy.toml"),
        ("--class", "supported-syntax"),
        ("--kind", "panic"),
        ("--family", "unwrap"),
    ] {
        let output = command(&fixture.0)
            .args(["--provider-contract", "--format", "json", option, value])
            .arg("--output")
            .arg(&output_path)
            .output()?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        if output.status.success()
            || !output.stdout.is_empty()
            || !stderr.contains("--provider-contract cannot be combined")
            || fs::read_to_string(&output_path)? != "existing output"
        {
            return Err(format!("{option} did not reject without writing: {output:?}").into());
        }
    }
    Ok(())
}

#[test]
fn provider_descriptor_does_not_replace_the_default_sensor_catalog() -> TestResult {
    let fixture = Fixture::new("catalog")?;
    let output = command(&fixture.0).args(["--format", "json"]).output()?;
    if !output.status.success() {
        return Err(format!("sensor catalog failed: {output:?}").into());
    }
    let catalog: Value = serde_json::from_slice(&output.stdout)?;
    let schema_id = catalog.get("schema").and_then(Value::as_str);
    if schema_id != Some("cargo-allow.sensor-capabilities.v1")
        || catalog.get("provider_contract").is_some()
        || validator()?.is_valid(&catalog)
    {
        return Err("default sensor catalog and provider descriptor were conflated".into());
    }
    Ok(())
}
