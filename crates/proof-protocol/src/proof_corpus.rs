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
