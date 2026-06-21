use allow_core::{AllowEntry, CargoAllowError, CargoAllowResult, Requirements};
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
        let checks: [CargoAllowResult<()>; 8] = [
            validate_allow_entry_identity(entry, &mut ids),
            validate_allow_entry_scope(entry),
            validate_selector(entry),
            validate_source_hints(entry),
            validate_lifecycle(entry),
            validate_allow_entry_requirements(entry, requirements, link_scope_validation),
            validate_lifecycle_requirements(entry, requirements),
            validate_allow_entry_evidence_and_limit(entry, requirements),
        ];
        for check in checks {
            if let Err(err) = check {
                errors.push(err);
            }
        }
    }

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

#[cfg(test)]
#[path = "entries_validation_tests.rs"]
mod tests;
