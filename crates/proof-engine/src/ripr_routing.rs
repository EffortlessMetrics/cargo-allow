//! RIPR route/preflight composition consuming the external proof corpus (#2713).
//!
//! Wires planner, execution gate, currentness, and phase-gate outcomes into
//! corpus-backed claim receipts and a required aggregate. Does not spawn
//! processes, access the network, or execute real RIPR providers.

use proof_protocol::{PROOF_CORPUS_DIGEST_V1, ProofResultStateV1, RIPR_EXTERNAL_PROOF_PROFILE_ID};

use crate::corpus_semantics::compose_blocking_aggregate;

use crate::execution::ExecutionApprovalV1;
use crate::phase_gate::PhaseGateOutcomeV1;
use proof_protocol::BindingCurrentnessV1;

pub const RIPR_ROUTE_RECEIPT_SCHEMA_ID: &str = "proof.ripr-route-receipt.v1";
pub const RIPR_PREFLIGHT_RECEIPT_SCHEMA_ID: &str = "proof.ripr-preflight-receipt.v1";
pub const RIPR_ROUTING_AGGREGATE_SCHEMA_ID: &str = "proof.ripr-routing-aggregate.v1";

pub const PHASE_ROUTE: &str = "route";
pub const PHASE_PREFLIGHT: &str = "preflight";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProofClaimPostureV1 {
    Required,
    Advisory,
}

