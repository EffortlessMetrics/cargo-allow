//! Operator-side authorization custody protocol (#3927).
//!
//! Synthetic authorizations only: no registry token is read, no tag is
//! created, and nothing leaves the process. These tests prove the mint gate,
//! out-of-tree readback, one-use selection, and append-only consumption
//! behavior the #3790 workflow gate and the #2502 execution lane will rely on.

use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use ReleaseAuthorizationConsumptionV1 as Consumption;
use allow_report::{
    AUTHORIZATION_CONSUMPTION_SCHEMA_ID, AUTHORIZATION_CONSUMPTION_SCHEMA_VERSION,
    AUTHORIZATION_CUSTODY_SCHEMA_ID, AUTHORIZATION_CUSTODY_SCHEMA_VERSION,
    AuthorizationCustodyMintInitV1, CargoAllowReleaseAuthorizationCustodyV1,
    CargoAllowReleaseOperationAssetRowV1, CargoAllowReleaseOperationAuthorityKindV1,
    CargoAllowReleaseOperationClassV1, CargoAllowReleaseOperationIdentityInitV1,
    CargoAllowReleaseOperationPackageRowV1, CustodyReadbackV1, RELEASE_AUTHORIZATION_AUTH_CLASS,
    RELEASE_AUTHORIZATION_FINAL_OPERATION, RELEASE_AUTHORIZATION_FINAL_TAG,
    RELEASE_AUTHORIZATION_FINAL_VERSION, RELEASE_AUTHORIZATION_RECOVERY_OPERATION,
    RELEASE_AUTHORIZATION_SCHEMA_ID, RELEASE_AUTHORIZATION_SCHEMA_VERSION,
    RELEASE_AUTHORIZATION_SELECTION, RELEASE_AUTHORIZATION_STABLE_CHANNEL,
    RELEASE_OPERATION_ASSET_SELECTION, ReleaseAuthorizationAuthorityKindV1,
    ReleaseAuthorizationAuthorityV1, ReleaseAuthorizationConsumptionV1,
    ReleaseAuthorizationEvidenceV1, ReleaseAuthorizationFreezeV1, ReleaseAuthorizationInputV1,
    ReleaseAuthorizationOperationV1, ReleaseAuthorizationPackageRowV1,
    ReleaseAuthorizationSharedRowV1, ReleaseAuthorizationSourceKindV1,
    ReleaseAuthorizationSourceV1, authorization_evidence_digest_v1,
    build_release_operation_identity_v1, mint_authorization_custody_v1, note_custody_readback_v1,
    note_irreversible_start_v1, release_operation_identity_digest_v1,
    render_release_authorization_consumption_v1, render_release_authorization_custody_v1,
    revoke_authorization_custody_v1, select_authorization_for_operation_v1,
    select_authorization_for_run_v1, selection_payload_v1, settle_authorization_consumption_v1,
    verify_custody_readback_v1,
};

const REPOSITORY: &str = "EffortlessMetrics/cargo-allow";
const EXACT_STATEMENT: &str = "Authorize publish_cargo_allow_final_0_2_0 for v0.2.0.";
const NONCE: &str = "nonce-0-2-0-0001";
const VALID_FROM: u64 = 1_786_000_000;
const EXPIRES_AT: u64 = 1_786_003_600;
const MINTED_AT: u64 = 1_786_000_100;
const SELECT_AT: u64 = 1_786_000_200;

fn digest(n: u64) -> String {
    format!("sha256:{n:064x}")
}

fn require(ok: bool, message: impl Into<String>) -> Result<(), Box<dyn Error>> {
    if ok {
        Ok(())
    } else {
        Err(io::Error::other(message.into()).into())
    }
}

