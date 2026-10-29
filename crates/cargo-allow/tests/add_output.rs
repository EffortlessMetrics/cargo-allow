use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

#[test]
fn add_rejects_unsafe_entry_without_evidence() {
    let root = temp_root("add-unsafe-evidence");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create source dir: {err}")));
    fs::write(root.join("src/lib.rs"), "pub fn read() { unsafe {} }\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write source: {err}")));
    fs::write(
        root.join("policy/allow.toml"),
        r#"schema_version = "0.1"
policy = "cargo-allow"
owner = "core/policy"
status = "active"

[workspace]
root = "."
inventory = "git-tracked"
default_mode = "no-new"
ignored = ["policy/**", "target/**"]
generated = ["target/**", "vendor/**"]

[requirements]
owner_required = true
reason_required = true
classification_required = true
evidence_required = false
expires_or_review_after_required = true
allow_bare_allow_attributes = false
lint_policy_id_required = false
stale_entries_fail = false

[requirements.unsafe]
evidence_required = true
safety_comment_required = false
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));

    let output_policy = root.join("policy/allow.added.toml");
    let result = cargo_allow_command()
        .arg("add")
        .arg("--root")
        .arg(&root)
        .arg("--config")
        .arg("policy/allow.toml")
        .arg("--kind")
        .arg("unsafe")
        .arg("--path")
        .arg("src/lib.rs")
        .arg("--line")
        .arg("1")
        .arg("--owner")
        .arg("core/tests")
        .arg("--reason")
        .arg("Unsafe boundary is reviewed before retention.")
        .arg("--write")
        .arg(&output_policy)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run add: {err}")));

    assert_status("add unsafe without evidence", &result, false);
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("unsafe allow entries require at least one --evidence reference"),
        "stderr should explain the unsafe evidence requirement: {stderr}"
    );
    assert!(
        !output_policy.exists(),
        "add should not write a reviewed policy when unsafe evidence is missing"
    );

    remove_temp_root(root);
}

#[test]
fn add_dry_run_keeps_candidate_stdout_summary_stderr_and_note_terminal_only() {
    // #4533: dry-run has the same stdout-candidate/stderr-summary placement
    // as the applied mode, so it gets the same terminal-only note decision.
    // In captured (non-terminal) output the note must be suppressed — this
    // pins both the placement and the note's terminal-only gate.
    let root = temp_root("add-dry-run-placement");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create src dir: {err}")));
    fs::write(
        root.join("src/lib.rs"),
        "pub fn value(v: Option<u8>) -> u8 { v.unwrap() }
",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write source: {err}")));
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
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));

    let output = cargo_allow_command()
        .current_dir(&root)
        .args([
            "add",
            "--kind",
            "panic",
            "--path",
            "src/lib.rs",
            "--line",
            "1",
            "--owner",
            "core/fixture",
            "--reason",
            "dry-run placement fixture",
            "--dry-run",
        ])
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run add: {err}")));
    assert!(
        output.status.success(),
        "dry-run add must succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("[[allow]]"),
        "stdout must carry the candidate entry: {stdout}"
    );
    assert!(
        !stdout.contains("Outcome:"),
        "stdout must not lead with the summary block: {stdout}"
    );
    assert!(
        stderr.contains("Outcome:"),
        "the summary block must render on stderr: {stderr}"
    );
    assert!(stderr.contains("dry-run: no files written"));
    assert!(
        !stdout.contains("summary follows below on stderr")
            && !stderr.contains("summary follows below on stderr"),
        "the terminal-only note must not leak into captured output"
    );
}

