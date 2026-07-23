use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ParityCorpusParityContract {
    pub scenario_id: String,
    pub parity_case: String,
    pub move_ledger_entry: String,
    pub intent_engine_module: String,
    pub required_dimensions: Vec<String>,
    pub corpus_digest: String,
}

pub fn parity_corpus_fixture_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/intent-engine/parity-corpus-v1.toml")
}

pub fn parity_corpus_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/intent-engine/parity-corpus-contract-v1.toml")
}

pub fn parity_corpus_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![parity_corpus_contract_path(root)]
}

pub fn load_parity_corpus_fixture(
    root: &Path,
) -> Result<crate::parity_corpus::ParityCorpusV1, String> {
    let path = parity_corpus_fixture_path(root);
    let text =
        std::fs::read_to_string(&path).map_err(|err| format!("read {}: {err}", path.display()))?;
    let corpus = crate::parity_corpus::load_parity_corpus_toml(&text)?;
    crate::parity_corpus::validate_parity_corpus(&corpus)?;
    Ok(corpus)
}

pub fn load_parity_corpus_contract(path: &Path) -> Result<ParityCorpusParityContract, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    toml::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
}