fn decision() -> Result<ReleaseAuthorizationInputV1, Box<dyn Error>> {
    let mut packages = Vec::new();
    let mut shared = Vec::new();
    for (index, (logical, package, version, is_shared)) in
        RELEASE_AUTHORIZATION_SELECTION.into_iter().enumerate()
    {
        if is_shared {
            shared.push(ReleaseAuthorizationSharedRowV1 {
                logical_id: logical.to_string(),
                package_name: package.to_string(),
                package_version: version.to_string(),
                expected_checksum: digest(20 + index as u64),
                authority_digest: digest(40 + index as u64),
            });
        } else {
            packages.push(ReleaseAuthorizationPackageRowV1 {
                logical_id: logical.to_string(),
                package_name: package.to_string(),
                package_version: version.to_string(),
                package_digest: digest(10 + index as u64),
                package_size_bytes: 10_000 + index as u64,
            });
        }
    }
    let mut document = ReleaseAuthorizationInputV1 {
        schema_id: RELEASE_AUTHORIZATION_SCHEMA_ID.to_string(),
        schema_version: RELEASE_AUTHORIZATION_SCHEMA_VERSION,
        operation: ReleaseAuthorizationOperationV1 {
            name: RELEASE_AUTHORIZATION_FINAL_OPERATION.to_string(),
            version: RELEASE_AUTHORIZATION_FINAL_VERSION.to_string(),
            tag: RELEASE_AUTHORIZATION_FINAL_TAG.to_string(),
            channel: RELEASE_AUTHORIZATION_STABLE_CHANNEL.to_string(),
            github_prerelease: false,
            authority_kind: ReleaseAuthorizationAuthorityKindV1::Clean,
        },
        freeze: ReleaseAuthorizationFreezeV1 {
            receipt_digest: digest(1),
            candidate_digest: digest(2),
            denominator_digest: String::new(),
            commit: "a".repeat(40),
            tree: "b".repeat(40),
            lock_digest: digest(4),
            topology_id: "CARGO-ALLOW-PKG-TOPOLOGY-V2-0001".to_string(),
            packages,
            shared_prerequisites: shared,
        },
        evidence: ReleaseAuthorizationEvidenceV1 {
            package_docs_digest: digest(30),
            preflight_result: allow_report::FinalRegistryPreflightResultV1::Complete,
            preflight_evaluated_at_unix_seconds: 100,
            preflight_maximum_age_seconds: 30,
            support_digest: digest(31),
            manifest_digest: digest(32),
            rehearsal_complete_except_authorization: true,
            rehearsal_digest: digest(33),
            source_controls_digest: digest(34),
            live_controls_digest: digest(35),
            workflow_digest: digest(36),
            action_inventory_digest: digest(37),
            observed_context_digest: digest(38),
            current_context_digest: digest(38),
        },
        authority: ReleaseAuthorizationAuthorityV1 {
            selected_auth_class: RELEASE_AUTHORIZATION_AUTH_CLASS.to_string(),
            maintainer_actor: "release-operator".to_string(),
            maintainer_role: "release-maintainer".to_string(),
            source: ReleaseAuthorizationSourceV1 {
                kind: ReleaseAuthorizationSourceKindV1::IssueComment,
                repository: REPOSITORY.to_string(),
                reference: "issue:3760#comment:1".to_string(),
                author: "release-operator".to_string(),
                body_digest: digest(60),
                statement: EXACT_STATEMENT.to_string(),
            },
            created_at_unix_seconds: VALID_FROM - 100,
            expires_at_unix_seconds: EXPIRES_AT,
            one_run_scope: true,
            nonce: NONCE.to_string(),
        },
    };
    document.freeze.denominator_digest =
        allow_report::release_authorization_denominator_binding_v1(&document.freeze)?;
    Ok(document)
}

fn mint_init(document: ReleaseAuthorizationInputV1) -> AuthorizationCustodyMintInitV1 {
    let freeze_receipt_digest = document.freeze.receipt_digest.clone();
    AuthorizationCustodyMintInitV1 {
        authorization_id: "auth-0-2-0-001".to_string(),
        decision: document,
        freeze_receipt_digest,
        replay_digest: digest(51),
        replay_result: "Complete".to_string(),
        candidate_custody_digest: digest(52),
        freeze_complete: true,
        replay_complete: true,
        storage_locator: "s3://release-authority-2026/cargo-allow/0.2.0/auth-0-2-0-001.json"
            .to_string(),
        repository_root: String::new(),
        storage_access_policy: "release-authority-read".to_string(),
        storage_retention_expiry_unix_seconds: MINTED_AT + 31_536_000,
        storage_provider_available: true,
        valid_from_unix_seconds: VALID_FROM,
        expires_at_unix_seconds: EXPIRES_AT,
        minted_by: "maintainer:release-operator".to_string(),
        minted_at_unix_seconds: MINTED_AT,
    }
}

fn repository_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let crates_dir = manifest_dir
        .parent()
        .ok_or_else(|| io::Error::other("cargo-allow manifest has no crates parent"))?;
    let root = crates_dir
        .parent()
        .ok_or_else(|| io::Error::other("cargo-allow crates directory has no repository parent"))?;
    Ok(root.to_path_buf())
}

fn minted() -> Result<CargoAllowReleaseAuthorizationCustodyV1, Box<dyn Error>> {
    Ok(mint_authorization_custody_v1(mint_init(decision()?)).map_err(io::Error::other)?)
}

fn selected() -> Result<CargoAllowReleaseAuthorizationCustodyV1, Box<dyn Error>> {
    let mut record = minted()?;
    let rendered = render_release_authorization_custody_v1(&record)?;
    require(
        note_custody_readback_v1(&mut record, rendered.as_bytes()) == CustodyReadbackV1::Match,
        "synthetic readback must match",
    )?;
    let evidence = record.evidence_digest.clone();
    select_authorization_for_run_v1(&mut record, &digest(77), NONCE, SELECT_AT, &evidence, true)
        .map_err(io::Error::other)?;
    Ok(record)
}

