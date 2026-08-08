//! Semantic evaluation functions moved from proof-protocol (#2943).
//!
//! These functions operate on proof-protocol DTOs but contain semantic logic
//! (composition rules, currentness evaluation, honesty validation) that belongs
//! in the engine layer, not the data-only protocol crate.

use proof_protocol::{
    BindingCurrentnessV1, ProofBindingIdentityV1, ProofCorpusDimensionV1, ProofCorpusV1,
    ProofResultStateV1, ProviderEnvelopeV1, RIPR_EXTERNAL_PROOF_PROFILE_ID,
};

/// Canonical set of proof result states used for corpus coverage checks.
pub fn canonical_proof_result_states() -> &'static [ProofResultStateV1] {
    &[
        ProofResultStateV1::NotSelected,
        ProofResultStateV1::SelectedNotRun,
        ProofResultStateV1::ProviderUnavailable,
        ProofResultStateV1::ProviderTempfail,
        ProofResultStateV1::ProofFailed,
        ProofResultStateV1::ProofPassed,
        ProofResultStateV1::Missing,
        ProofResultStateV1::Stale,
        ProofResultStateV1::Incomparable,
        ProofResultStateV1::Opaque,
        ProofResultStateV1::Unsupported,
    ]
}

/// Validate that a non-execution state does not claim passed composition (#2943).
pub fn validate_composition_honesty(state: ProofResultStateV1) -> Result<(), String> {
    if state.is_non_execution() && state.allows_passed_composition() {
        return Err(format!(
            "non-execution state {} cannot allow passed composition",
            state.as_str()
        ));
    }
    Ok(())
}

/// Compose multiple proof result states into a blocking aggregate (#2943).
///
/// The aggregate is pessimistic: any Incomparable, Missing, Stale, or failure
/// state dominates. Only all-Passed produces Passed.
pub fn compose_blocking_aggregate(states: &[ProofResultStateV1]) -> ProofResultStateV1 {
    if states.is_empty() {
        return ProofResultStateV1::Missing;
    }
    if states
        .iter()
        .any(|state| matches!(state, ProofResultStateV1::Incomparable))
    {
        return ProofResultStateV1::Incomparable;
    }
    if states
        .iter()
        .any(|state| matches!(state, ProofResultStateV1::Missing))
    {
        return ProofResultStateV1::Missing;
    }
    if states
        .iter()
        .any(|state| matches!(state, ProofResultStateV1::Stale))
    {
        return ProofResultStateV1::Stale;
    }
    if states.iter().any(|state| state.is_non_execution()) {
        return ProofResultStateV1::SelectedNotRun;
    }
    if states
        .iter()
        .any(|state| matches!(state, ProofResultStateV1::ProviderUnavailable))
    {
        return ProofResultStateV1::ProviderUnavailable;
    }
    if states
        .iter()
        .any(|state| matches!(state, ProofResultStateV1::ProviderTempfail))
    {
        return ProofResultStateV1::ProviderTempfail;
    }
    if states
        .iter()
        .any(|state| matches!(state, ProofResultStateV1::ProofFailed))
    {
        return ProofResultStateV1::ProofFailed;
    }
    if states
        .iter()
        .all(|state| matches!(state, ProofResultStateV1::ProofPassed))
    {
        return ProofResultStateV1::ProofPassed;
    }
    ProofResultStateV1::Opaque
}

/// Evaluate whether an observed binding identity is current, stale, or incomparable (#2943).
pub fn evaluate_binding_currentness(
    expected: &ProofBindingIdentityV1,
    observed: Option<&ProofBindingIdentityV1>,
) -> BindingCurrentnessV1 {
    let Some(observed) = observed else {
        return BindingCurrentnessV1::Missing;
    };
    if expected.repo_snapshot_id != observed.repo_snapshot_id
        || expected.config_digest != observed.config_digest
        || expected.tool_identity != observed.tool_identity
    {
        return BindingCurrentnessV1::Incomparable;
    }
    if expected.phase_id != observed.phase_id
        || expected.proof_reference_id != observed.proof_reference_id
    {
        return BindingCurrentnessV1::Stale;
    }
    BindingCurrentnessV1::Current
}

/// Validate that a provider envelope has required fields (#2943).
pub fn validate_provider_envelope(envelope: &ProviderEnvelopeV1) -> Result<(), String> {
    if envelope.provider_id.is_empty() {
        return Err("provider_id must not be empty".to_string());
    }
    if envelope.envelope_namespace.is_empty() {
        return Err("envelope_namespace must not be empty".to_string());
    }
    if !envelope.envelope_namespace.contains("::") {
        return Err(format!(
            "envelope_namespace {} must be namespaced",
            envelope.envelope_namespace
        ));
    }
    if envelope.result_class.is_empty() {
        return Err("result_class must not be empty".to_string());
    }
    if envelope.payload_digest.is_empty() {
        return Err("payload_digest must not be empty".to_string());
    }
    Ok(())
}

/// Check whether two provider envelopes are distinct (#2943).
pub fn provider_envelope_distinct(left: &ProviderEnvelopeV1, right: &ProviderEnvelopeV1) -> bool {
    left.envelope_namespace != right.envelope_namespace
        || left.result_class != right.result_class
        || left.payload_digest != right.payload_digest
}

/// Validate proof corpus coverage including semantic composition honesty checks (#2943).
pub fn validate_proof_corpus(corpus: &ProofCorpusV1) -> Result<(), String> {
    if corpus.profile_id != RIPR_EXTERNAL_PROOF_PROFILE_ID {
        return Err(format!("unexpected proof profile_id {}", corpus.profile_id));
    }
    if corpus.scenarios.is_empty() {
        return Err("proof corpus must include scenarios".to_string());
    }
    let mut ids = std::collections::BTreeSet::new();
    for scenario in &corpus.scenarios {
        if scenario.id.is_empty() {
            return Err("scenario id must not be empty".to_string());
        }
        if !ids.insert(scenario.id.clone()) {
            return Err(format!("duplicate scenario id {}", scenario.id));
        }
        if scenario.expected_state.is_empty() {
            return Err(format!("scenario {} missing expected_state", scenario.id));
        }
        if scenario.claim_boundary.is_empty() {
            return Err(format!("scenario {} missing claim_boundary", scenario.id));
        }
        validate_composition_honesty(scenario.result_state)
            .map_err(|err| format!("scenario {}: {err}", scenario.id))?;
    }
    for dimension in [
        ProofCorpusDimensionV1::ResultState,
        ProofCorpusDimensionV1::Composition,
        ProofCorpusDimensionV1::IdentityBinding,
        ProofCorpusDimensionV1::ProviderEnvelope,
        ProofCorpusDimensionV1::NegativeExperiment,
    ] {
        if corpus.scenarios_for_dimension(dimension).next().is_none() {
            return Err(format!(
                "proof corpus missing {} dimension scenarios",
                dimension.as_str()
            ));
        }
    }
    for state in canonical_proof_result_states() {
        if !corpus
            .scenarios_for_dimension(ProofCorpusDimensionV1::ResultState)
            .any(|scenario| scenario.result_state == *state)
        {
            return Err(format!(
                "proof corpus missing result_state scenario for {}",
                state.as_str()
            ));
        }
    }
    Ok(())
}
