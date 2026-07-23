//! Import root configuration DTOs used by spec-system config (#2584-B).

use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImportNodeRole {
    Owned,
    Imported,
    Legacy,
    Generated,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ImportRootEntry {
    pub id: String,
    pub path: String,
    pub ecosystem: String,
    pub role: ImportNodeRole,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct ImportRootsConfig {
    pub owned: Option<String>,
    #[serde(default)]
    pub entries: Vec<ImportRootEntry>,
}
