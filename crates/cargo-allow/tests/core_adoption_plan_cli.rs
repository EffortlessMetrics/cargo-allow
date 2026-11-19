use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[test]
fn adopt_supports_clean_no_git_repositories_and_human_json_parity() -> Result<(), String> {
    let root = temp_root("adoption-clean-no-git")?;
    write_source(&root, "pub fn value() -> u8 { 1 }\n")?;

    let human = run_adopt(&root, &[])?;
    require(
        human.status.success(),
        "clean human adoption should succeed",
    )?;
    let human_text = String::from_utf8(human.stdout).map_err(|error| error.to_string())?;
    require(
        human_text.contains("Repository state: clean_no_policy"),
        "human disposition",
    )?;
    require(
        human_text.contains("Recommended next step: continue_advisory_audit"),
        "human action",
    )?;
    require(
        human_text.contains("Writes: nothing"),
        "human write posture",
    )?;
    // #4500 pins the documented marker order (docs/getting-started.md): the
    // summary frame — `Outcome:` first, then `Recommended next step:`,
    // `Run:`, `Writes:`, `Rollback:` — leads, and `Repository state:` appears
    // inside the plan body that follows. An agent keying the first line as
    // `Repository state:` (the pre-#4500 wording) would misclassify every
    // healthy adopt run.
    let outcome_position = human_text
        .find("Outcome:")
        .ok_or("human adoption must open with the Outcome frame")?;
    let repository_state_position = human_text
        .find("Repository state:")
        .ok_or("human adoption must carry the repository disposition")?;
    require(
        outcome_position < repository_state_position,
        "the Outcome frame must lead and Repository state must appear in the plan body",
    )?;
    require(
        human_text.contains("Run: ") && human_text.contains("Rollback:"),
        "human adoption must carry the copyable Run command and the rollback statement",
    )?;
    // #4530 pins frame membership, not just ordering: `Subject:` and
    // `Coverage:` are summary-frame lines (before the plan body), while
    // `Recommended next step:`, `Run:`, and `Rollback:` are plan-body lines
    // (after `Repository state:`).
    let subject_position = human_text
        .find("Subject:")
        .ok_or("human adoption must carry the Subject frame line")?;
    let coverage_position = human_text
        .find("Coverage:")
        .ok_or("human adoption must carry the Coverage frame line")?;
    let next_step_position = human_text
        .find("Recommended next step:")
        .ok_or("human adoption must carry exactly the plan's recommended step")?;
    let run_position = human_text
        .find("Run: ")
        .ok_or("human adoption must carry a copyable Run command")?;
    let rollback_position = human_text
        .find("Rollback:")
        .ok_or("human adoption must carry a rollback statement")?;
    require(
        subject_position < repository_state_position
            && coverage_position < repository_state_position,
        "Subject and Coverage are frame lines and must precede the plan body",
    )?;
    require(
        repository_state_position < next_step_position
            && next_step_position < run_position
            && run_position < rollback_position,
        "Recommended next step, Run, and Rollback are plan-body lines after Repository state",
    )?;

    let human_output_name = format!("adoption-human-{}.txt", std::process::id());
    let human_output = run_adopt(&root, &["--output", human_output_name.as_str()])?;
    require(
        human_output.status.success() && human_output.stdout.is_empty(),
        "human output file should succeed without stdout data",
    )?;
    let human_file =
        fs::read_to_string(root.join(&human_output_name)).map_err(|error| error.to_string())?;
    require(
        human_file.contains("Repository state: clean_no_policy"),
        "human output file should use the plain renderer",
    )?;
    fs::remove_file(root.join(human_output_name)).map_err(|error| error.to_string())?;

    let json = run_adopt(&root, &["--format", "json"])?;
    require(json.status.success(), "clean JSON adoption should succeed")?;
    let artifact = parse_stdout(&json)?;
    require(
        artifact
            .pointer("/plan/bootstrap_disposition")
            .and_then(Value::as_str)
            == Some("CleanNoPolicy"),
        "JSON clean disposition",
    )?;
    require(
        artifact
            .pointer("/plan/primary_action/kind")
            .and_then(Value::as_str)
            == Some("ContinueAdvisoryAudit"),
        "JSON clean action",
    )?;
    require(
        artifact
            .pointer("/plan/inventory/mode")
            .and_then(Value::as_str)
            == Some("Filesystem"),
        "no-Git repositories should use filesystem inventory",
    )?;
    require(
        artifact
            .pointer("/plan/selected_root")
            .and_then(Value::as_str)
            == Some("<repository-root>"),
        "JSON should keep the root portable",
    )?;

    let strict = run_adopt(&root, &["--format", "json", "--strict"])?;
    require(
        strict.status.success(),
        "strict clean adoption should succeed",
    )?;
    let strict_artifact = parse_stdout(&strict)?;
    require(
        strict_artifact
            .pointer("/plan/primary_action/kind")
            .and_then(Value::as_str)
            == Some("PreviewInit"),
        "strict clean adoption should preview init",
    )?;

    let output_name = format!("adoption-plan-{}.json", std::process::id());
    let written = run_adopt(
        &root,
        &["--format", "json", "--output", output_name.as_str()],
    )?;
    require(written.status.success(), "JSON output file should succeed")?;
    require(
        written.stdout.is_empty(),
        "JSON output file should not mix data into stdout",
    )?;
    require(
        root.join(&output_name).is_file(),
        "relative output should resolve under --root",
    )?;
    let written_contents =
        fs::read_to_string(root.join(&output_name)).map_err(|error| error.to_string())?;
    serde_json::from_str::<Value>(&written_contents)
        .map_err(|error| format!("JSON output file should parse: {error}"))?;
    require(
        !PathBuf::from(&output_name).is_file(),
        "relative output must not resolve in the caller's working directory",
    )?;

    remove_temp_root(root)?;
    Ok(())
}

