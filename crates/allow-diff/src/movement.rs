use allow_core::{AllowConfig, AllowEntry, LedgerPosture, PostureDelta, PresenceMovement};
use std::collections::{BTreeMap, BTreeSet};

use crate::finding::{FindingPostureChange, FindingPostureKind};
use crate::policy_change::{PolicyChange, PolicyChangeKind, PolicyChangeSeverity};

/// Per-row movement and posture classification for diff artifacts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffRowClassification {
    pub movement: PresenceMovement,
    pub posture_delta: PostureDelta,
    pub changed_in_diff: bool,
}

impl DiffRowClassification {
    pub fn coverage_movement_classification(self) -> &'static str {
        LedgerPosture::new(self.movement, self.posture_delta)
            .coverage_movement_classification(self.changed_in_diff)
    }
}

/// Canonical movement counts for dual-summary blocks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DiffMovementCounts {
    pub introduced: usize,
    pub retained: usize,
    pub removed: usize,
}

/// Canonical posture delta counts for dual-summary blocks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DiffPostureDeltaCounts {
    pub improved: usize,
    pub worsened: usize,
    pub review_required: usize,
    pub unchanged: usize,
}

/// Dual movement/posture summary projected from diff rows and unchanged entries.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DiffLedgerMovementSummary {
    pub movement: DiffMovementCounts,
    pub posture_delta: DiffPostureDeltaCounts,
}

pub fn classify_finding_posture_change(change: &FindingPostureChange) -> DiffRowClassification {
    DiffRowClassification {
        movement: finding_posture_movement(change.kind),
        posture_delta: finding_posture_delta(change.kind),
        changed_in_diff: true,
    }
}

pub fn classify_policy_change(change: &PolicyChange) -> DiffRowClassification {
    DiffRowClassification {
        movement: policy_change_movement(change.kind),
        posture_delta: policy_change_posture_delta(change.severity),
        changed_in_diff: true,
    }
}

pub fn finding_posture_movement(kind: FindingPostureKind) -> PresenceMovement {
    kind.presence_movement()
}

pub fn finding_posture_delta(kind: FindingPostureKind) -> PostureDelta {
    match kind {
        FindingPostureKind::New => PostureDelta::ReviewRequired,
        FindingPostureKind::Removed => PostureDelta::Improved,
    }
}

pub fn policy_change_movement(kind: PolicyChangeKind) -> PresenceMovement {
    match kind {
        PolicyChangeKind::AddedAllow
        | PolicyChangeKind::BaselineDebtAdded
        | PolicyChangeKind::FamilyRuleAdded => PresenceMovement::Introduced,
        PolicyChangeKind::RemovedAllow | PolicyChangeKind::FamilyRuleRemoved => {
            PresenceMovement::Removed
        }
        _ => PresenceMovement::Retained,
    }
}

pub fn policy_change_posture_delta(severity: PolicyChangeSeverity) -> PostureDelta {
    match severity {
        PolicyChangeSeverity::Fail => PostureDelta::Worsened,
        PolicyChangeSeverity::Review => PostureDelta::ReviewRequired,
        PolicyChangeSeverity::Improvement => PostureDelta::Improved,
    }
}

pub fn finding_posture_subject(change: &FindingPostureChange) -> String {
    match change.family.as_deref() {
        Some(family) => format!("{}.{} at {}", change.finding_kind, family, change.path),
        None => format!("{} at {}", change.finding_kind, change.path),
    }
}

pub fn policy_change_subject(change: &PolicyChange) -> String {
    change.allow_id.clone()
}

pub fn entry_ledger_id(cfg: &AllowConfig) -> String {
    cfg.policy.clone()
}

pub fn entry_lane(cfg: &AllowConfig, entry: &AllowEntry) -> String {
    let kind = entry.kind.as_str();
    if cfg.lanes.contains_key(kind) {
        kind.to_string()
    } else {
        "source-exception".to_string()
    }
}