#[test]
fn add_dry_run_summary_output_receives_the_add_summary() {
    // #4532: --summary-output belongs to the add summary in every mode. The
    // old dry-run branch hijacked it for the candidate policy TOML and
    // returned before emitting the summary at all.
    let root = temp_root("add-dry-run-summary");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create src dir: {err}")));
    fs::write(
        root.join("src/lib.rs"),
        "pub fn value(v: Option<u8>) -> u8 { v.unwrap() }
",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write source: {err}")));
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
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));

    let sidecar = root.join("add-summary.json");
    let output = cargo_allow_command()
        .current_dir(&root)
        .args([
            "add",
            "--kind",
            "panic",
            "--path",
            "src/lib.rs",
            "--line",
            "1",
            "--owner",
            "core/fixture",
            "--reason",
            "dry-run summary fixture",
            "--dry-run",
            "--summary-format",
            "json",
            "--summary-output",
        ])
        .arg(&sidecar)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run add: {err}")));
    assert!(
        output.status.success(),
        "dry-run add must succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("[[allow]]"),
        "stdout must carry the candidate entry: {stdout}"
    );
    let summary_text = fs::read_to_string(&sidecar)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read sidecar: {err}")));
    assert!(
        summary_text.contains("\"schema_id\"") && summary_text.contains("cargo-allow.add.v1"),
        "the sidecar must hold the JSON add summary, not the candidate TOML: {summary_text}"
    );
    assert!(
        !stdout.contains("schema_id"),
        "stdout must not carry the summary"
    );
}

#[test]
fn add_candidate_stdout_carries_the_artifact_and_stderr_carries_the_summary() {
    // #4509 pins the documented carve-out (docs/schemas/README.md): `add`
    // stdout carries the candidate policy entry first, and the summary block
    // renders on stderr after it — automation reads the sidecar or stderr
    // for `add`, not stdout. (The terminal-only "summary follows below on
    // stderr" note is not visible through captured output.)
    let root = temp_root("add-summary-placement");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create src dir: {err}")));
    fs::write(
        root.join("src/lib.rs"),
        "pub fn value(v: Option<u8>) -> u8 { v.unwrap() }
",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write source: {err}")));
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
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));

    let output = cargo_allow_command()
        .current_dir(&root)
        .args([
            "add",
            "--kind",
            "panic",
            "--path",
            "src/lib.rs",
            "--line",
            "1",
            "--owner",
            "core/fixture",
            "--reason",
            "placement fixture",
        ])
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run add: {err}")));
    assert!(
        output.status.success(),
        "the candidate add must succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("[[allow]]") && stdout.contains("core/fixture"),
        "stdout must carry the candidate policy entry: {stdout}"
    );
    assert!(
        !stdout.contains("Outcome:"),
        "stdout must not lead with the summary block: {stdout}"
    );
    assert!(
        stderr.contains("Outcome:"),
        "the summary block must render on stderr: {stderr}"
    );
}

fn cargo_allow_command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
}

fn assert_status(command: &str, result: &Output, should_succeed: bool) {
    assert_eq!(
        result.status.success(),
        should_succeed,
        "{command} status mismatch: stdout=`{}` stderr=`{}`",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

fn temp_root(label: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_else(|err| std::panic::panic_any(format!("system clock: {err}")))
        .as_nanos();
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

#[test]
fn add_broad_family_only_names_the_structural_gate_not_the_glob() {
    // #4569: for structural-identity kinds, --family alone can never
    // satisfy the selector gate; the old error blamed the glob ("no
    // current panic findings match glob") even when the glob held matching
    // findings, and its remedy could not lead to the fix.
    let root = temp_root("add-broad-family-gate");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create src dir: {err}")));
    fs::write(
        root.join("src/lib.rs"),
        "pub fn value(v: Option<u8>) -> u8 { v.unwrap() }
",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write source: {err}")));
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
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));

    let output = cargo_allow_command()
        .current_dir(&root)
        .args([
            "add",
            "--kind",
            "panic",
            "--glob",
            "src/lib.rs",
            "--family",
            "unwrap",
            "--owner",
            "core/fixture",
            "--reason",
            "family-only fixture",
            "--dry-run",
        ])
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run broad add: {err}")));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(2),
        "the family-only broad add must be a usage error: {stderr}"
    );
    assert!(
        stderr.contains("findings require a structural selector identity"),
        "the error must name the structural gate: {stderr}"
    );
    assert!(
        stderr.contains("--callee"),
        "the error must direct at --callee: {stderr}"
    );
    // #4588: the suggested callees come from the findings under the glob,
    // not a hardcoded example.
    assert!(
        stderr.contains("callees present under this glob: unwrap"),
        "the error must list the actual callees: {stderr}"
    );
    assert!(
        !stderr.contains("no current panic findings match glob"),
        "the glob-blaming error must be gone: {stderr}"
    );

    remove_temp_root(root);
}

