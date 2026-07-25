//! Provider-independent proof corpus for external RIPR cargo-proof cutover (#2708).
//!
//! Records result taxonomy, binding identities, composition honesty rules, provider
//! envelopes, and negative experiments. Contract-only during the parity window;
//! does not execute real proof providers.

use serde::{Deserialize, Serialize};

pub const PROOF_CORPUS_SCHEMA_ID: &str = "proof.corpus.v1";
pub const PROOF_CORPUS_DIGEST_V1: &str = "sha256:v1:2708-ripr-external-proof-corpus";
pub const RIPR_EXTERNAL_PROOF_PROFILE_ID: &str = "ripr-external-proof-profile-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProofCorpusDimensionV1 {
    ResultState,
    Composition,
    IdentityBinding,
    ProviderEnvelope,
    NegativeExperiment,
}

impl ProofCorpusDimensionV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ResultState => "result_state",
            Self::Composition => "composition",
            Self::IdentityBinding => "identity_binding",
            Self::ProviderEnvelope => "provider_envelope",
            Self::NegativeExperiment => "negative_experiment",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProofResultStateV1 {
    NotSelected,
    SelectedNotRun,
    ProviderUnavailable,
    ProviderTempfail,
    ProofFailed,
    ProofPassed,
    Missing,
    Stale,
    Incomparable,
    Opaque,
    Unsupported,
}

impl ProofResultStateV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotSelected => "not_selected",
            Self::SelectedNotRun => "selected_not_run",
            Self::ProviderUnavailable => "provider_unavailable",
            Self::ProviderTempfail => "provider_tempfail",
            Self::ProofFailed => "proof_failed",
            Self::ProofPassed => "proof_passed",
            Self::Missing => "missing",
            Self::Stale => "stale",
            Self::Incomparable => "incomparable",
            Self::Opaque => "opaque",
            Self::Unsupported => "unsupported",
        }
    }

    /// Whether this state may contribute to a composed `proof_passed` aggregate.
    pub const fn allows_passed_composition(self) -> bool {
        matches!(self, Self::ProofPassed)
    }

    /// Whether this state reflects provider non-execution.
    pub const fn is_non_execution(self) -> bool {
        matches!(
            self,
            Self::NotSelected
                | Self::SelectedNotRun
                | Self::ProviderUnavailable
                | Self::ProviderTempfail
        )
    }

    /// Whether this state is machine-distinct from proof failure.
    pub const fn is_absence_or_currentness(self) -> bool {
        matches!(self, Self::Missing | Self::Stale | Self::Incomparable)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingCurrentnessV1 {
    Current,
    Stale,
    Missing,
    Incomparable,
}

impl BindingCurrentnessV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Stale => "stale",
            Self::Missing => "missing",
            Self::Incomparable => "incomparable",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofBindingIdentityV1 {
    pub repo_snapshot_id: String,
    pub phase_id: String,
    pub config_digest: String,
    pub tool_identity: String,
    pub proof_reference_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderEnvelopeV1 {
    pub provider_id: String,
    pub envelope_namespace: String,
    pub result_class: String,
    pub payload_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofCorpusScenarioV1 {
    pub id: String,
    pub dimension: ProofCorpusDimensionV1,
    pub result_state: ProofResultStateV1,
    pub producer: String,
    pub input_condition: String,
    pub expected_state: String,
    #[serde(default)]
    pub composed_aggregate: Option<String>,
    pub claim_boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofCorpusV1 {
    pub schema_id: String,
    pub corpus_id: String,
    pub controlling_issue: u32,
    pub move_ledger_entry: String,
    pub corpus_digest: String,
    pub profile_id: String,
    pub scenarios: Vec<ProofCorpusScenarioV1>,
}

impl ProofCorpusV1 {
    pub fn scenarios_for_dimension(
        &self,
        dimension: ProofCorpusDimensionV1,
    ) -> impl Iterator<Item = &ProofCorpusScenarioV1> {
        self.scenarios
            .iter()
            .filter(move |scenario| scenario.dimension == dimension)
    }

    pub fn scenario_by_id(&self, id: &str) -> Option<&ProofCorpusScenarioV1> {
        self.scenarios.iter().find(|scenario| scenario.id == id)
    }
}

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

pub fn load_proof_corpus_toml(text: &str) -> Result<ProofCorpusV1, String> {
    let corpus: ProofCorpusV1 =
        toml::from_str(text).map_err(|err| format!("parse proof corpus: {err}"))?;
    if corpus.schema_id != PROOF_CORPUS_SCHEMA_ID {
        return Err(format!(
            "unexpected proof corpus schema_id {}",
            corpus.schema_id
        ));
    }
    if corpus.corpus_digest != PROOF_CORPUS_DIGEST_V1 {
        return Err(format!(
            "unexpected proof corpus digest {}",
            corpus.corpus_digest
        ));
    }
    Ok(corpus)
}

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

pub fn validate_composition_honesty(state: ProofResultStateV1) -> Result<(), String> {
    if state.is_non_execution() && state.allows_passed_composition() {
        return Err(format!(
            "non-execution state {} cannot allow passed composition",
            state.as_str()
        ));
    }
    Ok(())
}

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

pub fn provider_envelope_distinct(left: &ProviderEnvelopeV1, right: &ProviderEnvelopeV1) -> bool {
    left.envelope_namespace != right.envelope_namespace
        || left.result_class != right.result_class
        || left.payload_digest != right.payload_digest
}