pub fn policy_change_ledger_id(cfg: &AllowConfig) -> String {
    entry_ledger_id(cfg)
}

pub fn policy_change_lane(cfg: &AllowConfig, change: &PolicyChange) -> Option<String> {
    if change.allow_id.starts_with("requirements.")
        || change.allow_id.starts_with("policy.")
        || change.allow_id.starts_with("workspace.")
    {
        return None;
    }
    head_entry_for_allow_id(cfg, &change.allow_id).map(|entry| entry_lane(cfg, entry))
}

fn head_entry_for_allow_id<'a>(cfg: &'a AllowConfig, allow_id: &str) -> Option<&'a AllowEntry> {
    cfg.allow.iter().find(|entry| entry.id == allow_id)
}

pub fn diff_ledger_movement_summary(
    base: &AllowConfig,
    head: &AllowConfig,
    finding_changes: &[FindingPostureChange],
    policy_changes: &[PolicyChange],
) -> DiffLedgerMovementSummary {
    let mut summary = DiffLedgerMovementSummary::default();

    for change in finding_changes {
        let row = classify_finding_posture_change(change);
        record_row(&mut summary, row);
    }

    let mut policy_rows_by_allow_id = BTreeMap::<&str, DiffRowClassification>::new();
    for change in policy_changes {
        if !is_allow_entry_id(change) {
            continue;
        }
        let row = classify_policy_change(change);
        policy_rows_by_allow_id
            .entry(change.allow_id.as_str())
            .and_modify(|existing| *existing = merge_policy_summary_rows(*existing, row))
            .or_insert(row);
    }
    for row in policy_rows_by_allow_id.values() {
        record_row(&mut summary, *row);
    }

    let base_ids = base
        .allow
        .iter()
        .map(|entry| entry.id.as_str())
        .collect::<BTreeSet<_>>();
    for entry in &head.allow {
        if !base_ids.contains(entry.id.as_str()) {
            continue;
        }
        if policy_rows_by_allow_id.contains_key(entry.id.as_str()) {
            continue;
        }
        record_row(
            &mut summary,
            DiffRowClassification {
                movement: PresenceMovement::Retained,
                posture_delta: PostureDelta::Unchanged,
                changed_in_diff: false,
            },
        );
    }

    summary
}

fn merge_policy_summary_rows(
    existing: DiffRowClassification,
    incoming: DiffRowClassification,
) -> DiffRowClassification {
    DiffRowClassification {
        movement: merge_presence_movement(existing.movement, incoming.movement),
        posture_delta: worst_posture_delta(existing.posture_delta, incoming.posture_delta),
        changed_in_diff: existing.changed_in_diff || incoming.changed_in_diff,
    }
}

fn merge_presence_movement(
    existing: PresenceMovement,
    incoming: PresenceMovement,
) -> PresenceMovement {
    match (existing, incoming) {
        (same, other) if same == other => same,
        (PresenceMovement::Retained, other) => other,
        (other, PresenceMovement::Retained) => other,
        (other, _) => other,
    }
}

fn worst_posture_delta(existing: PostureDelta, incoming: PostureDelta) -> PostureDelta {
    if posture_delta_severity(incoming) > posture_delta_severity(existing) {
        incoming
    } else {
        existing
    }
}

fn posture_delta_severity(delta: PostureDelta) -> u8 {
    match delta {
        PostureDelta::Worsened => 3,
        PostureDelta::ReviewRequired => 2,
        PostureDelta::Improved => 1,
        PostureDelta::Unchanged => 0,
    }
}

fn is_allow_entry_id(change: &PolicyChange) -> bool {
    !change.allow_id.starts_with("requirements.")
        && !change.allow_id.starts_with("policy.")
        && !change.allow_id.starts_with("workspace.")
}

