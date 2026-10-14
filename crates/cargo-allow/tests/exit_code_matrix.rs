//! Real-binary exit-code matrix for #2340.
//!
//! Proves process exit classes without calling the mapping helper:
//! Clap usage and structured `Usage` → 2; config/policy/runtime → 1; success → 0.
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
        "cargo-allow-exit-matrix-{label}-{}-{unique}",
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

fn assert_exit(label: &str, output: &Output, expected: i32, stderr_fragment: &str) {
    assert_eq!(
        output.status.code(),
        Some(expected),
        "{label}: expected exit {expected}, got {:?}; stdout=`{}` stderr=`{}`",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(stderr_fragment),
        "{label}: stderr should contain `{stderr_fragment}`; got `{stderr}`"
    );
}

fn assert_success(label: &str, output: &Output) {
    assert_eq!(
        output.status.code(),
        Some(0),
        "{label}: expected exit 0, got {:?}; stdout=`{}` stderr=`{}`",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn write_panic_source(root: &Path) {
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create src: {err}")));
    fs::write(
        root.join("src/lib.rs"),
        "pub fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write src/lib.rs: {err}")));
}

fn write_empty_policy(root: &Path) -> PathBuf {
    let policy_dir = root.join("policy");
    fs::create_dir_all(&policy_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    let path = policy_dir.join("allow.toml");
    fs::write(&path, "policy = \"cargo-allow\"\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write empty policy: {err}")));
    path
}

fn fixture_policy() -> &'static str {
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
"#
}

#[test]
fn exit_matrix_unknown_clap_flag_is_2() {
    let output = cargo_allow()
        .arg("doctor")
        .arg("--not-a-real-flag")
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run unknown flag: {err}")));
    assert_exit(
        "unknown clap flag",
        &output,
        2,
        "unexpected argument '--not-a-real-flag'",
    );
}

#[test]
fn exit_matrix_missing_required_clap_value_is_2() {
    let output = cargo_allow()
        .arg("check")
        .arg("--mode")
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run missing mode value: {err}")));
    assert_exit("missing required clap value", &output, 2, "--mode");
}

#[test]
fn exit_matrix_clap_conflicting_list_status_shortcuts_is_2() {
    let root = temp_root("exit-list-conflict");
    write_empty_policy(&root);
    let output = cargo_allow()
        .arg("list")
        .arg("--root")
        .arg(&root)
        .arg("--expired")
        .arg("--stale")
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run list conflict: {err}")));
    assert_exit("list --expired --stale", &output, 2, "--expired");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--stale"),
        "stderr should identify both shortcuts: {stderr}"
    );
    drop_root(root);
}

#[test]
fn exit_matrix_post_parse_structured_usage_is_2() {
    // This test exists to prove the *post-parse* route: a structured
    // `CargoAllowErrorKind::Usage` raised by our own code after clap has
    // accepted the argv still exits 2. It used to ride on `add --glob` with
    // `--path`, but #3203/#3210 moved that exclusion into clap's
    // `conflicts_with`, so that combination is now rejected at parse time and
    // no longer exercises this path. `--command-summary-output` on a command
    // that does not emit the summary is parse-valid and rejected afterwards,
    // so it exercises the same route this test was written for.
    let root = temp_root("exit-post-parse-usage");
    write_panic_source(&root);
    write_empty_policy(&root);
    let summary = root.join("summary.json");
    let output = cargo_allow()
        .arg("--command-summary-output")
        .arg(&summary)
        .arg("vocabulary")
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run post-parse usage: {err}")));
    assert_exit(
        "post-parse structured Usage (--command-summary-output on an unsupported command)",
        &output,
        2,
        "currently supports the source-exception",
    );
    assert!(
        !summary.exists(),
        "a rejected usage error must not leave a partial artifact behind"
    );
    drop_root(root);
}

#[test]
fn exit_matrix_command_summary_output_without_a_subcommand_is_2() {
    // `--command-summary-output` is a global flag, so it parses fine with no
    // subcommand at all. That reaches a second post-parse `Usage` branch,
    // distinct from the unsupported-subcommand one above.
    let root = temp_root("exit-summary-no-subcommand");
    let summary = root.join("summary.json");
    let output = cargo_allow()
        .arg("--command-summary-output")
        .arg(&summary)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run bare usage: {err}")));
    assert_exit(
        "post-parse structured Usage (--command-summary-output with no subcommand)",
        &output,
        2,
        // #4393: this message interpolates the one authoritative supported
        // set, `COMMAND_SUMMARY_SUPPORTED_COMMANDS` in `src/cli.rs`.
        "requires the adopt, doctor, audit, check, diff, init, propose, add, refresh, prune, migrate, explain, why, and worklist subcommand",
    );
    assert!(
        !summary.exists(),
        "a rejected usage error must not leave a partial artifact behind"
    );
    drop_root(root);
}

/// #4485: an unknown allow ID is a Usage failure whose fix is an existing
/// ID, not corrected syntax. The error must carry exactly one, specific
/// Next line and never the generic syntax advice.
#[test]
fn exit_matrix_explain_unknown_id_names_the_id_fix() {
    let root = temp_root("exit-explain-unknown-id");
    write_panic_source(&root);
    let policy = root.join("policy/allow.toml");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::write(&policy, fixture_policy())
        .unwrap_or_else(|err| std::panic::panic_any(format!("write fixture policy: {err}")));

    let output = cargo_allow()
        .arg("explain")
        .arg("--root")
        .arg(&root)
        .arg("--config")
        .arg(&policy)
        .arg("allow-9999")
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run explain: {err}")));
    assert_exit(
        "explain unknown id",
        &output,
        2,
        "no allow entry `allow-9999`",
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        stderr.matches("Next:").count(),
        1,
        "exactly one Next line: {stderr}"
    );
    assert!(
        stderr.contains("Next: run `cargo-allow list` to see valid IDs"),
        "specific Next names the ID fix: {stderr}"
    );
    assert!(
        !stderr.contains("review the command syntax"),
        "generic syntax advice must not appear: {stderr}"
    );
    drop_root(root);
}

