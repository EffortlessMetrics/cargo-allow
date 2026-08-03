use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{CORE_ADOPTION_PLAN_SCHEMA_ID, CORE_ADOPTION_PLAN_SCHEMA_VERSION};

const PORTABLE_ROOT: &str = "<repository-root>";
const CLAIM_BOUNDARY: &str = "This plan classifies source-tree exception-ledger adoption state and recommends one bounded next action; it does not execute commands or prove macro-expanded, type-aware, control-flow, data-flow, unsafe, coverage, or release behavior.";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InventoryMode {
    GitTracked,
    Filesystem,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InventoryCompleteness {
    Complete,
    Partial,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyState {
    Absent,
    Valid,
    Invalid,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BootstrapDisposition {
    CleanNoPolicy,
    FindingsNoPolicy,
    ExistingPolicyHealthy,
    ExistingPolicyHasNewFindings,
    ExistingPolicyNeedsRepair,
    PartialInventory,
    InvalidPolicy,
    UnsupportedRepositoryState,
    InstrumentFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdoptionActionKind {
    ContinueAdvisoryAudit,
    PreviewInit,
    PreviewPropose,
    RunNoNewCheck,
    InspectNewFinding,
    ApplyStaleSafeFindingPlan,
    InspectAllow,
    PreviewRefresh,
    RepairEvidence,
    PreviewPrune,
    ReconcileMirror,
    DiagnoseInventory,
    RepairPolicy,
    ConfigureCi,
}

impl AdoptionActionKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ContinueAdvisoryAudit => "continue_advisory_audit",
            Self::PreviewInit => "preview_init",
            Self::PreviewPropose => "preview_propose",
            Self::RunNoNewCheck => "run_no_new_check",
            Self::InspectNewFinding => "inspect_new_finding",
            Self::ApplyStaleSafeFindingPlan => "apply_stale_safe_finding_plan",
            Self::InspectAllow => "inspect_allow",
            Self::PreviewRefresh => "preview_refresh",
            Self::RepairEvidence => "repair_evidence",
            Self::PreviewPrune => "preview_prune",
            Self::ReconcileMirror => "reconcile_mirror",
            Self::DiagnoseInventory => "diagnose_inventory",
            Self::RepairPolicy => "repair_policy",
            Self::ConfigureCi => "configure_ci",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WritePosture {
    ReadOnly,
    PreviewOnly,
    MayWrite,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdoptionInventoryFacts {
    pub mode: InventoryMode,
    pub completeness: InventoryCompleteness,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdoptionPolicyFacts {
    pub state: PolicyState,
    pub path: Option<String>,
    pub schema_version: Option<String>,
    pub digest: Option<String>,
    pub total_findings: usize,
    pub new_unreceipted_findings: usize,
    pub stale_entries: usize,
    pub location_drift_entries: usize,
    pub broken_evidence_entries: usize,
    pub review_due_entries: usize,
    pub expired_entries: usize,
    pub occurrence_headroom_entries: usize,
    pub mirror_divergence: bool,
}

impl AdoptionPolicyFacts {
    fn has_repair_signal(&self) -> bool {
        self.stale_entries > 0
            || self.location_drift_entries > 0
            || self.broken_evidence_entries > 0
            || self.review_due_entries > 0
            || self.expired_entries > 0
            || self.occurrence_headroom_entries > 0
            || self.mirror_divergence
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdoptionFacts {
    pub tool_version: String,
    pub repository_identity: String,
    pub selected_root: String,
    pub channel: String,
    pub executable_identity: String,
    pub inventory: AdoptionInventoryFacts,
    pub policy: AdoptionPolicyFacts,
    pub policy_config_diagnostic: Option<String>,
    pub unsupported_repository_state: bool,
    pub instrument_failure: Option<String>,
    pub strict_gate_requested: bool,
    pub ci_guidance_completed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdoptionAction {
    pub kind: AdoptionActionKind,
    pub argv: Vec<String>,
    pub reason: String,
    pub write_posture: WritePosture,
    pub expected_result: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreAdoptionPlanV1 {
    pub schema_id: &'static str,
    pub schema_version: u32,
    pub tool_version: String,
    pub repository_identity: String,
    pub selected_root: &'static str,
    pub channel: String,
    pub executable_identity: String,
    pub inventory: AdoptionInventoryFacts,
    pub policy: AdoptionPolicyFacts,
    pub bootstrap_disposition: BootstrapDisposition,
    pub primary_action: AdoptionAction,
    pub follow_up_actions: Vec<AdoptionAction>,
    pub may_write_paths: Vec<String>,
    pub explicit_non_effects: Vec<String>,
    pub expected_result_markers: Vec<String>,
    pub ci_example_path: String,
    pub rollback_guide_path: String,
    pub limitations: Vec<String>,
    pub claim_boundary: &'static str,
}

pub fn recommend_core_adoption_plan(facts: &AdoptionFacts) -> CoreAdoptionPlanV1 {
    let policy = AdoptionPolicyFacts {
        path: facts
            .policy
            .path
            .as_deref()
            .map(|path| portable_path(&facts.selected_root, path)),
        ..facts.policy.clone()
    };
    let (bootstrap_disposition, primary_kind) = primary_route(facts, &policy);
    let primary_action = action(primary_kind, facts);
    let follow_up_actions = follow_up_actions(facts, &policy, primary_kind);
    let may_write_paths = if primary_action.write_posture == WritePosture::MayWrite {
        policy.path.clone().into_iter().collect()
    } else {
        Vec::new()
    };
    let mut limitations = facts.inventory.limitations.clone();
    if facts.policy_config_diagnostic.is_some() {
        limitations.push("policy diagnostics are available only as normalized input facts".into());
    }
    limitations.sort();
    limitations.dedup();

    CoreAdoptionPlanV1 {
        schema_id: CORE_ADOPTION_PLAN_SCHEMA_ID,
        schema_version: CORE_ADOPTION_PLAN_SCHEMA_VERSION,
        tool_version: facts.tool_version.clone(),
        repository_identity: facts.repository_identity.clone(),
        selected_root: PORTABLE_ROOT,
        channel: facts.channel.clone(),
        executable_identity: facts.executable_identity.clone(),
        inventory: facts.inventory.clone(),
        policy,
        bootstrap_disposition,
        primary_action,
        follow_up_actions,
        may_write_paths,
        explicit_non_effects: vec![
            "does not write policy or source files".into(),
            "does not execute the recommended command".into(),
            "does not infer owner, reason, evidence, expiry, or approval".into(),
        ],
        expected_result_markers: expected_result_markers(primary_kind),
        ci_example_path: "docs/how-to/adopt-cargo-allow.md#step-3-ci-integration".into(),
        rollback_guide_path: "docs/how-to/rollback-cargo-allow-adoption.md".into(),
        limitations,
        claim_boundary: CLAIM_BOUNDARY,
    }
}

fn primary_route(
    facts: &AdoptionFacts,
    policy: &AdoptionPolicyFacts,
) -> (BootstrapDisposition, AdoptionActionKind) {
    if facts.instrument_failure.is_some() {
        return (
            BootstrapDisposition::InstrumentFailure,
            AdoptionActionKind::DiagnoseInventory,
        );
    }
    if facts.unsupported_repository_state || facts.inventory.mode == InventoryMode::Unknown {
        return (
            BootstrapDisposition::UnsupportedRepositoryState,
            AdoptionActionKind::DiagnoseInventory,
        );
    }
    if facts.inventory.completeness != InventoryCompleteness::Complete {
        return (
            BootstrapDisposition::PartialInventory,
            AdoptionActionKind::DiagnoseInventory,
        );
    }
    if policy.state == PolicyState::Invalid
        || policy.state == PolicyState::Unknown
        || facts.policy_config_diagnostic.is_some()
    {
        return (
            BootstrapDisposition::InvalidPolicy,
            AdoptionActionKind::RepairPolicy,
        );
    }
    if policy.state == PolicyState::Absent {
        return if policy.total_findings == 0 {
            (
                BootstrapDisposition::CleanNoPolicy,
                if facts.strict_gate_requested {
                    AdoptionActionKind::PreviewInit
                } else {
                    AdoptionActionKind::ContinueAdvisoryAudit
                },
            )
        } else {
            (
                BootstrapDisposition::FindingsNoPolicy,
                AdoptionActionKind::PreviewPropose,
            )
        };
    }
    if policy.new_unreceipted_findings > 0 {
        return (
            BootstrapDisposition::ExistingPolicyHasNewFindings,
            AdoptionActionKind::InspectNewFinding,
        );
    }
    if policy.has_repair_signal() {
        return (
            BootstrapDisposition::ExistingPolicyNeedsRepair,
            repair_action(policy),
        );
    }
    (
        BootstrapDisposition::ExistingPolicyHealthy,
        if facts.ci_guidance_completed {
            AdoptionActionKind::RunNoNewCheck
        } else {
            AdoptionActionKind::ConfigureCi
        },
    )
}

fn repair_action(policy: &AdoptionPolicyFacts) -> AdoptionActionKind {
    if policy.stale_entries > 0 {
        AdoptionActionKind::ApplyStaleSafeFindingPlan
    } else if policy.location_drift_entries > 0 {
        AdoptionActionKind::PreviewRefresh
    } else if policy.broken_evidence_entries > 0 {
        AdoptionActionKind::RepairEvidence
    } else if policy.review_due_entries > 0 || policy.expired_entries > 0 {
        AdoptionActionKind::InspectAllow
    } else if policy.mirror_divergence {
        AdoptionActionKind::ReconcileMirror
    } else {
        AdoptionActionKind::PreviewPrune
    }
}

fn action(kind: AdoptionActionKind, facts: &AdoptionFacts) -> AdoptionAction {
    let (argv, reason, write_posture, expected_result) = match kind {
        AdoptionActionKind::ContinueAdvisoryAudit => (
            vec!["cargo-allow".into(), "audit".into()],
            "inventory is clean and no policy exists; inspect findings before adopting a gate"
                .into(),
            WritePosture::ReadOnly,
            "an advisory report is produced and repository bytes remain unchanged".into(),
        ),
        AdoptionActionKind::PreviewInit => (
            vec!["cargo-allow".into(), "init".into(), "--dry-run".into()],
            "inventory is clean and a strict empty gate was explicitly requested".into(),
            WritePosture::PreviewOnly,
            "the empty-policy preview is reviewable without creating a policy".into(),
        ),
        AdoptionActionKind::PreviewPropose => (
            vec!["cargo-allow".into(), "propose".into()],
            "findings exist without a policy; preview candidates before retaining exceptions"
                .into(),
            WritePosture::PreviewOnly,
            "candidate exceptions are reported without changing the repository".into(),
        ),
        AdoptionActionKind::RunNoNewCheck | AdoptionActionKind::ConfigureCi => (
            vec![
                "cargo-allow".into(),
                "check".into(),
                "--mode".into(),
                "no-new".into(),
            ],
            if kind == AdoptionActionKind::ConfigureCi {
                "the policy is healthy but the complete no-new CI path is not recorded".into()
            } else {
                "the policy is healthy and CI guidance is already recorded".into()
            },
            WritePosture::ReadOnly,
            "the no-new gate evaluates the current source tree".into(),
        ),
        AdoptionActionKind::InspectNewFinding => (
            vec![
                "cargo-allow".into(),
                "why".into(),
                "<finding>".into(),
                "--plan".into(),
            ],
            "a new unreceipted finding needs an explainable plan before policy mutation".into(),
            WritePosture::ReadOnly,
            "the finding and its safe add-plan candidates are inspectable".into(),
        ),
        AdoptionActionKind::ApplyStaleSafeFindingPlan => (
            vec![
                "cargo-allow".into(),
                "add".into(),
                "--from-plan".into(),
                "<plan>".into(),
                "--update".into(),
            ],
            "a stale finding has an exact safe plan; apply only that plan, then run the full check"
                .into(),
            WritePosture::MayWrite,
            "only the selected policy entry may change, followed by a full check".into(),
        ),
        AdoptionActionKind::InspectAllow => (
            vec!["cargo-allow".into(), "explain".into(), "<allow-id>".into()],
            "a policy entry is due for review before any lifecycle change".into(),
            WritePosture::ReadOnly,
            "the selected entry's lifecycle and evidence are visible".into(),
        ),
        AdoptionActionKind::PreviewRefresh => (
            vec![
                "cargo-allow".into(),
                "refresh".into(),
                "--allow-id".into(),
                "<allow-id>".into(),
                "--dry-run".into(),
            ],
            "a location drift signal needs an exact refresh preview".into(),
            WritePosture::PreviewOnly,
            "the proposed location update is reviewable without policy mutation".into(),
        ),
        AdoptionActionKind::RepairEvidence => (
            vec![
                "cargo-allow".into(),
                "worklist".into(),
                "--broken-evidence".into(),
                "--format".into(),
                "json".into(),
            ],
            "broken evidence must be repaired or explicitly reviewed before adoption".into(),
            WritePosture::ReadOnly,
            "broken evidence references are listed for bounded repair".into(),
        ),
        AdoptionActionKind::PreviewPrune => (
            vec![
                "cargo-allow".into(),
                "prune".into(),
                "--stale".into(),
                "--dry-run".into(),
            ],
            "lifecycle or occurrence headroom requires a bounded prune preview".into(),
            WritePosture::PreviewOnly,
            "prune candidates are reported without deleting policy entries".into(),
        ),
        AdoptionActionKind::ReconcileMirror => (
            vec![
                "cargo-allow".into(),
                "doctor".into(),
                "--format".into(),
                "json".into(),
            ],
            "mirror divergence needs an exact read-only reconciliation before adoption".into(),
            WritePosture::ReadOnly,
            "ledger mirror divergence is visible without selecting a mutation".into(),
        ),
        AdoptionActionKind::DiagnoseInventory => (
            vec![
                "cargo-allow".into(),
                "doctor".into(),
                "--format".into(),
                "json".into(),
            ],
            if facts.instrument_failure.is_some() {
                "instrumentation failed; adoption remains fail-closed until the diagnostic is understood".into()
            } else if facts.unsupported_repository_state {
                "the repository state is unsupported; adoption remains fail-closed".into()
            } else {
                "inventory is incomplete; adoption remains fail-closed until coverage is understood"
                    .into()
            },
            WritePosture::ReadOnly,
            "diagnostic facts are emitted without generating policy".into(),
        ),
        AdoptionActionKind::RepairPolicy => (
            vec![
                "cargo-allow".into(),
                "doctor".into(),
                "--require-clean".into(),
            ],
            "policy validation failed; repair the policy before evaluating adoption".into(),
            WritePosture::ReadOnly,
            "the policy failure is diagnosed without changing policy bytes".into(),
        ),
    };
    AdoptionAction {
        kind,
        argv,
        reason,
        write_posture,
        expected_result,
    }
}

fn follow_up_actions(
    facts: &AdoptionFacts,
    policy: &AdoptionPolicyFacts,
    primary_kind: AdoptionActionKind,
) -> Vec<AdoptionAction> {
    let candidates = [
        AdoptionActionKind::ApplyStaleSafeFindingPlan,
        AdoptionActionKind::PreviewRefresh,
        AdoptionActionKind::RepairEvidence,
        AdoptionActionKind::InspectAllow,
        AdoptionActionKind::PreviewPrune,
        AdoptionActionKind::ReconcileMirror,
        AdoptionActionKind::RunNoNewCheck,
        AdoptionActionKind::ConfigureCi,
    ];
    candidates
        .into_iter()
        .filter(|kind| *kind != primary_kind)
        .filter(|kind| action_is_relevant(*kind, facts, policy))
        .map(|kind| action(kind, facts))
        .collect()
}

fn action_is_relevant(
    kind: AdoptionActionKind,
    facts: &AdoptionFacts,
    policy: &AdoptionPolicyFacts,
) -> bool {
    match kind {
        AdoptionActionKind::ApplyStaleSafeFindingPlan => policy.stale_entries > 0,
        AdoptionActionKind::PreviewRefresh => policy.location_drift_entries > 0,
        AdoptionActionKind::RepairEvidence => policy.broken_evidence_entries > 0,
        AdoptionActionKind::InspectAllow => {
            policy.review_due_entries > 0 || policy.expired_entries > 0
        }
        AdoptionActionKind::PreviewPrune => {
            policy.occurrence_headroom_entries > 0 || policy.mirror_divergence
        }
        AdoptionActionKind::ReconcileMirror => policy.mirror_divergence,
        AdoptionActionKind::RunNoNewCheck => {
            policy.state == PolicyState::Valid
                && facts.inventory.completeness == InventoryCompleteness::Complete
        }
        AdoptionActionKind::ConfigureCi => !facts.ci_guidance_completed,
        _ => false,
    }
}

fn expected_result_markers(kind: AdoptionActionKind) -> Vec<String> {
    let mut markers = vec![format!("primary_action={}", kind.as_str())];
    if matches!(
        kind,
        AdoptionActionKind::DiagnoseInventory | AdoptionActionKind::RepairPolicy
    ) {
        markers.push("adoption_disposition=fail_closed".into());
    } else if matches!(
        kind,
        AdoptionActionKind::PreviewInit
            | AdoptionActionKind::PreviewPropose
            | AdoptionActionKind::PreviewRefresh
            | AdoptionActionKind::PreviewPrune
    ) {
        markers.push("write_posture=preview_only".into());
    } else {
        markers.push("write_posture=read_only_or_bounded_write".into());
    }
    markers
}

fn portable_path(root: &str, path: &str) -> String {
    let normalized_root = root.replace('\\', "/").trim_end_matches('/').to_string();
    let normalized_path = path.replace('\\', "/");
    let prefix = format!("{normalized_root}/");
    if normalized_path.eq_ignore_ascii_case(&normalized_root) {
        PORTABLE_ROOT.into()
    } else if normalized_path
        .get(..prefix.len())
        .is_some_and(|value| value.eq_ignore_ascii_case(&prefix))
    {
        normalized_path
            .get(prefix.len()..)
            .map(str::to_string)
            .unwrap_or_else(|| "<external-path>".into())
    } else if Path::new(path).is_absolute() {
        "<external-path>".into()
    } else {
        normalized_path.trim_start_matches("./").to_string()
    }
}
