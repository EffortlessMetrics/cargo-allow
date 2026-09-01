//! #2057: doctor, audit, and check must agree on a policy that mixes
//! `requirements.allow_bare_allow_attributes = false` with `lint_exception`
//! entries receipting bare `#[allow(...)]` attributes. All three must surface
//! the same configuration conflict and the next safe action.
//!
//! Focused test: inlines its own subprocess helpers (version_output /
//! policy_discovery convention) and does not pull in the shared tests/support
//! module.

use allow_policy::{ConfigCandidateSourceV1, resolve_cargo_allow_config_v1};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn cargo_allow_command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
}

fn temp_root(label: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("create temp root: {err}")));
    root
}

fn remove_temp_root(root: PathBuf) {
    match fs::remove_dir_all(&root) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => std::panic::panic_any(format!("remove temp root {}: {err}", root.display())),
    }
}

fn require(condition: bool, message: &str) -> Result<(), String> {
    condition.then_some(()).ok_or_else(|| message.to_string())
}

/// A policy that mixes `allow_bare_allow_attributes = false` with a
/// `lint_exception` entry receipting a bare `#[allow(...)]` — the #2057 repro.
fn write_conflict_policy(root: &std::path::Path) {
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    // The entry must keep covering the bare-allow finding for the check/add
    // agreement, so its lifecycle dates are computed relative to today
    // instead of hardcoded calendar days the test would eventually sail
    // past.
    let created = allow_core::SimpleDate::today_utc_approx().add_days(-30);
    let review_after = allow_core::SimpleDate::today_utc_approx().add_days(30);
    let expires = allow_core::SimpleDate::today_utc_approx().add_days(60);
    fs::write(
        root.join("policy/allow.toml"),
        format!(
            r#"schema_version = "0.1"
policy = "cargo-allow"
owner = "core"
status = "active"

[requirements]
allow_bare_allow_attributes = false

[[allow]]
id = "allow-bare-allow"
kind = "lint_exception"
family = "allow_attribute"
path = "src/lib.rs"
owner = "core"
classification = "reviewed_exception"
reason = "receipt the bare allow"
evidence = ["test:fixture"]
created = "{created}"
review_after = "{review_after}"
expires = "{expires}"

[allow.selector]
ast_kind = "attribute"
lint = "clippy::expect_used"
glob = "src/lib.rs"
"#
        ),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
}

/// All three commands (doctor, audit, check) must fail and surface the same
/// bare-allow configuration conflict with the next-safe-action hint.
#[test]
fn doctor_audit_check_agree_on_bare_allow_conflict() {
    let root = temp_root("bare-allow-conflict-agreement");
    write_conflict_policy(&root);
    let cfg = "policy/allow.toml";
    let conflict = "configuration conflict";
    let next_action = "allow_bare_allow_attributes = true";

    let doctor = cargo_allow_command()
        .arg("doctor")
        .arg("--root")
        .arg(&root)
        .arg("--config")
        .arg(cfg)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run doctor: {err}")));
    // doctor reports health and (without --require-clean) exits 0 even when the
    // config is invalid; the agreement contract is that it REPORTS the conflict
    // (config.valid=false + the diagnostic), not a matching exit code.
    assert!(
        doctor.status.success(),
        "doctor (plain) should still exit 0 reporting health; stderr=`{}`",
        String::from_utf8_lossy(&doctor.stderr)
    );
    let doctor_combined =
        String::from_utf8_lossy(&doctor.stdout) + String::from_utf8_lossy(&doctor.stderr);
    assert!(
        doctor_combined.contains(conflict),
        "doctor should name the conflict: `{doctor_combined}`"
    );
    assert!(
        doctor_combined.contains(next_action),
        "doctor should state the next safe action: `{doctor_combined}`"
    );

    // `doctor --require-clean` must fail (CI gate) on the invalid config.
    let doctor_gate = cargo_allow_command()
        .arg("doctor")
        .arg("--root")
        .arg(&root)
        .arg("--config")
        .arg(cfg)
        .arg("--require-clean")
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run doctor --require-clean: {err}")));
    assert!(
        !doctor_gate.status.success(),
        "doctor --require-clean should fail on the invalid config; stderr=`{}`",
        String::from_utf8_lossy(&doctor_gate.stderr)
    );

    let audit = cargo_allow_command()
        .arg("audit")
        .arg("--root")
        .arg(&root)
        .arg("--config")
        .arg(cfg)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run audit: {err}")));
    assert!(
        !audit.status.success(),
        "audit should fail on the bare-allow conflict; stderr=`{}`",
        String::from_utf8_lossy(&audit.stderr)
    );
    let audit_combined =
        String::from_utf8_lossy(&audit.stdout) + String::from_utf8_lossy(&audit.stderr);
    assert!(
        audit_combined.contains(conflict),
        "audit should name the conflict: `{audit_combined}`"
    );

    let check = cargo_allow_command()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--config")
        .arg(cfg)
        .arg("--mode")
        .arg("no-new")
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run check: {err}")));
    assert!(
        !check.status.success(),
        "check should fail on the bare-allow conflict; stderr=`{}`",
        String::from_utf8_lossy(&check.stderr)
    );
    let check_combined =
        String::from_utf8_lossy(&check.stdout) + String::from_utf8_lossy(&check.stderr);
    assert!(
        check_combined.contains(conflict),
        "check should name the conflict: `{check_combined}`"
    );

    remove_temp_root(root);
}

/// When `allow_bare_allow_attributes = true`, the same entry is NOT a conflict
/// — the three commands must not false-positive. (doctor should run clean.)
#[test]
fn no_conflict_when_bare_allows_explicitly_allowed() {
    let root = temp_root("bare-allow-allowed");
    write_conflict_policy(&root);
    // Flip the requirement so bare allows are explicitly permitted.
    let policy_path = root.join("policy/allow.toml");
    let policy = fs::read_to_string(&policy_path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read policy: {err}")));
    let policy = policy.replace(
        "allow_bare_allow_attributes = false",
        "allow_bare_allow_attributes = true",
    );
    fs::write(&policy_path, policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));

    let doctor = cargo_allow_command()
        .arg("doctor")
        .arg("--root")
        .arg(&root)
        .arg("--config")
        .arg("policy/allow.toml")
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run doctor: {err}")));
    assert!(
        doctor.status.success(),
        "doctor should pass when bare allows are explicitly allowed; stderr=`{}`",
        String::from_utf8_lossy(&doctor.stderr)
    );

    remove_temp_root(root);
}

/// The central resolved-config adapter and the diagnostic command must identify
/// the same explicit policy candidate. This characterizes current behavior
/// without changing selection.
#[test]
fn central_resolution_matches_command_policy_identity() -> Result<(), String> {
    let root = temp_root("resolved-config-command-characterization");
    write_conflict_policy(&root);
    let resolved = resolve_cargo_allow_config_v1(
        &root,
        Some(std::path::Path::new("policy/allow.toml")),
        "test:resolved-config-command-characterization",
    )
    .map_err(|error| format!("resolve config: {error}"))?;
    require(
        resolved.selection_source == Some(ConfigCandidateSourceV1::CliOverride),
        "central resolver should select the explicit CLI candidate",
    )?;
    let root_text = root.to_str().ok_or("root is not UTF-8")?;
    let doctor = cargo_allow_command()
        .args([
            "doctor",
            "--root",
            root_text,
            "--config",
            "policy/allow.toml",
            "--format",
            "json",
        ])
        .output()
        .map_err(|error| format!("run doctor: {error}"))?;
    let artifact: Value = serde_json::from_slice(&doctor.stdout)
        .map_err(|error| format!("parse doctor JSON: {error}"))?;
    let doctor_path = artifact
        .pointer("/config/path")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("doctor should report a config path: {artifact}"))?;
    let expected_path = root
        .join("policy/allow.toml")
        .canonicalize()
        .map_err(|error| format!("canonicalize expected policy: {error}"))?;
    let observed_path = std::path::Path::new(doctor_path)
        .canonicalize()
        .map_err(|error| format!("canonicalize doctor policy: {error}"))?;
    require(
        observed_path == expected_path
            && resolved
                .selected_policy
                .as_ref()
                .is_some_and(|policy| policy.path.path == "policy/allow.toml"),
        &format!("doctor should report the same explicit policy identity: {artifact}"),
    )?;
    require(
        artifact
            .pointer("/config/provenance/source")
            .and_then(Value::as_str)
            == Some("cli_override"),
        "doctor should preserve explicit CLI provenance",
    )?;
    remove_temp_root(root);
    Ok(())
}
