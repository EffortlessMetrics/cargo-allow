//! Proof orchestration engine for three-product extraction (#2589).
//!
//! Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow);
//! `proof-engine` orchestrates provider registry, captured receipts, obligation
//! planning, currentness, dry-run projection, explicit execution gates, cache,
//! contradiction detection, and phase gates. It also absorbs the provider API
//! contracts (#2603-A) and command adapter contracts (#2603-B) that previously
//! lived in standalone `proof-provider-api` and `proof-adapter-command` crates
//! (#2937). It does not scan source files, does not invoke Cargo, compile code,
//! execute repository code, spawn processes, or depend on intent crates.

mod boundary;
mod cache;
mod cache_surface;
mod captured_receipts;
mod captured_receipts_surface;
mod command_adapter;
mod contradiction;
mod contradiction_surface;
mod currentness;
mod currentness_surface;
mod dry_run;
mod dry_run_surface;
mod engine_surface;
mod execution;
mod execution_surface;
mod obligation_plan;
mod obligation_plan_surface;
mod parity;
mod phase_gate;
mod phase_gate_surface;
mod planner;
mod planner_surface;
mod provider_api;
mod provider_registry;
mod provider_registry_surface;
mod ripr_routing;
mod ripr_routing_surface;

#[cfg(test)]
mod tests;