#[test]
fn release_authorization_minting() -> Result<(), Box<dyn Error>> {
    let record = minted()?;
    require(
        record.schema_id == AUTHORIZATION_CUSTODY_SCHEMA_ID
            && record.schema_version == AUTHORIZATION_CUSTODY_SCHEMA_VERSION,
        "custody record must carry the current generation",
    )?;
    require(
        record.state == Consumption::Available && !record.readback_verified,
        "a minted record starts Available before any readback",
    )?;
    require(
        record.redacted && record.one_run_scope && record.transitions.is_empty(),
        "a minted record is redacted, one-run, and transition-free",
    )?;
    require(
        record.evidence_digest == authorization_evidence_digest_v1(&decision()?.evidence)?,
        "custody must bind the exact decision evidence digest",
    )?;
    require(
        record.nonce == NONCE && record.authorization_digest.len() > "sha256:".len(),
        "custody must bind the one-use nonce and decision digest",
    )?;
    require(
        record.claim_boundary.contains("does not authenticate"),
        "custody lost its modeling-only claim boundary",
    )
}

#[test]
fn release_authorization_minting_refusals() -> Result<(), Box<dyn Error>> {
    // Control: minting before a Complete freeze/replay is refused.
    let mut init = mint_init(decision()?);
    init.freeze_complete = false;
    require(
        mint_authorization_custody_v1(init).is_err(),
        "minting without a Complete freeze must fail",
    )?;
    let mut init = mint_init(decision()?);
    init.replay_complete = false;
    require(
        mint_authorization_custody_v1(init).is_err(),
        "minting without a Complete replay must fail",
    )?;
    let mut init = mint_init(decision()?);
    init.replay_result = "Incomplete".to_string();
    require(
        mint_authorization_custody_v1(init).is_err(),
        "minting against a non-Complete replay must fail",
    )?;
    // Control: incomplete custody/replay/package digests are refused.
    let mut init = mint_init(decision()?);
    init.replay_digest = "not-a-digest".to_string();
    require(
        mint_authorization_custody_v1(init).is_err(),
        "minting with a malformed replay digest must fail",
    )?;
    // Control: identity values are canonical lowercase hex, converged with
    // the #3930 tag transaction contract; uppercase aliases are refused.
    let mut document = decision()?;
    document.freeze.commit = "A".repeat(40);
    document.freeze.denominator_digest =
        allow_report::release_authorization_denominator_binding_v1(&document.freeze)?;
    require(
        mint_authorization_custody_v1(mint_init(document)).is_err(),
        "minting with an uppercase custody commit must fail",
    )?;
    let mut document = decision()?;
    document.freeze.packages.truncate(9);
    document.freeze.denominator_digest =
        allow_report::release_authorization_denominator_binding_v1(&document.freeze)?;
    require(
        mint_authorization_custody_v1(mint_init(document)).is_err(),
        "minting with nine package rows must fail",
    )?;
    let mut init = mint_init(decision()?);
    init.freeze_receipt_digest = digest(50);
    require(
        mint_authorization_custody_v1(init).is_err(),
        "minting against a different well-formed freeze receipt must fail",
    )?;
    let mut document = decision()?;
    document
        .freeze
        .packages
        .first_mut()
        .ok_or_else(|| io::Error::other("package row absent"))?
        .package_size_bytes = 0;
    document.freeze.denominator_digest =
        allow_report::release_authorization_denominator_binding_v1(&document.freeze)?;
    require(
        mint_authorization_custody_v1(mint_init(document)).is_err(),
        "minting with a zero-size package row must fail",
    )?;
    // Control: broad prose or another operation cannot substitute typed minting.
    let mut document = decision()?;
    document.operation.name = RELEASE_AUTHORIZATION_RECOVERY_OPERATION.to_string();
    require(
        mint_authorization_custody_v1(mint_init(document)).is_err(),
        "minting the recovery operation must fail",
    )?;
    let mut document = decision()?;
    document.operation.version = "0.2.0-rc.1".to_string();
    document.operation.tag = "v0.2.0-rc.1".to_string();
    require(
        mint_authorization_custody_v1(mint_init(document)).is_err(),
        "minting a prerelease identity must fail",
    )?;
    let mut document = decision()?;
    document.authority.one_run_scope = false;
    require(
        mint_authorization_custody_v1(mint_init(document)).is_err(),
        "minting without one-run scope must fail",
    )?;
    let mut document = decision()?;
    document.authority.nonce = String::new();
    require(
        mint_authorization_custody_v1(mint_init(document)).is_err(),
        "minting without a nonce must fail",
    )?;
    let mut document = decision()?;
    document.authority.nonce = "ghp_synthetic-secret-marker".to_string();
    require(
        mint_authorization_custody_v1(mint_init(document)).is_err(),
        "minting with secret material in the nonce must fail",
    )?;
    // Control: the authorization must never be committed into the tree.
    let mut init = mint_init(decision()?);
    init.storage_locator = "policy/allow.toml".to_string();
    require(
        mint_authorization_custody_v1(init).is_err(),
        "minting with an in-tree locator must fail",
    )?;
    let mut init = mint_init(decision()?);
    init.storage_locator = "authorizations/auth-001.json".to_string();
    require(
        mint_authorization_custody_v1(init).is_err(),
        "minting with a schemeless locator must fail",
    )?;
    // Control: unavailable storage and inverted windows are refused.
    let mut init = mint_init(decision()?);
    init.storage_provider_available = false;
    require(
        mint_authorization_custody_v1(init).is_err(),
        "minting against an unavailable provider must fail",
    )?;
    let mut init = mint_init(decision()?);
    init.expires_at_unix_seconds = init.valid_from_unix_seconds;
    require(
        mint_authorization_custody_v1(init).is_err(),
        "minting with an inverted validity window must fail",
    )?;
    // Control: secret markers never enter custody records.
    let mut init = mint_init(decision()?);
    init.storage_locator = "s3://release-authority-2026/auth.json?token=abc".to_string();
    require(
        mint_authorization_custody_v1(init).is_err(),
        "minting with secret material in the locator must fail",
    )?;
    // Control: custody expiry must not outlive the decision expiry.
    let mut document = decision()?;
    document.authority.expires_at_unix_seconds = EXPIRES_AT - 1;
    require(
        mint_authorization_custody_v1(mint_init(document)).is_err(),
        "minting custody that outlives the decision must fail",
    )?;
    // Control: absolute file locators resolve against the repository root.
    let mut init = mint_init(decision()?);
    init.storage_locator = "file:///vault/release/auth-0-2-0-001.json".to_string();
    init.repository_root = "/repo/checkout".to_string();
    require(
        mint_authorization_custody_v1(init).is_ok(),
        "an out-of-tree file locator must mint",
    )?;
    let mut init = mint_init(decision()?);
    init.storage_locator = "file:///repo/checkout/crates/allow-report/src/lib.rs".to_string();
    init.repository_root = "/repo/checkout".to_string();
    require(
        mint_authorization_custody_v1(init).is_err(),
        "an in-tree file locator must fail",
    )?;
    let mut init = mint_init(decision()?);
    init.storage_locator = "file:///repo/elsewhere/../checkout/auth.json".to_string();
    init.repository_root = "/repo/checkout".to_string();
    require(
        mint_authorization_custody_v1(init).is_err(),
        "a traversal-normalized in-tree file locator must fail",
    )?;
    let mut init = mint_init(decision()?);
    init.storage_locator = "file:///vault/release/auth-0-2-0-001.json".to_string();
    init.repository_root = "/".to_string();
    require(
        mint_authorization_custody_v1(init).is_err(),
        "the filesystem root must classify every absolute file locator as in-tree",
    )?;
    let mut init = mint_init(decision()?);
    init.storage_locator = "file:///vault/release/auth-0-2-0-001.json".to_string();
    init.repository_root = String::new();
    require(
        mint_authorization_custody_v1(init).is_err(),
        "a file locator without a repository root must fail closed",
    )
}
#[test]
fn release_authorization_custody() -> Result<(), Box<dyn Error>> {
    let mut record = minted()?;
    let rendered = render_release_authorization_custody_v1(&record)?;
    require(
        verify_custody_readback_v1(&record, rendered.as_bytes()) == CustodyReadbackV1::Match,
        "exact stored bytes must read back as a match",
    )?;
    require(
        note_custody_readback_v1(&mut record, rendered.as_bytes()) == CustodyReadbackV1::Match
            && record.readback_verified
            && record
                .readback_digest
                .as_deref()
                .is_some_and(|value| value.len() == 71 && value.starts_with("sha256:")),
        "a matching readback must record the raw-bytes digest on the custody record",
    )?;
    require(
        note_custody_readback_v1(&mut record, rendered.as_bytes()) == CustodyReadbackV1::Match,
        "repeating the identical stored readback must stay a match",
    )?;
    // Control: a later failed observation invalidates prior readback admission.
    let forged = rendered.replace("auth-0-2-0-001", "auth-0-2-0-002");
    require(
        note_custody_readback_v1(&mut record, forged.as_bytes()) == CustodyReadbackV1::Mismatch
            && !record.readback_verified
            && record.readback_digest.is_none(),
        "storage readback drift must invalidate a prior successful observation",
    )?;
    let evidence = record.evidence_digest.clone();
    require(
        select_authorization_for_run_v1(
            &mut record,
            &digest(77),
            NONCE,
            SELECT_AT,
            &evidence,
            true,
        )
        .is_err(),
        "selection after a failed readback observation must fail",
    )?;
    require(
        note_custody_readback_v1(&mut record, rendered.as_bytes()) == CustodyReadbackV1::Match,
        "a later exact readback may re-establish admission",
    )?;
    require(
        note_custody_readback_v1(&mut record, b"not json") == CustodyReadbackV1::Malformed
            && !record.readback_verified
            && record.readback_digest.is_none(),
        "malformed storage bytes must invalidate readback admission",
    )?;
    // Control: custody records carry no secret material.
    for marker in [
        "BEGIN PRIVATE KEY",
        "ghp_",
        "github_pat_",
        "AKIA",
        "xoxb-",
        "xoxp-",
        "password=",
        "token=",
        "CARGO_REGISTRY_TOKEN",
    ] {
        require(
            !rendered.contains(marker),
            format!("custody artifact leaks secret marker {marker}"),
        )?;
    }
    Ok(())
}

