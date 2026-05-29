mod support;

use std::fs;
use std::path::{Path, PathBuf};

use support::{
    assert_saved_json_artifact, assert_status, assert_stderr_empty, assert_stdout_empty,
    cargo_allow_command, remove_temp_root, temp_root,
};

#[test]
fn saved_json_outputs_keep_source_tree_contracts() {
    let fixture = SourceTreeFixture::new("saved-json-contracts");
    fixture.write_minimal_policy();

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let audit = artifact_dir.join("audit.json");
    let check = artifact_dir.join("check.json");
    let receipt = artifact_dir.join("check.receipt.json");
    let list = artifact_dir.join("list.json");
    let worklist = artifact_dir.join("worklist.json");
    let doctor = artifact_dir.join("doctor.json");

    run_cargo_allow(&[
        "audit",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--format",
        "json",
        "--output",
        path_arg(&audit),
    ]);
    assert_source_syntax_artifact(&audit, allow_report::REPORT_SCHEMA_ID, "audit");

    run_cargo_allow(&[
        "check",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--mode",
        "no-new",
        "--format",
        "json",
        "--output",
        path_arg(&check),
        "--receipt",
        path_arg(&receipt),
    ]);
    assert_source_syntax_artifact(&check, allow_report::REPORT_SCHEMA_ID, "check");
    assert_source_syntax_artifact(&receipt, allow_report::RECEIPT_SCHEMA_ID, "check");

    run_cargo_allow(&[
        "list",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--format",
        "json",
        "--output",
        path_arg(&list),
    ]);
    assert_source_syntax_artifact(&list, allow_report::LIST_SCHEMA_ID, "list");

    run_cargo_allow(&[
        "worklist",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--format",
        "json",
        "--output",
        path_arg(&worklist),
    ]);
    assert_source_syntax_artifact(&worklist, allow_report::WORKLIST_SCHEMA_ID, "worklist");

    run_cargo_allow(&[
        "doctor",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--format",
        "json",
        "--output",
        path_arg(&doctor),
    ]);
    assert_source_syntax_artifact(&doctor, allow_report::DOCTOR_SCHEMA_ID, "doctor");
}

#[test]
fn saved_summary_outputs_keep_policy_and_summary_streams_separate() {
    let fixture = SourceTreeFixture::new("saved-summary-contracts");
    fixture.write_minimal_policy();
    fixture.write_panic_source();

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let add_policy = artifact_dir.join("allow.add.toml");
    let add_summary = artifact_dir.join("add-summary.json");
    let propose_policy = artifact_dir.join("allow.proposed.toml");
    let propose_summary = artifact_dir.join("propose-summary.json");
    let migrate_policy = artifact_dir.join("allow.migrated.toml");
    let migrate_summary = artifact_dir.join("migrate-summary.json");

    run_cargo_allow(&[
        "add",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--kind",
        "panic",
        "--path",
        "src/lib.rs",
        "--line",
        "1",
        "--owner",
        "core/tests",
        "--reason",
        "Fixture exercises saved add summary output.",
        "--evidence",
        "test:saved_summary_outputs_keep_policy_and_summary_streams_separate",
        "--write",
        path_arg(&add_policy),
        "--summary-format",
        "json",
        "--summary-output",
        path_arg(&add_summary),
    ]);
    assert_policy_output(&add_policy);
    assert_source_syntax_artifact(&add_summary, allow_report::ADD_SCHEMA_ID, "add");

    run_cargo_allow(&[
        "propose",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--write",
        path_arg(&propose_policy),
        "--summary-format",
        "json",
        "--summary-output",
        path_arg(&propose_summary),
    ]);
    assert_policy_output(&propose_policy);
    assert_source_syntax_artifact(&propose_summary, allow_report::PROPOSE_SCHEMA_ID, "propose");

    run_cargo_allow(&[
        "migrate",
        "--from",
        path_arg(&fixture.root.join("policy/allow.toml")),
        "--out",
        path_arg(&migrate_policy),
        "--summary-format",
        "json",
        "--summary-output",
        path_arg(&migrate_summary),
    ]);
    assert_policy_output(&migrate_policy);
    assert_policy_migration_artifact(&migrate_summary, allow_report::MIGRATE_SCHEMA_ID, "migrate");
}