#[test]
fn adopt_projects_findings_without_policy_and_ci_guidance() -> Result<(), String> {
    let findings_root = temp_root("adoption-findings-no-policy")?;
    write_source(
        &findings_root,
        "pub fn value() -> u8 { None::<u8>.unwrap() }\n",
    )?;
    let findings = run_adopt(&findings_root, &["--format", "json"])?;
    require(
        findings.status.success(),
        "findings plan should be advisory success",
    )?;
    let findings_artifact = parse_stdout(&findings)?;
    require(
        findings_artifact
            .pointer("/plan/bootstrap_disposition")
            .and_then(Value::as_str)
            == Some("FindingsNoPolicy"),
        "findings disposition",
    )?;
    require(
        findings_artifact
            .pointer("/plan/primary_action/kind")
            .and_then(Value::as_str)
            == Some("PreviewPropose"),
        "findings action",
    )?;
    require_no_adopt_ci_follow_up(&findings_root, &findings_artifact, true)?;
    remove_temp_root(findings_root)?;

    let healthy_root = temp_root("adoption-healthy-policy")?;
    write_source(&healthy_root, "pub fn value() -> u8 { 1 }\n")?;
    fs::create_dir_all(healthy_root.join(".github/workflows"))
        .map_err(|error| error.to_string())?;
    fs::write(
        healthy_root.join(".github/workflows/ci.yml"),
        "cargo-allow check --mode no-new\n",
    )
    .map_err(|error| error.to_string())?;
    git(&healthy_root, &["init"])?;
    git(
        &healthy_root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    )?;
    git(&healthy_root, &["config", "user.name", "cargo-allow test"])?;
    git(&healthy_root, &["add", "."])?;
    git(&healthy_root, &["commit", "-m", "healthy adoption fixture"])?;
    let init = cargo_allow_command()
        .current_dir(&healthy_root)
        .args(["init", "--strict"])
        .output()
        .map_err(|error| format!("run init: {error}"))?;
    require(init.status.success(), "strict init should succeed")?;
    git(&healthy_root, &["add", "."])?;
    git(&healthy_root, &["commit", "-m", "healthy adoption policy"])?;
    let healthy = run_adopt(
        &healthy_root,
        &["--format", "json", "--config", "policy/allow.toml"],
    )?;
    require(healthy.status.success(), "healthy plan should succeed")?;
    let healthy_artifact = parse_stdout(&healthy)?;
    require(
        healthy_artifact
            .pointer("/plan/bootstrap_disposition")
            .and_then(Value::as_str)
            == Some("ExistingPolicyHasNewFindings"),
        "workflow guidance should remain observable alongside the new finding",
    )?;
    require(
        healthy_artifact
            .pointer("/plan/primary_action/kind")
            .and_then(Value::as_str)
            == Some("InspectNewFinding"),
        "new finding should remain the primary action",
    )?;
    require_no_adopt_ci_follow_up(&healthy_root, &healthy_artifact, true)?;
    let absolute_config = run_adopt(
        &healthy_root,
        &[
            "--format",
            "json",
            "--config",
            healthy_root
                .join("policy/allow.toml")
                .to_string_lossy()
                .as_ref(),
        ],
    )?;
    require(
        absolute_config.status.success(),
        "absolute in-root config should succeed",
    )?;
    parse_stdout(&absolute_config)?;
    let include_untracked = run_adopt(&healthy_root, &["--format", "json", "--include-untracked"])?;
    require(
        include_untracked.status.success(),
        "tracked adoption should tolerate include-untracked",
    )?;
    parse_stdout(&include_untracked)?;
    remove_temp_root(healthy_root)?;
    Ok(())
}

