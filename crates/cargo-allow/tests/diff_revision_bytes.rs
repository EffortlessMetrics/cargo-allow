//! Real Git and CLI regressions for strict revision source bytes (#4428).

use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const VALID: &[u8] = b"fn retained(value: Option<u8>) -> u8 { value.unwrap() }\n";
const INVALID: &[u8] =
    b"// invalid comment byte: \xff\nfn retained(value: Option<u8>) -> u8 { value.unwrap() }\n";
const POLICY: &str = r#"schema_version = "0.1"
policy = "cargo-allow"

[requirements]
owner_required = true
reason_required = true
classification_required = true
evidence_required = false
expires_or_review_after_required = true
stale_entries_fail = false
allow_bare_allow_attributes = false
lint_policy_id_required = false

[requirements.unsafe]
evidence_required = true
safety_comment_required = false
"#;

struct Fixture(PathBuf);

impl Fixture {
    fn new(label: &str) -> Result<Self, String> {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let fixture = Self(std::env::temp_dir().join(format!(
            "cargo-allow-revision-bytes-{label}-{}-{unique}",
            std::process::id(),
        )));
        for path in ["src", "policy", "target/cargo-allow"] {
            fs::create_dir_all(fixture.0.join(path)).map_err(|error| error.to_string())?;
        }
        fixture.write("policy/allow.toml", POLICY.as_bytes())?;
        fixture.git(&["init", "-q"])?;
        fixture.git(&["config", "user.name", "cargo-allow test"])?;
        fixture.git(&["config", "user.email", "cargo-allow@example.invalid"])?;
        fixture.git(&["config", "core.autocrlf", "false"])?;
        Ok(fixture)
    }

    fn write(&self, path: &str, bytes: &[u8]) -> Result<(), String> {
        fs::write(self.0.join(path), bytes).map_err(|error| error.to_string())
    }