#[test]
fn release_authorization_custody_refusals() -> Result<(), Box<dyn Error>> {
    // Control: selection requires a verified independent readback first.
    let mut record = minted()?;
    let evidence = record.evidence_digest.clone();
    require(
        select_authorization_for_run_v1(
            &mut record,
            &digest(77),
            NONCE,
            SELECT_AT,
            &evidence,
            true,
        )
        .is_err(),
        "selection without a verified readback must fail",
    )?;
    // Control: evidence identity is canonical lowercase hex; an uppercase
    // alias of the same digest is refused rather than case-folded.
    let mut record = minted()?;
    let rendered = render_release_authorization_custody_v1(&record)?;
    require(
        note_custody_readback_v1(&mut record, rendered.as_bytes()) == CustodyReadbackV1::Match,
        "synthetic readback must match",
    )?;
    let evidence = format!(
        "sha256:{}",
        record.evidence_digest["sha256:".len()..].to_uppercase()
    );
    require(
        select_authorization_for_run_v1(
            &mut record,
            &digest(77),
            NONCE,
            SELECT_AT,
            &evidence,
            true,
        )
        .is_err(),
        "selection with an uppercase evidence digest must fail",
    )?;
    // Control: revocation ends selection; terminal history cannot be revoked.
    let mut record = minted()?;
    let minted_bytes = render_release_authorization_custody_v1(&record)?;
    revoke_authorization_custody_v1(&mut record, "operator hold", SELECT_AT)
        .map_err(io::Error::other)?;
    require(
        record.state == Consumption::Revoked,
        "revocation must advance the custody state",
    )?;
    require(
        verify_custody_readback_v1(&record, minted_bytes.as_bytes()) == CustodyReadbackV1::Mismatch,
        "a revoked record must no longer read back as the minted bytes",
    )?;
    let evidence = record.evidence_digest.clone();
    require(
        select_authorization_for_run_v1(
            &mut record,
            &digest(77),
            NONCE,
            SELECT_AT + 5,
            &evidence,
            true,
        )
        .is_err(),
        "selection of a revoked authorization must fail",
    )?;
    require(
        revoke_authorization_custody_v1(&mut record, "", SELECT_AT).is_err(),
        "revocation without a reason must fail",
    )?;
    let mut record = minted()?;
    require(
        revoke_authorization_custody_v1(
            &mut record,
            "operator hold token=synthetic-secret",
            SELECT_AT,
        )
        .is_err(),
        "secret material in a revocation reason must fail",
    )?;
    Ok(())
}

