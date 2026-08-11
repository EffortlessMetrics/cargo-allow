use crate::extraction_repo_edit_runtime::run_repo_edit_parity;
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
