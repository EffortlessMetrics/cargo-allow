//! Intent edit planning and repo-edit settlement for three-product extraction
//! (#2613).
//!
//! Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow);
//! `intent-edit` plans intent-shaped edits, adapts dialects, and translates
//! approved actions into `repo-edit` apply requests. It does not scan source
//! files, does not invoke Cargo, compile code, execute repository artifacts,
//! or run proof commands.

mod approval_currentness;
mod approval_currentness_surface;
mod boundary;
mod dialect_adapter;
mod dialect_adapter_surface;
mod edit_plan;
mod edit_plan_surface;
mod parity;
mod recompile_contract;
mod recompile_contract_surface;
mod repo_edit_translation;
mod repo_edit_translation_surface;
mod settlement;
mod settlement_surface;

#[cfg(test)]
mod tests;

pub use approval_currentness::{
    ApprovalCurrentnessError, INTENT_EDIT_APPROVAL_CURRENTNESS_SCHEMA_ID,
    IntentEditApprovalCurrentnessV1, IntentEditApprovalStateV1, validate_approval_currentness,
};
pub use approval_currentness_surface::ApprovalCurrentnessSurface;
pub use boundary::{
    ALLOWED_UPSTREAM_CRATES, BoundarySurface, EVALUATOR_PACKET_MODULE_ID,
    FORBIDDEN_DEPENDENCY_EDGES, upstream_surface_markers,
};
pub use dialect_adapter::{
    CANONICAL_DIALECT_IDS, DialectAdapterError, INTENT_EDIT_DIALECT_ADAPTER_SCHEMA_ID,
    IntentEditDialectV1, adapt_selector,
};
pub use dialect_adapter_surface::DialectAdapterSurface;
pub use edit_plan::{
    INTENT_EDIT_PLAN_SCHEMA_ID, IntentEditActionKindV1, IntentEditActionV1, IntentEditPlanError,
    IntentEditPlanV1, IntentEditTargetResolutionV1, stable_action_id, validate_edit_plan,
};
pub use edit_plan_surface::EditPlanSurface;
pub use parity::{
    ApprovalCurrentnessParityContract, DialectAdapterParityContract, EditPlanParityContract,
    RecompileContractParityContract, RepoEditTranslationParityContract, SettlementParityContract,
    approval_currentness_parity_contract_path, approval_currentness_parity_contract_paths,
    dialect_adapter_parity_contract_path, dialect_adapter_parity_contract_paths,
    edit_plan_parity_contract_path, edit_plan_parity_contract_paths,
    load_approval_currentness_parity_contract, load_dialect_adapter_parity_contract,
    load_edit_plan_parity_contract, load_recompile_contract_parity_contract,
    load_repo_edit_translation_parity_contract, load_settlement_parity_contract,
    parity_contract_path, parity_contract_paths, recompile_contract_parity_contract_path,
    recompile_contract_parity_contract_paths, settlement_parity_contract_path,
    settlement_parity_contract_paths,
};
pub use recompile_contract::{
    INTENT_EDIT_RECOMPILE_CONTRACT_SCHEMA_ID, IntentEditRecompileContractV1,
    IntentEditRecompileObligationV1, PRECOMMIT_PHASE_ID, PhaseObligationTransportPlanV1,
    RecompileContractError, RecompileObligationKindV1, TARGET_PHASE_OBLIGATION_PLAN_SCHEMA_ID,
    compile_recompile_contract, validate_recompile_contract,
};
pub use recompile_contract_surface::RecompileContractSurface;
pub use repo_edit_translation::{
    INTENT_EDIT_REPO_EDIT_TRANSLATION_SCHEMA_ID, RepoEditTranslationError,
    RepoEditTranslationPlanV1, RepoEditTranslationRequestV1, translate_plan_to_repo_edit,
};
pub use repo_edit_translation_surface::RepoEditTranslationSurface;
pub use settlement::{
    INTENT_EDIT_SETTLEMENT_PLAN_SCHEMA_ID, IntentEditResidualObligationKindV1,
    IntentEditResidualObligationV1, IntentEditSettlementPlanV1, SettlementError,
    compile_settlement_plan, validate_settlement_plan,
};
pub use settlement_surface::SettlementSurface;