fn require_no_adopt_ci_follow_up(
    root: &Path,
    artifact: &Value,
    success: bool,
) -> Result<(), String> {
    require(
        artifact
            .pointer("/plan/follow_up_actions")
            .and_then(Value::as_array)
            .is_some_and(|actions| {
                !actions.iter().any(|action| {
                    matches!(
                        action.get("kind").and_then(Value::as_str),
                        Some("ConfigureCi" | "RunNoNewCheck")
                    )
                })
            }),
        "JSON must not suggest an executable CI follow-up before a healthy policy",
    )?;
    let human = run_adopt(root, &["--color", "never"])?;
    let text = String::from_utf8(human.stdout).map_err(|error| error.to_string())?;
    require(
        human.status.success() == success
            && !text.contains("Then: configure_ci")
            && !text.contains("Then: run_no_new_check")
            && !text.contains("Then: cargo-allow check --mode no-new")
            && !text.contains("CI: ready")
            && text.contains("CI: docs/how-to/adopt-cargo-allow.md#step-3-ci-integration"),
        &format!("human adoption must provide descriptive, not executable, CI guidance: {text}"),
    )
}

#[test]
fn adopt_ci_actions_require_healthy_fresh_evaluation() -> Result<(), String> {
    let root = temp_root("adoption-ci-eligibility")?;
    write_source(&root, "pub fn value() -> u8 { 1 }\n")?;
    git(&root, &["init"])?;
    git(&root, &["add", "src/lib.rs"])?;
    git(
        &root,
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-m",
            "adoption CI eligibility fixture",
        ],
    )?;

    fs::create_dir_all(root.join("policy")).map_err(|error| error.to_string())?;
    let policy_path = root.join("policy/allow.toml");
    let empty_policy = "policy = \"cargo-allow\"\n";
    fs::write(&policy_path, empty_policy).map_err(|error| error.to_string())?;
    let setup = run_adopt(&root, &["--format", "json"])?;
    require(
        setup.status.success(),
        "healthy adoption without CI must succeed",
    )?;
    require(
        parse_stdout(&setup)?
            .pointer("/plan/primary_action/kind")
            .and_then(Value::as_str)
            == Some("ConfigureCi"),
        "healthy policy without CI recommends configuration as the primary action",
    )?;

    write_source(&root, "pub fn value() -> u8 { None::<u8>.unwrap() }\n")?;
    let new_finding = run_adopt(&root, &["--format", "json"])?;
    require(
        new_finding.status.success(),
        "a new finding has an advisory plan",
    )?;
    let new_finding = parse_stdout(&new_finding)?;
    require(
        new_finding
            .pointer("/plan/primary_action/kind")
            .and_then(Value::as_str)
            == Some("InspectNewFinding"),
        "new findings must keep the read-only inspection action",
    )?;
    require_no_adopt_ci_follow_up(&root, &new_finding, true)?;
    write_source(&root, "pub fn value() -> u8 { 1 }\n")?;

    fs::create_dir_all(root.join(".github/workflows")).map_err(|error| error.to_string())?;
    fs::write(
        root.join(".github/workflows/ci.yml"),
        "cargo-allow check --mode no-new\n",
    )
    .map_err(|error| error.to_string())?;
    git(&root, &["add", ".github/workflows/ci.yml"])?;

    let ci_entry = r#"
