mod json_assertions;
mod support;

use std::fs;
use std::process::Command;

use json_assertions::{assert_json_str, assert_json_u64};
use support::{
    assert_saved_json_artifact, assert_status, assert_stderr_empty, assert_stdout_empty,
    cargo_allow_command, remove_temp_root, temp_root,
};

#[test]
fn check_receipt_includes_run_metadata() {
    let root = temp_root("receipt-run-metadata");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::write(root.join("policy/allow.toml"), policy())
        .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));

    let receipt_output = root.join("target/cargo-allow/check.receipt.json");
    let result = cargo_allow_command()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--mode")
        .arg("audit")
        .arg("--format")
        .arg("json")
        .arg("--receipt")
        .arg(&receipt_output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow check: {err}")));

    assert_status("check", &result, true);
    let receipt = assert_saved_json_artifact(
        &receipt_output,
        "check receipt",
        "cargo-allow.receipt.v1",
        "check",
    );
    assert_json_str(&receipt, "/mode", "audit", "receipt mode");
    assert_json_str(&receipt, "/enforcement", "advisory", "receipt enforcement");
    assert_json_str(
        &receipt,
        "/tool_version",
        env!("CARGO_PKG_VERSION"),
        "receipt tool_version",
    );
    // #1854: receipt carries run provenance (started_at + run_id).
    let started_at = receipt
        .pointer("/started_at")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| std::panic::panic_any("receipt should carry started_at (RFC 3339)"));
    assert!(
        started_at.ends_with('Z'),
        "started_at should be RFC 3339 UTC: {started_at}"
    );
    let run_id = receipt
        .pointer("/run_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| std::panic::panic_any("receipt should carry run_id"));
    assert!(
        run_id.starts_with("cargo-allow-"),
        "run_id should be a cargo-allow invocation id: {run_id}"
    );
    assert!(
        receipt
            .pointer("/policy_config")
            .and_then(|value| value.as_str())
            .is_some_and(|path| path.contains("allow.toml")),
        "receipt should name the loaded policy path: {:?}",
        receipt.pointer("/policy_config")
    );
    // #1781: outside any repository there is no commit to bind; the integrity
    // keys must stay absent rather than carry empty or placeholder values.
    assert!(
        receipt.pointer("/git_sha").is_none(),
        "receipt must not emit git_sha outside a repository: {:?}",
        receipt.pointer("/git_sha")
    );
    assert!(
        receipt.pointer("/policy_digest").is_some_and(|value| value
            .as_str()
            .is_some_and(|digest| digest.starts_with("sha256:v1:"))),
        "receipt should bind the ledger bytes via policy_digest: {:?}",
        receipt.pointer("/policy_digest")
    );

    remove_temp_root(root);
}

#[test]
fn check_receipt_binds_head_commit_and_policy_digest() {
    let root = temp_root("receipt-provenance-bindings");
    let decoy = temp_root("receipt-provenance-decoy");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::write(root.join("policy/allow.toml"), policy())
        .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "policy/allow.toml"]);
    git(&root, &["commit", "-m", "base policy"]);
    fs::create_dir_all(decoy.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create decoy policy dir: {err}")));
    fs::write(decoy.join("policy/allow.toml"), policy())
        .unwrap_or_else(|err| std::panic::panic_any(format!("write decoy policy: {err}")));
    git(&decoy, &["init"]);
    git(
        &decoy,
        &["config", "user.email", "cargo-allow-decoy@example.invalid"],
    );
    git(&decoy, &["config", "user.name", "cargo-allow decoy"]);
    git(&decoy, &["add", "policy/allow.toml"]);
    git(&decoy, &["commit", "-m", "decoy policy"]);

    let head = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("resolve HEAD: {err}")));
    let expected_head = String::from_utf8_lossy(&head.stdout).trim().to_string();

    let receipt_output = root.join("target/cargo-allow/check.receipt.json");
    let result = cargo_allow_command()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--mode")
        .arg("audit")
        .arg("--format")
        .arg("json")
        .arg("--receipt")
        .arg(&receipt_output)
        .env("GIT_DIR", decoy.join(".git"))
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow check: {err}")));

    assert_status("check", &result, true);
    let receipt = assert_saved_json_artifact(
        &receipt_output,
        "check receipt",
        "cargo-allow.receipt.v1",
        "check",
    );
    // #1850/#1781: the receipt binds to the exact commit and ledger bytes.
    let git_sha = receipt
        .pointer("/git_sha")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| std::panic::panic_any("receipt should bind HEAD as git_sha"));
    assert_eq!(
        git_sha, expected_head,
        "git_sha must be the resolved HEAD commit"
    );
    let policy_digest = receipt
        .pointer("/policy_digest")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| std::panic::panic_any("receipt should bind ledger bytes"));
    assert_eq!(
        policy_digest,
        allow_core::sha256_v1_bytes(policy().as_bytes()),
        "policy_digest must hash the exact evaluated ledger bytes"
    );

    remove_temp_root(root);
    remove_temp_root(decoy);
}

