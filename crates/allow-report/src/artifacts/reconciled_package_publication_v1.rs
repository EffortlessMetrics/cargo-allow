//! Typed reconciled package publication for ReleaseManifestV2 construction
//! (#3761).
//!
//! Each manifest package row must be built from a reconciled publication
//! result — never from a state string alone. The row class separates
//! cargo-allow candidate rows (whose expected checksum is the candidate's
//! local package bytes) from published shared prerequisites (whose expected
//! checksum is the retained namespace authority; a later local repackage is
//! diagnostic only). Classification is derived from expected/observed
//! equality and the producing evidence, so a same-version registry row from
//! another candidate can never pass as exact.

use serde::{Deserialize, Serialize};

pub const RECONCILED_PACKAGE_PUBLICATION_SCHEMA: &str =
    "cargo-allow.reconciled-package-publication.v1";

/// The role a package plays in the selected release closure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageRowClassV1 {
    /// Final cargo-allow candidate row: the expected checksum authority is
    /// this candidate's local package bytes.
    CargoAllowCandidate,
    /// Published shared prerequisite: the expected checksum authority is the
    /// retained namespace registry checksum (#3744), not a local repackage.
    PublishedSharedPrerequisite,
}

/// Registry/publication state as observed at reconciliation time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicationStateV1 {
    /// The row is absent from the registry.
    Missing,
    /// The registry row exists and was verified against this candidate.
    PublishedVerified,
    /// The registry row pre-existed and was verified existing.
    VerifiedExisting,
    /// Provider could not be reached; observation is incomplete.
    ProviderUnavailable,
}

/// The reconciled classification of one package row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicationClassificationV1 {
    CompleteExact,
    Missing,
    Conflict,
    Unavailable,
    Stale,
    Mismatch,
}

/// One reconciled package publication for manifest construction.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReconciledPackagePublicationV1 {
    pub logical_id: String,
    pub package_name: String,
    pub package_version: String,
    pub release_order: u32,
    pub row_class: PackageRowClassV1,
    pub state: PublicationStateV1,
    /// The expected checksum for this row per its class authority.
    pub expected_checksum: String,
    /// The observed registry checksum, when the registry was reachable.
    pub observed_registry_checksum: Option<String>,
}

impl ReconciledPackagePublicationV1 {
    /// Canonical sha256 digest form.
    fn checksum_valid(value: &str) -> bool {
        let digest = value.strip_prefix("sha256:").unwrap_or(value);
        value.starts_with("sha256:")
            && digest.len() == 64
            && digest.chars().all(|c| c.is_ascii_hexdigit())
    }

    /// Classify this row and validate the evidence that classification
    /// requires. A row is `CompleteExact` only when the observed registry
    /// checksum equals the class-correct expected checksum; every other
    /// outcome is named, and none of them may enter a manifest as exact.
    pub fn classify(&self) -> PublicationClassificationV1 {
        // A present-but-malformed observed checksum is malformed evidence
        // (Mismatch), distinct from an absent observation (Missing or
        // Unavailable per the provider state).
        let observed = match &self.observed_registry_checksum {
            Some(value) => {
                if !Self::checksum_valid(value) {
                    return PublicationClassificationV1::Mismatch;
                }
                value.as_str()
            }
            None => {
                return if self.state == PublicationStateV1::ProviderUnavailable {
                    PublicationClassificationV1::Unavailable
                } else {
                    PublicationClassificationV1::Missing
                };
            }
        };
        if !Self::checksum_valid(&self.expected_checksum) {
            return PublicationClassificationV1::Mismatch;
        }
        match self.row_class {
            // A candidate row whose registry bytes predate this candidate is
            // stale: the version matches but the candidate moved.
            PackageRowClassV1::CargoAllowCandidate => {
                if self.state == PublicationStateV1::VerifiedExisting
                    && observed != self.expected_checksum.as_str()
                {
                    PublicationClassificationV1::Stale
                } else if observed == self.expected_checksum.as_str() {
                    PublicationClassificationV1::CompleteExact
                } else {
                    PublicationClassificationV1::Conflict
                }
            }
            PackageRowClassV1::PublishedSharedPrerequisite => {
                if observed == self.expected_checksum.as_str() {
                    PublicationClassificationV1::CompleteExact
                } else {
                    PublicationClassificationV1::Conflict
                }
            }
        }
    }

