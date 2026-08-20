//! Deterministic feature-selected provider registry (#2938).

use proof_engine::{ProofProviderV1, validate_provider_surface};
use proof_protocol::{ProofCapabilityCatalogV1, validate_capability_catalog};
use serde::Serialize;

pub const PROVIDER_REGISTRY_SCHEMA_ID: &str = "cargo-proof.provider-registry.v1";
const KNOWN_PROVIDER_IDS: [&str; 3] = ["proof.cargo-allow.v1", "proof.hawk.v1", "proof.ripr.v1"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProviderProjectionV1 {
    pub provider_id: String,
    pub capabilities: ProofCapabilityCatalogV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderDispositionV1 {
    Selected,
    ProviderUnavailable,
    ProviderUnsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProviderAvailabilityV1 {
    pub provider_id: String,
    pub disposition: ProviderDispositionV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderRegistryError {
    InvalidProviderSurface { provider_id: String },
    DuplicateProviderId { provider_id: String },
    DuplicateCapabilityId { capability_id: String },
}

impl ProviderRegistryError {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::InvalidProviderSurface { .. } => "provider_surface_invalid",
            Self::DuplicateProviderId { .. } => "duplicate_provider_id",
            Self::DuplicateCapabilityId { .. } => "duplicate_capability_id",
        }
    }
}

pub struct StaticProviderRegistryV1 {
    providers: Vec<Box<dyn ProofProviderV1>>,
}

impl StaticProviderRegistryV1 {
    pub fn selected() -> Result<Self, ProviderRegistryError> {
        let providers: Vec<Box<dyn ProofProviderV1>> = vec![
            #[cfg(feature = "provider-cargo-allow")]
            Box::new(crate::providers::cargo_allow::CargoAllowProofProviderV1::new()),
            #[cfg(feature = "provider-ripr")]
            Box::new(crate::providers::ripr::RiprProofProviderV1::new()),
            #[cfg(feature = "provider-hawk")]
            Box::new(crate::providers::hawk::HawkProofProviderV1::new()),
        ];
        Self::from_providers(providers)
    }

    pub fn from_providers(
        providers: Vec<Box<dyn ProofProviderV1>>,
    ) -> Result<Self, ProviderRegistryError> {
        let mut provider_ids = std::collections::BTreeSet::new();
        let mut capability_ids = std::collections::BTreeSet::new();
        for provider in &providers {
            validate_provider_surface(provider.as_ref()).map_err(|_| {
                ProviderRegistryError::InvalidProviderSurface {
                    provider_id: provider.provider_id().to_string(),
                }
            })?;
            if provider.capability_catalog().provider_id != provider.provider_id() {
                return Err(ProviderRegistryError::InvalidProviderSurface {
                    provider_id: provider.provider_id().to_string(),
                });
            }
            if !provider_ids.insert(provider.provider_id().to_string()) {
                return Err(ProviderRegistryError::DuplicateProviderId {
                    provider_id: provider.provider_id().to_string(),
                });
            }
            validate_capability_catalog(provider.capability_catalog()).map_err(|_| {
                ProviderRegistryError::InvalidProviderSurface {
                    provider_id: provider.provider_id().to_string(),
                }
            })?;
            for capability in &provider.capability_catalog().capabilities {
                if !capability_ids.insert(capability.capability_id.clone()) {
                    return Err(ProviderRegistryError::DuplicateCapabilityId {
                        capability_id: capability.capability_id.clone(),
                    });
                }
            }
        }
        Ok(Self { providers })
    }

    pub fn projections(&self) -> Vec<ProviderProjectionV1> {
        let mut projections = self
            .providers
            .iter()
            .map(|provider| ProviderProjectionV1 {
                provider_id: provider.provider_id().to_string(),
                capabilities: provider.capability_catalog().clone(),
            })
            .collect::<Vec<_>>();
        projections.sort_by(|left, right| left.provider_id.cmp(&right.provider_id));
        for projection in &mut projections {
            projection
                .capabilities
                .capabilities
                .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
        }
        projections
    }

    pub fn provider_ids(&self) -> Vec<String> {
        self.projections()
            .into_iter()
            .map(|projection| projection.provider_id)
            .collect()
    }

    pub fn provider_available(&self, provider_id: &str) -> bool {
        self.providers
            .iter()
            .any(|provider| provider.provider_id() == provider_id)
    }

    /// Reports every known provider, including feature-disabled providers.
    pub fn availability(&self) -> Vec<ProviderAvailabilityV1> {
        KNOWN_PROVIDER_IDS
            .iter()
            .map(|provider_id| ProviderAvailabilityV1 {
                provider_id: (*provider_id).to_string(),
                disposition: if self.provider_available(provider_id) {
                    ProviderDispositionV1::Selected
                } else {
                    ProviderDispositionV1::ProviderUnavailable
                },
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proof_engine::{FakeProofProviderV1, ProviderApiError};
    use proof_protocol::{ProofCapabilityKindV1, ProofCapabilityV1, ProofPlanV1};

    struct TestProvider {
        id: String,
        catalog: ProofCapabilityCatalogV1,
    }

    impl TestProvider {
        fn new(id: &str, capability_id: &str) -> Self {
            Self {
                id: id.to_string(),
                catalog: ProofCapabilityCatalogV1::new(
                    id,
                    vec![ProofCapabilityV1 {
                        capability_id: capability_id.to_string(),
                        kind: ProofCapabilityKindV1::StaticReport,
                        program: "test".to_string(),
                        statement: "test provider".to_string(),
                    }],
                ),
            }
        }
    }

    impl ProofProviderV1 for TestProvider {
        fn provider_id(&self) -> &str {
            &self.id
        }
        fn capability_catalog(&self) -> &ProofCapabilityCatalogV1 {
            &self.catalog
        }
        fn validate_plan(&self, _plan: &ProofPlanV1) -> Result<(), ProviderApiError> {
            Ok(())
        }
    }

    #[test]
    fn no_default_registry_is_explicitly_empty() -> Result<(), String> {
        let registry = StaticProviderRegistryV1::selected().map_err(|error| error.as_str())?;
        #[cfg(not(any(
            feature = "provider-cargo-allow",
            feature = "provider-ripr",
            feature = "provider-hawk"
        )))]
        if !registry.provider_ids().is_empty() {
            return Err("no-default registry must not select providers".to_string());
        }
        Ok(())
    }

    #[test]
    fn duplicate_provider_ids_fail() -> Result<(), String> {
        let first = Box::new(FakeProofProviderV1::with_id("duplicate"));
        let second = Box::new(FakeProofProviderV1::with_id("duplicate"));
        match StaticProviderRegistryV1::from_providers(vec![first, second]) {
            Err(ProviderRegistryError::DuplicateProviderId { provider_id })
                if provider_id == "duplicate" =>
            {
                Ok(())
            }
            other => Err(format!(
                "unexpected duplicate result: {}",
                other.err().map(|error| error.as_str()).unwrap_or("ok")
            )),
        }
    }

    #[test]
    fn duplicate_capability_ids_fail_across_providers() -> Result<(), String> {
        let first = Box::new(TestProvider::new("first", "shared-capability"));
        let second = Box::new(TestProvider::new("second", "shared-capability"));
        match StaticProviderRegistryV1::from_providers(vec![first, second]) {
            Err(ProviderRegistryError::DuplicateCapabilityId { capability_id })
                if capability_id == "shared-capability" =>
            {
                Ok(())
            }
            other => Err(format!(
                "unexpected duplicate capability result: {}",
                other.err().map(|error| error.as_str()).unwrap_or("ok")
            )),
        }
    }

    #[test]
    fn projections_are_sorted_independently_of_construction_order() -> Result<(), String> {
        let first = Box::new(TestProvider::new("z-provider", "z-capability"));
        let second = Box::new(TestProvider::new("a-provider", "a-capability"));
        let registry = StaticProviderRegistryV1::from_providers(vec![first, second])
            .map_err(|error| error.as_str().to_string())?;
        let ids = registry.provider_ids();
        if ids != vec!["a-provider".to_string(), "z-provider".to_string()] {
            return Err(format!("unexpected provider order: {ids:?}"));
        }
        Ok(())
    }

    #[test]
    fn disabled_provider_is_unavailable_in_read_only_projection() -> Result<(), String> {
        let registry =
            StaticProviderRegistryV1::selected().map_err(|error| error.as_str().to_string())?;
        #[cfg(not(feature = "provider-hawk"))]
        if registry.provider_available("proof.hawk.v1") {
            return Err("disabled hawk provider was reported available".to_string());
        }
        #[cfg(not(feature = "provider-hawk"))]
        if registry
            .availability()
            .into_iter()
            .find(|entry| entry.provider_id == "proof.hawk.v1")
            .map(|entry| entry.disposition)
            != Some(ProviderDispositionV1::ProviderUnavailable)
        {
            return Err("disabled hawk provider lacked unavailable disposition".to_string());
        }
        Ok(())
    }

    #[cfg(feature = "provider-cargo-allow")]
    #[test]
    fn selected_cargo_allow_provider_does_not_resolve_processes() -> Result<(), String> {
        let provider = crate::providers::cargo_allow::CargoAllowProofProviderV1::new();
        if provider.resolution().is_some() {
            return Err("static registry construction unexpectedly resolved a process".to_string());
        }
        Ok(())
    }
}
