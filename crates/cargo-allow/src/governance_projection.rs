//! Bounded governance projection for the source-coupling guard
//! (#3548 / #2942 step 7).
//!
//! cargo-allow cannot consume intent-model at runtime (dependency law), so
//! the check pipeline owns this minimal reader of exactly the governance
//! fields the guard needs: crate identity (path, owner, library name,
//! aliases) and product-level forbidden dependencies. It is a bounded
//! projection, not a canonical authority: the canonical parsers are the
//! intent-model compat surface (#3327), and parity is proven at dev scope
//! by `governance_adapter_window_tests` plus the projection's own
//! equivalence test.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use allow_core::{CargoAllowError, CargoAllowResult};
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CrateIdentityProjection {
    pub logical_id: String,
    pub workspace_path: String,
    pub cargo_package_name: String,
    pub workspace_dependency_aliases: Vec<String>,
    pub rust_library_name: String,
    pub product_or_shared_owner: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct GovernanceProjection {
    pub crate_identities: Vec<CrateIdentityProjection>,
    pub forbidden_product_dependencies: BTreeMap<String, BTreeSet<String>>,
}

/// Load the governance projection from the live authority files.
///
/// Tolerant of unmodeled authority fields; strict on the projected fields
/// (non-empty identity keys, unique logical ids).
pub(crate) fn load_governance_projection_at(root: &Path) -> CargoAllowResult<GovernanceProjection> {
    #[derive(Deserialize)]
    struct IdentityManifestToml {
        #[serde(default)]
        crate_identity: Vec<IdentityRowToml>,
    }
    #[derive(Deserialize)]
    struct IdentityRowToml {
        logical_id: String,
        workspace_path: String,
        cargo_package_name: String,
        #[serde(default)]
        workspace_dependency_aliases: Vec<String>,
        rust_library_name: String,
        product_or_shared_owner: String,
    }
    #[derive(Deserialize)]
    struct ProductsToml {
        #[serde(default)]
        product: Vec<ProductRowToml>,
    }
    #[derive(Deserialize)]
    struct ProductRowToml {
        id: String,
        #[serde(default)]
        forbid_product_dependencies: Vec<String>,
    }

    let identity_text = std::fs::read_to_string(root.join("policy/product-crates-v2.toml"))
        .map_err(|error| {
            CargoAllowError::new(format!("governance identity authority unreadable: {error}"))
        })?;
    let products_text =
        std::fs::read_to_string(root.join("policy/product-crates.toml")).map_err(|error| {
            CargoAllowError::new(format!("governance law authority unreadable: {error}"))
        })?;

    let identity_manifest: IdentityManifestToml = toml::from_str(&identity_text)
        .map_err(|error| CargoAllowError::new(format!("governance identity parse: {error}")))?;
    let products: ProductsToml = toml::from_str(&products_text)
        .map_err(|error| CargoAllowError::new(format!("governance law parse: {error}")))?;

    let mut crate_identities = Vec::with_capacity(identity_manifest.crate_identity.len());
    let mut seen_logical_ids = BTreeSet::new();
    for row in identity_manifest.crate_identity {
        if row.logical_id.trim().is_empty()
            || row.workspace_path.trim().is_empty()
            || row.cargo_package_name.trim().is_empty()
            || row.rust_library_name.trim().is_empty()
            || row.product_or_shared_owner.trim().is_empty()
        {
            return Err(CargoAllowError::new(format!(
                "governance identity row `{}` has an empty projected field",
                row.logical_id
            )));
        }
        if !seen_logical_ids.insert(row.logical_id.clone()) {
            return Err(CargoAllowError::new(format!(
                "duplicate governance logical id `{}`",
                row.logical_id
            )));
        }
        crate_identities.push(CrateIdentityProjection {
            logical_id: row.logical_id,
            workspace_path: row.workspace_path,
            cargo_package_name: row.cargo_package_name,
            workspace_dependency_aliases: row.workspace_dependency_aliases,
            rust_library_name: row.rust_library_name,
            product_or_shared_owner: row.product_or_shared_owner,
        });
    }

    let mut forbidden_product_dependencies = BTreeMap::new();
    for row in products.product {
        if row.id.trim().is_empty() {
            return Err(CargoAllowError::new(
                "governance law has a product row with an empty id",
            ));
        }
        forbidden_product_dependencies.insert(
            row.id,
            row.forbid_product_dependencies.into_iter().collect(),
        );
    }

    Ok(GovernanceProjection {
        crate_identities,
        forbidden_product_dependencies,
    })
}

/// Resolve the crate identity owning a repository path (longest prefix).
pub(crate) fn crate_identity_for_path<'a>(
    projection: &'a GovernanceProjection,
    path: &str,
) -> Option<&'a CrateIdentityProjection> {
    projection
        .crate_identities
        .iter()
        .filter(|identity| {
            let root = identity.workspace_path.trim_end_matches('/');
            path == root
                || (path.starts_with(root) && path.as_bytes().get(root.len()) == Some(&b'/'))
        })
        .max_by_key(|identity| identity.workspace_path.len())
}

