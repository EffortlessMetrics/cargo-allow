//! Process-level conformance for the discovery-only provider descriptor
//! (#2567) and the published sensor-capabilities self-description schema
//! (#4627).

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

fn sensor_capabilities_validator() -> Result<jsonschema::Validator, Box<dyn std::error::Error>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/schemas/sensor-capabilities.schema.json");
    let schema: Value = serde_json::from_slice(&fs::read(path)?)?;
    Ok(jsonschema::validator_for(&schema)?)
}

fn catalog(root: &Path) -> Result<Value, Box<dyn std::error::Error>> {
    let output = command(root).args(["--format", "json"]).output()?;
    if !output.status.success() {
        return Err(format!(
            "sensor catalog failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}

/// Apply `mutate` to a copy of `instance` and require the schema to reject
/// the result.
fn expect_schema_rejection(
    validator: &jsonschema::Validator,
    instance: &Value,
    label: &str,
    mutate: impl FnOnce(&mut Value),
) -> Result<(), Box<dyn std::error::Error>> {
    let mut mutated = instance.clone();
    mutate(&mut mutated);
    if validator.is_valid(&mutated) {
        return Err(format!("schema accepted {label}").into());
    }
    Ok(())
}

#[test]
fn capabilities_with_absolute_default_policy_path_gets_the_init_remedy() -> TestResult {
    // #4558: an absolute spelling of the default policy (the CI `$(pwd)`
    // idiom) resolves to the same missing file as the relative default
    // spelling, so it must get the init remedy, not create-or-repoint.
    let fixture = Fixture::new("absolute-default-policy")?;
    let absolute = fixture.0.join("policy/allow.toml");
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
        .current_dir(&fixture.0)
        .arg("capabilities")
        .arg("--root")
        .arg(".")
        .arg("--config")
        .arg(&absolute)
        .arg("--format")
        .arg("json")
        .output()?;
    assert_eq!(
        output.status.code(),
        Some(1),
        "missing absolute default policy must exit 1: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("error[E0003_INVALID_POLICY]"),
        "the typed policy error must render: {stderr}"
    );
    assert!(
        stderr.contains(
            "Next: run `cargo-allow init` to create the starter policy, then rerun this command."
        ),
        "the absolute default spelling must get the init remedy: {stderr}"
    );
    assert!(
        !stderr.contains("create a cargo-allow policy at the reported path"),
        "the create-or-repoint remedy must not replace init for the default path: {stderr}"
    );
    Ok(())
}

#[test]
fn capabilities_with_missing_custom_policy_names_the_path_remedy() -> TestResult {
    // #4545 review: `init` only writes the default policy/allow.toml, so a
    // missing nondefault --config selection must get the create-or-repoint
    // remedy, not the init one.
    let fixture = Fixture::new("missing-custom-policy")?;
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
        .current_dir(&fixture.0)
        .args([
            "capabilities",
            "--root",
            ".",
            "--config",
            "custom/other.toml",
            "--format",
            "json",
        ])
        .output()?;
    assert_eq!(
        output.status.code(),
        Some(1),
        "missing custom policy must exit 1: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("error[E0003_INVALID_POLICY]"),
        "the typed policy error must render: {stderr}"
    );
    assert!(
        stderr.contains("create a cargo-allow policy at the reported path"),
        "the nondefault remedy must direct at the reported path: {stderr}"
    );
    assert!(
        !stderr.contains("run `cargo-allow init` to create the starter policy"),
        "init would not create the custom path, so it must not be suggested: {stderr}"
    );
    Ok(())
}

#[test]
fn capabilities_with_explicit_policy_before_adoption_fails_e0003() -> TestResult {
    // #4529 pins the documented prerequisite (docs/getting-started.md §2b):
    // the policy-scoped capabilities variant reads the policy ledger, which
    // §4's `init` creates. Before adoption it fails with E0003_INVALID_POLICY
    // (exit 1) — the remedy is to adopt first, not to repair a policy entry.
    let fixture = Fixture::new("pre-adoption-policy")?;
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
        .current_dir(&fixture.0)
        .args([
            "capabilities",
            "--root",
            ".",
            "--config",
            "policy/allow.toml",
            "--format",
            "json",
        ])
        .output()?;
    assert_eq!(
        output.status.code(),
        Some(1),
        "pre-adoption capabilities must exit 1: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("error[E0003_INVALID_POLICY]"),
        "the typed policy error must render: {stderr}"
    );
    // #4545: the Next line must send the operator to §4's init, not to
    // repairing a policy entry at a path that does not exist yet.
    assert!(
        stderr.contains(
            "Next: run `cargo-allow init` to create the starter policy, then rerun this command."
        ),
        "the pre-adoption remedy must be typed as init: {stderr}"
    );
    assert!(
        !stderr.contains("repair the policy entry"),
        "the generic policy-entry remedy must not misdirect pre-adoption recovery: {stderr}"
    );
    assert!(
        output.stdout.is_empty(),
        "no capabilities catalog may be produced before adoption"
    );
    Ok(())
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
        null.as_object_mut()
            .ok_or("descriptor must remain an object")?
            .insert(field.clone(), Value::Null);
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
            json!([
                "path_lookup",
                "compatibility_config",
                "explicit_environment"
            ]),
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
        invalid_instance
            .as_object_mut()
            .ok_or("descriptor must remain an object")?
            .insert(field.to_string(), invalid);
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

#[test]
fn sensor_capabilities_stdout_conforms_without_repository_or_policy() -> TestResult {
    // #4627: plain `capabilities --format json` must conform to the published
    // self-description schema in a bare directory, with no repository and no
    // policy — the catalog is discovery metadata, not a scan result.
    let fixture = Fixture::new("sensor-capabilities-stdout")?;
    let instance = catalog(&fixture.0)?;
    let validator = sensor_capabilities_validator()?;
    if !validator.is_valid(&instance) {
        let errors: Vec<_> = validator
            .iter_errors(&instance)
            .map(|error| error.to_string())
            .collect();
        return Err(format!("sensor capabilities catalog does not conform: {errors:?}").into());
    }
    if instance.get("schema").and_then(Value::as_str) != Some("cargo-allow.sensor-capabilities.v1")
        || instance.get("generation").and_then(Value::as_u64) != Some(1)
    {
        return Err("catalog identity changed under the published schema".into());
    }
    if instance
        .get("capabilities")
        .and_then(Value::as_array)
        .is_none_or(|rows| rows.is_empty())
        || instance
            .get("configured_file_families")
            .and_then(Value::as_array)
            .is_none_or(|rows| !rows.is_empty())
    {
        return Err("unfiltered catalog shape changed under the published schema".into());
    }
    Ok(())
}

#[test]
fn sensor_capabilities_schema_rejects_mutated_real_output() -> TestResult {
    let fixture = Fixture::new("sensor-capabilities-controls")?;
    let instance = catalog(&fixture.0)?;
    let validator = sensor_capabilities_validator()?;
    if !validator.is_valid(&instance) {
        return Err("negative controls require a valid producer baseline".into());
    }
    let fields = instance.as_object().ok_or("catalog must be an object")?;
    for field in fields.keys() {
        let label = format!("missing root field {field}");
        expect_schema_rejection(&validator, &instance, &label, |mutated| {
            mutated
                .as_object_mut()
                .expect("catalog must remain an object")
                .remove(field);
        })?;
        let label = format!("null root field {field}");
        expect_schema_rejection(&validator, &instance, &label, |mutated| {
            mutated
                .as_object_mut()
                .expect("catalog must remain an object")
                .insert(field.to_string(), Value::Null);
        })?;
    }
    for (field, invalid) in [
        ("schema", json!("cargo-allow.tool-identity.v1")),
        ("schema", json!("cargo-allow.sensor-capabilities.v2")),
        ("generation", json!(2)),
        ("generation", json!("1")),
        ("unexpected_field", json!(true)),
        ("schema_id", json!("cargo-allow.sensor-capabilities.v1")),
        ("inventory", json!([])),
    ] {
        let label = format!("invalid root {field}: {invalid}");
        expect_schema_rejection(&validator, &instance, &label, |mutated| {
            mutated
                .as_object_mut()
                .expect("catalog must remain an object")
                .insert(field.to_string(), invalid.clone());
        })?;
    }

    // Row-level controls: every one of the 18 pinned fields is required and
    // closed, and the typed vocabulary is enforced.
    let rows = instance
        .get("capabilities")
        .and_then(Value::as_array)
        .ok_or("capabilities must be an array")?;
    let row = rows.first().ok_or("unfiltered catalog must have rows")?;
    let row_fields = row.as_object().ok_or("capability row must be an object")?;
    if row_fields.len() != 18 {
        return Err(format!(
            "published schema pins 18 capability fields, row has {}",
            row_fields.len()
        )
        .into());
    }
    for field in row_fields.keys() {
        // kind/family are intentionally nullable on any row: not_included and
        // static-sensor rows carry null while finding rows carry strings. The
        // emission-side class invariant lives in the binary's catalog
        // validation, not in the published shape schema.
        if field == "kind" || field == "family" {
            continue;
        }
        let label = format!("missing capability field {field}");
        expect_schema_rejection(&validator, &instance, &label, |mutated| {
            mutated
                .get_mut("capabilities")
                .and_then(Value::as_array_mut)
                .and_then(|rows| rows.first_mut())
                .and_then(|row| row.as_object_mut())
                .expect("first capability row must remain an object")
                .remove(field);
        })?;
        let label = format!("null capability field {field}");
        expect_schema_rejection(&validator, &instance, &label, |mutated| {
            mutated
                .get_mut("capabilities")
                .and_then(Value::as_array_mut)
                .and_then(|rows| rows.first_mut())
                .and_then(|row| row.as_object_mut())
                .expect("first capability row must remain an object")
                .insert(field.to_string(), Value::Null);
        })?;
    }
    // Positive control for the intentional permissiveness above: a source row
    // with null kind/family still conforms (the shape does not encode the
    // per-class invariant).
    {
        let mut permissive = instance.clone();
        permissive
            .get_mut("capabilities")
            .and_then(Value::as_array_mut)
            .and_then(|rows| rows.first_mut())
            .and_then(|row| row.as_object_mut())
            .expect("first capability row must remain an object")
            .insert("kind".to_string(), Value::Null);
        permissive
            .get_mut("capabilities")
            .and_then(Value::as_array_mut)
            .and_then(|rows| rows.first_mut())
            .and_then(|row| row.as_object_mut())
            .expect("first capability row must remain an object")
            .insert("family".to_string(), Value::Null);
        if !validator.is_valid(&permissive) {
            return Err(
                "schema must keep accepting null kind/family for class-agnostic rows".into(),
            );
        }
    }
    for (field, invalid) in [
        ("generation", json!(2)),
        ("kind", json!(1)),
        ("family", json!(["unwrap"])),
        ("analysis_class", json!("runtime_reachability")),
        ("support_tier", json!("")),
        ("sensor_id", json!("")),
        ("limitations", json!([])),
        ("docs", json!([""])),
        (
            "claims_supported",
            json!(["recognized syntax construct exists in source text", ""]),
        ),
        ("unexpected_field", json!(true)),
    ] {
        let label = format!("invalid capability field {field}: {invalid}");
        expect_schema_rejection(&validator, &instance, &label, |mutated| {
            mutated
                .get_mut("capabilities")
                .and_then(Value::as_array_mut)
                .and_then(|rows| rows.first_mut())
                .and_then(|row| row.as_object_mut())
                .expect("first capability row must remain an object")
                .insert(field.to_string(), invalid.clone());
        })?;
    }

    // Configured family rows are closed too.
    let label = "invalid configured_file_families row";
    expect_schema_rejection(&validator, &instance, label, |mutated| {
        mutated
            .as_object_mut()
            .expect("catalog must remain an object")
            .insert(
                "configured_file_families".to_string(),
                json!([{
                    "rule_id": "r",
                    "family": "f",
                    "glob": "g/**",
                    "analysis_class": "supported_syntax",
                    "support_tier": "configured",
                    "claim_boundary": "boundary",
                }]),
            );
    })?;
    let label = "unknown-field configured_file_families row";
    expect_schema_rejection(&validator, &instance, label, |mutated| {
        mutated
            .as_object_mut()
            .expect("catalog must remain an object")
            .insert(
                "configured_file_families".to_string(),
                json!([{
                    "rule_id": "r",
                    "family": "f",
                    "glob": "g/**",
                    "analysis_class": "supported_presence",
                    "support_tier": "configured",
                    "claim_boundary": "boundary",
                    "inventory": [],
                }]),
            );
    })?;
    Ok(())
}
