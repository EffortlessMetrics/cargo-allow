use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

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
    assert_artifact(&audit, allow_report::REPORT_SCHEMA_ID, "audit");

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
    assert_artifact(&check, allow_report::REPORT_SCHEMA_ID, "check");
    assert_artifact(&receipt, allow_report::RECEIPT_SCHEMA_ID, "check");

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
    assert_artifact(&list, allow_report::LIST_SCHEMA_ID, "list");

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
    assert_artifact(&worklist, allow_report::WORKLIST_SCHEMA_ID, "worklist");

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
    assert_artifact(&doctor, allow_report::DOCTOR_SCHEMA_ID, "doctor");
}

fn run_cargo_allow(args: &[&str]) -> Output {
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
        .args(args)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow: {err}")));
    assert!(
        output.status.success(),
        "cargo-allow {:?} should pass\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
        args,
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "cargo-allow {:?} should not write stdout when --output is set:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        output.stderr.is_empty(),
        "cargo-allow {:?} should not write stderr when --output is set:\n{}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn assert_artifact(path: &Path, expected_schema_id: &str, expected_command: &str) -> Value {
    let json = fs::read_to_string(path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read {}: {err}", path.display())));
    let value: Value = serde_json::from_str(&json).unwrap_or_else(|err| {
        std::panic::panic_any(format!("parse {} as JSON: {err}\n{json}", path.display()))
    });
    assert_eq!(
        value.get("schema_version").and_then(Value::as_u64),
        Some(1),
        "{} schema_version",
        path.display()
    );
    assert_eq!(
        value.get("schema_id").and_then(Value::as_str),
        Some(expected_schema_id),
        "{} schema_id",
        path.display()
    );
    assert_eq!(
        value.get("command").and_then(Value::as_str),
        Some(expected_command),
        "{} command",
        path.display()
    );
    assert_json_array_contains(&value, "claim_boundary", "source_tree_inventory", path);
    assert_json_array_contains(
        &value,
        "scanner_limitations",
        "cargo_metadata_not_invoked",
        path,
    );
    assert_json_array_contains(
        &value,
        "scanner_limitations",
        "repository_code_not_executed",
        path,
    );
    assert_eq!(
        value.pointer("/inventory/scope").and_then(Value::as_str),
        Some(allow_report::INVENTORY_SCOPE_SOURCE_TREE),
        "{} inventory scope",
        path.display()
    );
    assert_eq!(
        value.pointer("/inventory/scanner").and_then(Value::as_str),
        Some(allow_report::INVENTORY_SCANNER_SOURCE_SYNTAX),
        "{} inventory scanner",
        path.display()
    );
    assert_eq!(
        value.pointer("/inventory/source").and_then(Value::as_str),
        Some("filesystem_fallback"),
        "{} inventory source",
        path.display()
    );
    value
}

fn assert_json_array_contains(value: &Value, field: &str, expected: &str, path: &Path) {
    let Some(items) = value.get(field).and_then(Value::as_array) else {
        std::panic::panic_any(format!("{} {field} should be an array", path.display()));
    };
    assert!(
        items.iter().any(|item| item.as_str() == Some(expected)),
        "{} {field} should contain {expected}",
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
        let id = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!(
            "cargo-allow-{prefix}-{}-{stamp}-{id}",
            std::process::id()
        ));
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
}

impl Drop for SourceTreeFixture {
    fn drop(&mut self) {
        match fs::remove_dir_all(&self.root) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                std::panic::panic_any(format!("remove fixture {}: {err}", self.root.display()))
            }
        }
    }
}
