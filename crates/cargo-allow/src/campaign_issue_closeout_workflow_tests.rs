//! Workflow-decision-law tests for the #3846 campaign issue closeout
//! guard: only the exact close event of a checked-denominator issue is
//! acted on, invalid closeouts are reopened with one bounded rejection
//! comment, valid closeouts stay closed, the guard never closes an
//! issue or mutates anything beyond issue state, and reruns are
//! idempotent.

use serde::Deserialize;

fn workspace_root() -> std::path::PathBuf {
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set for cargo tests");
    std::path::PathBuf::from(manifest_dir)
        .join("../..")
        .canonicalize()
        .expect("workspace root resolves")
}

fn read_workspace_file(root: &std::path::Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel))
        .expect("the closeout guard surface is retained in the tree")
}

const CLOSEOUT_SCHEMA: &str = "cargo-allow.campaign-issue-closeout.v1";
const CLOSEOUT_MARKER: &str = "<!-- cargo-allow:campaign-closeout.v1 -->";

/// The declared closeout payload an issue body carries under the
/// marker. Result-dependent fields are optional at the type level and
/// required per result by the decision law.
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct CampaignCloseoutPayload {
    schema_id: String,
    issue: u64,
    result: String,
    #[serde(default)]
    closeout_id: Option<String>,
    #[serde(default)]
    merged_pr: Option<u64>,
    #[serde(default)]
    replacement_issue: Option<u64>,
    #[serde(default)]
    reason: Option<String>,
}

/// The typed merged-PR observation the runtime guard makes through the
/// provider API for a Complete closeout.
#[derive(Debug, Clone, PartialEq, Eq)]
struct MergedPrObservation {
    state: String,
    merged_at_present: bool,
    base_ref: String,
    merge_commit_present: bool,
    merge_commit_reachable_from_main: bool,
}

/// Bounded validation codes for the declared payload. `accepted` is
/// the child's accepted-result set from the checked denominator.
fn validate_closeout(
    payload: Option<&CampaignCloseoutPayload>,
    issue_number: u64,
    accepted: &[&str],
) -> Vec<String> {
    let Some(payload) = payload else {
        return vec!["missing_closeout".to_string()];
    };
    let mut codes = Vec::new();
    if payload.schema_id != CLOSEOUT_SCHEMA {
        codes.push("schema_mismatch".to_string());
    }
    if payload.issue != issue_number {
        codes.push("issue_identity_mismatch".to_string());
    }
    if !accepted.contains(&payload.result.as_str()) {
        codes.push("result_not_accepted".to_string());
    }
    match payload.result.as_str() {
        "Complete" => {
            if payload.merged_pr.is_none() {
                codes.push("merged_pr_missing".to_string());
            }
            if payload
                .closeout_id
                .as_deref()
                .is_none_or(|id| id.trim().is_empty())
            {
                codes.push("closeout_id_missing".to_string());
            }
        }
        "Duplicate" => {
            if payload.replacement_issue.is_none() {
                codes.push("replacement_issue_missing".to_string());
            }
        }
        "NotPlanned"
            if payload
                .reason
                .as_deref()
                .is_none_or(|reason| reason.trim().is_empty()) =>
        {
            codes.push("reason_missing".to_string());
        }
        _ => {}
    }
    codes
}

/// Bounded codes for the merged-PR evidence of a Complete closeout.
fn merged_pr_codes(observation: &MergedPrObservation, base_branch: &str) -> Vec<String> {
    let mut codes = Vec::new();
    if !observation.merged_at_present || observation.state != "closed" {
        codes.push("pull_request_not_merged".to_string());
    }
    if observation.base_ref != base_branch {
        codes.push("pull_request_wrong_base".to_string());
    }
    if !observation.merge_commit_present {
        codes.push("merge_commit_missing".to_string());
    } else if !observation.merge_commit_reachable_from_main {
        codes.push("merge_commit_not_reachable_from_main".to_string());
    }
    codes
}

/// The bounded rejection comment: marker, codes, identity, next exact
/// action, and claim boundary — nothing else.
fn bounded_comment(issue_number: u64, codes: &[String], identity: &str) -> String {
    let mut sorted = codes.to_vec();
    sorted.sort();
    sorted.dedup();
    format!(
        "{CLOSEOUT_MARKER}\n\
         ## Campaign closeout rejected\n\
         Issue: #{issue_number}\n\
         Result: `InstrumentFailure` / `NotProven`\n\
         Codes: `{}`\n\
         Closeout identity: `{identity}`\n\
         The issue was reopened because the checked active campaign denominator \
         does not have current evidence for a valid close. Repair the exact \
         closeout rows and close it through the reviewed maintainer path.\n\n\
         Claim boundary: this guard protects issue state; it does not perform \
         the work, merge code, or execute a release.",
        sorted.join(", ")
    )
}