fn record_row(summary: &mut DiffLedgerMovementSummary, row: DiffRowClassification) {
    match row.movement {
        PresenceMovement::Introduced => summary.movement.introduced += 1,
        PresenceMovement::Retained => summary.movement.retained += 1,
        PresenceMovement::Removed => summary.movement.removed += 1,
    }
    match row.posture_delta {
        PostureDelta::Improved => summary.posture_delta.improved += 1,
        PostureDelta::Worsened => summary.posture_delta.worsened += 1,
        PostureDelta::ReviewRequired => summary.posture_delta.review_required += 1,
        PostureDelta::Unchanged => summary.posture_delta.unchanged += 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::FindingKind;

    #[test]
    fn finding_classification_maps_presence_and_posture() {
        let change = FindingPostureChange {
            kind: FindingPostureKind::New,
            key: "panic:src/lib.rs".to_string(),
            finding_kind: FindingKind::Panic.as_str().to_string(),
            family: Some("unwrap".to_string()),
            path: "src/lib.rs".to_string(),
            line: None,
            column: None,
            source_package: None,
            identity: allow_core::StructuralIdentity::new("rust", "method_call"),
        };

        let row = classify_finding_posture_change(&change);
        assert_eq!(row.movement, PresenceMovement::Introduced);
        assert_eq!(row.posture_delta, PostureDelta::ReviewRequired);
        assert!(row.changed_in_diff);
        assert_eq!(
            finding_posture_subject(&change),
            "panic.unwrap at src/lib.rs"
        );
    }

    #[test]
    fn row_classification_projects_coverage_movement() {
        let worsened = classify_policy_change(&PolicyChange::new(
            "allow-0001",
            PolicyChangeKind::ScopeBroadened,
            PolicyChangeSeverity::Fail,
            "scope broadened",
        ));
        assert_eq!(worsened.coverage_movement_classification(), "worsened");

        let inherited = DiffRowClassification {
            movement: PresenceMovement::Retained,
            posture_delta: PostureDelta::Unchanged,
            changed_in_diff: false,
        };
        assert_eq!(inherited.coverage_movement_classification(), "inherited");
    }

    #[test]
    fn policy_classification_maps_entry_lifecycle_and_severity() {
        let introduced = PolicyChange::new(
            "allow-0001",
            PolicyChangeKind::AddedAllow,
            PolicyChangeSeverity::Review,
            "added",
        );
        let retained = PolicyChange::new(
            "allow-0001",
            PolicyChangeKind::ScopeBroadened,
            PolicyChangeSeverity::Fail,
            "broadened",
        );
        let removed = PolicyChange::new(
            "allow-0002",
            PolicyChangeKind::RemovedAllow,
            PolicyChangeSeverity::Improvement,
            "removed",
        );

        assert_eq!(
            classify_policy_change(&introduced).movement,
            PresenceMovement::Introduced
        );
        assert_eq!(
            classify_policy_change(&retained).movement,
            PresenceMovement::Retained
        );
        assert_eq!(
            classify_policy_change(&removed).movement,
            PresenceMovement::Removed
        );
        assert_eq!(
            classify_policy_change(&retained).posture_delta,
            PostureDelta::Worsened
        );
    }

    #[test]
    fn policy_classification_projects_family_rule_and_ambiguity_movement() {
        for (kind, expected) in [
            (
                PolicyChangeKind::FamilyRuleAdded,
                PresenceMovement::Introduced,
            ),
            (
                PolicyChangeKind::FamilyRuleRemoved,
                PresenceMovement::Removed,
            ),
            (
                PolicyChangeKind::AmbiguousClassification,
                PresenceMovement::Retained,
            ),
        ] {
            assert_eq!(policy_change_movement(kind), expected, "{kind:?}");
        }
    }

    #[test]
    fn movement_summary_counts_unchanged_retained_entries() {
        let base = AllowConfig {
            allow: vec![entry("allow-0001"), entry("allow-0002")],
            ..AllowConfig::empty()
        };
        let head = base.clone();
        let summary = diff_ledger_movement_summary(&base, &head, &[], &[]);
        assert_eq!(summary.movement.retained, 2);
        assert_eq!(summary.posture_delta.unchanged, 2);
    }

    #[test]
    fn movement_summary_counts_finding_and_policy_rows() {
        let base = AllowConfig {
            allow: vec![entry("allow-0001")],
            ..AllowConfig::empty()
        };
        let head = AllowConfig {
            allow: vec![entry("allow-0001"), entry("allow-0002")],
            ..AllowConfig::empty()
        };
        let finding_changes = vec![FindingPostureChange {
            kind: FindingPostureKind::New,
            key: "panic:src/lib.rs".to_string(),
            finding_kind: "panic".to_string(),
            family: None,
            path: "src/lib.rs".to_string(),
            line: None,
            column: None,
            source_package: None,
            identity: allow_core::StructuralIdentity::new("rust", "method_call"),
        }];
        let policy_changes = vec![PolicyChange::new(
            "allow-0002",
            PolicyChangeKind::AddedAllow,
            PolicyChangeSeverity::Review,
            "added",
        )];

        let summary = diff_ledger_movement_summary(&base, &head, &finding_changes, &policy_changes);
        assert_eq!(summary.movement.introduced, 2);
        assert_eq!(summary.movement.retained, 1);
        assert_eq!(summary.posture_delta.review_required, 2);
        assert_eq!(summary.posture_delta.unchanged, 1);
    }

    #[test]
    fn movement_summary_counts_multiple_changes_for_one_entry_once() {
        let base = AllowConfig {
            allow: vec![entry("allow-0001")],
            ..AllowConfig::empty()
        };
        let head = base.clone();
        let policy_changes = vec![
            PolicyChange::new(
                "allow-0001",
                PolicyChangeKind::ScopeBroadened,
                PolicyChangeSeverity::Fail,
                "scope broadened",
            ),
            PolicyChange::new(
                "allow-0001",
                PolicyChangeKind::ReasonChanged,
                PolicyChangeSeverity::Review,
                "reason changed",
            ),
        ];

        let summary = diff_ledger_movement_summary(&base, &head, &[], &policy_changes);

        assert_eq!(summary.movement.retained, 1);
        assert_eq!(summary.posture_delta.worsened, 1);
        assert_eq!(summary.posture_delta.review_required, 0);
        assert_eq!(summary.posture_delta.unchanged, 0);
    }

    #[test]
    fn movement_summary_excludes_non_entry_policy_changes() {
        let base = AllowConfig {
            allow: vec![entry("allow-0001")],
            ..AllowConfig::empty()
        };
        let head = base.clone();
        let policy_changes = vec![
            PolicyChange::new(
                "policy.status",
                PolicyChangeKind::PolicyStatusWeakened,
                PolicyChangeSeverity::Fail,
                "policy status weakened",
            ),
            PolicyChange::new(
                "requirements.unsafe_evidence_required",
                PolicyChangeKind::RequirementLoosened,
                PolicyChangeSeverity::Fail,
                "requirement loosened",
            ),
            PolicyChange::new(
                "workspace.ignored.target",
                PolicyChangeKind::WorkspaceIgnoredAdded,
                PolicyChangeSeverity::Fail,
                "workspace ignore added",
            ),
        ];

        let summary = diff_ledger_movement_summary(&base, &head, &[], &policy_changes);

        assert_eq!(summary.movement.retained, 1);
        assert_eq!(summary.posture_delta.worsened, 0);
        assert_eq!(summary.posture_delta.unchanged, 1);
    }

    fn entry(id: &str) -> AllowEntry {
        AllowEntry {
            id: id.to_string(),
            kind: FindingKind::Panic,
            family: None,
            path: None,
            glob: None,
            owner: "owner".to_string(),
            classification: "reviewed".to_string(),
            reason: "reason".to_string(),
            evidence: Vec::new(),
            links: Vec::new(),
            occurrence_limit: None,
            lifecycle: allow_core::Lifecycle::empty(),
            selector: allow_core::Selector::default(),
            last_seen: None,
        }
    }
}
