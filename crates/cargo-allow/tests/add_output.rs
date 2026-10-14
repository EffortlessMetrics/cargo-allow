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
