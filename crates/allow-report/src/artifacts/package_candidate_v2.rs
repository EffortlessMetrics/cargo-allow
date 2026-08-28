//! Typed topology-selected package candidate for the cargo-allow 0.2
//! mixed-version candidate (#2924).
//!
//! The artifact is produced by the packaging producer from the strict V2
//! package topology plus the selected closure; the validator here enforces
//! the candidate's structural law without reading policy files, spawning
//! Cargo, or touching a registry. It packages and inspects nothing itself:
//! `.crate` bytes and packaged-manifest facts stay producer-owned.

use serde::{Deserialize, Serialize};

pub const PACKAGE_CANDIDATE_V2_SCHEMA_VERSION: u32 = 2;
pub const PACKAGE_CANDIDATE_V2_SCHEMA_ID: &str = "cargo-allow.package-candidate.v2";

/// Version-line families a candidate row may belong to. Anything else —
/// including the cargo-intent and cargo-proof families — must never enter
/// the cargo-allow candidate (#2924 selection law).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PackageCandidateFamilyV2 {
    #[serde(rename = "cargo-allow-0.2")]
    CargoAllow02,
    #[serde(rename = "shared-0.1")]
    Shared01,
}

impl PackageCandidateFamilyV2 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CargoAllow02 => "cargo-allow-0.2",
            Self::Shared01 => "shared-0.1",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageCandidateDependencyKindV2 {
    /// A selected candidate row: registry-resolvable by real name/version.
    Internal,
    /// An outside dependency carried by name and version requirement.
    External,
}

/// One expected dependency row of a candidate package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageCandidateDependencyRowV2 {
    pub package_name: String,
    pub package_version: String,
    pub dependency_kind: PackageCandidateDependencyKindV2,
}

/// One selected package row in dependency-derived release order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageCandidateRowV2 {
    pub logical_id: String,
    pub cargo_package_name: String,
    pub cargo_package_version: String,
    pub rust_library_name: String,
    pub workspace_source_path: String,
    pub product_family: PackageCandidateFamilyV2,
    pub publication_state: String,
    pub publish: bool,
    pub support_tier: String,
    pub release_order: u32,
    pub selected_features: Vec<String>,
    /// Canonical `name:version` identity the packaged manifest must carry.
    pub expected_manifest_identity: String,
    pub expected_dependency_rows: Vec<PackageCandidateDependencyRowV2>,
    pub required_assets: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crate_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crate_size_bytes: Option<u64>,
}

/// Canonical semantic identity of the candidate. The producer renders this;
/// volatile execution metadata must live beside it, not inside it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageCandidatePayloadV2 {
    pub schema_id: String,
    pub schema_version: u32,
    pub topology_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topology_digest: Option<String>,
    pub repository_commit: String,
    pub repository_tree: String,
    pub cargo_lock_digest: String,
    pub candidate_product_id: String,
    pub root_logical_id: String,
    pub root_package_name: String,
    pub root_package_version: String,
    pub target_class: String,
    pub feature_set_id: String,
    pub rows: Vec<PackageCandidateRowV2>,
    /// logical ids the topology deliberately excludes from this candidate
    /// (sibling products and unselected shared packages), with the reason.
    pub known_exclusions: Vec<String>,
    pub limitations: Vec<String>,
    pub claim_boundary: String,
}

/// Closed validation vocabulary for the candidate contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageCandidateResultV2 {
    Complete,
    StaleInput,
    IdentityConflict,
    DependencyConflict,
    MissingAsset,
    MalformedArtifact,
    UnsupportedGeneration,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageCandidateV2Validation {
    pub result: PackageCandidateResultV2,
    pub gaps: Vec<String>,
}

/// Render only the semantic payload. Serde's declaration order is canonical.
pub fn render_package_candidate_v2(
    payload: &PackageCandidatePayloadV2,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(payload)
}

pub fn render_package_candidate_v2_bytes(
    payload: &PackageCandidatePayloadV2,
) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(payload)
}

