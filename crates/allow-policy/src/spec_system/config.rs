use allow_core::{CargoAllowError, CargoAllowResult};
use serde::Deserialize;
use std::path::Path;

use crate::import_roots::ImportRootsConfig;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SpecSystemMode {
    Advisory,
    Shadow,
    Blocking,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SpecSystemGeneration {
    #[default]
    LegacyV1,
    CurrentV2,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpecSystemConfig {
    pub schema_version: String,
    pub profile: String,
    pub mode: SpecSystemMode,
    #[serde(default)]
    pub generation: SpecSystemGeneration,
    pub roots: SpecSystemRoots,
    #[serde(default)]
    pub requirements: SpecSystemRequirements,
    #[serde(default)]
    pub import_roots: Option<ImportRootsConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpecSystemRoots {
    pub proposals: String,
    pub specs: String,
    pub adrs: String,
    pub plans: String,
    #[serde(default)]
    pub goals: Option<String>,
    pub support_tiers: String,
    pub artifact_ledger: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpecSystemRequirements {
    #[serde(default)]
    pub ledger_required: bool,
    #[serde(default)]
    pub templates_required: bool,
    #[serde(default)]
    pub support_tiers_required: bool,
    #[serde(default)]
    pub active_goal_required: bool,
    #[serde(default)]
    pub closeout_required_for_done_items: bool,
}

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