[[allow]]
id = "fixture-ci"
kind = "non_rust_file"
family = "ci_declarative"
path = ".github/workflows/ci.yml"
owner = "core"
classification = "fixture"
reason = "Exercise the configured no-new route."
created = "2026-01-01"
review_after = "2099-01-01"

[allow.selector]
ast_kind = "tracked_file"
symbol = ".github/workflows/ci.yml"
target_fingerprint = "yml"
glob = ".github/workflows/ci.yml"
"#;
    let base = format!("{empty_policy}{ci_entry}");
    fs::write(&policy_path, &base).map_err(|error| error.to_string())?;
    let configured = run_adopt(&root, &["--format", "json"])?;
    require(
        configured.status.success(),
        "healthy adoption with CI must succeed",
    )?;
    require(
        parse_stdout(&configured)?
            .pointer("/plan/primary_action/kind")
            .and_then(Value::as_str)
            == Some("RunNoNewCheck"),
        "healthy policy with CI recommends its check as the primary action",
    )?;
    let configured_human = run_adopt(&root, &["--color", "never"])?;
    require(
        configured_human.status.success()
            && String::from_utf8_lossy(&configured_human.stdout).contains("CI: ready ("),
        "only the healthy configured plan shows the ready marker",
    )?;

    write_source(&root, "pub fn value() -> u8 { None::<u8>.unwrap() }\n")?;
    let mut tied = base;
    for suffix in ["a", "b"] {
        tied.push_str(&format!(
            r#"
[[allow]]
id = "allow-tied-{suffix}"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core"
classification = "reviewed_exception"
reason = "Equally matching reviewed exception."
evidence = ["test:tied_{suffix}"]
created = "2026-01-01"
review_after = "2099-01-01"

[allow.selector]
ast_kind = "method_call"
callee = "unwrap"
"#,
        ));
    }
    fs::write(&policy_path, tied).map_err(|error| error.to_string())?;
    let diagnostic = run_adopt(&root, &["--format", "json"])?;
    require_exit_code(&diagnostic, 1, "ambiguous policy match")?;
    let diagnostic = parse_stdout(&diagnostic)?;
    require(
        diagnostic
            .pointer("/plan/bootstrap_disposition")
            .and_then(Value::as_str)
            == Some("InvalidPolicy")
            && diagnostic
                .pointer("/plan/policy/state")
                .and_then(Value::as_str)
                == Some("Valid")
            && diagnostic
                .pointer("/plan/inventory/completeness")
                .and_then(Value::as_str)
                == Some("Complete"),
        "loaded valid ledger with ambiguous matches must exercise a policy diagnostic under complete inventory",
    )?;
    require_no_adopt_ci_follow_up(&root, &diagnostic, false)?;
    remove_temp_root(root)
}

