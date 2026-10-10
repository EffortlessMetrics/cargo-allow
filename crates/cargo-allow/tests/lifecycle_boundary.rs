//! Policy-entry expiry boundaries across the evaluator, read projections,
//! and real supported commands (#4351). Federation deadlines stay separate.

use allow_core::{MatchStatus, SimpleDate};
use allow_match::{CheckMode, evaluate};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[path = "support/repository_environment.rs"]
mod repository_environment;

use repository_environment::isolate_repository;

const TARGET_ID: &str = "allow-boundary";
const SOURCE: &str = "pub fn value(v: Option<u8>) -> u8 { v.unwrap() }\n";

struct BoundaryCase {
    name: &'static str,
    expires: Option<String>,
    review_after: Option<String>,
    status: &'static str,
    cadence_class: &'static str,
    days_remaining: Option<i64>,
}

fn boundary_cases(today: SimpleDate) -> Vec<BoundaryCase> {
    let date = |offset| Some(today.add_days(offset).to_string());
    [
        (
            "expiry-yesterday",
            date(-1),
            None,
            "expired",
            "expired",
            Some(-1),
        ),
        (
            "expiry-today",
            date(0),
            None,
            "matched",
            "expiring",
            Some(0),
        ),
        (
            "expiry-tomorrow",
            date(1),
            None,
            "matched",
            "expiring",
            Some(1),
        ),
        (
            "never",
            Some("never".to_string()),
            date(30),
            "matched",
            "current",
            None,
        ),
        ("absent", None, date(30), "matched", "current", None),
        (
            "review-yesterday",
            None,
            date(-1),
            "review_due",
            "review_overdue",
            Some(-1),
        ),
        (
            "review-today",
            None,
            date(0),
            "review_due",
            "review_overdue",
            Some(0),
        ),
        (
            "review-tomorrow",
            None,
            date(1),
            "matched",
            "review_due_soon",
            Some(1),
        ),
    ]
    .into_iter()
    .map(
        |(name, expires, review_after, status, cadence_class, days_remaining)| BoundaryCase {
            name,
            expires,
            review_after,
            status,
            cadence_class,
            days_remaining,
        },
    )
    .collect()
}

#[test]
fn lifecycle_boundary_evaluator_and_read_projections_share_one_fixed_day() -> Result<(), String> {
    let today = SimpleDate::parse("2026-10-02").ok_or("fixed day must parse")?;
    let findings = allow_rust::scan_rust_source("src/lib.rs", SOURCE);
    require(findings.len() == 1, "fixture must scan exactly one finding")?;
    for case in boundary_cases(today) {
        let cfg = allow_policy::parse_policy(&policy(&case, false, today))
            .map_err(|error| format!("parse {} fixture: {error}", case.name))?;
        let entry = cfg
            .allow
            .iter()
            .find(|entry| entry.id == TARGET_ID)
            .ok_or("fixture must retain its target entry")?;
        let outcomes = evaluate(&cfg, &findings, CheckMode::NoNew, today);
        let outcome = outcomes
            .iter()
            .find(|outcome| outcome.allow_id.as_deref() == Some(TARGET_ID))
            .ok_or("evaluator must retain the target outcome")?;
        let statuses = allow_report::ledger_read_statuses(&cfg, &outcomes, today);
        let projected = allow_report::ledger_project_outcomes(&cfg, &outcomes, today);
        let projected_outcome = projected
            .iter()
            .find(|outcome| outcome.allow_id.as_deref() == Some(TARGET_ID));
        require(
            outcome.status.as_str() == case.status
                && allow_report::ledger_read_state(entry, &[outcome], today).status
                    == outcome.status
                && allow_report::ledger_outcome_status(&statuses, outcome) == outcome.status
                && projected_outcome == Some(outcome),
            format!(
                "{} must retain evaluator status and details across projections",
                case.name
            ),
        )?;
    }

    // A programmatically constructed invalid entry still fails closed. The
    // parsed-date helper must not replace that defect with advisory Expired.
    let mut cfg = allow_policy::parse_policy(&policy(
        &BoundaryCase {
            name: "invalid",
            expires: Some("never".to_string()),
            review_after: Some("2026-11-01".to_string()),
            status: "missing_required_field",
            cadence_class: "invalid",
            days_remaining: None,
        },
        false,
        today,
    ))
    .map_err(|error| format!("parse valid control: {error}"))?;
    let entry = cfg
        .allow
        .iter_mut()
        .find(|entry| entry.id == TARGET_ID)
        .ok_or("invalid control must have an entry")?;
    entry.lifecycle.expires = Some("not-a-date".to_string());
    let outcomes = evaluate(&cfg, &findings, CheckMode::NoNew, today);
    let statuses = allow_report::ledger_read_statuses(&cfg, &outcomes, today);
    require(
        statuses.get(TARGET_ID) == Some(&MatchStatus::MissingRequiredField),
        format!("malformed expiry must remain a blocking policy defect: {statuses:?}"),
    )
}

