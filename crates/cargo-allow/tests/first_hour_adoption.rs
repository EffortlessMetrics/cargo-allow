//! First-hour adoption proof: run the real cargo-allow binary end-to-end through
//! the adoption path from an empty temporary repo —
//! doctor → audit → propose → check no-new → list → explain → worklist —
//! and assert exit codes, generated files, receipts, next-step guidance, and the
//! no-new ratchet (a new in-scope exception after baselining fails the gate).
//!
//! Also proves the clean-audit branch (no `propose`), the `init` bootstrap path,
//! and that `docs/getting-started.md` stays aligned with the checked step
//! inventory and fixture-derived expected-output markers (#2354).
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

fn getting_started_doc() -> &'static str {
    include_str!("../../../docs/getting-started.md")
}

fn step_inventory() -> &'static str {
    include_str!("../../../docs/dogfood/fixtures/getting-started/step-inventory.toml")
}

fn expected_markers_doc() -> &'static str {
    include_str!("../../../docs/dogfood/fixtures/getting-started/expected-markers.md")
}

/// Parse `[[step]]` rows from the committed step inventory (no extra deps).
#[derive(Debug, Clone, PartialEq, Eq)]
struct InventoryStep {
    id: String,
    channel: String,
    argv_head: String,
    exit_class: String,
    output_id: String,
}

fn inventory_steps(inventory: &str) -> Vec<InventoryStep> {
    let mut steps = Vec::new();
    let mut current: Option<InventoryStep> = None;
    let flush = |steps: &mut Vec<InventoryStep>, current: &mut Option<InventoryStep>| {
        if let Some(step) = current.take()
            && !step.id.is_empty()
        {
            steps.push(step);
        }
    };

    for line in inventory.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("[[step]]") {
            flush(&mut steps, &mut current);
            current = Some(InventoryStep {
                id: String::new(),
                channel: String::new(),
                argv_head: String::new(),
                exit_class: String::new(),
                output_id: String::new(),
            });
            continue;
        }
        let Some(step) = current.as_mut() else {
            continue;
        };
        let Some((key, raw)) = trimmed.split_once('=') else {
            continue;
        };
        let value = raw.trim().trim_matches('"');
        match key.trim() {
            "id" => step.id = value.to_string(),
            "channel" => step.channel = value.to_string(),
            "argv_head" => step.argv_head = value.to_string(),
            "exit_class" => step.exit_class = value.to_string(),
            "output_id" => step.output_id = value.to_string(),
            _ => {}
        }
    }
    flush(&mut steps, &mut current);
    steps
}

#[test]
fn getting_started_documents_checked_step_inventory() {
    let guide = getting_started_doc();
    let inventory = step_inventory();
    let steps = inventory_steps(inventory);
    assert!(
        !steps.is_empty(),
        "step-inventory.toml must list at least one step"
    );

    for step in &steps {
        assert!(!step.id.is_empty(), "inventory step missing id: {step:?}");
        assert!(
            matches!(step.channel.as_str(), "published" | "candidate" | "both"),
            "inventory step `{}` has invalid channel `{}`",
            step.id,
            step.channel
        );
        assert!(
            matches!(
                step.exit_class.as_str(),
                "success" | "failure" | "advisory_success"
            ),
            "inventory step `{}` has invalid exit_class `{}`",
            step.id,
            step.exit_class
        );
        assert!(
            !step.output_id.is_empty(),
            "inventory step `{}` missing output_id",
            step.id
        );
        assert!(
            guide.contains(&step.id),
            "getting-started must document step id `{}`",
            step.id
        );
        assert!(
            guide.contains(&step.output_id),
            "getting-started must document output_id `{}` for step `{}`",
            step.output_id,
            step.id
        );
        if !step.argv_head.is_empty() {
            assert!(
                guide.contains(&step.argv_head),
                "getting-started must name argv_head `{}` for step `{}`",
                step.argv_head,
                step.id
            );
        }
        if step.channel == "candidate" {
            assert!(
                guide.contains("Source-candidate") || guide.contains("source candidate"),
                "candidate step `{}` must stay labeled as source-candidate in the guide",
                step.id
            );
        }
    }

    for required in [
        "Illustrative only",
        "how-to/manage-an-exception.md",
        "do not manufacture baseline debt",
        "Choose ONE bootstrap path",
    ] {
        assert!(
            guide.contains(required),
            "getting-started missing required phrase: {required}"
        );
    }
}