#[test]
fn add_dry_run_write_summary_reports_stdout_not_written() {
    // #4549: --dry-run writes nothing, so the delivered summary must keep the
    // stdout posture even when --write/--update requested a target; a
    // "written" mutation receipt for a file that does not exist would lie to
    // dry-run pipelines.
    let root = temp_root("add-dry-run-write-posture");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create src dir: {err}")));
    fs::write(
        root.join("src/lib.rs"),
        "pub fn value(v: Option<u8>) -> u8 { v.unwrap() }
",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write source: {err}")));
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
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
    let policy_before = fs::read_to_string(root.join("policy/allow.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read policy before: {err}")));

    let sidecar = root.join("summary-write.json");
    let output = cargo_allow_command()
        .current_dir(&root)
        .args([
            "add",
            "--kind",
            "panic",
            "--path",
            "src/lib.rs",
            "--line",
            "1",
            "--owner",
            "core/fixture",
            "--reason",
            "dry-run write posture fixture",
            "--dry-run",
            "--write",
            "candidate.toml",
            "--summary-format",
            "json",
            "--summary-output",
        ])
        .arg(&sidecar)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run add --dry-run --write: {err}")));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "dry-run add --write must succeed: {stderr}"
    );
    assert!(
        !root.join("candidate.toml").exists(),
        "--dry-run must not create the candidate file"
    );
    let summary_text = fs::read_to_string(&sidecar)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read sidecar: {err}")));
    let summary: serde_json::Value = serde_json::from_str(&summary_text)
        .unwrap_or_else(|err| std::panic::panic_any(format!("sidecar must be JSON: {err}")));
    assert_eq!(
        summary
            .pointer("/mutation_receipt/result")
            .and_then(serde_json::Value::as_str),
        Some("stdout"),
        "the mutation receipt must not claim a write under --dry-run: {summary_text}"
    );
    assert!(
        summary
            .pointer("/options/policy_output")
            .map(serde_json::Value::is_null)
            .unwrap_or(false),
        "policy_output must stay unset for a dry-run preview: {summary_text}"
    );
    // #4568: the receipt's verify commands must not lead with the failing
    // explain argv before the entry is persisted.
    let first_next_command = summary
        .pointer("/mutation_receipt/next_commands/0")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    assert!(
        first_next_command.contains("rerun add without --dry-run"),
        "the dry-run receipt must lead with the persist step: {first_next_command}"
    );
    assert!(
        !first_next_command.starts_with("cargo-allow explain"),
        "the dry-run receipt must not lead with the verbatim-failing explain argv: {first_next_command}"
    );

    let sidecar_update = root.join("summary-update.json");
    let output = cargo_allow_command()
        .current_dir(&root)
        .args([
            "add",
            "--kind",
            "panic",
            "--path",
            "src/lib.rs",
            "--line",
            "1",
            "--owner",
            "core/fixture",
            "--reason",
            "dry-run update posture fixture",
            "--id",
            "allow-0002",
            "--dry-run",
            "--update",
            "--summary-format",
            "json",
            "--summary-output",
        ])
        .arg(&sidecar_update)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run add --dry-run --update: {err}")));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "dry-run add --update must succeed: {stderr}"
    );
    let policy_after = fs::read_to_string(root.join("policy/allow.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read policy after: {err}")));
    assert_eq!(
        policy_before, policy_after,
        "--dry-run --update must not change the live ledger"
    );
    let summary_text = fs::read_to_string(&sidecar_update)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read update sidecar: {err}")));
    let summary: serde_json::Value = serde_json::from_str(&summary_text)
        .unwrap_or_else(|err| std::panic::panic_any(format!("sidecar must be JSON: {err}")));
    assert_eq!(
        summary
            .pointer("/mutation_receipt/result")
            .and_then(serde_json::Value::as_str),
        Some("stdout"),
        "the update-preview receipt must not claim a write: {summary_text}"
    );
    assert!(
        summary
            .pointer("/options/policy_output")
            .map(serde_json::Value::is_null)
            .unwrap_or(false),
        "update-preview policy_output must stay unset: {summary_text}"
    );

    remove_temp_root(root);
}

