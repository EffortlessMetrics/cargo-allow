use std::collections::BTreeSet;

use super::{WORK_ITEM_KINDS, suggested_actions};

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
            "occurrence_headroom",
            "reduce occurrence_limit to the current matched count",
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
            "matched",
            "inspect the outcome and update policy or source accordingly",
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
            "mirror_divergence",
            "sync mirror ledger from canonical or document intentional drain posture",
        ),
        (
            "future_kind",
            "inspect the outcome and update policy or source accordingly",
        ),
    ];

    let covered = cases
        .iter()
        .map(|(kind, _)| *kind)
        .filter(|kind| *kind != "future_kind")
        .collect::<BTreeSet<_>>();
    let known = WORK_ITEM_KINDS.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(
        covered, known,
        "every known worklist item kind should have deliberate suggested-action coverage"
    );

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
fn weak_evidence_actions_name_canonical_prefix_examples() {
    let actions = suggested_actions("weak_evidence_reference");
    let guidance = actions.join(" ");

    for prefix in allow_policy::canonical_evidence_prefixes().map(|prefix| format!("{prefix}:")) {
        assert!(
            guidance.contains(&prefix),
            "weak evidence guidance should mention {prefix}"
        );
    }
}
