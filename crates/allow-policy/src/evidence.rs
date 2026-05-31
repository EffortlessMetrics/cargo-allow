use allow_core::{AllowConfig, CargoAllowResult};
use std::path::Path;

use crate::evidence_diagnostics::evidence_reference_diagnostics;
use crate::evidence_validation::evidence_reference_validation_error;

pub fn validate_local_evidence_references(
    root: impl AsRef<Path>,
    cfg: &AllowConfig,
) -> CargoAllowResult<()> {
    let root = root.as_ref();
    for entry in &cfg.allow {
        for diagnostic in evidence_reference_diagnostics(root, entry) {
            if let Some(error) = evidence_reference_validation_error(entry, &diagnostic) {
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
        .flat_map(|entry| evidence_reference_diagnostics(root, entry))
        .filter(|diagnostic| diagnostic.status.is_broken_local_link())
        .count()
}

pub fn weak_evidence_reference_count(root: impl AsRef<Path>, cfg: &AllowConfig) -> usize {
    let root = root.as_ref();
    cfg.allow
        .iter()
        .flat_map(|entry| evidence_reference_diagnostics(root, entry))
        .filter(|diagnostic| diagnostic.status.is_weak_reference())
        .count()
}