pub use boundary::{
    ALLOWED_UPSTREAM_CRATES, BoundarySurface, FORBIDDEN_DEPENDENCY_EDGES, upstream_surface_markers,
};
pub use cache::{
    CacheError, PROOF_CACHE_SCHEMA_ID, ProofCacheEntryV1, ProofCacheV1, cache_key_for_plan,
    validate_proof_cache,
};
pub use cache_surface::CacheSurface;
pub use captured_receipts::{
    CAPTURED_RECEIPT_STORE_SCHEMA_ID, CapturedReceiptError, CapturedReceiptStoreV1,
    validate_captured_receipt_store,
};
pub use captured_receipts_surface::CapturedReceiptsSurface;
// Re-export the absorbed command adapter contracts (#2937). Prefixed aliases
// avoid collisions with proof-engine's own boundary/dry_run surface markers.
pub use command_adapter::{
    ALLOWED_UPSTREAM_CRATES as COMMAND_ADAPTER_ALLOWED_UPSTREAM_CRATES,
    BoundarySurface as CommandAdapterBoundarySurface, COMMAND_INVOCATION_SPEC_SCHEMA_ID,
    COMMAND_RECEIPT_OUTCOME_SCHEMA_ID, COMMAND_REGISTRY_SCHEMA_ID, CancellationPostureV1,
    CommandInvocationSpecV1, CommandReceiptOutcomeV1, CommandReceiptStatusV1, CommandRegistryError,
    CommandRegistrySurface, CommandSourceKindV1, CommandSpecError, CommandSpecSurface, CwdPolicyV1,
    DRY_RUN_COMMAND_REPORT_SCHEMA_ID, DryRunCommandReportV1,
    DryRunSurface as CommandAdapterDryRunSurface,
    FORBIDDEN_DEPENDENCY_EDGES as COMMAND_ADAPTER_FORBIDDEN_DEPENDENCY_EDGES, NetworkAccessV1,
    ReceiptInterpretationSurface, ReviewedCommandEntryV1, ReviewedCommandRegistryV1,
    ShellProjectionKindV1, command_registry_parity_contract_path,
    command_registry_parity_contract_paths, compile_invocation_spec, default_cargo_allow_registry,
    interpret_receipt_binding, load_command_registry_parity_contract,
    parity_contract_path as command_adapter_parity_contract_path,
    parity_contract_paths as command_adapter_parity_contract_paths, reject_prose_as_executable,
    render_structured_argv, upstream_surface_markers as command_adapter_upstream_surface_markers,
    validate_command_registry,
};
pub use contradiction::{
    ContradictionError, detect_contradictions, validate_engine_contradiction_report,
};
pub use contradiction_surface::ContradictionSurface;
pub use currentness::{
    CURRENTNESS_REPORT_SCHEMA_ID, CurrentnessError, CurrentnessReportV1, CurrentnessStatusV1,
    evaluate_currentness, receipt_set_digest,
};
pub use currentness_surface::CurrentnessSurface;
pub use dry_run::{
    DRY_RUN_PLAN_REPORT_SCHEMA_ID, DryRunError, DryRunPlanLineV1, DryRunPlanReportV1,
    dry_run_proof_plan,
};
pub use dry_run_surface::DryRunSurface;
pub use engine_surface::EngineSurface;
pub use execution::{
    EXECUTION_GATE_SCHEMA_ID, ExecutionApprovalV1, ExecutionError, ExecutionGateReportV1,
    evaluate_execution_gate, require_explicit_execution,
};
pub use execution_surface::ExecutionSurface;
pub use obligation_plan::{
    CHANGE_OBLIGATION_PLAN_SCHEMA_ID, ChangeObligationPlanV1, ChangeObligationV1,
    ObligationPlanError, load_obligation_plan_toml, validate_obligation_plan,
};
pub use obligation_plan_surface::ObligationPlanSurface;
pub use parity::{
    RiprRoutingParityContract, load_ripr_routing_contract, parity_contract_path,
    parity_contract_paths, ripr_routing_contract_path,
};
pub use phase_gate::{
    PHASE_GATE_EVALUATION_SCHEMA_ID, PhaseGateError, PhaseGateEvaluationV1, PhaseGateOutcomeV1,
    evaluate_phase_gate,
};
pub use phase_gate_surface::PhaseGateSurface;
pub use planner::{PROOF_PLANNER_SCHEMA_ID, PlannerError, plan_proof_execution};
pub use planner_surface::PlannerSurface;
// Re-export the absorbed provider API contracts (#2937).
pub use provider_api::{
    ALLOWED_UPSTREAM_CRATES as PROVIDER_API_ALLOWED_UPSTREAM_CRATES,
    BoundarySurface as ProviderApiBoundarySurface, CONFORMANCE_SCENARIO_ID, FAKE_PROOF_PROVIDER_ID,
    FORBIDDEN_DEPENDENCY_EDGES as PROVIDER_API_FORBIDDEN_DEPENDENCY_EDGES, FakeProofProviderV1,
    PROOF_PROVIDER_API_SCHEMA_ID, ProofProviderV1, ProviderApiError,
    parity_contract_path as provider_api_parity_contract_path,
    parity_contract_paths as provider_api_parity_contract_paths, run_fake_provider_conformance,
    run_provider_conformance, upstream_surface_markers as provider_api_upstream_surface_markers,
    validate_provider_plan, validate_provider_surface,
};
pub use provider_registry::{
    PROVIDER_REGISTRY_SCHEMA_ID, ProviderRegistryEntryV1, ProviderRegistryError,
    ProviderRegistryV1, register_validated_provider, require_registered_provider,
    validate_provider_registry,
};
pub use provider_registry_surface::ProviderRegistrySurface;
pub use ripr_routing::{
    PHASE_PREFLIGHT, PHASE_ROUTE, ProofClaimPostureV1, ProofClaimV1,
    RIPR_PREFLIGHT_RECEIPT_SCHEMA_ID, RIPR_ROUTE_RECEIPT_SCHEMA_ID,
    RIPR_ROUTING_AGGREGATE_SCHEMA_ID, RiprPreflightClaimInputV1, RiprPreflightReceiptV1,
    RiprRouteClaimInputV1, RiprRouteReceiptV1, RiprRoutingAggregateV1, RiprRoutingError,
    compose_preflight_receipt, compose_route_receipt, compose_routing_aggregate,
    preflight_claim_result_state, route_claim_result_state,
};
pub use ripr_routing_surface::RiprRoutingSurface;
