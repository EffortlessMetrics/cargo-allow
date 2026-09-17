//! Operator-side custody for one real out-of-tree final-release authorization (#3927).
//!
//! The decision compiler (`release_authorization_v1`) reconciles an immutable
//! maintainer decision against an independently supplied trusted context. This
//! module owns the other half of the protocol: how that decision is minted,
//! stored outside the frozen source tree, read back, selected exactly once,
//! and settled, without ever becoming frozen package input or carrying secret
//! material.
//!
//! Everything here is pure and side-effect-free: no network access, no
//! credential reads, no tag creation, no uploads, no live-state mutation, and
//! no environment reads. Minting the real authorization remains a separate
//! explicit maintainer act performed only after a Complete #2501 freeze; the
//! constructor only models that act against caller-supplied evidence so the
//! workflow gate (#3790) and the operator packet can be tested with synthetic
//! authorizations.

use serde::{Deserialize, Serialize};

use super::release_authorization_v1::{
    RELEASE_AUTHORIZATION_AUTH_CLASS, RELEASE_AUTHORIZATION_FINAL_OPERATION,
    RELEASE_AUTHORIZATION_FINAL_TAG, RELEASE_AUTHORIZATION_FINAL_VERSION,
    RELEASE_AUTHORIZATION_SCHEMA_ID, RELEASE_AUTHORIZATION_SCHEMA_VERSION,
    RELEASE_AUTHORIZATION_STABLE_CHANNEL, ReleaseAuthorizationAuthorityKindV1,
    ReleaseAuthorizationConsumptionV1, ReleaseAuthorizationFreezeV1, ReleaseAuthorizationInputV1,
    ReleaseAuthorizationOperationV1, authorization_statement_digest,
};
use super::release_authorization_v1::{
    ReleaseAuthorizationEvidenceV1, transition_authorization_consumption,
};

pub const AUTHORIZATION_CUSTODY_SCHEMA_ID: &str = "cargo-allow.release-authorization-custody.v1";
pub const AUTHORIZATION_CUSTODY_SCHEMA_VERSION: u32 = 1;
pub const AUTHORIZATION_CONSUMPTION_SCHEMA_ID: &str =
    "cargo-allow.release-authorization-consumption.v1";
pub const AUTHORIZATION_CONSUMPTION_SCHEMA_VERSION: u32 = 1;

/// Replay outcomes that prove the frozen subject reproduces. Anything else
/// cannot be minted against.
pub const AUTHORIZATION_CUSTODY_COMPLETE_REPLAY_RESULTS: [&str; 2] =
    ["Complete", "CompleteEquivalent"];

/// Free-text markers that must never appear in custody records. Custody
/// carries digests, locators, and policy names; secret values never travel in
/// authorization documents.
const SECRET_MARKERS: [&str; 12] = [
    "BEGIN PRIVATE KEY",
    "BEGIN RSA PRIVATE KEY",
    "BEGIN EC PRIVATE KEY",
    "BEGIN OPENSSH PRIVATE KEY",
    "ghp_",
    "github_pat_",
    "AKIA",
    "xoxb-",
    "xoxp-",
    "xoxa-",
    "password=",
    "token=",
];

/// Source-tree prefixes a custody locator must never start with. The Mint
/// check is a fail-closed tripwire; independent storage observation (not
/// string matching) remains the real out-of-tree control.
const IN_TREE_PREFIXES: [&str; 12] = [
    "./",
    "../",
    "policy/",
    "crates/",
    "docs/",
    "target/",
    "fixtures/",
    "tests/",
    "scripts/",
    "examples/",
    "plans/",
    ".allow/",
];

