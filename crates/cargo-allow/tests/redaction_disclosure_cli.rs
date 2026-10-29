//! Identity redaction versus the exception ledger (#4628, #4629).
//!
//! Redaction clears source-text identity fields at scan time, *before*
//! matching, so ledger entries that pin those fields can never match a
//! redacted scan. These tests pin the repair's contract end to end:
//!
//! * a structural entry that dead-ends under redaction is disclosed as a
//!   typed hard error in `check` (unredacted scans of the same fixture pass);
//! * a snippet-hash-anchored entry still matches under redaction — the
//!   preserved promise;
//! * `why` under redaction carries no raw receiver fingerprint text (#4629);
//! * `add` under redaction generates a snippet-anchored entry and prints no
//!   dead-field warning.

use allow_core::{normalize_snippet, stable_hash_hex};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const RECEIPT_LINE: &str = "    value.unwrap()";
const STRUCTURAL_ENTRY_SECRET: &str = "container, callee";
const RAW_RECEIVER: &str = "customer_ssn";

struct TempGuard(PathBuf);

impl Drop for TempGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn make_fixture(label: &str, source: &str) -> Result<(TempGuard, PathBuf), String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-redaction-disclosure-{label}-{}-{stamp}",
        std::process::id()
    ));
    let guard = TempGuard(root.clone());
    fs::create_dir_all(root.join("src")).map_err(|error| error.to_string())?;
    fs::create_dir_all(root.join("policy")).map_err(|error| error.to_string())?;
    fs::write(root.join("src/lib.rs"), source).map_err(|error| error.to_string())?;
    fs::write(
        root.join("policy/allow.toml"),
        "schema_version = 1\n\n[workspace]\nignored = [\"policy/allow.toml\"]\ngenerated = []\n",
    )
    .map_err(|error| error.to_string())?;
    for args in [
        vec!["init"],
        vec!["config", "user.email", "cargo-allow@example.invalid"],
        vec!["config", "user.name", "cargo-allow test"],
        vec!["add", "--all"],
        vec!["commit", "-m", "redaction disclosure fixture"],
    ] {
        let status = Command::new("git")
            .args(&args)
            .current_dir(&root)
            .status()
            .map_err(|error| error.to_string())?;
        if !status.success() {
            return Err(format!("git {:?} failed with {status}", args));
        }
    }
    Ok((guard, root))
}

/// One cargo-allow invocation with `CARGO_ALLOW_REDACT_IDENTITY` controlled
/// per call (every invocation is a separate process, so there are no env
/// races).
fn run_tool(root: &Path, args: &[&str], redacted: bool) -> Result<(i32, String, String), String> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-allow"));
    command
        .args(args)
        .current_dir(root)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    if redacted {
        command.env("CARGO_ALLOW_REDACT_IDENTITY", "1");
    } else {
        command.env_remove("CARGO_ALLOW_REDACT_IDENTITY");
    }
    let output = command.output().map_err(|error| error.to_string())?;
    let stdout = String::from_utf8(output.stdout).map_err(|error| error.to_string())?;
    let stderr = String::from_utf8(output.stderr).map_err(|error| error.to_string())?;
    Ok((output.status.code().unwrap_or(-1), stdout, stderr))
}

fn check_args() -> Vec<&'static str> {
    vec![
        "check",
        "--config",
        "policy/allow.toml",
        "--mode",
        "no-new",
        "--format",
        "json",
    ]
}

/// Receipt the fixture's unwrap occurrence the way an unredacted operator
/// does: the generated entry pins container + callee (+ the param-slot
/// receiver fingerprint), which is exactly the shape that dead-ends under
/// redaction (#4628).
fn receipt_structural_entry(root: &Path) -> Result<(), String> {
    let (code, _stdout, stderr) = run_tool(
        root,
        &[
            "add",
            "--config",
            "policy/allow.toml",
            "--kind",
            "panic",
            "--path",
            "src/lib.rs",
            "--line",
            "2",
            "--owner",
            "test",
            "--reason",
            "fixture receipt",
            "--evidence",
            "test:fixture",
            "--review-after",
            "2099-12-31",
            "--update",
        ],
        false,
    )?;
    if code != 0 {
        return Err(format!("unredacted add failed: {stderr}"));
    }
    let policy =
        fs::read_to_string(root.join("policy/allow.toml")).map_err(|error| error.to_string())?;
    if !policy.contains("container") || !policy.contains("callee") {
        return Err("receipted entry must pin container and callee for this test".to_string());
    }
    Ok(())
}

