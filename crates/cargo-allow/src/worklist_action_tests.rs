use allow_core::FindingKind;

use super::test_support::{test_entry, test_finding};
use super::{proof_commands, suggested_actions};

#[test]
fn suggested_actions_cover_known_worklist_kinds_and_default() {
    let cases = [
        (
            "new_unreceipted_finding",
            "remove the new source exception if it is accidental",
        ),
        (
            "occurrence_limit_exceeded",
            "reduce the current findings back to the baseline count",
        ),
        (
            "expired_allow",
            "remove the expired allow if the exception is gone",
        ),
        (
            "stale_allow",
            "remove the stale allow entry if the exception no longer exists",
        ),
        (
            "ambiguous_selector",
            "narrow selectors so each finding matches exactly one allow entry",
        ),
        (
            "unsafe_missing_evidence",
            "add unsafe-review, test, spec, or boundary evidence for the unsafe exception",
        ),
        (
            "missing_evidence",
            "add evidence that supports the exception reason",
        ),
        (
            "missing_required_field",
            "fill the required owner, reason, classification, lifecycle, or evidence field",
        ),
        (
            "invalid_selector",
            "replace line-only or invalid selector data with structural identity",
        ),
        (
            "baseline_debt",
            "replace generated baseline debt with a reviewed allow entry",
        ),
        (
            "review_due",
            "review the retained exception and update evidence or remove it",
        ),
        (
            "broad_scope",
            "replace the broad glob with exact paths or a narrower glob where practical",
        ),
        (
            "broken_evidence_link",
            "restore or commit the referenced local evidence artifact",
        ),
        (
            "weak_evidence_reference",
            "replace the weak evidence string with a typed evidence reference",
        ),
        (
            "future_kind",
            "inspect the outcome and update policy or source accordingly",
        ),
    ];

    for (kind, expected_first_action) in cases {
        let actions = suggested_actions(kind);
        assert_eq!(
            actions.first().map(String::as_str),
            Some(expected_first_action)
        );
        assert!(
            actions.iter().all(|action| !action.is_empty()),
            "{kind} should not emit empty suggested actions"
        );
    }
}

#[test]
fn proof_commands_use_finding_kind_when_present() {
    let finding = test_finding(
        FindingKind::LintException,
        Some("clippy"),
        "src/lib.rs",
        "attribute_item",
    );
    let mut entry = test_entry("allow-unsafe", FindingKind::Unsafe);
    entry.family = Some("unsafe_fn".to_string());

    assert_eq!(
        proof_commands("new_unreceipted_finding", Some(&finding), Some(&entry)),
        vec![
            "cargo-allow explain allow-unsafe",
            "cargo-allow worklist --allow-id allow-unsafe --format json",
            "cargo-allow check --kind lint-exception --mode no-new",
            "cargo-allow worklist --item-kind new_unreceipted_finding --format json",
            "cargo-allow worklist --kind lint-exception --format json",
        ]
    );
}

#[test]
fn proof_commands_map_entry_only_kinds_and_worklist_shortcuts() {
    let mut entry = test_entry("allow-workflow", FindingKind::PolicyException);
    entry.family = Some("workflow_external_action".to_string());

    assert_eq!(
        proof_commands("broad_scope", None, Some(&entry)),
        vec![
            "cargo-allow explain allow-workflow",
            "cargo-allow worklist --allow-id allow-workflow --format json",
            "cargo-allow check --kind workflow --mode no-new",
            "cargo-allow worklist --broad-scope --format json",
            "cargo-allow worklist --item-kind broad_scope --format json",
            "cargo-allow worklist --kind workflow --format json",
        ]
    );

    entry.id = "allow-dependency".to_string();
    entry.family = Some("dependency_surface".to_string());
    assert_eq!(
        proof_commands("baseline_debt", None, Some(&entry)),
        vec![
            "cargo-allow explain allow-dependency",
            "cargo-allow worklist --allow-id allow-dependency --format json",
            "cargo-allow check --kind dependency-surface --mode no-new",
            "cargo-allow worklist --baseline-debt --format json",
            "cargo-allow worklist --item-kind baseline_debt --format json",
            "cargo-allow worklist --kind dependency-surface --format json",
        ]
    );

    entry.id = "allow-non-rust".to_string();
    entry.kind = FindingKind::NonRustFile;
    entry.family = Some("script".to_string());
    assert_eq!(
        proof_commands("missing_evidence", None, Some(&entry)),
        vec![
            "cargo-allow explain allow-non-rust",
            "cargo-allow worklist --allow-id allow-non-rust --format json",
            "cargo-allow check --kind non-rust --mode no-new",
            "cargo-allow worklist --missing-evidence --format json",
            "cargo-allow worklist --item-kind missing_evidence --format json",
            "cargo-allow worklist --kind non-rust --format json",
        ]
    );
}

