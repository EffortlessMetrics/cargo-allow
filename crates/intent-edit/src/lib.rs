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
    approval_currentness_parity_contract_path, approval_currentness_parity_contract_paths,
    dialect_adapter_parity_contract_path, dialect_adapter_parity_contract_paths,
    edit_plan_parity_contract_path, edit_plan_parity_contract_paths,
    load_approval_currentness_parity_contract, load_dialect_adapter_parity_contract,
    load_edit_plan_parity_contract, parity_contract_path, parity_contract_paths,
};
