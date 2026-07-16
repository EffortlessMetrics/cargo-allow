//! Source-candidate installed-binary first-hour smoke (#2278 Stage A+).
//!
//! Installs `cargo-allow` into an isolated root via `cargo install --path`
//! (after optional packaging gate is left to `package-candidate-smoke.sh`),
//! runs the published-channel brownfield first-hour journey in a temporary
//! consumer repository outside this checkout, and emits
//! `cargo-allow.source-candidate-smoke-receipt.v1`.
//!
//! Does **not** prove ExactCandidatePackageSetV1 local-registry isolation
//! (#2277), published crates.io install, checkout denial, or the full
//! diff/refresh/prune lifecycle from the #2278 issue body.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const SCHEMA_ID: &str = "cargo-allow.source-candidate-smoke-receipt.v1";
const EXAMPLE_RECEIPT: &str =
    include_str!("../../../docs/dogfood/receipts/source-candidate-smoke-pass.example.json");
const SCHEMA_DOC: &str = include_str!(
    "../../../docs/dogfood/fixtures/release/source-candidate-smoke-receipt.v1.schema.json"
);

const STEPS_EXPECTED: &[&str] = &[
    "version",
    "doctor_no_policy",
    "audit_with_finding",
    "bootstrap_propose_write",
    "check_no_new_pass",
    "list_explain_worklist",
];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap_or_else(|| std::panic::panic_any("cargo-allow manifest should be under crates/"))
        .to_path_buf()
}

fn workspace_version(root: &Path) -> String {
    let manifest = fs::read_to_string(root.join("Cargo.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read workspace Cargo.toml: {err}")));
    let mut in_ws = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed == "[workspace.package]" {
            in_ws = true;
            continue;
        }
        if in_ws && trimmed.starts_with('[') {
            break;
        }
        if in_ws {
            if let Some(rest) = trimmed.strip_prefix("version = \"") {
                if let Some(end) = rest.find('"') {
                    if let Some(version) = rest.get(..end) {
                        return version.to_string();
                    }
                }
            }
        }
    }
    std::panic::panic_any("workspace.package.version not found")
}

fn temp_dir(label: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-source-candidate-smoke-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("create temp dir: {err}")));
    root
}

fn drop_dir(root: PathBuf) {
    match fs::remove_dir_all(&root) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => std::panic::panic_any(format!("remove {}: {err}", root.display())),
    }
}

fn run(output: Output, label: &str) -> Output {
    if !output.status.success() {
        std::panic::panic_any(format!(
            "{label} failed (exit {:?}); stderr=`{}`",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    output
}

fn write_source(root: &Path, body: &str) {
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create src: {err}")));
    fs::write(root.join("src/lib.rs"), body)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write src/lib.rs: {err}")));
}

fn resolve_installed_bin(install_root: &Path) -> PathBuf {
    let unix = install_root.join("bin").join("cargo-allow");
    let windows = install_root.join("bin").join("cargo-allow.exe");
    if windows.is_file() {
        windows
    } else if unix.is_file() {
        unix
    } else {
        std::panic::panic_any(format!(
            "missing installed binary under {}",
            install_root.display()
        ))
    }
}

fn install_cargo_allow(workspace: &Path, install_root: &Path) -> PathBuf {
    if let Ok(override_bin) = std::env::var("CARGO_ALLOW_BIN") {
        let path = PathBuf::from(override_bin);
        if path.is_file() {
            return path;
        }
        std::panic::panic_any(format!(
            "CARGO_ALLOW_BIN is set but not a file: {}",
            path.display()
        ));
    }

    run(
        Command::new("cargo")
            .arg("install")
            .arg("--path")
            .arg(workspace.join("crates/cargo-allow"))
            .arg("--locked")
            .arg("--root")
            .arg(install_root)
            .arg("--force")
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("cargo install: {err}"))),
        "cargo install --path crates/cargo-allow",
    );
    resolve_installed_bin(install_root)
}

fn cargo_allow_cmd(bin: &Path) -> Command {
    Command::new(bin)
}

fn cmd_version(bin: &Path) -> (i32, String) {
    let output = run(
        cargo_allow_cmd(bin)
            .arg("--version")
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("--version: {err}"))),
        "--version",
    );
    (
        output.status.code().unwrap_or(1),
        String::from_utf8_lossy(&output.stdout).trim().to_string(),
    )
}

#[derive(Clone)]
struct StepResult {
    id: &'static str,
    exit_code: i32,
    artifact_schema_id: Option<&'static str>,
}