#[test]
fn add_dry_run_broad_json_summary_keeps_the_stdout_posture() {
    // #4549 → #4604: the broad (glob) JSON summary once carried action
    // "write" and the requested candidate path even under --dry-run; the
    // governed shape has no action field at all — the mutation receipt's
    // own result is the posture truth, and a dry run keeps the stdout
    // posture (#4549) while null selected_finding marks the route (#4604).
    let root = temp_root("add-dry-run-broad-posture");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create src dir: {err}")));
    fs::write(
        root.join("src/lib.rs"),
        "pub fn value(v: Option<u8>) -> u8 { v.unwrap() }
",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write source: {err}")));
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
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));

    let sidecar = root.join("summary-broad.json");
    let output = cargo_allow_command()
        .current_dir(&root)
        .args([
            "add",
            "--kind",
            "panic",
            "--glob",
            "src/lib.rs",
            "--family",
            "unwrap",
            "--callee",
            "unwrap",
            "--owner",
            "core/fixture",
            "--reason",
            "dry-run broad posture fixture",
            "--dry-run",
            "--write",
            "candidate.toml",
            "--summary-format",
            "json",
            "--summary-output",
        ])
        .arg(&sidecar)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run broad add: {err}")));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "dry-run broad add must succeed: {stderr}"
    );
    assert!(
        !root.join("candidate.toml").exists(),
        "--dry-run must not create the candidate file"
    );
    let summary_text = fs::read_to_string(&sidecar)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read sidecar: {err}")));
    let summary: serde_json::Value = serde_json::from_str(&summary_text)
        .unwrap_or_else(|err| std::panic::panic_any(format!("sidecar must be JSON: {err}")));
    assert_eq!(
        summary.pointer("/selected_finding").map(|v| v.is_null()),
        Some(true),
        "the broad route must mark itself with a null selected_finding: {summary_text}"
    );
    assert_eq!(
        summary.get("action"),
        None,
        "the redundant action field must be gone from the governed shape: {summary_text}"
    );
    assert_eq!(
        summary
            .pointer("/options/policy_output")
            .map(|v| v.is_null()),
        Some(true),
        "the governed options must report no policy output under --dry-run: {summary_text}"
    );
    assert_eq!(
        summary
            .pointer("/mutation_receipt/result")
            .and_then(serde_json::Value::as_str),
        Some("stdout"),
        "the broad mutation receipt must not claim a write: {summary_text}"
    );

    remove_temp_root(root);
}

