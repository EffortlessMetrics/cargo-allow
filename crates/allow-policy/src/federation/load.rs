use std::path::Path;

use allow_core::{
    CappedReadError, CargoAllowError, CargoAllowErrorKind, CargoAllowResult, read_text_file_capped,
};

use super::config::{ValidatedFederationConfig, parse_federation_config_at};
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
    /// True when the federation config file was found and parsed, regardless
    /// of whether validation passed. Use [`is_valid`](Self::is_valid) to check
    /// that the parsed config also passed validation (#1837).
    pub fn parsed(&self) -> bool {
        !matches!(self.outcome, FederationLoadOutcome::Missing)
    }

    /// True when the federation config was found, parsed, **and** passed
    /// validation (`valid == true`). A config with blocking diagnostics
    /// (e.g. `DuplicateId`) returns false here even though it was parsed.
    pub fn is_valid(&self) -> bool {
        match &self.outcome {
            FederationLoadOutcome::Parsed(validated) => validated.valid,
            FederationLoadOutcome::Missing => false,
        }
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
    if !path.exists() {
        return Ok(FederationLoadResult {
            path: FEDERATION_CONFIG_REL_PATH.to_string(),
            outcome: FederationLoadOutcome::Missing,
        });
    }
    let text = read_text_file_capped(&path).map_err(|err| match err {
        CappedReadError::Io(source) => CargoAllowError::from(source)
            .with_message_prefix(format!("failed to read {}: ", path.display())),
        CappedReadError::Oversized { .. } | CappedReadError::NotUtf8(_) => {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidConfig,
                format!("failed to read {}: {err}", path.display()),
            )
        }
    })?;
    let config = parse_federation_config_at(Some(&path), &text)?;
    let validated = validate_federation_config(config);
    Ok(FederationLoadResult {
        path: FEDERATION_CONFIG_REL_PATH.to_string(),
        outcome: FederationLoadOutcome::Parsed(validated),
    })
}