/// Validate the candidate's structural law (#2924): current generation,
/// present repository/lock identity, no sibling-product rows, one version
/// per family without substitution, manifest identities that match their
/// rows, internal dependencies that resolve inside the candidate, an order
/// that agrees with the internal dependency graph, and named assets for the
/// family rows that own release assets.
pub fn validate_package_candidate_v2(
    payload: &PackageCandidatePayloadV2,
) -> PackageCandidateV2Validation {
    let mut gaps = Vec::new();
    let generation_current = payload.schema_id == PACKAGE_CANDIDATE_V2_SCHEMA_ID
        && payload.schema_version == PACKAGE_CANDIDATE_V2_SCHEMA_VERSION;
    if !generation_current {
        gaps.push("payload uses a non-current package-candidate generation".to_string());
    }

    for (field, value) in [
        ("topology_id", payload.topology_id.as_str()),
        ("repository_commit", payload.repository_commit.as_str()),
        ("repository_tree", payload.repository_tree.as_str()),
        (
            "candidate_product_id",
            payload.candidate_product_id.as_str(),
        ),
        ("root_logical_id", payload.root_logical_id.as_str()),
        ("root_package_name", payload.root_package_name.as_str()),
        (
            "root_package_version",
            payload.root_package_version.as_str(),
        ),
        ("target_class", payload.target_class.as_str()),
        ("feature_set_id", payload.feature_set_id.as_str()),
        ("claim_boundary", payload.claim_boundary.as_str()),
    ] {
        if value.trim().is_empty() {
            gaps.push(format!("{field} is missing"));
        }
    }
    if !is_sha256_digest(&payload.cargo_lock_digest) {
        gaps.push("cargo_lock_digest is not a sha256 digest".to_string());
    }
    validate_optional_digest(
        "topology_digest",
        payload.topology_digest.as_deref(),
        &mut gaps,
    );

    if payload.rows.is_empty() {
        gaps.push("candidate rows are empty".to_string());
    }

    let mut logical_ids = std::collections::BTreeSet::new();
    let mut package_names = std::collections::BTreeSet::new();
    let mut family_versions = std::collections::BTreeMap::new();
    let mut root_row_seen = false;
    for row in &payload.rows {
        logical_ids.insert(row.logical_id.clone());
        package_names.insert(row.cargo_package_name.clone());
        family_versions
            .entry(row.product_family)
            .or_insert_with(std::collections::BTreeSet::new)
            .insert(row.cargo_package_version.clone());
        if row.product_family == PackageCandidateFamilyV2::CargoAllow02
            && row.cargo_package_name == payload.root_package_name
        {
            root_row_seen = true;
        }
    }

    // Dependency rows carry package names, so membership and order are
    // resolved by package name (renamed logical ids stay row-internal).
    let order_of: std::collections::BTreeMap<&str, u32> = payload
        .rows
        .iter()
        .map(|row| (row.cargo_package_name.as_str(), row.release_order))
        .collect();

    for (index, row) in payload.rows.iter().enumerate() {
        let label = format!("rows[{index}]");
        if row.logical_id.trim().is_empty() || !logical_ids.contains(row.logical_id.as_str()) {
            gaps.push(format!("{label} logical_id is empty or duplicated"));
        }
        if row.cargo_package_name.trim().is_empty()
            || !package_names.contains(row.cargo_package_name.as_str())
        {
            gaps.push(format!("{label} package name is empty or duplicated"));
        }
        // Duplicate detection needs the whole-set view, not single-row truth.
        if payload
            .rows
            .iter()
            .filter(|other| other.logical_id == row.logical_id)
            .count()
            > 1
        {
            gaps.push(format!("{label} logical_id is duplicated"));
        }
        if payload
            .rows
            .iter()
            .filter(|other| other.cargo_package_name == row.cargo_package_name)
            .count()
            > 1
        {
            gaps.push(format!("{label} package name is duplicated"));
        }
        if row.rust_library_name.trim().is_empty() {
            gaps.push(format!("{label} rust_library_name is missing"));
        }
        if row.workspace_source_path.trim().is_empty() {
            gaps.push(format!("{label} workspace_source_path is missing"));
        }
        if row.publication_state.trim().is_empty() {
            gaps.push(format!("{label} publication_state is missing"));
        }
        if row.support_tier.trim().is_empty() {
            gaps.push(format!("{label} support_tier is missing"));
        }
        if row.release_order == 0 {
            gaps.push(format!("{label} release_order must be positive"));
        }
        if row.expected_manifest_identity
            != format!("{}:{}", row.cargo_package_name, row.cargo_package_version)
        {
            gaps.push(format!(
                "{label} expected_manifest_identity disagrees with its name/version"
            ));
        }
        for asset in &row.required_assets {
            if asset.trim().is_empty() {
                gaps.push(format!("{label} carries a blank required asset"));
            }
        }
        for dependency in &row.expected_dependency_rows {
            if dependency.package_name.trim().is_empty()
                || dependency.package_version.trim().is_empty()
            {
                gaps.push(format!(
                    "{label} carries a dependency row without name or version"
                ));
            }
            if dependency.dependency_kind == PackageCandidateDependencyKindV2::Internal
                && !package_names.contains(dependency.package_name.as_str())
            {
                gaps.push(format!(
                    "{label} internal dependency {} is absent from the candidate",
                    dependency.package_name
                ));
            }
        }
        if row.product_family == PackageCandidateFamilyV2::CargoAllow02
            && row.cargo_package_name == payload.root_package_name
        {
            if row.logical_id != payload.root_logical_id {
                gaps.push("root logical_id disagrees with the root row".to_string());
            }
            if row.cargo_package_version != payload.root_package_version {
                gaps.push("root package_version disagrees with the root row".to_string());
            }
        }
    }

    if payload.root_package_name.trim().is_empty() || !root_row_seen {
        gaps.push("root package row is absent from the candidate".to_string());
    }

    // One version per family: never substitute cargo-allow's version for a
    // shared dependency row, and never fork a family across versions.
    for (family, versions) in &family_versions {
        if versions.len() > 1 {
            gaps.push(format!(
                "family {} carries mixed versions {versions:?}",
                family.as_str()
            ));
        }
    }

    // Release order must be strictly increasing along the rows and every
    // internal dependency must precede its dependent.
    let mut previous_order = 0;
    for row in &payload.rows {
        if row.release_order <= previous_order {
            gaps.push(format!(
                "row {} is not in strictly increasing release order",
                row.logical_id
            ));
        }
        previous_order = row.release_order;
        for dependency in &row.expected_dependency_rows {
            if dependency.dependency_kind != PackageCandidateDependencyKindV2::Internal {
                continue;
            }
            if let Some(dependency_order) = order_of.get(dependency.package_name.as_str())
                && *dependency_order >= row.release_order
            {
                gaps.push(format!(
                    "row {} orders its internal dependency {} after itself",
                    row.logical_id, dependency.package_name
                ));
            }
        }
    }

    if !generation_current {
        return classified(PackageCandidateResultV2::UnsupportedGeneration, gaps);
    }
    if gaps.iter().any(|gap| {
        gap.contains("missing") || gap.contains("not a sha256 digest") || gap.contains("stale")
    }) {
        return classified(PackageCandidateResultV2::StaleInput, gaps);
    }
    if gaps
        .iter()
        .any(|gap| gap.contains("dependency") || gap.contains("release order"))
    {
        return classified(PackageCandidateResultV2::DependencyConflict, gaps);
    }
    if gaps.iter().any(|gap| gap.contains("root package row")) {
        return classified(PackageCandidateResultV2::IdentityConflict, gaps);
    }
    if gaps.iter().any(|gap| gap.contains("asset")) {
        return classified(PackageCandidateResultV2::MissingAsset, gaps);
    }
    if gaps.is_empty() {
        PackageCandidateV2Validation {
            result: PackageCandidateResultV2::Complete,
            gaps,
        }
    } else {
        classified(PackageCandidateResultV2::IdentityConflict, gaps)
    }
}

