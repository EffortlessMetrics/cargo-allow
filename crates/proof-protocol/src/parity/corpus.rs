use std::path::{Path, PathBuf};

pub fn proof_corpus_fixture_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/proof-protocol/proof-corpus-v1.toml")
}

pub fn proof_corpus_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/proof-protocol/proof-corpus-contract-v1.toml")
}

pub fn proof_corpus_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![proof_corpus_contract_path(root)]
}