#[test]
fn staged_check_receipt_hashes_staged_policy_bytes() {
    let root = temp_root("receipt-staged-policy");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::write(root.join("policy/allow.toml"), policy())
        .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "policy/allow.toml"]);
    git(&root, &["commit", "-m", "base policy"]);

    let staged_policy = format!("{}\n# staged policy A\n", policy());
    fs::write(root.join("policy/allow.toml"), &staged_policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write staged policy: {err}")));
    git(&root, &["add", "policy/allow.toml"]);
    let worktree_policy = format!("{}\n# worktree policy B\n", policy());
    fs::write(root.join("policy/allow.toml"), &worktree_policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write worktree policy: {err}")));

    let receipt_output = root.join("target/cargo-allow/staged.receipt.json");
    let result = cargo_allow_command()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--staged")
        .arg("--phase")
        .arg("precommit")
        .arg("--mode")
        .arg("audit")
        .arg("--format")
        .arg("json")
        .arg("--receipt")
        .arg(&receipt_output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run staged check: {err}")));

    assert_status("staged check", &result, true);
    let receipt = assert_saved_json_artifact(
        &receipt_output,
        "staged check receipt",
        "cargo-allow.receipt.v1",
        "check",
    );
    assert_eq!(
        receipt
            .pointer("/policy_digest")
            .and_then(serde_json::Value::as_str),
        Some(allow_core::sha256_v1_bytes(staged_policy.as_bytes()).as_str()),
        "staged receipt must hash the staged policy bytes"
    );
    assert_ne!(
        allow_core::sha256_v1_bytes(staged_policy.as_bytes()),
        allow_core::sha256_v1_bytes(worktree_policy.as_bytes()),
        "staged and worktree policy fixtures must be distinct"
    );

    remove_temp_root(root);
}

#[test]
fn check_validation_error_writes_error_receipt_instead_of_leaving_stale_file() {
    let root = temp_root("receipt-validation-error");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::write(root.join("policy/allow.toml"), policy())
        .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));

    let receipt_output = root.join("target/cargo-allow/check.receipt.json");
    let passing = cargo_allow_command()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--mode")
        .arg("no-new")
        .arg("--receipt")
        .arg(&receipt_output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run passing check: {err}")));
    assert_status("check", &passing, true);

    let invalid_policy = format!(
        r#"{}

[[allow]]
id = "allow-invalid"
kind = "panic"
family = "unwrap"
path = "src/invalid.rs"
owner = "core"
classification = "fixture"
reason = "fixture invalid lifecycle"
created = "2026-08-01"
review_after = "2026-07-01"

[allow.selector]
ast_kind = "method_call"
callee = "unwrap"
"#,
        policy()
    );
    fs::write(root.join("policy/allow.toml"), invalid_policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write invalid policy: {err}")));

    let failing = cargo_allow_command()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--mode")
        .arg("no-new")
        .arg("--receipt")
        .arg(&receipt_output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run failing check: {err}")));
    assert_eq!(
        failing.status.code(),
        Some(1),
        "invalid policy should exit 1 (runtime failure, not usage error): {:?}",
        failing
    );

    let receipt = assert_saved_json_artifact(
        &receipt_output,
        "check error receipt",
        "cargo-allow.receipt.v1",
        "check",
    );
    assert_json_str(&receipt, "/status", "error", "error receipt status");
    assert!(
        receipt
            .pointer("/diagnostic")
            .and_then(|value| value.as_str())
            .is_some_and(|message| message.contains("review_after must not be before created")),
        "error receipt should carry the validation diagnostic"
    );
    assert_json_u64(
        &receipt,
        "/counts/matched",
        0,
        "error receipt should not preserve stale matched counts",
    );
    assert!(
        receipt.pointer("/policy_digest").is_none(),
        "generic error receipts intentionally omit policy_digest because the error writer does not retain evaluated provenance: {:?}",
        receipt.pointer("/policy_digest")
    );

    remove_temp_root(root);
}

#[test]
fn check_receipt_file_exposes_saved_json_contract() {
    let root = temp_root("receipt-output");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::write(root.join("policy/allow.toml"), policy())
        .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));

    let report_output = root.join("target/cargo-allow/check.md");
    let receipt_output = root.join("target/cargo-allow/check.receipt.json");
    let result = cargo_allow_command()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--mode")
        .arg("no-new")
        .arg("--format")
        .arg("markdown")
        .arg("--output")
        .arg(&report_output)
        .arg("--receipt")
        .arg(&receipt_output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow check: {err}")));

    assert_status("check", &result, true);
    assert_stdout_empty(
        "check",
        &result,
        "--output should not emit report markdown to stdout",
    );
    assert_stderr_empty(
        "check",
        &result,
        "--output and --receipt should not emit side-channel status to stderr",
    );
    assert_saved_json_artifact(
        &receipt_output,
        "check receipt",
        "cargo-allow.receipt.v1",
        "check",
    );

    remove_temp_root(root);
}

#[test]
fn check_success_reports_policy_missing_evidence_counts() {
    let root = temp_root("receipt-policy-missing-evidence");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::create_dir_all(root.join("docs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create docs dir: {err}")));
    fs::write(root.join("docs/policy.md"), "# Policy\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write doc fixture: {err}")));
    fs::write(
        root.join("policy/allow.toml"),
        policy_with_missing_evidence(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));

    let report_output = root.join("target/cargo-allow/check.json");
    let receipt_output = root.join("target/cargo-allow/check.receipt.json");
    let result = cargo_allow_command()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--mode")
        .arg("no-new")
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&report_output)
        .arg("--receipt")
        .arg(&receipt_output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow check: {err}")));

    assert_status("check", &result, true);
    assert_stdout_empty(
        "check",
        &result,
        "--output should not emit report JSON to stdout",
    );
    assert_stderr_empty(
        "check",
        &result,
        "--output and --receipt should not emit side-channel status to stderr",
    );
    let report =
        assert_saved_json_artifact(&report_output, "check", "cargo-allow.report.v1", "check");
    let receipt = assert_saved_json_artifact(
        &receipt_output,
        "check receipt",
        "cargo-allow.receipt.v1",
        "check",
    );

    assert_json_str(&report, "/status", "passed", "report status");
    assert_json_u64(
        &report,
        "/summary/policy_missing_evidence",
        1,
        "report summary policy_missing_evidence",
    );
    assert_json_u64(
        &report,
        "/trend/policy_missing_evidence",
        1,
        "report trend policy_missing_evidence",
    );
    assert_json_str(&receipt, "/status", "passed", "receipt status");
    assert_json_u64(
        &receipt,
        "/counts/policy_missing_evidence",
        1,
        "receipt policy_missing_evidence",
    );
    assert_json_u64(
        &receipt,
        "/advisory/review_items",
        1,
        "receipt advisory review_items",
    );
    assert_json_u64(
        &receipt,
        "/advisory/policy_missing_evidence",
        1,
        "receipt advisory policy_missing_evidence",
    );

    remove_temp_root(root);
}