fn run_brownfield_journey(bin: &Path, consumer: &Path) -> Vec<StepResult> {
    write_source(
        consumer,
        "pub fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    );

    let mut steps = Vec::new();

    let (version_code, version_text) = cmd_version(bin);
    assert!(
        version_text.starts_with("cargo-allow "),
        "unexpected version output: {version_text}"
    );
    steps.push(StepResult {
        id: "version",
        exit_code: version_code,
        artifact_schema_id: None,
    });

    let doctor = run(
        cargo_allow_cmd(bin)
            .arg("doctor")
            .arg("--root")
            .arg(consumer)
            .arg("--format")
            .arg("json")
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("doctor: {err}"))),
        "doctor",
    );
    let doctor_json: serde_json::Value = serde_json::from_slice(&doctor.stdout)
        .unwrap_or_else(|err| std::panic::panic_any(format!("doctor json: {err}")));
    assert_eq!(
        doctor_json
            .get("schema_id")
            .and_then(serde_json::Value::as_str),
        Some("cargo-allow.doctor.v1")
    );
    steps.push(StepResult {
        id: "doctor_no_policy",
        exit_code: doctor.status.code().unwrap_or(1),
        artifact_schema_id: Some("cargo-allow.doctor.v1"),
    });

    let audit = run(
        cargo_allow_cmd(bin)
            .arg("audit")
            .arg("--root")
            .arg(consumer)
            .arg("--kind")
            .arg("panic")
            .arg("--format")
            .arg("json")
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("audit: {err}"))),
        "audit",
    );
    let audit_json: serde_json::Value = serde_json::from_slice(&audit.stdout)
        .unwrap_or_else(|err| std::panic::panic_any(format!("audit json: {err}")));
    assert_eq!(
        audit_json
            .pointer("/summary/new")
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
    steps.push(StepResult {
        id: "audit_with_finding",
        exit_code: audit.status.code().unwrap_or(1),
        artifact_schema_id: Some("cargo-allow.report.v1"),
    });

    let policy = consumer.join("policy/allow.toml");
    let propose = run(
        cargo_allow_cmd(bin)
            .arg("propose")
            .arg("--root")
            .arg(consumer)
            .arg("--kind")
            .arg("panic")
            .arg("--write")
            .arg(&policy)
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("propose: {err}"))),
        "propose",
    );
    assert!(policy.is_file(), "propose must write policy/allow.toml");
    steps.push(StepResult {
        id: "bootstrap_propose_write",
        exit_code: propose.status.code().unwrap_or(1),
        artifact_schema_id: None,
    });

    let check = run(
        cargo_allow_cmd(bin)
            .arg("check")
            .arg("--root")
            .arg(consumer)
            .arg("--config")
            .arg(&policy)
            .arg("--kind")
            .arg("panic")
            .arg("--mode")
            .arg("no-new")
            .arg("--format")
            .arg("json")
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("check: {err}"))),
        "check no-new",
    );
    let check_json: serde_json::Value = serde_json::from_slice(&check.stdout)
        .unwrap_or_else(|err| std::panic::panic_any(format!("check json: {err}")));
    assert_eq!(
        check_json.get("status").and_then(serde_json::Value::as_str),
        Some("passed")
    );
    steps.push(StepResult {
        id: "check_no_new_pass",
        exit_code: check.status.code().unwrap_or(1),
        artifact_schema_id: Some("cargo-allow.report.v1"),
    });

    let list = run(
        cargo_allow_cmd(bin)
            .arg("list")
            .arg("--root")
            .arg(consumer)
            .arg("--config")
            .arg(&policy)
            .arg("--kind")
            .arg("panic")
            .arg("--format")
            .arg("json")
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("list: {err}"))),
        "list",
    );
    let list_json: serde_json::Value = serde_json::from_slice(&list.stdout)
        .unwrap_or_else(|err| std::panic::panic_any(format!("list json: {err}")));
    let allow_id = list_json
        .pointer("/allow_entries/0/id")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| std::panic::panic_any("list should report an allow id"));

    run(
        cargo_allow_cmd(bin)
            .arg("explain")
            .arg(&allow_id)
            .arg("--root")
            .arg(consumer)
            .arg("--config")
            .arg(&policy)
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("explain: {err}"))),
        "explain",
    );
    run(
        cargo_allow_cmd(bin)
            .arg("worklist")
            .arg("--root")
            .arg(consumer)
            .arg("--config")
            .arg(&policy)
            .arg("--kind")
            .arg("panic")
            .arg("--format")
            .arg("json")
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("worklist: {err}"))),
        "worklist",
    );
    steps.push(StepResult {
        id: "list_explain_worklist",
        exit_code: list.status.code().unwrap_or(1),
        artifact_schema_id: Some("cargo-allow.list.v1"),
    });

    steps
}

