//! End-to-end proof that the supported first-hour commands present one
//! operator grammar (#3149 PR A).
//!
//! These run the real binary rather than the in-process adapters, so they also
//! cover argv routing, `--command-summary-output` acceptance, and the human/machine
//! parity an automation consumer depends on.

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Every human summary block in the shared grammar, in order.
const GRAMMAR_FIELDS: [&str; 8] = [
    "Result:",
    "Why:",
    "Subject:",
    "Coverage:",
    "Next:",
    "Writes:",
    "Then:",
    "Not proven:",
];

/// Argv for the inspection commands, which need a subject argument to have
/// anything to inspect.
const EXPLAIN_ARGV: &[&str] = &["explain", "allow-0001"];
const WHY_ARGV: &[&str] = &[
    "why",
    "--kind",
    "panic",
    "--path",
    "src/lib.rs",
    "--line",
    "1",
];
const WORKLIST_ARGV: &[&str] = &["worklist"];

/// Every command that projects the summary.
const GRAMMAR_COMMANDS: [&[&str]; 6] = [
    &["adopt"],
    &["doctor"],
    &["audit"],
    EXPLAIN_ARGV,
    WHY_ARGV,
    WORKLIST_ARGV,
];

#[test]
fn first_hour_commands_share_one_operator_grammar() -> Result<(), String> {
    let root = temp_root("summary-grammar")?;
    // `explain` and `why` need a ledger and an unreceipted finding to inspect,
    // so the fixture carries both. `adopt`, `doctor`, `audit`, and `worklist`
    // are unaffected by their presence.
    write_source(&root, "pub fn value(v: Option<u8>) -> u8 { v.unwrap() }\n")?;
    run(&root, &["init"])?;

    for command in GRAMMAR_COMMANDS {
        let output = run(&root, command)?;
        let text = stdout(&output)?;
        // Walk the fields in order over an advancing suffix. `split_at` keeps
        // this free of panic-family slicing, which this repository's own
        // source-exception ledger tracks.
        let mut rest = text.as_str();
        for field in GRAMMAR_FIELDS {
            let found = rest.find(field).ok_or_else(|| {
                format!("`{command:?}` human output is missing `{field}` in order; got:\n{text}")
            })?;
            let (_, tail) = rest.split_at(found + field.len());
            rest = tail;
        }
    }

    remove_temp_root(root)
}

#[test]
fn inspection_commands_emit_a_read_only_summary_sidecar() -> Result<(), String> {
    let root = temp_root("summary-inspection")?;
    write_source(&root, "pub fn value(v: Option<u8>) -> u8 { v.unwrap() }\n")?;
    run(&root, &["init"])?;

    for (label, command) in [
        ("explain", EXPLAIN_ARGV),
        ("why", WHY_ARGV),
        ("worklist", WORKLIST_ARGV),
    ] {
        let sidecar = root.join(format!("{label}-summary.json"));
        let mut argv = vec!["--command-summary-output"];
        let sidecar_text = sidecar.to_string_lossy().to_string();
        argv.push(&sidecar_text);
        argv.extend(command.iter().copied());
        let text = stdout(&run(&root, &argv)?)?;
        let summary: Value = serde_json::from_str(
            &fs::read_to_string(&sidecar)
                .map_err(|error| format!("read {label} summary: {error}"))?,
        )
        .map_err(|error| format!("parse {label} summary: {error}"))?;

        require(
            field(&summary, &["operation"]) == Some(&Value::from(label)),
            format!("{label} summary must name its own operation"),
        )?;
        let reason = field(&summary, &["reason", "message"])
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{label} summary needs a human reason"))?;
        require(
            text.contains(reason),
            format!("{label} human `Why:` must match the summary reason"),
        )?;
        // None of these three writes anything without `why --plan`.
        require(
            field(&summary, &["operation_effects", "writes_repository"])
                == Some(&Value::Bool(false)),
            format!("{label} is read-only"),
        )?;
        require(
            text.contains("Writes: nothing in this operation"),
            format!("{label} must state its read-only posture in the summary"),
        )?;
    }

    remove_temp_root(root)
}

