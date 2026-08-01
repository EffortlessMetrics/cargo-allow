mod json_assertions;
mod support;

use std::fs;

use allow_core::SimpleDate;
use json_assertions::{assert_json_str, assert_json_u64};
use support::{
    assert_saved_json_artifact, assert_status, assert_stderr_empty, assert_stdout_empty,
    cargo_allow_command, remove_temp_root, temp_root,
};

#[test]
fn advisory_only_check_passes_without_deny() {
    let root = temp_root("deny-advisory-only-pass");
    write_policy_missing_evidence_fixture(&root);

    let result = cargo_allow_command()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--mode")
        .arg("no-new")
        .arg("--format")
        .arg("json")
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow check: {err}")));

    assert_status("check", &result, true);
    assert_stderr_empty(
        "check",
        &result,
        "advisory-only check should stay quiet on stderr",
    );
    let report = serde_json::from_slice::<serde_json::Value>(&result.stdout)
        .unwrap_or_else(|err| std::panic::panic_any(format!("check stdout should be JSON: {err}")));
    assert_json_str(&report, "/status", "passed", "report status");
    assert_json_u64(
        &report,
        "/trend/policy_missing_evidence",
        1,
        "report policy_missing_evidence trend",
    );

    remove_temp_root(root);
}

#[test]
fn advisory_only_check_fails_when_denied_status_count_is_positive() {
    let root = temp_root("deny-advisory-only-fail");
    write_policy_missing_evidence_fixture(&root);

    let result = cargo_allow_command()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--mode")
        .arg("no-new")
        .arg("--deny")
        .arg("policy_missing_evidence")
        .arg("--format")
        .arg("json")
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow check: {err}")));

    assert_status("check", &result, false);
    assert_stderr_empty(
        "check",
        &result,
        "deny escalation should surface in the report",
    );
    let report = serde_json::from_slice::<serde_json::Value>(&result.stdout)
        .unwrap_or_else(|err| std::panic::panic_any(format!("check stdout should be JSON: {err}")));
    assert_json_str(&report, "/status", "failed", "report status");
    assert_json_u64(
        &report,
        "/trend/policy_missing_evidence",
        1,
        "report policy_missing_evidence trend",
    );

    remove_temp_root(root);
}

#[test]
fn check_receipt_records_advisory_count_used_by_deny() {
    let root = temp_root("deny-receipt-advisory");
    write_policy_missing_evidence_fixture(&root);
    let receipt_output = root.join("target/cargo-allow/check.receipt.json");

    let result = cargo_allow_command()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--mode")
        .arg("no-new")
        .arg("--deny")
        .arg("policy_missing_evidence")
        .arg("--receipt")
        .arg(&receipt_output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow check: {err}")));

    assert_status("check", &result, false);
    assert_stdout_empty(
        "check",
        &result,
        "--receipt should not emit report JSON to stdout",
    );
    let receipt = assert_saved_json_artifact(
        &receipt_output,
        "check receipt",
        "cargo-allow.receipt.v1",
        "check",
    );
    assert_json_str(&receipt, "/status", "failed", "receipt status");
    assert_json_u64(
        &receipt,
        "/advisory/policy_missing_evidence",
        1,
        "receipt advisory policy_missing_evidence",
    );

    remove_temp_root(root);
}

#[test]
fn advisory_only_check_fails_when_denied_occurrence_headroom_is_positive() {
    let root = temp_root("deny-occurrence-headroom-fail");
    write_counted_headroom_fixture(&root);
    let receipt_output = root.join("target/cargo-allow/check.receipt.json");

    let result = cargo_allow_command()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--config")
        .arg("policy/allow.toml")
        .arg("--kind")
        .arg("non-rust")
        .arg("--mode")
        .arg("no-new")
        .arg("--deny")
        .arg("occurrence_headroom")
        .arg("--receipt")
        .arg(&receipt_output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow check: {err}")));

    assert_status("check", &result, false);
    assert_stdout_empty(
        "check",
        &result,
        "--receipt should not emit report JSON to stdout",
    );
    let receipt = assert_saved_json_artifact(
        &receipt_output,
        "check receipt",
        "cargo-allow.receipt.v1",
        "check",
    );
    assert_json_str(&receipt, "/status", "failed", "receipt status");
    assert_json_u64(
        &receipt,
        "/advisory/occurrence_headroom",
        1,
        "receipt advisory occurrence_headroom",
    );

    remove_temp_root(root);
}

fn write_counted_headroom_fixture(root: &std::path::Path) {
    let today = SimpleDate::today_utc_approx();
    let created = today;
    let review_after = today.add_days(30);
    let expires = today.add_days(90);
    std::fs::create_dir_all(root.join("crates/alpha"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create alpha dir: {err}")));
    std::fs::write(
        root.join("crates/alpha/Cargo.toml"),
        "[package]\nname = \"alpha\"\nversion = \"0.1.0\"\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write alpha manifest: {err}")));
    std::fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    std::fs::write(
        root.join("policy/allow.toml"),
        format!(
            r#"policy = "cargo-allow"

[[allow]]
id = "allow-manifest"
kind = "non_rust_file"
family = "package_metadata"
glob = "crates/*/Cargo.toml"
owner = "core"
classification = "baseline_debt"
reason = "fixture counted baseline headroom"
occurrence_limit = 2
evidence = ["test:deny_escalation_output"]
created = "{created}"
review_after = "{review_after}"
expires = "{expires}"

[allow.selector]
ast_kind = "tracked_file"
target_fingerprint = "toml"
glob = "crates/*/Cargo.toml"
"#
        ),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
}

fn write_policy_missing_evidence_fixture(root: &std::path::Path) {
    let review_after = SimpleDate::today_utc_approx().add_days(30);
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::create_dir_all(root.join("docs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create docs dir: {err}")));
    fs::write(root.join("docs/policy.md"), "# Policy\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write doc fixture: {err}")));
    fs::write(
        root.join("policy/allow.toml"),
        format!(
            r#"policy = "cargo-allow"

[[allow]]
id = "allow-policy"
kind = "non_rust_file"
family = "configuration"
path = "policy/allow.toml"
owner = "core"
classification = "fixture"
reason = "fixture policy file"
evidence = ["test:deny_escalation_output"]
created = "2026-06-01"
review_after = "{review_after}"

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
created = "2026-06-01"
review_after = "{review_after}"

[allow.selector]
ast_kind = "tracked_file"
symbol = "docs/policy.md"
target_fingerprint = "md"
glob = "docs/policy.md"
"#
        ),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
}
