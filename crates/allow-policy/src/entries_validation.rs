use allow_core::{
    AllowEntry, CargoAllowDiagnostic, CargoAllowError, CargoAllowResult, Requirements,
};
use std::collections::BTreeSet;

use crate::entry_validation::{
    LinkScopeValidation, validate_allow_entry_evidence_and_limit, validate_allow_entry_identity,
    validate_allow_entry_requirements,
};
use crate::lifecycle::{validate_lifecycle, validate_lifecycle_requirements};
use crate::scope_validation::validate_allow_entry_scope;
use crate::selector_validation::{validate_selector, validate_source_hints};

pub(crate) fn validate_allow_entries(
    entries: &[AllowEntry],
    requirements: &Requirements,
) -> CargoAllowResult<()> {
    validate_allow_entries_with_link_scope_validation(
        entries,
        requirements,
        LinkScopeValidation::Strict,
    )
}

pub(crate) fn validate_allow_entries_with_reportable_evidence(
    entries: &[AllowEntry],
    requirements: &Requirements,
) -> CargoAllowResult<()> {
    validate_allow_entries_with_link_scope_validation(
        entries,
        requirements,
        LinkScopeValidation::ReportOnly,
    )
}

fn validate_allow_entries_with_link_scope_validation(
    entries: &[AllowEntry],
    requirements: &Requirements,
    link_scope_validation: LinkScopeValidation,
) -> CargoAllowResult<()> {
    let mut ids = BTreeSet::new();
    let mut errors: Vec<CargoAllowError> = Vec::new();

    for entry in entries {
        // Collect all validation errors for this entry rather than
        // short-circuiting on the first one. This lets an adopter with N
        // broken entries see all N errors in a single run instead of
        // fixing one, re-running, fixing the next, etc.
        let checks: [(&str, CargoAllowResult<()>); 8] = [
            ("identity", validate_allow_entry_identity(entry, &mut ids)),
            ("scope", validate_allow_entry_scope(entry)),
            ("selector", validate_selector(entry)),
            ("source_hints", validate_source_hints(entry)),
            ("lifecycle", validate_lifecycle(entry)),
            (
                "requirements",
                validate_allow_entry_requirements(entry, requirements, link_scope_validation),
            ),
            (
                "lifecycle_requirements",
                validate_lifecycle_requirements(entry, requirements),
            ),
            (
                "evidence_and_limit",
                validate_allow_entry_evidence_and_limit(entry, requirements),
            ),
        ];
        for (field, check) in checks {
            if let Err(err) = check {
                let code = err.code();
                let message = err.message().to_owned();
                errors.push(err.with_diagnostic(CargoAllowDiagnostic::error(
                    code,
                    "policy_validation",
                    Some(&entry.id),
                    Some(field),
                    message,
                )));
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else if errors.len() == 1 {
        // Safety of the non-panicing extraction: we just checked len == 1,
        // so into_iter().next() is guaranteed to yield exactly one error.
        // Using next() without expect avoids a panic-macro scanner finding.
        Err(errors
            .into_iter()
            .next()
            .unwrap_or_else(|| CargoAllowError::new("validation error")))
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
            allow_core::CargoAllowErrorKind::InvalidPolicy,
            format!(
                "{count} policy validation errors:\n{summary}",
                count = errors.len()
            ),
        )
        .with_diagnostics(diagnostics))
    }
}

#[cfg(test)]
#[path = "entries_validation_tests.rs"]
mod tests;
