//! Test-only parity fixture path discovery for proof-protocol
//! (#2588-A / #2588-B / #2708 / #2943 step 6).
//!
//! These locators are extraction-era scaffolding with no external runtime
//! consumer; they live behind `cfg(test)` so they are not part of the public
//! protocol data seam. Retirement of the remaining marker surface is
//! tracked by #2940.

mod corpus;

pub use corpus::{proof_corpus_contract_paths, proof_corpus_fixture_path};

use std::path::{Path, PathBuf};

pub fn parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/proof-protocol/parity-boundary-v1.toml")
}

pub fn parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    let mut paths = plan_dtos_parity_contract_paths(root);
    paths.extend(capability_dtos_parity_contract_paths(root));
    paths.extend(receipt_dtos_parity_contract_paths(root));
    paths.extend(contradiction_dtos_parity_contract_paths(root));
    paths.extend(phase_gate_dtos_parity_contract_paths(root));
    paths.extend(proof_corpus_contract_paths(root));
    paths.push(proof_corpus_fixture_path(root));
    paths.insert(0, parity_contract_path(root));
    paths
}

pub fn plan_dtos_parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/proof-protocol/parity-plan-dtos-v1.toml")
}

pub fn plan_dtos_parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![plan_dtos_parity_contract_path(root)]
}

pub fn capability_dtos_parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/proof-protocol/parity-capability-dtos-v1.toml")
}

pub fn capability_dtos_parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![capability_dtos_parity_contract_path(root)]
}

pub fn receipt_dtos_parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/proof-protocol/parity-receipt-dtos-v1.toml")
}

pub fn receipt_dtos_parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![receipt_dtos_parity_contract_path(root)]
}

pub fn contradiction_dtos_parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/proof-protocol/parity-contradiction-dtos-v1.toml")
}

pub fn contradiction_dtos_parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![contradiction_dtos_parity_contract_path(root)]
}

pub fn phase_gate_dtos_parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/proof-protocol/parity-phase-gate-dtos-v1.toml")
}

pub fn phase_gate_dtos_parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![phase_gate_dtos_parity_contract_path(root)]
}
