mod json_assertions;
mod support;

use std::fs;

use json_assertions::{assert_json_str, assert_json_u64};
use support::{
    assert_saved_json_artifact, assert_status, assert_stderr_empty, assert_stdout_empty,
    cargo_allow_command, remove_temp_root, temp_root,
};

#[test]
fn audit_with_output_file_does_not_emit_human_status_to_stderr() {
    let root = temp_root("audit-output");
    fs::write(root.join("tracked.txt"), "tracked\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write tracked file: {err}")));
    let output = root.join("audit.json");

    let result = cargo_allow_command()
        .arg("audit")
        .arg("--root")
        .arg(&root)
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow audit: {err}")));

    assert_status("audit", &result, true);
    assert_stdout_empty(
        "audit",
        &result,
        "--output should not emit report JSON to stdout",
    );
    assert_stderr_empty(
        "audit",
        &result,
        "--output should not emit human status to stderr",
    );
    assert_saved_json_artifact(&output, "audit", "cargo-allow.report.v1", "audit");

    remove_temp_root(root);
}

#[test]
fn audit_with_broken_evidence_writes_saved_report_counts() {
    let root = temp_root("audit-broken-evidence-output");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::write(
        root.join("policy/allow.toml"),
        policy_with_broken_evidence(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
    let output = root.join("audit.json");

    let result = cargo_allow_command()
        .arg("audit")
        .arg("--root")
        .arg(&root)
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow audit: {err}")));

    assert_status("audit", &result, true);
    assert_stdout_empty(
        "audit",
        &result,
        "--output should not emit report JSON to stdout",
    );
    assert_stderr_empty(
        "audit",
        &result,
        "--output should not emit human status to stderr",
    );
    let report = assert_saved_json_artifact(&output, "audit", "cargo-allow.report.v1", "audit");
    assert_json_u64(
        &report,
        "/summary/broken_evidence_links",
        1,
        "audit summary broken_evidence_links",
    );
    assert_json_u64(
        &report,
        "/trend/broken_evidence_links",
        1,
        "audit trend broken_evidence_links",
    );

    remove_temp_root(root);
}

#[test]
fn audit_reports_policy_missing_evidence_counts() {
    let root = temp_root("audit-policy-missing-evidence");
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
    let output = root.join("audit.json");

    let result = cargo_allow_command()
        .arg("audit")
        .arg("--root")
        .arg(&root)
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow audit: {err}")));

    assert_status("audit", &result, true);
    assert_stdout_empty(
        "audit",
        &result,
        "--output should not emit report JSON to stdout",
    );
    assert_stderr_empty(
        "audit",
        &result,
        "--output should not emit human status to stderr",
    );
    let report = assert_saved_json_artifact(&output, "audit", "cargo-allow.report.v1", "audit");
    assert_json_u64(
        &report,
        "/summary/policy_missing_evidence",
        1,
        "audit summary policy_missing_evidence",
    );
    assert_json_u64(
        &report,
        "/trend/policy_missing_evidence",
        1,
        "audit trend policy_missing_evidence",
    );

    remove_temp_root(root);
}

#[test]
fn audit_reports_policy_baseline_debt_counts() {
    let root = temp_root("audit-policy-baseline-debt");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::create_dir_all(root.join("docs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create docs dir: {err}")));
    fs::write(root.join("docs/baseline.md"), "# Baseline\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write baseline doc: {err}")));
    fs::write(root.join("policy/allow.toml"), policy_with_baseline_debt())
        .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
    let output = root.join("audit.json");

    let result = cargo_allow_command()
        .arg("audit")
        .arg("--root")
        .arg(&root)
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow audit: {err}")));

    assert_status("audit", &result, true);
    assert_stdout_empty(
        "audit",
        &result,
        "--output should not emit report JSON to stdout",
    );
    assert_stderr_empty(
        "audit",
        &result,
        "--output should not emit human status to stderr",
    );
    let report = assert_saved_json_artifact(&output, "audit", "cargo-allow.report.v1", "audit");
    assert_json_u64(
        &report,
        "/summary/baseline_debt",
        0,
        "audit summary baseline_debt",
    );
    assert_json_u64(
        &report,
        "/summary/policy_baseline_debt",
        1,
        "audit summary policy_baseline_debt",
    );
    assert_json_u64(
        &report,
        "/trend/baseline_debt",
        1,
        "audit trend baseline_debt",
    );

    remove_temp_root(root);
}

