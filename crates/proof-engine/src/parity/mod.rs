//! Parity fixture discovery for proof-engine (#2589-A).

use std::path::{Path, PathBuf};

pub fn parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![parity_contract_path(root)]
}

pub fn parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/proof-engine/parity-boundary-v1.toml")
}