#[test]
fn first_hour_expected_markers_match_live_renderer() {
    let guide = getting_started_doc();
    let markers = expected_markers_doc();
    for required in [
        "cargo-allow.doctor.v1",
        "\"command\": \"doctor\"",
        "config: not found",
        "\"command\": \"audit\"",
        "\"findings\": 0",
        "\"new\": 0",
        "\"new\": 1",
        "\"status\": \"passed\"",
        "\"command\": \"check\"",
        "Result: passed (enforcing)",
        "Result: failed",
        "new: unreceipted",
        "cargo-allow list",
        "cargo-allow explain",
    ] {
        assert!(
            markers.contains(required),
            "expected-markers.md must list `{required}`"
        );
        assert!(
            guide.contains(required),
            "getting-started must carry fixture marker `{required}`"
        );
    }

    // Live renderer: clean tree → doctor + clean audit markers.
    let clean = temp_root("markers-clean");
    write_source(&clean, "pub fn ok() -> u8 { 1 }\n");
    let doctor = run(
        cargo_allow()
            .arg("doctor")
            .arg("--root")
            .arg(&clean)
            .arg("--format")
            .arg("json")
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("run doctor: {err}"))),
        "doctor json",
    );
    let doctor_text = combined(&doctor);
    assert!(
        doctor_text.contains("cargo-allow.doctor.v1"),
        "live doctor must emit schema_id marker"
    );
    assert!(
        doctor_text.contains("\"command\":\"doctor\"")
            || doctor_text.contains("\"command\": \"doctor\""),
        "live doctor must emit command marker: `{doctor_text}`"
    );

    let doctor_human = run(
        cargo_allow()
            .arg("doctor")
            .arg("--root")
            .arg(&clean)
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("run doctor human: {err}"))),
        "doctor human",
    );
    assert!(
        combined(&doctor_human).contains("config: not found"),
        "live human doctor must emit config-not-found marker"
    );

    let audit_clean = run(
        cargo_allow()
            .arg("audit")
            .arg("--root")
            .arg(&clean)
            .arg("--kind")
            .arg("panic")
            .arg("--format")
            .arg("json")
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("run audit clean: {err}"))),
        "audit clean",
    );
    let audit_clean_json: serde_json::Value = serde_json::from_slice(&audit_clean.stdout)
        .unwrap_or_else(|err| std::panic::panic_any(format!("audit clean json: {err}")));
    assert_eq!(
        audit_clean_json
            .pointer("/summary/findings")
            .and_then(serde_json::Value::as_u64),
        Some(0)
    );
    assert_eq!(
        audit_clean_json
            .pointer("/summary/new")
            .and_then(serde_json::Value::as_u64),
        Some(0)
    );
    drop_root(clean);

    // Live renderer: brownfield → audit new=1, check pass, then fail after new debt.
    let brown = temp_root("markers-brown");
    write_source(
        &brown,
        "pub fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    );
    let audit = run(
        cargo_allow()
            .arg("audit")
            .arg("--root")
            .arg(&brown)
            .arg("--kind")
            .arg("panic")
            .arg("--format")
            .arg("json")
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("run audit: {err}"))),
        "audit finding",
    );
    let audit_json: serde_json::Value = serde_json::from_slice(&audit.stdout)
        .unwrap_or_else(|err| std::panic::panic_any(format!("audit json: {err}")));
    assert_eq!(
        audit_json
            .pointer("/summary/new")
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
    assert_eq!(
        audit_json.get("status").and_then(serde_json::Value::as_str),
        Some("passed")
    );

    let policy = brown.join("policy/allow.toml");
    run(
        cargo_allow()
            .arg("propose")
            .arg("--root")
            .arg(&brown)
            .arg("--kind")
            .arg("panic")
            .arg("--write")
            .arg(&policy)
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("run propose: {err}"))),
        "propose",
    );

    let check_pass = run(
        cargo_allow()
            .arg("check")
            .arg("--root")
            .arg(&brown)
            .arg("--config")
            .arg(&policy)
            .arg("--kind")
            .arg("panic")
            .arg("--mode")
            .arg("no-new")
            .arg("--format")
            .arg("json")
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("run check pass: {err}"))),
        "check pass json",
    );
    let check_pass_json: serde_json::Value = serde_json::from_slice(&check_pass.stdout)
        .unwrap_or_else(|err| std::panic::panic_any(format!("check pass json: {err}")));
    assert_eq!(
        check_pass_json
            .get("status")
            .and_then(serde_json::Value::as_str),
        Some("passed")
    );
    assert_eq!(
        check_pass_json
            .pointer("/summary/new")
            .and_then(serde_json::Value::as_u64),
        Some(0)
    );

    let check_pass_human = run(
        cargo_allow()
            .arg("check")
            .arg("--root")
            .arg(&brown)
            .arg("--config")
            .arg(&policy)
            .arg("--kind")
            .arg("panic")
            .arg("--mode")
            .arg("no-new")
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("run check pass human: {err}"))),
        "check pass human",
    );
    assert!(
        combined(&check_pass_human).contains("Result: passed (enforcing)"),
        "live no-new pass must state the enforcing mode, not advisory"
    );

    write_source(
        &brown,
        concat!(
            "pub fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
            "pub fn reload(value: Result<u8, ()>) -> u8 { value.unwrap() }\n",
        ),
    );
    let check_fail = run_fail(
        cargo_allow()
            .arg("check")
            .arg("--root")
            .arg(&brown)
            .arg("--config")
            .arg(&policy)
            .arg("--kind")
            .arg("panic")
            .arg("--mode")
            .arg("no-new")
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("run check fail: {err}"))),
        "check fail",
    );
    let fail_text = combined(&check_fail);
    assert!(
        fail_text.contains("Result: failed"),
        "live failing check must emit Result: failed: `{fail_text}`"
    );
    assert!(
        fail_text.contains("new: unreceipted"),
        "live failing check must emit new: unreceipted: `{fail_text}`"
    );
    drop_root(brown);
}