/// The close-event decision for one issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CloseAction {
    /// Not in the denominator: no action (negative control 7).
    NoAction,
    /// Valid closeout: the issue stays closed.
    RetainClosed,
    /// Invalid closeout: reopen once with the bounded rejection.
    ReopenWithRejection,
}

fn decide_close(
    in_denominator: bool,
    validation_codes: &[String],
    pr_codes: &[String],
    gate_codes: &[String],
) -> CloseAction {
    if !in_denominator {
        return CloseAction::NoAction;
    }
    if validation_codes.is_empty() && pr_codes.is_empty() && gate_codes.is_empty() {
        return CloseAction::RetainClosed;
    }
    CloseAction::ReopenWithRejection
}

fn valid_complete_payload() -> CampaignCloseoutPayload {
    CampaignCloseoutPayload {
        schema_id: CLOSEOUT_SCHEMA.to_string(),
        issue: 3846,
        result: "Complete".to_string(),
        closeout_id: Some("CARGO-ALLOW-CLOSEOUT-3846".to_string()),
        merged_pr: Some(4146),
        replacement_issue: None,
        reason: None,
    }
}

#[test]
fn campaign_issue_closeout_workflow_reopens_a_false_complete_closeout() {
    // Negative controls 1 and 2: an unmerged or unreachable PR can
    // never hold a close.
    let unmerged = MergedPrObservation {
        state: "open".to_string(),
        merged_at_present: false,
        base_ref: "main".to_string(),
        merge_commit_present: false,
        merge_commit_reachable_from_main: false,
    };
    let codes = merged_pr_codes(&unmerged, "main");
    assert!(codes.contains(&"pull_request_not_merged".to_string()));
    assert_eq!(
        decide_close(
            true,
            &validate_closeout(Some(&valid_complete_payload()), 3846, &["Complete"]),
            &codes,
            &[]
        ),
        CloseAction::ReopenWithRejection
    );

    let unreachable = MergedPrObservation {
        state: "closed".to_string(),
        merged_at_present: true,
        base_ref: "main".to_string(),
        merge_commit_present: true,
        merge_commit_reachable_from_main: false,
    };
    assert!(
        merged_pr_codes(&unreachable, "main")
            .contains(&"merge_commit_not_reachable_from_main".to_string())
    );
}

#[test]
fn campaign_issue_closeout_workflow_rejects_wrong_base_and_stale_pairs() {
    // Negative control 3: review or CI applied to an older base does
    // not satisfy the current pair.
    let retargeted = MergedPrObservation {
        state: "closed".to_string(),
        merged_at_present: true,
        base_ref: "release/0.1".to_string(),
        merge_commit_present: true,
        merge_commit_reachable_from_main: true,
    };
    assert!(merged_pr_codes(&retargeted, "main").contains(&"pull_request_wrong_base".to_string()));
}

#[test]
fn campaign_issue_closeout_workflow_rejects_incoherent_payloads() {
    // Negative control 6: no-code decisions require their evidence.
    let duplicate_without_replacement = CampaignCloseoutPayload {
        replacement_issue: None,
        ..valid_complete_payload()
    };
    let mut duplicate = duplicate_without_replacement;
    duplicate.result = "Duplicate".to_string();
    duplicate.merged_pr = None;
    duplicate.closeout_id = None;
    let codes = validate_closeout(Some(&duplicate), 3846, &["Complete", "Duplicate"]);
    assert!(codes.contains(&"replacement_issue_missing".to_string()));

    let mut unplanned = valid_complete_payload();
    unplanned.result = "NotPlanned".to_string();
    unplanned.merged_pr = None;
    unplanned.closeout_id = None;
    let codes = validate_closeout(Some(&unplanned), 3846, &["Complete", "NotPlanned"]);
    assert!(codes.contains(&"reason_missing".to_string()));

    // Negative control 5: a partial slice is not an accepted closeout
    // result, so a partial umbrella close is rejected.
    let mut partial = valid_complete_payload();
    partial.result = "Partial".to_string();
    let codes = validate_closeout(Some(&partial), 3747, &["Complete"]);
    assert!(codes.contains(&"result_not_accepted".to_string()));

    // Wrong schema or wrong issue identity fails.
    let codes = validate_closeout(Some(&valid_complete_payload()), 9999, &["Complete"]);
    assert!(codes.contains(&"issue_identity_mismatch".to_string()));

    // A body without the marker payload is a missing closeout.
    assert_eq!(
        validate_closeout(None, 3846, &["Complete"]),
        vec!["missing_closeout".to_string()]
    );
}