#[cfg(unix)]
#[test]
fn adopt_does_not_trust_conventional_policy_after_federation_failure() -> Result<(), String> {
    let root = temp_root("adoption-federation-failure")?;
    fs::create_dir_all(root.join("policy")).map_err(|error| error.to_string())?;
    fs::write(
        root.join("policy/allow.toml"),
        "schema_version = \"0.1\"\npolicy = \"cargo-allow\"\nowner = \"core/policy\"\nstatus = \"active\"\n\n[workspace]\nroot = \".\"\ndefault_mode = \"no-new\"\n",
    )
        .map_err(|error| error.to_string())?;
    fs::create_dir_all(root.join(".allow")).map_err(|error| error.to_string())?;
    fs::write(root.join(".allow/config.toml"), "not valid = [toml\n")
        .map_err(|error| error.to_string())?;

    let result = run_adopt(&root, &["--format", "json", "--strict"])?;
    require(
        !result.status.success(),
        "strict adoption must fail closed when federation evaluation fails",
    )?;
    remove_temp_root(root)?;
    Ok(())
}

#[test]
fn adopt_fails_closed_for_invalid_policy_and_preserves_collision_targets() -> Result<(), String> {
    let invalid_root = temp_root("adoption-invalid-policy")?;
    write_source(&invalid_root, "pub fn value() -> u8 { 1 }\n")?;
    fs::create_dir_all(invalid_root.join("policy")).map_err(|error| error.to_string())?;
    fs::write(invalid_root.join("policy/allow.toml"), "not = [valid")
        .map_err(|error| error.to_string())?;
    let invalid = run_adopt(&invalid_root, &["--format", "json"])?;
    require(
        !invalid.status.success(),
        "invalid policy should fail closed",
    )?;
    require_exit_code(&invalid, 1, "invalid policy")?;
    let invalid_artifact = parse_stdout(&invalid)?;
    require(
        invalid_artifact
            .pointer("/plan/bootstrap_disposition")
            .and_then(Value::as_str)
            == Some("InvalidPolicy"),
        "invalid policy disposition",
    )?;
    let outside_config = invalid_root
        .parent()
        .ok_or_else(|| "invalid fixture should have a parent".to_string())?
        .join("outside-policy.toml");
    let invalid_config = run_adopt(
        &invalid_root,
        &[
            "--format",
            "json",
            "--config",
            outside_config.to_string_lossy().as_ref(),
        ],
    )?;
    require(
        !invalid_config.status.success(),
        "config outside the selected root should fail",
    )?;
    require_exit_code(&invalid_config, 2, "config outside root")?;
    let absolute_output = invalid_root
        .parent()
        .ok_or_else(|| "invalid fixture should have a parent".to_string())?
        .join("adoption-outside.json");
    let outside_output = run_adopt(
        &invalid_root,
        &[
            "--format",
            "json",
            "--output",
            absolute_output.to_string_lossy().as_ref(),
        ],
    )?;
    require(
        !outside_output.status.success(),
        "output outside the selected root should fail",
    )?;
    require_exit_code(&outside_output, 1, "output outside root")?;
    remove_temp_root(invalid_root)?;

    let collision_root = temp_root("adoption-output-collision")?;
    write_source(&collision_root, "pub fn value() -> u8 { 1 }\n")?;
    git(&collision_root, &["init"])?;
    git(
        &collision_root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    )?;
    git(
        &collision_root,
        &["config", "user.name", "cargo-allow test"],
    )?;
    fs::write(collision_root.join("README.md"), "preserve me\n")
        .map_err(|error| error.to_string())?;
    git(&collision_root, &["add", "."])?;
    git(
        &collision_root,
        &["commit", "-m", "tracked collision fixture"],
    )?;
    let collision = run_adopt(
        &collision_root,
        &["--format", "json", "--output", "README.md"],
    )?;
    require(
        !collision.status.success(),
        "tracked output collision should fail",
    )?;
    require_exit_code(&collision, 2, "tracked output collision")?;
    let contents =
        fs::read_to_string(collision_root.join("README.md")).map_err(|error| error.to_string())?;
    require(
        contents == "preserve me\n",
        "tracked output collision must preserve bytes",
    )?;

    fs::create_dir_all(collision_root.join("policy")).map_err(|error| error.to_string())?;
    fs::write(collision_root.join("policy/allow.toml"), "").map_err(|error| error.to_string())?;
    let policy_contents = fs::read_to_string(collision_root.join("policy/allow.toml"))
        .map_err(|error| error.to_string())?;
    let policy_collision = run_adopt(
        &collision_root,
        &["--format", "json", "--output", "policy/allow.toml"],
    )?;
    require(
        !policy_collision.status.success(),
        "repository metadata output collision should fail",
    )?;
    require_exit_code(&policy_collision, 2, "repository metadata collision")?;
    require(
        fs::read_to_string(collision_root.join("policy/allow.toml"))
            .map_err(|error| error.to_string())?
            == policy_contents,
        "output collision must preserve existing bytes",
    )?;
    remove_temp_root(collision_root)?;
    Ok(())
}

