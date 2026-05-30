use allow_core::{AllowConfig, CargoAllowError, CargoAllowResult};

const SUPPORTED_SCHEMA_VERSION: &str = "0.1";

pub(crate) fn validate_policy_header(cfg: &AllowConfig) -> CargoAllowResult<()> {
    if cfg.schema_version.trim().is_empty() {
        return Err(CargoAllowError::new(
            "policy schema_version must not be empty",
        ));
    }
    if cfg.schema_version != SUPPORTED_SCHEMA_VERSION {
        return Err(CargoAllowError::new(format!(
            "unsupported policy schema_version `{}`",
            cfg.schema_version
        )));
    }
    if cfg.policy != "cargo-allow" {
        return Err(CargoAllowError::new(format!(
            "unsupported policy `{}`",
            cfg.policy
        )));
    }
    if cfg
        .owner
        .as_deref()
        .is_some_and(|owner| owner.trim().is_empty())
    {
        return Err(CargoAllowError::new("policy owner must not be empty"));
    }
    if cfg
        .owner
        .as_deref()
        .is_some_and(|owner| owner.trim() != owner)
    {
        return Err(CargoAllowError::new(
            "policy owner must not have leading or trailing whitespace",
        ));
    }
    if cfg
        .status
        .as_deref()
        .is_some_and(|status| status.trim().is_empty())
    {
        return Err(CargoAllowError::new("policy status must not be empty"));
    }
    if let Some(status) = &cfg.status {
        if !matches!(status.as_str(), "active" | "advisory") {
            return Err(CargoAllowError::new(format!(
                "unsupported policy status `{status}`"
            )));
        }
    }
    Ok(())
}
