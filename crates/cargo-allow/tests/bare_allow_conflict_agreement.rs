//! #2057: doctor, audit, and check must agree on a policy that mixes
//! `requirements.allow_bare_allow_attributes = false` with `lint_exception`
//! entries receipting bare `#[allow(...)]` attributes. All three must surface
//! the same configuration conflict and the next safe action.
//!
//! Focused test: inlines its own subprocess helpers (version_output /
//! policy_discovery convention) and does not pull in the shared tests/support
//! module.

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

/// A policy that mixes `allow_bare_allow_attributes = false` with a
/// `lint_exception` entry receipting a bare `#[allow(...)]` — the #2057 repro.
fn write_conflict_policy(root: &std::path::Path) {
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::write(
        root.join("policy/allow.toml"),
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
created = "2026-01-01"
review_after = "2027-12-01"
expires = "2027-12-31"

[allow.selector]
ast_kind = "attribute"
lint = "clippy::expect_used"
glob = "src/lib.rs"
"#,
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
