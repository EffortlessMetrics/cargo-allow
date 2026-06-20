use crate::{DiffFindingChange, DiffPolicyChange, DiffPostureSummary};
use allow_core::NetPosture;

/// Aggregate PR diff net posture for summary surfaces.
pub type DiffNetPosture = NetPosture;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct DiffEvidenceDeltaSummary {
    pub(crate) evidence_added: usize,
    pub(crate) weak_evidence_added: usize,
    pub(crate) broken_evidence_added: usize,
    pub(crate) evidence_removed: usize,
    pub(crate) evidence_removal_failures: usize,
    pub(crate) evidence_removal_review_items: usize,
    pub(crate) evidence_removal_improvements: usize,
    pub(crate) link_added: usize,
    pub(crate) weak_link_added: usize,
    pub(crate) broken_link_added: usize,
    pub(crate) link_removed: usize,
    pub(crate) link_removal_failures: usize,
    pub(crate) link_removal_review_items: usize,
    pub(crate) link_removal_improvements: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct DiffStructuralDeltaSummary {
    pub(crate) scope_broadened: usize,
    pub(crate) scope_changed: usize,
    pub(crate) scope_narrowed: usize,
    pub(crate) selector_changed: usize,
    pub(crate) selector_precision_decreased: usize,
    pub(crate) selector_precision_increased: usize,
}

pub(crate) fn diff_structural_delta_summary(
    policy_changes: &[DiffPolicyChange<'_>],
) -> DiffStructuralDeltaSummary {
    let mut summary = DiffStructuralDeltaSummary::default();
    for change in policy_changes {
        match change.kind {
            "scope_broadened" => summary.scope_broadened += 1,
            "scope_changed" => summary.scope_changed += 1,
            "scope_narrowed" => summary.scope_narrowed += 1,
            "selector_changed" => summary.selector_changed += 1,
            "selector_precision_decreased" => summary.selector_precision_decreased += 1,
            "selector_precision_increased" => summary.selector_precision_increased += 1,
            _ => {}
        }
    }
    summary
}

pub(crate) fn diff_evidence_delta_summary(
    policy_changes: &[DiffPolicyChange<'_>],
) -> DiffEvidenceDeltaSummary {
    let mut summary = DiffEvidenceDeltaSummary::default();
    for change in policy_changes {
        match change.kind {
            "evidence_added" => {
                summary.evidence_added += 1;
                match change.severity {
                    "review" => summary.weak_evidence_added += 1,
                    "fail" => summary.broken_evidence_added += 1,
                    _ => {}
                }
            }
            "evidence_removed" => {
                summary.evidence_removed += 1;
                match change.severity {
                    "fail" => summary.evidence_removal_failures += 1,
                    "review" => summary.evidence_removal_review_items += 1,
                    "improvement" => summary.evidence_removal_improvements += 1,
                    _ => {}
                }
            }
            "link_added" => {
                summary.link_added += 1;
                match change.severity {
                    "review" => summary.weak_link_added += 1,
                    "fail" => summary.broken_link_added += 1,
                    _ => {}
                }
            }
            "link_removed" => {
                summary.link_removed += 1;
                match change.severity {
                    "fail" => summary.link_removal_failures += 1,
                    "review" => summary.link_removal_review_items += 1,
                    "improvement" => summary.link_removal_improvements += 1,
                    _ => {}
                }
            }
            _ => {}
        }
    }
    summary
}

pub fn diff_posture_summary(
    current_failures: usize,
    finding_changes: &[DiffFindingChange<'_>],
    policy_changes: &[DiffPolicyChange<'_>],
) -> DiffPostureSummary {
    use allow_core::PresenceMovement;

    DiffPostureSummary {
        current_failures,
        new_findings: finding_changes
            .iter()
            .filter(|change| change.change == PresenceMovement::Introduced.finding_change_label())
            .count(),
        removed_findings: finding_changes
            .iter()
            .filter(|change| change.change == PresenceMovement::Removed.finding_change_label())
            .count(),
        policy_failures: policy_changes
            .iter()
            .filter(|change| change.severity == "fail")
            .count(),
        policy_review_items: policy_changes
            .iter()
            .filter(|change| change.severity == "review")
            .count(),
        policy_improvements: policy_changes
            .iter()
            .filter(|change| change.severity == "improvement")
            .count(),
    }
}

pub fn diff_net_posture(summary: DiffPostureSummary) -> DiffNetPosture {
    if summary.current_failures > 0 || summary.policy_failures > 0 {
        return DiffNetPosture::Worse;
    }
    if summary.new_findings > 0 || summary.policy_review_items > 0 {
        return DiffNetPosture::ReviewRequired;
    }
    if summary.removed_findings > 0 || summary.policy_improvements > 0 {
        return DiffNetPosture::Improved;
    }
    DiffNetPosture::Unchanged
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{NetPosture, PresenceMovement};

    fn policy_change<'a>(severity: &'a str, kind: &'a str) -> DiffPolicyChange<'a> {
        DiffPolicyChange {
            severity,
            movement: "retained",
            posture_delta: match severity {
                "fail" => "worsened",
                "review" => "review_required",
                _ => "improved",
            },
            changed_in_diff: true,
            subject: Some("allow-test"),
            allow_id: "allow-test",
            ledger_id: None,
            lane: None,
            kind,
            message: "policy changed",
            exception_identity: None,
            selector_identity: None,
            selector_precision: None,
            scope: None,
            occurrence_limit: None,
            lifecycle: None,
            evidence: None,
            metadata: None,
            requirement: None,
            policy_status: None,
        }
    }

    fn finding_change<'a>(change: &'a str) -> DiffFindingChange<'a> {
        DiffFindingChange {
            change,
            movement: match change {
                "new" => "introduced",
                _ => "removed",
            },
            posture_delta: match change {
                "new" => "review_required",
                _ => "improved",
            },
            changed_in_diff: true,
            subject: None,
            allow_id: None,
            ledger_id: None,
            lane: None,
            key: "panic|unwrap|src/lib.rs",
            kind: "panic",
            family: Some("unwrap"),
            path: "src/lib.rs",
            line: Some(1),
            column: Some(1),
            source_package: Some("allow-report"),
            identity: None,
        }
    }

    fn summary(
        current_failures: usize,
        new_findings: usize,
        removed_findings: usize,
        policy_failures: usize,
        policy_review_items: usize,
        policy_improvements: usize,
    ) -> DiffPostureSummary {
        DiffPostureSummary {
            current_failures,
            new_findings,
            removed_findings,
            policy_failures,
            policy_review_items,
            policy_improvements,
        }
    }

    #[test]
    fn net_posture_strings_and_reviewer_actions_cover_all_variants() {
        let cases = [
            (
                NetPosture::Worse,
                "worse",
                "block until failing source exception changes are fixed, narrowed, or receipted.",
            ),
            (
                NetPosture::ReviewRequired,
                "review-required",
                "review the source exception posture change before merging.",
            ),
            (
                NetPosture::Improved,
                "improved",
                "verify the cleanup was intentional and keep the narrower posture.",
            ),
            (
                NetPosture::Unchanged,
                "unchanged",
                "no source exception posture change detected.",
            ),
        ];

        for (posture, as_str, action) in cases {
            assert_eq!(posture.as_str(), as_str);
            assert_eq!(posture.net_posture_label(), as_str);
            assert_eq!(posture.reviewer_action(), action);
        }
    }

    #[test]
    fn structural_delta_summary_counts_known_kinds_and_ignores_unknowns() {
        let changes = [
            policy_change("fail", "scope_broadened"),
            policy_change("review", "scope_broadened"),
            policy_change("review", "scope_changed"),
            policy_change("improvement", "scope_narrowed"),
            policy_change("review", "selector_changed"),
            policy_change("fail", "selector_precision_decreased"),
            policy_change("improvement", "selector_precision_increased"),
            policy_change("review", "evidence_added"),
        ];

        assert_eq!(
            diff_structural_delta_summary(&changes),
            DiffStructuralDeltaSummary {
                scope_broadened: 2,
                scope_changed: 1,
                scope_narrowed: 1,
                selector_changed: 1,
                selector_precision_decreased: 1,
                selector_precision_increased: 1,
            }
        );
    }

    #[test]
    fn evidence_delta_summary_counts_severity_buckets_for_evidence_and_links() {
        let changes = [
            policy_change("review", "evidence_added"),
            policy_change("fail", "evidence_added"),
            policy_change("improvement", "evidence_added"),
            policy_change("fail", "evidence_removed"),
            policy_change("review", "evidence_removed"),
            policy_change("improvement", "evidence_removed"),
            policy_change("review", "link_added"),
            policy_change("fail", "link_added"),
            policy_change("improvement", "link_added"),
            policy_change("fail", "link_removed"),
            policy_change("review", "link_removed"),
            policy_change("improvement", "link_removed"),
            policy_change("review", "scope_changed"),
        ];

        assert_eq!(
            diff_evidence_delta_summary(&changes),
            DiffEvidenceDeltaSummary {
                evidence_added: 3,
                weak_evidence_added: 1,
                broken_evidence_added: 1,
                evidence_removed: 3,
                evidence_removal_failures: 1,
                evidence_removal_review_items: 1,
                evidence_removal_improvements: 1,
                link_added: 3,
                weak_link_added: 1,
                broken_link_added: 1,
                link_removed: 3,
                link_removal_failures: 1,
                link_removal_review_items: 1,
                link_removal_improvements: 1,
            }
        );
    }

    #[test]
    fn posture_summary_counts_finding_and_policy_statuses() {
        let finding_changes = [
            finding_change(PresenceMovement::Introduced.finding_change_label()),
            finding_change(PresenceMovement::Introduced.finding_change_label()),
            finding_change(PresenceMovement::Removed.finding_change_label()),
            finding_change("unchanged"),
        ];
        let policy_changes = [
            policy_change("fail", "scope_broadened"),
            policy_change("review", "selector_changed"),
            policy_change("improvement", "scope_narrowed"),
            policy_change("info", "metadata_changed"),
        ];

        assert_eq!(
            diff_posture_summary(7, &finding_changes, &policy_changes),
            DiffPostureSummary {
                current_failures: 7,
                new_findings: 2,
                removed_findings: 1,
                policy_failures: 1,
                policy_review_items: 1,
                policy_improvements: 1,
            }
        );
    }

    #[test]
    fn net_posture_prioritizes_failures_then_review_then_improvement() {
        let cases = [
            (summary(1, 0, 0, 0, 0, 0), DiffNetPosture::Worse),
            (summary(0, 0, 0, 1, 1, 1), DiffNetPosture::Worse),
            (summary(0, 1, 1, 0, 0, 1), DiffNetPosture::ReviewRequired),
            (summary(0, 0, 1, 0, 1, 1), DiffNetPosture::ReviewRequired),
            (summary(0, 0, 1, 0, 0, 0), DiffNetPosture::Improved),
            (summary(0, 0, 0, 0, 0, 1), DiffNetPosture::Improved),
            (summary(0, 0, 0, 0, 0, 0), DiffNetPosture::Unchanged),
        ];

        for (summary, expected) in cases {
            assert_eq!(diff_net_posture(summary), expected);
        }
    }
}
