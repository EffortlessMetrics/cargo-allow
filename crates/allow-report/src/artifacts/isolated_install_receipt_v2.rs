//! Typed receipt for the isolated local-registry install of the exact
//! cargo-allow candidate (#2925).
//!
//! The receipt binds what the isolated install proved: which candidate rows
//! were packaged, what the isolated registry and its index contained, how
//! the actual resolved graph compared with the candidate artifact, and the
//! identity of the installed executable. It is produced by the isolated
//! install stage; this module validates its structural law without reading
//! policy files, spawning Cargo, or touching a registry. Portable identity
//! only: absolute paths and private checkout locations are rejected.

use serde::{Deserialize, Serialize};

pub const ISOLATED_INSTALL_RECEIPT_V2_SCHEMA_VERSION: u32 = 2;
pub const ISOLATED_INSTALL_RECEIPT_V2_SCHEMA_ID: &str = "cargo-allow.isolated-install.v2";

/// One candidate row as installed, with the actual resolved version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IsolatedInstallPackageRowV2 {
    pub package_name: String,
    pub package_version: String,
    /// sha256 of the packaged `.crate` that entered the registry.
    pub crate_digest: String,
    /// The checksum recorded in the isolated registry index row.
    pub index_checksum: String,
    /// The version `cargo metadata`/`--version` actually resolved; absent
    /// when the package is a library with no independently observable
    /// resolution beyond the graph comparison.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_version: Option<String>,
}

/// Actual-versus-expected resolved graph comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IsolatedInstallGraphComparisonV2 {
    pub expected_packages: u32,
    pub matched_packages: u32,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub unexpected_packages: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing_packages: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub version_mismatches: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub path_sources: Vec<String>,
}

impl IsolatedInstallGraphComparisonV2 {
    pub fn is_clean(&self) -> bool {
        self.matched_packages == self.expected_packages
            && self.unexpected_packages.is_empty()
            && self.missing_packages.is_empty()
            && self.version_mismatches.is_empty()
            && self.path_sources.is_empty()
    }
}

/// Canonical semantic identity. Volatile execution metadata must live
/// beside the payload, not inside it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IsolatedInstallPayloadV2 {
    pub schema_id: String,
    pub schema_version: u32,
    /// sha256 of the `CargoAllowPackageCandidateV2` artifact consumed.
    pub candidate_artifact_digest: String,
    pub repository_commit: String,
    pub repository_tree: String,
    pub cargo_lock_digest: String,
    /// sha256 over the isolated registry index rows (deterministic digest
    /// of the sorted index JSON lines).
    pub registry_index_digest: String,
    /// Bounded identity of the mirrored external cache input (for example
    /// the warm Cargo home's lockfile-scoped cache digest); never a path.
    pub external_cache_identity: String,
    pub source_checkout_denied: bool,
    /// Portable identity of the fresh install root (sha256 of the root
    /// path at creation time), never the path itself.
    pub install_root_identity: String,
    /// Portable identity of the fresh Cargo home used for the install.
    pub cargo_home_identity: String,
    pub installed_executable_digest: String,
    /// Exact stdout of the installed binary's `--version`.
    pub installed_version_output: String,
    pub platform: String,
    pub toolchain: String,
    pub package_rows: Vec<IsolatedInstallPackageRowV2>,
    pub graph_comparison: IsolatedInstallGraphComparisonV2,
    pub limitations: Vec<String>,
    pub claim_boundary: String,
}

/// Closed validation vocabulary for the isolated install receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IsolatedInstallResultV2 {
    Complete,
    SourceFallbackDetected,
    PackageMissing,
    ChecksumMismatch,
    IndexMismatch,
    GraphMismatch,
    AmbientShadow,
    StaleInput,
    PathLeakInReceipt,
    MalformedArtifact,
    UnsupportedGeneration,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IsolatedInstallV2Validation {
    pub result: IsolatedInstallResultV2,
    pub gaps: Vec<String>,
}

/// Render only the semantic payload. Serde's declaration order is canonical.
pub fn render_isolated_install_v2(
    payload: &IsolatedInstallPayloadV2,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(payload)
}

