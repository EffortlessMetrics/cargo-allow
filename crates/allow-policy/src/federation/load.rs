use std::fs;
use std::path::Path;

use allow_core::{CargoAllowError, CargoAllowResult};

use super::config::{ValidatedFederationConfig, parse_federation_config};
use super::validate::validate_federation_config;

pub const FEDERATION_CONFIG_REL_PATH: &str = ".allow/config.toml";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FederationLoadOutcome {
    Missing,
    Parsed(ValidatedFederationConfig),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FederationLoadResult {
    pub path: String,
    pub outcome: FederationLoadOutcome,
}

impl FederationLoadResult {
    pub fn found(&self) -> bool {
        !matches!(self.outcome, FederationLoadOutcome::Missing)
    }

    pub fn validated(&self) -> Option<&ValidatedFederationConfig> {
        match &self.outcome {
            FederationLoadOutcome::Parsed(validated) => Some(validated),
            FederationLoadOutcome::Missing => None,
        }
    }
}

pub fn load_federation_config(root: &Path) -> CargoAllowResult<FederationLoadResult> {
    let path = root.join(FEDERATION_CONFIG_REL_PATH);
    if !path.is_file() {
        return Ok(FederationLoadResult {
            path: FEDERATION_CONFIG_REL_PATH.to_string(),
            outcome: FederationLoadOutcome::Missing,
        });
    }
    let text = fs::read_to_string(&path).map_err(|err| {
        CargoAllowError::new(format!(
            "failed to read {}: {err}",
            FEDERATION_CONFIG_REL_PATH
        ))
    })?;
    let config = parse_federation_config(&text)?;
    let validated = validate_federation_config(config);
    Ok(FederationLoadResult {
        path: FEDERATION_CONFIG_REL_PATH.to_string(),
        outcome: FederationLoadOutcome::Parsed(validated),
    })
}
