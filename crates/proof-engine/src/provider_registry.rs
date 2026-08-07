//! Provider registry for proof-engine orchestration (#2589-A).

use crate::provider_api::{ProofProviderV1, ProviderApiError, validate_provider_surface};

pub const PROVIDER_REGISTRY_SCHEMA_ID: &str = "proof.provider-registry.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderRegistryEntryV1 {
    pub provider_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderRegistryV1 {
    pub schema_id: String,
    pub entries: Vec<ProviderRegistryEntryV1>,
}

impl ProviderRegistryV1 {
    pub fn new(entries: Vec<ProviderRegistryEntryV1>) -> Self {
        Self {
            schema_id: PROVIDER_REGISTRY_SCHEMA_ID.to_string(),
            entries,
        }
    }

    pub fn register_provider(&mut self, provider_id: impl Into<String>) {
        let provider_id = provider_id.into();
        if self
            .entries
            .iter()
            .any(|entry| entry.provider_id == provider_id)
        {
            return;
        }
        self.entries.push(ProviderRegistryEntryV1 { provider_id });
    }

    pub fn contains(&self, provider_id: &str) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.provider_id == provider_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderRegistryError {
    InvalidSchemaId { observed: String },
    EmptyEntries,
    ProviderNotRegistered { provider_id: String },
    ProviderSurface(ProviderApiError),
}

impl ProviderRegistryError {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::InvalidSchemaId { .. } => "invalid_schema_id",
            Self::EmptyEntries => "empty_entries",
            Self::ProviderNotRegistered { .. } => "provider_not_registered",
            Self::ProviderSurface(_) => "provider_surface_invalid",
        }
    }
}

pub fn validate_provider_registry(
    registry: &ProviderRegistryV1,
) -> Result<(), ProviderRegistryError> {
    if registry.schema_id != PROVIDER_REGISTRY_SCHEMA_ID {
        return Err(ProviderRegistryError::InvalidSchemaId {
            observed: registry.schema_id.clone(),
        });
    }
    if registry.entries.is_empty() {
        return Err(ProviderRegistryError::EmptyEntries);
    }
    Ok(())
}

pub fn register_validated_provider(
    registry: &mut ProviderRegistryV1,
    provider: &dyn ProofProviderV1,
) -> Result<(), ProviderRegistryError> {
    validate_provider_surface(provider).map_err(ProviderRegistryError::ProviderSurface)?;
    registry.register_provider(provider.provider_id());
    Ok(())
}

pub fn require_registered_provider(
    registry: &ProviderRegistryV1,
    provider_id: &str,
) -> Result<(), ProviderRegistryError> {
    validate_provider_registry(registry)?;
    if registry.contains(provider_id) {
        Ok(())
    } else {
        Err(ProviderRegistryError::ProviderNotRegistered {
            provider_id: provider_id.to_string(),
        })
    }
}
