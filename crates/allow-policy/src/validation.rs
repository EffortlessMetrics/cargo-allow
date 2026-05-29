use allow_core::{AllowConfig, CargoAllowError, CargoAllowResult};
use std::collections::BTreeSet;

use crate::entry_validation::{
    validate_allow_entry_evidence_and_limit, validate_allow_entry_identity,
    validate_allow_entry_requirements,
};
use crate::lifecycle::{has_real_lifecycle_review, validate_lifecycle};
use crate::policy_header::validate_policy_header;
use crate::scope_validation::{validate_allow_entry_scope, validate_workspace};
use crate::selector_validation::{validate_selector, validate_source_hints};

pub fn validate_policy(cfg: &AllowConfig) -> CargoAllowResult<()> {
    validate_policy_header(cfg)?;
    validate_workspace(&cfg.workspace)?;
    let mut ids = BTreeSet::new();
    for entry in &cfg.allow {
        validate_allow_entry_identity(entry)?;
        if !ids.insert(entry.id.clone()) {
            return Err(CargoAllowError::new(format!(
                "duplicate allow id `{}`",
                entry.id
            )));
        }
        validate_allow_entry_scope(entry)?;
        validate_selector(entry)?;
        validate_source_hints(entry)?;
        validate_lifecycle(entry)?;
        validate_allow_entry_requirements(entry, &cfg.requirements)?;
        if cfg.requirements.expires_or_review_after_required && !has_real_lifecycle_review(entry) {
            return Err(CargoAllowError::new(format!(
                "{} missing expires or review_after",
                entry.id
            )));
        }
        validate_allow_entry_evidence_and_limit(entry, &cfg.requirements)?;
    }
    Ok(())
}
