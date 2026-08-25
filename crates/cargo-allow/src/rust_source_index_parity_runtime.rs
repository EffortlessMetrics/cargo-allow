//! Runtime parity adapter for the RustSourceIndex extraction stage (#3375).
//!
//! The old `allow-rust` facade and the canonical `rust-source-index` crate
//! consume the same structural test-subject contract. This adapter checks the
//! shared contract and the copied subject-types implementation, then emits a
//! bounded semantic-parity observation. It does not claim test execution,
//! adequacy, or removal of the compatibility facade.

use allow_core::{CargoAllowError, CargoAllowResult, sha256_v1_bytes};
use allow_policy::extraction_parity::{
    ParityComparison, ParityObservation, compare_observations,
};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RustSourceIndexParityRun {
    pub test_subjects: RustSourceIndexParityCase,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RustSourceIndexParityCase {
    pub comparison: ParityComparison,
    pub old_output: String,
    pub new_output: String,
}

pub(crate) fn run_rust_source_index_parity(
    root: &Path,
) -> CargoAllowResult<RustSourceIndexParityRun> {
    let fixture = root.join("tests/fixtures/rust-source-index/parity-test-subjects-v1.toml");
    let contract = effortless_rust_source_index::load_test_subjects_parity_contract(&fixture)
        .map_err(|error| {
            CargoAllowError::new(format!(
                "load RustSourceIndex parity contract: {error}"
            ))
        })?;
    if contract.parity_case != "parity-rust-source-index-test-subjects-v1"
        || contract.move_ledger_entry != "move-allow-rust-test-subjects"
    {
        return Err(CargoAllowError::new(
            "RustSourceIndex parity contract is not bound to its registered case",
        ));
    }

    let canonical = read_subjects(root.join(
        "crates/effortless-rust-source-index/src/test_subjects.rs",
    ))?;
    let facade = read_subjects(
        root.join("crates/allow-rust/src/snapshot_package/test_subjects.rs"),
    )?;
    if canonical != facade {
        return Err(CargoAllowError::new(
            "allow-rust test-subject facade diverges from rust-source-index",
        ));
    }

    let output = format!(
        "scenario={}|fields={}|subjects_sha256={}",
        contract.scenario_id,
        contract.required_subject_fields.join(","),
        sha256_v1_bytes(canonical.as_bytes())
    );
    let source_identity = format!(
        "fixture:{}:{}",
        contract.parity_case,
        sha256_v1_bytes(fixture.to_string_lossy().as_bytes())
    );
    let comparison = compare_observations(
        &ParityObservation {
            source_identity: source_identity.clone(),
            canonical_output: output.clone(),
        },
        &ParityObservation {
            source_identity,
            canonical_output: output.clone(),
        },
    );
    Ok(RustSourceIndexParityRun {
        test_subjects: RustSourceIndexParityCase {
            comparison,
            old_output: output.clone(),
            new_output: output,
        },
    })
}

fn read_subjects(path: impl AsRef<Path>) -> CargoAllowResult<String> {
    std::fs::read_to_string(path.as_ref())
        .map(|text| text.replace("\r\n", "\n"))
        .map_err(|error| CargoAllowError::new(format!("read {}: {error}", path.as_ref().display())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_policy::extraction_parity::ParityComparisonResult;
    use std::path::PathBuf;

    #[test]
    fn rust_source_index_subject_contract_is_equivalent() -> Result<(), String> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let run = run_rust_source_index_parity(&root).map_err(|error| error.to_string())?;
        if run.test_subjects.comparison.result
            != ParityComparisonResult::SemanticallyEquivalent
        {
            return Err(format!("unexpected parity result: {:?}", run.test_subjects.comparison));
        }
        if run.test_subjects.old_output != run.test_subjects.new_output {
            return Err("old and new canonical outputs differ".to_string());
        }
        Ok(())
    }
}
