use std::error::Error;
use std::io;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RegistryDisposition {
    Missing,
    PropagationPartial,
    PublishedExact,
    ChecksumConflict,
    Yanked,
    DocsPending,
    DocsFailed,
    ProviderUnavailable,
    InstrumentFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DocsBuildStatus {
    Queued,
    Building,
    Succeeded,
    Failed,
    Unavailable,
    NotQueried,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CargoAllowFinalRegistryRowObservationV1 {
    pub package_id: String,
    pub expected_version: String,
    pub expected_checksum: String,
    pub api_visible: bool,
    pub index_checksum: Option<String>,
    pub download_checksum: Option<String>,
    pub resolver_exact_version: Option<String>,
    pub docs_status: DocsBuildStatus,
    pub yanked: bool,
    pub disposition: RegistryDisposition,
}

impl CargoAllowFinalRegistryRowObservationV1 {
    pub fn reconcile(&mut self) {
        if self.yanked {
            self.disposition = RegistryDisposition::Yanked;
            return;
        }

        let index_matches = self
            .index_checksum
            .as_deref()
            .map(|c| c == self.expected_checksum)
            .unwrap_or(false);

        let download_matches = self
            .download_checksum
            .as_deref()
            .map(|c| c == self.expected_checksum)
            .unwrap_or(false);

        let resolver_matches = self
            .resolver_exact_version
            .as_deref()
            .map(|v| v == self.expected_version)
            .unwrap_or(false);

        if self.api_visible && index_matches && download_matches && resolver_matches {
            self.disposition = RegistryDisposition::PublishedExact;
        } else if let Some(ref c) = self.download_checksum {
            if c != &self.expected_checksum {
                self.disposition = RegistryDisposition::ChecksumConflict;
            } else {
                self.disposition = RegistryDisposition::PropagationPartial;
            }
        } else if self.api_visible {
            self.disposition = RegistryDisposition::PropagationPartial;
        } else {
            self.disposition = RegistryDisposition::Missing;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CargoAllowFinalRegistryObservationV1 {
    pub schema_version: String,
    pub observation_id: String,
    pub rows: Vec<CargoAllowFinalRegistryRowObservationV1>,
}

fn require(cond: bool, msg: &str) -> Result<(), io::Error> {
    if !cond {
        Err(io::Error::other(msg))
    } else {
        Ok(())
    }
}

#[test]
fn test_reconciliation_published_exact() -> Result<(), Box<dyn Error>> {
    let mut row = CargoAllowFinalRegistryRowObservationV1 {
        package_id: "cargo-allow".to_string(),
        expected_version: "0.2.0".to_string(),
        expected_checksum: "sha256:abcd1234abcd1234".to_string(),
        api_visible: true,
        index_checksum: Some("sha256:abcd1234abcd1234".to_string()),
        download_checksum: Some("sha256:abcd1234abcd1234".to_string()),
        resolver_exact_version: Some("0.2.0".to_string()),
        docs_status: DocsBuildStatus::Succeeded,
        yanked: false,
        disposition: RegistryDisposition::Missing,
    };

    row.reconcile();
    require(
        row.disposition == RegistryDisposition::PublishedExact,
        "all matching surfaces must yield PublishedExact",
    )?;

    Ok(())
}

#[test]
fn test_reconciliation_negative_controls() -> Result<(), Box<dyn Error>> {
    // 1. API visible but index missing is PropagationPartial
    let mut r1 = CargoAllowFinalRegistryRowObservationV1 {
        package_id: "cargo-allow".to_string(),
        expected_version: "0.2.0".to_string(),
        expected_checksum: "sha256:abcd".to_string(),
        api_visible: true,
        index_checksum: None,
        download_checksum: None,
        resolver_exact_version: None,
        docs_status: DocsBuildStatus::Queued,
        yanked: false,
        disposition: RegistryDisposition::Missing,
    };
    r1.reconcile();
    require(
        r1.disposition == RegistryDisposition::PropagationPartial,
        "api-only visibility must be PropagationPartial",
    )?;

    // 2. Checksum mismatch is ChecksumConflict
    let mut r2 = CargoAllowFinalRegistryRowObservationV1 {
        package_id: "cargo-allow".to_string(),
        expected_version: "0.2.0".to_string(),
        expected_checksum: "sha256:abcd".to_string(),
        api_visible: true,
        index_checksum: Some("sha256:wrong".to_string()),
        download_checksum: Some("sha256:wrong".to_string()),
        resolver_exact_version: Some("0.2.0".to_string()),
        docs_status: DocsBuildStatus::Queued,
        yanked: false,
        disposition: RegistryDisposition::Missing,
    };
    r2.reconcile();
    require(
        r2.disposition == RegistryDisposition::ChecksumConflict,
        "wrong checksum must yield ChecksumConflict",
    )?;

    // 3. Yanked package is Yanked
    let mut r3 = CargoAllowFinalRegistryRowObservationV1 {
        package_id: "cargo-allow".to_string(),
        expected_version: "0.2.0".to_string(),
        expected_checksum: "sha256:abcd".to_string(),
        api_visible: true,
        index_checksum: Some("sha256:abcd".to_string()),
        download_checksum: Some("sha256:abcd".to_string()),
        resolver_exact_version: Some("0.2.0".to_string()),
        docs_status: DocsBuildStatus::Succeeded,
        yanked: true,
        disposition: RegistryDisposition::Missing,
    };
    r3.reconcile();
    require(
        r3.disposition == RegistryDisposition::Yanked,
        "yanked package must yield Yanked disposition",
    )?;

    Ok(())
}
