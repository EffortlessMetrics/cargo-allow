//! #4655: a parseable-but-invalid `.allow/config.toml` registry must not
//! silently revert `check` to the discovered ledger. The gate has to fail and
//! name the invalid registry instead of degrading to a green check that no
//! longer enforces the registered canonical ledger.

// Each test binary compiles `support` and `json_assertions` separately, so the
// helpers this one does not call would otherwise warn as dead code under
// `-D warnings`. Each attribute names its own ledger entry so the two
// identical-shaped attributes stay uniquely receipted (policy:allow-1173 /
// policy:allow-1174).
#[expect(
    dead_code,
    reason = "policy:allow-1173: shared json_assertions module; this binary uses a subset"
)]
mod json_assertions;
#[expect(
    dead_code,
    reason = "policy:allow-1174: shared support module; this binary uses a subset"
)]
mod support;

use std::fs;
use std::process::Command;

use json_assertions::assert_json_str;
use support::{
    assert_saved_json_artifact, assert_status, cargo_allow_command, remove_temp_root, temp_root,
};

fn policy() -> &'static str {
    r#"policy = "cargo-allow"

[[allow]]
id = "allow-policy"
kind = "non_rust_file"
family = "configuration"
path = "policy/allow.toml"
owner = "core"
classification = "fixture"
reason = "fixture policy file"
review_after = "2026-08-01"

[allow.selector]
ast_kind = "tracked_file"
symbol = "policy/allow.toml"
target_fingerprint = "toml"
glob = "policy/allow.toml"

[[allow]]
id = "allow-federation-registry"
kind = "non_rust_file"
family = "configuration"
path = ".allow/config.toml"
owner = "core"
classification = "fixture"
reason = "fixture federation registry file"
review_after = "2026-08-01"

[allow.selector]
ast_kind = "tracked_file"
symbol = ".allow/config.toml"
target_fingerprint = "toml"
glob = ".allow/config.toml"
"#
}

fn registry(id: &str) -> String {
    format!(
        r#"schema_version = "1.0"

[[ledgers]]
id = "{id}"
path = "policy/allow.toml"
dialect = "cargo-allow"
role = "canonical"
lanes = ["source-exception"]
mode = "blocking"
priority = 10

[[ledgers]]
id = "{id}"
path = "policy/allow.toml"
dialect = "cargo-allow"
role = "canonical"
lanes = ["source-exception"]
mode = "blocking"
priority = 10
"#
    )
}

fn valid_registry() -> &'static str {
    r#"schema_version = "1.0"

[[ledgers]]
id = "source-policy"
path = "policy/allow.toml"
dialect = "cargo-allow"
role = "canonical"
lanes = ["source-exception"]
mode = "blocking"
priority = 10
"#
}

fn git(root: &std::path::Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("git {args:?}: {err}")));
    if !output.status.success() {
        std::panic::panic_any(format!(
            "git {args:?} failed: stdout=`{}` stderr=`{}`",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
}

/// `registry_text` of `None` commits no `.allow/config.toml`, so the staged
/// federation-input guard (which rejects any indexed `.allow` entry) cannot
/// pre-empt the staged gate; the caller then writes the registry untracked.
fn tracked_fixture(label: &str, registry_text: Option<&str>) -> std::path::PathBuf {
    let root = temp_root(label);
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::create_dir_all(root.join(".allow"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create allow dir: {err}")));
    fs::write(root.join("policy/allow.toml"), policy())
        .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
    if let Some(registry_text) = registry_text {
        fs::write(root.join(".allow/config.toml"), registry_text)
            .unwrap_or_else(|err| std::panic::panic_any(format!("write registry: {err}")));
    }
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-m", "fixture"]);
    root
}

#[test]
fn invalid_registry_fails_check_and_names_the_condition() {
    let root = tracked_fixture("federation-invalid-gate", Some(&registry("source-policy")));
    let receipt = root.join("target/cargo-allow/check.receipt.json");

    let result = cargo_allow_command()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--mode")
        .arg("no-new")
        .arg("--format")
        .arg("json")
        .arg("--receipt")
        .arg(&receipt)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow check: {err}")));

    assert_status("check", &result, false);
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("federation configuration is invalid"),
        "stderr must name the invalid federation configuration: `{stderr}`"
    );

    let saved =
        assert_saved_json_artifact(&receipt, "check receipt", "cargo-allow.receipt.v1", "check");
    assert_json_str(
        &saved,
        "/federation/registry_validity",
        "invalid",
        "receipt registry validity",
    );
    assert_json_str(
        &saved,
        "/federation/precedence_applied",
        "discovery_fallback",
        "receipt precedence",
    );

    remove_temp_root(root);
}

#[test]
fn repaired_registry_enforces_from_the_registered_ledger() {
    let root = tracked_fixture("federation-repaired-gate", Some(valid_registry()));
    let receipt = root.join("target/cargo-allow/check.receipt.json");

    let result = cargo_allow_command()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--mode")
        .arg("no-new")
        .arg("--format")
        .arg("json")
        .arg("--receipt")
        .arg(&receipt)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow check: {err}")));

    assert_status("check", &result, true);
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        !stderr.contains("federation configuration is invalid"),
        "a repaired registry must not raise the invalid-registry signal: `{stderr}`"
    );

    let saved =
        assert_saved_json_artifact(&receipt, "check receipt", "cargo-allow.receipt.v1", "check");
    assert_json_str(
        &saved,
        "/federation/registry_validity",
        "valid",
        "receipt registry validity",
    );
    assert_json_str(
        &saved,
        "/federation/precedence_applied",
        "federation_registry",
        "receipt precedence",
    );

    remove_temp_root(root);
}

#[test]
fn staged_check_fails_on_invalid_registry() {
    // The staged lane shares the same evaluator. A tracked or valid registry
    // is rejected earlier by the staged federation-input guard, but an invalid
    // registry parses with empty contributors, slipped past that guard, and
    // used to run the staged gate on the discovered ledger silently (#4655).
    // The registry is deliberately untracked: staged evaluation reads the
    // index, and the federation evaluator reads the working tree.
    let root = tracked_fixture("federation-invalid-staged-gate", None);
    fs::write(root.join(".allow/config.toml"), registry("source-policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("write registry: {err}")));

    let result = cargo_allow_command()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--staged")
        .arg("--phase")
        .arg("precommit")
        .arg("--mode")
        .arg("audit")
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run staged check: {err}")));

    assert_status("check", &result, false);
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("federation configuration is invalid"),
        "staged stderr must name the invalid federation configuration: `{stderr}`"
    );

    remove_temp_root(root);
}
