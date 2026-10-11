//! Binary-level lifecycle cadence surface tests (#4239).
//!
//! Fixtures:
//! a. six-class totality + byte-stability over a fixed as-of,
//! b. boundary determinism around review_after and expires dates,
//! c. fail-closed handling of malformed/descending lifecycle dates,
//! d. identity retention with exact day deltas,
//! e. never-a-gate: policy bytes and the check verdict are untouched,
//! f. human/JSON/markdown parity from one semantic result,
//! g. the ambient-day default as-of,
//! h. malformed `--as-of` input fails loudly with the usage exit code.
//!
//! Loader law note (fixture c): the policy loader rejects malformed and
//! descending lifecycle dates at parse, so a cadence invocation over such a
//! policy fails loudly instead of rendering `invalid` rows. The typed
//! `invalid` class itself is pinned by the cadence contract sample and the
//! allow-report classifier tests; this file pins the loader boundary.

use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

fn cargo_allow_command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
}

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(name: &str, entries: &[String]) -> Self {
        let root = std::env::temp_dir().join(format!(
            "cargo-allow-cadence-{}-{}-{name}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0),
        ));
        fs::create_dir_all(root.join("policy"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
        fs::create_dir_all(root.join("src"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("src dir: {err}")));
        // Run outputs land in a directory the fixture policy ignores, so a
        // rerun sees the same inventoried file set and byte-stability
        // fixtures compare classification, not their own output files.
        fs::create_dir_all(root.join("out"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("out dir: {err}")));
        fs::write(root.join("src/lib.rs"), "pub fn cadence_fixture() {}\n")
            .unwrap_or_else(|err| std::panic::panic_any(format!("source: {err}")));
        let mut policy = String::from(
            "schema_version = 1\n\n[workspace]\nignored = [\"out/**\"]\ngenerated = []\n\n",
        );
        for entry in entries {
            policy.push_str(entry);
            policy.push('\n');
        }
        fs::write(root.join("policy/allow.toml"), policy)
            .unwrap_or_else(|err| std::panic::panic_any(format!("policy: {err}")));
        Self { root }
    }

    fn output_path(&self, name: &str) -> PathBuf {
        self.root.join("out").join(name)
    }

    fn run(&self, args: &[String]) -> Output {
        cargo_allow_command()
            .arg("cadence")
            .arg("--root")
            .arg(&self.root)
            .arg("--config")
            .arg("policy/allow.toml")
            .args(args)
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("cadence run: {err}")))
    }

    /// Run cadence in JSON mode writing the artifact into the fixture, and
    /// return the parsed document.
    fn cadence_json_run(&self, as_of: Option<&str>, name: &str) -> Value {
        let output_path = self.output_path(&format!("cadence-{name}.json"));
        let output_arg = output_path
            .to_str()
            .unwrap_or_else(|| std::panic::panic_any("fixture output path must be unicode"))
            .to_string();
        let mut args: Vec<String> = Vec::new();
        if let Some(as_of) = as_of {
            args.push("--as-of".to_string());
            args.push(as_of.to_string());
        }
        args.push("--format".to_string());
        args.push("json".to_string());
        args.push("--output".to_string());
        args.push(output_arg);
        let output = self.run(&args);
        assert!(
            output.status.success(),
            "cadence {name} should succeed: stdout=`{}` stderr=`{}`",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let contents = fs::read_to_string(&output_path).unwrap_or_else(|err| {
            std::panic::panic_any(format!("read {}: {err}", output_path.display()))
        });
        serde_json::from_str(&contents).unwrap_or_else(|err| {
            std::panic::panic_any(format!("cadence artifact parses: {err}\n{contents}"))
        })
    }

    fn cadence_json(&self, as_of: &str, name: &str) -> Value {
        self.cadence_json_run(Some(as_of), name)
    }

    fn cadence_json_ambient(&self, name: &str) -> Value {
        self.cadence_json_run(None, name)
    }

    fn run_expect_success(&self, args: &[&str]) -> String {
        let output_path = self.output_path("cadence-run-output.txt");
        let output_arg = output_path
            .to_str()
            .unwrap_or_else(|| std::panic::panic_any("fixture output path must be unicode"))
            .to_string();
        let mut all_args: Vec<String> = args.iter().map(|arg| (*arg).to_string()).collect();
        all_args.push("--output".to_string());
        all_args.push(output_arg);
        let output = self.run(&all_args);
        assert!(
            output.status.success(),
            "cadence run should succeed: stdout=`{}` stderr=`{}`",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        fs::read_to_string(&output_path)
            .unwrap_or_else(|err| std::panic::panic_any(format!("read cadence output: {err}")))
    }

    fn policy_bytes(&self) -> Vec<u8> {
        fs::read(self.root.join("policy/allow.toml"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("read policy: {err}")))
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// One reviewed panic-family entry selecting a finding-free path, so the
/// fixture stays fully policy-driven and dates own every class. The panic
/// kind requires structural selector identity at load, so every entry
/// carries the same call/unwrap selector; it is load-bearing for the
/// loader only, and the lifecycle dates own the classification.
fn reviewed_entry(
    id: &str,
    path: &str,
    created: &str,
    review_after: Option<&str>,
    expires: Option<&str>,
) -> String {
    let mut entry = format!(
        "[[allow]]\nid = \"{id}\"\nkind = \"panic\"\nfamily = \"unwrap\"\npath = \"{path}\"\nowner = \"core/parser\"\nclassification = \"reviewed\"\nreason = \"cadence surface fixture\"\nevidence = [\"test:cadence_{id}\"]\ncreated = \"{created}\"\n"
    );
    if let Some(review_after) = review_after {
        entry.push_str(&format!("review_after = \"{review_after}\"\n"));
    }
    if let Some(expires) = expires {
        entry.push_str(&format!("expires = \"{expires}\"\n"));
    }
    entry.push_str("\n[allow.selector]\nast_kind = \"call\"\ncallee = \"unwrap\"\n");
    entry
}

fn rows(document: &Value) -> Vec<(String, String, Option<i64>)> {
    document
        .pointer("/rows")
        .and_then(Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("cadence artifact carries rows"))
        .iter()
        .map(|row| {
            (
                row.pointer("/allow_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                row.pointer("/class")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                row.pointer("/days_remaining").and_then(Value::as_i64),
            )
        })
        .collect()
}

fn row_for<'a>(document: &'a Value, allow_id: &str) -> &'a Value {
    document
        .pointer("/rows")
        .and_then(Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("cadence artifact carries rows"))
        .iter()
        .find(|row| row.pointer("/allow_id").and_then(Value::as_str) == Some(allow_id))
        .unwrap_or_else(|| std::panic::panic_any(format!("cadence rows should contain {allow_id}")))
}

/// Fixture a (+d): a policy whose loadable entries land one per class at the
/// fixed as-of 2026-10-01; running twice is byte-identical, and every row
/// retains its owner, path, evidence, and exact signed day delta.
#[test]
fn cadence_classifies_loadable_classes_and_is_byte_stable() {
    let entries = vec![
        reviewed_entry(
            "allow-current",
            "src/current.rs",
            "2026-06-01",
            Some("2027-06-01"),
            None,
        ),
        reviewed_entry(
            "allow-due-soon",
            "src/due-soon.rs",
            "2026-06-01",
            Some("2026-10-08"),
            None,
        ),
        reviewed_entry(
            "allow-overdue",
            "src/overdue.rs",
            "2026-06-01",
            Some("2026-09-15"),
            None,
        ),
        reviewed_entry(
            "allow-expiring",
            "src/expiring.rs",
            "2026-06-01",
            None,
            Some("2026-10-08"),
        ),
        reviewed_entry(
            "allow-expired",
            "src/expired.rs",
            "2026-06-01",
            None,
            Some("2026-09-15"),
        ),
    ];
    let fixture = Fixture::new("totality", &entries);
    let run = |name: &str| {
        let output_path = fixture.output_path(&format!("run-{name}.json"));
        let output_arg = output_path
            .to_str()
            .unwrap_or_else(|| std::panic::panic_any("fixture output path must be unicode"))
            .to_string();
        let output = fixture.run(&[
            "--as-of".to_string(),
            "2026-10-01".to_string(),
            "--format".to_string(),
            "json".to_string(),
            "--output".to_string(),
            output_arg,
        ]);
        assert!(
            output.status.success(),
            "cadence totality run should succeed: stderr=`{}`",
            String::from_utf8_lossy(&output.stderr)
        );
        fs::read_to_string(&output_path)
            .unwrap_or_else(|err| std::panic::panic_any(format!("read run output: {err}")))
    };
    let first = run("first");
    let second = run("second");
    assert_eq!(
        first, second,
        "same (policy, as_of) must produce byte-identical cadence JSON"
    );

    let document: Value = serde_json::from_str(&first)
        .unwrap_or_else(|err| std::panic::panic_any(format!("cadence JSON parses: {err}")));
    assert_eq!(
        document.pointer("/schema_id").and_then(Value::as_str),
        Some("cargo-allow.cadence.v1")
    );
    assert_eq!(
        document.pointer("/command").and_then(Value::as_str),
        Some("cadence")
    );
    assert_eq!(
        document.pointer("/as_of").and_then(Value::as_str),
        Some("2026-10-01")
    );
    assert_eq!(
        document.pointer("/as_of_source").and_then(Value::as_str),
        Some("explicit")
    );
    for (class, count) in [
        ("current", 1),
        ("review_due_soon", 1),
        ("review_overdue", 1),
        ("expiring", 1),
        ("expired", 1),
        ("invalid", 0),
    ] {
        assert_eq!(
            document
                .pointer(&format!("/summary/{class}"))
                .and_then(Value::as_u64),
            Some(count),
            "summary class {class}"
        );
    }

    let expect = |document: &Value, id: &str, class: &str, days: i64| {
        let row = row_for(document, id);
        assert_eq!(
            row.pointer("/class").and_then(Value::as_str),
            Some(class),
            "{id} class"
        );
        assert_eq!(
            row.pointer("/days_remaining").and_then(Value::as_i64),
            Some(days),
            "{id} exact signed day delta"
        );
        assert_eq!(
            row.pointer("/owner").and_then(Value::as_str),
            Some("core/parser"),
            "{id} owner identity"
        );
        let path = row
            .pointer("/source_path")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert_eq!(
            path,
            format!("src/{}.rs", id.trim_start_matches("allow-")),
            "{id} path identity"
        );
        assert!(
            row.pointer("/evidence_refs")
                .and_then(Value::as_array)
                .is_some_and(|refs| !refs.is_empty()),
            "{id} retains evidence identity"
        );
        assert!(
            row.pointer("/required_disposition")
                .and_then(Value::as_str)
                .is_some_and(|disposition| !disposition.is_empty()),
            "{id} carries a required disposition"
        );
    };
    expect(&document, "allow-due-soon", "review_due_soon", 7);
    expect(&document, "allow-overdue", "review_overdue", -16);
    expect(&document, "allow-expiring", "expiring", 7);
    expect(&document, "allow-expired", "expired", -16);
    let current = row_for(&document, "allow-current");
    assert_eq!(
        current.pointer("/class").and_then(Value::as_str),
        Some("current"),
        "the far-future review entry stays current"
    );
    assert_eq!(
        current.pointer("/days_remaining").and_then(Value::as_i64),
        None,
        "current rows carry no driving-date delta"
    );
    // Driving-date attribution follows the class.
    assert_eq!(
        row_for(&document, "allow-overdue")
            .pointer("/driving_date")
            .and_then(Value::as_str),
        Some("review_after")
    );
    assert_eq!(
        row_for(&document, "allow-expired")
            .pointer("/driving_date")
            .and_then(Value::as_str),
        Some("expires")
    );
    assert_eq!(
        current.pointer("/driving_date").and_then(Value::as_str),
        Some("none")
    );
}

/// Fixture b: classification flips exactly at the documented boundaries.
/// review_after D=2026-09-20 (inclusive `<=`) and expires E=2026-10-25
/// (strict `<`): the expires day itself is still `expiring` with zero days
/// remaining, and only E+1 is `expired`.
#[test]
fn cadence_boundary_pins_review_and_expiry_transitions() {
    let entries = vec![reviewed_entry(
        "allow-boundary",
        "src/boundary.rs",
        "2026-06-01",
        Some("2026-09-20"),
        Some("2026-10-25"),
    )];
    let fixture = Fixture::new("boundaries", &entries);
    let class_at = |as_of: &str| {
        let document = fixture.cadence_json(as_of, &format!("boundary-{as_of}"));
        let row = row_for(&document, "allow-boundary");
        (
            row.pointer("/class")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            row.pointer("/days_remaining").and_then(Value::as_i64),
        )
    };
    assert_eq!(
        class_at("2026-09-19"),
        ("review_due_soon".to_string(), Some(1)),
        "D-1 is inside the due-soon horizon"
    );
    assert_eq!(
        class_at("2026-09-20"),
        ("review_overdue".to_string(), Some(0)),
        "D itself is overdue (inclusive review boundary)"
    );
    assert_eq!(
        class_at("2026-10-24"),
        ("expiring".to_string(), Some(1)),
        "E-1 is expiring; the overdue review loses precedence to expiring"
    );
    assert_eq!(
        class_at("2026-10-25"),
        ("expiring".to_string(), Some(0)),
        "E itself is NOT expired (strict policy-entry expiry boundary, #4351)"
    );
    assert_eq!(
        class_at("2026-10-26"),
        ("expired".to_string(), Some(-1)),
        "E+1 is expired"
    );
}

/// Fixture c: the loader owns the fail-closed boundary. Malformed
/// expires/review_after values and descending dates never reach the
/// classifier through the CLI; cadence fails loudly with the loader's
/// message instead of rendering rows.
#[test]
fn cadence_fails_loudly_on_malformed_and_descending_dates() {
    let cases: Vec<(&str, String, &str)> = vec![
        (
            "bad-expires",
            reviewed_entry(
                "allow-bad",
                "src/bad.rs",
                "2026-06-01",
                None,
                Some("2026-13-40"),
            ),
            "invalid expires date",
        ),
        (
            "bad-review",
            reviewed_entry(
                "allow-bad",
                "src/bad.rs",
                "2026-06-01",
                Some("not-a-date"),
                None,
            ),
            "invalid review_after date",
        ),
        (
            "descending",
            reviewed_entry(
                "allow-bad",
                "src/bad.rs",
                "2026-01-02",
                Some("2026-01-01"),
                None,
            ),
            "review_after must not be before created",
        ),
    ];
    for (name, entry, expected_message) in cases {
        let fixture = Fixture::new(name, &[entry]);
        let output = fixture.run(&["--as-of".to_string(), "2026-10-01".to_string()]);
        assert!(
            !output.status.success(),
            "cadence over {name} policy must fail closed: stdout=`{}`",
            String::from_utf8_lossy(&output.stdout)
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(expected_message),
            "{name} failure should name the defect ({expected_message}): {stderr}"
        );
    }
}

/// Fixture e: never a gate. Cadence leaves the policy bytes untouched, the
/// `check --mode no-new` verdict is identical before and after, and the
/// cadence artifact carries no check/diff verdict fields.
#[test]
fn cadence_never_mutates_policy_or_check_verdict() {
    let entries = vec![
        reviewed_entry(
            "allow-current",
            "src/current.rs",
            "2026-06-01",
            Some("2027-06-01"),
            None,
        ),
        reviewed_entry(
            "allow-overdue",
            "src/overdue.rs",
            "2026-06-01",
            Some("2026-09-15"),
            None,
        ),
    ];
    let fixture = Fixture::new("never-a-gate", &entries);
    let check = |label: &str| {
        let output = cargo_allow_command()
            .arg("check")
            .arg("--mode")
            .arg("no-new")
            .arg("--root")
            .arg(&fixture.root)
            .arg("--config")
            .arg("policy/allow.toml")
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("check {label}: {err}")));
        (output.status.code(), output.stdout.clone())
    };
    let before_bytes = fixture.policy_bytes();
    let check_before = check("before");
    let document = fixture.cadence_json("2026-10-01", "gate");
    let after_bytes = fixture.policy_bytes();
    assert_eq!(
        before_bytes, after_bytes,
        "cadence must not mutate policy bytes"
    );
    let check_after = check("after");
    assert_eq!(
        check_before, check_after,
        "the no-new verdict must be identical before and after cadence"
    );

    for verdict_key in ["failed", "passed", "status", "mode", "verdict", "advisory"] {
        assert!(
            document.get(verdict_key).is_none(),
            "cadence artifact must not carry verdict field {verdict_key}"
        );
    }
}