#[test]
fn proof_commands_cover_policy_family_aliases_and_unknown_policy_fallback() {
    let cases = [
        ("executable_file", "executable"),
        ("github_workflow", "workflow"),
        ("workflow_external_action", "workflow"),
        ("dependency_surface", "dependency-surface"),
        ("process_spawn", "process"),
        ("network_destination", "network"),
    ];

    for (family, kind_arg) in cases {
        let mut entry = test_entry(&format!("allow-{family}"), FindingKind::PolicyException);
        entry.family = Some(family.to_string());

        assert_eq!(
            proof_commands("review_due", None, Some(&entry)),
            vec![
                format!("cargo-allow explain allow-{family}"),
                format!("cargo-allow worklist --allow-id allow-{family} --format json"),
                format!("cargo-allow check --kind {kind_arg} --mode no-new"),
                "cargo-allow worklist --item-kind review_due --format json".to_string(),
                format!("cargo-allow worklist --kind {kind_arg} --format json"),
            ],
            "{family} should map to --kind {kind_arg}"
        );
    }

    let mut unknown = test_entry("allow-policy", FindingKind::PolicyException);
    unknown.family = Some("unknown_policy_family".to_string());
    assert_eq!(
        proof_commands("baseline_debt", None, Some(&unknown)),
        vec![
            "cargo-allow explain allow-policy",
            "cargo-allow worklist --allow-id allow-policy --format json",
            "cargo-allow check --mode no-new",
            "cargo-allow worklist --baseline-debt --format json",
            "cargo-allow worklist --item-kind baseline_debt --format json",
            "cargo-allow worklist --format json",
        ]
    );

    assert_eq!(
        proof_commands("review_due", None, Some(&unknown)),
        vec![
            "cargo-allow explain allow-policy",
            "cargo-allow worklist --allow-id allow-policy --format json",
            "cargo-allow check --mode no-new",
            "cargo-allow worklist --item-kind review_due --format json",
            "cargo-allow worklist --format json",
        ]
    );
}

#[test]
fn missing_evidence_keeps_shortcut_when_kind_is_unknown() {
    let mut entry = test_entry("allow-policy", FindingKind::PolicyException);
    entry.family = Some("unknown_policy_family".to_string());

    assert_eq!(
        proof_commands("missing_evidence", None, Some(&entry)),
        vec![
            "cargo-allow explain allow-policy",
            "cargo-allow worklist --allow-id allow-policy --format json",
            "cargo-allow check --mode no-new",
            "cargo-allow worklist --missing-evidence --format json",
            "cargo-allow worklist --item-kind missing_evidence --format json",
            "cargo-allow worklist --format json",
        ]
    );
}

#[test]
fn unsafe_missing_evidence_adds_unsafe_check_when_kind_is_unknown() {
    let mut entry = test_entry("allow-policy", FindingKind::PolicyException);
    entry.family = Some("unknown_policy_family".to_string());

    assert_eq!(
        proof_commands("unsafe_missing_evidence", None, Some(&entry)),
        vec![
            "cargo-allow explain allow-policy",
            "cargo-allow worklist --allow-id allow-policy --format json",
            "cargo-allow check --mode no-new",
            "cargo-allow worklist --item-kind unsafe_missing_evidence --format json",
            "cargo-allow worklist --format json",
            "cargo-allow check --kind unsafe --mode no-new",
        ]
    );
}