fn build_receipt(
    version: &str,
    git_head: Option<String>,
    version_output: &str,
    steps: &[StepResult],
    install_method: &str,
) -> serde_json::Value {
    let steps_executed: Vec<serde_json::Value> = steps
        .iter()
        .map(|step| {
            serde_json::json!({
                "id": step.id,
                "exit_code": step.exit_code,
                "artifact_schema_id": step.artifact_schema_id,
            })
        })
        .collect();

    serde_json::json!({
        "schema_version": 1,
        "schema_id": SCHEMA_ID,
        "tool": "cargo-allow",
        "result": "Passed",
        "claim_boundary": [
            "installed_binary_first_hour_journey",
            "temporary_consumer_repository",
            "source_candidate_not_published_registry"
        ],
        "candidate": {
            "workspace_version": version,
            "git_head": git_head,
            "package_set_provenance": "workspace_path_install_after_optional_package_gate",
            "install_method": install_method
        },
        "environment": {
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "rustc_version": null,
            "cargo_version": null,
            "network_posture": "not_required_for_core_journey"
        },
        "installed_binary": {
            "version_output": version_output,
            "path_redacted": true
        },
        "journey": {
            "fixture_generation": "first_hour_brownfield_v1",
            "steps_expected": STEPS_EXPECTED,
            "steps_executed": steps_executed
        },
        "limitations": [
            "package_set_not_consumed_from_isolated_registry",
            "source_checkout_not_denied_during_install",
            "lifecycle_diff_refresh_prune_not_executed",
            "negative_controls_not_run",
            "published_registry_install_not_executed"
        ]
    })
}

fn assert_receipt_shape(receipt: &serde_json::Value) {
    assert_eq!(
        receipt.get("schema_id").and_then(serde_json::Value::as_str),
        Some(SCHEMA_ID)
    );
    assert_eq!(
        receipt.get("result").and_then(serde_json::Value::as_str),
        Some("Passed")
    );
    let install_method = receipt
        .pointer("/candidate/install_method")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    assert!(
        install_method == "cargo_install_path" || install_method == "prebuilt_override",
        "unexpected install_method {install_method}"
    );
    let executed = receipt
        .pointer("/journey/steps_executed")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("steps_executed missing"));
    assert_eq!(executed.len(), STEPS_EXPECTED.len());
    for (idx, expected) in STEPS_EXPECTED.iter().enumerate() {
        let Some(step) = executed.get(idx) else {
            std::panic::panic_any(format!("missing executed step {idx}"));
        };
        assert_eq!(
            step.get("id").and_then(serde_json::Value::as_str),
            Some(*expected)
        );
        assert_eq!(
            step.get("exit_code").and_then(serde_json::Value::as_i64),
            Some(0)
        );
    }
    let limitations = receipt
        .get("limitations")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("limitations missing"));
    assert!(
        limitations
            .iter()
            .any(|v| { v.as_str() == Some("package_set_not_consumed_from_isolated_registry") }),
        "receipt must record #2277 isolation limitation"
    );
}

#[test]
fn example_source_candidate_smoke_receipt_matches_schema_constants() {
    assert!(
        SCHEMA_DOC.contains(SCHEMA_ID),
        "schema fixture must pin {SCHEMA_ID}"
    );
    let example: serde_json::Value = serde_json::from_str(EXAMPLE_RECEIPT)
        .unwrap_or_else(|err| std::panic::panic_any(format!("example receipt json: {err}")));
    assert_eq!(
        example.get("schema_id").and_then(serde_json::Value::as_str),
        Some(SCHEMA_ID)
    );
    assert_eq!(
        example.get("result").and_then(serde_json::Value::as_str),
        Some("Passed")
    );
}

#[test]
fn source_candidate_smoke_path_install_completes_first_hour_journey() {
    let workspace = workspace_root();
    let version = workspace_version(&workspace);
    let install_root = temp_dir("install");
    let consumer = temp_dir("consumer");
    let out_dir = workspace.join("target/source-candidate-smoke");
    let _ = fs::create_dir_all(&out_dir);

    let install_method = if std::env::var_os("CARGO_ALLOW_BIN").is_some() {
        "prebuilt_override"
    } else {
        "cargo_install_path"
    };
    let bin = install_cargo_allow(&workspace, &install_root);
    let (version_code, version_output) = cmd_version(&bin);
    assert_eq!(version_code, 0);
    assert!(
        version_output.contains(&version),
        "installed version `{version_output}` should contain workspace version `{version}`"
    );

    let steps = run_brownfield_journey(&bin, &consumer);
    assert_eq!(steps.len(), STEPS_EXPECTED.len());

    let git_head = Command::new("git")
        .arg("-C")
        .arg(&workspace)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());

    let receipt = build_receipt(&version, git_head, &version_output, &steps, install_method);
    assert_receipt_shape(&receipt);

    let receipt_path = out_dir.join("source-candidate-smoke.receipt.json");
    let rendered = serde_json::to_string_pretty(&receipt)
        .unwrap_or_else(|err| std::panic::panic_any(format!("serialize receipt: {err}")));
    fs::write(&receipt_path, format!("{rendered}\n"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("write receipt: {err}")));

    drop_dir(consumer);
    // Keep install_root only when override was not used; always drop temp install.
    if install_method == "cargo_install_path" {
        drop_dir(install_root);
    }
}
