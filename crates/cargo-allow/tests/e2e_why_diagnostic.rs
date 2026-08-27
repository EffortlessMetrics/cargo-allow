mod support;

use std::error::Error;
use std::fs;
use std::path::Path;
use std::process::Command;

use allow_core::SOURCE_FILE_READ_MAX_BYTES;
use serde_json::Value;
use support::{
    assert_saved_json_artifact, assert_status, assert_stderr_empty, assert_stdout_empty,
    cargo_allow_command, remove_temp_root, temp_root,
};

/// The `why` command's primary diagnostic mode (non-`--plan`) has zero
/// subprocess e2e coverage — only `--plan` output is tested via
/// `add_finding_plan_output.rs`. This test exercises the full integration
/// seam: scanner finds the source → select_add_finding → evaluate →
/// related_mismatch_candidates → JSON output.
#[test]
fn why_diagnostic_reports_unreceipted_finding_with_near_miss_candidates() {
    let root = temp_root("e2e-why-diagnostic");
    write_source_fixture(&root);
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);

    // Init a policy with a near-miss entry: same kind (panic), different
    // selector (different callee) so it doesn't match the finding.
    let policy = r#"schema_version = "0.1"
policy = "cargo-allow"

[requirements]
owner_required = true
reason_required = true
classification_required = true
evidence_required = false
expires_or_review_after_required = true
stale_entries_fail = false
allow_bare_allow_attributes = false
lint_policy_id_required = false

[requirements.unsafe]
evidence_required = true
safety_comment_required = false

[[allow]]
id = "allow-near-miss-unwrap"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core"
classification = "reviewed_exception"
reason = "Different unwrap that was reviewed."
evidence = ["test:near_miss"]
created = "2026-01-01"
review_after = "2027-01-01"

[allow.selector]
ast_kind = "method_call"
callee = "expect"
"#;
    fs::write(root.join("policy/allow.toml"), policy)
        .unwrap_or_else(|err| panic!("write policy: {err}"));

    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "fixture with near-miss entry"]);

    let why_output = root.join("target/cargo-allow/why.json");
    let why = cargo_allow_command()
        .arg("why")
        .arg("--root")
        .arg(&root)
        .arg("--kind")
        .arg("panic")
        .arg("--path")
        .arg("src/lib.rs")
        .arg("--line")
        .arg("1")
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&why_output)
        .output()
        .unwrap_or_else(|err| panic!("run why: {err}"));

    assert_status("why", &why, true);
    assert_stdout_empty("why", &why, "--output should not emit JSON to stdout");
    assert_stderr_empty(
        "why",
        &why,
        "--output should not emit side-channel status to stderr",
    );

    let report = assert_saved_json_artifact(&why_output, "why", "cargo-allow.why.v1", "why");

    assert_eq!(
        report.pointer("/evaluation/scope").and_then(Value::as_str),
        Some("scoped"),
        "path-scoped policy should keep the narrow why evaluation"
    );
    assert_eq!(
        report
            .pointer("/evaluation/locality")
            .and_then(Value::as_str),
        Some("proven")
    );

    // The finding should be "new" (unreceipted) since the near-miss entry
    // has callee=expect but the finding is unwrap.
    assert_eq!(
        report.pointer("/outcome/status").and_then(Value::as_str),
        Some("new"),
        "why should report the finding as new/unreceipted"
    );

    // Finding coordinates
    assert_eq!(
        report.pointer("/finding/kind").and_then(Value::as_str),
        Some("panic"),
        "why should report the finding kind"
    );
    let finding_path = report
        .pointer("/finding/path")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        finding_path.ends_with("src/lib.rs"),
        "finding path should end with src/lib.rs: {finding_path}"
    );

    // The near-miss entry should appear as a candidate with mismatch reasons
    let candidates = report
        .pointer("/candidate_entries")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| std::panic::panic_any("candidate_entries should be an array"));
    assert!(
        !candidates.is_empty(),
        "why should list near-miss candidates for an unreceipted finding"
    );
    let first_candidate_id = candidates
        .first()
        .and_then(|c| c.pointer("/id"))
        .and_then(Value::as_str)
        .unwrap_or("");
    assert_eq!(
        first_candidate_id, "allow-near-miss-unwrap",
        "first candidate should be the near-miss entry"
    );

    // Next steps should include a proof plan to receipt the finding
    let proof_plans = report
        .pointer("/next/proof_plans")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| std::panic::panic_any("next.proof_plans should be an array"));
    assert!(
        !proof_plans.is_empty(),
        "why should suggest proof plans for receipting"
    );
    assert_eq!(
        proof_plans
            .first()
            .and_then(|p| p.pointer("/program"))
            .and_then(Value::as_str),
        Some("cargo-allow"),
        "first proof plan should use cargo-allow"
    );

    let subdir_output = root.join("target/cargo-allow/why-subdir.json");
    let mut from_subdir = cargo_allow_command();
    from_subdir.current_dir(root.join("src"));
    let subdir_why = from_subdir
        .args([
            "why", "--kind", "panic", "--path", "lib.rs", "--line", "1", "--format", "json",
            "--output",
        ])
        .arg(&subdir_output)
        .output()
        .unwrap_or_else(|err| panic!("run why from subdirectory: {err}"));
    assert_status("why from subdirectory", &subdir_why, true);
    let subdir_report = assert_saved_json_artifact(
        &subdir_output,
        "why from subdirectory",
        "cargo-allow.why.v1",
        "why",
    );
    assert_eq!(
        subdir_report
            .pointer("/finding/path")
            .and_then(Value::as_str),
        Some("src/lib.rs")
    );

    remove_temp_root(root);
}

