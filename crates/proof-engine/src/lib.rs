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

#[cfg(test)]
mod boundary;
mod cache;
mod captured_receipts;
mod command_adapter;
mod contradiction;
mod corpus_semantics;
mod currentness;
mod dry_run;
mod execution;
mod obligation_plan;
mod parity;
mod phase_gate;
mod planner;
mod provider_api;
mod provider_registry;
mod ripr_routing;
mod subject_reconciliation;

#[cfg(test)]
mod tests;

pub use cache::{
    CacheError, PROOF_CACHE_SCHEMA_ID, ProofCacheEntryV1, ProofCacheV1, cache_key_for_plan,
    validate_proof_cache,
};
pub use captured_receipts::{
    CAPTURED_RECEIPT_STORE_SCHEMA_ID, CapturedReceiptError, CapturedReceiptStoreV1,
    validate_captured_receipt_store,
};
pub use corpus_semantics::{
    canonical_proof_result_states, compose_blocking_aggregate, evaluate_binding_currentness,
    provider_envelope_distinct, validate_composition_honesty, validate_proof_corpus,
    validate_provider_envelope,
};
// Re-export the absorbed command adapter contracts (#2937).
pub use command_adapter::{
    COMMAND_INVOCATION_SPEC_SCHEMA_ID, COMMAND_RECEIPT_OUTCOME_SCHEMA_ID,
    COMMAND_REGISTRY_SCHEMA_ID, CancellationPostureV1, CommandInvocationSpecV1,
    CommandReceiptOutcomeV1, CommandReceiptStatusV1, CommandRegistryError, CommandSourceKindV1,
    CommandSpecError, CwdPolicyV1, DRY_RUN_COMMAND_REPORT_SCHEMA_ID, DryRunCommandReportV1,
    NetworkAccessV1, ReviewedCommandEntryV1, ReviewedCommandRegistryV1, ShellProjectionKindV1,
    command_registry_parity_contract_path, command_registry_parity_contract_paths,
    compile_invocation_spec, default_cargo_allow_registry, interpret_receipt_binding,
    load_command_registry_parity_contract,
    parity_contract_path as command_adapter_parity_contract_path,
    parity_contract_paths as command_adapter_parity_contract_paths, reject_prose_as_executable,
    render_structured_argv, validate_command_registry,
};
pub use contradiction::{
    ContradictionError, detect_contradictions, validate_engine_contradiction_report,
};
pub use currentness::{
    CURRENTNESS_REPORT_SCHEMA_ID, CurrentnessError, CurrentnessReportV1, evaluate_currentness,
    receipt_set_digest,
};
pub use dry_run::{
    DRY_RUN_PLAN_REPORT_SCHEMA_ID, DryRunError, DryRunPlanLineV1, DryRunPlanReportV1,
    dry_run_proof_plan,
};
pub use execution::{
    EXECUTION_GATE_SCHEMA_ID, ExecutionApprovalV1, ExecutionError, ExecutionGateReportV1,
    evaluate_execution_gate, require_explicit_execution,
};
pub use obligation_plan::{
    CHANGE_OBLIGATION_PLAN_SCHEMA_ID, ChangeObligationPlanV1, ChangeObligationV1,
    ObligationPlanError, load_obligation_plan_toml, validate_obligation_plan,
};
pub use parity::{
    RiprRoutingParityContract, load_ripr_routing_contract, parity_contract_path,
    parity_contract_paths, ripr_routing_contract_path,
};
pub use phase_gate::{
    PHASE_GATE_EVALUATION_SCHEMA_ID, PhaseGateError, PhaseGateEvaluationV1, PhaseGateOutcomeV1,
    evaluate_phase_gate,
};
pub use planner::{PROOF_PLANNER_SCHEMA_ID, PlannerError, plan_proof_execution};
// Re-export the absorbed provider API contracts (#2937).
pub use provider_api::{
    CONFORMANCE_SCENARIO_ID, FAKE_PROOF_PROVIDER_ID, FakeProofProviderV1,
    PROOF_PROVIDER_API_SCHEMA_ID, ProofProviderV1, ProviderApiError,
    parity_contract_path as provider_api_parity_contract_path,
    parity_contract_paths as provider_api_parity_contract_paths, run_fake_provider_conformance,
    run_provider_conformance, validate_provider_plan, validate_provider_surface,
};
pub use provider_registry::{
    PROVIDER_REGISTRY_SCHEMA_ID, ProviderRegistryEntryV1, ProviderRegistryError,
    ProviderRegistryV1, register_validated_provider, require_registered_provider,
    validate_provider_registry,
};
pub use ripr_routing::{
    PHASE_PREFLIGHT, PHASE_ROUTE, ProofClaimPostureV1, ProofClaimV1,
    RIPR_PREFLIGHT_RECEIPT_SCHEMA_ID, RIPR_ROUTE_RECEIPT_SCHEMA_ID,
    RIPR_ROUTING_AGGREGATE_SCHEMA_ID, RiprPreflightClaimInputV1, RiprPreflightReceiptV1,
    RiprRouteClaimInputV1, RiprRouteReceiptV1, RiprRoutingAggregateV1, RiprRoutingError,
    compose_preflight_receipt, compose_route_receipt, compose_routing_aggregate,
    preflight_claim_result_state, route_claim_result_state,
};
pub use subject_reconciliation::{
    ObservedRustSubjectV1, ProofSubjectReconciliationClassV1, ProofSubjectReconciliationV1,
    reconcile_rust_subject_binding,
};
