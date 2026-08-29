//! Adapter projection tests (#3965): one exact diff evaluation drives the
//! whole check — clean PRs pass, blocking findings annotate, resolved
//! findings stay in the summary, and sibling products never appear.

use allow_report::{
    BaseScanCompletenessV1, GitHubPrCheckResultV1, GitHubPrCheckSubjectV1,
    GitHubPrDiffReportViewV1, GitHubPrFindingChangeRowViewV1, project_github_pr_check,
    validate_github_pr_check_v1,
};

fn subject() -> GitHubPrCheckSubjectV1 {
    GitHubPrCheckSubjectV1 {
        repository: "EffortlessMetrics/cargo-allow".to_string(),
        pr_number: 4021,
        base: "main".to_string(),
        merge_base: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        head: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
    }
}

fn row(
    change: &str,
    movement: &str,
    delta: &str,
    key: &str,
    path: &str,
    line: u32,
) -> GitHubPrFindingChangeRowViewV1 {
    GitHubPrFindingChangeRowViewV1 {
        change: change.to_string(),
        movement: movement.to_string(),
        posture_delta: delta.to_string(),
        changed_in_diff: true,
        key: key.to_string(),
        kind: "panic".to_string(),
        path: path.to_string(),
        line: Some(line),
    }
}

fn empty_report() -> GitHubPrDiffReportViewV1 {
    GitHubPrDiffReportViewV1 {
        schema_id: "cargo-allow.report.v1".to_string(),
        status: "passed".to_string(),
        failed: false,
        inventory: None,
        diff: None,
    }
}

#[test]
fn clean_exact_pr_projects_passed_without_annotations() -> Result<(), String> {
    let mut report = empty_report();
    report.diff = Some(allow_report::GitHubPrDiffViewV1 {
        net_posture: "unchanged".to_string(),
        finding_changes: Vec::new(),
    });
    let check = project_github_pr_check(
        &report,
        &subject(),
        BaseScanCompletenessV1::Complete,
        10,
        "artifact-set-001",
    );
    validate_github_pr_check_v1(&check).map_err(|error| format!("clean check invalid: {error}"))?;
    if check.result != GitHubPrCheckResultV1::Passed {
        return Err(format!("clean PR projected {check:?}"));
    }
    if !check.annotations.is_empty() || check.annotated_count != 0 {
        return Err("clean PR carried annotations".to_string());
    }
    Ok(())
}

#[test]
fn introduced_blocking_finding_annotates_first() -> Result<(), String> {
    let mut report = empty_report();
    report.diff = Some(allow_report::GitHubPrDiffViewV1 {
        net_posture: "worse".to_string(),
        finding_changes: vec![
            row("new", "introduced", "worsened", "k-1", "src/new.rs", 7),
            row("removed", "removed", "improved", "k-2", "src/old.rs", 3),
        ],
    });
    let check = project_github_pr_check(
        &report,
        &subject(),
        BaseScanCompletenessV1::Complete,
        10,
        "artifact-set-002",
    );
    if check.result != GitHubPrCheckResultV1::FindingsBlocking {
        return Err(format!("introduced finding projected {check:?}"));
    }
    if check.annotations.len() != 1 || check.annotations[0].annotation_id != "k-1" {
        return Err("introduced row was not the only annotation".to_string());
    }
    // The resolved row stays in the summary, never as an inline annotation.
    if check.resolved_count != 1 {
        return Err("resolved row was not counted in the summary".to_string());
    }
    Ok(())
}

#[test]
fn advisory_rows_cannot_become_blocking() -> Result<(), String> {
    let mut report = empty_report();
    // A retained, touched row is advisory movement, not a new blocking one.
    report.diff = Some(allow_report::GitHubPrDiffViewV1 {
        net_posture: "review-required".to_string(),
        finding_changes: vec![row(
            "new",
            "retained",
            "review_required",
            "k-touch",
            "src/touched.rs",
            11,
        )],
    });
    let check = project_github_pr_check(
        &report,
        &subject(),
        BaseScanCompletenessV1::Complete,
        10,
        "artifact-set-003",
    );
    if check.result != GitHubPrCheckResultV1::FindingsAdvisory {
        return Err(format!(
            "advisory row strengthened into blocking: {check:?}"
        ));
    }
    if check.persisting_touched_count != 1 || check.annotated_count != 1 {
        return Err("touched row was not annotated exactly once".to_string());
    }
    Ok(())
}

#[test]
fn malformed_or_unsupported_artifacts_stay_non_clean() -> Result<(), String> {
    let mut malformed = empty_report();
    malformed.schema_id = "not-cargo-allow".to_string();
    malformed.failed = true;
    let check = project_github_pr_check(
        &malformed,
        &subject(),
        BaseScanCompletenessV1::Complete,
        10,
        "artifact-set-004",
    );
    if check.result != GitHubPrCheckResultV1::Unsupported {
        return Err(format!("malformed artifact was not Unsupported: {check:?}"));
    }

    let mut failed = empty_report();
    failed.failed = true;
    failed.status = "failed".to_string();
    let check = project_github_pr_check(
        &failed,
        &subject(),
        BaseScanCompletenessV1::Complete,
        10,
        "artifact-set-005",
    );
    if check.result != GitHubPrCheckResultV1::InstrumentFailure {
        return Err(format!("failed scan was not InstrumentFailure: {check:?}"));
    }
    Ok(())
}

#[test]
fn annotation_identity_is_the_canonical_row_key() -> Result<(), String> {
    let mut report = empty_report();
    report.diff = Some(allow_report::GitHubPrDiffViewV1 {
        net_posture: "worse".to_string(),
        finding_changes: vec![row(
            "new",
            "introduced",
            "worsened",
            "panic:sha256:deadbeef",
            "src/new.rs",
            7,
        )],
    });
    let check = project_github_pr_check(
        &report,
        &subject(),
        BaseScanCompletenessV1::Complete,
        10,
        "artifact-set-006",
    );
    let annotation = check
        .annotations
        .first()
        .ok_or_else(|| "expected one annotation".to_string())?;
    if annotation.annotation_id != "panic:sha256:deadbeef" {
        return Err("annotation identity is not the canonical row key".to_string());
    }
    if annotation.message.contains("free-form remediation") {
        return Err("annotation message carries free-form prose".to_string());
    }
    Ok(())
}