#[test]
fn why_plan_reports_the_candidate_write_it_performed() -> Result<(), String> {
    let root = temp_root("summary-why-plan")?;
    write_source(&root, "pub fn value(v: Option<u8>) -> u8 { v.unwrap() }\n")?;
    run(&root, &["init"])?;
    // An add-finding plan requires an exact evaluation, which requires a
    // committed Git inventory rather than the filesystem fallback.
    git_commit_fixture(&root)?;

    let sidecar = root.join("why-summary.json");
    let sidecar_text = sidecar.to_string_lossy().to_string();
    let plan = root.join("add-finding.plan.json");
    let plan_text = plan.to_string_lossy().to_string();
    let mut argv = vec!["--command-summary-output", &sidecar_text];
    argv.extend(WHY_ARGV.iter().copied());
    argv.push("--plan");
    argv.push(&plan_text);
    run(&root, &argv)?;

    require(
        plan.exists(),
        "why --plan must write its candidate artifact",
    )?;
    let summary: Value = serde_json::from_str(
        &fs::read_to_string(&sidecar).map_err(|error| format!("read why summary: {error}"))?,
    )
    .map_err(|error| format!("parse why summary: {error}"))?;
    require(
        field(&summary, &["operation_effects", "writes_repository"]) == Some(&Value::Bool(true)),
        "why --plan is not read-only",
    )?;
    require(
        field(&summary, &["operation_effects", "write_paths"])
            == Some(&Value::from(vec!["add-finding.plan.json"])),
        format!(
            "the summary must name the exact plan path, got {:?}",
            field(&summary, &["operation_effects", "write_paths"])
        ),
    )?;

    remove_temp_root(root)
}

#[test]
fn adopt_and_doctor_agree_between_human_and_summary_artifacts() -> Result<(), String> {
    let root = temp_root("summary-parity")?;
    write_source(&root, "pub fn value() -> u8 { 1 }\n")?;

    for command in ["adopt", "doctor"] {
        let sidecar = root.join(format!("{command}-summary.json"));
        let output = run(
            &root,
            &[
                "--command-summary-output",
                &sidecar.to_string_lossy(),
                command,
            ],
        )?;
        let text = stdout(&output)?;
        let summary: Value = serde_json::from_str(
            &fs::read_to_string(&sidecar)
                .map_err(|error| format!("read {command} summary: {error}"))?,
        )
        .map_err(|error| format!("parse {command} summary: {error}"))?;

        require(
            field(&summary, &["schema_id"])
                == Some(&Value::from("cargo-allow.core-command-summary.v1")),
            format!("{command} summary must carry the versioned schema ID"),
        )?;
        require(
            field(&summary, &["operation"]) == Some(&Value::from(command)),
            format!("{command} summary must name its own operation"),
        )?;
        // The load-bearing discriminator is the machine field, and the human
        // reason text must not disagree with it.
        let reason = field(&summary, &["reason", "message"])
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{command} summary needs a human reason"))?;
        require(
            text.contains(reason),
            format!("{command} human `Why:` must match the summary reason"),
        )?;
        require(
            field(&summary, &["operation_effects", "writes_repository"])
                == Some(&Value::Bool(false)),
            format!("{command} is read-only"),
        )?;
        require(
            text.contains("Writes: nothing in this operation"),
            format!("{command} must state its read-only posture in the summary"),
        )?;
    }

    remove_temp_root(root)
}

#[test]
fn adopt_and_doctor_preserve_their_detailed_artifacts() -> Result<(), String> {
    let root = temp_root("summary-detail-preserved")?;
    write_source(&root, "pub fn value() -> u8 { 1 }\n")?;

    // The summary is additive: the pre-existing detailed human sections and the
    // command-specific JSON artifacts must survive the migration unchanged.
    let adopt = stdout(&run(&root, &["adopt"])?)?;
    require(
        adopt.contains("Repository state:") && adopt.contains("Schema:"),
        "adopt must keep its detailed plan section",
    )?;

    let doctor = stdout(&run(&root, &["doctor"])?)?;
    require(
        doctor.contains("source tree root:") && doctor.contains("inventory:"),
        "doctor must keep its detailed diagnosis section",
    )?;

    // JSON stays the command's own artifact, not the summary projection.
    let adopt_json: Value =
        serde_json::from_str(&stdout(&run(&root, &["adopt", "--format", "json"])?)?)
            .map_err(|error| format!("adopt JSON: {error}"))?;
    require(
        field(&adopt_json, &["schema_id"])
            == Some(&Value::from("cargo-allow.core-adoption-plan.v1")),
        "adopt --format json must remain the adoption plan artifact",
    )?;

    remove_temp_root(root)
}

