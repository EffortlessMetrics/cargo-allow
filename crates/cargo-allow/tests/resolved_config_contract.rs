use std::fs;
use std::path::{Path, PathBuf};

use allow_policy::{
    resolve_cargo_allow_config_v1, resolve_cargo_allow_config_v1_with_requested_root,
};
use allow_report::render_resolved_cargo_allow_config_json;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn produced_resolved_config_validates_against_the_published_component_schema() -> TestResult {
    let fixture = Fixture::new("schema-positive")?;
    fixture.write("policy/allow.toml", valid_policy())?;
    let resolved = resolve_cargo_allow_config_v1(fixture.path(), None, "subject:schema")?;
    let instance: serde_json::Value =
        serde_json::from_str(&render_resolved_cargo_allow_config_json(&resolved)?)?;
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/schemas/resolved-cargo-allow-config-v1.schema.json"
    ))?;
    let validator = jsonschema::validator_for(&schema)?;

    ensure(
        validator.is_valid(&instance),
        "producer output should validate against the component schema",
    )
}

#[test]
fn producer_distinguishes_requested_subdirectory_from_repository_root() -> TestResult {
    let fixture = Fixture::new("requested-root")?;
    fixture.write("policy/allow.toml", valid_policy())?;
    let requested = fixture.path().join("crates/example");
    fs::create_dir_all(&requested)?;
    let resolved = resolve_cargo_allow_config_v1_with_requested_root(
        &requested,
        fixture.path(),
        None,
        "subject:requested-root",
    )?;

    ensure(
        resolved.requested_root == "crates/example",
        "requested root should retain its repository-relative identity",
    )?;
    ensure(
        resolved.resolved_repository_root == ".",
        "resolved repository root should remain the portable anchor",
    )?;
    let instance: serde_json::Value =
        serde_json::from_str(&render_resolved_cargo_allow_config_json(&resolved)?)?;
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/schemas/resolved-cargo-allow-config-v1.schema.json"
    ))?;
    ensure(
        jsonschema::validator_for(&schema)?.is_valid(&instance),
        "requested-root projection should remain schema-valid",
    )?;
    Ok(())
}

#[test]
fn resolved_config_schema_rejects_non_portable_repository_paths() -> TestResult {
    let fixture = Fixture::new("schema-negative")?;
    fixture.write("policy/allow.toml", valid_policy())?;
    let resolved = resolve_cargo_allow_config_v1(fixture.path(), None, "subject:schema")?;
    let mut instance: serde_json::Value =
        serde_json::from_str(&render_resolved_cargo_allow_config_json(&resolved)?)?;
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/schemas/resolved-cargo-allow-config-v1.schema.json"
    ))?;
    let validator = jsonschema::validator_for(&schema)?;

    for unsafe_path in [
        "../outside.toml",
        "policy/../../outside.toml",
        "..\\outside.toml",
        "\\outside.toml",
        "C:outside.toml",
    ] {
        replace_string(&mut instance, "/selected_policy/path/path", unsafe_path)?;
        ensure(
            !validator.is_valid(&instance),
            &format!("schema should reject non-portable path {unsafe_path}"),
        )?;
    }
    Ok(())
}

#[test]
fn producer_keeps_foreign_style_cli_paths_schema_valid() -> TestResult {
    let fixture = Fixture::new("producer-path-matrix")?;
    fixture.write("policy/allow.toml", valid_policy())?;
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/schemas/resolved-cargo-allow-config-v1.schema.json"
    ))?;
    let validator = jsonschema::validator_for(&schema)?;

    for unsafe_path in [
        "../outside.toml",
        "policy/../../outside.toml",
        "..\\outside.toml",
        "policy\\..\\outside.toml",
        "\\outside.toml",
        "C:outside.toml",
    ] {
        let resolved = resolve_cargo_allow_config_v1(
            fixture.path(),
            Some(Path::new(unsafe_path)),
            "subject:producer-path-matrix",
        )?;
        let instance: serde_json::Value =
            serde_json::from_str(&render_resolved_cargo_allow_config_json(&resolved)?)?;
        ensure(
            validator.is_valid(&instance),
            &format!("producer output should remain schema-valid for CLI path {unsafe_path}"),
        )?;
    }
    Ok(())
}

#[test]
fn resolved_config_schema_rejects_path_bearing_source_subjects() -> TestResult {
    let fixture = Fixture::new("schema-subject-negative")?;
    fixture.write("policy/allow.toml", valid_policy())?;
    let resolved = resolve_cargo_allow_config_v1(fixture.path(), None, "subject:schema")?;
    let mut instance: serde_json::Value =
        serde_json::from_str(&render_resolved_cargo_allow_config_json(&resolved)?)?;
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/schemas/resolved-cargo-allow-config-v1.schema.json"
    ))?;
    let validator = jsonschema::validator_for(&schema)?;

    for unsafe_subject in ["subject:/home/private", "artifact:C:\\private\\x"] {
        replace_string(&mut instance, "/source_subject", unsafe_subject)?;
        ensure(
            !validator.is_valid(&instance),
            &format!("schema should reject path-bearing subject {unsafe_subject}"),
        )?;
    }
    Ok(())
}

