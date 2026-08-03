//! Reviewed command registry transport (#2603-B).
//!
//! Registry entries are the only executable command authority. Issue/spec prose
//! must never be promoted into argv through this module.

use serde::{Deserialize, Serialize};

pub const COMMAND_REGISTRY_SCHEMA_ID: &str = "proof.command-registry.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CwdPolicyV1 {
    RepositoryRoot,
}

impl CwdPolicyV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RepositoryRoot => "repository_root",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkAccessV1 {
    None,
}

impl NetworkAccessV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CancellationPostureV1 {
    Cooperative,
}

impl CancellationPostureV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cooperative => "cooperative",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewedCommandEntryV1 {
    pub command_id: String,
    pub program: String,
    pub argv_prefix: Vec<String>,
    /// Whether plan arguments may extend the reviewed prefix.
    ///
    /// The default preserves the v1 prefix behavior for existing registry
    /// entries. Read-only report commands can opt into exact argv binding.
    #[serde(default = "default_allow_trailing_args")]
    pub allow_trailing_args: bool,
    pub cwd_policy: CwdPolicyV1,
    pub env_allowlist: Vec<String>,
    pub read_paths: Vec<String>,
    pub write_paths: Vec<String>,
    pub network: NetworkAccessV1,
    pub timeout_ms: u64,
    pub cancellation: CancellationPostureV1,
}

const fn default_allow_trailing_args() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewedCommandRegistryV1 {
    pub schema_id: String,
    pub registry_id: String,
    pub commands: Vec<ReviewedCommandEntryV1>,
}

impl ReviewedCommandRegistryV1 {
    pub fn new(registry_id: impl Into<String>, commands: Vec<ReviewedCommandEntryV1>) -> Self {
        Self {
            schema_id: COMMAND_REGISTRY_SCHEMA_ID.to_string(),
            registry_id: registry_id.into(),
            commands,
        }
    }

    pub fn find(&self, command_id: &str) -> Option<&ReviewedCommandEntryV1> {
        self.commands
            .iter()
            .find(|entry| entry.command_id == command_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandRegistryError {
    InvalidSchemaId { observed: String },
    EmptyRegistry,
    DuplicateCommandId { command_id: String },
    UnknownCommandId { command_id: String },
}

impl CommandRegistryError {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::InvalidSchemaId { .. } => "invalid_schema_id",
            Self::EmptyRegistry => "empty_registry",
            Self::DuplicateCommandId { .. } => "duplicate_command_id",
            Self::UnknownCommandId { .. } => "unknown_command_id",
        }
    }
}

pub fn validate_command_registry(
    registry: &ReviewedCommandRegistryV1,
) -> Result<(), CommandRegistryError> {
    if registry.schema_id != COMMAND_REGISTRY_SCHEMA_ID {
        return Err(CommandRegistryError::InvalidSchemaId {
            observed: registry.schema_id.clone(),
        });
    }
    if registry.commands.is_empty() {
        return Err(CommandRegistryError::EmptyRegistry);
    }
    let mut seen = std::collections::BTreeSet::new();
    for entry in &registry.commands {
        if !seen.insert(entry.command_id.clone()) {
            return Err(CommandRegistryError::DuplicateCommandId {
                command_id: entry.command_id.clone(),
            });
        }
    }
    Ok(())
}

pub fn default_cargo_allow_registry() -> ReviewedCommandRegistryV1 {
    ReviewedCommandRegistryV1::new(
        "proof-adapter-command.default.v1",
        vec![
            ReviewedCommandEntryV1 {
                command_id: "cargo-allow.check.no-new".to_string(),
                program: "cargo-allow".to_string(),
                argv_prefix: vec![
                    "check".to_string(),
                    "--mode".to_string(),
                    "no-new".to_string(),
                ],
                allow_trailing_args: true,
                cwd_policy: CwdPolicyV1::RepositoryRoot,
                env_allowlist: vec!["CARGO_TARGET_DIR".to_string()],
                read_paths: vec!["policy/allow.toml".to_string()],
                write_paths: vec!["target/cargo-allow/".to_string()],
                network: NetworkAccessV1::None,
                timeout_ms: 600_000,
                cancellation: CancellationPostureV1::Cooperative,
            },
            ReviewedCommandEntryV1 {
                command_id: "cargo-allow.capabilities.json".to_string(),
                program: "cargo-allow".to_string(),
                argv_prefix: vec![
                    "capabilities".to_string(),
                    "--format".to_string(),
                    "json".to_string(),
                ],
                allow_trailing_args: false,
                cwd_policy: CwdPolicyV1::RepositoryRoot,
                env_allowlist: vec![],
                read_paths: vec![],
                write_paths: vec![],
                network: NetworkAccessV1::None,
                timeout_ms: 60_000,
                cancellation: CancellationPostureV1::Cooperative,
            },
        ],
    )
}
