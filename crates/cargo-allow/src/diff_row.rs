use allow_core::AllowConfig;

use allow_report::{
    DiffFindingChange, DiffLedgerMovementSummary, DiffMovementCounts, DiffPolicyChange,
    DiffPostureDeltaCounts,
};

pub struct FindingRowBundle {
    pub change: allow_diff::FindingPostureChange,
    pub subject: String,
    pub ledger_id: String,
    pub lane: String,
}

pub struct PolicyRowBundle {
    pub change: allow_diff::PolicyChange,
    pub subject: String,
    pub ledger_id: String,
    pub lane: Option<String>,
}

pub struct DiffLedgerContext<'a> {
    pub base_cfg: &'a AllowConfig,
    pub head_cfg: &'a AllowConfig,
    pub finding_changes: &'a [allow_diff::FindingPostureChange],
    pub policy_changes: &'a [allow_diff::PolicyChange],
    pub diff_analysis: allow_report::DiffAnalysisContext<'a>,
}

impl<'a> DiffLedgerContext<'a> {
    pub fn new(
        base_cfg: &'a AllowConfig,
        head_cfg: &'a AllowConfig,
        finding_changes: &'a [allow_diff::FindingPostureChange],
        policy_changes: &'a [allow_diff::PolicyChange],
        diff_analysis: allow_report::DiffAnalysisContext<'a>,
    ) -> Self {
        Self {
            base_cfg,
            head_cfg,
            finding_changes,
            policy_changes,
            diff_analysis,
        }
    }

    pub fn ledger_movement_summary(&self) -> DiffLedgerMovementSummary {
        ledger_movement_summary_from_diff(allow_diff::diff_ledger_movement_summary(
            self.base_cfg,
            self.head_cfg,
            self.finding_changes,
            self.policy_changes,
        ))
    }
}

pub fn ledger_movement_summary_from_diff(
    summary: allow_diff::DiffLedgerMovementSummary,
) -> DiffLedgerMovementSummary {
    DiffLedgerMovementSummary {
        movement: DiffMovementCounts {
            introduced: summary.movement.introduced,
            retained: summary.movement.retained,
            removed: summary.movement.removed,
        },
        posture_delta: DiffPostureDeltaCounts {
            improved: summary.posture_delta.improved,
            worsened: summary.posture_delta.worsened,
            review_required: summary.posture_delta.review_required,
            unchanged: summary.posture_delta.unchanged,
        },
    }
}

pub fn finding_row_bundles(
    changes: &[allow_diff::FindingPostureChange],
    head_cfg: &AllowConfig,
) -> Vec<FindingRowBundle> {
    let ledger_id = allow_diff::entry_ledger_id(head_cfg);
    changes
        .iter()
        .map(|change| FindingRowBundle {
            subject: allow_diff::finding_posture_subject(change),
            lane: default_lane_for_finding_kind(head_cfg, &change.finding_kind),
            ledger_id: ledger_id.clone(),
            change: change.clone(),
        })
        .collect()
}

pub fn policy_row_bundles(
    changes: &[allow_diff::PolicyChange],
    head_cfg: &AllowConfig,
) -> Vec<PolicyRowBundle> {
    let ledger_id = allow_diff::policy_change_ledger_id(head_cfg);
    changes
        .iter()
        .map(|change| PolicyRowBundle {
            subject: allow_diff::policy_change_subject(change),
            lane: allow_diff::policy_change_lane(head_cfg, change)
                .or_else(|| default_lane_for_policy_change(head_cfg, change)),
            ledger_id: ledger_id.clone(),
            change: change.clone(),
        })
        .collect()
}

pub fn finding_change_rows(bundles: &[FindingRowBundle]) -> Vec<DiffFindingChange<'_>> {
    bundles
        .iter()
        .map(|bundle| {
            let class = allow_diff::classify_finding_posture_change(&bundle.change);
            DiffFindingChange {
                change: bundle.change.kind.as_str(),
                movement: class.movement.field_name(),
                posture_delta: class.posture_delta.field_name(),
                changed_in_diff: class.changed_in_diff,
                subject: Some(bundle.subject.as_str()),
                allow_id: None,
                ledger_id: Some(bundle.ledger_id.as_str()),
                lane: Some(bundle.lane.as_str()),
                key: &bundle.change.key,
                kind: &bundle.change.finding_kind,
                family: bundle.change.family.as_deref(),
                path: &bundle.change.path,
                line: bundle.change.line,
                column: bundle.change.column,
                source_package: bundle.change.source_package.as_deref(),
                identity: Some(&bundle.change.identity),
            }
        })
        .collect()
}

