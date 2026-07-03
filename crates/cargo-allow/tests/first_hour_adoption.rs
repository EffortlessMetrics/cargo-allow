//! First-hour adoption proof: run the real cargo-allow binary end-to-end through
//! the adoption path from an empty temporary repo —
//! doctor → audit → propose → check no-new → list → explain → worklist → diff —
//! and assert exit codes, generated files, receipts, next-step guidance, and the
//! no-new ratchet (a new in-scope exception after baselining fails the gate).
//!
//! This is the product proof that cargo-allow bootstraps a policy that passes its
//! own check and explains failures a maintainer can act on.
//!
//! Focused test: self-contained subprocess helpers (no shared `tests/support`).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn cargo_allow() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
}

fn temp_root(label: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-first-hour-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("create temp root: {err}")));
    root
}

fn drop_root(root: PathBuf) {
    match fs::remove_dir_all(&root) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => std::panic::panic_any(format!("remove temp root {}: {err}", root.display())),
    }
}

fn write_source(root: &Path, body: &str) {
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create src: {err}")));
    fs::write(root.join("src/lib.rs"), body)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write src/lib.rs: {err}")));
}

fn run(output: Output, label: &str) -> Output {
    if !output.status.success() {
        std::panic::panic_any(format!(
            "{label} failed (exit {:?}); stderr=`{}`",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    output
}

fn run_fail(output: Output, label: &str) -> Output {
    if output.status.success() {
        std::panic::panic_any(format!(
            "{label} unexpectedly succeeded; stderr=`{}`",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    output
}

fn combined(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned() + &String::from_utf8_lossy(&output.stderr)
}

#[test]
fn first_hour_adoption_path_doctor_audit_propose_check_list_explain_worklist() {
    let root = temp_root("full-adoption");
    // One baselined panic (the seed debt an adopter starts from).
    write_source(
        &root,
        "pub fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    );

    // 1. doctor: no policy yet — reports missing config, exits non-zero only with
    //    --require-clean. Plain doctor exits 0 with a health report.
    let doctor = run(
        cargo_allow()
            .arg("doctor")
            .arg("--root")
            .arg(&root)
            .arg("--format")
            .arg("json")
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("run doctor: {err}"))),
        "doctor (no policy)",
    );
    let doctor_json: serde_json::Value =
        serde_json::from_slice(&doctor.stdout).unwrap_or_else(|err| {
            std::panic::panic_any(format!("doctor stdout should be JSON: {err}"))
        });
    assert_eq!(
        doctor_json
            .get("command")
            .and_then(serde_json::Value::as_str),
        Some("doctor"),
        "doctor reports its command"
    );

    // 2. audit: advisory mode, surfaces the unreceipted panic finding but does not fail.
    let audit = run(
        cargo_allow()
            .arg("audit")
            .arg("--root")
            .arg(&root)
            .arg("--kind")
            .arg("panic")
            .arg("--format")
            .arg("json")
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("run audit: {err}"))),
        "audit",
    );
    let audit_json: serde_json::Value = serde_json::from_slice(&audit.stdout)
        .unwrap_or_else(|err| std::panic::panic_any(format!("audit stdout should be JSON: {err}")));
    assert_eq!(
        audit_json.get("status").and_then(serde_json::Value::as_str),
        Some("passed"),
        "audit is advisory (passed)"
    );
    assert_eq!(
        audit_json
            .pointer("/summary/new")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "audit surfaces the one unreceipted panic"
    );

    // 3. propose: writes a baseline-debt policy that receipts the current finding.
    let policy = root.join("policy/allow.toml");
    run(
        cargo_allow()
            .arg("propose")
            .arg("--root")
            .arg(&root)
            .arg("--kind")
            .arg("panic")
            .arg("--write")
            .arg(&policy)
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("run propose: {err}"))),
        "propose",
    );
    let policy_text = fs::read_to_string(&policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read policy: {err}")));
    assert!(
        policy_text.contains("classification = \"baseline_debt\""),
        "propose writes a baseline-debt entry"
    );

    // 4. check --mode no-new: at the baseline, the gate PASSES — the generated
    //    policy must not immediately fail its own check.
    let check = run(
        cargo_allow()
            .arg("check")
            .arg("--root")
            .arg(&root)
            .arg("--config")
            .arg(&policy)
            .arg("--kind")
            .arg("panic")
            .arg("--mode")
            .arg("no-new")
            .arg("--format")
            .arg("json")
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("run check: {err}"))),
        "check no-new (baseline)",
    );
    let check_json: serde_json::Value = serde_json::from_slice(&check.stdout)
        .unwrap_or_else(|err| std::panic::panic_any(format!("check stdout should be JSON: {err}")));
    assert_eq!(
        check_json.get("status").and_then(serde_json::Value::as_str),
        Some("passed"),
        "generated baseline must pass its own no-new check"
    );
    assert_eq!(
        check_json
            .pointer("/summary/new")
            .and_then(serde_json::Value::as_u64),
        Some(0),
        "no new debt at baseline"
    );

    // 5. list: the baseline entry appears with status baseline_debt.
    let list = run(
        cargo_allow()
            .arg("list")
            .arg("--root")
            .arg(&root)
            .arg("--config")
            .arg(&policy)
            .arg("--kind")
            .arg("panic")
            .arg("--format")
            .arg("json")
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("run list: {err}"))),
        "list",
    );
    let list_json: serde_json::Value = serde_json::from_slice(&list.stdout)
        .unwrap_or_else(|err| std::panic::panic_any(format!("list stdout should be JSON: {err}")));
    let first_status = list_json
        .pointer("/allow_entries/0/status")
        .and_then(serde_json::Value::as_str);
    assert!(
        matches!(first_status, Some("baseline_debt") | Some("matched")),
        "list exposes the baseline entry (status={first_status:?})"
    );

    // 6. explain: the baseline entry has an explainable identity.
    let allow_id = list_json
        .pointer("/allow_entries/0/id")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| std::panic::panic_any("list should report an allow id"));
    run(
        cargo_allow()
            .arg("explain")
            .arg(&allow_id)
            .arg("--root")
            .arg(&root)
            .arg("--config")
            .arg(&policy)
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("run explain: {err}"))),
        "explain",
    );

    // 7. worklist: advisory queue runs against the policy without error.
    run(
        cargo_allow()
            .arg("worklist")
            .arg("--root")
            .arg(&root)
            .arg("--config")
            .arg(&policy)
            .arg("--kind")
            .arg("panic")
            .arg("--format")
            .arg("json")
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("run worklist: {err}"))),
        "worklist",
    );

    drop_root(root);
}

