use allow_core::{AllowConfig, CargoAllowError, CargoAllowResult};

use crate::text_validation::{validate_optional_text, validate_required_text};

pub(crate) const SUPPORTED_SCHEMA_VERSION: &str = "0.1";
pub(crate) const SUPPORTED_SCHEMA_VERSION_ALIAS: &str = "1";

pub(crate) fn validate_policy_header(cfg: &AllowConfig) -> CargoAllowResult<()> {
    validate_required_text("policy schema_version", &cfg.schema_version)?;
    if cfg.schema_version != SUPPORTED_SCHEMA_VERSION
        && cfg.schema_version != SUPPORTED_SCHEMA_VERSION_ALIAS
    {
        return Err(CargoAllowError::new(format!(
            "unsupported policy schema_version `{}`",
            cfg.schema_version
        )));
    }
    validate_required_text("policy name", &cfg.policy)?;
    if cfg.policy != "cargo-allow" {
        return Err(CargoAllowError::new(format!(
            "unsupported policy `{}`",
            cfg.policy
        )));
    }
    validate_optional_text("policy owner", cfg.owner.as_deref())?;
    validate_optional_text("policy status", cfg.status.as_deref())?;
    if let Some(status) = &cfg.status {
        if !matches!(status.as_str(), "active" | "advisory") {
            return Err(CargoAllowError::new(format!(
                "unsupported policy status `{status}`"
            )));
        }
    }
    Ok(())
}
