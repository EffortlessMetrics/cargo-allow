use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

#[cfg(unix)]
use std::os::unix::fs::symlink;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn hooks_verify_accepts_explicit_preview_binary_and_reports_digest_mismatch() -> TestResult {
    let fixture = Fixture::new("verify")?;
    let binary = path_arg(Path::new(env!("CARGO_BIN_EXE_cargo-allow")));
    let identity = run_success(&fixture.path, &["tool", "identity", "--format", "json"])?;
    let identity: Value = serde_json::from_slice(&identity.stdout)?;
    let digest = identity
        .get("executable_digest")
        .and_then(Value::as_str)
        .ok_or("tool identity omitted executable_digest")?;
    let report = fixture.path.join("target/verification.json");
    let report_arg = path_arg(&report);
    run_success(
        &fixture.path,
        &[
            "hooks",
            "verify",
            "--binary",
            &binary,
            "--digest",
            digest,
            "--mode",
            "explicit-tool-under-test",
            "--format",
            "json",
            "--output",
            &report_arg,
        ],
    )?;
    let report_value: Value = serde_json::from_slice(&fs::read(&report)?)?;
    require(
        report_value.get("selected").and_then(Value::as_bool) == Some(true)
            && report_value.get("result").and_then(Value::as_str)
                == Some("ToolPrebuiltAndSelected")
            && report_value
                .get("preview_evidence")
                .and_then(Value::as_bool)
                == Some(true),
        "explicit preview verification did not report a selected tool",
    )?;

    let mismatch = fixture.path.join("target/mismatch.json");
    let mismatch_arg = path_arg(&mismatch);
    let output = run(
        &fixture.path,
        &[
            "hooks",
            "verify",
            "--binary",
            &binary,
            "--digest",
            "sha256:v1:deliberate-mismatch",
            "--mode",
            "explicit-tool-under-test",
            "--format",
            "json",
            "--output",
            &mismatch_arg,
        ],
    )?;
    require(
        !output.status.success(),
        "binary verification unexpectedly accepted a mismatched digest",
    )?;
    let mismatch_value: Value = serde_json::from_slice(&fs::read(&mismatch)?)?;
    require(
        mismatch_value.get("selected").and_then(Value::as_bool) == Some(false)
            && mismatch_value.get("result").and_then(Value::as_str) == Some("ToolIdentityMismatch"),
        "digest mismatch report did not preserve the fail-closed result",
    )?;
    Ok(())
}

#[test]
fn hooks_apply_creates_and_reports_a_managed_hook() -> TestResult {
    let fixture = Fixture::new("create")?;
    init_git(&fixture.path)?;
    let plan = fixture.path.join("target/hook-plan.json");
    let receipt = fixture.path.join("target/hook-receipt.json");
    let plan_arg = path_arg(&plan);
    let receipt_arg = path_arg(&receipt);

    run_success(
        &fixture.path,
        &[
            "hooks",
            "plan",
            "--stage",
            "pre-commit",
            "--format",
            "json",
            "--output",
            &plan_arg,
        ],
    )?;
    run_success(
        &fixture.path,
        &[
            "hooks",
            "apply",
            "--plan",
            &plan_arg,
            "--accept",
            "--receipt",
            &receipt_arg,
        ],
    )?;

    let hook = fixture.path.join(".git/hooks/pre-commit");
    let hook_text = fs::read_to_string(&hook)?;
    require(
        hook_text.contains("cargo-allow managed hook"),
        "created hook omitted its managed marker",
    )?;
    require(
        hook_text.contains("exec 'cargo-allow' 'check' '--mode' 'no-new'"),
        "created hook did not preserve quoted argv",
    )?;
    let receipt_text = fs::read_to_string(&receipt)?;
    require(
        receipt_text.contains("\"applied\": true"),
        "apply receipt did not record the create",
    )?;

    let status = run_success(
        &fixture.path,
        &[
            "hooks",
            "status",
            "--stage",
            "pre-commit",
            "--format",
            "json",
        ],
    )?;
    let status: Value = serde_json::from_slice(&status.stdout)?;
    require(
        status.get("disposition").and_then(Value::as_str) == Some("AlreadyPresent"),
        "status did not recognize the managed hook",
    )?;

    run_success(
        &fixture.path,
        &[
            "hooks",
            "apply",
            "--plan",
            &plan_arg,
            "--accept",
            "--receipt",
            &receipt_arg,
        ],
    )?;
    require(
        fs::read_to_string(&receipt)?.contains("\"operation\": \"none\""),
        "reapplying a matching managed hook did not record a no-op",
    )?;
    Ok(())
}