pub fn policy_change_rows(bundles: &[PolicyRowBundle]) -> Vec<DiffPolicyChange<'_>> {
    bundles
        .iter()
        .map(|bundle| {
            let class = allow_diff::classify_policy_change(&bundle.change);
            DiffPolicyChange {
                severity: bundle.change.severity.as_str(),
                movement: class.movement.field_name(),
                posture_delta: class.posture_delta.field_name(),
                changed_in_diff: class.changed_in_diff,
                subject: Some(bundle.subject.as_str()),
                allow_id: &bundle.change.allow_id,
                ledger_id: Some(bundle.ledger_id.as_str()),
                lane: bundle.lane.as_deref(),
                kind: bundle.change.kind.as_str(),
                message: &bundle.change.message,
                exception_identity: bundle.change.exception_identity.as_ref().map(|identity| {
                    allow_report::DiffExceptionIdentityChange {
                        field: identity.field.as_str(),
                        before: identity.before.as_deref(),
                        after: identity.after.as_deref(),
                    }
                }),
                selector_identity: bundle.change.selector_identity.as_ref().map(|identity| {
                    allow_report::DiffSelectorIdentityChange {
                        changed_fields: &identity.changed_fields,
                    }
                }),
                selector_precision: bundle.change.selector_precision.as_ref().map(|selector| {
                    allow_report::DiffSelectorPrecisionChange {
                        before: selector.before,
                        after: selector.after,
                        removed_fields: &selector.removed_fields,
                        added_fields: &selector.added_fields,
                    }
                }),
                scope: bundle
                    .change
                    .scope
                    .as_ref()
                    .map(|scope| allow_report::DiffScopeChange {
                        field: scope.field.as_str(),
                        before: scope.before.as_deref(),
                        after: scope.after.as_deref(),
                    }),
                occurrence_limit: bundle.change.occurrence_limit.as_ref().map(|limit| {
                    allow_report::DiffOccurrenceLimitChange {
                        before: limit.before,
                        after: limit.after,
                    }
                }),
                lifecycle: bundle.change.lifecycle.as_ref().map(|lifecycle| {
                    allow_report::DiffLifecycleChange {
                        field: lifecycle.field.as_str(),
                        before: lifecycle.before.as_deref(),
                        after: lifecycle.after.as_deref(),
                    }
                }),
                evidence: bundle.change.evidence.as_ref().map(|evidence| {
                    allow_report::DiffEvidenceChange {
                        field: evidence.field.as_str(),
                        removed: &evidence.removed,
                        added: &evidence.added,
                    }
                }),
                metadata: bundle.change.metadata.as_ref().map(|metadata| {
                    allow_report::DiffMetadataChange {
                        field: metadata.field.as_str(),
                        before: metadata.before.as_deref(),
                        after: metadata.after.as_deref(),
                    }
                }),
                requirement: bundle.change.requirement.as_ref().map(|requirement| {
                    allow_report::DiffRequirementChange {
                        field: requirement.field.as_str(),
                        before: requirement.before,
                        after: requirement.after,
                    }
                }),
                policy_status: bundle.change.policy_status.as_ref().map(|policy_status| {
                    allow_report::DiffPolicyStatusChange {
                        before: policy_status.before.as_deref(),
                        after: policy_status.after.as_deref(),
                    }
                }),
            }
        })
        .collect()
}
fn default_lane_for_finding_kind(cfg: &AllowConfig, kind: &str) -> String {
    if cfg.lanes.contains_key(kind) {
        kind.to_string()
    } else {
        "source-exception".to_string()
    }
}

fn default_lane_for_policy_change(
    cfg: &AllowConfig,
    change: &allow_diff::PolicyChange,
) -> Option<String> {
    if change.allow_id.starts_with("requirements.")
        || change.allow_id.starts_with("policy.")
        || change.allow_id.starts_with("workspace.")
    {
        None
    } else if let Some(entry) = cfg.allow.iter().find(|entry| entry.id == change.allow_id) {
        Some(allow_diff::entry_lane(cfg, entry))
    } else {
        Some("source-exception".to_string())
    }
}
