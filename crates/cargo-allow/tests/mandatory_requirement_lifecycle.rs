//! Mandatory requirement failures stay blocking across lifecycle dates in the
//! real CLI gates (#4644). A due review or a passed expiry must not turn a
//! missing SAFETY comment or a missing/mismatched `policy:<allow-id>`
//! reference into a passing `check --mode no-new`, while an otherwise-valid
//! entry keeps its lifecycle annotation.

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Keep disposable Git fixtures independent of a caller's repository
/// selection (same law as `tests/support/repository_environment.rs`).
fn isolate_repository(command: &mut Command) -> &mut Command {
    for name in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_COMMON_DIR",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    ] {
        command.env_remove(name);
    }
    command
}

/// One combined date × requirement-health case from the #4644 probe: the
/// finding, policy, selector, and evidence stay fixed while only the
/// lifecycle dates and the requirement health change.
struct RequirementCase {
    kind: &'static str,
    source: &'static str,
    family: &'static str,
    ast_kind: &'static str,
    review_after: &'static str,
    expires: &'static str,
    requirement_healthy: bool,
    expected_status: &'static str,
    expected_exit: i32,
}

fn requirement_cases() -> Vec<RequirementCase> {
    let mut cases = Vec::new();
    // (review_after, expires, healthy status, healthy exit) date postures:
    // both dates in the future, review due, and expiry passed.
    let date_postures = [
        ("2090-01-01", "2099-01-01", "matched", 0),
        ("2021-01-01", "2099-01-01", "review_due", 0),
        ("2021-01-01", "2022-01-01", "expired", 0),
    ];
    // Unsafe lane: required SAFETY comment present vs absent.
    for (source, healthy) in [
        ("// SAFETY: fixture control\nunsafe fn probe() {}\n", true),
        ("unsafe fn probe() {}\n", false),
    ] {
        for (review_after, expires, healthy_status, healthy_exit) in date_postures {
            let (expected_status, expected_exit) = if healthy {
                (healthy_status, healthy_exit)
            } else {
                ("evidence_missing", 1)
            };
            cases.push(RequirementCase {
                kind: "unsafe",
                source,
                family: "unsafe_fn",
                ast_kind: "unsafe_fn",
                review_after,
                expires,
                requirement_healthy: healthy,
                expected_status,
                expected_exit,
            });
        }
    }
    // Lint lane: required policy ID present vs absent.
    for (source, healthy) in [
        (
            "#[expect(dead_code, reason = \"policy:probe-entry\")]\nfn probe() {}\n",
            true,
        ),
        ("#[expect(dead_code)]\nfn probe() {}\n", false),
    ] {
        for (review_after, expires, healthy_status, healthy_exit) in date_postures {
            let (expected_status, expected_exit) = if healthy {
                (healthy_status, healthy_exit)
            } else {
                ("invalid_selector", 1)
            };
            cases.push(RequirementCase {
                kind: "lint_exception",
                source,
                family: "expect_attribute",
                ast_kind: "attribute",
                review_after,
                expires,
                requirement_healthy: healthy,
                expected_status,
                expected_exit,
            });
        }
    }
    cases
}

