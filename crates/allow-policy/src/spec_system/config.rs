use super::*;

use allow_core::{CargoAllowError, CargoAllowResult};
use std::path::Path;

pub fn parse_spec_system_config(input: &str) -> CargoAllowResult<SpecSystemConfig> {
    parse_spec_system_config_at(None, input)
}

pub fn parse_spec_system_config_at(
    path: Option<&Path>,
    input: &str,
) -> CargoAllowResult<SpecSystemConfig> {
    let config = toml::from_str::<SpecSystemConfig>(input).map_err(|e| {
        CargoAllowError::with_kind(
            allow_core::CargoAllowErrorKind::InvalidConfig,
            format!("failed to parse spec-system config TOML: {e}"),
        )
        .with_toml_span(path, input, e.span())
    })?;

    if matches!(config.generation, SpecSystemGeneration::CurrentV2)
        && (config.roots.goals.is_some() || config.requirements.active_goal_required)
    {
        return Err(CargoAllowError::with_kind(
            allow_core::CargoAllowErrorKind::InvalidConfig,
            "current-v2 spec-system profiles cannot configure roots.goals or active_goal_required",
        ));
    }

    Ok(config)
}
