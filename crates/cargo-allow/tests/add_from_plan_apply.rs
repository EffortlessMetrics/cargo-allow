mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use support::{
    assert_saved_json_artifact, assert_status, assert_stderr_empty, assert_stdout_empty,
    cargo_allow_command, remove_temp_root, temp_root,
};

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("git {args:?}: {error}"));
    assert_status("git fixture", &output, true);
}

/// Initialise a git repo with an unreceipted `panic.unwrap` finding and return
/// its root.
fn init_fixture(label: &str) -> PathBuf {
    let root = temp_root(label);
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|error| panic!("create source dir: {error}"));
    git(&root, &["init"]);
    git(&root, &["config", "user.email", "fixture@example.com"]);
    git(&root, &["config", "user.name", "fixture"]);
    let init = cargo_allow_command()
        .args(["init", "--root"])
        .arg(&root)
        .output()
        .unwrap_or_else(|error| panic!("init fixture: {error}"));
    assert_status("init fixture", &init, true);
    fs::write(
        root.join("src/lib.rs"),
        "pub fn load() -> usize { Some(1).unwrap() }\n",
    )
    .unwrap_or_else(|error| panic!("write source: {error}"));
    // Commit so the inventory is the stable `git_tracked` set; untracked
    // artifacts written into the tree later (the plan, the receipt) then do not
    // perturb the recomputed inventory basis between generation and application.
    git(&root, &["add", "policy/allow.toml", "src/lib.rs"]);
    git(&root, &["commit", "-q", "-m", "fixture"]);
    root
}

fn generate_plan(root: &Path) -> PathBuf {
    let plan_path = root.join("add-plan.json");
    let why = cargo_allow_command()
        .args(["why", "--root"])
        .arg(root)
        .args(["--kind", "panic", "--path", "src/lib.rs", "--line", "1"])
        .arg("--plan")
        .arg(&plan_path)
        .output()
        .unwrap_or_else(|error| panic!("why plan: {error}"));
    assert_status("why plan", &why, true);
    assert!(plan_path.exists(), "plan should be written");
    plan_path
}

fn is_sha256_v1(value: Option<&str>) -> bool {
    value.is_some_and(|value| value.starts_with("sha256:v1:") && value.len() == 74)
}

#[test]
fn add_from_plan_applies_a_verified_plan_and_binds_a_receipt()
-> Result<(), Box<dyn std::error::Error>> {
    let root = init_fixture("add-from-plan-apply");
    let plan_path = generate_plan(&root);
    let policy_path = root.join("policy/allow.toml");
    let policy_before = fs::read_to_string(&policy_path)
        .unwrap_or_else(|error| panic!("read policy: {error}"));

    let receipt_path = root.join("receipt.json");
    let common_summary_path = root.join("common-summary.json");
    let apply = cargo_allow_command()
        .arg("--command-summary-output")
        .arg(&common_summary_path)
        .args(["add", "--root"])
        .arg(&root)
        .arg("--from-plan")
        .arg(&plan_path)
        .args([
            "--owner",
            "fixture",
            "--reason",
            "covered by the add-from-plan lifecycle test",
            "--update",
        ])
        .arg("--summary-output")
        .arg(&receipt_path)
        .output()
        .unwrap_or_else(|error| panic!("add from-plan: {error}"));
    assert_status("add from-plan", &apply, true);
    assert_stdout_empty(
        "add from-plan",
        &apply,
        "policy goes to the live ledger and the receipt to --summary-output",
    );
    assert_stderr_empty(
        "add from-plan",
        &apply,
        "the receipt goes to --summary-output, not stderr",
    );

    let common_summary: Value = serde_json::from_str(
        &fs::read_to_string(&common_summary_path)
            .unwrap_or_else(|error| panic!("read common summary: {error}")),
    )?;
    assert_eq!(
        common_summary.get("schema_id").and_then(Value::as_str),
        Some("cargo-allow.core-command-summary.v1")
    );
    assert_eq!(
        common_summary.get("tool").and_then(Value::as_str),
        Some("cargo-allow")
    );
    assert_eq!(
        common_summary.pointer("/operation").and_then(Value::as_str),
        Some("add_from_plan")
    );
    assert_eq!(
        common_summary.pointer("/posture").and_then(Value::as_str),
        Some("satisfied")
    );
    assert_eq!(
        common_summary
            .pointer("/operation_effects/writes_repository")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        common_summary
            .pointer("/operation_effects/write_paths/0")
            .and_then(Value::as_str),
        Some("policy/allow.toml")
    );
    assert_eq!(
        common_summary
            .pointer("/next_proof/args/0")
            .and_then(Value::as_str),
        Some("check")
    );
    assert_eq!(
        common_summary
            .pointer("/next_proof/args/2")
            .and_then(Value::as_str),
        Some("no-new")
    );

    let receipt = assert_saved_json_artifact(
        &receipt_path,
        "add-plan-application",
        "cargo-allow.add-plan-application.v1",
        "add",
    );
    let schema: Value = serde_json::from_str(include_str!(
        "../../../docs/schemas/add-plan-application.schema.json"
    ))?;
    let validator = jsonschema::validator_for(&schema)?;
    assert!(
        validator.validate(&receipt).is_ok(),
        "runtime-produced add-plan-application receipt should validate against its published schema"
    );
    for pointer in [
        "/plan_digest",
        "/finding_digest",
        "/repository_identity",
        "/policy_before_digest",
        "/policy_after_digest",
    ] {
        assert!(
            is_sha256_v1(receipt.pointer(pointer).and_then(Value::as_str)),
            "{pointer} should be a versioned SHA-256 binding"
        );
    }
    assert_ne!(
        receipt.pointer("/policy_before_digest"),
        receipt.pointer("/policy_after_digest"),
        "before/after policy digests must differ after a real write"
    );
    assert_eq!(
        receipt.pointer("/targeted_recheck").and_then(Value::as_str),
        Some("matched")
    );
    assert_eq!(
        receipt
            .pointer("/full_check_argv/0")
            .and_then(Value::as_str),
        Some("check")
    );
    assert!(
        receipt
            .pointer("/added_allow_id")
            .and_then(Value::as_str)
            .is_some_and(|id| id.starts_with("allow-")),
        "receipt should record the added allow id"
    );

    let policy_after = fs::read_to_string(&policy_path)
        .unwrap_or_else(|error| panic!("reread policy: {error}"));
    assert_ne!(policy_before, policy_after, "policy should have changed");
    assert!(
        policy_after.contains("fixture"),
        "policy should record the operator-supplied owner"
    );

    // Replay: the same plan applied again must fail (the finding is no longer
    // `New`) and must leave policy untouched.
    let replay = cargo_allow_command()
        .args(["add", "--root"])
        .arg(&root)
        .arg("--from-plan")
        .arg(&plan_path)
        .args([
            "--owner",
            "fixture",
            "--reason",
            "replay attempt after success",
            "--update",
        ])
        .output()
        .unwrap_or_else(|error| panic!("replay: {error}"));
    assert_status("replay", &replay, false);
    let policy_after_replay = fs::read_to_string(&policy_path)
        .unwrap_or_else(|error| panic!("reread policy: {error}"));
    assert_eq!(
        policy_after, policy_after_replay,
        "a rejected replay must not mutate policy"
    );

    remove_temp_root(root);
    Ok(())
}