#[test]
fn why_diagnostic_falls_back_for_broad_policy_scope() {
    let root = temp_root("e2e-why-fallback");
    fs::create_dir_all(root.join("src/nested"))
        .unwrap_or_else(|err| panic!("create src dir: {err}"));
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| panic!("create policy dir: {err}"));
    fs::write(
        root.join("src/nested/lib.rs"),
        "pub fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| panic!("write source: {err}"));
    fs::write(
        root.join("policy/allow.toml"),
        r#"schema_version = "0.1"
policy = "cargo-allow"

[requirements]
owner_required = true
reason_required = true
classification_required = true
evidence_required = false
expires_or_review_after_required = true
stale_entries_fail = false
allow_bare_allow_attributes = false
lint_policy_id_required = false

[requirements.unsafe]
evidence_required = true
safety_comment_required = false

[[allow]]
id = "allow-broad-unwrap"
kind = "panic"
family = "unwrap"
glob = "src/**/*.rs"
owner = "core"
classification = "reviewed_exception"
reason = "Broad fixture entry"
evidence = ["test:broad"]
created = "2026-01-01"
review_after = "2027-01-01"

[allow.selector]
ast_kind = "method_call"
callee = "unwrap"
"#,
    )
    .unwrap_or_else(|err| panic!("write policy: {err}"));
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "fixture with broad scope"]);

    let why_output = root.join("target/cargo-allow/why.json");
    let why = cargo_allow_command()
        .args(["why", "--root"])
        .arg(&root)
        .args([
            "--kind",
            "panic",
            "--path",
            "src/nested/lib.rs",
            "--line",
            "1",
            "--format",
            "json",
            "--output",
        ])
        .arg(&why_output)
        .output()
        .unwrap_or_else(|err| panic!("run why fallback: {err}"));
    assert_status("why fallback", &why, true);

    let report =
        assert_saved_json_artifact(&why_output, "why fallback", "cargo-allow.why.v1", "why");
    assert_eq!(
        report.pointer("/evaluation/scope").and_then(Value::as_str),
        Some("full_fallback")
    );
    assert_eq!(
        report
            .pointer("/evaluation/locality")
            .and_then(Value::as_str),
        Some("global_dependency")
    );
    assert!(
        report
            .pointer("/evaluation/reasons/0")
            .and_then(Value::as_str)
            .is_some_and(|reason| reason.contains("broad path scope")),
        "fallback should explain the broad policy dependency"
    );

    remove_temp_root(root);
}

