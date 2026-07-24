//! Parity fixture discovery for proof-protocol (#2588-A).

use std::path::{Path, PathBuf};

pub fn parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![root.join("tests/fixtures/proof-protocol/parity-boundary-v1.toml")]
}

pub fn parity_contract_path(root: &Path) -> PathBuf {
    parity_contract_paths(root)
        .into_iter()
        .next()
        .unwrap_or_else(|| root.join("tests/fixtures/proof-protocol/parity-boundary-v1.toml"))
}
