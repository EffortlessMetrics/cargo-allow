//! Graph-compiler parity contract (#3524 slice E).
//!
//! The compiled-graph converter approach validated in #3645 (both DTO
//! families serialize the same shape, so a serde round-trip fails loudly
//! on drift) is formalized here as a parity dimension: the legacy
//! allow-policy compiler and the canonical intent-engine compiler must
//! produce semantically identical graphs from the same authority-file
//! inputs. The cargo-allow parity harness reads this contract, parses
//! the scenario's authority files with both families' own parsers, and
//! compares the compiled graphs after converting the legacy output.

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// One graph-compiler parity scenario plus the record of covered parity
/// dimensions for the spec-system move (#3307 slice 5).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct GraphCompilerParityContract {
    pub scenario_id: String,
    pub parity_case: String,
    pub move_ledger_entry: String,
    pub intent_engine_module: String,
    /// Parity scenarios, each with its own authority inputs and the
    /// diagnostic codes both compilers must produce (sorted). Seams and
    /// evidence paths are optional: when present the scenario includes
    /// authored-registration compile inputs.
    pub scenarios: Vec<GraphCompilerParityScenario>,
    /// Parity dimensions already proven for the spec-system move, with
    /// the lane that proved each. Recorded so the slice-E parity case
    /// states its coverage honestly.
    pub covered_dimensions: Vec<ParityDimensionRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct GraphCompilerParityScenario {
    pub id: String,
    pub description: String,
    pub requirement_path: String,
    pub slice_path: String,
    pub seams_path: Option<String>,
    pub evidence_path: Option<String>,
    /// Diagnostic codes (as_str form) both compilers must emit for this
    /// scenario, compared order-independently after the canonical sort.
    pub expect_diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ParityDimensionRecord {
    pub dimension: String,
    pub proven_by: String,
}

pub fn graph_compiler_parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/intent-engine/parity-graph-compiler-v1.toml")
}

pub fn graph_compiler_parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![graph_compiler_parity_contract_path(root)]
}

pub fn load_graph_compiler_parity_contract(
    path: &Path,
) -> Result<GraphCompilerParityContract, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    toml::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
}
