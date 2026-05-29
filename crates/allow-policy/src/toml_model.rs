use allow_core::{AllowConfig, CargoAllowError, CargoAllowResult, Requirements, WorkspaceConfig};
use serde::Deserialize;

use crate::toml_de::{option_bool_or_string, string_or_vec};
use crate::toml_entry::AllowEntryToml;

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

#[derive(Debug, Default, Deserialize)]
struct WorkspaceToml {
    root: Option<String>,
    inventory: Option<String>,
    default_mode: Option<String>,
    #[serde(default, deserialize_with = "string_or_vec")]
    ignored: Vec<String>,
    #[serde(default, deserialize_with = "string_or_vec")]
    generated: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RequirementsToml {
    #[serde(default, deserialize_with = "option_bool_or_string")]
    owner_required: Option<bool>,
    #[serde(default, deserialize_with = "option_bool_or_string")]
    reason_required: Option<bool>,
    #[serde(default, deserialize_with = "option_bool_or_string")]
    classification_required: Option<bool>,
    #[serde(default, deserialize_with = "option_bool_or_string")]
    evidence_required: Option<bool>,
    #[serde(default, deserialize_with = "option_bool_or_string")]
    expires_or_review_after_required: Option<bool>,
    #[serde(default, deserialize_with = "option_bool_or_string")]
    allow_bare_allow_attributes: Option<bool>,
    #[serde(default, deserialize_with = "option_bool_or_string")]
    lint_policy_id_required: Option<bool>,
    #[serde(default, deserialize_with = "option_bool_or_string")]
    stale_entries_fail: Option<bool>,
    #[serde(default, rename = "unsafe")]
    unsafe_requirements: UnsafeRequirementsToml,
}

#[derive(Debug, Default, Deserialize)]
struct UnsafeRequirementsToml {
    #[serde(default, deserialize_with = "option_bool_or_string")]
    evidence_required: Option<bool>,
    #[serde(default, deserialize_with = "option_bool_or_string")]
    safety_comment_required: Option<bool>,
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

impl WorkspaceToml {
    fn into_workspace_config(self) -> WorkspaceConfig {
        let default = WorkspaceConfig::default();
        WorkspaceConfig {
            root: self.root.unwrap_or(default.root),
            inventory: self.inventory.unwrap_or(default.inventory),
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

impl RequirementsToml {
    fn into_requirements(self) -> Requirements {
        let default = Requirements::default();
        Requirements {
            owner_required: self.owner_required.unwrap_or(default.owner_required),
            reason_required: self.reason_required.unwrap_or(default.reason_required),
            classification_required: self
                .classification_required
                .unwrap_or(default.classification_required),
            evidence_required: self.evidence_required.unwrap_or(default.evidence_required),
            expires_or_review_after_required: self
                .expires_or_review_after_required
                .unwrap_or(default.expires_or_review_after_required),
            allow_bare_allow_attributes: self
                .allow_bare_allow_attributes
                .unwrap_or(default.allow_bare_allow_attributes),
            lint_policy_id_required: self
                .lint_policy_id_required
                .unwrap_or(default.lint_policy_id_required),
            stale_entries_fail: self
                .stale_entries_fail
                .unwrap_or(default.stale_entries_fail),
            unsafe_evidence_required: self
                .unsafe_requirements
                .evidence_required
                .unwrap_or(default.unsafe_evidence_required),
            unsafe_safety_comment_required: self
                .unsafe_requirements
                .safety_comment_required
                .unwrap_or(default.unsafe_safety_comment_required),
        }
    }
}

pub(crate) fn parse_policy_toml(input: &str) -> CargoAllowResult<AllowConfig> {
    let raw = toml::from_str::<PolicyToml>(input)
        .map_err(|e| CargoAllowError::new(format!("failed to parse policy TOML: {e}")))?;
    raw.into_config()
}
