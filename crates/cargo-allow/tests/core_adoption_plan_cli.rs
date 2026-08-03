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
    let accidental_workspace_output = PathBuf::from(&output_name);
    if accidental_workspace_output.is_file() {
        fs::remove_file(&accidental_workspace_output).map_err(|error| error.to_string())?;
    }

    remove_temp_root(root)?;
    Ok(())
}

#[test]
fn adopt_projects_findings_without_policy_and_healthy_policy() -> Result<(), String> {
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
    let init = cargo_allow_command()
        .current_dir(&healthy_root)
        .args(["init", "--strict"])
        .output()
        .map_err(|error| format!("run init: {error}"))?;
    require(init.status.success(), "strict init should succeed")?;
    let healthy = run_adopt(&healthy_root, &["--format", "json"])?;
    require(healthy.status.success(), "healthy plan should succeed")?;
    let healthy_artifact = parse_stdout(&healthy)?;
    require(
        healthy_artifact
            .pointer("/plan/bootstrap_disposition")
            .and_then(Value::as_str)
            == Some("ExistingPolicyHealthy"),
        "healthy disposition",
    )?;
    require(
        healthy_artifact
            .pointer("/plan/primary_action/kind")
            .and_then(Value::as_str)
            == Some("ConfigureCi"),
        "healthy action without CI guidance",
    )?;
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
    let invalid_artifact = parse_stdout(&invalid)?;
    require(
        invalid_artifact
            .pointer("/plan/bootstrap_disposition")
            .and_then(Value::as_str)
            == Some("InvalidPolicy"),
        "invalid policy disposition",
    )?;
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
    let contents =
        fs::read_to_string(collision_root.join("README.md")).map_err(|error| error.to_string())?;
    require(
        contents == "preserve me\n",
        "tracked output collision must preserve bytes",
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