#[test]
fn summary_output_is_rejected_for_unmigrated_commands() -> Result<(), String> {
    let root = temp_root("summary-unmigrated")?;
    write_source(&root, "pub fn value() -> u8 { 1 }\n")?;
    let sidecar = root.join("summary.json");

    let output = run(
        &root,
        &[
            "--command-summary-output",
            &sidecar.to_string_lossy(),
            "vocabulary",
        ],
    )?;
    require(
        !output.status.success(),
        "an unmigrated command must reject --command-summary-output rather than silently ignore it",
    )?;
    require(
        !sidecar.exists(),
        "a rejected --command-summary-output must not leave a partial artifact behind",
    )?;

    remove_temp_root(root)
}

#[test]
fn doctor_summary_identity_is_stable_across_repository_relocation() -> Result<(), String> {
    // Same content, two locations: the portable identity an automation
    // consumer keys on must not depend on where the checkout lives.
    let mut identities = Vec::new();
    let mut roots = Vec::new();
    for label in ["relocation-left", "relocation-right"] {
        let root = temp_root(label)?;
        write_source(&root, "pub fn value() -> u8 { 1 }\n")?;
        let sidecar = root.join("summary.json");
        run(
            &root,
            &[
                "--command-summary-output",
                &sidecar.to_string_lossy(),
                "doctor",
            ],
        )?;
        let summary: Value = serde_json::from_str(
            &fs::read_to_string(&sidecar).map_err(|error| format!("read summary: {error}"))?,
        )
        .map_err(|error| format!("parse summary: {error}"))?;
        let identity = field(&summary, &["subject", "repository_identity"])
            .and_then(Value::as_str)
            .ok_or("summary needs a repository identity")?
            .to_string();
        require(
            !identity.contains(&root.to_string_lossy().to_string()),
            "the repository identity must not embed a private absolute path",
        )?;
        identities.push(identity);
        roots.push(root);
    }

    let mut distinct = identities.clone();
    distinct.dedup();
    require(
        distinct.len() == 1,
        format!("relocated identity drifted: {identities:?}"),
    )?;

    for root in roots {
        remove_temp_root(root)?;
    }
    Ok(())
}

/// Read a nested JSON field without panicking-index syntax.
fn field<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for key in path {
        current = current.get(key)?;
    }
    Some(current)
}

/// Run the real binary. `args` carries any global flags plus the subcommand, in
/// that order; `--root` is appended because it belongs to the subcommand.
fn run(root: &Path, args: &[&str]) -> Result<Output, String> {
    Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
        .args(args)
        .arg("--root")
        .arg(root)
        .output()
        .map_err(|error| format!("run {args:?}: {error}"))
}

fn stdout(output: &Output) -> Result<String, String> {
    String::from_utf8(output.stdout.clone()).map_err(|error| error.to_string())
}

/// Commit the fixture so the Git inventory, not the filesystem fallback,
/// backs the scan.
fn git_commit_fixture(root: &Path) -> Result<(), String> {
    for args in [
        vec!["init", "-q"],
        vec!["add", "-A"],
        vec![
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "user.name=fixture",
            "commit",
            "-q",
            "-m",
            "fixture",
        ],
    ] {
        let output = Command::new("git")
            .current_dir(root)
            .args(&args)
            .output()
            .map_err(|error| format!("git {args:?}: {error}"))?;
        require(
            output.status.success(),
            format!(
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        )?;
    }
    Ok(())
}

fn write_source(root: &Path, source: &str) -> Result<(), String> {
    fs::create_dir_all(root.join("src")).map_err(|error| error.to_string())?;
    fs::write(root.join("src/lib.rs"), source).map_err(|error| error.to_string())
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

fn require(condition: bool, message: impl Into<String>) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}
