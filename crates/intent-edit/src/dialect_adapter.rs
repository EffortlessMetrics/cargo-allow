//! Dialect adapters for intent-shaped edit selectors (#2613-C).
//!
//! Adapters normalize selector strings per ledger dialect. They do not parse
//! file contents, invoke repo-edit, or execute repository artifacts.

use serde::{Deserialize, Serialize};

pub const INTENT_EDIT_DIALECT_ADAPTER_SCHEMA_ID: &str = "intent.edit-dialect-adapter.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentEditDialectV1 {
    CargoAllowPolicy,
    CargoAllowDocArtifacts,
    SpecSystem,
    GenericToml,
}

impl IntentEditDialectV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CargoAllowPolicy => "cargo-allow",
            Self::CargoAllowDocArtifacts => "cargo-allow-doc-artifacts",
            Self::SpecSystem => "spec-system",
            Self::GenericToml => "generic-toml",
        }
    }

    pub fn parse_id(value: &str) -> Option<Self> {
        match value.trim() {
            "cargo-allow" => Some(Self::CargoAllowPolicy),
            "cargo-allow-doc-artifacts" => Some(Self::CargoAllowDocArtifacts),
            "spec-system" => Some(Self::SpecSystem),
            "generic-toml" => Some(Self::GenericToml),
            _ => None,
        }
    }
}

pub const CANONICAL_DIALECT_IDS: &[&str] = &[
    "cargo-allow",
    "cargo-allow-doc-artifacts",
    "spec-system",
    "generic-toml",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialectAdapterError {
    EmptySelector,
    UnsupportedDialect { dialect: String },
}

impl DialectAdapterError {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::EmptySelector => "empty_selector",
            Self::UnsupportedDialect { .. } => "unsupported_dialect",
        }
    }
}

pub fn adapt_selector(
    dialect: IntentEditDialectV1,
    selector: &str,
) -> Result<String, DialectAdapterError> {
    let trimmed = selector.trim();
    if trimmed.is_empty() {
        return Err(DialectAdapterError::EmptySelector);
    }
    let normalized = match dialect {
        IntentEditDialectV1::CargoAllowPolicy | IntentEditDialectV1::CargoAllowDocArtifacts => {
            normalize_repository_relative_path(trimmed)
        }
        IntentEditDialectV1::SpecSystem => normalize_spec_system_selector(trimmed),
        IntentEditDialectV1::GenericToml => trimmed.to_string(),
    };
    Ok(normalized)
}

fn normalize_repository_relative_path(selector: &str) -> String {
    selector
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string()
}

fn normalize_spec_system_selector(selector: &str) -> String {
    let path = normalize_repository_relative_path(selector);
    if let Some(stripped) = path.strip_prefix(".allow/") {
        stripped.to_string()
    } else {
        path
    }
}
