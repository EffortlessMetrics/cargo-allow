use allow_core::{AllowConfig, CargoAllowResult};
use std::path::Path;

use crate::evidence_diagnostics::policy_reference_diagnostics;
use crate::evidence_validation::policy_reference_validation_error;

pub fn validate_local_evidence_references(
    root: impl AsRef<Path>,
    cfg: &AllowConfig,
) -> CargoAllowResult<()> {
    let root = root.as_ref();
    for entry in &cfg.allow {
        for reference in policy_reference_diagnostics(root, entry) {
            if let Some(error) = policy_reference_validation_error(entry, &reference) {
                return Err(error);
            }
        }
    }
    Ok(())
}

pub fn broken_evidence_link_count(root: impl AsRef<Path>, cfg: &AllowConfig) -> usize {
    let root = root.as_ref();
    cfg.allow
        .iter()
        .flat_map(|entry| policy_reference_diagnostics(root, entry))
        .filter(|reference| reference.diagnostic.status.is_broken_local_link())
        .count()
}

pub fn weak_evidence_reference_count(root: impl AsRef<Path>, cfg: &AllowConfig) -> usize {
    let root = root.as_ref();
    cfg.allow
        .iter()
        .flat_map(|entry| policy_reference_diagnostics(root, entry))
        .filter(|reference| reference.diagnostic.status.is_weak_reference())
        .count()
}
