use allow_core::{AllowEntry, CargoAllowResult, Requirements};
use std::collections::BTreeSet;

use crate::entry_validation::{
    validate_allow_entry_evidence_and_limit, validate_allow_entry_identity,
    validate_allow_entry_requirements,
};
use crate::lifecycle::{validate_lifecycle, validate_lifecycle_requirements};
use crate::scope_validation::validate_allow_entry_scope;
use crate::selector_validation::{validate_selector, validate_source_hints};

pub(crate) fn validate_allow_entries(
    entries: &[AllowEntry],
    requirements: &Requirements,
) -> CargoAllowResult<()> {
    let mut ids = BTreeSet::new();
    for entry in entries {
        validate_allow_entry_identity(entry, &mut ids)?;
        validate_allow_entry_scope(entry)?;
        validate_selector(entry)?;
        validate_source_hints(entry)?;
        validate_lifecycle(entry)?;
        validate_allow_entry_requirements(entry, requirements)?;
        validate_lifecycle_requirements(entry, requirements)?;
        validate_allow_entry_evidence_and_limit(entry, requirements)?;
    }
    Ok(())
}
