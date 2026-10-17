mod json_assertions;
mod support;
#[test]
fn audit_artifact_emit_failure_fails_with_typed_artifact_error() {
    // #4505: a runtime artifact-emit failure carries the typed
    // error[E0007_ARTIFACT] and the artifact Next hint, not a bare exit 1.
    let root = temp_root("audit-emit-failure-typed");
    fs::write(
        root.join("tracked.txt"),
        "tracked
",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write tracked file: {err}")));
    fs::write(
        root.join("notadir"),
        "a file where a directory must go
",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write collision file: {err}")));
    let output = cargo_allow_command()
        .current_dir(&root)
        .args([
            "audit",
            "--kind",
            "unsafe",
            "--artifact-dir",
            "notadir",
            "--emit",
            "markdown",
        ])
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run audit: {err}")));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "an emit failure must fail: {stderr}"
    );
    assert!(
        stderr.contains("error[E0007_ARTIFACT]: artifact emit:"),
        "the typed artifact error must render: {stderr}"
    );
    assert!(
        stderr.contains("Next:"),
        "the structured Next line must render: {stderr}"
    );
    assert!(
        !stderr.contains("cargo-allow audit: artifact emit:"),
        "the bare untyped stderr line must be gone: {stderr}"
    );
    remove_temp_root(root);
}

#[test]
fn audit_multi_format_emit_writes_every_requested_format() {
    // #4528: `--emit markdown,json` must emit BOTH formats like check, not
    // silently keep only the first comma token.
    let root = temp_root("audit-multi-format-emit");
    fs::write(
        root.join("tracked.txt"),
        "tracked
",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write tracked file: {err}")));
    // A selected policy is required so the artifact set's
    // resolved_config_identity exists; without it the set fails closed as
    // InstrumentFailure under the #4526 validation gate.
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::write(
        root.join("policy/allow.toml"),
        "schema_version = \"0.1\"\npolicy = \"cargo-allow\"\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
    let output = cargo_allow_command()
        .current_dir(&root)
        .args([
            "audit",
            "--kind",
            "unsafe",
            "--artifact-dir",
            "artifacts",
            "--emit",
            "markdown,json",
        ])
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run audit: {err}")));
    assert!(
        output.status.success(),
        "multi-format audit must succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let manifest_text = fs::read_to_string(root.join("artifacts/audit-artifact_set_manifest.json"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read manifest: {err}")));
    assert!(
        manifest_text.contains("\"markdown\"") && manifest_text.contains("\"json\""),
        "both requested formats must be recorded in the manifest: {manifest_text}"
    );
    assert!(root.join("artifacts/audit-markdown.md").is_file());
    assert!(root.join("artifacts/audit-json.json").is_file());
    remove_temp_root(root);
}

#[test]
fn audit_render_failed_emit_fails_with_typed_artifact_error() {
    // #4526: a parser-accepted format whose render fails (`human` has no
    // report renderer) must fail the invocation like check, not exit 0 with
    // a RenderFailed manifest.
    let root = temp_root("audit-render-failed-typed");
    fs::write(
        root.join("tracked.txt"),
        "tracked
",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write tracked file: {err}")));
    let output = cargo_allow_command()
        .current_dir(&root)
        .args([
            "audit",
            "--kind",
            "unsafe",
            "--artifact-dir",
            "artifacts",
            "--emit",
            "human",
        ])
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run audit: {err}")));
    let stderr = String::from_utf8_lossy(&output.stderr);
    // #4596: unrenderable tokens are rejected at parse time (exit 2,
    // E0001_USAGE) instead of parse-accepting a guaranteed RenderFailed
    // after writes.
    assert_eq!(
        output.status.code(),
        Some(2),
        "an unrenderable emit token must be a usage error: {stderr}"
    );
    assert!(
        stderr.contains("unsupported --emit format for audit"),
        "the usage error must name the command and token: {stderr}"
    );
    remove_temp_root(root);
}

#[test]
fn audit_bad_emit_value_fails_with_typed_usage_error() {
    // #4505: the identical --emit mistake on `check` exits 2 with
    // error[E0001_USAGE] and a structured Next line; audit must match instead
    // of a bare "cargo-allow audit: ..." exit 1.
    let root = temp_root("audit-bad-emit-typed");
    fs::write(
        root.join("tracked.txt"),
        "tracked
",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write tracked file: {err}")));
    let output = cargo_allow_command()
        .current_dir(&root)
        .args([
            "audit",
            "--kind",
            "unsafe",
            "--artifact-dir",
            "artifacts",
            "--emit",
            "bogus-format",
        ])
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run audit: {err}")));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "a bad --emit value must fail: {stderr}"
    );
    assert_eq!(
        output.status.code(),
        Some(2),
        "usage mistakes exit 2 like check: {stderr}"
    );
    assert!(
        stderr.contains("error[E0001_USAGE]: unsupported --emit format"),
        "the typed usage error must render: {stderr}"
    );
    assert!(
        stderr.contains("Next:"),
        "the structured Next line must render: {stderr}"
    );
    assert!(
        !stderr.contains("cargo-allow audit: unsupported"),
        "the bare untyped stderr line must be gone: {stderr}"
    );
    remove_temp_root(root);
}

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

