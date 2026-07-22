//! Product package topology manifest (#2604).
//!
//! Report-only classification of workspace packages by release family and posture.

mod config;
mod validate;

pub use config::{
    PackagePosture, PackageTopologyEntry, ProductPackageTopology, parse_product_package_topology,
    parse_product_package_topology_at,
};
pub use validate::{
    PackageTopologyDiagnostic, PackageTopologyDiagnosticKind, PackageTopologyReport,
    validate_product_package_topology, validate_product_package_topology_at,
};

#[cfg(test)]
mod tests;
