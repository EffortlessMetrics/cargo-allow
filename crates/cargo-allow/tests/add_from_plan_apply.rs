mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use support::{
    assert_saved_json_artifact, assert_status, assert_stderr_empty, assert_stdout_empty,
    cargo_allow_command, remove_temp_root, temp_root,
};

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap_or_else(|error| std::panic::panic_any(format!("git {args:?}: {error}")));
    assert_status("git fixture", &output, true);
}

/// Initialise a git repo with an unreceipted `panic.unwrap` finding and return
/// its root.
fn init_fixture(label: &str) -> PathBuf {
    let root = temp_root(label);
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|error| std::panic::panic_any(format!("create source dir: {error}")));
    git(&root, &["init"]);
    git(&root, &["config", "user.email", "fixture@example.com"]);
    git(&root, &["config", "user.name", "fixture"]);
    // The fixture's raw bytes are the oracle, independent of host Git defaults.
    git(&root, &["config", "core.autocrlf", "false"]);
    let init = cargo_allow_command()
        .args(["init", "--root"])
        .arg(&root)
        .output()
        .unwrap_or_else(|error| std::panic::panic_any(format!("init fixture: {error}")));
    assert_status("init fixture", &init, true);
    fs::write(
        root.join("src/lib.rs"),
        "pub fn load() -> usize { Some(1).unwrap() }\n",
    )
    .unwrap_or_else(|error| std::panic::panic_any(format!("write source: {error}")));
    // Commit so the inventory is the stable `git_tracked` set; untracked
    // artifacts written into the tree later (the plan, the receipt) then do not
    // perturb the recomputed inventory basis between generation and application.
    git(&root, &["add", "policy/allow.toml", "src/lib.rs"]);
    git(&root, &["commit", "-q", "-m", "fixture"]);
    root
}

fn generate_plan(root: &Path) -> PathBuf {
    let plan_path = root.join("add-plan.json");
    let why = cargo_allow_command()
        .args(["why", "--root"])
        .arg(root)
        .args(["--kind", "panic", "--path", "src/lib.rs", "--line", "1"])
        .arg("--plan")
        .arg(&plan_path)
        .output()
        .unwrap_or_else(|error| std::panic::panic_any(format!("why plan: {error}")));
    assert_status("why plan", &why, true);
    assert!(plan_path.exists(), "plan should be written");
    plan_path
}

fn is_sha256_v1(value: Option<&str>) -> bool {
    value.is_some_and(|value| value.starts_with("sha256:v1:") && value.len() == 74)
}

fn append_past_read_limit_refuses_without_mutation(
    from_plan: bool,
    label: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = init_fixture(label);
    let policy_path = root.join("policy/allow.toml");
    let initial = fs::read_to_string(&policy_path)?;
    let limit = usize::try_from(allow_core::SOURCE_FILE_READ_MAX_BYTES)?;
    let mut before = format!(
        "\u{feff}# Historical comment\r\n{}\n# ",
        initial.replace('\n', "\r\n")
    );
    before.push_str(&"x".repeat(limit - 64 - before.len()));
    assert_eq!(before.len(), limit - 64);
    fs::write(&policy_path, &before)?;
    assert!(allow_policy::load_policy(&policy_path).is_ok());
    git(&root, &["add", "policy/allow.toml"]);
    git(&root, &["commit", "-q", "-m", "near-limit history"]);

    let plan = from_plan.then(|| generate_plan(&root));
    let receipt = root.join("overflow-receipt.json");
    let mut command = cargo_allow_command();
    command.args(["add", "--root"]).arg(&root);
    if let Some(plan) = &plan {
        command.arg("--from-plan").arg(plan);
    } else {
        command.args(["--kind", "panic", "--path", "src/lib.rs", "--line", "1"]);
    }
    let output = command
        .args([
            "--owner",
            "fixture",
            "--reason",
            "reviewed boundary finding",
            "--update",
        ])
        .arg("--summary-output")
        .arg(&receipt)
        .output()?;
    let after = fs::read(&policy_path)?;
    let readable = allow_policy::load_policy(&policy_path).is_ok();
    assert!(
        !output.status.success(),
        "append must refuse before mutation: from_plan={from_plan}, before={}, after={}, limit={limit}, prefix={}, readable={readable}",
        before.len(),
        after.len(),
        after.starts_with(before.as_bytes()),
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("8388608") && stderr.contains("appended policy"),
        "{stderr}"
    );
    assert_eq!(after.len(), before.len());
    assert!(
        after.as_slice() == before.as_bytes(),
        "refusal must preserve every policy byte"
    );
    assert!(readable, "refusal must leave the policy readable");
    assert!(
        !receipt.exists(),
        "refusal must not claim an application receipt"
    );
    let diff = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["diff", "--numstat", "--", "policy/allow.toml"])
        .output()?;
    assert_status("refused policy diff", &diff, true);
    assert!(
        diff.stdout.is_empty(),
        "refusal must produce no policy diff"
    );
    remove_temp_root(root);
    Ok(())
}

