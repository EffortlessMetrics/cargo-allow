//! Config entrypoint for cargo-intent (#2599-A).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const CONFIG_SCHEMA_ID: &str = "cargo-intent.config.v1";
pub const DEFAULT_CONFIG_RELATIVE_PATH: &str = ".allow/intent.toml";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigProfileV1 {
    SpecSystem,
}

impl ConfigProfileV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SpecSystem => "spec-system",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentConfigV1 {
    pub schema_id: String,
    pub profile: ConfigProfileV1,
    pub root: String,
}

impl IntentConfigV1 {
    pub fn default_for_root(root: impl Into<String>) -> Self {
        Self {
            schema_id: CONFIG_SCHEMA_ID.to_string(),
            profile: ConfigProfileV1::SpecSystem,
            root: root.into(),
        }
    }
}

pub fn resolve_config_path(root: &Path, explicit: Option<&Path>) -> PathBuf {
    explicit
        .map(Path::to_path_buf)
        .unwrap_or_else(|| root.join(DEFAULT_CONFIG_RELATIVE_PATH))
}

pub fn load_config(root: &Path, explicit: Option<&Path>) -> Result<IntentConfigV1, String> {
    let path = resolve_config_path(root, explicit);
    if !path.is_file() {
        return Ok(IntentConfigV1::default_for_root(
            root.to_string_lossy().into_owned(),
        ));
    }
    let text =
        std::fs::read_to_string(&path).map_err(|err| format!("read {}: {err}", path.display()))?;
    let mut config: IntentConfigV1 =
        toml::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))?;
    if config.schema_id != CONFIG_SCHEMA_ID {
        return Err(format!(
            "unexpected config schema_id {} in {}",
            config.schema_id,
            path.display()
        ));
    }
    if config.root.is_empty() {
        config.root = root.to_string_lossy().into_owned();
    }
    Ok(config)
}
