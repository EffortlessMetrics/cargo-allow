//! #4645: real CLI `check --mode no-new` fixtures — equivalent source forms
//! must not receive a zero-finding clean result merely because of spelling.
//!
//! Raw-identifier and comment/space spellings must produce the same
//! selected-kind findings, report `failed` flag, receipt status/failure flag,
//! and process exit as their ordinary spellings; string/comment-only content
//! must stay clean. Modeled on the issue's self-contained reproduction and
//! the `bare_allow_conflict_agreement` convention: inlined subprocess helpers,
//! no shared support module, Result-style bodies without unwrap/expect.

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn cargo_allow_command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
}

fn temp_root(label: &str) -> Result<PathBuf, String> {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("system clock: {error}"))?
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-spelling-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&root).map_err(|error| format!("create temp root: {error}"))?;
    Ok(root)
}

fn remove_temp_root(root: PathBuf) -> Result<(), String> {
    match fs::remove_dir_all(&root) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(format!("remove temp root {}: {err}", root.display())),
    }
}

fn write_fixture(root: &Path, source: &str) -> Result<(), String> {
    fs::create_dir_all(root.join("src")).map_err(|error| format!("create src: {error}"))?;
    fs::create_dir_all(root.join("policy")).map_err(|error| format!("create policy: {error}"))?;
    fs::write(root.join("policy/allow.toml"), "policy = \"cargo-allow\"\n")
        .map_err(|error| format!("write policy: {error}"))?;
    fs::write(root.join("src/lib.rs"), source)
        .map_err(|error| format!("write fixture source: {error}"))?;
    Ok(())
}

fn receipt_path(root: &Path) -> PathBuf {
    root.join("target/cargo-allow/receipt.json")
}

/// Run the enforcing selected-kind no-new check and return
/// (exit success, report JSON, receipt JSON).
fn run_no_new(root: &Path, kind: &str) -> Result<(bool, Value, Value), String> {
    let root_text = root.to_str().ok_or("fixture root is not UTF-8")?;
    let receipt = receipt_path(root);
    let receipt_text = receipt.to_str().ok_or("receipt path is not UTF-8")?;
    let output = cargo_allow_command()
        .args([
            "check",
            "--root",
            root_text,
            "--kind",
            kind,
            "--mode",
            "no-new",
            "--format",
            "json",
            "--persistent-cache",
            "off",
            "--receipt",
            receipt_text,
        ])
        .output()
        .map_err(|error| format!("run check: {error}"))?;
    let report: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("parse check report JSON: {error}"))?;
    let receipt_text = fs::read_to_string(&receipt)
        .map_err(|error| format!("read receipt {}: {error}", receipt.display()))?;
    let receipt_json: Value = serde_json::from_str(&receipt_text)
        .map_err(|error| format!("parse receipt JSON: {error}"))?;
    Ok((output.status.success(), report, receipt_json))
}

fn families(report: &Value) -> Vec<String> {
    // Use `pointer`/`get`, never `Value` indexing: `[...]` on a serde_json
    // `Value` is a real index expression the scanner must receipt.
    report
        .pointer("/findings")
        .and_then(Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("report findings must be an array"))
        .iter()
        .filter_map(|finding| {
            finding
                .get("family")
                .and_then(Value::as_str)
                .map(std::string::ToString::to_string)
        })
        .collect()
}

fn assert_failed(
    label: &str,
    kind: &str,
    expected_families: &[&str],
    (success, report, receipt): (bool, Value, Value),
) -> Result<(), String> {
    if success {
        return Err(format!(
            "{label}/{kind}: enforcing no-new check must exit non-zero"
        ));
    }
    let failed = report
        .get("failed")
        .and_then(Value::as_bool)
        .ok_or("report failed flag")?;
    if !failed {
        return Err(format!("{label}/{kind}: report must be failed"));
    }
    let receipt_status = receipt
        .get("status")
        .and_then(Value::as_str)
        .ok_or("receipt status")?;
    if receipt_status != "failed" {
        return Err(format!(
            "{label}/{kind}: receipt status must be failed, got {receipt_status}"
        ));
    }
    if receipt.get("failed").and_then(Value::as_bool) != Some(true) {
        return Err(format!("{label}/{kind}: receipt failed flag must be true"));
    }
    let mut observed = families(&report);
    observed.sort();
    let mut expected = expected_families
        .iter()
        .map(|family| (*family).to_string())
        .collect::<Vec<_>>();
    expected.sort();
    if observed != expected {
        return Err(format!(
            "{label}/{kind}: families must be {expected:?}, got {observed:?}"
        ));
    }
    Ok(())
}