#[test]
fn add_dry_run_write_core_summary_does_not_suggest_checking_missing_candidate() {
    // #4550: the Then: proof is argv for verbatim execution, but --dry-run
    // guarantees the candidate path does not exist; the suggested proof must
    // be to persist the candidate first, not a check that fails E0002.
    let root = temp_root("add-dry-run-then-proof");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create src dir: {err}")));
    fs::write(
        root.join("src/lib.rs"),
        "pub fn value(v: Option<u8>) -> u8 { v.unwrap() }
",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write source: {err}")));
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
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));

    let sidecar = root.join("summary.txt");
    let output = cargo_allow_command()
        .current_dir(&root)
        .args([
            "add",
            "--kind",
            "panic",
            "--path",
            "src/lib.rs",
            "--line",
            "1",
            "--owner",
            "core/fixture",
            "--reason",
            "dry-run then-proof fixture",
            "--dry-run",
            "--write",
            "candidate.toml",
            "--summary-output",
        ])
        .arg(&sidecar)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run add: {err}")));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "dry-run add must succeed: {stderr}"
    );
    assert!(
        !root.join("candidate.toml").exists(),
        "--dry-run must not create the candidate file"
    );
    assert!(
        stderr.contains(
            "Then: rerun add without --dry-run to write the candidate, then run the targeted no-new check against it"
        ),
        "the Then: proof must direct the operator to persist the candidate first: {stderr}"
    );
    assert!(
        !stderr.contains("--config candidate.toml"),
        "the Then: proof must not target the candidate path --dry-run guarantees is absent: {stderr}"
    );
    let summary_text = fs::read_to_string(&sidecar)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read sidecar: {err}")));
    assert!(
        summary_text.contains("output: stdout"),
        "the human sidecar must report stdout, not the unwritten candidate path: {summary_text}"
    );

    remove_temp_root(root);
}

#[test]
fn add_dry_run_without_write_target_selects_no_proof_command() {
    // #4550/#4554 review: a bare preview persists nothing and rerunning
    // without --dry-run would still print to stdout, so no verbatim proof
    // command applies; the old adapter suggested a check against the live
    // policy path as if it evaluated the candidate.
    let root = temp_root("add-dry-run-bare-no-proof");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create src dir: {err}")));
    fs::write(
        root.join("src/lib.rs"),
        "pub fn value(v: Option<u8>) -> u8 { v.unwrap() }
",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write source: {err}")));
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
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));

    let output = cargo_allow_command()
        .current_dir(&root)
        .args([
            "add",
            "--kind",
            "panic",
            "--path",
            "src/lib.rs",
            "--line",
            "1",
            "--owner",
            "core/fixture",
            "--reason",
            "bare preview fixture",
            "--dry-run",
        ])
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run add: {err}")));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "bare dry-run add must succeed: {stderr}"
    );
    assert!(
        stderr.contains("Then: no follow-up proof command selected"),
        "a bare preview must select no proof command: {stderr}"
    );
    assert!(
        !stderr.contains("--config"),
        "the summary must not point any check at a policy that lacks the preview: {stderr}"
    );

    remove_temp_root(root);
}

#[test]
fn add_dry_run_update_preview_directs_persist_first() {
    // #4554 review: the live-update preview's old Then: ran the enforcing
    // check against the unchanged ledger, which cannot verify the previewed
    // entry; it must direct the operator to persist first, like the
    // candidate-write preview does.
    let root = temp_root("add-dry-run-update-proof");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create src dir: {err}")));
    fs::write(
        root.join("src/lib.rs"),
        "pub fn value(v: Option<u8>) -> u8 { v.unwrap() }
",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write source: {err}")));
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
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));

    let output = cargo_allow_command()
        .current_dir(&root)
        .args([
            "add",
            "--kind",
            "panic",
            "--path",
            "src/lib.rs",
            "--line",
            "1",
            "--owner",
            "core/fixture",
            "--reason",
            "update preview fixture",
            "--dry-run",
            "--update",
        ])
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run add: {err}")));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "update-preview dry-run add must succeed: {stderr}"
    );
    // #4571: Next: names the persist step and must not carry the
    // verbatim-failing explain argv; Then: is the enforcing check, which is
    // runnable after the operator persists.
    assert!(
        stderr.contains(
            "Next: rerun add without --dry-run to write this entry into the live ledger, then inspect it with explain"
        ),
        "the update preview's Next: must direct the operator to persist first: {stderr}"
    );
    assert!(
        !stderr.contains("explain allow-"),
        "the update preview must not suggest explain before the entry exists: {stderr}"
    );
    assert!(
        stderr.contains("Then: cargo-allow check --mode no-new"),
        "the update preview's Then: proof must be the runnable enforcing check: {stderr}"
    );

    remove_temp_root(root);
}