#[test]
fn add_update_refuses_policy_growth_past_read_limit() -> Result<(), Box<dyn std::error::Error>> {
    append_past_read_limit_refuses_without_mutation(false, "add-update-read-limit")
}

#[test]
fn add_from_plan_refuses_policy_growth_past_read_limit() -> Result<(), Box<dyn std::error::Error>> {
    append_past_read_limit_refuses_without_mutation(true, "add-plan-read-limit")
}

#[test]
fn add_from_plan_applies_a_verified_plan_and_binds_a_receipt()
-> Result<(), Box<dyn std::error::Error>> {
    let root = init_fixture("add-from-plan-apply");
    let policy_path = root.join("policy/allow.toml");
    let initial_policy = fs::read_to_string(&policy_path)?;
    let preserved_policy = format!(
        "\u{feff}# Historical CRLF header\r\n{}\n# Recent LF comment with 'literal' quoting\n",
        initial_policy.replace('\n', "\r\n")
    );
    fs::write(&policy_path, &preserved_policy)?;
    git(&root, &["add", "policy/allow.toml"]);
    git(&root, &["commit", "-q", "-m", "mixed policy envelope"]);
    let plan_path = generate_plan(&root);
    let policy_before = fs::read_to_string(&policy_path)
        .unwrap_or_else(|error| std::panic::panic_any(format!("read policy: {error}")));

    let repeated_plan = root.join("repeated-plan.json");
    let repeat = cargo_allow_command()
        .args(["why", "--root"])
        .arg(&root)
        .args(["--kind", "panic", "--path", "src/lib.rs", "--line", "1"])
        .arg("--plan")
        .arg(&repeated_plan)
        .output()?;
    assert_status("repeated why plan", &repeat, true);
    assert_eq!(fs::read(&plan_path)?, fs::read(&repeated_plan)?);
    assert_eq!(fs::read(&policy_path)?, policy_before.as_bytes());

    // Rebase two empty commits while the scanned source and policy stay exact.
    // HEAD movement alone must not stale a source-bound add plan.
    git(&root, &["branch", "integration-base"]);
    git(&root, &["switch", "-c", "candidate"]);
    git(
        &root,
        &["commit", "--allow-empty", "-q", "-m", "candidate no-op"],
    );
    git(&root, &["switch", "integration-base"]);
    git(
        &root,
        &["commit", "--allow-empty", "-q", "-m", "base no-op"],
    );
    git(&root, &["switch", "candidate"]);
    git(&root, &["rebase", "integration-base"]);
    assert_eq!(fs::read(&policy_path)?, policy_before.as_bytes());

    let receipt_path = root.join("receipt.json");
    let common_summary_path = root.join("common-summary.json");
    let apply = cargo_allow_command()
        .arg("--command-summary-output")
        .arg(&common_summary_path)
        .args(["add", "--root"])
        .arg(&root)
        .arg("--from-plan")
        .arg(&plan_path)
        .args([
            "--owner",
            "fixture",
            "--reason",
            "covered by the add-from-plan lifecycle test",
            "--update",
        ])
        .arg("--summary-output")
        .arg(&receipt_path)
        .output()
        .unwrap_or_else(|error| std::panic::panic_any(format!("add from-plan: {error}")));
    assert_status("add from-plan", &apply, true);
    assert_stdout_empty(
        "add from-plan",
        &apply,
        "policy goes to the live ledger and the receipt to --summary-output",
    );
    assert_stderr_empty(
        "add from-plan",
        &apply,
        "the receipt goes to --summary-output, not stderr",
    );

    let common_summary: Value = serde_json::from_str(
        &fs::read_to_string(&common_summary_path)
            .unwrap_or_else(|error| std::panic::panic_any(format!("read common summary: {error}"))),
    )?;
    assert_eq!(
        common_summary.get("schema_id").and_then(Value::as_str),
        Some("cargo-allow.core-command-summary.v1")
    );
    assert_eq!(
        common_summary.get("tool").and_then(Value::as_str),
        Some("cargo-allow")
    );
    assert_eq!(
        common_summary.pointer("/operation").and_then(Value::as_str),
        Some("add_from_plan")
    );
    assert_eq!(
        common_summary.pointer("/posture").and_then(Value::as_str),
        Some("satisfied")
    );
    assert_eq!(
        common_summary
            .pointer("/operation_effects/writes_repository")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        common_summary
            .pointer("/operation_effects/write_paths/0")
            .and_then(Value::as_str),
        Some("policy/allow.toml")
    );
    assert_eq!(
        common_summary
            .pointer("/next_proof/args/0")
            .and_then(Value::as_str),
        Some("check")
    );
    assert_eq!(
        common_summary
            .pointer("/next_proof/args/2")
            .and_then(Value::as_str),
        Some("no-new")
    );

    let receipt = assert_saved_json_artifact(
        &receipt_path,
        "add-plan-application",
        "cargo-allow.add-plan-application.v1",
        "add",
    );
    let schema: Value = serde_json::from_str(include_str!(
        "../../../docs/schemas/add-plan-application.schema.json"
    ))?;
    let validator = jsonschema::validator_for(&schema)?;
    assert!(
        validator.validate(&receipt).is_ok(),
        "runtime-produced add-plan-application receipt should validate against its published schema"
    );
    for pointer in [
        "/plan_digest",
        "/finding_digest",
        "/repository_identity",
        "/policy_before_digest",
        "/policy_after_digest",
    ] {
        assert!(
            is_sha256_v1(receipt.pointer(pointer).and_then(Value::as_str)),
            "{pointer} should be a versioned SHA-256 binding"
        );
    }
    assert_ne!(
        receipt.pointer("/policy_before_digest"),
        receipt.pointer("/policy_after_digest"),
        "before/after policy digests must differ after a real write"
    );
    assert_eq!(
        receipt.pointer("/targeted_recheck").and_then(Value::as_str),
        Some("matched")
    );
    assert_eq!(
        receipt
            .pointer("/full_check_argv/0")
            .and_then(Value::as_str),
        Some("check")
    );
    assert!(
        receipt
            .pointer("/added_allow_id")
            .and_then(Value::as_str)
            .is_some_and(|id| id.starts_with("allow-")),
        "receipt should record the added allow id"
    );

    let policy_after = fs::read_to_string(&policy_path)
        .unwrap_or_else(|error| std::panic::panic_any(format!("reread policy: {error}")));
    assert_ne!(policy_before, policy_after, "policy should have changed");
    assert!(
        policy_after
            .as_bytes()
            .starts_with(policy_before.as_bytes()),
        "adding one receipt must preserve every existing policy byte"
    );
    assert_eq!(
        receipt
            .pointer("/policy_before_digest")
            .and_then(Value::as_str),
        Some(allow_core::sha256_v1_bytes(policy_before.as_bytes()).as_str())
    );
    assert_eq!(
        receipt
            .pointer("/policy_after_digest")
            .and_then(Value::as_str),
        Some(allow_core::sha256_v1_bytes(policy_after.as_bytes()).as_str())
    );
    let numstat = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["diff", "--numstat", "--", "policy/allow.toml"])
        .output()?;
    assert_status("policy numstat", &numstat, true);
    assert_eq!(
        String::from_utf8(numstat.stdout)?.split('\t').nth(1),
        Some("0"),
        "receipt append must contain no deleted policy lines"
    );
    assert!(
        policy_after.contains("fixture"),
        "policy should record the operator-supplied owner"
    );

    // Replay: the same plan applied again must fail (the finding is no longer
    // `New`) and must leave policy untouched.
    let replay = cargo_allow_command()
        .args(["add", "--root"])
        .arg(&root)
        .arg("--from-plan")
        .arg(&plan_path)
        .args([
            "--owner",
            "fixture",
            "--reason",
            "replay attempt after success",
            "--update",
        ])
        .output()
        .unwrap_or_else(|error| std::panic::panic_any(format!("replay: {error}")));
    assert_status("replay", &replay, false);
    let policy_after_replay = fs::read_to_string(&policy_path)
        .unwrap_or_else(|error| std::panic::panic_any(format!("reread policy: {error}")));
    assert_eq!(
        policy_after, policy_after_replay,
        "a rejected replay must not mutate policy"
    );

    remove_temp_root(root);
    Ok(())
}