/// Fixture f: all three renderings derive from one semantic result — every
/// (class, id) row in the JSON appears in both the human and the markdown
/// renderings.
#[test]
fn cadence_human_and_json_renderings_agree() {
    let entries = vec![
        reviewed_entry(
            "allow-current",
            "src/current.rs",
            "2026-06-01",
            Some("2027-06-01"),
            None,
        ),
        reviewed_entry(
            "allow-due-soon",
            "src/due-soon.rs",
            "2026-06-01",
            Some("2026-10-08"),
            None,
        ),
        reviewed_entry(
            "allow-overdue",
            "src/overdue.rs",
            "2026-06-01",
            Some("2026-09-15"),
            None,
        ),
        reviewed_entry(
            "allow-expiring",
            "src/expiring.rs",
            "2026-06-01",
            None,
            Some("2026-10-08"),
        ),
        reviewed_entry(
            "allow-expired",
            "src/expired.rs",
            "2026-06-01",
            None,
            Some("2026-09-15"),
        ),
    ];
    let fixture = Fixture::new("parity", &entries);
    let document = fixture.cadence_json("2026-10-01", "parity-json");
    let human = fixture.run_expect_success(&["--as-of", "2026-10-01", "--format", "human"]);
    let markdown = fixture.run_expect_success(&["--as-of", "2026-10-01", "--format", "markdown"]);
    for (allow_id, class, _) in rows(&document) {
        let marker = format!("[{class}] {allow_id}");
        assert!(
            human.contains(&marker),
            "human rendering must carry the JSON row {marker}"
        );
        let markdown_marker = format!("| {allow_id} | {class} | ");
        assert!(
            markdown.contains(&markdown_marker),
            "markdown rendering must carry the JSON row {markdown_marker}"
        );
    }
    assert!(
        human.contains("as_of: 2026-10-01 (explicit)"),
        "human rendering should state the explicit as-of"
    );
    assert!(
        human.contains("horizons: review_due_soon within 14 days; expiring within 14 days"),
        "human rendering should document both horizons"
    );
    assert!(
        markdown.starts_with("# cargo-allow cadence"),
        "markdown rendering should open with the cadence heading"
    );
    assert!(
        markdown.contains("| Class | Entries |"),
        "markdown rendering should carry the summary table"
    );
}

