use std::fs;
use std::path::PathBuf;
use std::process::Output;

use allow_core::CargoAllowErrorKind;
use serde_json::Value;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const VALID_POLICY: &str = "schema_version = \"0.1\"\npolicy = \"cargo-allow\"\n";
const MALFORMED_POLICY: &str = "schema_version = \"0.1\"\npolicy = \"cargo-allow\"\nowner = [";
const REPAIR_NEXT: &str = "Next: repair the policy TOML at the reported location and retry.";

#[test]
fn malformed_conventional_candidates_keep_their_precedence() -> TestResult {
    for (position, relative) in allow_policy::DISCOVERY_REL_PATHS.iter().enumerate() {
        let fixture = Fixture::new("malformed-conventional")?;
        fixture.write(relative, MALFORMED_POLICY)?;
        // A lower-priority valid ledger must not hide the selected parse error.
        for fallback in allow_policy::DISCOVERY_REL_PATHS.iter().skip(position + 1) {
            fixture.write(fallback, VALID_POLICY)?;
        }
        fixture.require_invalid_policy(relative, None, "check")?;
    }
    Ok(())
}

#[test]
fn malformed_nearest_policy_does_not_select_an_ancestor_ledger() -> TestResult {
    let ancestor = Fixture::new("malformed-nearest")?;
    ancestor.write("policy/allow.toml", VALID_POLICY)?;
    let root = ancestor.root.join("nested");
    fs::create_dir_all(&root)?;
    let nested = Fixture { root };
    nested.write("policy/allow.toml", MALFORMED_POLICY)?;

    nested.require_invalid_policy("policy/allow.toml", None, "check")
}

#[test]
fn malformed_metadata_policy_does_not_select_a_conventional_ledger() -> TestResult {
    for section in ["package", "workspace"] {
        let fixture = Fixture::new("malformed-metadata")?;
        fixture.write("policy/allow.toml", VALID_POLICY)?;
        fixture.write("policies/selected.toml", MALFORMED_POLICY)?;
        fixture.write_metadata(section, "policies/selected.toml")?;

        fixture.require_invalid_policy("policies/selected.toml", None, "check")?;
    }
    Ok(())
}

#[test]
fn malformed_explicit_and_federated_policies_keep_the_selected_identity() -> TestResult {
    for explicit in [true, false] {
        let fixture = Fixture::new("malformed-selected")?;
        fixture.write("policy/allow.toml", VALID_POLICY)?;
        fixture.write("policies/selected policy.toml", MALFORMED_POLICY)?;
        if !explicit {
            fixture.write_federation("policies/selected policy.toml")?;
        }
        fixture.require_invalid_policy(
            "policies/selected policy.toml",
            explicit.then_some("policies/selected policy.toml"),
            "check",
        )?;
    }
    Ok(())
}

#[test]
fn malformed_header_shapes_are_not_proven_foreign_dialects() -> TestResult {
    for text in [
        "schema_version = true\npolicy = \"cargo-allow\"\n",
        "schema_version = \"1\"\npolicy = \"other-policy\"\nowner = [",
    ] {
        let fixture = Fixture::new("malformed-header")?;
        fixture.write("policy/allow.toml", text)?;
        fixture.write(".cargo/allow.toml", VALID_POLICY)?;
        fixture.require_invalid_policy("policy/allow.toml", None, "check")?;
    }
    Ok(())
}

#[test]
fn optional_audit_policy_does_not_turn_a_parse_error_into_defaults() -> TestResult {
    let fixture = Fixture::new("malformed-optional")?;
    fixture.write("policy/allow.toml", MALFORMED_POLICY)?;
    fixture.require_invalid_policy("policy/allow.toml", None, "audit")
}

#[test]
fn valid_winners_ignore_lower_priority_malformed_candidates() -> TestResult {
    for source in ["cli", "federation", "package", "workspace", "native"] {
        let fixture = Fixture::new("valid-higher-priority")?;
        for relative in allow_policy::DISCOVERY_REL_PATHS {
            fixture.write(relative, MALFORMED_POLICY)?;
        }
        let selected = if source == "native" {
            "policy/cargo-allow.toml"
        } else {
            "policies/selected policy.toml"
        };
        fixture.write(selected, VALID_POLICY)?;
        match source {
            "cli" => fixture.write_federation("policy/allow.toml")?,
            "federation" => fixture.write_federation(selected)?,
            "package" => {
                fixture.write_metadata("package", selected)?;
                // Package metadata wins before the workspace metadata candidate.
                let manifest = fs::read_to_string(fixture.root.join("Cargo.toml"))?;
                fixture.write(
                    "Cargo.toml",
                    &format!(
                        "{manifest}\n[workspace.metadata.cargo-allow]\nconfig = \"policy/allow.toml\"\n"
                    ),
                )?;
            }
            "workspace" => fixture.write_metadata("workspace", selected)?,
            _ => {}
        }
        fixture.require_selected_policy(selected, (source == "cli").then_some(selected))?;
    }
    Ok(())
}