#[test]
fn clean_advisory_audit_previews_policy_before_the_first_enforcing_check() -> Result<(), String> {
    let root = temp_root("adoption-first-enforcing-check")?;
    write_source(&root, "pub fn value() -> u8 { 1 }\n")?;
    git(&root, &["init"])?;
    git(&root, &["add", "src/lib.rs"])?;
    let run = |args: &[&str]| -> Result<Output, String> {
        cargo_allow_command()
            .current_dir(&root)
            .args(args)
            .output()
            .map_err(|error| format!("run {args:?}: {error}"))
    };
    let adopt = run(&["adopt"])?;
    require_exit_code(&adopt, 0, "initial adoption remains advisory")?;
    for format in ["human", "markdown", "html"] {
        let audit = run(&["audit", "--format", format])?;
        require_exit_code(&audit, 0, "initial advisory scan")?;
        let text = String::from_utf8_lossy(&audit.stdout);
        require(
            text.contains("cargo-allow init --dry-run") && !text.contains("check --mode no-new"),
            "clean audit must preview setup instead of recommending unavailable enforcement",
        )?;
    }
    let adopt_text = String::from_utf8_lossy(&adopt.stdout);
    require(
        adopt_text.contains("Run: cargo-allow audit") && !adopt_text.contains("Then: configure_ci"),
        "clean adoption hands off to audit without premature setup",
    )?;
    let restricted_audit = run(&["audit", "--kind", "panic"])?;
    require_exit_code(&restricted_audit, 0, "filtered audit")?;
    let restricted_text = String::from_utf8_lossy(&restricted_audit.stdout);
    require(
        !restricted_text.contains("init --dry-run")
            && !restricted_text.contains("check --mode no-new"),
        "a filtered scan must not recommend bootstrapping",
    )?;
    let explicit_audit = run(&["audit", "--config", "missing-custom.toml"])?;
    let explicit_text = String::from_utf8_lossy(&explicit_audit.stdout);
    require(
        !explicit_text.contains("init --dry-run") && !explicit_text.contains("check --mode no-new"),
        "a rejected explicit config is not confirmed canonical absence",
    )?;
    let registry = root.join(".allow/config.toml");
    fs::create_dir_all(root.join(".allow")).map_err(|error| error.to_string())?;
    let duplicate = r#"
[[ledgers]]
id = "source-policy"
path = "policy/allow.toml"
dialect = "cargo-allow"
role = "canonical"
lanes = ["source-exception"]
mode = "blocking"
priority = 10
"#;
    let invalid_registry = format!(r#"schema_version = "1.0"{duplicate}{duplicate}"#);
    for (contents, explanation) in [
        ("mode = [\n", "unparseable federation registry"),
        (
            invalid_registry.as_str(),
            "parsed registry with duplicate ledger IDs",
        ),
    ] {
        fs::write(&registry, contents).map_err(|error| error.to_string())?;
        let report = run(&["audit"])?;
        require_exit_code(&report, 0, explanation)?;
        let text = String::from_utf8_lossy(&report.stdout);
        require(
            !text.contains("init --dry-run") && !text.contains("check --mode no-new"),
            "a present invalid registry must not be treated as absent configuration",
        )?;
    }
    fs::remove_file(registry).map_err(|error| error.to_string())?;
    fs::remove_dir(root.join(".allow")).map_err(|error| error.to_string())?;

    write_source(&root, "pub fn value() -> u8 { None::<u8>.unwrap() }\n")?;
    git(&root, &["add", "src/lib.rs"])?;
    for format in ["human", "markdown", "html", "json"] {
        let audit = run(&["audit", "--format", format])?;
        require_exit_code(
            &audit,
            0,
            "policyless audit with a finding remains advisory",
        )?;
        let text = String::from_utf8_lossy(&audit.stdout);
        require(
            text.contains("src/lib.rs")
                && !text.contains("init --dry-run")
                && !text.contains("check --mode no-new")
                && !text.contains("cargo-allow worklist")
                && !text.contains("cargo-allow why"),
            "policyless audit must retain the finding without emitting unavailable commands",
        )?;
    }
    write_source(&root, "pub fn value() -> u8 { 1 }\n")?;
    git(&root, &["add", "src/lib.rs"])?;
    let preview = run(&["init", "--dry-run"])?;
    require_exit_code(&preview, 0, "explicit non-writing bootstrap preview")?;
    require(
        !root.join("policy/allow.toml").exists(),
        "preview must not write the policy",
    )?;
    let early_check = run(&["check", "--mode", "no-new"])?;
    require_exit_code(
        &early_check,
        1,
        "without intent and init, check still refuses",
    )?;
    require(
        String::from_utf8_lossy(&early_check.stderr).contains("E0002"),
        "no policy must remain an enforcing configuration failure",
    )?;
    let init = run(&["init"])?;
    require_exit_code(&init, 0, "separately requested init")?;
    git(&root, &["add", "policy/allow.toml"])?;
    let post_init = run(&["check", "--mode", "no-new"])?;
    require_exit_code(
        &post_init,
        0,
        "tracked 0.2 starter policy authorizes its own ledger",
    )?;
    let existing_audit = run(&["audit"])?;
    require_exit_code(&existing_audit, 0, "initialized advisory scan")?;
    require(
        !String::from_utf8_lossy(&existing_audit.stdout).contains("init --dry-run"),
        "initialized policy does not advise starting over",
    )?;
    remove_temp_root(root)?;
    Ok(())
}

fn write_source(root: &Path, source: &str) -> Result<(), String> {
    fs::create_dir_all(root.join("src")).map_err(|error| error.to_string())?;
    fs::write(root.join("src/lib.rs"), source).map_err(|error| error.to_string())
}

fn cargo_allow_command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
}