impl ProofClaimPostureV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Required => "required",
            Self::Advisory => "advisory",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofClaimV1 {
    pub claim_id: String,
    pub proof_reference_id: String,
    pub result_state: ProofResultStateV1,
    pub posture: ProofClaimPostureV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiprRouteClaimInputV1 {
    pub claim_id: String,
    pub proof_reference_id: String,
    pub posture: ProofClaimPostureV1,
    pub selected: bool,
    pub provider_registered: bool,
    pub execution_approval: ExecutionApprovalV1,
    pub provider_executed: bool,
    pub provider_passed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiprRouteReceiptV1 {
    pub schema_id: String,
    pub profile_id: String,
    pub corpus_digest: String,
    pub phase_id: String,
    pub repo_snapshot_id: String,
    pub plan_id: String,
    pub claims: Vec<ProofClaimV1>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiprPreflightClaimInputV1 {
    pub claim_id: String,
    pub proof_reference_id: String,
    pub posture: ProofClaimPostureV1,
    pub currentness: BindingCurrentnessV1,
    pub gate_outcome: PhaseGateOutcomeV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiprPreflightReceiptV1 {
    pub schema_id: String,
    pub profile_id: String,
    pub corpus_digest: String,
    pub phase_id: String,
    pub repo_snapshot_id: String,
    pub plan_id: String,
    pub claims: Vec<ProofClaimV1>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiprRoutingAggregateV1 {
    pub schema_id: String,
    pub profile_id: String,
    pub corpus_digest: String,
    pub repo_snapshot_id: String,
    pub plan_id: String,
    pub route_claims: Vec<ProofClaimV1>,
    pub preflight_claims: Vec<ProofClaimV1>,
    pub required_aggregate: ProofResultStateV1,
    pub advisory_aggregate: ProofResultStateV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RiprRoutingError {
    EmptyRouteClaims,
    EmptyPreflightClaims,
    DuplicateClaimId {
        claim_id: String,
    },
    HonestyViolation {
        claim_id: String,
        state: ProofResultStateV1,
    },
}

impl RiprRoutingError {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::EmptyRouteClaims => "empty_route_claims",
            Self::EmptyPreflightClaims => "empty_preflight_claims",
            Self::DuplicateClaimId { .. } => "duplicate_claim_id",
            Self::HonestyViolation { .. } => "honesty_violation",
        }
    }
}

pub fn route_claim_result_state(input: &RiprRouteClaimInputV1) -> ProofResultStateV1 {
    if !input.selected {
        return ProofResultStateV1::NotSelected;
    }
    if !input.provider_registered {
        return ProofResultStateV1::ProviderUnavailable;
    }
    if input.execution_approval == ExecutionApprovalV1::Denied {
        return ProofResultStateV1::SelectedNotRun;
    }
    if !input.provider_executed {
        return ProofResultStateV1::SelectedNotRun;
    }
    if input.provider_passed {
        ProofResultStateV1::ProofPassed
    } else {
        ProofResultStateV1::ProofFailed
    }
}

pub fn preflight_claim_result_state(input: &RiprPreflightClaimInputV1) -> ProofResultStateV1 {
    match input.currentness {
        BindingCurrentnessV1::Missing => ProofResultStateV1::Missing,
        BindingCurrentnessV1::Stale => ProofResultStateV1::Stale,
        BindingCurrentnessV1::Incomparable => ProofResultStateV1::Incomparable,
        BindingCurrentnessV1::Current => match input.gate_outcome {
            PhaseGateOutcomeV1::Open => ProofResultStateV1::ProofPassed,
            PhaseGateOutcomeV1::Blocked => ProofResultStateV1::Missing,
            PhaseGateOutcomeV1::Advisory => ProofResultStateV1::ProofPassed,
        },
    }
}

pub fn compose_route_receipt(
    repo_snapshot_id: &str,
    plan_id: &str,
    inputs: &[RiprRouteClaimInputV1],
) -> Result<RiprRouteReceiptV1, RiprRoutingError> {
    if inputs.is_empty() {
        return Err(RiprRoutingError::EmptyRouteClaims);
    }
    let claims = build_claims(inputs, route_claim_result_state)?;
    Ok(RiprRouteReceiptV1 {
        schema_id: RIPR_ROUTE_RECEIPT_SCHEMA_ID.to_string(),
        profile_id: RIPR_EXTERNAL_PROOF_PROFILE_ID.to_string(),
        corpus_digest: PROOF_CORPUS_DIGEST_V1.to_string(),
        phase_id: PHASE_ROUTE.to_string(),
        repo_snapshot_id: repo_snapshot_id.to_string(),
        plan_id: plan_id.to_string(),
        claims,
    })
}

pub fn compose_preflight_receipt(
    repo_snapshot_id: &str,
    plan_id: &str,
    inputs: &[RiprPreflightClaimInputV1],
) -> Result<RiprPreflightReceiptV1, RiprRoutingError> {
    if inputs.is_empty() {
        return Err(RiprRoutingError::EmptyPreflightClaims);
    }
    let claims = build_claims(inputs, preflight_claim_result_state)?;
    Ok(RiprPreflightReceiptV1 {
        schema_id: RIPR_PREFLIGHT_RECEIPT_SCHEMA_ID.to_string(),
        profile_id: RIPR_EXTERNAL_PROOF_PROFILE_ID.to_string(),
        corpus_digest: PROOF_CORPUS_DIGEST_V1.to_string(),
        phase_id: PHASE_PREFLIGHT.to_string(),
        repo_snapshot_id: repo_snapshot_id.to_string(),
        plan_id: plan_id.to_string(),
        claims,
    })
}

pub fn compose_routing_aggregate(
    repo_snapshot_id: &str,
    plan_id: &str,
    route: &RiprRouteReceiptV1,
    preflight: &RiprPreflightReceiptV1,
) -> Result<RiprRoutingAggregateV1, RiprRoutingError> {
    validate_receipt_profile(route.profile_id.as_str(), route.corpus_digest.as_str())?;
    validate_receipt_profile(
        preflight.profile_id.as_str(),
        preflight.corpus_digest.as_str(),
    )?;
    let required_states = required_claim_states(route, preflight);
    let advisory_states = advisory_claim_states(route, preflight);
    let required_aggregate = compose_blocking_aggregate(&required_states);
    let advisory_aggregate = compose_blocking_aggregate(&advisory_states);
    validate_aggregate_honesty(&required_aggregate)?;
    Ok(RiprRoutingAggregateV1 {
        schema_id: RIPR_ROUTING_AGGREGATE_SCHEMA_ID.to_string(),
        profile_id: RIPR_EXTERNAL_PROOF_PROFILE_ID.to_string(),
        corpus_digest: PROOF_CORPUS_DIGEST_V1.to_string(),
        repo_snapshot_id: repo_snapshot_id.to_string(),
        plan_id: plan_id.to_string(),
        route_claims: route.claims.clone(),
        preflight_claims: preflight.claims.clone(),
        required_aggregate,
        advisory_aggregate,
    })
}

fn build_claims<T>(
    inputs: &[T],
    map_state: fn(&T) -> ProofResultStateV1,
) -> Result<Vec<ProofClaimV1>, RiprRoutingError>
where
    T: ClaimInput,
{
    let mut claims = Vec::with_capacity(inputs.len());
    let mut seen = std::collections::BTreeSet::new();
    for input in inputs {
        if !seen.insert(input.claim_id().to_string()) {
            return Err(RiprRoutingError::DuplicateClaimId {
                claim_id: input.claim_id().to_string(),
            });
        }
        let result_state = map_state(input);
        if result_state.is_non_execution() && result_state.allows_passed_composition() {
            return Err(RiprRoutingError::HonestyViolation {
                claim_id: input.claim_id().to_string(),
                state: result_state,
            });
        }
        claims.push(ProofClaimV1 {
            claim_id: input.claim_id().to_string(),
            proof_reference_id: input.proof_reference_id().to_string(),
            result_state,
            posture: input.posture(),
        });
    }
    Ok(claims)
}

trait ClaimInput {
    fn claim_id(&self) -> &str;
    fn proof_reference_id(&self) -> &str;
    fn posture(&self) -> ProofClaimPostureV1;
}

impl ClaimInput for RiprRouteClaimInputV1 {
    fn claim_id(&self) -> &str {
        &self.claim_id
    }
    fn proof_reference_id(&self) -> &str {
        &self.proof_reference_id
    }
    fn posture(&self) -> ProofClaimPostureV1 {
        self.posture
    }
}

impl ClaimInput for RiprPreflightClaimInputV1 {
    fn claim_id(&self) -> &str {
        &self.claim_id
    }
    fn proof_reference_id(&self) -> &str {
        &self.proof_reference_id
    }
    fn posture(&self) -> ProofClaimPostureV1 {
        self.posture
    }
}

fn required_claim_states(
    route: &RiprRouteReceiptV1,
    preflight: &RiprPreflightReceiptV1,
) -> Vec<ProofResultStateV1> {
    route
        .claims
        .iter()
        .chain(preflight.claims.iter())
        .filter(|claim| claim.posture == ProofClaimPostureV1::Required)
        .map(|claim| claim.result_state)
        .collect()
}

fn advisory_claim_states(
    route: &RiprRouteReceiptV1,
    preflight: &RiprPreflightReceiptV1,
) -> Vec<ProofResultStateV1> {
    route
        .claims
        .iter()
        .chain(preflight.claims.iter())
        .filter(|claim| claim.posture == ProofClaimPostureV1::Advisory)
        .map(|claim| claim.result_state)
        .collect()
}

fn validate_receipt_profile(profile_id: &str, corpus_digest: &str) -> Result<(), RiprRoutingError> {
    if profile_id != RIPR_EXTERNAL_PROOF_PROFILE_ID {
        return Err(RiprRoutingError::HonestyViolation {
            claim_id: "profile".to_string(),
            state: ProofResultStateV1::Incomparable,
        });
    }
    if corpus_digest != PROOF_CORPUS_DIGEST_V1 {
        return Err(RiprRoutingError::HonestyViolation {
            claim_id: "corpus_digest".to_string(),
            state: ProofResultStateV1::Incomparable,
        });
    }
    Ok(())
}

fn validate_aggregate_honesty(aggregate: &ProofResultStateV1) -> Result<(), RiprRoutingError> {
    if aggregate.is_non_execution() && aggregate.allows_passed_composition() {
        return Err(RiprRoutingError::HonestyViolation {
            claim_id: "required_aggregate".to_string(),
            state: *aggregate,
        });
    }
    Ok(())
}