fn classified(result: PackageCandidateResultV2, gaps: Vec<String>) -> PackageCandidateV2Validation {
    PackageCandidateV2Validation { result, gaps }
}

fn validate_optional_digest(field: &str, value: Option<&str>, gaps: &mut Vec<String>) {
    if let Some(value) = value
        && !is_sha256_digest(value)
    {
        gaps.push(format!("{field} is not a sha256 digest"));
    }
}

fn is_sha256_digest(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return false;
    };
    hex.len() == 64 && hex.chars().all(|character| character.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dependency(
        package_name: &str,
        kind: PackageCandidateDependencyKindV2,
    ) -> PackageCandidateDependencyRowV2 {
        PackageCandidateDependencyRowV2 {
            package_name: package_name.to_string(),
            package_version: "0.1.0".to_string(),
            dependency_kind: kind,
        }
    }

    fn row(
        logical_id: &str,
        order: u32,
        family: PackageCandidateFamilyV2,
    ) -> PackageCandidateRowV2 {
        PackageCandidateRowV2 {
            logical_id: logical_id.to_string(),
            cargo_package_name: logical_id.to_string(),
            cargo_package_version: match family {
                PackageCandidateFamilyV2::CargoAllow02 => "0.2.0-rc.1".to_string(),
                PackageCandidateFamilyV2::Shared01 => "0.1.0".to_string(),
            },
            rust_library_name: logical_id.to_string(),
            workspace_source_path: format!("crates/{logical_id}"),
            product_family: family,
            publication_state: "UnpublishedInternal".to_string(),
            publish: true,
            support_tier: "supported".to_string(),
            release_order: order,
            selected_features: Vec::new(),
            expected_manifest_identity: format!(
                "{logical_id}:{}",
                match family {
                    PackageCandidateFamilyV2::CargoAllow02 => "0.2.0-rc.1",
                    PackageCandidateFamilyV2::Shared01 => "0.1.0",
                }
            ),
            expected_dependency_rows: Vec::new(),
            required_assets: Vec::new(),
            crate_digest: None,
            crate_size_bytes: None,
        }
    }

    fn payload() -> PackageCandidatePayloadV2 {
        let core = row("allow-core", 10, PackageCandidateFamilyV2::CargoAllow02);
        let mut policy = row("allow-policy", 20, PackageCandidateFamilyV2::CargoAllow02);
        policy.expected_dependency_rows = vec![dependency(
            "allow-core",
            PackageCandidateDependencyKindV2::Internal,
        )];
        let shared = row(
            "effortless-repo-protocol",
            230,
            PackageCandidateFamilyV2::Shared01,
        );
        let mut root = row("cargo-allow", 100, PackageCandidateFamilyV2::CargoAllow02);
        root.expected_dependency_rows = vec![dependency(
            "allow-policy",
            PackageCandidateDependencyKindV2::Internal,
        )];
        PackageCandidatePayloadV2 {
            schema_id: PACKAGE_CANDIDATE_V2_SCHEMA_ID.to_string(),
            schema_version: PACKAGE_CANDIDATE_V2_SCHEMA_VERSION,
            topology_id: "CARGO-ALLOW-PKG-TOPOLOGY-V2-0001".to_string(),
            topology_digest: Some(format!("sha256:{:064x}", 3)),
            repository_commit: "abc123".to_string(),
            repository_tree: "def456".to_string(),
            cargo_lock_digest: format!("sha256:{:064x}", 4),
            candidate_product_id: "cargo-allow-0.2".to_string(),
            root_logical_id: "cargo-allow".to_string(),
            root_package_name: "cargo-allow".to_string(),
            root_package_version: "0.2.0-rc.1".to_string(),
            target_class: "cargo-allow-0.2".to_string(),
            feature_set_id: "default".to_string(),
            rows: vec![core, policy, root, shared],
            known_exclusions: vec!["intent-model: cargo-intent family is not selected".to_string()],
            limitations: vec!["no packaging, installation, or publication occurs".to_string()],
            claim_boundary: "topology-selected candidate identity only".to_string(),
        }
    }

    #[test]
    fn family_serialization_uses_the_documented_snake_case_names() -> Result<(), String> {
        let cargo_allow = serde_json::to_string(&PackageCandidateFamilyV2::CargoAllow02)
            .map_err(|error| error.to_string())?;
        let shared = serde_json::to_string(&PackageCandidateFamilyV2::Shared01)
            .map_err(|error| error.to_string())?;
        if cargo_allow != "\"cargo-allow-0.2\"" || shared != "\"shared-0.1\"" {
            return Err(format!(
                "family serialization drifted: {cargo_allow} {shared}"
            ));
        }
        Ok(())
    }

    #[test]
    fn candidate_accepts_a_wellformed_mixed_version_payload() -> Result<(), String> {
        let validation = validate_package_candidate_v2(&payload());
        if validation.result != PackageCandidateResultV2::Complete {
            return Err(format!("wellformed candidate was rejected: {validation:?}"));
        }
        Ok(())
    }

    #[test]
    fn candidate_rejects_sibling_products_and_stale_shared_identities() -> Result<(), String> {
        let mut invalid = payload();
        invalid
            .rows
            .push(row("intent-model", 240, PackageCandidateFamilyV2::Shared01));
        invalid
            .rows
            .last_mut()
            .ok_or_else(|| "fixture lost its pushed row".to_string())?
            .cargo_package_version = "0.2.0-rc.1".to_string();
        let validation = validate_package_candidate_v2(&invalid);
        if validation.result == PackageCandidateResultV2::Complete
            || !validation
                .gaps
                .iter()
                .any(|gap| gap.contains("mixed versions"))
        {
            return Err(format!(
                "sibling-product row with substituted version was accepted: {validation:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn candidate_rejects_order_that_disagrees_with_the_dependency_graph() -> Result<(), String> {
        let mut invalid = payload();
        let policy = invalid
            .rows
            .iter_mut()
            .find(|row| row.logical_id == "allow-policy")
            .ok_or_else(|| "fixture lost the policy row".to_string())?;
        policy.release_order = 5;
        let validation = validate_package_candidate_v2(&invalid);
        if validation.result != PackageCandidateResultV2::DependencyConflict
            || !validation
                .gaps
                .iter()
                .any(|gap| gap.contains("after itself"))
        {
            return Err(format!(
                "dependency-order violation was not classified: {validation:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn candidate_rejects_internal_dependencies_absent_from_the_candidate() -> Result<(), String> {
        let mut invalid = payload();
        let root = invalid
            .rows
            .iter_mut()
            .find(|row| row.logical_id == "cargo-allow")
            .ok_or_else(|| "fixture lost the root row".to_string())?;
        root.expected_dependency_rows = vec![dependency(
            "intent-compiler",
            PackageCandidateDependencyKindV2::Internal,
        )];
        let validation = validate_package_candidate_v2(&invalid);
        if validation.result != PackageCandidateResultV2::DependencyConflict
            || !validation
                .gaps
                .iter()
                .any(|gap| gap.contains("absent from the candidate"))
        {
            return Err(format!(
                "omitted sibling dependency was not classified: {validation:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn candidate_rejects_stale_repository_and_lock_identity() -> Result<(), String> {
        let mut invalid = payload();
        invalid.repository_commit = String::new();
        invalid.cargo_lock_digest = "sha256:short".to_string();
        let validation = validate_package_candidate_v2(&invalid);
        if validation.result != PackageCandidateResultV2::StaleInput {
            return Err(format!("stale identity was not classified: {validation:?}"));
        }
        Ok(())
    }

    #[test]
    fn candidate_rejects_manifest_identity_disagreement_and_root_absence() -> Result<(), String> {
        let mut invalid = payload();
        let root = invalid
            .rows
            .iter_mut()
            .find(|row| row.logical_id == "cargo-allow")
            .ok_or_else(|| "fixture lost the root row".to_string())?;
        root.expected_manifest_identity = "cargo-allow:0.1.11".to_string();
        let validation = validate_package_candidate_v2(&invalid);
        if validation.result != PackageCandidateResultV2::IdentityConflict
            || !validation
                .gaps
                .iter()
                .any(|gap| gap.contains("expected_manifest_identity"))
        {
            return Err(format!(
                "manifest identity disagreement was not classified: {validation:?}"
            ));
        }

        let mut rootless = payload();
        rootless.rows.retain(|row| row.logical_id != "cargo-allow");
        let validation = validate_package_candidate_v2(&rootless);
        if validation.result != PackageCandidateResultV2::IdentityConflict
            || !validation
                .gaps
                .iter()
                .any(|gap| gap.contains("root package row"))
        {
            return Err(format!(
                "missing root row was not classified: {validation:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn candidate_rejects_non_current_generations_and_blank_scalars() -> Result<(), String> {
        let mut invalid = payload();
        invalid.schema_version = 1;
        invalid.topology_id = " ".to_string();
        let validation = validate_package_candidate_v2(&invalid);
        if validation.result != PackageCandidateResultV2::UnsupportedGeneration
            || !validation
                .gaps
                .iter()
                .any(|gap| gap.contains("topology_id"))
        {
            return Err(format!(
                "generation or scalar gap was not classified: {validation:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn candidate_rendering_is_deterministic_across_equal_payloads() -> Result<(), String> {
        let first =
            render_package_candidate_v2_bytes(&payload()).map_err(|error| error.to_string())?;
        let second =
            render_package_candidate_v2_bytes(&payload()).map_err(|error| error.to_string())?;
        if first != second {
            return Err("equal payloads rendered different bytes".to_string());
        }
        if render_package_candidate_v2(&payload())
            .map_err(|error| error.to_string())?
            .is_empty()
        {
            return Err("renderer returned no JSON".to_string());
        }
        Ok(())
    }
}