fn temp_root(label: &str) -> Result<PathBuf, String> {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("system clock: {error}"))?
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&root).map_err(|error| format!("create temp root: {error}"))?;
    Ok(root)
}

fn remove_temp_root(root: PathBuf) -> Result<(), String> {
    fs::remove_dir_all(&root)
        .map_err(|error| format!("remove temp root {}: {error}", root.display()))
}

fn run_adopt(root: &Path, args: &[&str]) -> Result<Output, String> {
    let mut command = cargo_allow_command();
    command.arg("adopt").arg("--root").arg(root).args(args);
    command
        .output()
        .map_err(|error| format!("run adopt: {error}"))
}

fn parse_stdout(output: &Output) -> Result<Value, String> {
    serde_json::from_slice(&output.stdout).map_err(|error| {
        format!(
            "adopt JSON should be on stdout: {error}; stderr={}",
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn git(root: &Path, args: &[&str]) -> Result<(), String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|error| format!("git {args:?}: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

fn require(condition: bool, message: &str) -> Result<(), String> {
    condition.then_some(()).ok_or_else(|| message.to_string())
}

fn require_exit_code(output: &Output, expected: i32, label: &str) -> Result<(), String> {
    require(
        output.status.code() == Some(expected),
        &format!(
            "{label} should exit {expected}, got {:?}",
            output.status.code()
        ),
    )
}
