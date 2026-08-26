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

fn build_release_qualification() -> CargoAllowPostMergeQualificationV1 {
    CargoAllowPostMergeQualificationV1::new(PostMergeQualificationInitV1 {
        qualification_id: "post-merge-qual-0.2.0".to_string(),
        reviewed: ReviewedContextV1 {
            base_sha: "aaaa1111aaaa1111aaaa1111aaaa1111aaaa1111".to_string(),
            head_sha: "bbbb2222bbbb2222bbbb2222bbbb2222bbbb2222".to_string(),
            merge_base_sha: "aaaa1111aaaa1111aaaa1111aaaa1111aaaa1111".to_string(),
            tree_sha: "cccc3333cccc3333cccc3333cccc3333cccc3333".to_string(),
        },
        merged: MergedStateV1 {
            pr_number: 3988,
            merge_commit_sha: "dddd4444dddd4444dddd4444dddd4444dddd4444".to_string(),
            merge_tree_sha: "cccc3333cccc3333cccc3333cccc3333cccc3333".to_string(),
            current_main_commit_sha: "dddd4444dddd4444dddd4444dddd4444dddd4444".to_string(),
            current_main_tree_sha: "cccc3333cccc3333cccc3333cccc3333cccc3333".to_string(),
            merge_method: MergeMethodV1::Squash,
            merge_parents: vec!["aaaa1111aaaa1111aaaa1111aaaa1111aaaa1111".to_string()],
        },
        changed_files: vec!["crates/allow-report/src/artifacts.rs".to_string()],
        semantic_owners: vec!["artifacts".to_string()],
        premerge_evidence_digest: "sha256:premerge-valid-digest".to_string(),
        preserved_evidence_nodes: vec!["test-suite".to_string(), "package-smoke".to_string()],
        invalidated_evidence_nodes: vec![],
        required_rerun_set: vec![],
        created_at_utc: "2026-08-26T09:00:00Z".to_string(),
    })
}

#[test]
fn test_post_merge_qualification_clean_freeze_path() -> Result<(), io::Error> {
    let qual = build_release_qualification();
    let verdict = qual.evaluate_verdict();

    require(
        verdict == PostMergeEquivalenceVerdictV1::EquivalentTree,
        "clean tree equivalence must yield EquivalentTree verdict",
    )?;
    Ok(())
}

#[test]
fn test_post_merge_qualification_selected_bytes_docs_only() -> Result<(), io::Error> {
    let mut qual = build_release_qualification();
    qual.reviewed.tree_sha = "different-tree-sha".to_string();
    qual.changed_files = vec![
        "README.md".to_string(),
        ".changes/Added-20260826-change.yaml".to_string(),
    ];

    let verdict = qual.evaluate_verdict();

    require(
        verdict == PostMergeEquivalenceVerdictV1::EquivalentSelectedBytes,
        "doc/changes-only differences must evaluate as EquivalentSelectedBytes",
    )?;
    Ok(())
}

#[test]
fn test_post_merge_qualification_divergent_code_movement() -> Result<(), io::Error> {
    let mut qual = build_release_qualification();
    qual.reviewed.tree_sha = "different-tree-sha".to_string();
    qual.changed_files = vec!["crates/allow-core/src/lib.rs".to_string()];

    let verdict = qual.evaluate_verdict();

    require(
        verdict == PostMergeEquivalenceVerdictV1::RequalificationRequired,
        "divergent code movement must require requalification",
    )?;
    Ok(())
}
