use allow_core::{AllowConfig, CargoAllowError, CargoAllowResult};

use crate::entries_validation::{
    validate_allow_entries, validate_allow_entries_with_reportable_evidence,
};
use crate::lane_validation::validate_lanes;
use crate::policy_header::validate_policy_header;
use crate::scope_validation::validate_workspace;

/// Validate header + workspace + lanes + entries, collecting ALL errors
/// rather than short-circuiting on the first one. This lets adopters see
/// every problem in a single run.
fn collect_validation_errors(cfg: &AllowConfig) -> Vec<CargoAllowError> {
    let mut errors = Vec::new();
    if let Err(e) = validate_policy_header(cfg) {
        errors.push(e);
    }
    if let Err(e) = validate_workspace(&cfg.workspace) {
        errors.push(e);
    }
    if let Err(e) = validate_lanes(cfg) {
        errors.push(e);
    }
    if let Err(e) = validate_allow_entries(&cfg.allow, &cfg.requirements) {
        errors.push(e);
    }
    errors
}

fn collect_validation_errors_with_reportable_evidence(cfg: &AllowConfig) -> Vec<CargoAllowError> {
    let mut errors = Vec::new();
    if let Err(e) = validate_policy_header(cfg) {
        errors.push(e);
    }
    if let Err(e) = validate_workspace(&cfg.workspace) {
        errors.push(e);
    }
    if let Err(e) = validate_lanes(cfg) {
        errors.push(e);
    }
    if let Err(e) = validate_allow_entries_with_reportable_evidence(&cfg.allow, &cfg.requirements) {
        errors.push(e);
    }
    errors
}

/// Join a list of validation errors into a single `CargoAllowError`.
/// Returns `Ok(())` if the list is empty.
fn join_errors(errors: Vec<CargoAllowError>) -> CargoAllowResult<()> {
    if errors.is_empty() {
        Ok(())
    } else if errors.len() == 1 {
        Err(errors.into_iter().next().expect("exactly one error"))
    } else {
        let summary = errors
            .iter()
            .map(|e| format!("  - {e}"))
            .collect::<Vec<_>>()
            .join("\n");
        Err(CargoAllowError::new(format!(
            "{count} policy validation errors:\n{summary}",
            count = errors.len()
        )))
    }
}

pub fn validate_policy(cfg: &AllowConfig) -> CargoAllowResult<()> {
    join_errors(collect_validation_errors(cfg))
}

pub(crate) fn validate_policy_with_reportable_evidence(cfg: &AllowConfig) -> CargoAllowResult<()> {
    join_errors(collect_validation_errors_with_reportable_evidence(cfg))
}

#[cfg(test)]
#[path = "validation_orchestration_tests.rs"]
mod tests;