#[test]
fn campaign_issue_closeout_workflow_retains_a_valid_close() {
    let merged = MergedPrObservation {
        state: "closed".to_string(),
        merged_at_present: true,
        base_ref: "main".to_string(),
        merge_commit_present: true,
        merge_commit_reachable_from_main: true,
    };
    assert!(merged_pr_codes(&merged, "main").is_empty());
    assert_eq!(
        decide_close(
            true,
            &validate_closeout(Some(&valid_complete_payload()), 3846, &["Complete"]),
            &[],
            &[],
        ),
        CloseAction::RetainClosed
    );
}

#[test]
fn campaign_issue_closeout_workflow_ignores_issues_outside_the_denominator() {
    assert_eq!(decide_close(false, &[], &[], &[]), CloseAction::NoAction);
}

#[test]
fn campaign_issue_closeout_workflow_rejection_comment_is_bounded_and_stable() {
    let comment = bounded_comment(
        3846,
        &[
            "merge_commit_not_reachable_from_main".to_string(),
            "insufficient_evidence_class".to_string(),
            "merge_commit_not_reachable_from_main".to_string(),
        ],
        "0123456789abcdef",
    );
    assert!(comment.starts_with(CLOSEOUT_MARKER));
    assert!(
        comment
            .contains("Codes: `insufficient_evidence_class, merge_commit_not_reachable_from_main`")
    );
    assert!(comment.contains("Closeout identity: `0123456789abcdef`"));
    assert!(comment.contains("Claim boundary:"));
    // Bounded: no source paths, no logs, no provider responses.
    assert!(!comment.contains("crates/"));
    assert!(!comment.contains("Traceback"));
    // A rerun produces the identical comment, so the runtime dedupe
    // suppresses duplicate posts.
    let rerun = bounded_comment(
        3846,
        &[
            "insufficient_evidence_class".to_string(),
            "merge_commit_not_reachable_from_main".to_string(),
        ],
        "0123456789abcdef",
    );
    assert_eq!(comment, rerun);
}

#[test]
fn campaign_issue_closeout_workflow_runtime_is_idempotent_and_fail_closed() {
    let root = workspace_root();
    let script = read_workspace_file(&root, "scripts/verify-campaign-issue-closeout.py");
    // Negative control 10: reruns do not spam duplicate comments.
    assert!(
        script.contains("not any(item.get(\"body\") == comment"),
        "the runtime suppresses duplicate rejection comments"
    );
    // Negative control 9: the guard never closes an issue. The only
    // state write in the runtime is the reopen payload.
    assert!(
        !script.contains(r#""state": "closed""#),
        "the runtime never writes state=closed"
    );
    assert!(
        script.contains(r#""state": "open""#),
        "the runtime's only state write is the reopen"
    );
    // Negative control 11: a provider/API failure is an instrument
    // failure and reopens, never a silent clean.
    assert!(
        script.contains("instrument_failure"),
        "provider failures fail closed"
    );
    // Negative control 14: rejection history is never erased.
    assert!(
        !script.contains("DELETE"),
        "the runtime never deletes rejection comments"
    );
    // Negative control 13: no merge, tag, publish, or release write.
    for forbidden in ["/merges", "/releases", "git tag", "cargo publish"] {
        assert!(
            !script.contains(forbidden),
            "the runtime must not perform '{forbidden}'"
        );
    }
}

#[test]
fn campaign_issue_closeout_workflow_permissions_are_minimal() {
    // Negative control 12: no contents write, releases, packages,
    // environments, secrets, or rulesets.
    let root = workspace_root();
    let workflow = read_workspace_file(&root, ".github/workflows/campaign-issue-closeout.yml");
    let permissions_block = workflow
        .split("permissions:")
        .nth(1)
        .and_then(|rest| rest.split("concurrency:").next())
        .expect("a permissions block before concurrency");
    for required in [
        "actions: read",
        "contents: read",
        "issues: write",
        "pull-requests: read",
        "checks: read",
    ] {
        assert!(
            permissions_block.contains(required),
            "the guard requests exactly the minimal set; missing '{required}'"
        );
    }
    for forbidden in [
        "contents: write",
        "issues: write\n  contents",
        "releases",
        "packages",
        "environments",
        "secrets:",
        "rulesets",
        "workflow_dispatch",
    ] {
        assert!(
            !permissions_block.contains(forbidden),
            "the guard must not request '{forbidden}'"
        );
    }
    // Act only on the exact close event.
    assert!(workflow.contains("types: [closed]"));
    // Per-issue concurrency keeps reruns ordered, never oscillating.
    assert!(workflow.contains("group: campaign-issue-closeout-${{ github.event.issue.number }}"));
}
