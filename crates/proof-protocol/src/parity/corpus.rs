use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ProofCorpusParityContract {
    pub scenario_id: String,
    pub proof_protocol_module: String,
    pub parity_case: String,
    pub move_ledger_entry: String,
    pub corpus_digest: String,
    pub profile_id: String,
    pub required_dimensions: Vec<String>,
}

pub fn proof_corpus_fixture_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/proof-protocol/proof-corpus-v1.toml")
}

pub fn proof_corpus_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/proof-protocol/proof-corpus-contract-v1.toml")
}

pub fn proof_corpus_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![proof_corpus_contract_path(root)]
}

pub fn load_proof_corpus_fixture(
    root: &Path,
) -> Result<crate::proof_corpus::ProofCorpusV1, String> {
    let path = proof_corpus_fixture_path(root);
    let text =
        std::fs::read_to_string(&path).map_err(|err| format!("read {}: {err}", path.display()))?;
    let corpus = crate::proof_corpus::load_proof_corpus_toml(&text)?;
    // Semantic validation moved to proof-engine::corpus_semantics (#2943).
    // Structural validation (schema_id, digest) is done by load_proof_corpus_toml.
    Ok(corpus)
}

pub fn load_proof_corpus_contract(path: &Path) -> Result<ProofCorpusParityContract, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    toml::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
}
