//! Governance authority reconciliation over supplied facts (#2942 step 2 /
//! #3328).
//!
//! Consumes the V2 governance DTOs from intent-model plus explicitly
//! supplied workspace/manifest facts and emits a current/target topology
//! report, workspace/member denominator, move/shim/parity reconciliation,
//! and transition deletion eligibility.
//!
//! Boundary: the engine consumes explicit bounded input only. It never
//! invokes Cargo, spawns processes, writes the filesystem, or reads ambient
//! workspace state.

use intent_model::{
    CutoverReferenceV2, GovernanceCrateIdentityV2, GovernancePackagePostureV2, MoveReferenceV2,
    ParityReferenceV2, ShimReferenceV2, ShimStatusV2, TargetDispositionV2, TransitionExpiryV2,
};

/// A supplied workspace member fact (parsed from a member manifest, never
/// discovered ambiently).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceMemberFactV2 {
    pub workspace_path: String,
    pub cargo_package_name: String,
}

/// A supplied authored disposition for one logical component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentDispositionRecordV2 {
    pub logical_id: String,
    pub disposition: TargetDispositionV2,
}

/// Explicit bounded reconciliation input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceReconciliationInputV2<'a> {
    pub crate_identities: &'a [GovernanceCrateIdentityV2],
    pub package_postures: &'a [GovernancePackagePostureV2],
    pub moves: &'a [MoveReferenceV2],
    pub shims: &'a [ShimReferenceV2],
    pub expiries: &'a [TransitionExpiryV2],
    pub parity_cases: &'a [ParityReferenceV2],
    pub cutover_receipts: &'a [CutoverReferenceV2],
    pub workspace_members: &'a [WorkspaceMemberFactV2],
    pub dispositions: &'a [ComponentDispositionRecordV2],
}

/// Severity of a reconciliation finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GovernanceFindingSeverityV2 {
    Blocking,
    Advisory,
}

impl GovernanceFindingSeverityV2 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Blocking => "blocking",
            Self::Advisory => "advisory",
        }
    }
}

/// Kind of reconciliation finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GovernanceFindingKindV2 {
    IdentityWithoutWorkspaceMember,
    WorkspaceMemberWithoutIdentity,
    DispositionWithoutIdentity,
    RetainPackageWithoutPosture,
    CollapseWithoutTargetMove,
    CompatibilityWithoutActiveShim,
    CompatibilityShimWithoutExpiry,
    RemoveWithoutCutoverReceipt,
    DeferUntilEvidence,
    ShimReferencesUnknownMove,
    ParityReferencesUnknownMove,
    ParityReferencesUnknownShim,
}

impl GovernanceFindingKindV2 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::IdentityWithoutWorkspaceMember => "identity_without_workspace_member",
            Self::WorkspaceMemberWithoutIdentity => "workspace_member_without_identity",
            Self::DispositionWithoutIdentity => "disposition_without_identity",
            Self::RetainPackageWithoutPosture => "retain_package_without_posture",
            Self::CollapseWithoutTargetMove => "collapse_without_target_move",
            Self::CompatibilityWithoutActiveShim => "compatibility_without_active_shim",
            Self::CompatibilityShimWithoutExpiry => "compatibility_shim_without_expiry",
            Self::RemoveWithoutCutoverReceipt => "remove_without_cutover_receipt",
            Self::DeferUntilEvidence => "defer_until_evidence",
            Self::ShimReferencesUnknownMove => "shim_references_unknown_move",
            Self::ParityReferencesUnknownMove => "parity_references_unknown_move",
            Self::ParityReferencesUnknownShim => "parity_references_unknown_shim",
        }
    }
}

/// One reconciliation finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceFindingV2 {
    pub kind: GovernanceFindingKindV2,
    pub severity: GovernanceFindingSeverityV2,
    pub subject: String,
    pub message: String,
}

/// Deletion eligibility for one compatibility component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeletionEligibilityV2 {
    /// The shim is retired and its removal condition is recorded: safe to
    /// delete after evidence review.
    Eligible,
    /// The shim still serves compatibility traffic.
    NotEligibleActive,
    /// No expiry/removal condition is recorded.
    NotEligibleMissingExpiry,
}

