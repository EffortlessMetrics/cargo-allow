use allow_report::{
    CargoAllowPostMergeQualificationV1, MergeMethodV1, MergedStateV1,
    PostMergeEquivalenceVerdictV1, PostMergeQualificationInitV1, ReviewedContextV1,
};
use std::io;

fn require(condition: bool, message: &str) -> Result<(), io::Error> {
    if !condition {
        return Err(io::Error::other(message));
    }
    Ok(())
}

fn sample_qualification() -> CargoAllowPostMergeQualificationV1 {
    CargoAllowPostMergeQualificationV1::new(PostMergeQualificationInitV1 {
        qualification_id: "qual-3914-001".to_string(),
        reviewed: ReviewedContextV1 {
            base_sha: "base1111base1111base1111base1111base1111".to_string(),
            head_sha: "head2222head2222head2222head2222head2222".to_string(),
            merge_base_sha: "mbase333mbase333mbase333mbase333mbase333".to_string(),
            tree_sha: "tree4444tree4444tree4444tree4444tree4444".to_string(),
        },
        merged: MergedStateV1 {
            pr_number: 3914,
            merge_commit_sha: "merge555merge555merge555merge555merge555".to_string(),
            merge_tree_sha: "tree4444tree4444tree4444tree4444tree4444".to_string(),
            current_main_commit_sha: "merge555merge555merge555merge555merge555".to_string(),
            current_main_tree_sha: "tree4444tree4444tree4444tree4444tree4444".to_string(),
            merge_method: MergeMethodV1::Squash,
            merge_parents: vec!["base1111base1111base1111base1111base1111".to_string()],
        },
        changed_files: vec!["crates/allow-report/src/lib.rs".to_string()],
        semantic_owners: vec!["report".to_string()],
        premerge_evidence_digest: "sha256:premerge-digest-001".to_string(),
        preserved_evidence_nodes: vec!["test-suite".to_string()],
        invalidated_evidence_nodes: vec![],
        required_rerun_set: vec![],
        created_at_utc: "2026-08-26T08:00:00Z".to_string(),
    })
}

#[test]
fn test_post_merge_qualification_equivalent_tree() -> Result<(), io::Error> {
    let qual = sample_qualification();
    let verdict = qual.evaluate_verdict();

    require(
        verdict == PostMergeEquivalenceVerdictV1::EquivalentTree,
        "exact reviewed and merged tree must evaluate as EquivalentTree",
    )?;
    Ok(())
}

#[test]
fn test_post_merge_qualification_stale_main() -> Result<(), io::Error> {
    let mut qual = sample_qualification();
    qual.merged.current_main_commit_sha = "later666later666later666later666later666".to_string();
    qual.merged.current_main_tree_sha = "differ777differ777differ777differ777differ777".to_string();

    let verdict = qual.evaluate_verdict();

    require(
        verdict == PostMergeEquivalenceVerdictV1::Stale,
        "qualification against old main commit must evaluate as Stale",
    )?;
    Ok(())
}

#[test]
fn test_post_merge_qualification_requalification_required() -> Result<(), io::Error> {
    let mut qual = sample_qualification();
    qual.invalidated_evidence_nodes = vec!["ci/review".to_string()];
    qual.required_rerun_set = vec!["test-core-platforms".to_string()];

    let verdict = qual.evaluate_verdict();

    require(
        verdict == PostMergeEquivalenceVerdictV1::RequalificationRequired,
        "invalidated evidence nodes must require requalification",
    )?;
    Ok(())
}

#[test]
fn test_post_merge_qualification_serde_roundtrip() -> Result<(), io::Error> {
    let qual = sample_qualification();
    let json = serde_json::to_string(&qual).map_err(io::Error::other)?;
    let parsed: CargoAllowPostMergeQualificationV1 =
        serde_json::from_str(&json).map_err(io::Error::other)?;

    require(
        parsed == qual,
        "deserialized qualification must match original",
    )?;
    Ok(())
}