#[test]
fn add_from_plan_rejects_source_drift_without_mutation() {
    let root = init_fixture("add-from-plan-drift");
    let plan_path = generate_plan(&root);
    let policy_path = root.join("policy/allow.toml");

    // Drift the source file after the plan was generated.
    fs::write(
        root.join("src/lib.rs"),
        "// drifted\npub fn load() -> usize { Some(1).unwrap() }\n",
    )
    .unwrap_or_else(|error| std::panic::panic_any(format!("rewrite source: {error}")));
    git(&root, &["add", "src/lib.rs"]);

    let policy_before = fs::read_to_string(&policy_path)
        .unwrap_or_else(|error| std::panic::panic_any(format!("read policy: {error}")));
    let apply = cargo_allow_command()
        .args(["add", "--root"])
        .arg(&root)
        .arg("--from-plan")
        .arg(&plan_path)
        .args([
            "--owner",
            "fixture",
            "--reason",
            "should be rejected for drift",
            "--update",
        ])
        .output()
        .unwrap_or_else(|error| std::panic::panic_any(format!("add from-plan drift: {error}")));
    assert_status("add from-plan drift", &apply, false);
    let policy_after = fs::read_to_string(&policy_path)
        .unwrap_or_else(|error| std::panic::panic_any(format!("reread policy: {error}")));
    assert_eq!(
        policy_before, policy_after,
        "a stale plan must not mutate policy"
    );

    remove_temp_root(root);
}