#[test]
fn check_success_reports_policy_baseline_debt_counts() {
    let root = temp_root("receipt-policy-baseline-debt");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::create_dir_all(root.join("docs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create docs dir: {err}")));
    fs::write(root.join("docs/baseline.md"), "# Baseline\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write baseline doc: {err}")));
    fs::write(root.join("policy/allow.toml"), policy_with_baseline_debt())
        .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));

    let report_output = root.join("target/cargo-allow/check.json");
    let receipt_output = root.join("target/cargo-allow/check.receipt.json");
    let result = cargo_allow_command()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--mode")
        .arg("no-new")
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&report_output)
        .arg("--receipt")
        .arg(&receipt_output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow check: {err}")));

    assert_status("check", &result, true);
    assert_stdout_empty(
        "check",
        &result,
        "--output should not emit report JSON to stdout",
    );
    assert_stderr_empty(
        "check",
        &result,
        "--output and --receipt should not emit side-channel status to stderr",
    );
    let report =
        assert_saved_json_artifact(&report_output, "check", "cargo-allow.report.v1", "check");
    let receipt = assert_saved_json_artifact(
        &receipt_output,
        "check receipt",
        "cargo-allow.receipt.v1",
        "check",
    );

    assert_json_str(&report, "/status", "passed", "report status");
    assert_json_u64(
        &report,
        "/summary/baseline_debt",
        0,
        "report summary baseline_debt",
    );
    assert_json_u64(
        &report,
        "/summary/policy_baseline_debt",
        1,
        "report summary policy_baseline_debt",
    );
    assert_json_u64(
        &report,
        "/trend/baseline_debt",
        1,
        "report trend baseline_debt",
    );
    assert_json_u64(
        &receipt,
        "/counts/baseline_debt",
        0,
        "receipt baseline_debt",
    );
    assert_json_u64(
        &receipt,
        "/counts/policy_baseline_debt",
        1,
        "receipt policy_baseline_debt",
    );

    remove_temp_root(root);
}