#[test]
fn lifecycle_boundary_cli_surfaces_and_calendar_postures_agree() -> Result<(), String> {
    // Only cadence exposes --as-of. Pin the fixed-day projection above, then
    // exercise the actual ambient-day commands. If midnight intervenes,
    // rebuild the fixture once for the new day rather than silently skipping.
    for _ in 0..2 {
        let today = SimpleDate::today_utc_approx();
        let cases = boundary_cases(today);
        let first = cases.first().ok_or("boundary cases must not be empty")?;
        let fixture = Fixture::new(&policy(first, false, today))?;
        let result = check_cli_cases(&fixture.root, &cases, today);
        if today == SimpleDate::today_utc_approx() {
            return result;
        }
    }
    Err("UTC day changed during both attempts to inspect lifecycle boundaries".to_string())
}

#[test]
fn lifecycle_fixture_children_ignore_repository_environment() -> Result<(), String> {
    repository_environment::require_isolated_fixture_test(
        "lifecycle_boundary_cli_surfaces_and_calendar_postures_agree",
    )
}

fn check_cli_cases(root: &Path, cases: &[BoundaryCase], today: SimpleDate) -> Result<(), String> {
    let as_of = today.to_string();
    for case in cases {
        for calendar_blocks in [false, true] {
            fs::write(
                root.join("policy/allow.toml"),
                policy(case, calendar_blocks, today),
            )
            .map_err(|error| format!("write {} policy: {error}", case.name))?;
            let check_output = run(root, &["check", "--mode", "no-new"])?;
            let fails = calendar_blocks && case.status == "expired";
            require(
                check_output.status.code() == Some(if fails { 1 } else { 0 }),
                format!(
                    "{} check gate disagrees (calendar_blocks={calendar_blocks}): {check_output:?}",
                    case.name
                ),
            )?;
            let check = json(&check_output)?;
            require(
                check.get("failed") == Some(&Value::Bool(fails))
                    && row(&check, "outcomes", "allow_id")?.get("status")
                        == Some(&Value::from(case.status)),
                format!(
                    "{} check artifact must preserve its evaluated state: {check}",
                    case.name
                ),
            )?;
        }
        let list = successful_json(root, &["list", "--allow-id", TARGET_ID])?;
        require(
            row(&list, "allow_entries", "id")?.get("status") == Some(&Value::from(case.status)),
            format!("{} list must agree with check: {list}", case.name),
        )?;
        let explain = successful_json(root, &["explain", TARGET_ID])?;
        require(
            explain.pointer("/summary/current_status") == Some(&Value::from(case.status))
                && explain.pointer("/current_findings/0/status") == Some(&Value::from(case.status))
                && row(&explain, "match_outcomes", "allow_id")?.get("status")
                    == Some(&Value::from(case.status)),
            format!("{} explain must agree with check: {explain}", case.name),
        )?;
        let worklist = successful_json(root, &["worklist", "--allow-id", TARGET_ID])?;
        let items = worklist
            .get("work_items")
            .and_then(Value::as_array)
            .ok_or("worklist must expose work_items")?;
        if case.status == "matched" {
            require(
                items.is_empty(),
                format!(
                    "{} must not manufacture expired/review work: {worklist}",
                    case.name
                ),
            )?;
        } else {
            require(
                !items.is_empty()
                    && items
                        .iter()
                        .all(|item| item.get("status") == Some(&Value::from(case.status))),
                format!(
                    "{} worklist must retain the due state: {worklist}",
                    case.name
                ),
            )?;
        }
        let cadence = successful_json(root, &["cadence", "--as-of", &as_of])?;
        let cadence_row = row(&cadence, "rows", "allow_id")?;
        require(
            cadence_row.get("class") == Some(&Value::from(case.cadence_class))
                && cadence_row.get("days_remaining").and_then(Value::as_i64) == case.days_remaining,
            format!(
                "{} cadence must use the same expiry day: {cadence_row}",
                case.name
            ),
        )?;
    }
    Ok(())
}