#[test]
fn hooks_apply_preserves_unmanaged_hook_and_records_manual_merge() -> TestResult {
    let fixture = Fixture::new("manual-merge")?;
    init_git(&fixture.path)?;
    let hook = fixture.path.join(".git/hooks/pre-commit");
    fs::write(&hook, "#!/bin/sh\ncustom hook\n")?;
    let plan = fixture.path.join("target/hook-plan.json");
    let receipt = fixture.path.join("target/hook-receipt.json");
    let plan_arg = path_arg(&plan);
    let receipt_arg = path_arg(&receipt);

    run_success(
        &fixture.path,
        &[
            "hooks",
            "plan",
            "--stage",
            "pre-commit",
            "--format",
            "json",
            "--output",
            &plan_arg,
        ],
    )?;
    let output = run(
        &fixture.path,
        &[
            "hooks",
            "apply",
            "--plan",
            &plan_arg,
            "--accept",
            "--receipt",
            &receipt_arg,
        ],
    )?;
    require(
        !output.status.success(),
        "apply unexpectedly replaced an unmanaged hook",
    )?;
    require(
        String::from_utf8_lossy(&output.stderr).contains("ManualMerge"),
        "manual-merge error did not name the disposition",
    )?;
    require(
        fs::read_to_string(&hook)? == "#!/bin/sh\ncustom hook\n",
        "unmanaged hook bytes changed",
    )?;
    require(
        fs::read_to_string(&receipt)?.contains("ManualMerge"),
        "manual-merge receipt was not retained",
    )?;

    fs::write(
        &hook,
        "#!/bin/sh\n# BEGIN cargo-allow managed hook: another-plan\n# END cargo-allow managed hook\n",
    )?;
    let conflict_receipt = fixture.path.join("target/conflict-receipt.json");
    let conflict_receipt_arg = path_arg(&conflict_receipt);
    let output = run(
        &fixture.path,
        &[
            "hooks",
            "apply",
            "--plan",
            &plan_arg,
            "--accept",
            "--receipt",
            &conflict_receipt_arg,
        ],
    )?;
    require(
        !output.status.success() && String::from_utf8_lossy(&output.stderr).contains("Conflict"),
        "mismatched managed hook did not report Conflict",
    )?;
    require(
        fs::read_to_string(&conflict_receipt)?.contains("Conflict"),
        "conflict receipt was not retained",
    )?;
    Ok(())
}

#[test]
fn hooks_remove_requires_exact_receipt_and_restores_apply_rollback_path() -> TestResult {
    let fixture = Fixture::new("remove")?;
    init_git(&fixture.path)?;
    let plan = fixture.path.join("target/hook-plan.json");
    let apply_receipt = fixture.path.join("target/hook-receipt.json");
    let remove_receipt = fixture.path.join("target/remove-receipt.json");
    let plan_arg = path_arg(&plan);
    let apply_receipt_arg = path_arg(&apply_receipt);
    let remove_receipt_arg = path_arg(&remove_receipt);

    run_success(
        &fixture.path,
        &[
            "hooks",
            "plan",
            "--stage",
            "pre-commit",
            "--format",
            "json",
            "--output",
            &plan_arg,
        ],
    )?;
    run_success(
        &fixture.path,
        &[
            "hooks",
            "apply",
            "--plan",
            &plan_arg,
            "--accept",
            "--receipt",
            &apply_receipt_arg,
        ],
    )?;

    let output = run(
        &fixture.path,
        &[
            "hooks",
            "remove",
            "--receipt",
            &apply_receipt_arg,
            "--result-receipt",
            &remove_receipt_arg,
        ],
    )?;
    require(
        !output.status.success() && String::from_utf8_lossy(&output.stderr).contains("--accept"),
        "remove did not require explicit acceptance",
    )?;
    require(
        fixture.path.join(".git/hooks/pre-commit").is_file(),
        "preview remove changed the managed hook",
    )?;

    run_success(
        &fixture.path,
        &[
            "hooks",
            "remove",
            "--receipt",
            &apply_receipt_arg,
            "--accept",
            "--result-receipt",
            &remove_receipt_arg,
        ],
    )?;
    require(
        !fixture.path.join(".git/hooks/pre-commit").exists(),
        "accepted remove did not remove the exact managed hook",
    )?;
    let removal: Value = serde_json::from_slice(&fs::read(&remove_receipt)?)?;
    require(
        removal.get("schema").and_then(Value::as_str)
            == Some("cargo-allow.local-hook-remove-receipt.v1")
            && removal.get("operation").and_then(Value::as_str) == Some("remove")
            && removal.get("removed").and_then(Value::as_bool) == Some(true),
        "removal receipt did not record the exact managed deletion",
    )?;
    require(
        removal
            .get("rollback")
            .and_then(Value::as_str)
            .is_some_and(|value| value.contains("hooks apply")),
        "removal receipt omitted the recreate rollback route",
    )?;
    Ok(())
}