#[test]
fn check_failure_with_broken_evidence_still_writes_report_and_receipt() {
    assert_check_failure_reports_broken_evidence(
        "receipt-broken-evidence",
        policy_with_broken_evidence(),
    );
}

#[test]
fn check_failure_with_broken_traceability_link_still_writes_report_and_receipt() {
    assert_check_failure_reports_broken_evidence("receipt-broken-link", policy_with_broken_link());
}

#[test]
fn check_failure_with_invalid_evidence_scope_still_writes_report_and_receipt() {
    assert_check_failure_reports_broken_evidence(
        "receipt-invalid-evidence-scope",
        policy_with_escaping_evidence(),
    );
}

#[test]
fn check_failure_with_untracked_local_evidence_still_writes_report_and_receipt_by_default() {
    let root = temp_root("receipt-untracked-evidence");
    write_git_policy_with_untracked_evidence(&root);

    let report_output = root.join("target/cargo-allow/check.json");
    let receipt_output = root.join("target/cargo-allow/check.receipt.json");
    let result = cargo_allow_command()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--mode")
        .arg("no-new")
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&report_output)
        .arg("--receipt")
        .arg(&receipt_output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow check: {err}")));

    assert_status("check", &result, false);
    assert_stdout_empty(
        "check",
        &result,
        "--output should not emit report JSON to stdout",
    );
    assert_stderr_empty(
        "check",
        &result,
        "--output and --receipt should not emit side-channel status to stderr",
    );
    let report =
        assert_saved_json_artifact(&report_output, "check", "cargo-allow.report.v1", "check");
    let receipt = assert_saved_json_artifact(
        &receipt_output,
        "check receipt",
        "cargo-allow.receipt.v1",
        "check",
    );

    assert_json_str(&report, "/status", "failed", "report status");
    assert_json_u64(
        &report,
        "/summary/broken_evidence_links",
        1,
        "default check should not treat untracked local evidence as present",
    );
    assert_json_u64(
        &receipt,
        "/counts/broken_evidence_links",
        1,
        "default check receipt should not treat untracked local evidence as present",
    );

    remove_temp_root(root);
}