#[test]
fn structural_entry_dead_under_redaction_is_disclosed() -> Result<(), String> {
    let (_guard, root) = make_fixture(
        "disclosure",
        "pub fn load(value: Option<u32>) -> u32 {\n    value.unwrap()\n}\n",
    )?;
    receipt_structural_entry(&root)?;

    // Control: the same ledger and scan pass unredacted — the disclosure must
    // not fire when redaction is off.
    let (unredacted_code, unredacted_stdout, unredacted_stderr) =
        run_tool(&root, &check_args(), false)?;
    if unredacted_code != 0 {
        return Err(format!(
            "unredacted control check must pass: stdout={unredacted_stdout} stderr={unredacted_stderr}"
        ));
    }

    // #4628: under redaction the entry can never match, so check fails closed
    // with a typed disclosure naming the entry, the dead fields, and the Next
    // step — not a silent matched:0 report.
    let (redacted_code, redacted_stdout, redacted_stderr) = run_tool(&root, &check_args(), true)?;
    if redacted_code == 0 {
        return Err(format!(
            "redacted check must fail closed on a dead structural entry: {redacted_stdout}"
        ));
    }
    for expected in [
        "CARGO_ALLOW_REDACT_IDENTITY=1 is active",
        "`allow-0001`",
        STRUCTURAL_ENTRY_SECRET,
        "Next: re-run without CARGO_ALLOW_REDACT_IDENTITY=1",
    ] {
        if !redacted_stderr.contains(expected) {
            return Err(format!(
                "disclosure must contain {expected:?}; stderr was: {redacted_stderr}"
            ));
        }
    }
    Ok(())
}

#[test]
fn snippet_hash_anchored_entry_still_matches_under_redaction() -> Result<(), String> {
    let (_guard, root) = make_fixture(
        "preserved-promise",
        "pub fn load(value: Option<u32>) -> u32 {\n    value.unwrap()\n}\n",
    )?;
    // The snippet-hash anchor is computed exactly as the scanner computes it
    // (stable_hash_hex over the normalized finding line), so this entry
    // matches the finding on the ExactOccurrence tier in both modes without
    // pinning any redacted field.
    let hash = stable_hash_hex(&normalize_snippet(RECEIPT_LINE));
    let policy = format!(
        "schema_version = 1\n\n[workspace]\nignored = [\"policy/allow.toml\"]\ngenerated = []\n\n\
         [[allow]]\nid = \"allow-0001\"\nkind = \"panic\"\nfamily = \"unwrap\"\npath = \"src/lib.rs\"\n\
         owner = \"test\"\nclassification = \"reviewed_exception\"\nreason = \"snippet-anchored\"\n\
         evidence = [\"test:fixture\"]\ncreated = \"2026-10-17\"\nreview_after = \"2099-12-31\"\n\n\
         [allow.selector]\nast_kind = \"method_call\"\nnormalized_snippet_hash = \"{hash}\"\n\n\
         [allow.last_seen]\nline = 2\ncolumn = 11\n"
    );
    fs::write(root.join("policy/allow.toml"), policy).map_err(|error| error.to_string())?;

    for (redacted, label) in [(false, "unredacted"), (true, "redacted")] {
        let (code, stdout, stderr) = run_tool(&root, &check_args(), redacted)?;
        if code != 0 {
            return Err(format!(
                "{label} check must pass for a snippet-hash-anchored entry: \
                 stdout={stdout} stderr={stderr}"
            ));
        }
        let report: Value = serde_json::from_str(&stdout)
            .map_err(|error| format!("{label} report JSON: {error}: {stdout}"))?;
        let matched = report
            .pointer("/summary/matched")
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("{label} report has no matched count: {stdout}"))?;
        if matched != 1 {
            return Err(format!(
                "snippet-hash tier must match under {label} scan, matched={matched}: {stdout}"
            ));
        }
    }
    Ok(())
}