const ORDINARY_FIXTURE: &str = "#[allow(dead_code)]\nfn u(v: Option<u8>) -> u8 { v.unwrap() }\nfn e(v: Option<u8>) -> u8 { v.expect(\"retained\") }\nfn p() { std::panic!(\"retained\"); }\n";

/// Same source with `r#allow`, `r#unwrap`, `r#expect`, and `r#panic` — the
/// issue's minimal fixture. Spelling alone must not change the verdict.
const RAW_FIXTURE: &str = "#[r#allow(dead_code)]\nfn u(v: Option<u8>) -> u8 { v.r#unwrap() }\nfn e(v: Option<u8>) -> u8 { v.r#expect(\"retained\") }\nfn p() { std::r#panic!(\"retained\"); }\n";

/// Legal spaces and comments around a qualified macro path, a raw attribute
/// name, and a comment between the attribute name and its parenthesis.
const TRIVIA_FIXTURE: &str = "fn p() { std :: panic!(\"a\"); std::/* note */panic!(\"b\"); }\n#[r#allow(dead_code)]\n#[allow/* note */(dead_code)]\nfn q() {}\n";

const STRING_ONLY_FIXTURE: &str = "fn p() { let _ = \"r#unwrap std::r#panic!\"; } // panic!()\n";

#[test]
fn raw_spellings_fail_no_new_like_ordinary_spellings() -> Result<(), String> {
    for (label, source) in [("ordinary", ORDINARY_FIXTURE), ("raw", RAW_FIXTURE)] {
        let root = temp_root(label)?;
        let outcome = (|root: &Path| -> Result<(), String> {
            write_fixture(root, source)?;
            let panic_run = run_no_new(root, "panic")?;
            assert_failed(
                label,
                "panic",
                &["panic_macro", "unwrap", "expect"],
                panic_run,
            )?;
            let lint_run = run_no_new(root, "lint_exception")?;
            assert_failed(label, "lint_exception", &["allow_attribute"], lint_run)
        })(&root);
        remove_temp_root(root)?;
        outcome?;
    }
    Ok(())
}

#[test]
fn trivia_spellings_fail_no_new() -> Result<(), String> {
    let root = temp_root("trivia")?;
    let outcome = (|root: &Path| -> Result<(), String> {
        write_fixture(root, TRIVIA_FIXTURE)?;
        let panic_run = run_no_new(root, "panic")?;
        assert_failed(
            "trivia",
            "panic",
            &["panic_macro", "panic_macro"],
            panic_run,
        )?;
        let lint_run = run_no_new(root, "lint_exception")?;
        assert_failed(
            "trivia",
            "lint_exception",
            &["allow_attribute", "allow_attribute"],
            lint_run,
        )
    })(&root);
    remove_temp_root(root)?;
    outcome
}

#[test]
fn string_only_fixture_stays_clean() -> Result<(), String> {
    let root = temp_root("string-only")?;
    let outcome = (|root: &Path| -> Result<(), String> {
        write_fixture(root, STRING_ONLY_FIXTURE)?;
        for kind in ["panic", "lint_exception"] {
            let (success, report, receipt) = run_no_new(root, kind)?;
            if !success {
                return Err(format!(
                    "string-only/{kind}: no-new check must exit 0 for string/comment content"
                ));
            }
            if report.get("failed").and_then(Value::as_bool) != Some(false) {
                return Err(format!("string-only/{kind}: report must not be failed"));
            }
            if receipt.get("status").and_then(Value::as_str) != Some("passed")
                || receipt.get("failed").and_then(Value::as_bool) != Some(false)
            {
                return Err(format!(
                    "string-only/{kind}: receipt must be passed/not-failed: {receipt}"
                ));
            }
            if !families(&report).is_empty() {
                return Err(format!(
                    "string-only/{kind}: report must have no findings: {report}"
                ));
            }
        }
        Ok(())
    })(&root);
    remove_temp_root(root)?;
    outcome
}
