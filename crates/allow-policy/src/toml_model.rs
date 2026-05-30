use allow_core::{AllowConfig, CargoAllowError, CargoAllowResult};
use serde::Deserialize;

use crate::toml_entry::AllowEntryToml;
use crate::toml_requirements::RequirementsToml;
use crate::toml_workspace::WorkspaceToml;

#[derive(Debug, Default, Deserialize)]
struct PolicyToml {
    schema_version: Option<String>,
    policy: Option<String>,
    owner: Option<String>,
    status: Option<String>,
    #[serde(default)]
    workspace: WorkspaceToml,
    #[serde(default)]
    requirements: RequirementsToml,
    #[serde(default)]
    allow: Vec<AllowEntryToml>,
}

impl PolicyToml {
    fn into_config(self) -> CargoAllowResult<AllowConfig> {
        let allow = self
            .allow
            .into_iter()
            .enumerate()
            .map(|(index, entry)| entry.into_allow_entry(index))
            .collect::<CargoAllowResult<Vec<_>>>()?;
        Ok(AllowConfig {
            schema_version: self.schema_version.unwrap_or_else(|| "0.1".to_string()),
            policy: self.policy.unwrap_or_else(|| "cargo-allow".to_string()),
            owner: self.owner,
            status: self.status,
            workspace: self.workspace.into_workspace_config(),
            requirements: self.requirements.into_requirements(),
            allow,
        })
    }
}

pub(crate) fn parse_policy_toml(input: &str) -> CargoAllowResult<AllowConfig> {
    let raw = toml::from_str::<PolicyToml>(input)
        .map_err(|e| CargoAllowError::new(format!("failed to parse policy TOML: {e}")))?;
    raw.into_config()
}
