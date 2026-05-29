use allow_core::{AllowConfig, CargoAllowResult};
use std::collections::BTreeSet;

use crate::entry_validation::{
    validate_allow_entry_evidence_and_limit, validate_allow_entry_identity,
    validate_allow_entry_requirements,
};
use crate::lifecycle::{validate_lifecycle, validate_lifecycle_requirements};
use crate::policy_header::validate_policy_header;
use crate::scope_validation::{validate_allow_entry_scope, validate_workspace};
use crate::selector_validation::{validate_selector, validate_source_hints};

pub fn validate_policy(cfg: &AllowConfig) -> CargoAllowResult<()> {
    validate_policy_header(cfg)?;
    validate_workspace(&cfg.workspace)?;
    let mut ids = BTreeSet::new();
    for entry in &cfg.allow {
        validate_allow_entry_identity(entry, &mut ids)?;
        validate_allow_entry_scope(entry)?;
        validate_selector(entry)?;
        validate_source_hints(entry)?;
        validate_lifecycle(entry)?;
        validate_allow_entry_requirements(entry, &cfg.requirements)?;
        validate_lifecycle_requirements(entry, &cfg.requirements)?;
        validate_allow_entry_evidence_and_limit(entry, &cfg.requirements)?;
    }
    Ok(())
}