    /// A row may enter the manifest only as exact.
    pub fn is_manifest_ready(&self) -> bool {
        self.classify() == PublicationClassificationV1::CompleteExact
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CANONICAL: &str =
        "sha256:a3f6c99da45120b96968349bc51c83ddf81e6d7a2c49e06e8727d3ab4195a3b2";
    const CONFLICTING: &str =
        "sha256:ae3f4ed17b8877640c00e8fabd1e4e1778eff52dc102cc5103ee49ca92c5a488";

    fn candidate(
        version: &str,
        state: PublicationStateV1,
        expected: &str,
        observed: Option<&str>,
    ) -> ReconciledPackagePublicationV1 {
        ReconciledPackagePublicationV1 {
            logical_id: "cargo-allow".to_string(),
            package_name: "cargo-allow".to_string(),
            package_version: version.to_string(),
            release_order: 100,
            row_class: PackageRowClassV1::CargoAllowCandidate,
            state,
            expected_checksum: expected.to_string(),
            observed_registry_checksum: observed.map(str::to_string),
        }
    }

    fn shared(observed: Option<&str>) -> ReconciledPackagePublicationV1 {
        ReconciledPackagePublicationV1 {
            logical_id: "effortless-repo-edit".to_string(),
            package_name: "effortless-repo-edit".to_string(),
            package_version: "0.1.0".to_string(),
            release_order: 120,
            row_class: PackageRowClassV1::PublishedSharedPrerequisite,
            state: PublicationStateV1::VerifiedExisting,
            expected_checksum: CANONICAL.to_string(),
            observed_registry_checksum: observed.map(str::to_string),
        }
    }

    #[test]
    fn exact_rows_classify_complete() {
        let row = candidate(
            "0.2.0-rc.1",
            PublicationStateV1::PublishedVerified,
            CANONICAL,
            Some(CANONICAL),
        );
        assert_eq!(row.classify(), PublicationClassificationV1::CompleteExact);
        assert!(row.is_manifest_ready());
        let shared_row = shared(Some(CANONICAL));
        assert_eq!(
            shared_row.classify(),
            PublicationClassificationV1::CompleteExact
        );
    }

    #[test]
    fn candidate_drift_is_stale_not_conflict() {
        // Same version, different candidate bytes: the registry row is stale
        // against this candidate.
        let row = candidate(
            "0.2.0-rc.1",
            PublicationStateV1::VerifiedExisting,
            CANONICAL,
            Some(CONFLICTING),
        );
        assert_eq!(row.classify(), PublicationClassificationV1::Stale);
        assert!(!row.is_manifest_ready());
    }

    #[test]
    fn shared_drift_is_conflict() {
        let row = shared(Some(CONFLICTING));
        assert_eq!(row.classify(), PublicationClassificationV1::Conflict);
        assert!(!row.is_manifest_ready());
    }

    #[test]
    fn missing_and_unavailable_are_named() {
        let missing = candidate("0.2.0-rc.1", PublicationStateV1::Missing, CANONICAL, None);
        assert_eq!(missing.classify(), PublicationClassificationV1::Missing);
        let unavailable = candidate(
            "0.2.0-rc.1",
            PublicationStateV1::ProviderUnavailable,
            CANONICAL,
            None,
        );
        assert_eq!(
            unavailable.classify(),
            PublicationClassificationV1::Unavailable
        );
    }

    #[test]
    fn malformed_checksums_are_mismatch() {
        let row = candidate(
            "0.2.0-rc.1",
            PublicationStateV1::PublishedVerified,
            "short",
            Some(CANONICAL),
        );
        assert_eq!(row.classify(), PublicationClassificationV1::Mismatch);
        let bad_observed = candidate(
            "0.2.0-rc.1",
            PublicationStateV1::PublishedVerified,
            CANONICAL,
            Some("sha256:nope"),
        );
        assert_eq!(
            bad_observed.classify(),
            PublicationClassificationV1::Mismatch
        );
    }
}