fn digest_shape(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn git_sha_shape(value: &str) -> bool {
    (value.len() == 40 || value.len() == 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn content_digest<T: Serialize + ?Sized>(value: &T) -> Result<String, serde_json::Error> {
    let bytes = serde_json::to_vec(value)?;
    Ok(allow_core::sha256_v1_bytes(&bytes).replacen("sha256:v1:", "sha256:", 1))
}

fn secret_marker(value: &str) -> Option<&'static str> {
    SECRET_MARKERS
        .into_iter()
        .find(|marker| value.contains(marker))
}

fn in_tree_locator(locator: &str) -> bool {
    !locator.contains("://")
        || IN_TREE_PREFIXES
            .into_iter()
            .any(|prefix| locator.starts_with(prefix))
}

/// Out-of-tree storage binding for one minted authorization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorizationCustodyStorageV1 {
    /// Absolute storage locator (URI with explicit scheme). Never an in-tree path.
    pub locator: String,
    /// Bounded access policy name. Names who may read, never carries credentials.
    pub access_policy: String,
    pub retention_expiry_unix_seconds: u64,
}

/// Mint declaration: the exact Complete freeze/replay result the maintainer
/// decision was made against. The constructor requires both halves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorizationCustodyMintV1 {
    pub freeze_receipt_digest: String,
    pub replay_digest: String,
    pub replay_result: String,
    pub candidate_custody_digest: String,
    pub minted_by: String,
    pub minted_at_unix_seconds: u64,
}

/// One append-only custody state transition. The immutable decision is never
/// rewritten; only the custody record advances.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorizationCustodyTransitionV1 {
    pub from: ReleaseAuthorizationConsumptionV1,
    pub to: ReleaseAuthorizationConsumptionV1,
    pub at_unix_seconds: u64,
    pub reason: String,
}

/// Custody record for one minted authorization. The use-state vocabulary is
/// the shared consumption lifecycle so the compiler, the lease (#3925), and
/// the operation record (#3940) observe the same states.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CargoAllowReleaseAuthorizationCustodyV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub authorization_id: String,
    pub authorization_digest: String,
    pub operation: ReleaseAuthorizationOperationV1,
    pub freeze: ReleaseAuthorizationFreezeV1,
    pub evidence_digest: String,
    pub mint: AuthorizationCustodyMintV1,
    pub storage: AuthorizationCustodyStorageV1,
    pub valid_from_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
    pub one_run_scope: bool,
    pub nonce: String,
    pub state: ReleaseAuthorizationConsumptionV1,
    pub consumed_nonces: Vec<String>,
    pub transitions: Vec<AuthorizationCustodyTransitionV1>,
    pub readback_verified: bool,
    pub readback_digest: Option<String>,
    /// Always true: secret values never travel in authorization documents.
    pub redacted: bool,
    pub claim_boundary: String,
    pub limitations: Vec<String>,
}

/// Append-only consumption observation binding one selection to the custody
/// record that authorized it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CargoAllowReleaseAuthorizationConsumptionV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub authorization_id: String,
    pub authorization_digest: String,
    pub nonce: String,
    pub from: ReleaseAuthorizationConsumptionV1,
    pub to: ReleaseAuthorizationConsumptionV1,
    pub observed_at_unix_seconds: u64,
    pub evidence_digest: String,
    pub claim_boundary: String,
}

/// Bounded selection payload the workflow gate (#3790) may consume. It binds
/// identity and digests only; the type cannot carry secret material because no
/// such field exists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorizationSelectionPayloadV1 {
    pub authorization_id: String,
    pub authorization_digest: String,
    pub operation_name: String,
    pub operation_version: String,
    pub operation_tag: String,
    pub operation_channel: String,
    pub commit: String,
    pub tree: String,
    pub denominator_digest: String,
    pub evidence_digest: String,
    pub nonce: String,
    pub state: ReleaseAuthorizationConsumptionV1,
    pub valid_from_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
}

/// Caller-supplied mint inputs. Every digest below is independently retained
/// production evidence; the constructor validates shape and consistency but
/// never fetches, mints authority by itself, or touches live state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizationCustodyMintInitV1 {
    pub authorization_id: String,
    pub decision: ReleaseAuthorizationInputV1,
    pub freeze_receipt_digest: String,
    pub replay_digest: String,
    pub replay_result: String,
    pub candidate_custody_digest: String,
    pub freeze_complete: bool,
    pub replay_complete: bool,
    pub storage_locator: String,
    pub storage_access_policy: String,
    pub storage_retention_expiry_unix_seconds: u64,
    pub storage_provider_available: bool,
    pub valid_from_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
    pub minted_by: String,
    pub minted_at_unix_seconds: u64,
}

