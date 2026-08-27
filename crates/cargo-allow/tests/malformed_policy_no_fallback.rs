//! A present-but-unusable ledger must never be treated as an absent one (#1952).
//!
//! Commands that tolerate a missing policy (`audit`, `why`, `explain`,
//! `worklist`, ...) resolve policy with `require_config = false`. That fallback
//! exists for repositories which genuinely have no ledger yet — the adoption
//! path. Before this contract existed, the fallback also absorbed a ledger that
//! was present but unreadable: a single typo in `policy/allow.toml` made the
//! whole exception ledger disappear, the scan ran against an empty policy, and
//! the run still reported success with every evidence counter at zero.
//!
//! These tests pin both halves of the boundary: a broken ledger fails, and the
//! two legitimate reasons to fall back still fall back.
//!
//! This is a focused test: per the repo convention for focused tests
//! (version_output, policy_discovery, check_output_path_containment) it inlines
//! its own subprocess helpers rather than pulling in the shared `tests/support`
//! module.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn cargo_allow_command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
}

fn temp_root(label: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
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

fn assert_status(command: &str, result: &Output, should_succeed: bool) {
    assert_eq!(
        result.status.success(),
        should_succeed,
        "{command} status mismatch: stdout=`{}` stderr=`{}`",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

/// Source with one finding, so a run that reaches the scanner has something to
/// report and a silently-empty policy would still look like a healthy result.
const SOURCE_WITH_FINDING: &str =
    "pub fn risky(values: &[u32]) -> u32 {\n    unsafe { *values.get_unchecked(0) }\n}\n";

fn seed_repository(label: &str) -> PathBuf {
    let root = temp_root(label);
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create src dir: {err}")));
    fs::write(root.join("src/lib.rs"), SOURCE_WITH_FINDING)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write source: {err}")));
    root
}

fn write_policy(root: &Path, contents: &str) {
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::write(root.join("policy/allow.toml"), contents)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
}

fn run_audit(root: &Path) -> Output {
    cargo_allow_command()
        .arg("audit")
        .arg("--root")
        .arg(root)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow audit: {err}")))
}

fn combined(result: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    )
}

#[test]
fn audit_rejects_a_ledger_that_exists_but_cannot_be_parsed() {
    let root = seed_repository("malformed-policy-audit");
    write_policy(&root, "this is not valid toml [[[\n");

    let result = run_audit(&root);

    assert_status("audit", &result, false);
    let text = combined(&result);
    assert!(
        text.contains("policy/allow.toml"),
        "operator must be told which ledger was rejected: {text}"
    );
    assert!(
        text.contains("present but unusable"),
        "a broken ledger must not be reported as a missing one: {text}"
    );
    assert!(
        !text.contains("run `cargo-allow init`"),
        "`init` is the wrong remedy for a ledger that already exists: {text}"
    );

    remove_temp_root(root);
}

#[test]
fn why_rejects_a_ledger_that_exists_but_cannot_be_parsed() {
    let root = seed_repository("malformed-policy-why");
    write_policy(&root, "this is not valid toml [[[\n");

    // `why` resolves policy through a separate scoped loader, which carries its
    // own copy of the fallback rule.
    let result = cargo_allow_command()
        .arg("why")
        .arg("--root")
        .arg(&root)
        .arg("--kind")
        .arg("unsafe")
        .arg("--path")
        .arg("src/lib.rs")
        .arg("--line")
        .arg("2")
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow why: {err}")));

    assert_status("why", &result, false);
    assert!(
        combined(&result).contains("present but unusable"),
        "scoped policy loading must apply the same fallback law: {}",
        combined(&result)
    );

    remove_temp_root(root);
}

#[test]
fn audit_still_falls_back_when_no_ledger_exists_at_all() {
    // The adoption path: a repository that has not run `init` yet must still
    // get an advisory scan rather than a hard failure.
    let root = seed_repository("malformed-policy-absent");

    let result = run_audit(&root);

    assert_status("audit", &result, true);

    remove_temp_root(root);
}

#[test]
fn audit_still_falls_back_past_a_foreign_dialect_ledger() {
    // A candidate that parses and declares itself as another tool's is
    // genuinely not ours; skipping it and scanning without a policy stays
    // correct. Only undetermined ownership is fatal.
    let root = seed_repository("malformed-policy-foreign");
    write_policy(
        &root,
        "schema_version = \"1\"\npolicy = \"some-other-tool\"\n",
    );

    let result = run_audit(&root);

    assert_status("audit", &result, true);

    remove_temp_root(root);
}

#[test]
fn audit_accepts_a_well_formed_ledger() {
    let root = seed_repository("malformed-policy-valid");
    write_policy(&root, "schema_version = \"1\"\npolicy = \"cargo-allow\"\n");

    let result = run_audit(&root);

    assert_status("audit", &result, true);

    remove_temp_root(root);
}

#[test]
fn doctor_diagnoses_a_broken_ledger_instead_of_advertising_init() {
    // `doctor` deliberately still runs against a broken ledger — a diagnostic
    // command that refuses to start is useless. What it must not do is report
    // the file as absent and tell the operator to create it.
    let root = seed_repository("malformed-policy-doctor");
    write_policy(&root, "this is not valid toml [[[\n");

    let result = cargo_allow_command()
        .arg("doctor")
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow doctor: {err}")));

    let text = combined(&result);
    // Matched against the init advertisement specifically: the unrelated
    // `federation config: not found` line is correct and must not trip this.
    assert!(
        !text.contains("config: not found; run"),
        "doctor must not report an existing ledger as absent: {text}"
    );
    assert!(
        text.contains("config status: invalid"),
        "doctor must report the ledger as invalid: {text}"
    );
    assert!(
        text.contains("TOML parse error"),
        "doctor must surface the underlying parse failure: {text}"
    );

    remove_temp_root(root);
}

#[test]
fn doctor_still_reports_a_genuinely_absent_ledger_as_not_found() {
    let root = seed_repository("malformed-policy-doctor-absent");

    let result = cargo_allow_command()
        .arg("doctor")
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow doctor: {err}")));

    assert!(
        combined(&result).contains("config: not found; run"),
        "an absent ledger is still absent: {}",
        combined(&result)
    );

    remove_temp_root(root);
}

#[test]
fn an_unreadable_ledger_is_rejected_rather_than_skipped() {
    // Distinct from a parse failure: the bytes never arrive, so the dialect
    // cannot be determined at all. This is the case most likely to be mistaken
    // for "no policy here" by a permissions accident in CI.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let root = seed_repository("malformed-policy-unreadable");
        write_policy(&root, "schema_version = \"1\"\npolicy = \"cargo-allow\"\n");
        let policy = root.join("policy/allow.toml");
        fs::set_permissions(&policy, fs::Permissions::from_mode(0o000))
            .unwrap_or_else(|err| std::panic::panic_any(format!("chmod policy: {err}")));

        // A process that ignores mode bits (running as root, or a filesystem
        // mounted without permission support) would make this vacuous.
        if fs::read(&policy).is_ok() {
            let _ = fs::set_permissions(&policy, fs::Permissions::from_mode(0o644));
            remove_temp_root(root);
            return;
        }

        let result = run_audit(&root);

        assert_status("audit", &result, false);
        assert!(
            combined(&result).contains("present but unusable"),
            "an unreadable ledger must not be reported as absent: {}",
            combined(&result)
        );

        fs::set_permissions(&policy, fs::Permissions::from_mode(0o644))
            .unwrap_or_else(|err| std::panic::panic_any(format!("restore policy mode: {err}")));
        remove_temp_root(root);
    }
}