impl DeletionEligibilityV2 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Eligible => "eligible",
            Self::NotEligibleActive => "not_eligible_active",
            Self::NotEligibleMissingExpiry => "not_eligible_missing_expiry",
        }
    }
}

/// Deletion eligibility decision for one shim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletionEligibilityDecisionV2 {
    pub shim_id: String,
    pub eligibility: DeletionEligibilityV2,
    pub reason: String,
}

/// Full reconciliation report.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GovernanceReconciliationReportV2 {
    pub findings: Vec<GovernanceFindingV2>,
    pub deletion_eligibility: Vec<DeletionEligibilityDecisionV2>,
}

impl GovernanceReconciliationReportV2 {
    pub fn has_blocking(&self) -> bool {
        self.findings
            .iter()
            .any(|finding| finding.severity == GovernanceFindingSeverityV2::Blocking)
    }
}

/// Reconcile governance authority over supplied facts.
pub fn reconcile_governance_authority(
    input: &GovernanceReconciliationInputV2<'_>,
) -> GovernanceReconciliationReportV2 {
    let mut findings = Vec::new();
    findings.extend(reconcile_denominator(input));
    findings.extend(reconcile_dispositions(input));
    findings.extend(reconcile_linkage(input));

    let deletion_eligibility = reconcile_deletion_eligibility(input);

    GovernanceReconciliationReportV2 {
        findings,
        deletion_eligibility,
    }
}

/// Workspace/member denominator: every crate identity must map to a supplied
/// workspace member by path, and every member must map back to an identity.
fn reconcile_denominator(input: &GovernanceReconciliationInputV2<'_>) -> Vec<GovernanceFindingV2> {
    let mut findings = Vec::new();
    for identity in input.crate_identities {
        let matched = input
            .workspace_members
            .iter()
            .any(|member| member.workspace_path == identity.workspace_path);
        if !matched {
            findings.push(GovernanceFindingV2 {
                kind: GovernanceFindingKindV2::IdentityWithoutWorkspaceMember,
                severity: GovernanceFindingSeverityV2::Blocking,
                subject: identity.logical_id.clone(),
                message: format!(
                    "crate identity `{}` has no supplied workspace member at {}",
                    identity.logical_id, identity.workspace_path
                ),
            });
        }
    }
    for member in input.workspace_members {
        let matched = input
            .crate_identities
            .iter()
            .any(|identity| identity.workspace_path == member.workspace_path);
        if !matched {
            findings.push(GovernanceFindingV2 {
                kind: GovernanceFindingKindV2::WorkspaceMemberWithoutIdentity,
                severity: GovernanceFindingSeverityV2::Blocking,
                subject: member.cargo_package_name.clone(),
                message: format!(
                    "workspace member `{}` at {} has no governance identity",
                    member.cargo_package_name, member.workspace_path
                ),
            });
        }
    }
    findings
}