    fn git(&self, args: &[&str]) -> Result<String, String> {
        let output = Command::new("git")
            .arg("-C")
            .arg(&self.0)
            .args(args)
            .output()
            .map_err(|error| error.to_string())?;
        if !output.status.success() {
            return Err(format!(
                "git {args:?}: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        String::from_utf8(output.stdout)
            .map(|text| text.trim().to_string())
            .map_err(|error| error.to_string())
    }

    fn commit(&self, label: &str) -> Result<String, String> {
        self.git(&["add", "src", "policy"])?;
        self.git(&["-c", "commit.gpgsign=false", "commit", "-q", "-m", label])?;
        self.git(&["rev-parse", "HEAD"])
    }

    fn diff(&self, base: &str, head: Option<&str>, format: &str) -> Result<Output, String> {
        let receipt = self
            .0
            .join(format!("target/cargo-allow/{format}.receipt.json"));
        let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-allow"));
        command
            .arg("diff")
            .arg("--root")
            .arg(&self.0)
            .args([
                "--config",
                "policy/allow.toml",
                "--kind",
                "panic",
                "--base",
                base,
                "--format",
                format,
            ])
            .arg("--receipt")
            .arg(&receipt);
        if let Some(head) = head {
            command.args(["--head", head]);
        }
        command.output().map_err(|error| error.to_string())
    }

    fn receipt(&self, format: &str) -> Result<Value, String> {
        let bytes = fs::read(
            self.0
                .join(format!("target/cargo-allow/{format}.receipt.json")),
        )
        .map_err(|error| error.to_string())?;
        serde_json::from_slice(&bytes).map_err(|error| error.to_string())
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn json_report(output: &Output) -> Result<Value, String> {
    serde_json::from_slice(&output.stdout).map_err(|error| {
        format!(
            "diff JSON: {error}; stderr: {}",
            String::from_utf8_lossy(&output.stderr),
        )
    })
}

fn require_side_facts(
    value: &Value,
    base_complete: bool,
    head_complete: bool,
    class: &str,
) -> Result<(), String> {
    if value.get("result_class").and_then(Value::as_str) != Some(class)
        || value
            .get("base_inventory_complete")
            .and_then(Value::as_bool)
            != Some(true)
        || value
            .get("head_inventory_complete")
            .and_then(Value::as_bool)
            != Some(true)
        || value.get("base_scanner_complete").and_then(Value::as_bool) != Some(base_complete)
        || value.get("head_scanner_complete").and_then(Value::as_bool) != Some(head_complete)
    {
        return Err(format!("independent revision facts changed: {value}"));
    }
    Ok(())
}

#[test]
fn diff_preserves_partial_side_facts_across_formats() -> Result<(), String> {
    for (base_complete, head_complete, class) in [
        (false, true, "base_partial"),
        (true, false, "head_partial"),
        (false, false, "both_partial"),
    ] {
        let fixture = Fixture::new(class)?;
        fixture.write("src/retained.rs", VALID)?;
        fixture.write(
            "src/subject.rs",
            if base_complete { VALID } else { INVALID },
        )?;
        let base = fixture.commit("base")?;
        fixture.write(
            "src/subject.rs",
            if head_complete { VALID } else { INVALID },
        )?;
        fixture.write(
            "src/new.rs",
            b"fn added(value: Option<u8>) -> u8 { value.unwrap() }\n",
        )?;
        let head = fixture.commit("head")?;
        let output = fixture.diff(&base, Some(&head), "json")?;
        if output.status.code() != Some(1) || !output.stderr.is_empty() {
            return Err(format!(
                "partial diff must fail with quiet JSON stderr: {}",
                String::from_utf8_lossy(&output.stderr),
            ));
        }
        let report = json_report(&output)?;
        let analysis = report
            .pointer("/diff/diff_analysis")
            .ok_or_else(|| "report lost diff analysis".to_string())?;
        require_side_facts(analysis, base_complete, head_complete, class)?;
        let receipt = fixture.receipt("json")?;
        if receipt.get("diff_analysis") != Some(analysis)
            || receipt.get("status").and_then(Value::as_str) != Some("failed")
        {
            return Err(format!(
                "receipt does not retain the report side facts: {receipt}"
            ));
        }
        let changes = report
            .pointer("/diff/finding_changes")
            .and_then(Value::as_array)
            .ok_or_else(|| "diff lost finding changes".to_string())?;
        if changes
            .iter()
            .any(|change| change.get("path").and_then(Value::as_str) == Some("src/subject.rs"))
            || analysis.get("removed").and_then(Value::as_u64) != Some(0)
            || (!base_complete && analysis.get("introduced").and_then(Value::as_u64) != Some(0))
        {
            return Err(format!(
                "incomplete input earned introduced/resolved movement: {analysis}, {changes:?}"
            ));
        }
        let findings = report
            .get("findings")
            .and_then(Value::as_array)
            .ok_or_else(|| "diff lost retained findings".to_string())?;
        for path in ["src/retained.rs", "src/new.rs"] {
            if !findings
                .iter()
                .any(|finding| finding.get("path").and_then(Value::as_str) == Some(path))
            {
                return Err(format!(
                    "valid path {path} disappeared from partial output: {findings:?}"
                ));
            }
        }
        // Feed the actual binary's report to the supported PR-check consumer.
        let pr_report: allow_report::GitHubPrDiffReportViewV1 =
            serde_json::from_value(report.clone()).map_err(|error| error.to_string())?;
        let completeness: allow_report::BaseScanCompletenessV1 =
            serde_json::from_value(Value::String(class.to_string()))
                .map_err(|error| error.to_string())?;
        let pr_check = allow_report::project_github_pr_check(
            &pr_report,
            &allow_report::GitHubPrCheckSubjectV1 {
                repository: "fixture/revision-bytes".to_string(),
                pr_number: 1,
                base: base.clone(),
                merge_base: base.clone(),
                head: head.clone(),
            },
            completeness,
            10,
            "fixture-report",
        );
        // The adapter currently prioritizes a failed artifact as instrument
        // failure. Either non-clean result must preserve the producer's exact
        // completeness and cannot award ordinary movement counts.
        if !matches!(
            pr_check.result,
            allow_report::GitHubPrCheckResultV1::Partial
                | allow_report::GitHubPrCheckResultV1::InstrumentFailure
        ) || pr_check.base_scan_completeness != completeness
            || pr_check.introduced_count != 0
            || pr_check.resolved_count != 0
        {
            return Err(format!(
                "PR consumer invented confident movement: {pr_check:?}"
            ));
        }
        for format in ["human", "markdown"] {
            let rendered = fixture.diff(&base, Some(&head), format)?;
            let text = String::from_utf8(rendered.stdout).map_err(|error| error.to_string())?;
            let diagnostics =
                String::from_utf8(rendered.stderr).map_err(|error| error.to_string())?;
            if rendered.status.code() != Some(1)
                || !diagnostics.contains("skipped `src/subject.rs`")
                || !diagnostics.contains("not valid UTF-8")
            {
                return Err(format!(
                    "{format} lost non-clean relative-path diagnostics: {diagnostics}"
                ));
            }
            let expected = if format == "human" {
                vec![
                    format!("result_class={class}"),
                    format!("base_scanner_complete={base_complete}"),
                    format!("head_scanner_complete={head_complete}"),
                ]
            } else {
                vec![
                    "## PR Summary".to_string(),
                    format!("Result class: `{class}`"),
                    format!("Base scanner complete: `{base_complete}`"),
                    format!("Head scanner complete: `{head_complete}`"),
                ]
            };
            if expected.iter().any(|part| !text.contains(part)) {
                return Err(format!(
                    "{format} or its PR summary lost independent facts: {text}"
                ));
            }
            if fixture.receipt(format)?.get("diff_analysis") != Some(analysis) {
                return Err(format!("{format} receipt disagrees with JSON analysis"));
            }
        }
    }
    Ok(())
}

#[test]
fn diff_distinguishes_deleted_and_unreadable_source() -> Result<(), String> {
    let fixture = Fixture::new("deletion-control")?;
    fixture.write("src/subject.rs", VALID)?;
    let base = fixture.commit("base")?;
    fixture.write("src/subject.rs", INVALID)?;
    let unreadable = fixture.commit("unreadable")?;
    let partial = json_report(&fixture.diff(&base, Some(&unreadable), "json")?)?;
    require_side_facts(
        partial
            .pointer("/diff/diff_analysis")
            .ok_or_else(|| "missing partial analysis".to_string())?,
        true,
        false,
        "head_partial",
    )?;
    fs::remove_file(fixture.0.join("src/subject.rs")).map_err(|error| error.to_string())?;
    fixture.write("src/empty.rs", b"fn empty() {}\n")?;
    let deleted = fixture.commit("deleted")?;
    let complete_output = fixture.diff(&base, Some(&deleted), "json")?;
    let complete = json_report(&complete_output)?;
    let analysis = complete
        .pointer("/diff/diff_analysis")
        .ok_or_else(|| "missing complete analysis".to_string())?;
    require_side_facts(analysis, true, true, "complete")?;
    if complete_output.status.code() != Some(0)
        || analysis.get("removed").and_then(Value::as_u64) != Some(1)
        || partial
            .pointer("/diff/diff_analysis/removed")
            .and_then(Value::as_u64)
            != Some(0)
    {
        return Err(format!(
            "genuine deletion and skipped blob were collapsed: {partial} / {complete}"
        ));
    }
    Ok(())
}

#[test]
fn current_tree_diff_keeps_unstaged_missing_source_partial() -> Result<(), String> {
    let fixture = Fixture::new("current-deletion-control")?;
    fixture.write("src/subject.rs", VALID)?;
    fixture.write("src/kept.rs", b"fn kept() {}\n")?;
    let base = fixture.commit("base")?;
    fs::remove_file(fixture.0.join("src/subject.rs")).map_err(|error| error.to_string())?;

    let cases = [(false, "head_partial", 0, 1), (true, "complete", 1, 0)];
    for (staged, class, removed, exit) in cases {
        if staged {
            fixture.git(&["add", "-u", "--", "src/subject.rs"])?;
        }
        let output = fixture.diff(&base, None, "json")?;
        let report = json_report(&output)?;
        let analysis = report
            .pointer("/diff/diff_analysis")
            .ok_or_else(|| "missing current-tree deletion analysis".to_string())?;
        if output.status.code() != Some(exit)
            || !output.stderr.is_empty()
            || analysis.get("result_class").and_then(Value::as_str) != Some(class)
            || analysis
                .get("base_inventory_complete")
                .and_then(Value::as_bool)
                != Some(true)
            || analysis
                .get("head_inventory_complete")
                .and_then(Value::as_bool)
                != Some(staged)
            || analysis
                .get("base_scanner_complete")
                .and_then(Value::as_bool)
                != Some(true)
            || analysis
                .get("head_scanner_complete")
                .and_then(Value::as_bool)
                != Some(true)
            || analysis.get("introduced").and_then(Value::as_u64) != Some(0)
            || analysis.get("removed").and_then(Value::as_u64) != Some(removed)
        {
            return Err(format!(
                "current-tree deletion staged={staged} lost coverage or movement: {report}"
            ));
        }
        let receipt = fixture.receipt("json")?;
        if receipt.get("diff_analysis") != Some(analysis)
            || receipt.get("status").and_then(Value::as_str)
                != Some(if staged { "passed" } else { "failed" })
        {
            return Err(format!(
                "current-tree deletion receipt disagrees with coverage: {receipt}"
            ));
        }
        let changes = report
            .pointer("/diff/finding_changes")
            .and_then(Value::as_array)
            .ok_or_else(|| "current-tree deletion lost finding changes".to_string())?;
        let valid_changes = if staged {
            changes.len() == 1
                && changes.iter().any(|change| {
                    change.get("kind").and_then(Value::as_str) == Some("removed")
                        && change.get("path").and_then(Value::as_str) == Some("src/subject.rs")
                })
        } else {
            changes.is_empty()
        };
        if !valid_changes {
            return Err(format!(
                "current-tree deletion staged={staged} lost finding details: {changes:?}"
            ));
        }
    }
    Ok(())
}

#[test]
fn current_tree_head_and_committed_head_reject_the_same_invalid_source() -> Result<(), String> {
    let fixture = Fixture::new("current-tree-parity")?;
    fixture.write("src/subject.rs", VALID)?;
    let base = fixture.commit("base")?;
    fixture.write("src/subject.rs", INVALID)?;
    let head = fixture.commit("head")?;
    for revision in [Some(head.as_str()), None] {
        let output = fixture.diff(&base, revision, "json")?;
        let report = json_report(&output)?;
        require_side_facts(
            report
                .pointer("/diff/diff_analysis")
                .ok_or_else(|| "missing parity analysis".to_string())?,
            true,
            false,
            "head_partial",
        )?;
        if output.status.code() != Some(1)
            || report
                .pointer("/diff/diff_analysis/removed")
                .and_then(Value::as_u64)
                != Some(0)
        {
            return Err(format!(
                "current/committed head falsely cleaned missing source: {report}"
            ));
        }
    }
    Ok(())
}

#[test]
fn binary_diff_rejects_cap_plus_one_but_scans_the_exact_cap() -> Result<(), String> {
    let fixture = Fixture::new("binary-cap-boundary")?;
    let cap = usize::try_from(allow_core::SOURCE_FILE_READ_MAX_BYTES)
        .map_err(|error| error.to_string())?;
    let mut bytes = VALID.to_vec();
    bytes.extend_from_slice(b"//");
    bytes.resize(cap, b'x');
    fixture.write("src/subject.rs", &bytes)?;
    let base = fixture.commit("at cap")?;
    bytes.push(b'x');
    fixture.write("src/subject.rs", &bytes)?;
    drop(bytes);
    let head = fixture.commit("over cap")?;
    let output = fixture.diff(&base, Some(&head), "json")?;
    let report = json_report(&output)?;
    require_side_facts(
        report
            .pointer("/diff/diff_analysis")
            .ok_or_else(|| "missing cap analysis".to_string())?,
        true,
        false,
        "head_partial",
    )?;
    if output.status.code() != Some(1)
        || !output.stderr.is_empty()
        || report
            .pointer("/diff/diff_analysis/removed")
            .and_then(Value::as_u64)
            != Some(0)
    {
        return Err(format!(
            "binary cap boundary produced false confidence: {report}"
        ));
    }
    Ok(())
}

#[test]
fn malformed_revision_identity_is_an_instrument_failure_not_a_clean_diff() -> Result<(), String> {
    let fixture = Fixture::new("missing-revision")?;
    fixture.write("src/subject.rs", VALID)?;
    let head = fixture.commit("head")?;
    let output = fixture.diff("refs/heads/does-not-exist", Some(&head), "json")?;
    if output.status.success() {
        return Err("missing Git revision produced a successful diff".to_string());
    }
    if let Ok(value) = serde_json::from_slice::<Value>(&output.stdout)
        && value
            .pointer("/diff/diff_analysis/result_class")
            .and_then(Value::as_str)
            == Some("complete")
    {
        return Err(format!(
            "missing Git input produced a clean diff artifact: {value}"
        ));
    }
    Ok(())
}
