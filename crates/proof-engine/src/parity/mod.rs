//! Parity fixture discovery for proof-engine (#2589-A / #2713).

mod routing;

use std::path::{Path, PathBuf};

pub use routing::{
    RiprRoutingParityContract, load_ripr_routing_contract, ripr_routing_contract_path,
    ripr_routing_fixture_path,
};

pub fn parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![
        parity_contract_path(root),
        ripr_routing_contract_path(root),
        ripr_routing_fixture_path(root),
    ]
}

pub fn parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/proof-engine/parity-boundary-v1.toml")
}