/// Current/target topology reconciliation per authored disposition.
fn reconcile_dispositions(input: &GovernanceReconciliationInputV2<'_>) -> Vec<GovernanceFindingV2> {
    let mut findings = Vec::new();
    for record in input.dispositions {
        let Some(identity) = input
            .crate_identities
            .iter()
            .find(|identity| identity.logical_id == record.logical_id)
        else {
            findings.push(GovernanceFindingV2 {
                kind: GovernanceFindingKindV2::DispositionWithoutIdentity,
                severity: GovernanceFindingSeverityV2::Blocking,
                subject: record.logical_id.clone(),
                message: format!(
                    "disposition for `{}` references an unknown identity",
                    record.logical_id
                ),
            });
            continue;
        };
        match record.disposition {
            TargetDispositionV2::RetainPackage => {
                let has_posture = input
                    .package_postures
                    .iter()
                    .any(|posture| posture.logical_id == record.logical_id);
                if !has_posture {
                    findings.push(GovernanceFindingV2 {
                        kind: GovernanceFindingKindV2::RetainPackageWithoutPosture,
                        severity: GovernanceFindingSeverityV2::Blocking,
                        subject: record.logical_id.clone(),
                        message: format!(
                            "retain_package disposition for `{}` requires a package posture",
                            record.logical_id
                        ),
                    });
                }
            }
            TargetDispositionV2::CollapseIntoPackage => {
                let has_target = input
                    .moves
                    .iter()
                    .any(|movement| movement.target_crate == identity.cargo_package_name);
                if !has_target {
                    findings.push(GovernanceFindingV2 {
                        kind: GovernanceFindingKindV2::CollapseWithoutTargetMove,
                        severity: GovernanceFindingSeverityV2::Blocking,
                        subject: record.logical_id.clone(),
                        message: format!(
                            "collapse_into_package disposition for `{}` requires a move targeting it",
                            record.logical_id
                        ),
                    });
                }
            }
            TargetDispositionV2::CompatibilityOnly => {
                let active_shim = input.shims.iter().find(|shim| {
                    shim.status == ShimStatusV2::Active
                        && shim.new_identity.contains(&identity.rust_library_name)
                });
                match active_shim {
                    None => findings.push(GovernanceFindingV2 {
                        kind: GovernanceFindingKindV2::CompatibilityWithoutActiveShim,
                        severity: GovernanceFindingSeverityV2::Blocking,
                        subject: record.logical_id.clone(),
                        message: format!(
                            "compatibility_only disposition for `{}` requires an active shim",
                            record.logical_id
                        ),
                    }),
                    Some(shim) => {
                        let has_expiry = input
                            .expiries
                            .iter()
                            .any(|expiry| expiry.component_id == shim.shim_id);
                        if !has_expiry {
                            findings.push(GovernanceFindingV2 {
                                kind: GovernanceFindingKindV2::CompatibilityShimWithoutExpiry,
                                severity: GovernanceFindingSeverityV2::Blocking,
                                subject: shim.shim_id.clone(),
                                message: format!(
                                    "active shim `{}` for compatibility component `{}` has no expiry record",
                                    shim.shim_id, record.logical_id
                                ),
                            });
                        }
                    }
                }
            }
            TargetDispositionV2::RemoveAfterCutover => {
                let owner_label = identity.owner.as_str();
                let has_receipt = input.cutover_receipts.iter().any(|receipt| {
                    receipt.product == owner_label && !receipt.receipt_id.is_empty()
                });
                if !has_receipt {
                    findings.push(GovernanceFindingV2 {
                        kind: GovernanceFindingKindV2::RemoveWithoutCutoverReceipt,
                        severity: GovernanceFindingSeverityV2::Blocking,
                        subject: record.logical_id.clone(),
                        message: format!(
                            "remove_after_cutover disposition for `{}` requires a supplied cutover receipt",
                            record.logical_id
                        ),
                    });
                }
            }
            TargetDispositionV2::DeferUntilEvidence => findings.push(GovernanceFindingV2 {
                kind: GovernanceFindingKindV2::DeferUntilEvidence,
                severity: GovernanceFindingSeverityV2::Advisory,
                subject: record.logical_id.clone(),
                message: format!(
                    "component `{}` is deferred until evidence; no reconciliation action",
                    record.logical_id
                ),
            }),
        }
    }
    findings
}

/// Move/shim/parity cross-reference reconciliation.
fn reconcile_linkage(input: &GovernanceReconciliationInputV2<'_>) -> Vec<GovernanceFindingV2> {
    let mut findings = Vec::new();
    for shim in input.shims {
        let known = input
            .moves
            .iter()
            .any(|movement| movement.entry_id == shim.move_ledger_entry);
        if !known {
            findings.push(GovernanceFindingV2 {
                kind: GovernanceFindingKindV2::ShimReferencesUnknownMove,
                severity: GovernanceFindingSeverityV2::Blocking,
                subject: shim.shim_id.clone(),
                message: format!(
                    "shim `{}` references unknown move entry `{}`",
                    shim.shim_id, shim.move_ledger_entry
                ),
            });
        }
    }
    for case in input.parity_cases {
        let known_move = input
            .moves
            .iter()
            .any(|movement| movement.entry_id == case.move_ledger_entry);
        if !known_move {
            findings.push(GovernanceFindingV2 {
                kind: GovernanceFindingKindV2::ParityReferencesUnknownMove,
                severity: GovernanceFindingSeverityV2::Blocking,
                subject: case.case_id.clone(),
                message: format!(
                    "parity case `{}` references unknown move entry `{}`",
                    case.case_id, case.move_ledger_entry
                ),
            });
        }
        if let Some(shim_id) = &case.shim_id {
            let known_shim = input.shims.iter().any(|shim| &shim.shim_id == shim_id);
            if !known_shim {
                findings.push(GovernanceFindingV2 {
                    kind: GovernanceFindingKindV2::ParityReferencesUnknownShim,
                    severity: GovernanceFindingSeverityV2::Blocking,
                    subject: case.case_id.clone(),
                    message: format!(
                        "parity case `{}` references unknown shim `{shim_id}`",
                        case.case_id
                    ),
                });
            }
        }
    }
    findings
}

