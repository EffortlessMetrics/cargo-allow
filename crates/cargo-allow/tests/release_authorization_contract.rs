use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use ReleaseAuthorizationConsumptionV1 as Consumption;
use ReleaseAuthorizationResultV1 as State;
use allow_report::{
    CargoAllowReleaseAuthorizationV1, RELEASE_AUTHORIZATION_AUTH_CLASS,
    RELEASE_AUTHORIZATION_FINAL_OPERATION, RELEASE_AUTHORIZATION_FINAL_TAG,
    RELEASE_AUTHORIZATION_FINAL_VERSION, RELEASE_AUTHORIZATION_SCHEMA_ID,
    RELEASE_AUTHORIZATION_SCHEMA_VERSION, RELEASE_AUTHORIZATION_SELECTION,
    RELEASE_AUTHORIZATION_STABLE_CHANNEL, ReleaseAuthorizationAuthorityKindV1,
    ReleaseAuthorizationAuthorityV1, ReleaseAuthorizationConsumptionV1,
    ReleaseAuthorizationEvidenceV1, ReleaseAuthorizationFreezeV1, ReleaseAuthorizationInputV1,
    ReleaseAuthorizationOperationV1, ReleaseAuthorizationPackageRowV1,
    ReleaseAuthorizationResultV1, ReleaseAuthorizationSecretAvailabilityV1,
    ReleaseAuthorizationSecretStateV1, ReleaseAuthorizationSharedRowV1,
    ReleaseAuthorizationSourceKindV1, ReleaseAuthorizationSourceV1,
    compile_release_authorization_v1, release_authorization_denominator_binding_v1,
    render_release_authorization_v1, transition_authorization_consumption,
};

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