/// Clean-audit branch: doctor + zero-finding audit must succeed without propose.
#[test]
fn first_hour_clean_audit_branch_does_not_require_propose() {
    let root = temp_root("clean-audit");
    write_source(&root, "pub fn ok() -> u8 { 1 }\n");

    run(
        cargo_allow()
            .arg("doctor")
            .arg("--root")
            .arg(&root)
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("run doctor: {err}"))),
        "doctor",
    );

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
        "audit clean",
    );
    let audit_json: serde_json::Value = serde_json::from_slice(&audit.stdout)
        .unwrap_or_else(|err| std::panic::panic_any(format!("audit json: {err}")));
    assert_eq!(
        audit_json
            .pointer("/summary/new")
            .and_then(serde_json::Value::as_u64),
        Some(0),
        "clean tree must not surface unreceipted panic findings"
    );
    assert!(
        !root.join("policy/allow.toml").exists(),
        "clean-audit branch must not invent a policy file"
    );

    drop_root(root);
}

/// Strict-repo bootstrap: init creates a policy that passes no-new without propose.
#[test]
fn first_hour_init_bootstrap_passes_no_new_without_propose() {
    let root = temp_root("init-bootstrap");
    write_source(&root, "pub fn ok() -> u8 { 1 }\n");

    run(
        cargo_allow()
            .arg("init")
            .arg("--root")
            .arg(&root)
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("run init: {err}"))),
        "init",
    );
    let policy = root.join("policy/allow.toml");
    assert!(policy.is_file(), "init must create policy/allow.toml");

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
        "check after init",
    );
    let check_json: serde_json::Value = serde_json::from_slice(&check.stdout)
        .unwrap_or_else(|err| std::panic::panic_any(format!("check json: {err}")));
    assert_eq!(
        check_json.get("status").and_then(serde_json::Value::as_str),
        Some("passed")
    );
    assert_eq!(
        check_json
            .pointer("/summary/new")
            .and_then(serde_json::Value::as_u64),
        Some(0)
    );

    drop_root(root);
}

/// `init` writes `allow_bare_allow_attributes = false`, so `propose` used to
/// generate a `lint_exception`/`allow_attribute` entry that the very same
/// policy rejected — aborting the whole preview with a conflict naming an id
/// the operator could not find in their file. The two documented bootstrap
/// paths were unusable in sequence on any tree containing a bare
/// `#[allow(...)]` (#3023).
#[test]
fn init_then_propose_skips_findings_the_policy_forbids_receipting() {
    let root = temp_root("init-then-propose");
    write_source(
        &root,
        "#[allow(dead_code)]\nfn helper() {}\npub fn boom() -> u8 { None::<u8>.unwrap() }\n",
    );

    run(
        cargo_allow()
            .arg("init")
            .arg("--root")
            .arg(&root)
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("run init: {err}"))),
        "init",
    );
    let policy = root.join("policy/allow.toml");

    let propose = run(
        cargo_allow()
            .arg("propose")
            .arg("--root")
            .arg(&root)
            .arg("--config")
            .arg(&policy)
            .arg("--summary-format")
            .arg("json")
            .arg("--summary-output")
            .arg(root.join("propose.json"))
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("run propose: {err}"))),
        "propose after init",
    );
    let _ = propose;

    let summary: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("propose.json")).unwrap_or_default())
            .unwrap_or_else(|err| std::panic::panic_any(format!("propose json: {err}")));

    // The bare `#[allow(dead_code)]` is reported as skipped, with the reason,
    // rather than aborting the run.
    assert_eq!(
        summary
            .pointer("/summary/unreceiptable_new_findings")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "propose should report the forbidden finding as skipped: {summary}"
    );
    let reason = summary
        .pointer("/summary/unreceiptable_reason")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    assert!(
        reason.contains("allow_bare_allow_attributes"),
        "the skip must name the requirement that caused it: {reason}"
    );
    // Skipping is not truncation, and the rest of the tree is still baselined.
    assert_eq!(
        summary
            .pointer("/summary/truncated_new_findings")
            .and_then(serde_json::Value::as_u64),
        Some(0),
        "a forbidden finding is not a --max truncation: {summary}"
    );
    assert!(
        summary
            .pointer("/summary/baseline_debt_entries_proposed")
            .and_then(serde_json::Value::as_u64)
            .is_some_and(|proposed| proposed >= 1),
        "the panic finding should still be baselined: {summary}"
    );

    drop_root(root);
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
