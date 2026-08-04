use allow_core::{
    AllowConfig, CargoAllowDiagnostic, CargoAllowError, CargoAllowErrorKind, CargoAllowResult,
};

use crate::bare_allow_conflict::detect_bare_allow_conflict;
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
        errors.push(with_validation_diagnostic(e, "header", None));
    }
    if let Err(e) = validate_workspace(&cfg.workspace) {
        errors.push(with_validation_diagnostic(e, "workspace", None));
    }
    if let Err(e) = validate_lanes(cfg) {
        errors.push(with_validation_diagnostic(e, "lanes", None));
    }
    if let Err(e) = validate_allow_entries(&cfg.allow, &cfg.requirements) {
        errors.push(e);
    }
    if let Err(e) = detect_bare_allow_conflict(cfg) {
        errors.push(with_validation_diagnostic(e, "bare_allow_conflict", None));
    }
    errors
}

fn collect_validation_errors_with_reportable_evidence(cfg: &AllowConfig) -> Vec<CargoAllowError> {
    let mut errors = Vec::new();
    if let Err(e) = validate_policy_header(cfg) {
        errors.push(with_validation_diagnostic(e, "header", None));
    }
    if let Err(e) = validate_workspace(&cfg.workspace) {
        errors.push(with_validation_diagnostic(e, "workspace", None));
    }
    if let Err(e) = validate_lanes(cfg) {
        errors.push(with_validation_diagnostic(e, "lanes", None));
    }
    if let Err(e) = validate_allow_entries_with_reportable_evidence(&cfg.allow, &cfg.requirements) {
        errors.push(e);
    }
    if let Err(e) = detect_bare_allow_conflict(cfg) {
        errors.push(with_validation_diagnostic(e, "bare_allow_conflict", None));
    }
    errors
}

/// Join a list of validation errors into a single `CargoAllowError`.
/// Returns `Ok(())` if the list is empty.
fn join_errors(errors: Vec<CargoAllowError>) -> CargoAllowResult<()> {
    if errors.is_empty() {
        Ok(())
    } else if errors.len() == 1 {
        // Safety of the non-panicing extraction: we just checked len == 1,
        // so into_iter().next() is guaranteed to yield exactly one error.
        // Using next() without expect avoids a panic-macro scanner finding.
        Err(errors
            .into_iter()
            .next()
            .unwrap_or_else(|| CargoAllowError::new("validation error"))
            .with_kind_preserving_metadata(CargoAllowErrorKind::InvalidPolicy))
    } else {
        let summary = errors
            .iter()
            .map(|e| format!("  - {e}"))
            .collect::<Vec<_>>()
            .join("\n");
        let diagnostics = errors
            .iter()
            .flat_map(|error| error.diagnostics().iter().cloned())
            .collect::<Vec<_>>();
        Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidPolicy,
            format!(
                "{count} policy validation errors:\n{summary}",
                count = errors.len()
            ),
        )
        .with_diagnostics(diagnostics))
    }
}

fn with_validation_diagnostic(
    error: CargoAllowError,
    field: &str,
    entry_id: Option<&str>,
) -> CargoAllowError {
    let code = error.code();
    let message = error.message().to_owned();
    error.with_diagnostic(CargoAllowDiagnostic::error(
        code,
        "policy_validation",
        entry_id,
        Some(field),
        message,
    ))
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