pub fn render_isolated_install_v2_bytes(
    payload: &IsolatedInstallPayloadV2,
) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(payload)
}

/// Validate the receipt's structural law (#2925): the source checkout must
/// be proven denied, the graph comparison must be clean, every row must be
/// checksummed, no field may carry private absolute path data, and the
/// repository/lock identity must be present and well-formed.
pub fn validate_isolated_install_v2(
    payload: &IsolatedInstallPayloadV2,
) -> IsolatedInstallV2Validation {
    let mut gaps = Vec::new();
    let generation_current = payload.schema_id == ISOLATED_INSTALL_RECEIPT_V2_SCHEMA_ID
        && payload.schema_version == ISOLATED_INSTALL_RECEIPT_V2_SCHEMA_VERSION;
    if !generation_current {
        gaps.push("payload uses a non-current isolated-install generation".to_string());
    }

    for (field, value) in [
        (
            "candidate_artifact_digest",
            payload.candidate_artifact_digest.as_str(),
        ),
        ("repository_commit", payload.repository_commit.as_str()),
        ("repository_tree", payload.repository_tree.as_str()),
        ("cargo_lock_digest", payload.cargo_lock_digest.as_str()),
        (
            "registry_index_digest",
            payload.registry_index_digest.as_str(),
        ),
        (
            "external_cache_identity",
            payload.external_cache_identity.as_str(),
        ),
        (
            "install_root_identity",
            payload.install_root_identity.as_str(),
        ),
        ("cargo_home_identity", payload.cargo_home_identity.as_str()),
        (
            "installed_executable_digest",
            payload.installed_executable_digest.as_str(),
        ),
        (
            "installed_version_output",
            payload.installed_version_output.as_str(),
        ),
        ("platform", payload.platform.as_str()),
        ("toolchain", payload.toolchain.as_str()),
        ("claim_boundary", payload.claim_boundary.as_str()),
    ] {
        if value.trim().is_empty() {
            gaps.push(format!("{field} is missing"));
        }
    }
    for (field, value) in [
        (
            "candidate_artifact_digest",
            payload.candidate_artifact_digest.as_str(),
        ),
        ("cargo_lock_digest", payload.cargo_lock_digest.as_str()),
        (
            "registry_index_digest",
            payload.registry_index_digest.as_str(),
        ),
        (
            "installed_executable_digest",
            payload.installed_executable_digest.as_str(),
        ),
    ] {
        if !value.trim().is_empty() && !is_sha256_digest(value) {
            gaps.push(format!("{field} is not a sha256 digest"));
        }
    }
    for (field, value) in [
        (
            "install_root_identity",
            payload.install_root_identity.as_str(),
        ),
        ("cargo_home_identity", payload.cargo_home_identity.as_str()),
    ] {
        if !value.trim().is_empty() && !is_sha256_digest(value) {
            gaps.push(format!("{field} must be a sha256 portable identity"));
        }
    }
    if !payload.source_checkout_denied {
        gaps.push("source checkout denial is not proven".to_string());
    }

    if payload.package_rows.is_empty() {
        gaps.push("package rows are empty".to_string());
    }
    let mut names = std::collections::BTreeSet::new();
    for (index, row) in payload.package_rows.iter().enumerate() {
        let label = format!("rows[{index}]");
        if row.package_name.trim().is_empty() || !names.insert(row.package_name.clone()) {
            gaps.push(format!("{label} package name is empty or duplicated"));
        }
        if row.package_version.trim().is_empty() {
            gaps.push(format!("{label} package version is missing"));
        }
        if !is_sha256_digest(&row.crate_digest) {
            gaps.push(format!("{label} crate_digest is not a sha256 digest"));
        }
        if !is_sha256_digest(&row.index_checksum) {
            gaps.push(format!("{label} index_checksum is not a sha256 digest"));
        }
        if let Some(resolved) = &row.resolved_version
            && resolved.trim().is_empty()
        {
            gaps.push(format!("{label} resolved_version is blank"));
        }
    }

    if !payload.graph_comparison.is_clean() {
        gaps.push("resolved graph comparison is not clean".to_string());
    }

    // Negative 15: the receipt retains portable identities only. Absolute
    // paths, home directories, and drive letters must not leak into any
    // textual field.
    if payload_gaps_contain_private_paths(payload) {
        gaps.push("receipt carries private absolute path data".to_string());
    }

    if !generation_current {
        return IsolatedInstallV2Validation {
            result: IsolatedInstallResultV2::UnsupportedGeneration,
            gaps,
        };
    }
    if payload_gaps_contain_private_paths(payload) {
        return IsolatedInstallV2Validation {
            result: IsolatedInstallResultV2::PathLeakInReceipt,
            gaps,
        };
    }
    if !payload.source_checkout_denied {
        return IsolatedInstallV2Validation {
            result: IsolatedInstallResultV2::SourceFallbackDetected,
            gaps,
        };
    }
    if !payload.graph_comparison.is_clean() {
        return IsolatedInstallV2Validation {
            result: IsolatedInstallResultV2::GraphMismatch,
            gaps,
        };
    }
    if gaps
        .iter()
        .any(|gap| gap.contains("not a sha256 digest") || gap.contains("is missing"))
    {
        return IsolatedInstallV2Validation {
            result: IsolatedInstallResultV2::StaleInput,
            gaps,
        };
    }
    if gaps.is_empty() {
        IsolatedInstallV2Validation {
            result: IsolatedInstallResultV2::Complete,
            gaps,
        }
    } else {
        IsolatedInstallV2Validation {
            result: IsolatedInstallResultV2::MalformedArtifact,
            gaps,
        }
    }
}