#[test]
fn valid_foreign_policies_still_skip_and_absent_policies_remain_missing() -> TestResult {
    for foreign in [
        "schema_version = \"1\"\nowner = \"foreign-owner\"\n",
        "schema_version = \"0.1\"\npolicy = \"other-policy\"\n",
        "schema_version = \"99\"\npolicy = \"other-policy\"\n",
    ] {
        let fixture = Fixture::new("valid-foreign-control")?;
        fixture.write("policy/allow.toml", foreign)?;
        fixture.require_missing_policy(true)?;
        fixture.write(".cargo/allow.toml", VALID_POLICY)?;
        fixture.require_selected_policy(".cargo/allow.toml", None)?;
        require(
            fs::read_to_string(fixture.root.join("policy/allow.toml"))? == foreign,
            "foreign policy bytes changed",
        )?;
    }

    Fixture::new("absent-policy-control")?.require_missing_policy(false)
}

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(label: &str) -> TestResult<Self> {
        let root = super::temp_root(label).canonicalize()?;
        let fixture = Self { root };
        fixture.write("src/lib.rs", "pub fn value() -> u8 { 1 }\n")?;
        fixture.write("unrelated-canary.txt", "preserve this file\n")?;
        Ok(fixture)
    }

    fn write(&self, relative: &str, text: &str) -> TestResult {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, text)?;
        Ok(())
    }

    fn write_metadata(&self, section: &str, selected: &str) -> TestResult {
        self.write(
            "Cargo.toml",
            &format!("[{section}.metadata.cargo-allow]\nconfig = \"{selected}\"\n"),
        )
    }

    fn write_federation(&self, selected: &str) -> TestResult {
        self.write(
            ".allow/config.toml",
            &format!(
                "schema_version = \"1.0\"\n\n[[ledgers]]\nid = \"selected-source\"\npath = \"{selected}\"\ndialect = \"cargo-allow\"\nrole = \"canonical\"\nlanes = [\"source-exception\"]\nmode = \"blocking\"\npriority = 10\n"
            ),
        )
    }

    fn run(&self, operation: &str, explicit: Option<&str>) -> TestResult<Output> {
        let mut command = super::cargo_allow_command();
        // Keep fixture-owned CLI Git probes bound to this temporary root.
        for key in [
            "GIT_DIR",
            "GIT_WORK_TREE",
            "GIT_INDEX_FILE",
            "GIT_COMMON_DIR",
            "GIT_OBJECT_DIRECTORY",
            "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        ] {
            command.env_remove(key);
        }
        command
            .current_dir(&self.root)
            .arg(operation)
            .arg("--root")
            .arg(&self.root)
            .args(["--kind", "panic", "--format", "json"])
            .arg("--command-summary-output")
            .arg(self.root.join("target/command-summary.json"));
        if operation == "check" {
            command
                .args(["--mode", "no-new", "--receipt"])
                .arg(self.root.join("target/check.receipt.json"));
        }
        if let Some(explicit) = explicit {
            command.arg("--config").arg(explicit);
        }
        Ok(command.output()?)
    }

    fn require_invalid_policy(
        &self,
        selected: &str,
        explicit: Option<&str>,
        operation: &str,
    ) -> TestResult {
        let path = self.root.join(selected);
        let original = fs::read_to_string(&path)?;
        let error = allow_policy::parse_policy_at(&path, &original)
            .err()
            .ok_or("malformed control must fail the existing parser")?;
        let location = error.location().ok_or("parser location missing")?;
        require(
            error.kind() == CargoAllowErrorKind::InvalidPolicy
                && location.path.as_deref() == Some(path.to_string_lossy().as_ref())
                && location.line > 0
                && location.column > 0,
            format!("selected parser error lost its kind or located identity: {error:?}"),
        )?;
        if original == MALFORMED_POLICY {
            require(
                location.line == 3 && location.column == 10,
                format!("expected one-based EOF location 3:10, got {location:?}"),
            )?;
        }

        let output = self.run(operation, explicit)?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        require(
            output.status.code() == Some(1)
                && stderr.contains(&format!("error[E0003_INVALID_POLICY]: {}", error.message()))
                && stderr.contains(REPAIR_NEXT)
                && !stderr.contains("foreign-dialect")
                && !stderr.contains("no policy config found")
                && !stderr.contains("cargo-allow init"),
            format!("selected malformed policy was misreported: {output:?}"),
        )?;
        let summary = self.summary()?;
        require(
            summary.get("operation").and_then(Value::as_str) == Some(operation)
                && summary.get("result_class").and_then(Value::as_str) == Some("malformed_input")
                && summary.get("posture").and_then(Value::as_str) == Some("blocking")
                && summary.pointer("/reason/code").and_then(Value::as_str)
                    == Some("E0003_INVALID_POLICY")
                && summary.pointer("/reason/message").and_then(Value::as_str)
                    == Some(error.message()),
            format!("CLI and summary disagree about the selected policy: {summary}"),
        )?;
        if operation == "check" {
            let receipt = self.require_error_receipt()?;
            let path_text = allow_core::strip_win32_verbatim_prefix(&path.display().to_string());
            require(
                receipt.get("policy_config").and_then(Value::as_str) == Some(path_text.as_str())
                    && receipt.get("diagnostic").and_then(Value::as_str) == Some(error.message()),
                format!("error receipt lost the selected policy diagnostic: {receipt}"),
            )?;
        }
        require(
            fs::read_to_string(&path)? == original,
            "a failed parse changed the selected policy",
        )
    }

    fn require_selected_policy(&self, selected: &str, explicit: Option<&str>) -> TestResult {
        let path = self.root.join(selected);
        let original = fs::read(&path)?;
        let output = self.run("check", explicit)?;
        require(
            output.status.success(),
            format!("valid higher-priority policy failed: {output:?}"),
        )?;
        let receipt = self.receipt()?;
        require(
            receipt.get("policy_config").and_then(Value::as_str)
                == Some(allow_report::source_tree_path_text(&path).as_str())
                && receipt.get("policy_digest").and_then(Value::as_str)
                    == Some(allow_core::sha256_v1_bytes(&original).as_str()),
            format!("successful CLI selected a different policy identity or bytes: {receipt}"),
        )?;
        require(
            fs::read(&path)? == original
                && fs::read_to_string(self.root.join("unrelated-canary.txt"))?
                    == "preserve this file\n",
            "read-only selection changed policy or unrelated bytes",
        )
    }

    fn require_missing_policy(&self, foreign: bool) -> TestResult {
        let output = self.run("check", None)?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        require(
            output.status.code() == Some(1)
                && stderr.contains("error[E0002_INVALID_CONFIG]")
                && stderr.contains("cargo-allow init")
                && stderr.contains("skipped 1 foreign-dialect candidate") == foreign
                && !stderr.contains("E0003_INVALID_POLICY"),
            format!("valid foreign or absent policy changed classification: {output:?}"),
        )?;
        let summary = self.summary()?;
        require(
            summary.pointer("/reason/code").and_then(Value::as_str) == Some("E0002_INVALID_CONFIG"),
            format!("missing-policy summary changed classification: {summary}"),
        )?;
        let receipt = self.require_error_receipt()?;
        require(
            receipt.get("policy_config").is_none(),
            format!("an absent or skipped policy acquired a selected identity: {receipt}"),
        )
    }

    fn require_error_receipt(&self) -> TestResult<Value> {
        let receipt = self.receipt()?;
        require(
            receipt.get("status").and_then(Value::as_str) == Some("error")
                && receipt.get("failed").and_then(Value::as_bool) == Some(true)
                && receipt.get("policy_digest").is_none(),
            format!("failed parse produced a completed scan receipt: {receipt}"),
        )?;
        Ok(receipt)
    }

    fn receipt(&self) -> TestResult<Value> {
        Ok(serde_json::from_slice(&fs::read(
            self.root.join("target/check.receipt.json"),
        )?)?)
    }

    fn summary(&self) -> TestResult<Value> {
        Ok(serde_json::from_slice(&fs::read(
            self.root.join("target/command-summary.json"),
        )?)?)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn require(condition: bool, message: impl Into<String>) -> TestResult {
    if condition {
        Ok(())
    } else {
        Err(message.into().into())
    }
}
