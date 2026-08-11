use crate::extraction_repo_edit_runtime::{map_repo_edit_error, run_repo_edit_parity};
use allow_core::CargoAllowErrorKind;
use allow_policy::extraction_parity::ParityComparisonResult;

#[test]
fn repo_edit_authorities_are_parity_equivalent() -> Result<(), String> {
    let run = run_repo_edit_parity().map_err(|error| error.to_string())?;
    for case in run.cases {
        if case.comparison.result != ParityComparisonResult::SemanticallyEquivalent {
            return Err(format!(
                "{} parity differed: {:?}",
                case.id, case.comparison
            ));
        }
        if case.old_output != case.new_output {
            return Err(format!("{} canonical outputs differed", case.id));
        }
    }
    Ok(())
}

#[test]
fn repo_edit_containment_failures_are_invalid_config() {
    for message in [
        "target is outside repository root",
        "target is not inside the allowed root",
        "target would escape the repository",
    ] {
        let projected = map_repo_edit_error(effortless_repo_edit::RepoEditError::new(message));
        assert_eq!(projected.kind(), CargoAllowErrorKind::InvalidConfig);
        assert_eq!(projected.message(), message);
    }
}

#[test]
fn repo_edit_other_failures_are_artifacts() {
    let message = "atomic replacement failed";
    let projected = map_repo_edit_error(effortless_repo_edit::RepoEditError::new(message));
    assert_eq!(projected.kind(), CargoAllowErrorKind::Artifact);
    assert_eq!(projected.message(), message);
}
