use allow_core::{CargoAllowResult, FileFamilyRule, WorkspaceConfig};
use serde::Deserialize;

use crate::toml_de::string_or_vec;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorkspaceToml {
    root: Option<String>,
    inventory: Option<String>,
    default_mode: Option<String>,
    #[serde(default, deserialize_with = "string_or_vec")]
    ignored: Vec<String>,
    #[serde(default, deserialize_with = "string_or_vec")]
    generated: Vec<String>,
    #[serde(default, rename = "file_family")]
    file_families: Vec<FileFamilyRuleToml>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileFamilyRuleToml {
    id: String,
    family: String,
    glob: String,
    reason: String,
}

impl WorkspaceToml {
    pub(crate) fn into_workspace_config(self) -> CargoAllowResult<WorkspaceConfig> {
        let default = WorkspaceConfig::default();
        Ok(WorkspaceConfig {
            root: self.root.unwrap_or(default.root),
            inventory: self
                .inventory
                .map(normalize_inventory)
                .unwrap_or(default.inventory),
            ignored: if self.ignored.is_empty() {
                default.ignored
            } else {
                self.ignored
            },
            generated: if self.generated.is_empty() {
                default.generated
            } else {
                self.generated
            },
            default_mode: self.default_mode.unwrap_or(default.default_mode),
            file_families: self
                .file_families
                .into_iter()
                .map(|rule| FileFamilyRule {
                    id: rule.id,
                    family: rule.family,
                    glob: rule.glob,
                    reason: rule.reason,
                })
                .collect(),
        })
    }
}

fn normalize_inventory(inventory: String) -> String {
    if inventory == "git_tracked" {
        "git-tracked".to_string()
    } else {
        inventory
    }
}

#[cfg(test)]
#[path = "toml_workspace_tests.rs"]
mod tests;