#[test]
fn why_diagnostic_explains_inventory_exclusion_and_accepts_include_untracked() {
    let root = temp_root("e2e-why-untracked");
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| panic!("create src dir: {err}"));
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| panic!("create policy dir: {err}"));
    fs::write(
        root.join("src/untracked.rs"),
        "pub fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| panic!("write source: {err}"));
    fs::write(
        root.join("policy/allow.toml"),
        r#"schema_version = "0.1"
policy = "cargo-allow"

[requirements]
owner_required = true
reason_required = true
classification_required = true
evidence_required = false
expires_or_review_after_required = true
stale_entries_fail = false
allow_bare_allow_attributes = false
lint_policy_id_required = false

[requirements.unsafe]
evidence_required = true
safety_comment_required = false

[[allow]]
id = "allow-untracked-unwrap"
kind = "panic"
family = "unwrap"
path = "src/untracked.rs"
owner = "core"
classification = "reviewed_exception"
reason = "Untracked fixture entry"
evidence = ["test:untracked"]
created = "2026-01-01"
review_after = "2027-01-01"

[allow.selector]
ast_kind = "method_call"
callee = "unwrap"
"#,
    )
    .unwrap_or_else(|err| panic!("write policy: {err}"));
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "policy/allow.toml"]);
    git(&root, &["commit", "-m", "fixture with untracked source"]);

    let missing = cargo_allow_command()
        .args([
            "why",
            "--root",
            root.to_str().unwrap_or(""),
            "--kind",
            "panic",
            "--path",
            "src/untracked.rs",
            "--line",
            "1",
        ])
        .output()
        .unwrap_or_else(|err| panic!("run why without untracked: {err}"));
    assert_status("why without untracked", &missing, false);
    let missing_stderr = String::from_utf8_lossy(&missing.stderr);
    assert!(
        missing_stderr.contains("not present in the source inventory"),
        "missing inventory diagnostic should explain the exclusion: {missing_stderr}"
    );

    let why_output = root.join("target/cargo-allow/why-untracked.json");
    let included = cargo_allow_command()
        .args([
            "why",
            "--root",
            root.to_str().unwrap_or(""),
            "--include-untracked",
            "--kind",
            "panic",
            "--path",
            "src/untracked.rs",
            "--line",
            "1",
            "--format",
            "json",
            "--output",
        ])
        .arg(&why_output)
        .output()
        .unwrap_or_else(|err| panic!("run why with untracked: {err}"));
    assert_status("why with untracked", &included, true);
    let report = assert_saved_json_artifact(
        &why_output,
        "why with untracked",
        "cargo-allow.why.v1",
        "why",
    );
    assert_eq!(
        report.pointer("/evaluation/scope").and_then(Value::as_str),
        Some("scoped")
    );

    remove_temp_root(root);
}

#[test]
fn why_diagnostic_reports_skipped_target_without_inventing_a_finding() -> Result<(), Box<dyn Error>>
{
    let root = temp_root("e2e-why-target-skipped");
    write_source_fixture(&root);
    fs::write(
        root.join("src/large.rs"),
        vec![b' '; (SOURCE_FILE_READ_MAX_BYTES as usize).saturating_add(1)],
    )?;
    fs::write(
        root.join("policy/allow.toml"),
        r#"schema_version = "0.1"
policy = "cargo-allow"

[requirements]
owner_required = true
reason_required = true
classification_required = true
evidence_required = false
expires_or_review_after_required = true
stale_entries_fail = false
allow_bare_allow_attributes = false
lint_policy_id_required = false

[requirements.unsafe]
evidence_required = true
safety_comment_required = false
"#,
    )?;
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "fixture with skipped target"]);

    let output_path = root.join("target/cargo-allow/why.json");
    let result = cargo_allow_command()
        .args([
            "why",
            "--root",
            root.to_str().ok_or("root is not UTF-8")?,
            "--kind",
            "panic",
            "--path",
            "src/large.rs",
            "--line",
            "1",
            "--format",
            "json",
            "--output",
        ])
        .arg(&output_path)
        .output()?;
    if !result.status.success() {
        return Err(format!("why failed: {}", String::from_utf8_lossy(&result.stderr)).into());
    }
    let report: Value = serde_json::from_str(&fs::read_to_string(&output_path)?)?;
    for (pointer, expected) in [
        ("/evaluation/result_class", "target_scanner_partial"),
        ("/evaluation/scanner_completeness", "partial"),
        ("/target/status", "skipped"),
    ] {
        if report.pointer(pointer).and_then(Value::as_str) != Some(expected) {
            return Err(format!("{pointer} did not equal {expected}: {report}").into());
        }
    }
    for pointer in ["/finding", "/outcome"] {
        if !report.pointer(pointer).is_some_and(Value::is_null) {
            return Err(format!("{pointer} should be null: {report}").into());
        }
    }
    let proof_plans_missing_or_nonempty = match report
        .pointer("/next/proof_plans")
        .and_then(Value::as_array)
    {
        None => true,
        Some(plans) => !plans.is_empty(),
    };
    if proof_plans_missing_or_nonempty {
        return Err(format!("skipped target should not emit proof plans: {report}").into());
    }
    remove_temp_root(root);
    Ok(())
}

fn write_source_fixture(root: &Path) {
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| panic!("create src dir: {err}"));
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| panic!("create policy dir: {err}"));
    fs::write(
        root.join("src/lib.rs"),
        "pub fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| panic!("write source fixture: {err}"));
}

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap_or_else(|err| panic!("git {args:?}: {err}"));
    if !output.status.success() {
        panic!(
            "git {args:?} failed: stdout=`{}` stderr=`{}`",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
