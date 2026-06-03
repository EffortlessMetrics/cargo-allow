use crate::{DiffFindingChange, DiffPolicyChange, DiffPostureSummary};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffNetPosture {
    Worse,
    ReviewRequired,
    Improved,
    Unchanged,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct DiffEvidenceDeltaSummary {
    pub(crate) evidence_added: usize,
    pub(crate) evidence_removed: usize,
    pub(crate) link_added: usize,
    pub(crate) link_removed: usize,
}

impl DiffNetPosture {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Worse => "worse",
            Self::ReviewRequired => "review-required",
            Self::Improved => "improved",
            Self::Unchanged => "unchanged",
        }
    }

    pub fn reviewer_action(self) -> &'static str {
        match self {
            Self::Worse => {
                "block until failing source exception changes are fixed, narrowed, or receipted."
            }
            Self::ReviewRequired => "review the source exception posture change before merging.",
            Self::Improved => "verify the cleanup was intentional and keep the narrower posture.",
            Self::Unchanged => "no source exception posture change detected.",
        }
    }
}

pub(crate) fn diff_evidence_delta_summary(
    policy_changes: &[DiffPolicyChange<'_>],
) -> DiffEvidenceDeltaSummary {
    let mut summary = DiffEvidenceDeltaSummary::default();
    for change in policy_changes {
        match change.kind {
            "evidence_added" => summary.evidence_added += 1,
            "evidence_removed" => summary.evidence_removed += 1,
            "link_added" => summary.link_added += 1,
            "link_removed" => summary.link_removed += 1,
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
    DiffPostureSummary {
        current_failures,
        new_findings: finding_changes
            .iter()
            .filter(|change| change.change == "new")
            .count(),
        removed_findings: finding_changes
            .iter()
            .filter(|change| change.change == "removed")
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