const CLAIM_BOUNDARY: &str = "This record models operator-side minting, out-of-tree custody, readback, one-use selection, and consumption of one immutable final-release authorization decision. It does not authenticate the maintainer, create or move a tag, read a credential, upload a package, grant recovery authority, or execute publication.";

fn validate_mint_operation(
    operation: &ReleaseAuthorizationOperationV1,
) -> Result<(), &'static str> {
    if operation.name != RELEASE_AUTHORIZATION_FINAL_OPERATION
        || operation.version != RELEASE_AUTHORIZATION_FINAL_VERSION
        || operation.tag != RELEASE_AUTHORIZATION_FINAL_TAG
        || operation.channel != RELEASE_AUTHORIZATION_STABLE_CHANNEL
        || operation.github_prerelease
        || operation.authority_kind != ReleaseAuthorizationAuthorityKindV1::Clean
    {
        return Err("minting requires the exact clean final 0.2.0 stable operation");
    }
    Ok(())
}

fn validate_mint_freeze(freeze: &ReleaseAuthorizationFreezeV1) -> Result<(), &'static str> {
    for value in [
        freeze.receipt_digest.as_str(),
        freeze.candidate_digest.as_str(),
        freeze.denominator_digest.as_str(),
        freeze.lock_digest.as_str(),
    ] {
        if !digest_shape(value) {
            return Err("minting requires well-formed freeze digests");
        }
    }
    if !git_sha_shape(&freeze.commit) || !git_sha_shape(&freeze.tree) {
        return Err("minting requires canonical freeze commit and tree SHAs");
    }
    if freeze.packages.len() != 10 || freeze.shared_prerequisites.len() != 3 {
        return Err("minting requires exactly ten final and three shared rows");
    }
    for row in &freeze.packages {
        if !digest_shape(&row.package_digest) || row.package_size_bytes == 0 {
            return Err("minting requires well-formed final package digests and sizes");
        }
    }
    for row in &freeze.shared_prerequisites {
        if !digest_shape(&row.expected_checksum) || !digest_shape(&row.authority_digest) {
            return Err("minting requires well-formed shared checksum authority");
        }
    }
    Ok(())
}