#[test]
fn check_include_untracked_accepts_untracked_local_evidence() {
    let root = temp_root("receipt-include-untracked-evidence");
    write_git_policy_with_untracked_evidence(&root);

    let report_output = root.join("target/cargo-allow/check.json");
    let receipt_output = root.join("target/cargo-allow/check.receipt.json");
    let result = cargo_allow_command()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--mode")
        .arg("no-new")
        .arg("--include-untracked")
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&report_output)
        .arg("--receipt")
        .arg(&receipt_output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow check: {err}")));

    assert_status("check", &result, true);
    assert_stdout_empty(
        "check",
        &result,
        "--output should not emit report JSON to stdout",
    );
    assert_stderr_empty(
        "check",
        &result,
        "--output and --receipt should not emit side-channel status to stderr",
    );
    let report =
        assert_saved_json_artifact(&report_output, "check", "cargo-allow.report.v1", "check");
    let receipt = assert_saved_json_artifact(
        &receipt_output,
        "check receipt",
        "cargo-allow.receipt.v1",
        "check",
    );

    assert_json_str(&report, "/status", "passed", "report status");
    assert!(
        report.pointer("/summary/broken_evidence_links").is_none(),
        "include-untracked check should accept existing untracked local evidence"
    );
    assert!(
        receipt.pointer("/counts/broken_evidence_links").is_none(),
        "include-untracked receipt should accept existing untracked local evidence"
    );

    remove_temp_root(root);
}

fn write_git_policy_with_untracked_evidence(root: &std::path::Path) {
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::write(
        root.join("policy/allow.toml"),
        policy_with_untracked_local_evidence(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
    git(root, &["init"]);
    git(
        root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(root, &["config", "user.name", "cargo-allow test"]);
    git(root, &["add", "policy/allow.toml"]);
    git(root, &["commit", "-m", "base policy"]);
    fs::write(root.join("policy/evidence.md"), "untracked evidence")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write evidence: {err}")));
}

fn assert_check_failure_reports_broken_evidence(fixture: &str, policy: &str) {
    let root = temp_root(fixture);
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::write(root.join("policy/allow.toml"), policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));

    let report_output = root.join("target/cargo-allow/check.json");
    let receipt_output = root.join("target/cargo-allow/check.receipt.json");
    let result = cargo_allow_command()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--mode")
        .arg("no-new")
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&report_output)
        .arg("--receipt")
        .arg(&receipt_output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow check: {err}")));

    assert_status("check", &result, false);
    assert_stdout_empty(
        "check",
        &result,
        "--output should not emit report JSON to stdout",
    );
    assert_stderr_empty(
        "check",
        &result,
        "--output and --receipt should not emit side-channel status to stderr",
    );
    let report =
        assert_saved_json_artifact(&report_output, "check", "cargo-allow.report.v1", "check");
    let receipt = assert_saved_json_artifact(
        &receipt_output,
        "check receipt",
        "cargo-allow.receipt.v1",
        "check",
    );

    assert_json_str(&report, "/status", "failed", "report status");
    assert_json_u64(
        &report,
        "/summary/broken_evidence_links",
        1,
        "report summary broken_evidence_links",
    );
    assert_json_u64(
        &report,
        "/trend/broken_evidence_links",
        1,
        "report trend broken_evidence_links",
    );
    assert_json_str(&receipt, "/status", "failed", "receipt status");
    assert_json_u64(
        &receipt,
        "/counts/broken_evidence_links",
        1,
        "receipt broken_evidence_links",
    );

    remove_temp_root(root);
}

fn git(root: &std::path::Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("git {args:?}: {err}")));
    if !output.status.success() {
        std::panic::panic_any(format!(
            "git {args:?} failed: stdout=`{}` stderr=`{}`",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
}

fn policy() -> &'static str {
    r#"policy = "cargo-allow"

[[allow]]
id = "allow-policy"
kind = "non_rust_file"
family = "configuration"
path = "policy/allow.toml"
owner = "core"
classification = "fixture"
reason = "fixture policy file"
review_after = "2026-08-01"

[allow.selector]
ast_kind = "tracked_file"
symbol = "policy/allow.toml"
target_fingerprint = "toml"
glob = "policy/allow.toml"
"#
}

fn policy_with_untracked_local_evidence() -> &'static str {
    r#"policy = "cargo-allow"

[workspace]
ignored = ["policy/evidence.md"]

[[allow]]
id = "allow-policy"
kind = "non_rust_file"
family = "configuration"
path = "policy/allow.toml"
owner = "core"
classification = "fixture"
reason = "fixture policy file"
evidence = ["doc:policy/evidence.md"]
review_after = "2026-08-01"

[allow.selector]
ast_kind = "tracked_file"
symbol = "policy/allow.toml"
target_fingerprint = "toml"
glob = "policy/allow.toml"
"#
}

fn policy_with_missing_evidence() -> &'static str {
    // Keep reviewed entries matched so this fixture remains date-stable.
    r#"policy = "cargo-allow"

[[allow]]
id = "allow-policy"
kind = "non_rust_file"
family = "configuration"
path = "policy/allow.toml"
owner = "core"
classification = "fixture"
reason = "fixture policy file"
evidence = ["test:check_success_reports_policy_missing_evidence_counts"]
review_after = "2099-01-01"

[allow.selector]
ast_kind = "tracked_file"
symbol = "policy/allow.toml"
target_fingerprint = "toml"
glob = "policy/allow.toml"

[[allow]]
id = "allow-doc"
kind = "non_rust_file"
family = "documentation"
path = "docs/policy.md"
owner = "core"
classification = "fixture"
reason = "fixture policy documentation"
review_after = "2099-01-01"

[allow.selector]
ast_kind = "tracked_file"
symbol = "docs/policy.md"
target_fingerprint = "md"
glob = "docs/policy.md"
"#
}