fn run_cargo_allow(args: &[&str]) -> Output {
    let output = cargo_allow_command()
        .args(args)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow: {err}")));
    let command = format!("cargo-allow {}", args.join(" "));
    assert_status(&command, &output, true);
    assert_stdout_empty(
        &command,
        &output,
        "should not write stdout when output files are set",
    );
    assert_stderr_empty(
        &command,
        &output,
        "should not write stderr when output files are set",
    );
    output
}

fn assert_source_syntax_artifact(path: &Path, expected_schema_id: &str, expected_command: &str) {
    let value =
        assert_saved_json_artifact(path, expected_command, expected_schema_id, expected_command);
    assert_inventory(
        &value,
        allow_report::INVENTORY_SCANNER_SOURCE_SYNTAX,
        "filesystem_fallback",
    );
}

fn assert_policy_migration_artifact(path: &Path, expected_schema_id: &str, expected_command: &str) {
    let value =
        assert_saved_json_artifact(path, expected_command, expected_schema_id, expected_command);
    assert_inventory(
        &value,
        allow_report::INVENTORY_SCANNER_POLICY_MIGRATION,
        allow_report::INVENTORY_SOURCE_UNKNOWN,
    );
}

type Output = std::process::Output;

fn assert_inventory(value: &serde_json::Value, expected_scanner: &str, expected_source: &str) {
    assert_eq!(
        value
            .pointer("/inventory/scanner")
            .and_then(serde_json::Value::as_str),
        Some(expected_scanner),
        "inventory scanner"
    );
    assert_eq!(
        value
            .pointer("/inventory/source")
            .and_then(serde_json::Value::as_str),
        Some(expected_source),
        "inventory source"
    );
}

fn assert_policy_output(path: &Path) {
    let text = fs::read_to_string(path).unwrap_or_else(|err| {
        std::panic::panic_any(format!("read policy output {}: {err}", path.display()))
    });
    assert!(
        text.contains("schema_version = \"0.1\""),
        "{} should be policy TOML",
        path.display()
    );
    assert!(
        !text.contains("\"schema_id\""),
        "{} should not contain summary JSON",
        path.display()
    );
}

fn path_arg(path: &Path) -> &str {
    path.to_str()
        .unwrap_or_else(|| std::panic::panic_any(format!("non-UTF-8 path: {}", path.display())))
}

struct SourceTreeFixture {
    root: PathBuf,
    root_arg: String,
}

impl SourceTreeFixture {
    fn new(prefix: &str) -> Self {
        let root = temp_root(prefix);
        fs::create_dir_all(root.join("policy"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("create fixture: {err}")));
        let root_arg = root
            .to_str()
            .unwrap_or_else(|| {
                std::panic::panic_any(format!("non-UTF-8 fixture path: {}", root.display()))
            })
            .to_string();
        Self { root, root_arg }
    }

    fn root_str(&self) -> &str {
        &self.root_arg
    }

    fn write_minimal_policy(&self) {
        fs::write(
            self.root.join("policy/allow.toml"),
            r#"schema_version = "0.1"
policy = "cargo-allow"
owner = "core/policy"
status = "active"

[workspace]
root = "."
inventory = "git-tracked"
default_mode = "no-new"
ignored = ["policy/**", "target/**"]
generated = ["target/**", "vendor/**"]

[requirements]
owner_required = true
reason_required = true
classification_required = true
evidence_required = false
expires_or_review_after_required = true
allow_bare_allow_attributes = false
lint_policy_id_required = false
stale_entries_fail = false

[requirements.unsafe]
evidence_required = true
safety_comment_required = false
"#,
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
    }

    fn write_panic_source(&self) {
        fs::create_dir_all(self.root.join("src"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("create src dir: {err}")));
        fs::write(
            self.root.join("src/lib.rs"),
            "pub fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("write source fixture: {err}")));
    }
}

impl Drop for SourceTreeFixture {
    fn drop(&mut self) {
        remove_temp_root(self.root.clone());
    }
}