#[test]
fn add_from_plan_rejects_source_drift_without_mutation() {
    let root = init_fixture("add-from-plan-drift");
    let plan_path = generate_plan(&root);
    let policy_path = root.join("policy/allow.toml");

    // Drift the source file after the plan was generated.
    fs::write(
        root.join("src/lib.rs"),
        "// drifted\npub fn load() -> usize { Some(1).unwrap() }\n",
    )
    .unwrap_or_else(|error| panic!("rewrite source: {error}"));
    git(&root, &["add", "src/lib.rs"]);

    let policy_before = fs::read_to_string(&policy_path)
        .unwrap_or_else(|error| panic!("read policy: {error}"));
    let apply = cargo_allow_command()
        .args(["add", "--root"])
        .arg(&root)
        .arg("--from-plan")
        .arg(&plan_path)
        .args([
            "--owner",
            "fixture",
            "--reason",
            "should be rejected for drift",
            "--update",
        ])
        .output()
        .unwrap_or_else(|error| panic!("add from-plan drift: {error}"));
    assert_status("add from-plan drift", &apply, false);
    let policy_after = fs::read_to_string(&policy_path)
        .unwrap_or_else(|error| panic!("reread policy: {error}"));
    assert_eq!(
        policy_before, policy_after,
        "a stale plan must not mutate policy"
    );

    remove_temp_root(root);
}

#[test]
fn add_from_plan_requires_update_and_conflicts_with_write() {
    let root = init_fixture("add-from-plan-flags");
    let plan_path = generate_plan(&root);

    // Missing --update: the live-ledger route is mandatory.
    let no_update = cargo_allow_command()
        .args(["add", "--root"])
        .arg(&root)
        .arg("--from-plan")
        .arg(&plan_path)
        .args(["--owner", "fixture", "--reason", "no update flag"])
        .output()
        .unwrap_or_else(|error| panic!("add from-plan no update: {error}"));
    assert_status("add from-plan without --update", &no_update, false);

    // Conflict with a candidate-file write target.
    let with_write = cargo_allow_command()
        .args(["add", "--root"])
        .arg(&root)
        .arg("--from-plan")
        .arg(&plan_path)
        .args([
            "--owner",
            "fixture",
            "--reason",
            "write conflict",
            "--update",
        ])
        .arg("--write")
        .arg(root.join("candidate.toml"))
        .output()
        .unwrap_or_else(|error| panic!("add from-plan write: {error}"));
    assert_status("add from-plan with --write", &with_write, false);

    // Conflict with a manual target selector.
    let with_kind = cargo_allow_command()
        .args(["add", "--root"])
        .arg(&root)
        .arg("--from-plan")
        .arg(&plan_path)
        .args([
            "--owner",
            "fixture",
            "--reason",
            "kind conflict",
            "--update",
            "--kind",
            "panic",
        ])
        .output()
        .unwrap_or_else(|error| panic!("add from-plan kind: {error}"));
    assert_status("add from-plan with --kind", &with_kind, false);

    remove_temp_root(root);
}
