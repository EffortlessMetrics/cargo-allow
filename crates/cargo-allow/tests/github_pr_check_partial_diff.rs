//! Partial-diff tests (#3965): missing or shallow bases stay non-clean with
//! `UnknownAttribution`, partial coverage cannot earn ordinary
//! introduced/resolved labels, and only Complete coverage projects clean.

use allow_report::{
    BaseScanCompletenessV1, GitHubPrAnnotationClassV1, GitHubPrCheckResultV1,
    GitHubPrCheckSubjectV1, GitHubPrDiffReportViewV1, GitHubPrDiffViewV1,
    GitHubPrFindingChangeRowViewV1, project_github_pr_check,
};

fn subject() -> GitHubPrCheckSubjectV1 {
    GitHubPrCheckSubjectV1 {
        repository: "EffortlessMetrics/cargo-allow".to_string(),
        pr_number: 4021,
        base: "main".to_string(),
        merge_base: "ffffffffffffffffffffffffffffffffffffffff".to_string(),
        head: "6666666666666666666666666666666666666666".to_string(),
    }
}

fn introduced_row() -> GitHubPrFindingChangeRowViewV1 {
    GitHubPrFindingChangeRowViewV1 {
        change: "new".to_string(),
        movement: "introduced".to_string(),
        posture_delta: "worsened".to_string(),
        changed_in_diff: true,
        key: "panic:partial-coverage".to_string(),
        kind: "panic".to_string(),
        path: "src/partial.rs".to_string(),
        line: Some(2),
    }
}

fn partial_report() -> GitHubPrDiffReportViewV1 {
    GitHubPrDiffReportViewV1 {
        schema_id: "cargo-allow.report.v1".to_string(),
        status: "passed".to_string(),
        failed: false,
        inventory: None,
        diff: Some(GitHubPrDiffViewV1 {
            net_posture: "review-required".to_string(),
            finding_changes: vec![introduced_row()],
        }),
    }
}

#[test]
fn missing_base_projects_unknown_attribution_without_annotations() -> Result<(), String> {
    let check = project_github_pr_check(
        &partial_report(),
        &subject(),
        BaseScanCompletenessV1::MissingBase,
        10,
        "artifact-set-partial",
    );
    if check.result != GitHubPrCheckResultV1::UnknownAttribution {
        return Err(format!("missing base projected {check:?}"));
    }
    // Partial coverage cannot earn ordinary introduced labels or inline
    // annotations with false confidence.
    if check.introduced_count != 0 {
        return Err("missing base still incremented introduced".to_string());
    }
    if !check.annotations.is_empty() {
        return Err("missing base produced inline annotations".to_string());
    }
    if check.unknown_count == 0 {
        return Err("unknown rows were not retained in the counts".to_string());
    }
    Ok(())
}

#[test]
fn partial_base_keeps_rows_unknown_but_visible() -> Result<(), String> {
    for completeness in [
        BaseScanCompletenessV1::BasePartial,
        BaseScanCompletenessV1::HeadPartial,
        BaseScanCompletenessV1::BothPartial,
    ] {
        let check = project_github_pr_check(
            &partial_report(),
            &subject(),
            completeness,
            10,
            "artifact-set-partial",
        );
        if check.result != GitHubPrCheckResultV1::Partial {
            return Err(format!("partial coverage projected {check:?}"));
        }
        if check.unknown_count != 1 {
            return Err(format!(
                "partial coverage did not retain the unknown row: {check:?}"
            ));
        }
    }
    Ok(())
}

#[test]
fn removed_rows_under_partial_coverage_stay_unknown() -> Result<(), String> {
    let mut partial_removed = partial_report();
    partial_removed
        .diff
        .as_mut()
        .ok_or_else(|| "fixture lost its diff".to_string())?
        .finding_changes = vec![GitHubPrFindingChangeRowViewV1 {
        change: "removed".to_string(),
        movement: "removed".to_string(),
        posture_delta: "improved".to_string(),
        changed_in_diff: true,
        key: "panic:removed-under-partial".to_string(),
        kind: "panic".to_string(),
        path: "src/gone.rs".to_string(),
        line: Some(9),
    }];
    let check = project_github_pr_check(
        &partial_removed,
        &subject(),
        BaseScanCompletenessV1::HeadPartial,
        10,
        "artifact-set-partial-removed",
    );
    if check.result != GitHubPrCheckResultV1::Partial {
        return Err(format!("partial removed projected {check:?}"));
    }
    if check.resolved_count != 0 || check.unknown_count != 1 {
        return Err(format!(
            "removed row under partial coverage earned an ordinary resolved label: {check:?}"
        ));
    }
    Ok(())
}

#[test]
fn complete_coverage_is_the_only_clean_projection() -> Result<(), String> {
    let check = project_github_pr_check(
        &partial_report(),
        &subject(),
        BaseScanCompletenessV1::Complete,
        10,
        "artifact-set-partial",
    );
    if check.result != GitHubPrCheckResultV1::FindingsBlocking {
        return Err(format!("complete coverage projected {check:?}"));
    }
    let annotated = check
        .annotations
        .first()
        .ok_or_else(|| "complete coverage lost its annotation".to_string())?;
    if annotated.classification != GitHubPrAnnotationClassV1::Introduced {
        return Err("complete coverage lost the introduced class".to_string());
    }
    Ok(())
}
