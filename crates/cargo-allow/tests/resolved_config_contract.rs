use std::fs;
use std::path::{Path, PathBuf};

use allow_policy::resolve_cargo_allow_config_v1;
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
        instance["selected_policy"]["path"] = serde_json::Value::String(unsafe_path.to_string());
        ensure(
            !validator.is_valid(&instance),
            &format!("schema should reject non-portable path {unsafe_path}"),
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
        instance["source_subject"] = serde_json::Value::String(unsafe_subject.to_string());
        ensure(
            !validator.is_valid(&instance),
            &format!("schema should reject path-bearing subject {unsafe_subject}"),
        )?;
    }
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
