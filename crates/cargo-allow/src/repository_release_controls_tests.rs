//! Retained review projection semantics and the #4602 queue-only transport.
//! A source change does not activate the H publisher/control join; ordinary
//! development events no longer allocate an Actions wrapper.

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
fn repository_release_controls_retain_the_typed_context_without_an_actions_wrapper() {
    assert_eq!(REVIEW_READINESS_CHECK_CONTEXT, "review-readiness");
    let root = workspace_root();
    let script = read_workspace_file(&root, "scripts/project-review-readiness.sh");
    assert!(script.contains(concat!("CHECK_NAME=", "\"review-readiness\"")));
    let workflow = read_workspace_file(&root, ".github/workflows/review-readiness.yml");
    assert!(
        workflow.contains("  merge_group:"),
        "the required review-readiness context must still publish for the queue"
    );
    assert!(
        !workflow.contains("  pull_request:"),
        "ordinary pull_request events must not allocate the review-readiness wrapper"
    );
}

#[test]
fn repository_release_controls_allocate_ordinary_work_only_for_merge_groups() {
    let root = workspace_root();
    for path in [
        ".github/workflows/ci.yml",
        ".github/workflows/sentinel.yml",
        ".github/workflows/frozen-subject-lock.yml",
        ".github/workflows/ub-review.yml",
    ] {
        let workflow = read_workspace_file(&root, path);
        assert!(workflow.contains("  merge_group:"), "{path}");
        assert!(workflow.contains("types: [checks_requested]"), "{path}");
        assert!(!workflow.contains("  pull_request:"), "{path}");
        assert!(!workflow.contains("  pull_request_target:"), "{path}");
        // ci.yml keeps the main push proof; the other queue roots stay
        // merge_group-only.
        if path != ".github/workflows/ci.yml" {
            assert!(!workflow.contains("  push:"), "{path}");
        }
    }
}

#[test]
fn repository_release_controls_keep_queue_execution_read_only() {
    let root = workspace_root();
    for path in [
        ".github/workflows/ci.yml",
        ".github/workflows/sentinel.yml",
        ".github/workflows/frozen-subject-lock.yml",
        ".github/workflows/ub-review.yml",
    ] {
        let workflow = read_workspace_file(&root, path);
        assert!(workflow.contains("  contents: read"), "{path}");
        for forbidden in [
            "checks: write",
            "contents: write",
            "pull-requests: write",
            "secrets.",
        ] {
            assert!(!workflow.contains(forbidden), "{path}: {forbidden}");
        }
    }
}

fn validate_readiness_api_calls(script: &str) -> Result<(), String> {
    let mut found_check_run = false;
    for call in script
        .lines()
        .filter(|line| line.contains("gh api"))
        .map(|line| {
            line.split_once("gh api ")
                .map_or(line, |(_, call)| call)
                .trim()
        })
    {
        // Exact retained command-start shapes, not a general shell parser.
        // In particular, ref acquisition has no continuation or flags that
        // could override the default GET or implicitly select POST.
        match call {
            r#""${API}/git/ref/heads/${encoded}" 2>/dev/null)"; then"# => {}
            // Publication reads the check on the subject SHA: the reviewed
            // head, or the merge-group checkout after that identity is
            // proved. The fail-closed queue path reads GITHUB_SHA directly.
            r#""${API}/commits/${check_sha}/check-runs?check_name=${CHECK_NAME}" \"#
            | r#""${API}/commits/${GITHUB_SHA}/check-runs?check_name=${CHECK_NAME}" \"#
            | r#""${API}/check-runs/${existing_id}" -X PATCH \"#
            | r#""${API}/check-runs" -X POST \"# => found_check_run = true,
            _ => return Err(format!("unexpected readiness API command: {call}")),
        }
    }
    if found_check_run {
        Ok(())
    } else {
        Err("the adapter must retain its check-run API surface".to_owned())
    }
}