fn policy_with_baseline_debt() -> String {
    // The baseline entry must stay live for the asserted audit counts, and
    // baseline_debt expiry is validated against the 120-day window from
    // creation, so both dates are computed relative to today instead of
    // hardcoded calendar days the test would eventually sail past. (Mirrors
    // receipt_output::policy_with_baseline_debt.)
    let created = allow_core::SimpleDate::today_utc_approx().add_days(-30);
    let expires = allow_core::SimpleDate::today_utc_approx().add_days(30);
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
created = "{created}"
expires = "{expires}"

[allow.selector]
ast_kind = "tracked_file"
symbol = "docs/baseline.md"
target_fingerprint = "md"
glob = "docs/baseline.md"
"#
    )
}

#[test]
fn audit_receipt_emit_fails_typed_not_panicked() {
    // #4546: the shared parser accepts `receipt` for every command, but
    // receipt artifacts are contract-bound to check/diff. audit must fail
    // through the typed artifact path, not panic at the renderer's assert
    // (exit 101) after already printing a green report.
    let root = temp_root("audit-receipt-emit-typed");
    fs::write(
        root.join("tracked.txt"),
        "tracked
",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write tracked file: {err}")));
    // A selected policy keeps the failure on the receipt render, not the
    // structural InstrumentFailure gate.
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::write(
        root.join("policy/allow.toml"),
        "schema_version = \"0.1\"\npolicy = \"cargo-allow\"\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
    let output = cargo_allow_command()
        .current_dir(&root)
        .args([
            "audit",
            "--kind",
            "unsafe",
            "--artifact-dir",
            "artifacts",
            "--emit",
            "receipt",
        ])
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run audit: {err}")));
    let stderr = String::from_utf8_lossy(&output.stderr);
    // #4596: `receipt` can never render for audit, so the token is
    // rejected at parse time (exit 2) instead of parse-accepting a
    // guaranteed RenderFailed after writes.
    assert_eq!(
        output.status.code(),
        Some(2),
        "an unrenderable receipt emit must be a usage error: {stderr}"
    );
    assert!(
        stderr.contains("unsupported --emit format for audit"),
        "the usage error must name the command and token: {stderr}"
    );
    remove_temp_root(root);
}

#[test]
fn audit_artifact_dir_may_not_contain_the_selected_policy() {
    // #4561: emitting artifacts beside the live policy is a real collision
    // the OutputConflict classification could never see; the invocation
    // must refuse it typed before any write.
    let root = temp_root("audit-artifact-dir-policy-collision");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::write(
        root.join("policy/allow.toml"),
        "schema_version = \"0.1\"
policy = \"cargo-allow\"
",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
    fs::write(
        root.join("tracked.txt"),
        "tracked
",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write tracked file: {err}")));
    let output = cargo_allow_command()
        .current_dir(&root)
        .args([
            "audit",
            "--kind",
            "unsafe",
            "--config",
            "policy/allow.toml",
            "--artifact-dir",
            "policy",
            "--emit",
            "markdown",
            "--output",
            "report.md",
        ])
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run audit: {err}")));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "a policy-colliding artifact dir must fail: {stderr}"
    );
    assert!(
        stderr.contains("would contain the selected policy file"),
        "the typed error must name the collision: {stderr}"
    );
    assert!(
        !root.join("policy/audit-markdown.md").is_file(),
        "no artifact may be written into the policy directory"
    );
    assert!(
        !root.join("report.md").is_file(),
        "no success-shaped report may survive the refusal"
    );
    remove_temp_root(root);
}

#[test]
fn audit_emit_without_artifact_dir_fails_at_arg_parse() {
    // #4548: a lone --emit used to be a silent no-op (exit 0, nothing
    // emitted); the pairing must be refused at argument parsing instead.
    let root = temp_root("audit-emit-requires-dir");
    fs::write(
        root.join("tracked.txt"),
        "tracked
",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write tracked file: {err}")));
    let output = cargo_allow_command()
        .current_dir(&root)
        .args(["audit", "--kind", "unsafe", "--emit", "json"])
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run audit: {err}")));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(2),
        "the missing sibling flag must be a usage error: {stderr}"
    );
    assert!(
        stderr.contains("--artifact-dir"),
        "the usage error must name the required sibling flag: {stderr}"
    );
    assert!(
        !root.join("artifacts").exists(),
        "no artifact directory may be created for a rejected invocation"
    );
    remove_temp_root(root);
}