fn input() -> Result<ReleaseAuthorizationInputV1, Box<dyn Error>> {
    let mut packages = Vec::new();
    let mut shared = Vec::new();
    for (logical, package, version, is_shared) in RELEASE_AUTHORIZATION_SELECTION {
        if is_shared {
            shared.push(ReleaseAuthorizationSharedRowV1 {
                logical_id: logical.to_string(),
                package_name: package.to_string(),
                package_version: version.to_string(),
                expected_checksum: digest(20),
                authority_digest: digest(21),
            });
        } else {
            packages.push(ReleaseAuthorizationPackageRowV1 {
                logical_id: logical.to_string(),
                package_name: package.to_string(),
                package_version: version.to_string(),
                package_digest: digest(10),
                package_size_bytes: 10_000,
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
            secret_availability: ReleaseAuthorizationSecretAvailabilityV1 {
                redacted: true,
                state: ReleaseAuthorizationSecretStateV1::Unknown,
            },
            maintainer_actor: "release-operator".to_string(),
            maintainer_role: "release-maintainer".to_string(),
            source: ReleaseAuthorizationSourceV1 {
                kind: ReleaseAuthorizationSourceKindV1::IssueComment,
                repository: "EffortlessMetrics/cargo-allow".to_string(),
                reference: "issue:2502#comment:1".to_string(),
                author: "release-operator".to_string(),
                body_digest: digest(40),
                statement: "Authorize publish_cargo_allow_final_0_2_0 for v0.2.0.".to_string(),
            },
            created_at_unix_seconds: 90,
            expires_at_unix_seconds: 200,
            one_run_scope: true,
            nonce: "nonce-0-2-0-0001".to_string(),
            prior_consumptions: Vec::new(),
            consumption: Consumption::Available,
        },
        frozen_file_digests: vec![digest(50)],
        evaluated_at_unix_seconds: 110,
    };
    document.freeze.denominator_digest =
        release_authorization_denominator_binding_v1(&document.freeze)?;
    Ok(document)
}

fn receipt(input: &ReleaseAuthorizationInputV1) -> CargoAllowReleaseAuthorizationV1 {
    compile_release_authorization_v1(input)
}

#[test]
fn release_authorization_contract_exact_document_compiles() -> Result<(), Box<dyn Error>> {
    let document = input()?;
    let compiled = receipt(&document);
    require(
        compiled.result == State::Complete,
        format!("exact document must compile: {compiled:?}"),
    )?;
    require(
        !compiled.authorization_digest.is_empty(),
        "compiled receipt must carry the authorization digest",
    )
}

#[test]
fn release_authorization_contract_state_machine_parity() -> Result<(), Box<dyn Error>> {
    require(
        transition_authorization_consumption(Consumption::Available, Consumption::SelectedForRun)?
            == Consumption::SelectedForRun,
        "available must select",
    )?;
    require(
        transition_authorization_consumption(
            Consumption::SelectedForRun,
            Consumption::IrreversibleOperationStarted,
        )? == Consumption::IrreversibleOperationStarted,
        "selection must start the irreversible operation",
    )?;
    require(
        transition_authorization_consumption(
            Consumption::IrreversibleOperationStarted,
            Consumption::ConsumedComplete,
        )? == Consumption::ConsumedComplete,
        "started operations must complete",
    )?;
    let mut consumed = input()?;
    consumed.authority.consumption = Consumption::ConsumedComplete;
    require(
        receipt(&consumed).result == State::Reused,
        "consumed authorization must not reselect",
    )?;
    for terminal in [Consumption::Expired, Consumption::Revoked] {
        require(
            transition_authorization_consumption(terminal, Consumption::Available).is_err(),
            "terminal states must not reselect",
        )?;
    }
    Ok(())
}

#[test]
fn release_authorization_contract_hostile_documents_fail() -> Result<(), Box<dyn Error>> {
    let document = input()?;
    // Broad prose cannot construct a typed document.
    let prose: Result<ReleaseAuthorizationInputV1, _> =
        serde_json::from_value(serde_json::json!({"statement": "ship it"}));
    require(prose.is_err(), "broad prose constructed a document")?;
    // Moved commit mismatches the denominator binding.
    let mut moved = document.clone();
    moved.freeze.commit = "c".repeat(40);
    require(
        receipt(&moved).result == State::Mismatch,
        "moved commit compiled clean",
    )?;
    // RC identity mismatches the final operation.
    let mut rc = document.clone();
    rc.operation.version = "0.2.0-rc.1".to_string();
    rc.operation.tag = "v0.2.0-rc.1".to_string();
    rc.operation.github_prerelease = true;
    require(
        receipt(&rc).result != State::Complete,
        "RC identity compiled clean",
    )?;
    // Reused nonce and consumed states cannot reselect.
    let mut reused = document.clone();
    reused.authority.prior_consumptions = vec![reused.authority.nonce.clone()];
    require(
        receipt(&reused).result == State::Reused,
        "reused nonce compiled clean",
    )?;
    // Wrong auth class and revoked authority are unauthorized.
    let mut wrong_class = document.clone();
    wrong_class.authority.selected_auth_class = "github_pat".to_string();
    require(
        receipt(&wrong_class).result == State::Unauthorized,
        "wrong auth class compiled clean",
    )?;
    let mut revoked = document.clone();
    revoked.authority.consumption = Consumption::Revoked;
    require(
        receipt(&revoked).result == State::Unauthorized,
        "revoked authority compiled clean",
    )?;
    // Expired documents stay expired.
    let mut expired = document;
    expired.authority.expires_at_unix_seconds = 100;
    require(
        receipt(&expired).result == State::Expired,
        "expired document compiled clean",
    )
}

#[test]
fn release_authorization_contract_rendered_receipt_matches_schema() -> Result<(), Box<dyn Error>> {
    let root = repository_root()?;
    if !root.join(".git").exists() {
        return Ok(());
    }
    let schema: serde_json::Value = serde_json::from_str(&fs::read_to_string(
        root.join("docs/schemas/cargo-allow.release-authorization.v1.schema.json"),
    )?)?;
    require(
        schema
            .pointer("/properties/schema_id/const")
            .and_then(serde_json::Value::as_str)
            == Some(RELEASE_AUTHORIZATION_SCHEMA_ID),
        "schema identity drifted from the production contract",
    )?;
    let required = schema
        .get("required")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| io::Error::other("schema has no required fields"))?;
    for field in [
        "schema_id",
        "schema_version",
        "result",
        "findings",
        "caveats",
        "authorization_digest",
        "evaluated_at_unix_seconds",
    ] {
        require(
            required.iter().any(|entry| entry.as_str() == Some(field)),
            format!("schema dropped required field {field}"),
        )?;
    }
    let document = input()?;
    let rendered: serde_json::Value =
        serde_json::from_str(&render_release_authorization_v1(&receipt(&document))?)?;
    require(
        rendered.get("result").and_then(serde_json::Value::as_str) == Some("complete"),
        "rendered receipt must carry the compiled result",
    )
}
