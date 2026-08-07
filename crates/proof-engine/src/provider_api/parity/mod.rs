//! Parity fixture discovery for proof-provider-api (#2603-A).

use std::path::{Path, PathBuf};

pub fn parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![parity_contract_path(root)]
}

pub fn parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/proof-provider-api/parity-boundary-v1.toml")
}