fn policy_with_baseline_debt() -> &'static str {
    r#"policy = "cargo-allow"

[[allow]]
id = "allow-policy"
kind = "non_rust_file"
family = "configuration"
path = "policy/allow.toml"
owner = "core"
classification = "fixture"
reason = "fixture policy file"
evidence = ["test:check_success_reports_policy_baseline_debt_counts"]
review_after = "2026-08-01"

[allow.selector]
ast_kind = "tracked_file"
symbol = "policy/allow.toml"
target_fingerprint = "toml"
glob = "policy/allow.toml"

[[allow]]
id = "allow-baseline"
kind = "non_rust_file"
family = "documentation"
path = "docs/baseline.md"
owner = "unowned"
classification = "baseline_debt"
reason = "Generated by cargo-allow propose; requires human review."
created = "2026-05-29"
expires = "2026-08-29"

[allow.selector]
ast_kind = "tracked_file"
symbol = "docs/baseline.md"
target_fingerprint = "md"
glob = "docs/baseline.md"
"#
}

fn policy_with_broken_evidence() -> &'static str {
    r#"policy = "cargo-allow"

[[allow]]
id = "allow-policy"
kind = "non_rust_file"
family = "configuration"
path = "policy/allow.toml"
owner = "core"
classification = "fixture"
reason = "fixture policy file"
evidence = ["doc:docs/missing-evidence.md"]
review_after = "2026-08-01"

[allow.selector]
ast_kind = "tracked_file"
symbol = "policy/allow.toml"
target_fingerprint = "toml"
glob = "policy/allow.toml"
"#
}

fn policy_with_broken_link() -> &'static str {
    r#"policy = "cargo-allow"

[[allow]]
id = "allow-policy"
kind = "non_rust_file"
family = "configuration"
path = "policy/allow.toml"
owner = "core"
classification = "fixture"
reason = "fixture policy file"
evidence = ["test:policy_with_broken_link"]
links = ["doc:docs/missing-rationale.md"]
review_after = "2026-08-01"

[allow.selector]
ast_kind = "tracked_file"
symbol = "policy/allow.toml"
target_fingerprint = "toml"
glob = "policy/allow.toml"
"#
}

fn policy_with_escaping_evidence() -> &'static str {
    r#"policy = "cargo-allow"

[[allow]]
id = "allow-policy"
kind = "non_rust_file"
family = "configuration"
path = "policy/allow.toml"
owner = "core"
classification = "fixture"
reason = "fixture policy file"
evidence = ["doc:../outside.md"]
review_after = "2026-08-01"

[allow.selector]
ast_kind = "tracked_file"
symbol = "policy/allow.toml"
target_fingerprint = "toml"
glob = "policy/allow.toml"
"#
}
