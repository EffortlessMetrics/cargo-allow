use allow_core::{AllowConfig, CargoAllowError, CargoAllowResult};
use std::collections::BTreeSet;

use crate::entry_validation::{
    validate_allow_entry_evidence_and_limit, validate_allow_entry_identity,
    validate_allow_entry_requirements,
};
use crate::lifecycle::{has_real_lifecycle_review, validate_lifecycle};
use crate::policy_header::validate_policy_header;
use crate::scope_validation::{
    validate_glob, validate_path_scope, validate_scope_consistency, validate_workspace,
};
use crate::selector_validation::{validate_selector, validate_source_hints};

pub fn validate_policy(cfg: &AllowConfig) -> CargoAllowResult<()> {
    validate_policy_header(cfg)?;
    validate_workspace(&cfg.workspace)?;
    for pattern in &cfg.workspace.ignored {
        validate_glob("source-tree ignored glob", pattern)?;
    }
    for pattern in &cfg.workspace.generated {
        validate_glob("source-tree generated glob", pattern)?;
    }
    let mut ids = BTreeSet::new();
    for entry in &cfg.allow {
        validate_allow_entry_identity(entry)?;
        if !ids.insert(entry.id.clone()) {
            return Err(CargoAllowError::new(format!(
                "duplicate allow id `{}`",
                entry.id
            )));
        }
        if entry.path.is_none() && entry.glob.is_none() && entry.selector.glob.is_none() {
            return Err(CargoAllowError::new(format!(
                "{} has no path or glob",
                entry.id
            )));
        }
        if let Some(path) = &entry.path {
            validate_path_scope(&entry.id, path)?;
        }
        if let Some(glob) = &entry.glob {
            validate_glob(&format!("{} glob", entry.id), glob)?;
        }
        if let Some(glob) = &entry.selector.glob {
            validate_glob(&format!("{} selector glob", entry.id), glob)?;
        }
        validate_scope_consistency(entry)?;
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