#[test]
fn audit_scans_rust_when_package_manifest_is_not_utf8() {
    let root = temp_root("audit-non-utf8-manifest");
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create source dir: {err}")));
    fs::write(
        root.join("Cargo.toml"),
        b"[package]\nname = \"broken\"\n\xFF",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write non-utf8 manifest: {err}")));
    fs::write(
        root.join("src/lib.rs"),
        "pub fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write rust source: {err}")));
    let output = root.join("audit.json");

    let result = cargo_allow_command()
        .arg("audit")
        .arg("--root")
        .arg(&root)
        .arg("--kind")
        .arg("panic")
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow audit: {err}")));

    assert_status("audit", &result, true);
    assert_stdout_empty(
        "audit",
        &result,
        "--output should not emit report JSON to stdout",
    );
    assert_stderr_empty(
        "audit",
        &result,
        "--output should not emit human status to stderr",
    );
    let report = assert_saved_json_artifact(&output, "audit", "cargo-allow.report.v1", "audit");
    assert_json_u64(
        &report,
        "/summary/findings",
        1,
        "audit should still scan panic finding",
    );
    assert_json_str(
        &report,
        "/findings/0/path",
        "src/lib.rs",
        "audit finding path",
    );
    assert_eq!(
        report.pointer("/findings/0/source_package"),
        None,
        "invalid manifest text should not provide package context"
    );

    remove_temp_root(root);
}

#[test]
fn audit_scans_rust_when_package_manifest_is_invalid_toml() {
    let root = temp_root("audit-invalid-manifest");
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create source dir: {err}")));
    fs::write(root.join("Cargo.toml"), "[package\nname = \"broken\"\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write invalid manifest: {err}")));
    fs::write(
        root.join("src/lib.rs"),
        "pub fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write rust source: {err}")));
    let output = root.join("audit.json");

    let result = cargo_allow_command()
        .arg("audit")
        .arg("--root")
        .arg(&root)
        .arg("--kind")
        .arg("panic")
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow audit: {err}")));

    assert_status("audit", &result, true);
    assert_stdout_empty(
        "audit",
        &result,
        "--output should not emit report JSON to stdout",
    );
    assert_stderr_empty(
        "audit",
        &result,
        "--output should not emit human status to stderr",
    );
    let report = assert_saved_json_artifact(&output, "audit", "cargo-allow.report.v1", "audit");
    assert_json_u64(
        &report,
        "/summary/findings",
        1,
        "audit should still scan panic finding",
    );
    assert_json_str(
        &report,
        "/findings/0/path",
        "src/lib.rs",
        "audit finding path",
    );
    assert_eq!(
        report.pointer("/findings/0/source_package"),
        None,
        "invalid manifest TOML should not provide package context"
    );

    remove_temp_root(root);
}

#[test]
fn audit_scans_invalid_rust_without_package_manifest() {
    let root = temp_root("audit-invalid-rust-no-manifest");
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create source dir: {err}")));
    fs::write(
        root.join("src/lib.rs"),
        "pub fn load(value: Option<u8>) -> u8 { value.unwrap()",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write invalid rust source: {err}")));
    let output = root.join("audit.json");

    let result = cargo_allow_command()
        .arg("audit")
        .arg("--root")
        .arg(&root)
        .arg("--kind")
        .arg("panic")
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow audit: {err}")));

    assert_status("audit", &result, true);
    assert_stdout_empty(
        "audit",
        &result,
        "--output should not emit report JSON to stdout",
    );
    assert_stderr_empty(
        "audit",
        &result,
        "--output should not emit human status to stderr",
    );
    let report = assert_saved_json_artifact(&output, "audit", "cargo-allow.report.v1", "audit");
    assert_json_str(
        &report,
        "/inventory/source",
        "filesystem_fallback",
        "audit should not require git or Cargo metadata",
    );
    assert_json_u64(
        &report,
        "/summary/findings",
        1,
        "audit should scan the visible panic finding in invalid Rust",
    );
    assert_json_str(
        &report,
        "/findings/0/path",
        "src/lib.rs",
        "audit finding path",
    );
    assert_json_str(
        &report,
        "/findings/0/family",
        "unwrap",
        "audit should classify the visible panic-family call",
    );
    assert_eq!(
        report.pointer("/findings/0/source_package"),
        None,
        "source package should remain absent without Cargo.toml"
    );

    remove_temp_root(root);
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
evidence = ["test:audit_reports_policy_missing_evidence_counts"]
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
evidence = ["test:audit_reports_policy_baseline_debt_counts"]
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
