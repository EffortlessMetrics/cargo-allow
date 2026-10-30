//! #4677 end-to-end: `check --include-untracked` over a junctioned source
//! directory must never double-count the target tree under the junction
//! spelling, must skip and disclose the alias traversal, and must fail every
//! enforcing mode through the #4646 completeness gate. Junction creation is
//! Windows-only; the tests degrade to a reported skip when `mklink /J` is
//! unavailable.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn unique_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    root
}

fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap_or_else(|err| {
            std::panic::panic_any(format!("create {}: {err}", parent.display()))
        });
    }
    fs::write(path, contents)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write {}: {err}", path.display())));
}

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("git {args:?}: {err}")));
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn create_dir_junction(target: &Path, link: &Path) -> bool {
    let output = Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .output();
    match output {
        Ok(output) if output.status.success() => true,
        Ok(output) => {
            eprintln!(
                "skipped: junction creation failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
            false
        }
        Err(err) => {
            eprintln!("skipped: junction spawn failed: {err}");
            false
        }
    }
}

const FIXTURE_POLICY: &str = r#"schema_version = "0.1"
policy = "cargo-allow"

[workspace]
ignored = [".git/**", "target/**"]

[[allow]]
id = "allow-policy"
kind = "non_rust_file"
family = "configuration"
path = "policy/allow.toml"
owner = "core"
classification = "fixture"
reason = "self-receipt for the fixture policy"
evidence = ["doc:policy/allow.toml"]
created = "2026-05-09"
review_after = "2027-08-01"

[allow.selector]
ast_kind = "tracked_file"
symbol = "policy/allow.toml"
target_fingerprint = "toml"
glob = "policy/allow.toml"
"#;

/// Tracked `src/lib.rs` plus an untracked `real/lib.rs` that is also reachable
/// through a `linked_src` junction. git's untracked listing sees the target
/// directory under both spellings.
fn build_junction_fixture(label: &str, real_source: &str) -> Option<PathBuf> {
    let root = unique_root(label);
    write_file(&root.join("src/lib.rs"), "pub fn ok() {}\n");
    write_file(&root.join("real/lib.rs"), real_source);
    write_file(&root.join("policy/allow.toml"), FIXTURE_POLICY);
    git(&root, &["init", "-q"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "src/lib.rs", "policy/allow.toml"]);
    if !create_dir_junction(&root.join("real"), &root.join("linked_src")) {
        let _ = fs::remove_dir_all(&root);
        return None;
    }
    Some(root)
}

struct CheckRun {
    exit: i32,
    report: serde_json::Value,
    stdout: String,
}

fn run_check(root: &Path, mode: &str, kind: Option<&str>) -> CheckRun {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-allow"));
    command.arg("check").arg("--mode").arg(mode);
    command.arg("--include-untracked");
    command.arg("--root").arg(root);
    if let Some(kind) = kind {
        command.arg("--kind").arg(kind);
    }
    command.arg("--format").arg("json");
    command.arg("--persistent-cache").arg("off");
    let output = command
        .current_dir(root)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run check {mode}: {err}")));
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let report: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|err| {
        std::panic::panic_any(format!("parse check report: {err}: {stdout}"))
    });
    CheckRun {
        exit: output.status.code().unwrap_or(-1),
        report,
        stdout,
    }
}

/// A clean tree whose only defect is the junction alias: the enforcing gate
/// must fail with the partial disclosure, and the findings-free report must
/// carry no alias spelling at all.
#[test]
fn check_include_untracked_fails_the_gate_on_junction_alias_disclosure() {
    let Some(root) = build_junction_fixture("check-junction-gate", "pub fn ok() {}\n") else {
        return;
    };
    let run = run_check(&root, "no-new", None);

    assert_ne!(
        run.exit, 0,
        "a junction-alias disclosure must fail the enforcing gate: {}",
        run.stdout
    );
    assert_eq!(
        run.report
            .pointer("/inventory/completeness")
            .and_then(serde_json::Value::as_str),
        Some("partial"),
        "the report must disclose the partial inventory"
    );
    // The fixture is finding-free, so the failure can only come from the
    // completeness gate: every outcome must be matched (the self-receipted
    // policy file) and no alias spelling may appear anywhere in the report.
    let outcomes = run
        .report
        .get("outcomes")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    assert!(
        outcomes.iter().all(
            |outcome| outcome.get("status").and_then(serde_json::Value::as_str) == Some("matched")
        ),
        "a finding-free fixture must produce only matched outcomes: {outcomes:?}"
    );
    let rendered = serde_json::to_string(&run.report)
        .unwrap_or_else(|err| std::panic::panic_any(format!("re-render report: {err}")));
    assert!(
        !rendered.contains("linked_src"),
        "no alias spelling may appear anywhere in the report: {rendered}"
    );
    let _ = fs::remove_dir_all(&root);
}

/// The zero-signal double count: pre-repair, the unreceipted `panic!` in the
/// junction target was reported twice — once under each spelling. The repaired
/// inventory scans it exactly once, and no alias spelling reaches any finding
/// or outcome.
#[test]
fn check_include_untracked_never_double_counts_findings_behind_junction() {
    let Some(root) = build_junction_fixture(
        "check-junction-double",
        "fn probe() { panic!(\"retained\"); }\nfn main() {}\n",
    ) else {
        return;
    };
    let run = run_check(&root, "audit", Some("panic"));

    assert_eq!(run.exit, 0, "audit stays advisory: {}", run.stdout);
    let outcomes = run
        .report
        .get("outcomes")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    assert_eq!(
        outcomes.len(),
        1,
        "the junction target must be scanned exactly once: {outcomes:?}"
    );
    let rendered = serde_json::to_string(&run.report)
        .unwrap_or_else(|err| std::panic::panic_any(format!("re-render report: {err}")));
    assert!(
        !rendered.contains("linked_src"),
        "no alias spelling may appear anywhere in the report: {rendered}"
    );
    let _ = fs::remove_dir_all(&root);
}