#[test]
fn hooks_remove_refuses_changed_managed_content() -> TestResult {
    let fixture = Fixture::new("remove-changed")?;
    init_git(&fixture.path)?;
    let plan = fixture.path.join("target/hook-plan.json");
    let apply_receipt = fixture.path.join("target/hook-receipt.json");
    let remove_receipt = fixture.path.join("target/remove-receipt.json");
    let plan_arg = path_arg(&plan);
    let apply_receipt_arg = path_arg(&apply_receipt);
    let remove_receipt_arg = path_arg(&remove_receipt);
    run_success(
        &fixture.path,
        &[
            "hooks", "plan", "--stage", "pre-push", "--format", "json", "--output", &plan_arg,
        ],
    )?;
    run_success(
        &fixture.path,
        &[
            "hooks",
            "apply",
            "--plan",
            &plan_arg,
            "--accept",
            "--receipt",
            &apply_receipt_arg,
        ],
    )?;
    let hook = fixture.path.join(".git/hooks/pre-push");
    fs::write(&hook, "#!/bin/sh\n# consumer change\n")?;

    let output = run(
        &fixture.path,
        &[
            "hooks",
            "remove",
            "--receipt",
            &apply_receipt_arg,
            "--accept",
            "--result-receipt",
            &remove_receipt_arg,
        ],
    )?;
    require(
        !output.status.success() && String::from_utf8_lossy(&output.stderr).contains("Changed"),
        &format!(
            "remove did not report changed managed content: status={}, stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ),
    )?;
    require(
        fs::read_to_string(&hook)?.contains("# consumer change"),
        "changed hook was removed despite the identity mismatch",
    )?;
    require(
        fs::read_to_string(&remove_receipt)?.contains("\"removed\": false"),
        "changed-hook receipt did not record a non-mutating result",
    )?;
    Ok(())
}

#[test]
fn hooks_compose_recognized_block_and_remove_only_that_block() -> TestResult {
    let fixture = Fixture::new("compose")?;
    init_git(&fixture.path)?;
    let plan = fixture.path.join("target/hook-plan.json");
    let apply_receipt = fixture.path.join("target/hook-receipt.json");
    let remove_receipt = fixture.path.join("target/remove-receipt.json");
    let plan_arg = path_arg(&plan);
    let apply_receipt_arg = path_arg(&apply_receipt);
    let remove_receipt_arg = path_arg(&remove_receipt);

    run_success(
        &fixture.path,
        &["hooks", "plan", "--format", "json", "--output", &plan_arg],
    )?;
    run_success(
        &fixture.path,
        &[
            "hooks",
            "apply",
            "--plan",
            &plan_arg,
            "--accept",
            "--receipt",
            &apply_receipt_arg,
        ],
    )?;

    let hook = fixture.path.join(".git/hooks/pre-commit");
    let managed = fs::read_to_string(&hook)?;
    let managed_block = managed.lines().skip(2).collect::<Vec<_>>().join("\r\n");
    fs::write(
        &hook,
        format!("#!/bin/sh\r\ncustom-before\r\n{managed_block}\r\ncustom-after\r\n"),
    )?;

    let status = run_success(&fixture.path, &["hooks", "status", "--format", "json"])?;
    let status: Value = serde_json::from_slice(&status.stdout)?;
    require(
        status.get("disposition").and_then(Value::as_str) == Some("Composed"),
        "status did not recognize the composed managed block",
    )?;

    let before_apply = fs::read_to_string(&hook)?;
    run_success(
        &fixture.path,
        &[
            "hooks",
            "apply",
            "--plan",
            &plan_arg,
            "--accept",
            "--receipt",
            &apply_receipt_arg,
        ],
    )?;
    require(
        fs::read_to_string(&hook)? == before_apply,
        "reapplying a composed managed block changed unrelated hook bytes",
    )?;

    run_success(
        &fixture.path,
        &[
            "hooks",
            "remove",
            "--receipt",
            &apply_receipt_arg,
            "--accept",
            "--result-receipt",
            &remove_receipt_arg,
        ],
    )?;
    require(
        fs::read_to_string(&hook)? == "#!/bin/sh\r\ncustom-before\r\ncustom-after\r\n",
        "composed removal did not preserve unrelated hook bytes",
    )?;
    let removal: Value = serde_json::from_slice(&fs::read(&remove_receipt)?)?;
    require(
        removal.get("disposition").and_then(Value::as_str) == Some("Composed")
            && removal.get("operation").and_then(Value::as_str) == Some("remove_block")
            && removal.get("removed").and_then(Value::as_bool) == Some(true),
        "composed removal receipt did not describe block removal",
    )?;
    Ok(())
}

#[test]
fn hooks_remove_is_a_receipted_noop_when_the_exact_hook_is_missing() -> TestResult {
    let fixture = Fixture::new("remove-missing")?;
    init_git(&fixture.path)?;
    let plan = fixture.path.join("target/hook-plan.json");
    let apply_receipt = fixture.path.join("target/hook-receipt.json");
    let remove_receipt = fixture.path.join("target/remove-receipt.json");
    let plan_arg = path_arg(&plan);
    let apply_receipt_arg = path_arg(&apply_receipt);
    let remove_receipt_arg = path_arg(&remove_receipt);
    run_success(
        &fixture.path,
        &[
            "hooks",
            "plan",
            "--stage",
            "pre-commit",
            "--format",
            "json",
            "--output",
            &plan_arg,
        ],
    )?;
    run_success(
        &fixture.path,
        &[
            "hooks",
            "apply",
            "--plan",
            &plan_arg,
            "--accept",
            "--receipt",
            &apply_receipt_arg,
        ],
    )?;
    fs::remove_file(fixture.path.join(".git/hooks/pre-commit"))?;
    run_success(
        &fixture.path,
        &[
            "hooks",
            "remove",
            "--receipt",
            &apply_receipt_arg,
            "--accept",
            "--result-receipt",
            &remove_receipt_arg,
        ],
    )?;
    let removal: Value = serde_json::from_slice(&fs::read(&remove_receipt)?)?;
    require(
        removal.get("disposition").and_then(Value::as_str) == Some("Missing")
            && removal.get("operation").and_then(Value::as_str) == Some("none")
            && removal.get("removed").and_then(Value::as_bool) == Some(false),
        "missing-hook removal did not record a non-mutating no-op",
    )?;
    Ok(())
}

#[test]
fn hooks_remove_refuses_composed_managed_content() -> TestResult {
    let fixture = Fixture::new("remove-composed")?;
    init_git(&fixture.path)?;
    let plan = fixture.path.join("target/hook-plan.json");
    let apply_receipt = fixture.path.join("target/hook-receipt.json");
    let remove_receipt = fixture.path.join("target/remove-receipt.json");
    let plan_arg = path_arg(&plan);
    let apply_receipt_arg = path_arg(&apply_receipt);
    let remove_receipt_arg = path_arg(&remove_receipt);
    run_success(
        &fixture.path,
        &["hooks", "plan", "--format", "json", "--output", &plan_arg],
    )?;
    run_success(
        &fixture.path,
        &[
            "hooks",
            "apply",
            "--plan",
            &plan_arg,
            "--accept",
            "--receipt",
            &apply_receipt_arg,
        ],
    )?;
    let hook = fixture.path.join(".git/hooks/pre-commit");
    let mut composed = fs::read_to_string(&hook)?;
    composed.push_str("# BEGIN cargo-allow managed hook: another-plan\n");
    fs::write(&hook, composed)?;
    let output = run(
        &fixture.path,
        &[
            "hooks",
            "remove",
            "--receipt",
            &apply_receipt_arg,
            "--accept",
            "--result-receipt",
            &remove_receipt_arg,
        ],
    )?;
    require(
        !output.status.success() && String::from_utf8_lossy(&output.stderr).contains("Conflict"),
        "composed hook removal did not fail closed",
    )?;
    require(
        fs::read_to_string(&hook)?.contains("another-plan"),
        "composed hook was removed despite the conflict",
    )?;
    Ok(())
}

#[test]
fn hooks_remove_refuses_receipt_for_a_different_hook_path() -> TestResult {
    let fixture = Fixture::new("remove-path-mismatch")?;
    init_git(&fixture.path)?;
    let plan = fixture.path.join("target/hook-plan.json");
    let apply_receipt = fixture.path.join("target/hook-receipt.json");
    let tampered_receipt = fixture.path.join("target/tampered-receipt.json");
    let plan_arg = path_arg(&plan);
    let apply_receipt_arg = path_arg(&apply_receipt);
    let tampered_receipt_arg = path_arg(&tampered_receipt);
    run_success(
        &fixture.path,
        &["hooks", "plan", "--format", "json", "--output", &plan_arg],
    )?;
    run_success(
        &fixture.path,
        &[
            "hooks",
            "apply",
            "--plan",
            &plan_arg,
            "--accept",
            "--receipt",
            &apply_receipt_arg,
        ],
    )?;
    let mut receipt: Value = serde_json::from_slice(&fs::read(&apply_receipt)?)?;
    receipt
        .as_object_mut()
        .ok_or("apply receipt was not a JSON object")?
        .insert(
            "hook_path".to_string(),
            Value::String(".git/hooks/pre-push".to_string()),
        );
    fs::write(&tampered_receipt, serde_json::to_vec_pretty(&receipt)?)?;
    let output = run(
        &fixture.path,
        &[
            "hooks",
            "remove",
            "--receipt",
            &tampered_receipt_arg,
            "--accept",
        ],
    )?;
    require(
        !output.status.success() && String::from_utf8_lossy(&output.stderr).contains("targets"),
        "path-mismatched receipt was accepted",
    )?;
    require(
        fixture.path.join(".git/hooks/pre-commit").is_file(),
        "path-mismatched receipt removed the managed hook",
    )?;
    Ok(())
}

#[test]
fn hooks_remove_refuses_malformed_and_stale_apply_receipts() -> TestResult {
    let fixture = Fixture::new("remove-invalid-receipt")?;
    init_git(&fixture.path)?;
    fs::create_dir_all(fixture.path.join("target"))?;
    let malformed = fixture.path.join("target/malformed-receipt.json");
    let malformed_arg = path_arg(&malformed);
    fs::write(&malformed, "not-json")?;
    let output = run(
        &fixture.path,
        &["hooks", "remove", "--receipt", &malformed_arg, "--accept"],
    )?;
    require(
        !output.status.success() && String::from_utf8_lossy(&output.stderr).contains("parse"),
        "malformed apply receipt was accepted",
    )?;

    let plan = fixture.path.join("target/hook-plan.json");
    let apply_receipt = fixture.path.join("target/hook-receipt.json");
    let stale_receipt = fixture.path.join("target/stale-receipt.json");
    let plan_arg = path_arg(&plan);
    let apply_receipt_arg = path_arg(&apply_receipt);
    let stale_receipt_arg = path_arg(&stale_receipt);
    run_success(
        &fixture.path,
        &["hooks", "plan", "--format", "json", "--output", &plan_arg],
    )?;
    run_success(
        &fixture.path,
        &[
            "hooks",
            "apply",
            "--plan",
            &plan_arg,
            "--accept",
            "--receipt",
            &apply_receipt_arg,
        ],
    )?;
    let mut receipt: Value = serde_json::from_slice(&fs::read(&apply_receipt)?)?;
    receipt
        .as_object_mut()
        .ok_or("apply receipt was not a JSON object")?
        .insert(
            "plan_identity".to_string(),
            Value::String("stale-plan".to_string()),
        );
    fs::write(&stale_receipt, serde_json::to_vec_pretty(&receipt)?)?;
    let output = run(
        &fixture.path,
        &[
            "hooks",
            "remove",
            "--receipt",
            &stale_receipt_arg,
            "--accept",
        ],
    )?;
    require(
        !output.status.success()
            && String::from_utf8_lossy(&output.stderr).contains("exact successful"),
        "stale apply receipt was accepted",
    )?;
    require(
        fixture.path.join(".git/hooks/pre-commit").is_file(),
        "stale apply receipt removed the managed hook",
    )?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn hooks_remove_refuses_symbolic_link_even_when_target_bytes_match() -> TestResult {
    let fixture = Fixture::new("remove-symlink")?;
    init_git(&fixture.path)?;
    let plan = fixture.path.join("target/hook-plan.json");
    let apply_receipt = fixture.path.join("target/hook-receipt.json");
    let target = fixture.path.join("target/managed-hook-target");
    let hook = fixture.path.join(".git/hooks/pre-commit");
    let plan_arg = path_arg(&plan);
    let apply_receipt_arg = path_arg(&apply_receipt);
    run_success(
        &fixture.path,
        &["hooks", "plan", "--format", "json", "--output", &plan_arg],
    )?;
    run_success(
        &fixture.path,
        &[
            "hooks",
            "apply",
            "--plan",
            &plan_arg,
            "--accept",
            "--receipt",
            &apply_receipt_arg,
        ],
    )?;
    let hook_bytes = fs::read(&hook)?;
    fs::write(&target, hook_bytes)?;
    fs::remove_file(&hook)?;
    symlink(&target, &hook)?;
    let output = run(
        &fixture.path,
        &[
            "hooks",
            "remove",
            "--receipt",
            &apply_receipt_arg,
            "--accept",
        ],
    )?;
    require(
        !output.status.success() && String::from_utf8_lossy(&output.stderr).contains("symbolic"),
        "symbolic-link hook removal was not refused",
    )?;
    require(hook.exists(), "symbolic-link hook was removed")?;
    Ok(())
}

#[test]
fn hooks_apply_reports_created_hook_when_receipt_cannot_be_written() -> TestResult {
    let fixture = Fixture::new("receipt-failure")?;
    init_git(&fixture.path)?;
    let plan = fixture.path.join("target/hook-plan.json");
    let receipt_parent = fixture.path.join("receipt-parent");
    fs::write(&receipt_parent, "not a directory")?;
    let plan_arg = path_arg(&plan);

    run_success(
        &fixture.path,
        &[
            "hooks",
            "plan",
            "--stage",
            "pre-commit",
            "--format",
            "json",
            "--output",
            &plan_arg,
        ],
    )?;
    let receipt = receipt_parent.join("receipt.json");
    let receipt_arg = path_arg(&receipt);
    let output = run(
        &fixture.path,
        &[
            "hooks",
            "apply",
            "--plan",
            &plan_arg,
            "--accept",
            "--receipt",
            &receipt_arg,
        ],
    )?;
    require(
        !output.status.success(),
        "apply unexpectedly wrote a receipt through a file parent",
    )?;
    require(
        String::from_utf8_lossy(&output.stderr).contains("created managed hook"),
        "receipt failure did not identify the created hook",
    )?;
    require(
        fixture.path.join(".git/hooks/pre-commit").is_file(),
        "receipt failure did not leave the created hook observable",
    )?;
    Ok(())
}

fn init_git(root: &Path) -> TestResult {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["-c", "init.defaultBranch=main", "init", "--quiet"])
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "git init failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(())
}

fn run_success(root: &Path, args: &[&str]) -> Result<Output, Box<dyn std::error::Error>> {
    let output = run(root, args)?;
    if !output.status.success() {
        return Err(format!(
            "cargo-allow failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(output)
}

fn run(root: &Path, args: &[&str]) -> Result<Output, Box<dyn std::error::Error>> {
    Ok(Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
        .current_dir(root)
        .args(args)
        .output()?)
}

fn path_arg(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn require(condition: bool, message: &str) -> TestResult {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

struct Fixture {
    path: PathBuf,
}

impl Fixture {
    fn new(label: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let path = std::env::temp_dir().join(format!("cargo-allow-hooks-{label}-{stamp}"));
        fs::create_dir_all(&path)?;
        Ok(Self { path })
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
