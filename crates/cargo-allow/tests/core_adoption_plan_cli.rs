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
    require(
        healthy_artifact
            .pointer("/plan/follow_up_actions/0/kind")
            .and_then(Value::as_str)
            == Some("RunNoNewCheck"),
        "ci guidance should produce a no-new follow-up",
    )?;
    let include_untracked = run_adopt(&healthy_root, &["--format", "json", "--include-untracked"])?;
    require(
        include_untracked.status.success(),
        "tracked adoption should tolerate include-untracked",
    )?;
    parse_stdout(&include_untracked)?;
    remove_temp_root(healthy_root)?;
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