#[test]
fn mandatory_requirements_and_lifecycle_dates_agree_in_the_cli_gate() -> Result<(), String> {
    for case in requirement_cases() {
        let fixture = Fixture::new(case.kind, case.source)?;
        fs::write(
            fixture.root.join("policy/allow.toml"),
            policy(
                case.kind,
                case.family,
                case.ast_kind,
                case.review_after,
                case.expires,
            ),
        )
        .map_err(|error| format!("write policy: {error}"))?;
        let receipt_path = fixture.root.join("target/cargo-allow/receipt.json");
        let output = run_check_with_receipt(&fixture.root, case.kind, &receipt_path)?;
        let posture = format!(
            "{} {} requirement at review {}/expiry {}",
            case.kind,
            if case.requirement_healthy {
                "met"
            } else {
                "missing"
            },
            case.review_after,
            case.expires
        );
        require(
            output.status.code() == Some(case.expected_exit),
            format!("{posture} must exit {}: {output:?}", case.expected_exit),
        )?;
        let report = json(&output)?;
        require(
            report.get("failed") == Some(&Value::Bool(case.expected_exit == 1)),
            format!("{posture}: report failed flag must agree: {report}"),
        )?;
        let rows = report
            .get("outcomes")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("{posture}: report must expose outcomes: {report}"))?;
        require(
            rows.iter()
                .any(|row| row.get("status").and_then(Value::as_str) == Some(case.expected_status)),
            format!("{posture}: must project {}: {report}", case.expected_status),
        )?;
        if !case.requirement_healthy {
            let finding_row = rows
                .iter()
                .find(|row| row.get("finding_index").and_then(Value::as_u64) == Some(0))
                .ok_or_else(|| format!("{posture}: must keep a finding-level row: {report}"))?;
            require(
                finding_row.get("status").and_then(Value::as_str) == Some(case.expected_status),
                format!(
                    "{posture}: the finding-level row must stay {}: {report}",
                    case.expected_status
                ),
            )?;
            let receipt = fs::read(&receipt_path)
                .map_err(|error| format!("{posture}: read receipt: {error}"))?;
            let receipt: Value = serde_json::from_slice(&receipt)
                .map_err(|error| format!("{posture}: parse receipt: {error}"))?;
            require(
                receipt.get("status").and_then(Value::as_str) == Some("failed")
                    && receipt.get("failed") == Some(&Value::Bool(true)),
                format!("{posture}: receipt must record the blocking failure: {receipt}"),
            )?;
        }
    }
    Ok(())
}

fn policy(kind: &str, family: &str, ast_kind: &str, review_after: &str, expires: &str) -> String {
    let requirements = if kind == "unsafe" {
        "[requirements.unsafe]\nsafety_comment_required = true\n"
    } else {
        "lint_policy_id_required = true\n"
    };
    format!(
        r#"schema_version = "0.1"
policy = "cargo-allow"

[requirements]
{requirements}
[[allow]]
id = "probe-entry"
kind = "{kind}"
family = "{family}"
path = "src/lib.rs"
owner = "core"
classification = "reviewed_exception"
reason = "Retained probe exception"
evidence = ["test:probe"]
created = "2020-01-01"
review_after = "{review_after}"
expires = "{expires}"

[allow.selector]
ast_kind = "{ast_kind}"
container = "probe"
"#
    )
}

fn run_check_with_receipt(root: &Path, kind: &str, receipt: &Path) -> Result<Output, String> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-allow"));
    command
        .args(["check", "--root"])
        .arg(root)
        .args(["--kind", kind, "--mode", "no-new", "--format", "json"])
        .arg("--receipt")
        .arg(receipt);
    isolate_repository(&mut command)
        .output()
        .map_err(|error| format!("run check ({kind}): {error}"))
}

fn json(output: &Output) -> Result<Value, String> {
    serde_json::from_slice(&output.stdout).map_err(|error| format!("parse CLI JSON: {error}"))
}

fn require(condition: bool, message: impl Into<String>) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(kind: &'static str, source: &str) -> Result<Self, String> {
        let unique = Fixture::sequence().fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "cargo-allow-requirement-lifecycle-{}-{unique}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src")).map_err(|error| error.to_string())?;
        fs::create_dir_all(root.join("policy")).map_err(|error| error.to_string())?;
        fs::write(root.join("src/lib.rs"), source)
            .map_err(|error| format!("write {kind} source: {error}"))?;
        fs::write(
            root.join("policy/allow.toml"),
            policy(kind, "", "", "2090-01-01", "2099-01-01"),
        )
        .map_err(|error| format!("write policy: {error}"))?;
        for args in [
            vec!["init", "-q"],
            vec!["add", "."],
            vec![
                "-c",
                "user.name=fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "--no-gpg-sign",
                "-q",
                "-m",
                "fixture",
            ],
        ] {
            let mut command = Command::new("git");
            command.current_dir(&root).args(&args);
            let output = isolate_repository(&mut command)
                .output()
                .map_err(|error| format!("git {args:?}: {error}"))?;
            require(
                output.status.success(),
                format!("git {args:?} failed: {output:?}"),
            )?;
        }
        Ok(Fixture { root })
    }

    fn sequence() -> &'static AtomicUsize {
        static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
        &SEQUENCE
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