#[test]
fn redacted_why_carries_no_raw_receiver_fingerprint_text() -> Result<(), String> {
    // #4629 repro: the receiver is a local binding, so its fingerprint is raw
    // node text; under redaction it must be cleared, and the source-derived
    // identifier must not appear anywhere in the artifact.
    let (_guard, root) = make_fixture(
        "why-fingerprint",
        "pub fn load() -> u32 {\n    // pending audit hook\n    let customer_ssn: Option<u32> = None;\n    customer_ssn.unwrap()\n}\n",
    )?;
    let why_args = [
        "why",
        "--config",
        "policy/allow.toml",
        "--kind",
        "panic",
        "--path",
        "src/lib.rs",
        "--line",
        "4",
        "--format",
        "json",
    ];

    // Sensitivity guard: unredacted, the raw receiver text IS emitted, so the
    // redacted assertion below exercises the clearing behavior.
    let (unredacted_code, unredacted_stdout, unredacted_stderr) =
        run_tool(&root, &why_args, false)?;
    if unredacted_code != 0 {
        return Err(format!("unredacted why failed: {unredacted_stderr}"));
    }
    if !unredacted_stdout.contains(RAW_RECEIVER) {
        return Err(format!(
            "fixture must emit the raw receiver fingerprint unredacted: {unredacted_stdout}"
        ));
    }

    let (redacted_code, redacted_stdout, redacted_stderr) = run_tool(&root, &why_args, true)?;
    if redacted_code != 0 {
        return Err(format!(
            "redacted why must succeed: stdout={redacted_stdout} stderr={redacted_stderr}"
        ));
    }
    if redacted_stdout.contains(RAW_RECEIVER) || redacted_stderr.contains(RAW_RECEIVER) {
        return Err(format!(
            "redacted why leaked the raw receiver fingerprint: {redacted_stdout} {redacted_stderr}"
        ));
    }
    let report: Value = serde_json::from_str(&redacted_stdout)
        .map_err(|error| format!("redacted why JSON: {error}: {redacted_stdout}"))?;
    let receiver = report
        .pointer("/finding/identity/receiver_fingerprint")
        .map(|value| value.is_null())
        .ok_or_else(|| "redacted why JSON has no identity.receiver_fingerprint".to_string())?;
    if !receiver {
        return Err(format!(
            "receiver_fingerprint must be null under redaction: {redacted_stdout}"
        ));
    }
    Ok(())
}

#[test]
fn redacted_add_of_clean_receipt_prints_no_dead_field_warning() -> Result<(), String> {
    // #4628: add never refuses under redaction, and an entry generated from a
    // redacted finding anchors on normalized_snippet_hash alone, so no
    // dead-field warning may print.
    let (_guard, root) = make_fixture(
        "clean-add",
        "pub fn load(value: Option<u32>) -> u32 {\n    value.unwrap()\n}\n",
    )?;
    let (code, stdout, stderr) = run_tool(
        &root,
        &[
            "add",
            "--config",
            "policy/allow.toml",
            "--kind",
            "panic",
            "--path",
            "src/lib.rs",
            "--line",
            "2",
            "--owner",
            "test",
            "--reason",
            "redacted receipt",
            "--evidence",
            "test:fixture",
            "--review-after",
            "2099-12-31",
            "--update",
        ],
        true,
    )?;
    if code != 0 {
        return Err(format!("redacted add failed: {stdout} {stderr}"));
    }
    if stderr.contains("only match unredacted scans") {
        return Err(format!(
            "snippet-anchored redacted receipt must not warn: {stderr}"
        ));
    }
    let policy =
        fs::read_to_string(root.join("policy/allow.toml")).map_err(|error| error.to_string())?;
    for absent in ["container", "callee", RAW_RECEIVER] {
        if policy.contains(absent) {
            return Err(format!(
                "entry generated under redaction must not pin {absent}: {policy}"
            ));
        }
    }
    if !policy.contains("normalized_snippet_hash") {
        return Err(format!(
            "entry generated under redaction must anchor on the snippet hash: {policy}"
        ));
    }
    Ok(())
}