#[test]
fn add_broad_family_excluded_glob_keeps_the_empty_scope_message() {
    // #4576 review: the blocked-findings count applies the same --family
    // filter the broad selector will, so a family-excluded glob is
    // genuinely empty for this request and keeps the empty-scope message
    // instead of the structural-gate one that --callee cannot fix.
    let root = temp_root("add-broad-family-excluded");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create src dir: {err}")));
    fs::write(
        root.join("src/lib.rs"),
        "pub fn value(v: Option<u8>) -> u8 { v.unwrap() }\\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write source: {err}")));
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
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));

    let output = cargo_allow_command()
        .current_dir(&root)
        .args([
            "add",
            "--kind",
            "panic",
            "--glob",
            "src/lib.rs",
            "--family",
            "somewhere-else",
            "--owner",
            "core/fixture",
            "--reason",
            "family-excluded fixture",
            "--dry-run",
        ])
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run broad add: {err}")));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(2),
        "a family-excluded glob must be a usage error: {stderr}"
    );
    // #4587: the glob DOES hold matching-kind findings excluded only by
    // --family, so the message must name the filters, not blame the glob.
    assert!(
        stderr.contains("findings match glob `src/lib.rs`"),
        "the message must reference the glob: {stderr}"
    );
    assert!(
        stderr.contains("--family"),
        "the message must name the excluding filter: {stderr}"
    );
    assert!(
        !stderr.contains("cannot baseline an empty scope"),
        "the glob is not empty; the empty-scope message must not fire: {stderr}"
    );
    assert!(
        !stderr.contains("structural selector identity"),
        "the structural-gate message must not fire for a family-excluded glob: {stderr}"
    );

    remove_temp_root(root);
}

fn add_inference_fixture(label: &str) -> PathBuf {
    // One unwrap finding at src/lib.rs:1 and the minimal live policy.
    let root = temp_root(label);
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create src dir: {err}")));
    fs::write(
        root.join("src/lib.rs"),
        "pub fn value(v: Option<u8>) -> u8 { v.unwrap() }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write source: {err}")));
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
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
    root
}

/// #4606/#3194: `add --path --line` without `--kind` must reach cmd_add and
/// infer the kind from the scanned finding — before #4606 clap rejected the
/// invocation with "required arguments were not provided: --kind" before the
/// inference branch could run.
#[test]
fn add_path_line_without_kind_infers_the_kind_from_the_finding() -> Result<(), std::io::Error> {
    let root = add_inference_fixture("add-kind-inference");
    let output = cargo_allow_command()
        .current_dir(&root)
        .args([
            "add",
            "--path",
            "src/lib.rs",
            "--line",
            "1",
            "--owner",
            "core/fixture",
            "--reason",
            "kind inference fixture",
            "--dry-run",
        ])
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "path+line without --kind must reach kind inference and succeed: {stderr}"
    );
    assert!(
        stdout.contains("[[allow]]") && stdout.contains("kind = \"panic\""),
        "the candidate must carry the inferred panic kind: {stdout}"
    );
    assert!(
        !stderr.contains("required arguments were not provided"),
        "clap must not reject the inference invocation: {stderr}"
    );

    remove_temp_root(root);
    Ok(())
}

/// #4606: the relaxed clap gate must not let an invalid combination through —
/// `add --glob` without `--kind` still fails, now with the typed broad-scope
/// usage error from cmd_add (a broad scope cannot infer a kind).
#[test]
fn add_glob_without_kind_fails_with_the_typed_broad_scope_error() -> Result<(), std::io::Error> {
    let root = add_inference_fixture("add-glob-without-kind");
    let output = cargo_allow_command()
        .current_dir(&root)
        .args([
            "add",
            "--glob",
            "src/lib.rs",
            "--owner",
            "core/fixture",
            "--reason",
            "glob without kind fixture",
            "--dry-run",
        ])
        .output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(2),
        "glob without --kind must be a usage error: {stderr}"
    );
    assert!(
        stderr.contains("--kind is required with --glob"),
        "the typed broad-scope error must fire: {stderr}"
    );

    remove_temp_root(root);
    Ok(())
}

