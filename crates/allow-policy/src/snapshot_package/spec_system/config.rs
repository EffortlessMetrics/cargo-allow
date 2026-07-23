//! Spec-system profile configuration DTOs (#2584-B).

use serde::Deserialize;

use super::import_roots::ImportRootsConfig;

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
