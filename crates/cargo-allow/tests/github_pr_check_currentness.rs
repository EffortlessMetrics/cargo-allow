//! Currentness tests (#3965): each exact head receives its own current
//! result, reruns for the same subject are idempotent, and stale or unknown
//! generations stay non-clean.

use allow_report::{
    BaseScanCompletenessV1, GitHubPrCheckResultV1, GitHubPrCheckSubjectV1,
    GitHubPrDiffReportViewV1, GitHubPrDiffViewV1, GitHubPrFindingChangeRowViewV1,
    project_github_pr_check,
};

fn subject(head: &str) -> GitHubPrCheckSubjectV1 {
    GitHubPrCheckSubjectV1 {
        repository: "EffortlessMetrics/cargo-allow".to_string(),
        pr_number: 4021,
        base: "main".to_string(),
        merge_base: "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_string(),
        head: head.to_string(),
    }
}

fn clean_report() -> GitHubPrDiffReportViewV1 {
    GitHubPrDiffReportViewV1 {
        schema_id: "cargo-allow.report.v1".to_string(),
        status: "passed".to_string(),
        failed: false,
        inventory: None,
        diff: Some(GitHubPrDiffViewV1 {
            net_posture: "unchanged".to_string(),
            finding_changes: Vec::new(),
        }),
    }
}

fn introduced_report() -> GitHubPrDiffReportViewV1 {
    GitHubPrDiffReportViewV1 {
        schema_id: "cargo-allow.report.v1".to_string(),
        status: "passed".to_string(),
        failed: false,
        inventory: None,
        diff: Some(GitHubPrDiffViewV1 {
            net_posture: "worse".to_string(),
            finding_changes: vec![GitHubPrFindingChangeRowViewV1 {
                change: "new".to_string(),
                movement: "introduced".to_string(),
                posture_delta: "worsened".to_string(),
                changed_in_diff: true,
                key: "panic:stale-check".to_string(),
                kind: "panic".to_string(),
                path: "src/moved.rs".to_string(),
                line: Some(4),
            }],
        }),
    }
}

#[test]
fn head_movement_binds_a_new_current_subject() -> Result<(), String> {
    let old_head = "1111111111111111111111111111111111111111";
    let new_head = "2222222222222222222222222222222222222222";
    let old_check = project_github_pr_check(
        &introduced_report(),
        &subject(old_head),
        BaseScanCompletenessV1::Complete,
        10,
        "artifact-set-currentness",
    );
    let new_check = project_github_pr_check(
        &introduced_report(),
        &subject(new_head),
        BaseScanCompletenessV1::Complete,
        10,
        "artifact-set-currentness",
    );
    if old_check.subject.head != old_head || new_check.subject.head != new_head {
        return Err("subject head binding drifted".to_string());
    }
    if old_check == new_check {
        return Err("different heads projected one identical check".to_string());
    }
    Ok(())
}

#[test]
fn rerun_for_the_same_exact_subject_is_idempotent() -> Result<(), String> {
    let first = project_github_pr_check(
        &introduced_report(),
        &subject("3333333333333333333333333333333333333333"),
        BaseScanCompletenessV1::Complete,
        10,
        "artifact-set-currentness",
    );
    let second = project_github_pr_check(
        &introduced_report(),
        &subject("3333333333333333333333333333333333333333"),
        BaseScanCompletenessV1::Complete,
        10,
        "artifact-set-currentness",
    );
    if first != second {
        return Err("same-subject rerun was not idempotent".to_string());
    }
    Ok(())
}

#[test]
fn unsupported_generations_and_missing_diff_stay_non_clean() -> Result<(), String> {
    let mut stale = clean_report();
    stale.schema_id = "cargo-allow.report.v0".to_string();
    let check = project_github_pr_check(
        &stale,
        &subject("4444444444444444444444444444444444444444"),
        BaseScanCompletenessV1::Complete,
        10,
        "artifact-set-currentness",
    );
    if check.result != GitHubPrCheckResultV1::Unsupported {
        return Err(format!("stale generation was not Unsupported: {check:?}"));
    }

    let mut empty = clean_report();
    empty.diff = None;
    let check = project_github_pr_check(
        &empty,
        &subject("4444444444444444444444444444444444444444"),
        BaseScanCompletenessV1::Complete,
        10,
        "artifact-set-currentness",
    );
    if check.result == GitHubPrCheckResultV1::Passed {
        return Err("missing diff evaluation projected Passed".to_string());
    }
    Ok(())
}