/// Map library names and workspace aliases to their owning identity.
pub(crate) fn identity_owners(
    projection: &GovernanceProjection,
) -> BTreeMap<String, (&CrateIdentityProjection, String)> {
    let mut owners = BTreeMap::new();
    for identity in &projection.crate_identities {
        let owner = identity.product_or_shared_owner.clone();
        owners.insert(
            normalize_segment(&identity.rust_library_name),
            (identity, owner.clone()),
        );
        for alias in &identity.workspace_dependency_aliases {
            owners.insert(normalize_segment(alias), (identity, owner.clone()));
        }
    }
    owners
}

pub(crate) fn forbidden_product_targets(
    projection: &GovernanceProjection,
) -> std::collections::BTreeMap<String, std::collections::BTreeSet<String>> {
    projection
        .forbidden_product_dependencies
        .iter()
        .map(|(from, targets)| (from.clone(), targets.iter().cloned().collect()))
        .collect()
}

fn normalize_segment(value: &str) -> String {
    value.trim().replace('-', "_")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn projection_matches_allow_policy_canonical_parsers() -> Result<(), String> {
        // Bounded parity: the projection must agree with the canonical
        // allow-policy parsers on every projected field (dev scope).
        let root = repo_root();
        let projection = load_governance_projection_at(&root).map_err(|err| format!("{err}"))?;

        let identity_text = std::fs::read_to_string(root.join("policy/product-crates-v2.toml"))
            .map_err(|err| err.to_string())?;
        let manifest =
            allow_policy::product_crates::v2::parse_architecture_manifest_v2(&identity_text)
                .map_err(|err| format!("{err}"))?;
        if projection.crate_identities.len() != manifest.crate_identity.len() {
            return Err(format!(
                "identity count drift: projection {} vs canonical {}",
                projection.crate_identities.len(),
                manifest.crate_identity.len()
            ));
        }
        for projected in &projection.crate_identities {
            let canonical = manifest
                .crate_identity
                .iter()
                .find(|row| row.logical_id == projected.logical_id)
                .ok_or_else(|| {
                    format!("logical id missing canonically: {}", projected.logical_id)
                })?;
            if canonical.workspace_path != projected.workspace_path
                || canonical.rust_library_name != projected.rust_library_name
                || canonical.product_or_shared_owner != projected.product_or_shared_owner
                || canonical.workspace_dependency_aliases != projected.workspace_dependency_aliases
            {
                return Err(format!(
                    "identity field drift for `{}`: {:?} vs {:?}",
                    projected.logical_id, projected, canonical
                ));
            }
        }

        let law_text = std::fs::read_to_string(root.join("policy/product-crates.toml"))
            .map_err(|err| err.to_string())?;
        let law = allow_policy::product_crates::parse_architecture_manifest(&law_text)
            .map_err(|err| format!("{err}"))?;
        for product in &law.product {
            let projected_set = projection
                .forbidden_product_dependencies
                .get(&product.id)
                .ok_or_else(|| format!("product missing from projection: {}", product.id))?;
            let canonical_set: BTreeSet<&str> = product
                .forbid_product_dependencies
                .iter()
                .map(String::as_str)
                .collect();
            let projected_set: BTreeSet<&str> = projected_set.iter().map(String::as_str).collect();
            if projected_set != canonical_set {
                return Err(format!(
                    "forbidden dependency drift for product `{}`",
                    product.id
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn longest_prefix_wins_for_path_resolution() -> Result<(), String> {
        let projection = GovernanceProjection {
            crate_identities: vec![
                CrateIdentityProjection {
                    logical_id: "a".to_string(),
                    workspace_path: "crates/a".to_string(),
                    cargo_package_name: "a".to_string(),
                    workspace_dependency_aliases: vec!["a".to_string()],
                    rust_library_name: "a".to_string(),
                    product_or_shared_owner: "shared".to_string(),
                },
                CrateIdentityProjection {
                    logical_id: "a-b".to_string(),
                    workspace_path: "crates/a/b".to_string(),
                    cargo_package_name: "a-b".to_string(),
                    workspace_dependency_aliases: vec![],
                    rust_library_name: "a_b".to_string(),
                    product_or_shared_owner: "shared".to_string(),
                },
            ],
            forbidden_product_dependencies: BTreeMap::new(),
        };
        let direct = crate_identity_for_path(&projection, "crates/a/b/mod.rs")
            .ok_or("nested path must resolve")?;
        if direct.logical_id != "a-b" {
            return Err(format!("longest prefix must win: {}", direct.logical_id));
        }
        let parent = crate_identity_for_path(&projection, "crates/a/src/lib.rs")
            .ok_or("parent path must resolve")?;
        if parent.logical_id != "a" {
            return Err(format!("parent resolution drift: {}", parent.logical_id));
        }
        if crate_identity_for_path(&projection, "crates/other/lib.rs").is_some() {
            return Err("unknown path must not resolve".into());
        }
        Ok(())
    }
}
