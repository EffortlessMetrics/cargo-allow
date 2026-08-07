//! Product package topology manifest (#2604).
//!
//! Report-only classification of workspace packages by release family and posture.

mod config;
mod v2;
mod validate;

pub use config::{
    PackagePosture, PackageTopologyEntry, ProductPackageTopology, parse_product_package_topology,
    parse_product_package_topology_at,
};
pub use v2::{
    PRODUCT_PACKAGE_TOPOLOGY_V2_AUTHORITY_GENERATION, PRODUCT_PACKAGE_TOPOLOGY_V2_SCHEMA_VERSION,
    PackageTopologyEntryV2, ProductPackageTopologyV2, PublicationStateV2, VersionSourceV2,
    parse_product_package_topology_v2, parse_product_package_topology_v2_at,
};
pub use validate::{
    PackageTopologyDiagnostic, PackageTopologyDiagnosticKind, PackageTopologyReport,
    validate_product_package_topology, validate_product_package_topology_at,
};

#[cfg(test)]
mod tests;