#[test]
fn add_from_plan_requires_update_and_conflicts_with_write() {
    let root = init_fixture("add-from-plan-flags");
    let plan_path = generate_plan(&root);

    // Missing --update: the live-ledger route is mandatory.
    let no_update = cargo_allow_command()
        .args(["add", "--root"])
        .arg(&root)
        .arg("--from-plan")
        .arg(&plan_path)
        .args(["--owner", "fixture", "--reason", "no update flag"])
        .output()
        .unwrap_or_else(|error| std::panic::panic_any(format!("add from-plan no update: {error}")));
    assert_status("add from-plan without --update", &no_update, false);

    // Conflict with a candidate-file write target.
    let with_write = cargo_allow_command()
        .args(["add", "--root"])
        .arg(&root)
        .arg("--from-plan")
        .arg(&plan_path)
        .args([
            "--owner",
            "fixture",
            "--reason",
            "write conflict",
            "--update",
        ])
        .arg("--write")
        .arg(root.join("candidate.toml"))
        .output()
        .unwrap_or_else(|error| std::panic::panic_any(format!("add from-plan write: {error}")));
    assert_status("add from-plan with --write", &with_write, false);

    // Conflict with a manual target selector.
    let with_kind = cargo_allow_command()
        .args(["add", "--root"])
        .arg(&root)
        .arg("--from-plan")
        .arg(&plan_path)
        .args([
            "--owner",
            "fixture",
            "--reason",
            "kind conflict",
            "--update",
            "--kind",
            "panic",
        ])
        .output()
        .unwrap_or_else(|error| std::panic::panic_any(format!("add from-plan kind: {error}")));
    assert_status("add from-plan with --kind", &with_kind, false);

    remove_temp_root(root);
}