/// Model the maintainer mint act against caller-supplied Complete evidence.
/// Refuses before a Complete #2501 freeze/replay, on any scope or shape
/// violation, on in-tree storage, on unavailable storage, or on any secret
/// marker in operator-supplied text.
pub fn mint_authorization_custody_v1(
    init: AuthorizationCustodyMintInitV1,
) -> Result<CargoAllowReleaseAuthorizationCustodyV1, &'static str> {
    if !init.freeze_complete || !init.replay_complete {
        return Err("minting requires a Complete #2501 freeze and replay result");
    }
    if !AUTHORIZATION_CUSTODY_COMPLETE_REPLAY_RESULTS.contains(&init.replay_result.as_str()) {
        return Err("minting requires a Complete or CompleteEquivalent replay result");
    }
    for value in [
        init.freeze_receipt_digest.as_str(),
        init.replay_digest.as_str(),
        init.candidate_custody_digest.as_str(),
    ] {
        if !digest_shape(value) {
            return Err("minting requires well-formed freeze, replay, and custody digests");
        }
    }
    validate_mint_operation(&init.decision.operation)?;
    validate_mint_freeze(&init.decision.freeze)?;
    if init.decision.schema_id != RELEASE_AUTHORIZATION_SCHEMA_ID
        || init.decision.schema_version != RELEASE_AUTHORIZATION_SCHEMA_VERSION
    {
        return Err("minting requires the current authorization decision generation");
    }
    if !init.decision.authority.one_run_scope || init.decision.authority.nonce.trim().is_empty() {
        return Err("minting requires one-run scope and a non-empty nonce");
    }
    if init.decision.authority.selected_auth_class != RELEASE_AUTHORIZATION_AUTH_CLASS {
        return Err("minting requires the token-backed authentication class");
    }
    if in_tree_locator(&init.storage_locator) {
        return Err("authorization storage must live outside the frozen source tree");
    }
    if !init.storage_provider_available {
        return Err("authorization storage provider must be available at mint time");
    }
    if init.storage_retention_expiry_unix_seconds <= init.minted_at_unix_seconds {
        return Err("storage retention must outlive the mint act");
    }
    if init.expires_at_unix_seconds <= init.valid_from_unix_seconds {
        return Err("authorization expiry must follow its valid-from bound");
    }
    if init.authorization_id.trim().is_empty() || init.minted_by.trim().is_empty() {
        return Err("minting requires an authorization identity and maintainer reference");
    }
    for value in [
        init.authorization_id.as_str(),
        init.storage_locator.as_str(),
        init.storage_access_policy.as_str(),
        init.minted_by.as_str(),
    ] {
        if let Some(marker) = secret_marker(value) {
            let _ = marker;
            return Err("secret material must never enter authorization custody records");
        }
    }
    let authorization_digest = authorization_statement_digest(&init.decision)
        .map_err(|_| "authorization digest serialization failed")?;
    let evidence_digest =
        content_digest(&init.decision.evidence).map_err(|_| "evidence digest failed")?;
    Ok(CargoAllowReleaseAuthorizationCustodyV1 {
        schema_id: AUTHORIZATION_CUSTODY_SCHEMA_ID.to_string(),
        schema_version: AUTHORIZATION_CUSTODY_SCHEMA_VERSION,
        authorization_id: init.authorization_id,
        authorization_digest,
        operation: init.decision.operation,
        freeze: init.decision.freeze,
        evidence_digest,
        mint: AuthorizationCustodyMintV1 {
            freeze_receipt_digest: init.freeze_receipt_digest,
            replay_digest: init.replay_digest,
            replay_result: init.replay_result,
            candidate_custody_digest: init.candidate_custody_digest,
            minted_by: init.minted_by,
            minted_at_unix_seconds: init.minted_at_unix_seconds,
        },
        storage: AuthorizationCustodyStorageV1 {
            locator: init.storage_locator,
            access_policy: init.storage_access_policy,
            retention_expiry_unix_seconds: init.storage_retention_expiry_unix_seconds,
        },
        valid_from_unix_seconds: init.valid_from_unix_seconds,
        expires_at_unix_seconds: init.expires_at_unix_seconds,
        one_run_scope: true,
        nonce: init.decision.authority.nonce,
        state: ReleaseAuthorizationConsumptionV1::Available,
        consumed_nonces: Vec::new(),
        transitions: Vec::new(),
        readback_verified: false,
        readback_digest: None,
        redacted: true,
        claim_boundary: CLAIM_BOUNDARY.to_string(),
        limitations: vec![
            "does_not_authenticate_maintainer".to_string(),
            "does_not_create_or_move_tags".to_string(),
            "does_not_read_credentials".to_string(),
            "does_not_authorize_recovery".to_string(),
        ],
    })
}

/// Custody readback verdict for independently stored bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustodyReadbackV1 {
    Match,
    Mismatch,
    Malformed,
}

/// Verify independently read-back storage bytes against the minted record.
/// The readback must be the exact canonical record bytes; any drift is a
/// mismatch, and non-JSON bytes are malformed.
pub fn verify_custody_readback_v1(
    record: &CargoAllowReleaseAuthorizationCustodyV1,
    readback_json: &[u8],
) -> CustodyReadbackV1 {
    let stored = match serde_json::to_vec(record) {
        Ok(bytes) => bytes,
        Err(_) => return CustodyReadbackV1::Malformed,
    };
    let parsed: serde_json::Value = match serde_json::from_slice(readback_json) {
        Ok(value) => value,
        Err(_) => return CustodyReadbackV1::Malformed,
    };
    let stored_value: serde_json::Value = match serde_json::from_slice(&stored) {
        Ok(value) => value,
        Err(_) => return CustodyReadbackV1::Malformed,
    };
    if parsed == stored_value {
        CustodyReadbackV1::Match
    } else {
        CustodyReadbackV1::Mismatch
    }
}