/// Fixture g: omitting `--as-of` classifies at the ambient UTC day, marked
/// `ambient_utc_day`, and agrees row-for-row with the explicit run for that
/// same day (relative-date fixture, no date bomb).
#[test]
fn cadence_default_as_of_is_the_ambient_day() {
    let today = allow_core::SimpleDate::today_utc_approx();
    let entries = vec![
        reviewed_entry(
            "allow-relative-due",
            "src/due.rs",
            "2026-01-01",
            Some(&today.add_days(3).to_string()),
            None,
        ),
        reviewed_entry(
            "allow-relative-overdue",
            "src/overdue.rs",
            "2026-01-01",
            Some(&today.add_days(-1).to_string()),
            None,
        ),
    ];
    let fixture = Fixture::new("default-as-of", &entries);
    let default_document = fixture.cadence_json_ambient("default");
    let today_text = today.to_string();
    assert_eq!(
        default_document.pointer("/as_of").and_then(Value::as_str),
        Some(today_text.as_str()),
        "the default as-of is the ambient UTC day"
    );
    assert_eq!(
        default_document
            .pointer("/as_of_source")
            .and_then(Value::as_str),
        Some("ambient_utc_day")
    );
    let explicit_document = fixture.cadence_json(&today_text, "default-explicit");
    assert_eq!(
        explicit_document
            .pointer("/as_of_source")
            .and_then(Value::as_str),
        Some("explicit")
    );
    assert_eq!(
        rows(&default_document),
        rows(&explicit_document),
        "the ambient-day default must equal the explicit run for that day"
    );
    let due = row_for(&default_document, "allow-relative-due");
    assert_eq!(
        due.pointer("/class").and_then(Value::as_str),
        Some("review_due_soon"),
        "today+3 lands inside the due-soon horizon"
    );
    let overdue = row_for(&default_document, "allow-relative-overdue");
    assert_eq!(
        overdue.pointer("/class").and_then(Value::as_str),
        Some("review_overdue"),
        "today-1 is overdue"
    );
}

/// Fixture h: an unparseable `--as-of` value fails the invocation at the
/// argument boundary with the usage exit code, before any policy read.
#[test]
fn cadence_malformed_as_of_fails_with_usage_exit_code() {
    let entries = vec![reviewed_entry(
        "allow-current",
        "src/current.rs",
        "2026-06-01",
        Some("2027-06-01"),
        None,
    )];
    let fixture = Fixture::new("malformed-as-of", &entries);
    let output = fixture.run(&["--as-of".to_string(), "not-a-date".to_string()]);
    assert_eq!(
        output.status.code(),
        Some(2),
        "malformed --as-of exits with the usage code like every other arg error"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("invalid --as-of date `not-a-date`"),
        "stderr should name the cadence date contract: {stderr}"
    );
}
