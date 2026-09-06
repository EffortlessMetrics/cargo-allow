//! Repository release-controls surface for the #3844 review-readiness
//! check: the stable context name, the minimal permission posture, the
//! event coverage, and the no-mutation/no-self-requirement law are all
//! checked against the source-controlled workflow and adapter.

use allow_report::REVIEW_READINESS_CHECK_CONTEXT;

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
        .expect("the release-controls surface is retained in the tree")
}

#[test]
fn repository_release_controls_publish_one_stable_check_context() {
    // #2283 names the check; the context string is the contract.
    assert_eq!(REVIEW_READINESS_CHECK_CONTEXT, "review-readiness");

    let root = workspace_root();
    let workflow = read_workspace_file(&root, ".github/workflows/review-readiness.yml");
    assert!(
        workflow.contains("  review-readiness:"),
        "the job id is the visible check context"
    );
    assert!(
        workflow.contains("name: Review readiness"),
        "the workflow is named for the readiness lane"
    );
}

#[test]
fn repository_release_controls_cover_every_readiness_relevant_event() {
    let root = workspace_root();
    let workflow = read_workspace_file(&root, ".github/workflows/review-readiness.yml");
    for event in [
        "opened",
        "reopened",
        "synchronize",
        "ready_for_review",
        "converted_to_draft",
    ] {
        assert!(
            workflow.contains(event),
            "the readiness workflow must trigger on {event}"
        );
    }
    assert!(
        !workflow.contains("push:"),
        "the readiness check is pull-request scoped"
    );
}

#[test]
fn repository_release_controls_use_minimum_permissions() {
    let root = workspace_root();
    let workflow = read_workspace_file(&root, ".github/workflows/review-readiness.yml");
    let permissions_block = workflow
        .split("permissions:")
        .nth(1)
        .and_then(|rest| rest.split("jobs:").next())
        .expect("a permissions block before jobs");
    assert!(
        permissions_block.contains("contents: read"),
        "contents stays read-only: {permissions_block}"
    );
    assert!(
        permissions_block.contains("pull-requests: read"),
        "pull-requests stays read-only: {permissions_block}"
    );
    for forbidden in [
        "issues:",
        "checks:",
        "contents: write",
        "pull-requests: write",
    ] {
        assert!(
            !permissions_block.contains(forbidden),
            "the readiness workflow must not request '{forbidden}'"
        );
    }
}

#[test]
fn repository_release_controls_never_mutate_or_self_require() {
    let root = workspace_root();
    let script = read_workspace_file(&root, "scripts/project-review-readiness.sh");
    for mutation in [
        "gh api",
        "gh pr merge",
        "gh pr ready",
        "gh pr edit",
        "gh pr close",
        "gh pr update-branch",
        "gh release",
        "ruleset",
    ] {
        assert!(
            !script.contains(mutation),
            "the readiness adapter must not mutate state via '{mutation}'"
        );
    }
    assert!(
        script.contains("review-readiness project"),
        "the adapter runs the typed projection"
    );
    // The workflow cannot make itself a required check: it has no
    // check-run write surface (permissions stay read-only) and never
    // calls the check-runs API to register or update contexts; live
    // required-context configuration is #2284's alone.
    let workflow = read_workspace_file(&root, ".github/workflows/review-readiness.yml");
    assert!(
        !workflow.contains("check-runs") && !workflow.contains("check_runs"),
        "a source workflow cannot register or update its own check context"
    );
}

#[test]
fn repository_release_controls_bind_the_disposition_location() {
    let root = workspace_root();
    let script = read_workspace_file(&root, "scripts/project-review-readiness.sh");
    assert!(
        script.contains(".allow/review-dispositions"),
        "the checked adapter reads retained dispositions from the declared location"
    );
    assert!(
        script.contains("sha256sum"),
        "the adapter binds the diff digest with the documented recipe"
    );
    assert!(
        script.contains("git merge-base"),
        "the adapter binds the effective merge base"
    );
}