#[test]
fn add_from_plan_moved_line_recovery_hint_runs_verbatim() -> Result<(), Box<dyn std::error::Error>>
{
    let root = init_fixture("add-from-plan-moved-line-recovery");
    let policy_path = root.join("policy/allow.toml");
    let plan_path = generate_plan(&root);
    let policy_before = fs::read(&policy_path)?;
    let old_plan_before = fs::read(&plan_path)?;
    let old_plan: Value = serde_json::from_slice(&old_plan_before)?;
    assert_eq!(
        old_plan.pointer("/finding/line").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        old_plan.pointer("/outcome/status").and_then(Value::as_str),
        Some("new")
    );

    let initial_head = Command::new("git")
        .current_dir(&root)
        .args(["rev-parse", "HEAD"])
        .output()?;
    assert_status("initial fixture HEAD", &initial_head, true);

    // Move the only finding, without changing its expression or receipting it.
    // Tracked filenames and HEAD remain fixed. Generated JSON stays untracked.
    let source_path = root.join("src/lib.rs");
    let original_source = fs::read_to_string(&source_path)?;
    fs::write(
        &source_path,
        format!("// moved without receipting\n\n{original_source}"),
    )?;
    git(&root, &["add", "src/lib.rs"]);

    let moved_why = cargo_allow_command()
        .current_dir(&root)
        .args([
            "why",
            "--kind",
            "panic",
            "--path",
            "src/lib.rs",
            "--line",
            "1",
            "--format",
            "json",
            "--output",
            "moved-why.json",
        ])
        .output()?;
    assert_status("live moved finding", &moved_why, true);
    let moved_report: Value = serde_json::from_slice(&fs::read(root.join("moved-why.json"))?)?;
    assert_eq!(
        moved_report
            .pointer("/finding/line")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        moved_report
            .pointer("/outcome/status")
            .and_then(Value::as_str),
        Some("new"),
        "the moved target must remain New before recovery"
    );
    assert_eq!(
        moved_report
            .pointer("/line_targeting/requested_line")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        moved_report
            .pointer("/line_targeting/matched_line")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(fs::read(&policy_path)?, policy_before);
    assert_eq!(fs::read(&plan_path)?, old_plan_before);

    // Relative ASCII argv and per-command fixture cwd isolate the recorded-line
    // lead from separate root/config/quoting recovery questions.
    let rejected = cargo_allow_command()
        .current_dir(&root)
        .args([
            "add",
            "--from-plan",
            "add-plan.json",
            "--owner",
            "fixture",
            "--reason",
            "moved-line recovery fixture",
            "--update",
        ])
        .output()?;
    assert_status("stale moved-line add", &rejected, false);
    let rejection_text = String::from_utf8(rejected.stderr.clone())?;
    assert!(
        rejection_text.contains("source inventory changed since the plan was generated")
            || rejection_text.contains("finding location changed since the plan was generated"),
        "the refusal must be source-binding drift: {rejection_text}"
    );
    assert!(rejection_text.contains("(policy unchanged)"));
    assert_eq!(fs::read(&policy_path)?, policy_before);
    assert_eq!(fs::read(&plan_path)?, old_plan_before);

    // Parse the actual advertised command; do not rebuild or correct its line.
    // This fixture uses no spaces, quoting, absolute paths, or shell syntax.
    let (_, tail) = rejection_text
        .split_once("; regenerate with ")
        .ok_or("stale binding rejection lacked regeneration advice")?;
    let (printed, _) = tail
        .split_once(" (")
        .ok_or("regeneration advice lacked its explanation boundary")?;
    let tokens: Vec<_> = printed.split_ascii_whitespace().collect();
    assert_eq!(tokens.first().copied(), Some("cargo-allow"));
    assert_eq!(tokens.get(1).copied(), Some("why"));
    let retry_argument = tokens
        .windows(2)
        .find_map(|pair| match pair {
            ["--plan", argument] => Some(*argument),
            _ => None,
        })
        .ok_or("printed regeneration command lacked --plan")?;
    assert!(Path::new(retry_argument).is_relative());
    assert_eq!(
        Path::new(retry_argument)
            .file_name()
            .and_then(std::ffi::OsStr::to_str),
        Some(retry_argument),
        "this fixture's hint must use a relative sibling filename"
    );
    assert_ne!(retry_argument, "add-plan.json");
    assert_ne!(retry_argument, "control-plan.json");
    let retry_path = root.join(retry_argument);
    assert!(!retry_path.exists(), "hint output must initially be fresh");

    // The binary comes from the existing CARGO_BIN_EXE helper. Every printed
    // argv token after cargo-allow is passed unchanged; no format flag is added.
    let hinted = cargo_allow_command()
        .current_dir(&root)
        .args(tokens.iter().skip(1))
        .output()?;
    let hinted_stderr = String::from_utf8(hinted.stderr.clone())?;
    assert_eq!(fs::read(&policy_path)?, policy_before);
    assert_eq!(fs::read(&plan_path)?, old_plan_before);
    if !hinted.status.success() {
        assert!(
            hinted_stderr.contains("add-finding plan refused")
                && hinted_stderr.contains("did not exactly match")
                && hinted_stderr.contains("src/lib.rs:3"),
            "the failing hint must expose the recorded/live line mismatch: {hinted_stderr}"
        );
        assert!(!retry_path.exists(), "refused hint must not write a plan");
    }

    // Run a separate fresh-path positive control BEFORE the expected-red
    // success assertion, even if the printed hint failed. This proves the same
    // New finding can be planned at its exact live line in the same environment.
    let control_path = root.join("control-plan.json");
    assert!(!control_path.exists());
    let control = cargo_allow_command()
        .current_dir(&root)
        .args([
            "why",
            "--kind",
            "panic",
            "--path",
            "src/lib.rs",
            "--line",
            "3",
            "--plan",
            "control-plan.json",
        ])
        .output()?;
    assert_status("explicit live-line plan control", &control, true);
    let control_plan: Value = serde_json::from_slice(&fs::read(&control_path)?)?;
    assert_eq!(
        control_plan
            .pointer("/finding/line")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        control_plan
            .pointer("/outcome/status")
            .and_then(Value::as_str),
        Some("new")
    );
    assert_eq!(fs::read(&policy_path)?, policy_before);
    assert_eq!(fs::read(&plan_path)?, old_plan_before);
    let control_head = Command::new("git")
        .current_dir(&root)
        .args(["rev-parse", "HEAD"])
        .output()?;
    assert_status("unchanged fixture HEAD", &control_head, true);
    assert_eq!(control_head.stdout, initial_head.stdout);
    eprintln!(
        "New line 3 and explicit-line-3 plan verified; original plan/policy bytes and HEAD unchanged; printed recovery={printed:?}; hint status={}; hint stderr={hinted_stderr:?}",
        hinted.status
    );

    // Expected RED before the repair: the printed command retains --line 1 and
    // refuses while the independent line-3 control above succeeds. A repaired
    // head must pass this contract and all subsequent normal recovery steps.
    assert_status("verbatim moved-line recovery hint", &hinted, true);
    let retry_before = fs::read(&retry_path)?;
    let retry_plan: Value = serde_json::from_slice(&retry_before)?;
    assert_eq!(
        retry_plan.pointer("/finding/line").and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        retry_plan
            .pointer("/outcome/status")
            .and_then(Value::as_str),
        Some("new")
    );

    let apply = cargo_allow_command()
        .current_dir(&root)
        .args([
            "add",
            "--from-plan",
            retry_argument,
            "--owner",
            "fixture",
            "--reason",
            "moved-line recovery fixture",
            "--update",
            "--summary-format",
            "json",
            "--summary-output",
            "moved-add-receipt.json",
        ])
        .output()?;
    assert_status("apply regenerated moved-line plan", &apply, true);
    let policy_after_apply = fs::read(&policy_path)?;
    assert_ne!(policy_after_apply, policy_before);
    assert_eq!(fs::read(&plan_path)?, old_plan_before);
    assert_eq!(fs::read(&retry_path)?, retry_before);

    let matched_why = cargo_allow_command()
        .current_dir(&root)
        .args([
            "why",
            "--kind",
            "panic",
            "--path",
            "src/lib.rs",
            "--line",
            "3",
            "--format",
            "json",
            "--output",
            "matched-why.json",
        ])
        .output()?;
    assert_status("matched moved-line finding", &matched_why, true);
    let matched_report: Value = serde_json::from_slice(&fs::read(root.join("matched-why.json"))?)?;
    assert_eq!(
        matched_report
            .pointer("/finding/line")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        matched_report
            .pointer("/outcome/status")
            .and_then(Value::as_str),
        Some("matched")
    );

    let replay = cargo_allow_command()
        .current_dir(&root)
        .args([
            "add",
            "--from-plan",
            retry_argument,
            "--owner",
            "fixture",
            "--reason",
            "moved-line recovery fixture",
            "--update",
        ])
        .output()?;
    assert_status("replay regenerated moved-line plan", &replay, false);
    let replay_text = String::from_utf8(replay.stderr)?;
    assert!(replay_text.contains("status `matched`"));
    assert!(replay_text.contains("(policy unchanged)"));
    assert!(
        !replay_text.contains("regenerate with"),
        "matched replay must not advertise New-only regeneration: {replay_text}"
    );
    assert_eq!(fs::read(&policy_path)?, policy_after_apply);
    assert_eq!(fs::read(&plan_path)?, old_plan_before);
    assert_eq!(fs::read(&retry_path)?, retry_before);
    eprintln!(
        "Regenerated plan applied; line 3 matched; replay refused without mutation or impossible regeneration advice"
    );

    remove_temp_root(root);
    Ok(())
}

fn replacement_finding_must_not_receive_recovery_hint(
    label: &str,
    replacement: &str,
    callee: &str,
    same_family: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = init_fixture(label);
    let policy_path = root.join("policy/allow.toml");
    let plan_path = generate_plan(&root);
    let policy_before = fs::read(&policy_path)?;
    let plan_before = fs::read(&plan_path)?;
    let original: Value = serde_json::from_slice(&plan_before)?;
    assert_eq!(
        original
            .pointer("/finding/identity/callee")
            .and_then(Value::as_str),
        Some("unwrap")
    );
    let head_before = Command::new("git")
        .current_dir(&root)
        .args(["rev-parse", "HEAD"])
        .output()?;
    assert_status("initial replacement fixture HEAD", &head_before, true);

    // Delete the recorded semantic target; the only remaining finding is a
    // different New target at line 3. Keep HEAD and tracked paths unchanged.
    fs::write(
        root.join("src/lib.rs"),
        format!("// replacement\n\n{replacement}"),
    )?;
    git(&root, &["add", "src/lib.rs"]);
    let control = cargo_allow_command()
        .current_dir(&root)
        .args([
            "why",
            "--kind",
            "panic",
            "--path",
            "src/lib.rs",
            "--line",
            "3",
            "--plan",
            "replacement-control.json",
        ])
        .output()?;
    assert_status("independent replacement plan control", &control, true);
    let control_path = root.join("replacement-control.json");
    let control_before = fs::read(&control_path)?;
    let replacement_plan: Value = serde_json::from_slice(&control_before)?;
    assert_eq!(
        replacement_plan
            .pointer("/finding/line")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        replacement_plan
            .pointer("/outcome/status")
            .and_then(Value::as_str),
        Some("new")
    );
    assert_eq!(
        replacement_plan
            .pointer("/finding/identity/callee")
            .and_then(Value::as_str),
        Some(callee)
    );
    assert_ne!(
        original.pointer("/finding/digest"),
        replacement_plan.pointer("/finding/digest")
    );
    assert_eq!(
        original.pointer("/finding/family") == replacement_plan.pointer("/finding/family"),
        same_family
    );

    let receipt_path = root.join("rejected-receipt.json");
    assert!(!receipt_path.exists());
    let refused = cargo_allow_command()
        .current_dir(&root)
        .args([
            "add",
            "--from-plan",
            "add-plan.json",
            "--owner",
            "fixture",
            "--reason",
            "replacement-finding recovery regression",
            "--update",
            "--summary-format",
            "json",
            "--summary-output",
            "rejected-receipt.json",
        ])
        .output()?;
    assert_status("stale replaced finding add", &refused, false);
    let rejection = String::from_utf8(refused.stderr)?;
    assert!(
        rejection.contains("source inventory changed since the plan was generated"),
        "{rejection}"
    );
    assert!(rejection.contains("(policy unchanged)"), "{rejection}");
    assert_eq!(fs::read(&policy_path)?, policy_before);
    assert_eq!(fs::read(&plan_path)?, plan_before);
    assert_eq!(fs::read(&control_path)?, control_before);
    assert!(!receipt_path.exists(), "refusal must not claim application");

    // Before the repair, execute the ACTUAL advertised command unchanged and
    // inspect its plan. This proves retargeting, rather than mere bad wording.
    if let Some((_, tail)) = rejection.split_once("; regenerate with ") {
        let (printed, _) = tail
            .split_once(" (")
            .ok_or("hint explanation boundary missing")?;
        let tokens: Vec<_> = printed.split_ascii_whitespace().collect();
        assert_eq!(tokens.first().copied(), Some("cargo-allow"));
        assert_eq!(tokens.get(1).copied(), Some("why"));
        let retry_name = tokens
            .windows(2)
            .find_map(|pair| match pair {
                ["--plan", value] => Some(*value),
                _ => None,
            })
            .ok_or("hint plan argument missing")?;
        assert_eq!(
            Path::new(retry_name)
                .file_name()
                .and_then(std::ffi::OsStr::to_str),
            Some(retry_name)
        );
        let retry_path = root.join(retry_name);
        assert!(!retry_path.exists());
        let hinted = cargo_allow_command()
            .current_dir(&root)
            .args(tokens.iter().skip(1))
            .output()?;
        assert_status("advertised different-target regeneration", &hinted, true);
        let hinted_plan: Value = serde_json::from_slice(&fs::read(&retry_path)?)?;
        assert_eq!(
            hinted_plan.pointer("/finding/digest"),
            replacement_plan.pointer("/finding/digest")
        );
        assert_ne!(
            hinted_plan.pointer("/finding/digest"),
            original.pointer("/finding/digest")
        );
        assert_eq!(fs::read(&policy_path)?, policy_before);
        assert_eq!(fs::read(&plan_path)?, plan_before);
        assert_eq!(fs::read(&control_path)?, control_before);
        assert!(!receipt_path.exists());
        eprintln!(
            "Replacement {label}: stale add refused and preserved policy/plan/output; printed argv {printed:?} succeeded but generated the different {callee} target, not the original unwrap"
        );
    }
    let head_after = Command::new("git")
        .current_dir(&root)
        .args(["rev-parse", "HEAD"])
        .output()?;
    assert_status("unchanged replacement fixture HEAD", &head_after, true);
    assert_eq!(head_before.stdout, head_after.stdout);
    assert!(
        !rejection.contains("regenerate with"),
        "a different semantic target must not be advertised as relocation: {rejection}"
    );
    remove_temp_root(root);
    Ok(())
}

#[test]
fn add_from_plan_replaced_callee_does_not_advertise_recovery()
-> Result<(), Box<dyn std::error::Error>> {
    replacement_finding_must_not_receive_recovery_hint(
        "add-from-plan-replaced-callee",
        "pub fn load() -> usize { Some(1).expect(\"checked\") }\n",
        "expect",
        false,
    )
}

#[test]
fn add_from_plan_replaced_same_family_does_not_advertise_recovery()
-> Result<(), Box<dyn std::error::Error>> {
    replacement_finding_must_not_receive_recovery_hint(
        "add-from-plan-replaced-container",
        "pub fn replacement() -> usize { Some(1).unwrap() }\n",
        "unwrap",
        true,
    )
}