/// Transition deletion eligibility: a shim is eligible when it is retired
/// and an expiry with a removal condition is recorded.
fn reconcile_deletion_eligibility(
    input: &GovernanceReconciliationInputV2<'_>,
) -> Vec<DeletionEligibilityDecisionV2> {
    input
        .shims
        .iter()
        .map(|shim| {
            let has_expiry = input
                .expiries
                .iter()
                .any(|expiry| expiry.component_id == shim.shim_id);
            match (shim.status, has_expiry) {
                (ShimStatusV2::Retired, true) => DeletionEligibilityDecisionV2 {
                    shim_id: shim.shim_id.clone(),
                    eligibility: DeletionEligibilityV2::Eligible,
                    reason: "shim is retired with a recorded removal condition".to_string(),
                },
                (ShimStatusV2::Retired, false) => DeletionEligibilityDecisionV2 {
                    shim_id: shim.shim_id.clone(),
                    eligibility: DeletionEligibilityV2::NotEligibleMissingExpiry,
                    reason: "shim is retired but no expiry/removal condition is recorded"
                        .to_string(),
                },
                (_, _) => DeletionEligibilityDecisionV2 {
                    shim_id: shim.shim_id.clone(),
                    eligibility: DeletionEligibilityV2::NotEligibleActive,
                    reason: format!("shim status is `{}`", shim.status.as_str()),
                },
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use intent_model::{GovernanceCrateRoleV2, GovernanceOwnerV2, ParityDispositionV2};

    fn identity(
        logical_id: &str,
        package: &str,
        owner: GovernanceOwnerV2,
    ) -> GovernanceCrateIdentityV2 {
        GovernanceCrateIdentityV2 {
            logical_id: logical_id.to_string(),
            workspace_path: format!("crates/{logical_id}"),
            workspace_dependency_aliases: vec![package.to_string()],
            cargo_package_name: package.to_string(),
            rust_library_name: logical_id.replace('-', "_"),
            owner,
            role: GovernanceCrateRoleV2::CargoProof,
        }
    }

    fn member(logical_id: &str, package: &str) -> WorkspaceMemberFactV2 {
        WorkspaceMemberFactV2 {
            workspace_path: format!("crates/{logical_id}"),
            cargo_package_name: package.to_string(),
        }
    }

    fn shim(shim_id: &str, status: ShimStatusV2, move_entry: &str) -> ShimReferenceV2 {
        ShimReferenceV2 {
            shim_id: shim_id.to_string(),
            old_identity: format!("{shim_id}::old"),
            new_identity: format!("{shim_id}::new"),
            status,
            move_ledger_entry: move_entry.to_string(),
            controlling_issue: 2606,
            latest_allowed_stage: 1,
        }
    }

    fn move_entry(entry_id: &str, target: &str) -> MoveReferenceV2 {
        MoveReferenceV2 {
            entry_id: entry_id.to_string(),
            source_kind: "RustModule".to_string(),
            current_product: "cargo-allow".to_string(),
            current_crate: "allow-policy".to_string(),
            target_product: "cargo-intent".to_string(),
            target_crate: target.to_string(),
        }
    }

    fn parity(case_id: &str, move_entry: &str, shim_id: Option<&str>) -> ParityReferenceV2 {
        ParityReferenceV2 {
            case_id: case_id.to_string(),
            stage: "ProofEngineAndCli".to_string(),
            move_ledger_entry: move_entry.to_string(),
            shim_id: shim_id.map(str::to_string),
            disposition: ParityDispositionV2::ContractOnly,
        }
    }

    #[test]
    fn retain_disposition_requires_posture() -> Result<(), String> {
        let identities = vec![identity(
            "cargo-allow",
            "cargo-allow",
            GovernanceOwnerV2::CargoAllow,
        )];
        let members = vec![member("cargo-allow", "cargo-allow")];
        let dispositions = vec![ComponentDispositionRecordV2 {
            logical_id: "cargo-allow".to_string(),
            disposition: TargetDispositionV2::RetainPackage,
        }];
        let input = GovernanceReconciliationInputV2 {
            crate_identities: &identities,
            package_postures: &[],
            moves: &[],
            shims: &[],
            expiries: &[],
            parity_cases: &[],
            cutover_receipts: &[],
            workspace_members: &members,
            dispositions: &dispositions,
        };
        let report = reconcile_governance_authority(&input);
        if !report.has_blocking() {
            return Err("retain without posture must be blocking".into());
        }
        if !report
            .findings
            .iter()
            .any(|f| f.kind == GovernanceFindingKindV2::RetainPackageWithoutPosture)
        {
            return Err(format!("expected retain finding: {:?}", report.findings));
        }
        Ok(())
    }

    #[test]
    fn collapse_disposition_requires_target_move() -> Result<(), String> {
        let identities = vec![identity(
            "proof-engine",
            "proof-orchestrator",
            GovernanceOwnerV2::CargoProof,
        )];
        let members = vec![member("proof-engine", "proof-orchestrator")];
        let dispositions = vec![ComponentDispositionRecordV2 {
            logical_id: "proof-engine".to_string(),
            disposition: TargetDispositionV2::CollapseIntoPackage,
        }];
        let input = GovernanceReconciliationInputV2 {
            crate_identities: &identities,
            package_postures: &[],
            moves: &[],
            shims: &[],
            expiries: &[],
            parity_cases: &[],
            cutover_receipts: &[],
            workspace_members: &members,
            dispositions: &dispositions,
        };
        let report = reconcile_governance_authority(&input);
        if !report
            .findings
            .iter()
            .any(|f| f.kind == GovernanceFindingKindV2::CollapseWithoutTargetMove)
        {
            return Err("collapse without target move must be blocking".into());
        }
        Ok(())
    }

    #[test]
    fn compatibility_disposition_requires_active_shim_with_expiry() -> Result<(), String> {
        let identities = vec![identity(
            "allow-policy-legacy",
            "allow-policy-legacy",
            GovernanceOwnerV2::CargoAllow,
        )];
        let members = vec![member("allow-policy-legacy", "allow-policy-legacy")];
        let dispositions = vec![ComponentDispositionRecordV2 {
            logical_id: "allow-policy-legacy".to_string(),
            disposition: TargetDispositionV2::CompatibilityOnly,
        }];
        let shims = vec![ShimReferenceV2 {
            shim_id: "shim-legacy".to_string(),
            old_identity: "allow-policy::spec_system".to_string(),
            new_identity: "allow_policy_legacy::compat".to_string(),
            status: ShimStatusV2::Active,
            move_ledger_entry: "MOVE-LEGACY".to_string(),
            controlling_issue: 2606,
            latest_allowed_stage: 1,
        }];
        let no_expiry = GovernanceReconciliationInputV2 {
            crate_identities: &identities,
            package_postures: &[],
            moves: &[move_entry("MOVE-LEGACY", "allow-policy")],
            shims: &shims,
            expiries: &[],
            parity_cases: &[],
            cutover_receipts: &[],
            workspace_members: &members,
            dispositions: &dispositions,
        };
        let report = reconcile_governance_authority(&no_expiry);
        if !report
            .findings
            .iter()
            .any(|f| f.kind == GovernanceFindingKindV2::CompatibilityShimWithoutExpiry)
        {
            return Err("active shim without expiry must be blocking".into());
        }

        let expiries = vec![TransitionExpiryV2 {
            component_id: "shim-legacy".to_string(),
            removal_condition: "issue:#2568 core retirement".to_string(),
            rollback_note: String::new(),
        }];
        let with_expiry = GovernanceReconciliationInputV2 {
            expiries: &expiries,
            ..no_expiry
        };
        let report = reconcile_governance_authority(&with_expiry);
        if report.findings.iter().any(|f| {
            f.kind == GovernanceFindingKindV2::CompatibilityWithoutActiveShim
                || f.kind == GovernanceFindingKindV2::CompatibilityShimWithoutExpiry
        }) {
            return Err(format!(
                "satisfied compatibility disposition must not flag: {:?}",
                report.findings
            ));
        }
        Ok(())
    }

    #[test]
    fn remove_disposition_requires_cutover_receipt() -> Result<(), String> {
        let identities = vec![identity(
            "intent-edit",
            "intent-edit",
            GovernanceOwnerV2::CargoIntent,
        )];
        let members = vec![member("intent-edit", "intent-edit")];
        let dispositions = vec![ComponentDispositionRecordV2 {
            logical_id: "intent-edit".to_string(),
            disposition: TargetDispositionV2::RemoveAfterCutover,
        }];
        let receipts = vec![CutoverReferenceV2 {
            stage: 2,
            product: "cargo-intent".to_string(),
            receipt_id: "cutover-intent-stage-2".to_string(),
        }];
        let input = GovernanceReconciliationInputV2 {
            crate_identities: &identities,
            package_postures: &[],
            moves: &[],
            shims: &[],
            expiries: &[],
            parity_cases: &[],
            cutover_receipts: &receipts,
            workspace_members: &members,
            dispositions: &dispositions,
        };
        let satisfied = reconcile_governance_authority(&input);
        if satisfied
            .findings
            .iter()
            .any(|f| f.kind == GovernanceFindingKindV2::RemoveWithoutCutoverReceipt)
        {
            return Err("supplied receipt must satisfy remove disposition".into());
        }

        let empty_receipts = Vec::new();
        let missing = GovernanceReconciliationInputV2 {
            cutover_receipts: &empty_receipts,
            ..input
        };
        let report = reconcile_governance_authority(&missing);
        if !report
            .findings
            .iter()
            .any(|f| f.kind == GovernanceFindingKindV2::RemoveWithoutCutoverReceipt)
        {
            return Err("remove without receipt must be blocking".into());
        }
        Ok(())
    }

    #[test]
    fn defer_disposition_is_advisory_only() -> Result<(), String> {
        let identities = vec![identity(
            "future-crate",
            "future-crate",
            GovernanceOwnerV2::Shared,
        )];
        let members = vec![member("future-crate", "future-crate")];
        let dispositions = vec![ComponentDispositionRecordV2 {
            logical_id: "future-crate".to_string(),
            disposition: TargetDispositionV2::DeferUntilEvidence,
        }];
        let input = GovernanceReconciliationInputV2 {
            crate_identities: &identities,
            package_postures: &[],
            moves: &[],
            shims: &[],
            expiries: &[],
            parity_cases: &[],
            cutover_receipts: &[],
            workspace_members: &members,
            dispositions: &dispositions,
        };
        let report = reconcile_governance_authority(&input);
        if report.has_blocking() {
            return Err("defer disposition must never block".into());
        }
        if !report.findings.iter().any(|f| {
            f.kind == GovernanceFindingKindV2::DeferUntilEvidence
                && f.severity == GovernanceFindingSeverityV2::Advisory
        }) {
            return Err("defer must produce an advisory finding".into());
        }
        Ok(())
    }

    #[test]
    fn denominator_flags_both_directions() -> Result<(), String> {
        let members = vec![
            member("known-crate", "known-crate"),
            WorkspaceMemberFactV2 {
                workspace_path: "crates/orphan".to_string(),
                cargo_package_name: "orphan".to_string(),
            },
        ];
        let identities = vec![
            identity("known-crate", "known-crate", GovernanceOwnerV2::Shared),
            identity("ghost", "ghost", GovernanceOwnerV2::Shared),
        ];
        let input = GovernanceReconciliationInputV2 {
            crate_identities: &identities,
            package_postures: &[],
            moves: &[],
            shims: &[],
            expiries: &[],
            parity_cases: &[],
            cutover_receipts: &[],
            workspace_members: &members,
            dispositions: &[],
        };
        let report = reconcile_governance_authority(&input);
        if !report.findings.iter().any(|f| {
            f.kind == GovernanceFindingKindV2::WorkspaceMemberWithoutIdentity
                && f.subject == "orphan"
        }) {
            return Err("orphan member must be flagged".into());
        }
        if !report.findings.iter().any(|f| {
            f.kind == GovernanceFindingKindV2::IdentityWithoutWorkspaceMember
                && f.subject == "ghost"
        }) {
            return Err("member-less identity must be flagged".into());
        }
        Ok(())
    }

    #[test]
    fn linkage_flags_unknown_move_and_shim_references() -> Result<(), String> {
        let shims = vec![shim("shim-a", ShimStatusV2::Active, "MOVE-UNKNOWN")];
        let cases = vec![
            parity("case-known", "MOVE-KNOWN", None),
            parity("case-bad-move", "MOVE-NOPE", None),
            parity("case-bad-shim", "MOVE-KNOWN", Some("shim-ghost")),
        ];
        let input = GovernanceReconciliationInputV2 {
            crate_identities: &[],
            package_postures: &[],
            moves: &[move_entry("MOVE-KNOWN", "target")],
            shims: &shims,
            expiries: &[],
            parity_cases: &cases,
            cutover_receipts: &[],
            workspace_members: &[],
            dispositions: &[],
        };
        let report = reconcile_governance_authority(&input);
        for (subject, kind) in [
            ("shim-a", GovernanceFindingKindV2::ShimReferencesUnknownMove),
            (
                "case-bad-move",
                GovernanceFindingKindV2::ParityReferencesUnknownMove,
            ),
            (
                "case-bad-shim",
                GovernanceFindingKindV2::ParityReferencesUnknownShim,
            ),
        ] {
            if !report
                .findings
                .iter()
                .any(|f| f.subject == subject && f.kind == kind)
            {
                return Err(format!(
                    "expected {subject} flagged with {:?}: {:?}",
                    kind, report.findings
                ));
            }
        }
        if report.findings.iter().any(|f| f.subject == "case-known") {
            return Err("known linkage must not be flagged".into());
        }
        Ok(())
    }

    #[test]
    fn deletion_eligibility_tracks_retirement_and_expiry() -> Result<(), String> {
        let shims = vec![
            shim("shim-retired", ShimStatusV2::Retired, "MOVE-A"),
            shim("shim-active", ShimStatusV2::Active, "MOVE-A"),
            shim("shim-no-expiry", ShimStatusV2::Retired, "MOVE-A"),
        ];
        let expiries = vec![
            TransitionExpiryV2 {
                component_id: "shim-retired".to_string(),
                removal_condition: "cutover receipt present".to_string(),
                rollback_note: String::new(),
            },
            TransitionExpiryV2 {
                component_id: "shim-active".to_string(),
                removal_condition: "cutover receipt present".to_string(),
                rollback_note: String::new(),
            },
        ];
        let input = GovernanceReconciliationInputV2 {
            crate_identities: &[],
            package_postures: &[],
            moves: &[move_entry("MOVE-A", "target")],
            shims: &shims,
            expiries: &expiries,
            parity_cases: &[],
            cutover_receipts: &[],
            workspace_members: &[],
            dispositions: &[],
        };
        let report = reconcile_governance_authority(&input);
        let decision = |id: &str| {
            report
                .deletion_eligibility
                .iter()
                .find(|d| d.shim_id == id)
                .map(|d| d.eligibility)
                .ok_or_else(|| format!("missing decision for {id}"))
        };
        if decision("shim-retired")? != DeletionEligibilityV2::Eligible {
            return Err("retired shim with expiry must be eligible".into());
        }
        if decision("shim-active")? != DeletionEligibilityV2::NotEligibleActive {
            return Err("active shim must not be eligible".into());
        }
        if decision("shim-no-expiry")? != DeletionEligibilityV2::NotEligibleMissingExpiry {
            return Err("retired shim without expiry must flag missing expiry".into());
        }
        Ok(())
    }
}