#[test]
fn empty_federation_identity_fails_closed_in_schema_valid_projection() -> TestResult {
    let fixture = Fixture::new("empty-federation-id")?;
    fixture.write("policy/allow.toml", valid_policy())?;
    fixture.write("policy/invalid-registry-winner.toml", valid_policy())?;
    fixture.write(
        ".allow/config.toml",
        r#"[[ledgers]]
id = ""
path = "policy/invalid-registry-winner.toml"
dialect = "cargo-allow"
role = "canonical"
lanes = ["source-exception"]
priority = 10
"#,
    )?;
    let resolved = resolve_cargo_allow_config_v1(fixture.path(), None, "subject:empty-id")?;
    let instance: serde_json::Value =
        serde_json::from_str(&render_resolved_cargo_allow_config_json(&resolved)?)?;
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/schemas/resolved-cargo-allow-config-v1.schema.json"
    ))?;
    let validator = jsonschema::validator_for(&schema)?;

    ensure(
        resolved.status == allow_policy::ConfigResolutionStatusV1::Partial,
        "invalid registry plus conventional fallback should stay partial",
    )?;
    ensure(
        resolved.federation.configured_ledgers.is_empty(),
        "invalid empty identity should not enter configured ledgers",
    )?;
    ensure(
        !resolved.federation.selected_for_source_exception
            && resolved
                .selected_policy
                .as_ref()
                .is_some_and(|policy| policy.path.path == "policy/allow.toml"),
        "invalid empty identity must not influence the selected policy",
    )?;
    ensure(
        resolved.selection_source == Some(allow_policy::ConfigCandidateSourceV1::ConventionalPath)
            && resolved.precedence_tier
                == Some(allow_policy::ConfigPrecedenceTierV1::DiscoveryFallback)
            && resolved.fallback.considered
            && resolved.fallback.selected,
        "empty-id registry should produce an explicit conventional fallback posture",
    )?;
    ensure(
        validator.is_valid(&instance),
        "fail-closed empty-id projection should remain schema-valid",
    )
}

#[test]
fn resolved_config_schema_rejects_path_shaped_configured_ledger_ids() -> TestResult {
    let fixture = Fixture::new("schema-ledger-id-negative")?;
    fixture.write("policy/allow.toml", valid_policy())?;
    let resolved = resolve_cargo_allow_config_v1(fixture.path(), None, "subject:ledger-id")?;
    let mut instance: serde_json::Value =
        serde_json::from_str(&render_resolved_cargo_allow_config_json(&resolved)?)?;
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/schemas/resolved-cargo-allow-config-v1.schema.json"
    ))?;
    let validator = jsonschema::validator_for(&schema)?;

    for rejected in [
        "",
        " ",
        "team/source",
        r"team\source",
        "C:private",
        "équipe",
    ] {
        set_configured_ledger_id(&mut instance, rejected)?;
        ensure(
            !validator.is_valid(&instance),
            &format!("schema should reject non-portable ledger identity: {rejected}"),
        )?;
    }
    set_configured_ledger_id(&mut instance, &"a".repeat(1_025))?;
    ensure(
        !validator.is_valid(&instance),
        "schema should reject oversized ledger identity",
    )?;
    set_configured_ledger_id(&mut instance, "Team_1.release@owner+next")?;
    ensure(
        validator.is_valid(&instance),
        "schema should accept the producer's portable punctuation grammar",
    )
}

fn set_configured_ledger_id(instance: &mut serde_json::Value, value: &str) -> TestResult {
    let configured = instance
        .pointer_mut("/federation/configured_ledgers")
        .and_then(serde_json::Value::as_array_mut)
        .ok_or("configured_ledgers array missing from produced artifact")?;
    configured.clear();
    configured.push(serde_json::Value::String(value.to_string()));
    Ok(())
}

fn valid_policy() -> &'static str {
    r#"schema_version = "0.1"
policy = "cargo-allow"
owner = "repo-infra"
status = "active"

[workspace]
ignored = [".git/**", "target/**"]
generated = ["target/**"]
default_mode = "no-new"
"#
}

fn ensure(condition: bool, message: &str) -> TestResult {
    if condition {
        Ok(())
    } else {
        Err(message.to_string().into())
    }
}

fn replace_string(instance: &mut serde_json::Value, pointer: &str, value: &str) -> TestResult {
    let target = instance
        .pointer_mut(pointer)
        .ok_or_else(|| format!("test instance should contain {pointer}"))?;
    *target = serde_json::Value::String(value.to_string());
    Ok(())
}

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(label: &str) -> std::io::Result<Self> {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(std::io::Error::other)?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "cargo-allow-resolved-config-contract-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn write(&self, relative: &str, contents: &str) -> std::io::Result<()> {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, contents)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