#[test]
fn release_authorization_consumption() -> Result<(), Box<dyn Error>> {
    let mut record = selected()?;
    require(
        record.state == Consumption::SelectedForRun
            && record.consumed_nonces == vec![NONCE.to_string()]
            && record.transitions.len() == 1,
        "selection must advance state, consume the nonce, and append one transition",
    )?;
    note_irreversible_start_v1(&mut record, SELECT_AT + 10).map_err(io::Error::other)?;
    let observation =
        settle_authorization_consumption_v1(&mut record, &digest(77), true, SELECT_AT + 20)
            .map_err(io::Error::other)?;
    require(
        record.state == Consumption::ConsumedComplete
            && observation.from == Consumption::IrreversibleOperationStarted
            && observation.to == Consumption::ConsumedComplete
            && observation.nonce == NONCE,
        "settlement must close the consumption lifecycle exactly once",
    )?;
    require(
        record.transitions.len() == 3,
        "mint-free lifecycle must append select, start, and settle transitions",
    )?;
    // Control: terminal history cannot be revoked or selected again.
    require(
        revoke_authorization_custody_v1(&mut record, "late hold", SELECT_AT + 30).is_err(),
        "revoking terminal consumption history must fail",
    )?;
    let evidence = record.evidence_digest.clone();
    require(
        select_authorization_for_run_v1(
            &mut record,
            &digest(77),
            NONCE,
            SELECT_AT + 30,
            &evidence,
            true,
        )
        .is_err(),
        "selecting a consumed authorization must fail",
    )?;
    // Control: the bounded workflow payload binds identity without secrets.
    let identity = canonical_operation_identity_for(&record)?;
    let payload = selection_payload_v1(&identity, &record).map_err(io::Error::other)?;
    require(
        payload.operation_name == RELEASE_AUTHORIZATION_FINAL_OPERATION
            && payload.operation_version == "0.2.0"
            && payload.operation_tag == "v0.2.0"
            && payload.operation_channel == "stable"
            && payload.authorization_digest == record.authorization_digest
            && payload.operation_identity_digest
                == release_operation_identity_digest_v1(&identity).map_err(io::Error::other)?
            && payload.denominator_digest == record.freeze.denominator_digest
            && payload.evidence_digest == record.evidence_digest
            && payload.nonce == NONCE,
        "selection payload must bind the exact selected operation",
    )?;
    // Control: a foreign authorization never yields this operation's payload.
    let mut foreign_record = record.clone();
    foreign_record.authorization_digest = digest(999);
    require(
        selection_payload_v1(&identity, &foreign_record).is_err(),
        "a foreign authorization must never yield the operation payload",
    )?;
    let payload_json = serde_json::to_string(&payload)?;
    for marker in [
        "token=",
        "password=",
        "BEGIN PRIVATE KEY",
        "CARGO_REGISTRY_TOKEN",
    ] {
        require(
            !payload_json.contains(marker),
            format!("selection payload leaks secret marker {marker}"),
        )?;
    }
    Ok(())
}

