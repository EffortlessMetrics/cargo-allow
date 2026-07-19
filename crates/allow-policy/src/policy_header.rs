use allow_core::{AllowConfig, CargoAllowError, CargoAllowResult, POLICY_NAME};

// Re-export the canonical schema-version constants so existing
// `crate::policy_header::*` imports keep resolving; the single source of truth
// now lives in allow-core.
pub(crate) use allow_core::{SUPPORTED_SCHEMA_VERSION, SUPPORTED_SCHEMA_VERSION_ALIAS};

use crate::text_validation::{validate_optional_text, validate_required_text};

/// The core invariants (schema_version, policy name, status) are codified once
/// in `allow_core::AllowConfig::validate`; this header check delegates to the
/// same constants so the two layers cannot drift.
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
    if cfg.policy != POLICY_NAME {
        return Err(CargoAllowError::new(format!(
            "unsupported policy `{}`",
            cfg.policy
        )));
    }
    validate_optional_text("policy owner", cfg.owner.as_deref())?;
    validate_optional_text("policy status", cfg.status.as_deref())?;
    if let Some(status) = &cfg.status
        && !matches!(status.as_str(), "active" | "advisory")
    {
        return Err(CargoAllowError::new(format!(
            "unsupported policy status `{status}`"
        )));
    }
    Ok(())
}