#[test]
fn lifecycle_boundary_malformed_dates_fail_at_every_cli_loader() -> Result<(), String> {
    let today = SimpleDate::today_utc_approx();
    let cases = boundary_cases(today);
    let first = cases.first().ok_or("boundary cases must not be empty")?;
    let fixture = Fixture::new(&policy(first, false, today))?;
    for field in ["expires", "review_after"] {
        let case = BoundaryCase {
            name: "malformed",
            expires: (field == "expires").then(|| "not-a-date".to_string()),
            review_after: (field == "review_after").then(|| "not-a-date".to_string()),
            status: "missing_required_field",
            cadence_class: "invalid",
            days_remaining: None,
        };
        fs::write(
            fixture.root.join("policy/allow.toml"),
            policy(&case, true, today),
        )
        .map_err(|error| format!("write malformed {field}: {error}"))?;
        for args in [
            vec!["check", "--mode", "no-new"],
            vec!["list"],
            vec!["worklist"],
            vec!["explain", TARGET_ID],
            vec!["cadence"],
        ] {
            let output = run(&fixture.root, &args)?;
            let stderr = String::from_utf8_lossy(&output.stderr);
            require(
                !output.status.success() && stderr.contains(&format!("invalid {field} date")),
                format!("{args:?} must reject malformed {field} before rendering: {output:?}"),
            )?;
        }
    }
    Ok(())
}

fn policy(case: &BoundaryCase, calendar_blocks: bool, today: SimpleDate) -> String {
    let created = today.add_days(-30);
    let future = today.add_days(30);
    let mut text = format!(
        r#"schema_version = "0.1"
policy = "cargo-allow"

[requirements]
calendar_expiry_blocks_no_new = {calendar_blocks}

[[allow]]
id = "allow-policy-file"
kind = "non_rust_file"
path = "policy/allow.toml"
owner = "core"
classification = "reviewed_exception"
reason = "The fixture's policy file is intentional."
evidence = ["test:lifecycle_boundary"]
created = "{created}"
review_after = "{future}"

[allow.selector]
ast_kind = "tracked_file"

[[allow]]
id = "{TARGET_ID}"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core"
classification = "reviewed_exception"
reason = "The fixture compares lifecycle boundaries for one accepted identity."
evidence = ["test:lifecycle_boundary"]
created = "{created}"
"#
    );
    if let Some(expires) = &case.expires {
        text.push_str(&format!("expires = \"{expires}\"\n"));
    }
    if let Some(review_after) = &case.review_after {
        text.push_str(&format!("review_after = \"{review_after}\"\n"));
    }
    text.push_str("\n[allow.selector]\nast_kind = \"method_call\"\ncallee = \"unwrap\"\n");
    text
}

fn row<'a>(document: &'a Value, array: &str, id_field: &str) -> Result<&'a Value, String> {
    document
        .get(array)
        .and_then(Value::as_array)
        .and_then(|rows| {
            rows.iter()
                .find(|row| row.get(id_field).and_then(Value::as_str) == Some(TARGET_ID))
        })
        .ok_or_else(|| format!("{array} must contain {TARGET_ID}: {document}"))
}

fn run(root: &Path, args: &[&str]) -> Result<Output, String> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-allow"));
    command.args(args).arg("--root").arg(root).args([
        "--config",
        "policy/allow.toml",
        "--format",
        "json",
    ]);
    isolate_repository(&mut command)
        .output()
        .map_err(|error| format!("run {args:?}: {error}"))
}

fn json(output: &Output) -> Result<Value, String> {
    serde_json::from_slice(&output.stdout).map_err(|error| format!("parse CLI JSON: {error}"))
}

fn successful_json(root: &Path, args: &[&str]) -> Result<Value, String> {
    let output = run(root, args)?;
    require(
        output.status.success(),
        format!("{args:?} must succeed: {output:?}"),
    )?;
    json(&output)
}

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(policy: &str) -> Result<Self, String> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| format!("fixture clock: {error}"))?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "cargo-allow-lifecycle-boundary-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("src")).map_err(|error| error.to_string())?;
        fs::create_dir_all(root.join("policy")).map_err(|error| error.to_string())?;
        fs::write(root.join("src/lib.rs"), SOURCE).map_err(|error| error.to_string())?;
        fs::write(root.join("policy/allow.toml"), policy).map_err(|error| error.to_string())?;
        for args in [
            vec!["init", "-q"],
            vec!["add", "-A"],
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
        Ok(Self { root })
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn require(condition: bool, message: impl Into<String>) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}