/// #4606: with no kind, no target selector, and no plan, `add` still fails
/// fast at parse time with clap's missing-argument error — unchanged.
#[test]
fn add_without_kind_or_target_still_requires_kind_at_parse_time() -> Result<(), std::io::Error> {
    let root = add_inference_fixture("add-still-requires-kind");
    let output = cargo_allow_command()
        .current_dir(&root)
        .args(["add", "--owner", "core/fixture", "--reason", "bare fixture"])
        .output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(2),
        "bare add must be rejected at parse time: {stderr}"
    );
    assert!(
        stderr.contains("required arguments were not provided"),
        "clap must still require --kind without a target selector: {stderr}"
    );
    assert!(
        stderr.contains("--kind"),
        "the rejection must name --kind: {stderr}"
    );

    remove_temp_root(root);
    Ok(())
}

/// #4610: the no-current-finding add error must render the typed next step on
/// stderr and must not append the contradictory generic "review the command
/// syntax" line after the message already names the real remedy.
#[test]
fn add_no_finding_error_renders_typed_next_step_not_generic_syntax_line()
-> Result<(), std::io::Error> {
    let root = add_inference_fixture("add-no-finding-next-step");
    // A source file with no findings at all: the add request is syntactically
    // valid, so the generic syntax remedy would be contradictory.
    fs::write(root.join("src/empty.rs"), "pub fn plain() -> u8 { 0 }\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write empty source: {err}")));
    let output = cargo_allow_command()
        .current_dir(&root)
        .args([
            "add",
            "--kind",
            "panic",
            "--path",
            "src/empty.rs",
            "--line",
            "1",
            "--owner",
            "core/fixture",
            "--reason",
            "no-finding fixture",
            "--dry-run",
        ])
        .output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(2),
        "adding where no finding exists must be a usage error: {stderr}"
    );
    assert!(
        stderr.contains("no current panic finding found near"),
        "the no-finding message must render: {stderr}"
    );
    assert!(
        stderr.contains("Next: pass the exact --line"),
        "the typed next step must render: {stderr}"
    );
    assert!(
        !stderr.contains("review the command syntax"),
        "the generic syntax line must not follow the typed remedy: {stderr}"
    );

    remove_temp_root(root);
    Ok(())
}

/// #4610: the ambiguous-add error must render the typed next step pointing at
/// the read-only exact-line listing, not the generic syntax line.
#[test]
fn add_ambiguous_error_renders_typed_next_step_not_generic_syntax_line()
-> Result<(), std::io::Error> {
    let root = temp_root("add-ambiguous-next-step");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create src dir: {err}")));
    // Unwraps on lines 1 and 3: --line 2 is an exact tie.
    fs::write(
        root.join("src/amb.rs"),
        "pub fn a(v: Option<u8>) -> u8 { v.unwrap() }\npub fn pad() -> u8 { 0 }\npub fn b(v: Option<u8>) -> u8 { v.unwrap() }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write ambiguous source: {err}")));
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
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
    let output = cargo_allow_command()
        .current_dir(&root)
        .args([
            "add",
            "--kind",
            "panic",
            "--path",
            "src/amb.rs",
            "--line",
            "2",
            "--owner",
            "core/fixture",
            "--reason",
            "ambiguous fixture",
            "--dry-run",
        ])
        .output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(2),
        "an ambiguous add request must be a usage error: {stderr}"
    );
    assert!(
        stderr.contains("ambiguous add request"),
        "the ambiguous message must render: {stderr}"
    );
    assert!(
        stderr.contains("Next: run `cargo-allow check --kind panic --format json`"),
        "the typed next step must render: {stderr}"
    );
    assert!(
        !stderr.contains("review the command syntax"),
        "the generic syntax line must not follow the typed remedy: {stderr}"
    );

    remove_temp_root(root);
    Ok(())
}
