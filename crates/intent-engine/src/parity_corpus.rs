//! Old/new parity corpus for intent-engine extraction (#2586-E).
//!
//! Records expected dispositions across profiles, selectors, staged movement,
//! diagnostics, and exit posture. Contract-only during the parity window.

use serde::{Deserialize, Serialize};

pub const PARITY_CORPUS_SCHEMA_ID: &str = "intent.parity-corpus.v1";
pub const PARITY_CORPUS_DIGEST_V1: &str = "sha256:v1:2586-e-spec-system-parity-corpus";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParityCorpusDimensionV1 {
    Profile,
    Selector,
    StagedMovement,
    Diagnostic,
    ExitPosture,
}

impl ParityCorpusDimensionV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Profile => "profile",
            Self::Selector => "selector",
            Self::StagedMovement => "staged_movement",
            Self::Diagnostic => "diagnostic",
            Self::ExitPosture => "exit_posture",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParityCorpusScenarioV1 {
    pub id: String,
    pub dimension: ParityCorpusDimensionV1,
    pub old_producer: String,
    pub new_producer: String,
    pub old_value: String,
    pub new_value: String,
    pub disposition: String,
    pub claim_boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParityCorpusV1 {
    pub schema_id: String,
    pub corpus_id: String,
    pub controlling_issue: u32,
    pub move_ledger_entry: String,
    pub corpus_digest: String,
    pub scenarios: Vec<ParityCorpusScenarioV1>,
}

impl ParityCorpusV1 {
    pub fn scenarios_for_dimension(
        &self,
        dimension: ParityCorpusDimensionV1,
    ) -> impl Iterator<Item = &ParityCorpusScenarioV1> {
        self.scenarios
            .iter()
            .filter(move |scenario| scenario.dimension == dimension)
    }
}

pub fn load_parity_corpus_toml(text: &str) -> Result<ParityCorpusV1, String> {
    let corpus: ParityCorpusV1 =
        toml::from_str(text).map_err(|err| format!("parse parity corpus: {err}"))?;
    if corpus.schema_id != PARITY_CORPUS_SCHEMA_ID {
        return Err(format!(
            "unexpected parity corpus schema_id {}",
            corpus.schema_id
        ));
    }
    if corpus.corpus_digest != PARITY_CORPUS_DIGEST_V1 {
        return Err(format!(
            "unexpected parity corpus digest {}",
            corpus.corpus_digest
        ));
    }
    Ok(corpus)
}

pub fn validate_parity_corpus(corpus: &ParityCorpusV1) -> Result<(), String> {
    if corpus.scenarios.is_empty() {
        return Err("parity corpus must include scenarios".to_string());
    }
    let mut ids = std::collections::BTreeSet::new();
    for scenario in &corpus.scenarios {
        if scenario.id.is_empty() {
            return Err("scenario id must not be empty".to_string());
        }
        if !ids.insert(scenario.id.clone()) {
            return Err(format!("duplicate scenario id {}", scenario.id));
        }
        if scenario.disposition.is_empty() {
            return Err(format!("scenario {} missing disposition", scenario.id));
        }
        if scenario.claim_boundary.is_empty() {
            return Err(format!("scenario {} missing claim_boundary", scenario.id));
        }
    }
    for dimension in [
        ParityCorpusDimensionV1::Profile,
        ParityCorpusDimensionV1::Selector,
        ParityCorpusDimensionV1::StagedMovement,
        ParityCorpusDimensionV1::Diagnostic,
        ParityCorpusDimensionV1::ExitPosture,
    ] {
        if corpus.scenarios_for_dimension(dimension).next().is_none() {
            return Err(format!(
                "parity corpus missing {} dimension scenarios",
                dimension.as_str()
            ));
        }
    }
    Ok(())
}

pub fn canonical_parity_dispositions() -> &'static [&'static str] {
    &[
        "SemanticallyEquivalent",
        "EquivalentWithCanonicalRenaming",
        "IntentionalDifferenceAccepted",
        "OutOfScope",
    ]
}