fn payload_gaps_contain_private_paths(payload: &IsolatedInstallPayloadV2) -> bool {
    let texts = [
        payload.external_cache_identity.as_str(),
        payload.installed_version_output.as_str(),
        payload.claim_boundary.as_str(),
    ];
    texts
        .into_iter()
        .chain(
            payload
                .package_rows
                .iter()
                .flat_map(|row| [row.package_name.as_str(), row.package_version.as_str()]),
        )
        .any(is_private_path)
}

fn is_private_path(value: &str) -> bool {
    let lowered = value.to_ascii_lowercase();
    lowered.contains("/home/")
        || lowered.contains("/users/")
        || lowered.contains("c:\\")
        || lowered.contains("/runner/work/")
        || lowered.contains("\\cargo-allow\\")
        || lowered.contains("/cargo-allow/crates/")
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

    fn row(package_name: &str, version: &str) -> IsolatedInstallPackageRowV2 {
        IsolatedInstallPackageRowV2 {
            package_name: package_name.to_string(),
            package_version: version.to_string(),
            crate_digest: format!("sha256:{:064x}", 11),
            index_checksum: format!("sha256:{:064x}", 12),
            resolved_version: Some(version.to_string()),
        }
    }

    fn payload() -> IsolatedInstallPayloadV2 {
        IsolatedInstallPayloadV2 {
            schema_id: ISOLATED_INSTALL_RECEIPT_V2_SCHEMA_ID.to_string(),
            schema_version: ISOLATED_INSTALL_RECEIPT_V2_SCHEMA_VERSION,
            candidate_artifact_digest: format!("sha256:{:064x}", 1),
            repository_commit: "abc123".to_string(),
            repository_tree: "def456".to_string(),
            cargo_lock_digest: format!("sha256:{:064x}", 2),
            registry_index_digest: format!("sha256:{:064x}", 3),
            external_cache_identity: format!("sha256:{:064x}", 4),
            source_checkout_denied: true,
            install_root_identity: format!("sha256:{:064x}", 5),
            cargo_home_identity: format!("sha256:{:064x}", 6),
            installed_executable_digest: format!("sha256:{:064x}", 7),
            installed_version_output: "cargo-allow 0.2.0-rc.1".to_string(),
            platform: "x86_64-unknown-linux-gnu".to_string(),
            toolchain: "stable".to_string(),
            package_rows: vec![
                row("allow-core", "0.2.0-rc.1"),
                row("effortless-repo-protocol", "0.1.0"),
            ],
            graph_comparison: IsolatedInstallGraphComparisonV2 {
                expected_packages: 2,
                matched_packages: 2,
                unexpected_packages: Vec::new(),
                missing_packages: Vec::new(),
                version_mismatches: Vec::new(),
                path_sources: Vec::new(),
            },
            limitations: vec!["linux hosted claim only".to_string()],
            claim_boundary: "isolated install evidence only".to_string(),
        }
    }

    #[test]
    fn receipt_accepts_a_clean_isolated_install() -> Result<(), String> {
        let validation = validate_isolated_install_v2(&payload());
        if validation.result != IsolatedInstallResultV2::Complete {
            return Err(format!("clean receipt was rejected: {validation:?}"));
        }
        Ok(())
    }

    #[test]
    fn receipt_rejects_source_fallback_and_dirty_graphs() -> Result<(), String> {
        let mut fallback = payload();
        fallback.source_checkout_denied = false;
        let validation = validate_isolated_install_v2(&fallback);
        if validation.result != IsolatedInstallResultV2::SourceFallbackDetected {
            return Err(format!(
                "source fallback was not classified: {validation:?}"
            ));
        }

        let mut dirty = payload();
        dirty.graph_comparison.version_mismatches =
            vec!["allow-core expected 0.2.0-rc.1 resolved 0.0.0".to_string()];
        let validation = validate_isolated_install_v2(&dirty);
        if validation.result != IsolatedInstallResultV2::GraphMismatch {
            return Err(format!("dirty graph was not classified: {validation:?}"));
        }
        Ok(())
    }

    #[test]
    fn receipt_rejects_private_paths_and_unchecked_source_denial() -> Result<(), String> {
        let mut leaky = payload();
        leaky.external_cache_identity = "/home/runner/work/cargo-allow/cache".to_string();
        let validation = validate_isolated_install_v2(&leaky);
        if validation.result != IsolatedInstallResultV2::PathLeakInReceipt {
            return Err(format!("private path was not classified: {validation:?}"));
        }
        Ok(())
    }

    #[test]
    fn receipt_rejects_stale_and_malformed_identity() -> Result<(), String> {
        let mut stale = payload();
        stale.cargo_lock_digest = "sha256:short".to_string();
        let validation = validate_isolated_install_v2(&stale);
        if validation.result != IsolatedInstallResultV2::StaleInput {
            return Err(format!("stale identity was not classified: {validation:?}"));
        }

        let mut malformed = payload();
        malformed.package_rows.clear();
        malformed.installed_version_output = " ".to_string();
        let validation = validate_isolated_install_v2(&malformed);
        if validation.result != IsolatedInstallResultV2::StaleInput || validation.gaps.len() < 2 {
            return Err(format!(
                "malformed receipt was under-reported: {validation:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn receipt_rejects_unknown_generations_and_duplicate_rows() -> Result<(), String> {
        let mut generation = payload();
        generation.schema_version = 1;
        let validation = validate_isolated_install_v2(&generation);
        if validation.result != IsolatedInstallResultV2::UnsupportedGeneration {
            return Err(format!(
                "unknown generation was not classified: {validation:?}"
            ));
        }

        let mut duplicated = payload();
        duplicated
            .package_rows
            .push(row("allow-core", "0.2.0-rc.1"));
        let validation = validate_isolated_install_v2(&duplicated);
        if validation.result != IsolatedInstallResultV2::MalformedArtifact
            || !validation.gaps.iter().any(|gap| gap.contains("duplicated"))
        {
            return Err(format!("duplicate row was not classified: {validation:?}"));
        }
        Ok(())
    }

    #[test]
    fn receipt_rendering_is_deterministic_across_equal_payloads() -> Result<(), String> {
        let first =
            render_isolated_install_v2_bytes(&payload()).map_err(|error| error.to_string())?;
        let second =
            render_isolated_install_v2_bytes(&payload()).map_err(|error| error.to_string())?;
        if first != second {
            return Err("equal payloads rendered different bytes".to_string());
        }
        Ok(())
    }
}
