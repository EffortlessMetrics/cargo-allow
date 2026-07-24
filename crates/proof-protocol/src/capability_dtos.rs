//! Provider capability catalog transport (#2588-B).
//!
//! Describes provider-advertised proof capabilities without executing them.

use serde::{Deserialize, Serialize};

pub const PROOF_CAPABILITY_CATALOG_SCHEMA_ID: &str = "proof.capability-catalog.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProofCapabilityKindV1 {
    CommandArgv,
    StaticReport,
}

impl ProofCapabilityKindV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CommandArgv => "command_argv",
            Self::StaticReport => "static_report",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofCapabilityV1 {
    pub capability_id: String,
    pub kind: ProofCapabilityKindV1,
    pub program: String,
    pub statement: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofCapabilityCatalogV1 {
    pub schema_id: String,
    pub provider_id: String,
    pub capabilities: Vec<ProofCapabilityV1>,
}

impl ProofCapabilityCatalogV1 {
    pub fn new(provider_id: impl Into<String>, capabilities: Vec<ProofCapabilityV1>) -> Self {
        Self {
            schema_id: PROOF_CAPABILITY_CATALOG_SCHEMA_ID.to_string(),
            provider_id: provider_id.into(),
            capabilities,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofCapabilityError {
    InvalidSchemaId { observed: String },
    EmptyCatalog,
    DuplicateCapabilityId { capability_id: String },
}

impl ProofCapabilityError {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::InvalidSchemaId { .. } => "invalid_schema_id",
            Self::EmptyCatalog => "empty_catalog",
            Self::DuplicateCapabilityId { .. } => "duplicate_capability_id",
        }
    }
}

pub fn validate_capability_catalog(
    catalog: &ProofCapabilityCatalogV1,
) -> Result<(), ProofCapabilityError> {
    if catalog.schema_id != PROOF_CAPABILITY_CATALOG_SCHEMA_ID {
        return Err(ProofCapabilityError::InvalidSchemaId {
            observed: catalog.schema_id.clone(),
        });
    }
    if catalog.capabilities.is_empty() {
        return Err(ProofCapabilityError::EmptyCatalog);
    }
    let mut seen = std::collections::BTreeSet::new();
    for capability in &catalog.capabilities {
        if !seen.insert(capability.capability_id.clone()) {
            return Err(ProofCapabilityError::DuplicateCapabilityId {
                capability_id: capability.capability_id.clone(),
            });
        }
    }
    Ok(())
}