#[test]
fn repository_release_controls_never_mutate_or_self_require() -> Result<(), String> {
    let root = workspace_root();
    let script = read_workspace_file(&root, "scripts/project-review-readiness.sh");
    // The only GitHub write is publishing the review-readiness check
    // run itself; every other mutation verb is absent.
    for mutation in [
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
    validate_readiness_api_calls(&script)?;
    // Queue source cannot make itself a required context. Live required
    // context configuration and the H publisher join remain #2284/#4602's
    // coordinated activation, not a successful source-test side effect.
    let workflow = read_workspace_file(&root, ".github/workflows/ci.yml");
    assert!(
        !workflow.contains("required_status_check"),
        "a source workflow cannot self-require"
    );
    Ok(())
}

#[test]
fn repository_release_controls_reject_ref_writes_and_unrelated_api_calls() -> Result<(), String> {
    let root = workspace_root();
    let script = read_workspace_file(&root, "scripts/project-review-readiness.sh");
    let read = r#"gh api "${API}/git/ref/heads/${encoded}" 2>/dev/null)"; then"#;
    if script.matches(read).count() != 1 {
        return Err("the ref-read control must replace exactly one real acquisition".to_owned());
    }
    validate_readiness_api_calls(&script)?;
    for replacement in [
        r#"gh api "${API}/git/ref/heads/${encoded}" -X PATCH 2>/dev/null)"; then"#,
        r#"gh api "${API}/git/ref/heads/${encoded}" --method POST 2>/dev/null)"; then"#,
        r#"gh api "${API}/git/ref/heads/${encoded}" -f sha=changed 2>/dev/null)"; then"#,
        r#"gh api "${API}/git/ref/heads/${encoded}" --input payload.json 2>/dev/null)"; then"#,
        "gh api \"${API}/git/ref/heads/${encoded}\" \\\n  -X PATCH 2>/dev/null)\"; then",
        r#"gh api "${API}/git/matching-refs/heads/${encoded}" 2>/dev/null)"; then"#,
        r#"gh api "${API}/git/ref/tags/${encoded}" 2>/dev/null)"; then"#,
        r#"gh api "${API}/issues?label=check-runs" 2>/dev/null)"; then"#,
    ] {
        let changed = script.replace(read, replacement);
        if validate_readiness_api_calls(&changed).is_ok() {
            return Err(format!("API surface guard admitted: {replacement}"));
        }
    }
    for missing_check_run in ["no API calls", read] {
        if validate_readiness_api_calls(missing_check_run).is_ok() {
            return Err("API surface guard admitted a missing check-run surface".to_owned());
        }
    }
    Ok(())
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
    // Ambiguous disposition records fail closed, and the head delta
    // is delivered so the projection can prove the review-ledger
    // bootstrap.
    assert!(
        script.contains("ambiguous retained dispositions"),
        "duplicate dispositions fail closed"
    );
    assert!(
        script.contains("--head-delta-path"),
        "the adapter passes the head delta for the ledger-bootstrap proof"
    );
    // Ancestor-bound records feed the ledger bootstrap; exact-head
    // records take precedence; base-changing edits recompute; the
    // check run is updated in place instead of accumulating stale
    // duplicates; and enumeration failures fail the run.
    assert!(
        script.contains("git merge-base --is-ancestor"),
        "ancestor-bound records feed the ledger bootstrap"
    );
    assert!(
        script.contains("check-runs/${existing_id}") && script.contains("-X PATCH"),
        "the authoritative check run is updated in place"
    );
    assert!(
        script.contains("PR_BASE_CHANGED"),
        "base-changing edits map to the base-moved event"
    );
    assert!(
        script.contains(r#"open_prs="$(gh pr list"#),
        "enumeration failure must fail the run instead of looking empty"
    );
    // An unreadable retained record is review evidence that fails
    // closed; it never degrades to a missing review.
    assert!(
        script.contains("unreadable retained disposition records"),
        "malformed ledger records fail closed"
    );
}