#[test]
fn audit_instrument_failure_next_does_not_suggest_dropping_formats() {
    // #4551: the drop-the-format remedy only fits RenderFailure. Structural
    // gaps (here: no selected policy, so resolved_config_identity is empty)
    // must point at the manifest's gaps instead of an impossible format fix.
    let root = temp_root("audit-instrument-failure-next");
    fs::write(
        root.join("tracked.txt"),
        "tracked
",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write tracked file: {err}")));
    let output = cargo_allow_command()
        .current_dir(&root)
        .args([
            "audit",
            "--kind",
            "unsafe",
            "--artifact-dir",
            "artifacts",
            "--emit",
            "markdown,json",
        ])
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run audit: {err}")));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "an InstrumentFailure emit must fail: {stderr}"
    );
    assert!(
        stderr.contains("InstrumentFailure"),
        "the structural outcome must be named: {stderr}"
    );
    assert!(
        stderr.contains(
            "Next: inspect the failing gaps recorded in the artifact-set manifest, resolve the reported condition, then retry."
        ),
        "the structural failure must type its manifest-gap next step: {stderr}"
    );
    assert!(
        !stderr.contains("drop the failing format"),
        "the format remedy must not misdirect structural recovery: {stderr}"
    );
    // #4560: the Next line points at the manifest's recorded gaps, so the
    // persisted manifest must carry them.
    let manifest_text = fs::read_to_string(root.join("artifacts/audit-artifact_set_manifest.json"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read manifest: {err}")));
    let manifest: serde_json::Value = serde_json::from_str(&manifest_text)
        .unwrap_or_else(|err| std::panic::panic_any(format!("manifest must be JSON: {err}")));
    let gaps = manifest
        .get("validation_gaps")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("manifest must record validation_gaps"));
    assert!(
        gaps.iter()
            .any(|gap| { gap.as_str() == Some("resolved_config_identity is missing") }),
        "the manifest must record the failing gap: {manifest_text}"
    );
    remove_temp_root(root);
}

#[test]
fn audit_artifact_dir_without_emit_fails_at_arg_parse() {
    // #4564: a lone --artifact-dir used to be a silent no-op (exit 0,
    // nothing emitted), the inverse asymmetry of the lone --emit rejection;
    // the pairing must be refused at argument parsing instead.
    let root = temp_root("audit-artifact-dir-requires-emit");
    fs::write(
        root.join("tracked.txt"),
        "tracked
",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write tracked file: {err}")));
    let output = cargo_allow_command()
        .current_dir(&root)
        .args(["audit", "--kind", "unsafe", "--artifact-dir", "artifacts"])
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run audit: {err}")));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(2),
        "the missing sibling flag must be a usage error: {stderr}"
    );
    assert!(
        stderr.contains("--emit"),
        "the usage error must name the required sibling flag: {stderr}"
    );
    assert!(
        !root.join("artifacts").exists(),
        "no artifact directory may be created for a rejected invocation"
    );
    remove_temp_root(root);
}

#[test]
fn audit_compat_guard_protects_the_legacy_policy() {
    // #4576 review + codecov/patch: under --compat the run loads a legacy
    // policy file (here the network allowlist), and the artifact-collision
    // guard must protect THAT file; this also exercises the compat arm of
    // the guard and the compat path helper end to end.
    let root = temp_root("audit-compat-guard");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::write(
        root.join("policy/network-allowlist.toml"),
        r#"schema_version = 1
policy = "network-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "net-crates-io-fetch"
destination = "crates.io"
auth_required = false
lane = "build"
owner = "release"
reason = "cargo fetch resolves and downloads crate dependencies."
created = "2026-05-09"
expires = "permanent"
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write legacy policy: {err}")));
    fs::write(root.join("tracked.txt"), "tracked\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write tracked file: {err}")));

    let output = cargo_allow_command()
        .current_dir(&root)
        .args([
            "audit",
            "--compat",
            "--kind",
            "network",
            "--config",
            "policy/network-allowlist.toml",
            "--artifact-dir",
            "policy",
            "--emit",
            "markdown",
        ])
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run compat audit: {err}")));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "a legacy-policy-colliding artifact dir must fail: {stderr}"
    );
    assert!(
        stderr.contains("would contain the selected policy file"),
        "the typed error must name the collision: {stderr}"
    );
    assert!(
        !root.join("policy/audit-markdown.md").is_file(),
        "no artifact may be written into the legacy policy directory"
    );

    // The non-collision side succeeds: artifacts outside the legacy policy
    // directory are emitted normally.
    let output = cargo_allow_command()
        .current_dir(&root)
        .args([
            "audit",
            "--compat",
            "--kind",
            "network",
            "--config",
            "policy/network-allowlist.toml",
            "--artifact-dir",
            "artifacts",
            "--emit",
            "markdown",
        ])
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run compat audit: {err}")));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "a non-colliding compat emit must succeed: {stderr}"
    );
    assert!(root.join("artifacts/audit-markdown.md").is_file());

    remove_temp_root(root);
}