/// Record a successful independent readback on the custody record.
pub fn note_custody_readback_v1(
    record: &mut CargoAllowReleaseAuthorizationCustodyV1,
    readback_json: &[u8],
) -> CustodyReadbackV1 {
    match verify_custody_readback_v1(record, readback_json) {
        CustodyReadbackV1::Match => {
            let digest = content_digest(readback_json).unwrap_or_default();
            record.readback_verified = true;
            record.readback_digest = Some(digest);
            CustodyReadbackV1::Match
        }
        verdict => verdict,
    }
}

fn advance_custody_state(
    record: &mut CargoAllowReleaseAuthorizationCustodyV1,
    next: ReleaseAuthorizationConsumptionV1,
    at_unix_seconds: u64,
    reason: &str,
) -> Result<(), &'static str> {
    let checked = transition_authorization_consumption(record.state, next)?;
    record.transitions.push(AuthorizationCustodyTransitionV1 {
        from: record.state,
        to: checked,
        at_unix_seconds,
        reason: reason.to_string(),
    });
    record.state = checked;
    Ok(())
}

/// Select the authorization for exactly one run. Requires a verified
/// readback, a live validity window, the exact bound nonce, a fresh evidence
/// binding from the #3790 assembler, and an available storage provider. The
/// second selection of the same authorization is refused.
pub fn select_authorization_for_run_v1(
    record: &mut CargoAllowReleaseAuthorizationCustodyV1,
    nonce: &str,
    now_unix_seconds: u64,
    current_evidence_digest: &str,
    storage_provider_available: bool,
) -> Result<CargoAllowReleaseAuthorizationConsumptionV1, &'static str> {
    use ReleaseAuthorizationConsumptionV1 as Consumption;
    if record.state == Consumption::Expired
        || record.state == Consumption::Revoked
        || now_unix_seconds > record.expires_at_unix_seconds
    {
        if record.state != Consumption::Expired {
            let _ =
                advance_custody_state(record, Consumption::Expired, now_unix_seconds, "expired");
        }
        return Err("expired or revoked authorization cannot be selected");
    }
    if record.state != Consumption::Available {
        return Err("authorization was already selected or consumed");
    }
    if !record.readback_verified {
        return Err("selection requires a verified independent readback");
    }
    if !storage_provider_available {
        return Err("selection requires an available storage provider");
    }
    if nonce.trim().is_empty() || nonce != record.nonce {
        return Err("selection requires the exact bound one-use nonce");
    }
    if record.consumed_nonces.iter().any(|seen| seen == nonce) {
        return Err("authorization nonce was already consumed");
    }
    if !digest_shape(current_evidence_digest)
        || !current_evidence_digest.eq_ignore_ascii_case(&record.evidence_digest)
    {
        return Err("changed evidence invalidates selection before token access");
    }
    advance_custody_state(
        record,
        Consumption::SelectedForRun,
        now_unix_seconds,
        "selected",
    )?;
    record.consumed_nonces.push(nonce.to_string());
    Ok(CargoAllowReleaseAuthorizationConsumptionV1 {
        schema_id: AUTHORIZATION_CONSUMPTION_SCHEMA_ID.to_string(),
        schema_version: AUTHORIZATION_CONSUMPTION_SCHEMA_VERSION,
        authorization_id: record.authorization_id.clone(),
        authorization_digest: record.authorization_digest.clone(),
        nonce: nonce.to_string(),
        from: Consumption::Available,
        to: Consumption::SelectedForRun,
        observed_at_unix_seconds: now_unix_seconds,
        evidence_digest: record.evidence_digest.clone(),
        claim_boundary: CLAIM_BOUNDARY.to_string(),
    })
}

/// Note the first irreversible action on a selected authorization. The
/// Started state itself is owned by the operation record (#3940); custody
/// mirrors it so later selection attempts observe consumption.
pub fn note_irreversible_start_v1(
    record: &mut CargoAllowReleaseAuthorizationCustodyV1,
    now_unix_seconds: u64,
) -> Result<(), &'static str> {
    use ReleaseAuthorizationConsumptionV1 as Consumption;
    if record.state != Consumption::SelectedForRun {
        return Err("only a selected authorization can start the irreversible operation");
    }
    advance_custody_state(
        record,
        Consumption::IrreversibleOperationStarted,
        now_unix_seconds,
        "irreversible-operation-started",
    )
}