#[test]
fn release_authorization_consumption_refusals() -> Result<(), Box<dyn Error>> {
    // Control: the same authorization can never be selected twice.
    let mut record = selected()?;
    let evidence = record.evidence_digest.clone();
    require(
        select_authorization_for_run_v1(
            &mut record,
            &digest(77),
            NONCE,
            SELECT_AT + 5,
            &evidence,
            true,
        )
        .is_err(),
        "the second selection of one authorization must fail",
    )?;
    // Control: wrong or empty nonces are refused.
    let mut record = minted()?;
    let rendered = render_release_authorization_custody_v1(&record)?;
    require(
        note_custody_readback_v1(&mut record, rendered.as_bytes()) == CustodyReadbackV1::Match,
        "synthetic readback must match",
    )?;
    let evidence = record.evidence_digest.clone();
    require(
        select_authorization_for_run_v1(
            &mut record,
            &digest(77),
            "wrong-nonce",
            SELECT_AT,
            &evidence,
            true,
        )
        .is_err(),
        "selection with the wrong nonce must fail",
    )?;
    require(
        select_authorization_for_run_v1(&mut record, &digest(77), "", SELECT_AT, &evidence, true)
            .is_err(),
        "selection with an empty nonce must fail",
    )?;
    // Control: the validity window binds both ends.
    require(
        select_authorization_for_run_v1(
            &mut record,
            &digest(77),
            NONCE,
            VALID_FROM - 1,
            &evidence,
            true,
        )
        .is_err(),
        "selection before valid-from must fail",
    )?;
    // Control: changed workflow/control evidence invalidates selection.
    require(
        select_authorization_for_run_v1(
            &mut record,
            &digest(77),
            NONCE,
            SELECT_AT,
            &digest(99),
            true,
        )
        .is_err(),
        "selection against changed evidence must fail",
    )?;
    // Control: unavailable storage cannot carry selection.
    require(
        select_authorization_for_run_v1(
            &mut record,
            &digest(77),
            NONCE,
            SELECT_AT,
            &evidence,
            false,
        )
        .is_err(),
        "selection against an unavailable provider must fail",
    )?;
    // Control: expiry ends selection and is observed on the record.
    require(
        select_authorization_for_run_v1(
            &mut record,
            &digest(77),
            NONCE,
            EXPIRES_AT + 1,
            &evidence,
            true,
        )
        .is_err()
            && record.state == Consumption::Expired,
        "selection past expiry must fail and observe expiry",
    )?;
    // Control: a clean authorization is never reused after an incident.
    let mut record = selected()?;
    note_irreversible_start_v1(&mut record, SELECT_AT + 10).map_err(io::Error::other)?;
    settle_authorization_consumption_v1(&mut record, &digest(77), false, SELECT_AT + 20)
        .map_err(io::Error::other)?;
    require(
        record.state == Consumption::ConsumedIncident,
        "incident settlement must be observed",
    )?;
    let evidence = record.evidence_digest.clone();
    require(
        select_authorization_for_run_v1(
            &mut record,
            &digest(77),
            NONCE,
            SELECT_AT + 30,
            &evidence,
            true,
        )
        .is_err(),
        "reuse after an incident must fail",
    )?;
    // Control: expiry is rechecked at the irreversible boundary.
    let mut record = selected()?;
    require(
        note_irreversible_start_v1(&mut record, EXPIRES_AT + 1).is_err()
            && record.state == Consumption::Expired,
        "an authorization that expires after selection cannot start irreversible work",
    )?;
    // Control: lifecycle event time is monotonic and failures do not mutate state.
    let mut record = selected()?;
    let before = record.clone();
    require(
        note_irreversible_start_v1(&mut record, SELECT_AT - 1).is_err() && record == before,
        "a backwards irreversible-start timestamp must fail without mutation",
    )?;
    // Control: work started while authority is live may settle after expiry.
    let mut record = selected()?;
    note_irreversible_start_v1(&mut record, EXPIRES_AT).map_err(io::Error::other)?;
    require(
        settle_authorization_consumption_v1(&mut record, &digest(77), true, EXPIRES_AT + 10)
            .is_ok()
            && record.state == Consumption::ConsumedComplete,
        "already-started irreversible work must remain settleable after expiry",
    )?;
    // Control: lifecycle order is enforced.
    let mut record = minted()?;
    require(
        note_irreversible_start_v1(&mut record, SELECT_AT).is_err(),
        "starting before selection must fail",
    )?;
    require(
        settle_authorization_consumption_v1(&mut record, &digest(77), true, SELECT_AT).is_err(),
        "settling before selection must fail",
    )
}