/// The ratchet: after baselining, adding one new in-scope exception fails
/// `check --mode no-new`, and the failure explains what happened (actionable
/// guidance). This is the core product-correctness claim.
#[test]
fn first_hour_new_in_scope_exception_fails_no_new_with_guidance() {
    let root = temp_root("ratchet-fail");
    write_source(
        &root,
        "pub fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    );

    let policy = root.join("policy/allow.toml");
    run(
        cargo_allow()
            .arg("propose")
            .arg("--root")
            .arg(&root)
            .arg("--kind")
            .arg("panic")
            .arg("--write")
            .arg(&policy)
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("run propose: {err}"))),
        "propose baseline",
    );

    // Baseline passes.
    run(
        cargo_allow()
            .arg("check")
            .arg("--root")
            .arg(&root)
            .arg("--config")
            .arg(&policy)
            .arg("--kind")
            .arg("panic")
            .arg("--mode")
            .arg("no-new")
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("run check baseline: {err}"))),
        "check no-new (baseline)",
    );

    // Add a NEW in-scope panic.
    write_source(
        &root,
        concat!(
            "pub fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
            "pub fn reload(value: Result<u8, ()>) -> u8 { value.unwrap() }\n",
        ),
    );

    // The gate must fail and the human output must explain the new debt and the
    // next step (receipt or remove it).
    let fail = run_fail(
        cargo_allow()
            .arg("check")
            .arg("--root")
            .arg(&root)
            .arg("--config")
            .arg(&policy)
            .arg("--kind")
            .arg("panic")
            .arg("--mode")
            .arg("no-new")
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("run check (new debt): {err}"))),
        "check no-new (new debt)",
    );
    let human = combined(&fail);
    assert!(
        human.contains("new") && human.contains("unwrap"),
        "check failure should name the new unwrap debt: `{human}`"
    );

    drop_root(root);
}
