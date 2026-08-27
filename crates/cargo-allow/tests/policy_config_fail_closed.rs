//! A policy or federation config that exists but cannot be loaded must fail
//! closed on every command, including the ones that tolerate a source tree with
//! no policy config at all (#1952).
//!
//! Before this was pinned, `audit` and `propose` treated *any* policy-resolution
//! failure as "this repository has no policy". A malformed `.allow/config.toml`
//! therefore discarded the entire exception ledger and still reported a
//! complete, clean, exit-0 result.
//!
//! These are black-box tests: they assert the process exit status and the
//! rendered diagnostic, not internal state.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const VALID_FEDERATION_CONFIG: &str = r#"schema_version = "1.0"

[[ledgers]]
id = "source-policy"
path = "policy/allow.toml"
dialect = "cargo-allow"
role = "canonical"
lanes = ["source-exception"]
mode = "blocking"
priority = 10
"#;

/// `[[ledgers]` is never closed, so the federation config cannot be parsed.
const MALFORMED_FEDERATION_CONFIG: &str = r#"schema_version = "1.0"

[[ledgers]
id = "source-policy"
"#;

const MINIMAL_POLICY: &str = r#"schema_version = "0.1"
policy = "cargo-allow"
owner = "core/policy"
status = "active"

[workspace]
root = "."
inventory = "git-tracked"
default_mode = "no-new"
ignored = [".git/**", "target/**"]
generated = []

[requirements]
owner_required = false
reason_required = false
classification_required = false
evidence_required = false
expires_or_review_after_required = false
allow_bare_allow_attributes = true
lint_policy_id_required = false
stale_entries_fail = false
"#;

fn write(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .unwrap_or_else(|err| std::panic::panic_any(format!("create dir: {err}")));
    }
    fs::write(path, contents)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write {}: {err}", path.display())));
}

fn temp_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-fail-closed-{label}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("create temp root: {err}")));
    root
}

fn run(command: &str, root: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
        .arg(command)
        .arg("--root")
        .arg(root)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow {command}: {err}")))
}

/// A source tree with a usable ledger and a federation config that cannot parse.
fn root_with_malformed_federation_config(label: &str) -> PathBuf {
    let root = temp_root(label);
    write(&root.join("policy/allow.toml"), MINIMAL_POLICY);
    write(
        &root.join(".allow/config.toml"),
        MALFORMED_FEDERATION_CONFIG,
    );
    write(&root.join("src/lib.rs"), "pub fn f() {}\n");
    root
}

fn assert_reports_invalid_config(command: &str, result: &Output) {
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        !result.status.success(),
        "{command} should fail closed on an unloadable federation config, got stderr=`{stderr}`"
    );
    assert!(
        stderr.contains("E0002_INVALID_CONFIG"),
        "{command} should report the invalid-config diagnostic, got stderr=`{stderr}`"
    );
    assert!(
        stderr.contains("federation config"),
        "{command} should name the unloadable federation config, got stderr=`{stderr}`"
    );
}

#[test]
fn audit_fails_closed_when_the_federation_config_cannot_be_parsed() {
    let root = root_with_malformed_federation_config("audit");
    assert_reports_invalid_config("audit", &run("audit", &root));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn propose_fails_closed_when_the_federation_config_cannot_be_parsed() {
    let root = root_with_malformed_federation_config("propose");
    assert_reports_invalid_config("propose", &run("propose", &root));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn check_fails_closed_when_the_federation_config_cannot_be_parsed() {
    let root = root_with_malformed_federation_config("check");
    assert_reports_invalid_config("check", &run("check", &root));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn audit_still_falls_back_when_no_policy_config_exists() {
    // The benign case the fallback exists for: there is nothing to enforce, so
    // `audit` scans without a ledger instead of demanding one.
    let root = temp_root("audit-no-policy");
    write(&root.join("src/lib.rs"), "pub fn f() {}\n");

    let result = run("audit", &root);

    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        result.status.success(),
        "audit without any policy config should still scan, got stderr=`{stderr}`"
    );
    assert!(
        !stderr.contains("E0002_INVALID_CONFIG"),
        "audit without any policy config should not report invalid config, got stderr=`{stderr}`"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn audit_still_reads_the_ledger_when_the_federation_config_is_valid() {
    // Guards the other direction: failing closed must not make a healthy
    // federated repository look broken.
    let root = temp_root("audit-valid-federation");
    write(&root.join("policy/allow.toml"), MINIMAL_POLICY);
    write(&root.join(".allow/config.toml"), VALID_FEDERATION_CONFIG);
    write(&root.join("src/lib.rs"), "pub fn f() {}\n");

    let result = run("audit", &root);

    assert!(
        result.status.success(),
        "audit should succeed with a valid federation config, got stderr=`{}`",
        String::from_utf8_lossy(&result.stderr)
    );

    let _ = fs::remove_dir_all(&root);
}