/// Settle a started authorization as complete or incident. A clean
/// authorization can never be selected again after an incident.
pub fn settle_authorization_consumption_v1(
    record: &mut CargoAllowReleaseAuthorizationCustodyV1,
    complete: bool,
    now_unix_seconds: u64,
) -> Result<CargoAllowReleaseAuthorizationConsumptionV1, &'static str> {
    use ReleaseAuthorizationConsumptionV1 as Consumption;
    let next = if complete {
        Consumption::ConsumedComplete
    } else {
        Consumption::ConsumedIncident
    };
    let from = record.state;
    advance_custody_state(
        record,
        next,
        now_unix_seconds,
        if complete {
            "consumed-complete"
        } else {
            "consumed-incident"
        },
    )?;
    Ok(CargoAllowReleaseAuthorizationConsumptionV1 {
        schema_id: AUTHORIZATION_CONSUMPTION_SCHEMA_ID.to_string(),
        schema_version: AUTHORIZATION_CONSUMPTION_SCHEMA_VERSION,
        authorization_id: record.authorization_id.clone(),
        authorization_digest: record.authorization_digest.clone(),
        nonce: record.nonce.clone(),
        from,
        to: next,
        observed_at_unix_seconds: now_unix_seconds,
        evidence_digest: record.evidence_digest.clone(),
        claim_boundary: CLAIM_BOUNDARY.to_string(),
    })
}

/// Revoke a minted authorization. Terminal consumption history
/// (complete/incident) is immutable and cannot be revoked after the fact.
pub fn revoke_authorization_custody_v1(
    record: &mut CargoAllowReleaseAuthorizationCustodyV1,
    reason: &str,
    now_unix_seconds: u64,
) -> Result<(), &'static str> {
    use ReleaseAuthorizationConsumptionV1 as Consumption;
    match record.state {
        Consumption::ConsumedComplete | Consumption::ConsumedIncident => {
            return Err("terminal consumption history cannot be revoked");
        }
        _ => {}
    }
    if reason.trim().is_empty() {
        return Err("revocation requires a reason");
    }
    advance_custody_state(record, Consumption::Revoked, now_unix_seconds, reason)
}

/// Bounded selection payload for the #3790 workflow gate. Identity and
/// digests only; the shape cannot carry secret material.
pub fn selection_payload_v1(
    record: &CargoAllowReleaseAuthorizationCustodyV1,
) -> AuthorizationSelectionPayloadV1 {
    AuthorizationSelectionPayloadV1 {
        authorization_id: record.authorization_id.clone(),
        authorization_digest: record.authorization_digest.clone(),
        operation_name: record.operation.name.clone(),
        operation_version: record.operation.version.clone(),
        operation_tag: record.operation.tag.clone(),
        operation_channel: record.operation.channel.clone(),
        commit: record.freeze.commit.clone(),
        tree: record.freeze.tree.clone(),
        denominator_digest: record.freeze.denominator_digest.clone(),
        evidence_digest: record.evidence_digest.clone(),
        nonce: record.nonce.clone(),
        state: record.state,
        valid_from_unix_seconds: record.valid_from_unix_seconds,
        expires_at_unix_seconds: record.expires_at_unix_seconds,
    }
}

/// Canonical JSON renderer for custody records.
pub fn render_release_authorization_custody_v1(
    record: &CargoAllowReleaseAuthorizationCustodyV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(record)
}

/// Canonical JSON renderer for consumption observations.
pub fn render_release_authorization_consumption_v1(
    observation: &CargoAllowReleaseAuthorizationConsumptionV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(observation)
}

/// Evidence snapshot digest helper for the #3790 assembler: binds the exact
/// evidence object a selection must later reproduce.
pub fn authorization_evidence_digest_v1(
    evidence: &ReleaseAuthorizationEvidenceV1,
) -> Result<String, serde_json::Error> {
    content_digest(evidence)
}
