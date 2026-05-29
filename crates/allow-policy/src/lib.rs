use allow_core::{AllowConfig, CargoAllowError, CargoAllowResult};
use std::fs;
use std::path::{Path, PathBuf};

mod entries_validation;
mod entry_validation;
mod evidence;
mod lifecycle;
mod policy_header;
mod render;
mod render_entry;
mod render_sections;
mod render_toml;
mod scope_validation;
mod selector_validation;
mod starter;
mod toml_de;
mod toml_entry;
mod toml_model;
mod toml_sections;
mod validation;
pub use evidence::{
    EvidenceReferenceDiagnostic, EvidenceReferenceStatus, evidence_reference_diagnostics,
    validate_local_evidence_references,
};
pub use render::render_policy;
pub use starter::starter_policy;
pub use validation::validate_policy;

pub fn find_config(start: impl AsRef<Path>) -> Option<PathBuf> {
    let mut dir = start.as_ref().canonicalize().ok()?;
    loop {
        for rel in ["policy/allow.toml", ".cargo/allow.toml", "allow.toml"] {
            let candidate = dir.join(rel);
            if candidate.exists() {
                return Some(candidate);
            }
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

pub fn load_policy(path: impl AsRef<Path>) -> CargoAllowResult<AllowConfig> {
    let text = fs::read_to_string(path.as_ref()).map_err(|e| {
        CargoAllowError::new(format!("failed to read {}: {e}", path.as_ref().display()))
    })?;
    parse_policy(&text)
}

pub fn parse_policy(input: &str) -> CargoAllowResult<AllowConfig> {
    let cfg = toml_model::parse_policy_toml(input)?;
    validate_policy(&cfg)?;
    Ok(cfg)
}

#[cfg(test)]
mod evidence_tests;
#[cfg(test)]
mod render_tests;
#[cfg(test)]
mod starter_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod validation_tests;
