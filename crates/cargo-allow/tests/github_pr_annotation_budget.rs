//! Annotation budget tests (#3965): deterministic priority order, honest
//! omitted counts, and truncation never becoming a clean result.

use allow_report::{
    BaseScanCompletenessV1, GitHubPrCheckResultV1, GitHubPrCheckSubjectV1,
    GitHubPrDiffReportViewV1, GitHubPrDiffViewV1, GitHubPrFindingChangeRowViewV1,
    project_github_pr_check,
};

fn subject() -> GitHubPrCheckSubjectV1 {
    GitHubPrCheckSubjectV1 {
        repository: "EffortlessMetrics/cargo-allow".to_string(),
        pr_number: 4021,
        base: "main".to_string(),
        merge_base: "cccccccccccccccccccccccccccccccccccccccc".to_string(),
        head: "dddddddddddddddddddddddddddddddddddddddd".to_string(),
    }
}

fn introduced_row(index: usize) -> GitHubPrFindingChangeRowViewV1 {
    GitHubPrFindingChangeRowViewV1 {
        change: "new".to_string(),
        movement: "introduced".to_string(),
        posture_delta: "worsened".to_string(),
        changed_in_diff: true,
        key: format!("panic:k{index:03}"),
        kind: "panic".to_string(),
        path: format!("src/file{index}.rs"),
        line: Some((index as u32) + 1),
    }
}

fn budget_report(introduced_rows: usize) -> GitHubPrDiffReportViewV1 {
    GitHubPrDiffReportViewV1 {
        schema_id: "cargo-allow.report.v1".to_string(),
        status: "passed".to_string(),
        failed: false,
        inventory: None,
        diff: Some(GitHubPrDiffViewV1 {
            net_posture: "worse".to_string(),
            finding_changes: (0..introduced_rows).map(introduced_row).collect(),
        }),
    }
}

#[test]
fn budget_truncates_in_deterministic_priority_order() -> Result<(), String> {
    let report = budget_report(9);
    let first = project_github_pr_check(
        &report,
        &subject(),
        BaseScanCompletenessV1::Complete,
        3,
        "artifact-set-budget",
    );
    let second = project_github_pr_check(
        &report,
        &subject(),
        BaseScanCompletenessV1::Complete,
        3,
        "artifact-set-budget",
    );
    if first != second {
        return Err("equal inputs projected different checks".to_string());
    }
    if first.annotated_count != 3 || first.omitted_count != 6 {
        return Err(format!(
            "budget accounting drifted: annotated {} omitted {}",
            first.annotated_count, first.omitted_count
        ));
    }
    // Introduced rows all share priority; ties break by stable key, so the
    // first three keys in sorted order must be annotated.
    let ids: Vec<String> = first
        .annotations
        .iter()
        .map(|annotation| annotation.annotation_id.clone())
        .collect();
    let mut expected = vec![
        "panic:k000".to_string(),
        "panic:k001".to_string(),
        "panic:k002".to_string(),
    ];
    expected.sort();
    let mut sorted_ids = ids.clone();
    sorted_ids.sort();
    if sorted_ids != expected {
        return Err(format!("priority selection drifted: {ids:?}"));
    }
    if !first
        .limitations
        .iter()
        .any(|limit| limit.contains("budget truncated"))
    {
        return Err("truncation was not disclosed in limitations".to_string());
    }
    Ok(())
}

#[test]
fn truncated_budget_never_projects_passed() -> Result<(), String> {
    // A truncated blocking row set stays FindingsBlocking; a truncated
    // advisory row set stays FindingsAdvisory — truncation never cleans.
    let blocking = project_github_pr_check(
        &budget_report(5),
        &subject(),
        BaseScanCompletenessV1::Complete,
        2,
        "artifact-set-budget-blocking",
    );
    if blocking.result != GitHubPrCheckResultV1::FindingsBlocking {
        return Err(format!("truncated blocking became {blocking:?}"));
    }
    if blocking.omitted_count != 3 {
        return Err(format!(
            "omitted blocking rows were hidden: {}",
            blocking.omitted_count
        ));
    }
    Ok(())
}

#[test]
fn under_budget_projection_annotates_everything_without_omissions() -> Result<(), String> {
    let report = budget_report(2);
    let check = project_github_pr_check(
        &report,
        &subject(),
        BaseScanCompletenessV1::Complete,
        50,
        "artifact-set-budget-under",
    );
    if check.annotated_count != 2 || check.omitted_count != 0 {
        return Err(format!(
            "under-budget projection drifted: annotated {} omitted {}",
            check.annotated_count, check.omitted_count
        ));
    }
    Ok(())
}