#[test]
fn rendered_consumption_validates_against_json_schema() -> Result<(), Box<dyn Error>> {
    let root = repository_root()?;
    if !root.join(".git").exists() {
        return Ok(());
    }
    let schema: serde_json::Value = serde_json::from_str(&fs::read_to_string(
        root.join("docs/schemas/cargo-allow.release-authorization-consumption.v1.schema.json"),
    )?)?;
    let mut record = minted()?;
    let stored = render_release_authorization_custody_v1(&record)?;
    require(
        note_custody_readback_v1(&mut record, stored.as_bytes()) == CustodyReadbackV1::Match,
        "synthetic readback must match before consumption rendering",
    )?;
    let evidence = record.evidence_digest.clone();
    let observation = select_authorization_for_run_v1(
        &mut record,
        &digest(77),
        NONCE,
        SELECT_AT,
        &evidence,
        true,
    )
    .map_err(io::Error::other)?;
    require(
        observation.schema_id == AUTHORIZATION_CONSUMPTION_SCHEMA_ID
            && observation.schema_version == AUTHORIZATION_CONSUMPTION_SCHEMA_VERSION,
        "consumption observation must carry the current schema generation",
    )?;
    let rendered: serde_json::Value =
        serde_json::from_str(&render_release_authorization_consumption_v1(&observation)?)?;
    let validator = jsonschema::validator_for(&schema)
        .map_err(|error| io::Error::other(format!("consumption schema compiles: {error}")))?;
    validator.validate(&rendered).map_err(|error| {
        io::Error::other(format!(
            "rendered consumption observation violates schema: {error}"
        ))
    })?;
    Ok(())
}

#[test]
fn rendered_custody_validates_against_json_schema() -> Result<(), Box<dyn Error>> {
    let root = repository_root()?;
    if !root.join(".git").exists() {
        return Ok(());
    }
    let schema: serde_json::Value = serde_json::from_str(&fs::read_to_string(
        root.join("docs/schemas/cargo-allow.release-authorization-custody.v1.schema.json"),
    )?)?;
    let rendered: serde_json::Value =
        serde_json::from_str(&render_release_authorization_custody_v1(&minted()?)?)?;
    let validator = jsonschema::validator_for(&schema)
        .map_err(|error| io::Error::other(format!("custody schema compiles: {error}")))?;
    validator.validate(&rendered).map_err(|error| {
        io::Error::other(format!("rendered custody record violates schema: {error}"))
    })?;
    for field in [
        "schema_id",
        "schema_version",
        "authorization_id",
        "authorization_digest",
        "operation",
        "freeze",
        "evidence_digest",
        "mint",
        "storage",
        "state",
        "transitions",
        "redacted",
        "claim_boundary",
    ] {
        require(
            rendered.get(field).is_some(),
            format!("rendered custody dropped required field {field}"),
        )?;
    }
    require(
        rendered.get("redacted") == Some(&serde_json::Value::Bool(true)),
        "rendered custody must stay redacted",
    )
}

