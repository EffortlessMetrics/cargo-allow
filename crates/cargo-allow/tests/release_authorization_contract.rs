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
    ReleaseAuthorizationResultV1, ReleaseAuthorizationSharedRowV1,
    ReleaseAuthorizationSourceKindV1, ReleaseAuthorizationSourceV1,
    compile_release_authorization_v1, release_authorization_denominator_binding_v1,
    render_release_authorization_v1, transition_authorization_consumption,
};

const EXPECTED_CONTEXT_SCHEMA_ID: &str = "cargo-allow.release-authorization-expected-context.v1";
const REPOSITORY: &str = "EffortlessMetrics/cargo-allow";
const EXACT_STATEMENT: &str = "Authorize publish_cargo_allow_final_0_2_0 for v0.2.0.";

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
            preflight_result:
                allow_report::FinalRegistryPreflightResultV1::CompleteWithResidualAuthorityRisk,
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
                reference: "issue:2502#comment:1".to_string(),
                author: "release-operator".to_string(),
                body_digest: digest(60),
                statement: EXACT_STATEMENT.to_string(),
            },
            created_at_unix_seconds: 90,
            expires_at_unix_seconds: 200,
            one_run_scope: true,
            nonce: "nonce-0-2-0-0001".to_string(),
        },
    };
    document.freeze.denominator_digest =
        release_authorization_denominator_binding_v1(&document.freeze)?;
    Ok(document)
}

fn expected_context(document: &ReleaseAuthorizationInputV1) -> serde_json::Value {
    serde_json::json!({
        "schema_id": EXPECTED_CONTEXT_SCHEMA_ID,
        "schema_version": 1,
        "repository": REPOSITORY,
        "freeze": document.freeze.clone(),
        "evidence": document.evidence.clone(),
        "secret_availability": {
            "redacted": true,
            "state": "unknown"
        },
        "use_observation": {
            "state": "available",
            "consumed_nonces": []
        },
        "frozen_file_digests": [digest(70), digest(71)],
        "evaluated_at_unix_seconds": 110
    })
}

fn receipt(
    document: &ReleaseAuthorizationInputV1,
    context: &serde_json::Value,
) -> Result<CargoAllowReleaseAuthorizationV1, Box<dyn Error>> {
    Ok(compile_release_authorization_v1(
        document,
        &serde_json::to_vec(context)?,
    ))
}

#[test]
fn exact_decision_compiles_against_external_context() -> Result<(), Box<dyn Error>> {
    let document = decision()?;
    let context = expected_context(&document);
    let compiled = receipt(&document, &context)?;
    require(
        compiled.result == State::Complete,
        format!("exact decision must compile: {compiled:?}"),
    )?;
    require(
        !compiled.authorization_digest.is_empty() && !compiled.expected_context_digest.is_empty(),
        "compiled receipt must carry both independent identities",
    )?;
    require(
        compiled
            .claim_boundary
            .contains("independently supplied trusted"),
        "compiled receipt lost its two-sided claim boundary",
    )
}

#[test]
fn redigested_submission_cannot_move_trusted_freeze() -> Result<(), Box<dyn Error>> {
    let mut document = decision()?;
    let context = expected_context(&document);
    document
        .freeze
        .packages
        .first_mut()
        .ok_or_else(|| io::Error::other("package row absent"))?
        .package_size_bytes += 1;
    document.freeze.denominator_digest =
        release_authorization_denominator_binding_v1(&document.freeze)?;
    let compiled = receipt(&document, &context)?;
    require(
        compiled.result == State::Mismatch,
        format!("redigested forged denominator compiled: {compiled:?}"),
    )
}

#[test]
fn typed_broad_prose_and_external_reuse_fail() -> Result<(), Box<dyn Error>> {
    let mut document = decision()?;
    let mut context = expected_context(&document);
    document.authority.source.statement = "ship it".to_string();
    require(
        receipt(&document, &context)?.result == State::Unauthorized,
        "typed broad prose compiled clean",
    )?;

    document.authority.source.statement = EXACT_STATEMENT.to_string();
    context["use_observation"]["consumed_nonces"] =
        serde_json::json!([document.authority.nonce.clone()]);
    require(
        receipt(&document, &context)?.result == State::Reused,
        "externally consumed nonce compiled clean",
    )
}

#[test]
fn state_machine_records_append_only_operation_progress() -> Result<(), Box<dyn Error>> {
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
            Consumption::ConsumedIncident,
        )? == Consumption::ConsumedIncident,
        "started operation must retain incident completion",
    )?;
    require(
        transition_authorization_consumption(
            Consumption::ConsumedIncident,
            Consumption::SelectedForRun,
        )
        .is_err(),
        "terminal incident state reselected",
    )
}

#[test]
fn rendered_receipt_validates_against_json_schema() -> Result<(), Box<dyn Error>> {
    let root = repository_root()?;
    if !root.join(".git").exists() {
        return Ok(());
    }
    let schema: serde_json::Value = serde_json::from_str(&fs::read_to_string(
        root.join("docs/schemas/cargo-allow.release-authorization.v1.schema.json"),
    )?)?;
    let document = decision()?;
    let context = expected_context(&document);
    let rendered: serde_json::Value = serde_json::from_str(&render_release_authorization_v1(
        &receipt(&document, &context)?,
    )?)?;
    let validator = jsonschema::validator_for(&schema)
        .map_err(|error| io::Error::other(format!("authorization schema compiles: {error}")))?;
    validator.validate(&rendered).map_err(|error| {
        io::Error::other(format!(
            "rendered authorization receipt violates schema: {error}"
        ))
    })?;
    for field in [
        "schema_id",
        "schema_version",
        "result",
        "findings",
        "caveats",
        "authorization_digest",
        "expected_context_digest",
        "evaluated_at_unix_seconds",
        "claim_boundary",
    ] {
        require(
            rendered.get(field).is_some(),
            format!("rendered receipt dropped required field {field}"),
        )?;
    }
    require(
        rendered.get("result").and_then(serde_json::Value::as_str) == Some("complete"),
        "rendered receipt must carry the compiled result",
    )
}