/// #4497: numeric shorthand past `usize::MAX` cannot name a real ledger slot,
/// but the unknown-id error must echo the id the user typed instead of
/// silently rewriting the overflowed number to `allow-0000`.
#[test]
fn exit_matrix_explain_numeric_overflow_echoes_the_typed_id() {
    let root = temp_root("exit-explain-shorthand-overflow");
    write_panic_source(&root);
    let policy = root.join("policy/allow.toml");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::write(&policy, fixture_policy())
        .unwrap_or_else(|err| std::panic::panic_any(format!("write fixture policy: {err}")));

    let output = cargo_allow()
        .arg("explain")
        .arg("--root")
        .arg(&root)
        .arg("--config")
        .arg(&policy)
        .arg("18446744073709551616")
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run explain: {err}")));
    assert_exit(
        "explain numeric shorthand overflow",
        &output,
        2,
        "no allow entry `allow-18446744073709551616`",
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("allow-0000"),
        "the overflowed shorthand must not be rewritten to allow-0000: {stderr}"
    );
    assert_eq!(
        stderr.matches("Next:").count(),
        1,
        "exactly one Next line: {stderr}"
    );
    drop_root(root);
}

/// #4488: `diff` without a base and without an upstream fails with the
/// typed next step (pass --base / set upstream), never the generic syntax
/// advice, and renders exactly one Next line.
#[test]
fn exit_matrix_diff_without_base_names_the_base_fix() {
    let root = temp_root("exit-diff-no-base");
    write_panic_source(&root);
    // A real repository with a committed HEAD and no upstream: merge-base
    // auto-detection has nothing to find, so the missing-base error must
    // fire deterministically (an unresolved HEAD would be ambiguous).
    let git = |args: &[&str]| {
        let out = std::process::Command::new("git")
            .arg("-c")
            .arg("user.email=exit-matrix@example.invalid")
            .arg("-c")
            .arg("user.name=Exit Matrix")
            .args(args)
            .current_dir(&root)
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("run git {args:?}: {err}")));
        assert!(out.status.success(), "git {args:?} should succeed");
    };
    git(&["init", "-q"]);
    git(&["add", "src/lib.rs"]);
    git(&["commit", "-qm", "fixture"]);

    let output = cargo_allow()
        .arg("diff")
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run diff: {err}")));
    assert_exit("diff without base", &output, 2, "diff requires --base");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        stderr.matches("Next:").count(),
        1,
        "exactly one Next line: {stderr}"
    );
    assert!(
        stderr.contains("Next: pass --base <revision>"),
        "typed Next names the base fix: {stderr}"
    );
    assert!(
        stderr.contains("set an upstream"),
        "typed Next also names the upstream remedy: {stderr}"
    );
    assert!(
        !stderr.contains("review the command syntax"),
        "generic syntax advice must not appear: {stderr}"
    );
    drop_root(root);
}

#[test]
fn exit_matrix_missing_config_is_1() {
    let root = temp_root("exit-missing-config");
    write_panic_source(&root);
    let missing = root.join("policy/does-not-exist.toml");
    let output = cargo_allow()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--mode")
        .arg("audit")
        .arg("--config")
        .arg(&missing)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run missing config: {err}")));
    assert_exit("missing config", &output, 1, "error[");
    drop_root(root);
}

#[test]
fn exit_matrix_invalid_policy_content_is_1() {
    let root = temp_root("exit-invalid-policy");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    let invalid = format!(
        r#"{}

[[allow]]
id = "allow-invalid"
kind = "panic"
family = "unwrap"
path = "src/invalid.rs"
owner = "core"
classification = "fixture"
reason = "fixture invalid lifecycle"
created = "2026-08-01"
review_after = "2026-07-01"

[allow.selector]
ast_kind = "method_call"
callee = "unwrap"
"#,
        fixture_policy()
    );
    fs::write(root.join("policy/allow.toml"), invalid)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write invalid policy: {err}")));
    let output = cargo_allow()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--mode")
        .arg("audit")
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run invalid policy: {err}")));
    assert_exit("invalid policy content", &output, 1, "review_after");
    drop_root(root);
}

#[test]
fn exit_matrix_check_policy_violation_is_1() {
    let root = temp_root("exit-policy-violation");
    write_panic_source(&root);
    write_empty_policy(&root);
    let output = cargo_allow()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--mode")
        .arg("strict")
        .arg("--kind")
        .arg("panic")
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run strict check: {err}")));
    assert_eq!(
        output.status.code(),
        Some(1),
        "strict policy violation should exit 1; stdout=`{}` stderr=`{}`",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    drop_root(root);
}

#[test]
fn exit_matrix_successful_doctor_is_0() {
    let root = temp_root("exit-doctor-ok");
    write_empty_policy(&root);
    let output = cargo_allow()
        .arg("doctor")
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run doctor: {err}")));
    assert_success("doctor success", &output);
    drop_root(root);
}
