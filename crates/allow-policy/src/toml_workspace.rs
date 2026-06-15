use allow_core::WorkspaceConfig;
use serde::Deserialize;

use crate::toml_de::string_or_vec;

#[derive(Debug, Default, Deserialize)]
pub(crate) struct WorkspaceToml {
    root: Option<String>,
    inventory: Option<String>,
    default_mode: Option<String>,
    #[serde(default, deserialize_with = "string_or_vec")]
    ignored: Vec<String>,
    #[serde(default, deserialize_with = "string_or_vec")]
    generated: Vec<String>,
}

impl WorkspaceToml {
    pub(crate) fn into_workspace_config(self) -> WorkspaceConfig {
        let default = WorkspaceConfig::default();
        WorkspaceConfig {
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
        }
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
