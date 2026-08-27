use std::collections::BTreeMap;
use std::error::Error;
use std::io;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CloseoutResult {
    Complete,
    Incomplete,
    ReleaseIncident,
    InstrumentFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReleaseCloseoutReceiptV1 {
    pub schema_version: String,
    pub repository: String,
    pub tag: String,
    pub release_manifest_digest: String,
    pub expected_assets: BTreeMap<String, String>, // name -> sha256
    pub actual_assets: BTreeMap<String, String>,   // name -> sha256
    pub result: CloseoutResult,
}

impl ReleaseCloseoutReceiptV1 {
    pub fn reconcile(&mut self) {
        if self.expected_assets == self.actual_assets && !self.release_manifest_digest.is_empty() {
            self.result = CloseoutResult::Complete;
        } else {
            self.result = CloseoutResult::Incomplete;
        }
    }
}

fn require(cond: bool, msg: &str) -> Result<(), io::Error> {
    if !cond {
        Err(io::Error::other(msg))
    } else {
        Ok(())
    }
}

#[test]
fn test_closeout_receipt_complete() -> Result<(), Box<dyn Error>> {
    let mut assets = BTreeMap::new();
    assets.insert("manifest.json".to_string(), "sha256:1111".to_string());
    assets.insert(
        "cargo-allow-x86_64-linux.tar.gz".to_string(),
        "sha256:2222".to_string(),
    );

    let mut receipt = ReleaseCloseoutReceiptV1 {
        schema_version: "1.0".to_string(),
        repository: "EffortlessMetrics/cargo-allow".to_string(),
        tag: "v0.2.0".to_string(),
        release_manifest_digest: "sha256:manifest123".to_string(),
        expected_assets: assets.clone(),
        actual_assets: assets,
        result: CloseoutResult::Incomplete,
    };

    receipt.reconcile();
    require(
        receipt.result == CloseoutResult::Complete,
        "matching expected and actual assets must yield Complete closeout",
    )?;

    Ok(())
}

#[test]
fn test_closeout_receipt_negative_controls() -> Result<(), Box<dyn Error>> {
    let mut expected = BTreeMap::new();
    expected.insert("manifest.json".to_string(), "sha256:1111".to_string());
    expected.insert(
        "cargo-allow-x86_64-linux.tar.gz".to_string(),
        "sha256:2222".to_string(),
    );

    // Missing one asset
    let mut actual_partial = BTreeMap::new();
    actual_partial.insert("manifest.json".to_string(), "sha256:1111".to_string());

    let mut partial_receipt = ReleaseCloseoutReceiptV1 {
        schema_version: "1.0".to_string(),
        repository: "EffortlessMetrics/cargo-allow".to_string(),
        tag: "v0.2.0".to_string(),
        release_manifest_digest: "sha256:manifest123".to_string(),
        expected_assets: expected.clone(),
        actual_assets: actual_partial,
        result: CloseoutResult::Complete,
    };
    partial_receipt.reconcile();
    require(
        partial_receipt.result == CloseoutResult::Incomplete,
        "missing asset must yield Incomplete",
    )?;

    // Asset hash mismatch
    let mut actual_mismatch = expected.clone();
    actual_mismatch.insert(
        "cargo-allow-x86_64-linux.tar.gz".to_string(),
        "sha256:wrong".to_string(),
    );

    let mut mismatch_receipt = ReleaseCloseoutReceiptV1 {
        schema_version: "1.0".to_string(),
        repository: "EffortlessMetrics/cargo-allow".to_string(),
        tag: "v0.2.0".to_string(),
        release_manifest_digest: "sha256:manifest123".to_string(),
        expected_assets: expected,
        actual_assets: actual_mismatch,
        result: CloseoutResult::Complete,
    };
    mismatch_receipt.reconcile();
    require(
        mismatch_receipt.result == CloseoutResult::Incomplete,
        "hash mismatch must yield Incomplete",
    )?;

    Ok(())
}