fn canonical_operation_identity_for(
    record: &CargoAllowReleaseAuthorizationCustodyV1,
) -> Result<allow_report::CargoAllowReleaseOperationIdentityV1, Box<dyn Error>> {
    let packages = RELEASE_AUTHORIZATION_SELECTION
        .iter()
        .filter(|(_, _, _, shared)| !*shared)
        .enumerate()
        .map(|(index, (logical_id, package_name, version, _))| {
            CargoAllowReleaseOperationPackageRowV1 {
                logical_id: (*logical_id).to_string(),
                package_name: (*package_name).to_string(),
                package_version: (*version).to_string(),
                package_digest: digest(100 + index as u64),
            }
        })
        .collect();
    let assets = RELEASE_OPERATION_ASSET_SELECTION
        .iter()
        .enumerate()
        .map(
            |(index, (asset_id, asset_name))| CargoAllowReleaseOperationAssetRowV1 {
                asset_id: (*asset_id).to_string(),
                asset_name: (*asset_name).to_string(),
                asset_digest: digest(200 + index as u64),
            },
        )
        .collect();
    Ok(
        build_release_operation_identity_v1(CargoAllowReleaseOperationIdentityInitV1 {
            nonce: "custody-binding-0001".to_string(),
            operation_class: CargoAllowReleaseOperationClassV1::CleanFinalPublication,
            authority_kind: CargoAllowReleaseOperationAuthorityKindV1::Clean,
            repository: "EffortlessMetrics/cargo-allow".to_string(),
            product: "cargo-allow".to_string(),
            version: "0.2.0".to_string(),
            tag: "v0.2.0".to_string(),
            channel: "stable".to_string(),
            github_prerelease: false,
            freeze_digest: digest(1),
            final_evidence_graph_digest: digest(2),
            custody_digest: digest(3),
            replay_digest: digest(4),
            authorization_digest: record.authorization_digest.clone(),
            cargo_lock_digest: digest(6),
            topology_digest: digest(7),
            support_digest: digest(8),
            channel_digest: digest(9),
            packages,
            assets,
            workflow_digest: digest(10),
            action_inventory_digest: digest(11),
            live_controls_digest: digest(12),
            incident_predecessor_operation_digest: None,
            incident_predecessor_head_digest: None,
            one_run_scope: true,
            expires_at_unix_seconds: 1_800_000_000,
        })
        .map_err(io::Error::other)?,
    )
}

#[test]
fn selection_binds_canonical_operation_authorization() -> Result<(), Box<dyn Error>> {
    let mut record = minted()?;
    let rendered = render_release_authorization_custody_v1(&record)?;
    require(
        note_custody_readback_v1(&mut record, rendered.as_bytes()) == CustodyReadbackV1::Match,
        "synthetic readback must match",
    )?;
    let identity = canonical_operation_identity_for(&record)?;
    let expected = release_operation_identity_digest_v1(&identity).map_err(io::Error::other)?;
    let evidence = record.evidence_digest.clone();
    let observation = select_authorization_for_operation_v1(
        &identity,
        &mut record,
        NONCE,
        SELECT_AT,
        &evidence,
        true,
    )
    .map_err(io::Error::other)?;
    require(
        observation.operation_identity_digest == expected
            && observation.authorization_digest == identity.authorization_digest,
        "selection must bind the operation identity and its authorization",
    )?;

    // A foreign authorization never selects under this operation.
    let mut foreign = minted()?;
    let rendered = render_release_authorization_custody_v1(&foreign)?;
    require(
        note_custody_readback_v1(&mut foreign, rendered.as_bytes()) == CustodyReadbackV1::Match,
        "synthetic readback must match",
    )?;
    foreign.authorization_digest = digest(999);
    let evidence = foreign.evidence_digest.clone();
    require(
        select_authorization_for_operation_v1(
            &identity,
            &mut foreign,
            NONCE,
            SELECT_AT,
            &evidence,
            true,
        )
        .is_err(),
        "a foreign authorization must never select under this operation",
    )?;

    // Settlement with a different digest fails and leaves the custody
    // record unchanged: no transition is recorded, no state moves.
    let mut record = minted()?;
    let rendered = render_release_authorization_custody_v1(&record)?;
    require(
        note_custody_readback_v1(&mut record, rendered.as_bytes()) == CustodyReadbackV1::Match,
        "synthetic readback must match",
    )?;
    let identity = canonical_operation_identity_for(&record)?;
    let evidence = record.evidence_digest.clone();
    select_authorization_for_operation_v1(
        &identity,
        &mut record,
        NONCE,
        SELECT_AT,
        &evidence,
        true,
    )
    .map_err(io::Error::other)?;
    let transitions_before = record.transitions.len();
    require(
        settle_authorization_consumption_v1(&mut record, &digest(999), true, SELECT_AT + 20)
            .is_err(),
        "settlement with a different digest must fail",
    )?;
    require(
        record.state == Consumption::SelectedForRun
            && record.transitions.len() == transitions_before,
        "failed settlement must leave the custody record unchanged",
    )?;
    Ok(())
}
