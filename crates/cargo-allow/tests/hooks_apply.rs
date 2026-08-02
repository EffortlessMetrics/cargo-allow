use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

type TestResult = Result<(), Box<dyn std::error::Error>>;

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
