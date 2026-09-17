use super::*;
use crate::FinalRegistryPreflightResultV1 as Preflight;
use crate::artifacts::release_identity_v1::ReleaseIdentityV1;

use ReleaseAuthorizationResultV1 as ResultState;

fn finding(
    findings: &mut Vec<ReleaseAuthorizationFindingV1>,
    result: ResultState,
    reason: impl Into<String>,
) {
    findings.push(ReleaseAuthorizationFindingV1 {
        result,
        reason: reason.into(),
    });
}

fn digest(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn git_sha(value: &str) -> bool {
    (value.len() == 40 || value.len() == 64)
        && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn content_digest<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    let bytes = serde_json::to_vec(value)?;
    Ok(allow_core::sha256_v1_bytes(&bytes).replacen("sha256:v1:", "sha256:", 1))
}

/// Canonical digest of the authorization statement. The frozen file inventory
/// is filing metadata, not statement identity: excluding it keeps the digest
/// stable so the inside-the-tree check cannot defeat itself.
fn authorization_statement_digest(
    input: &ReleaseAuthorizationInputV1,
) -> Result<String, serde_json::Error> {
    content_digest(&(
        input.schema_id.as_str(),
        input.schema_version,
        &input.operation,
        &input.freeze,
        &input.evidence,
        &input.authority,
        input.evaluated_at_unix_seconds,
    ))
}

/// Canonical binding over the full frozen denominator. The producer (#2501
/// tooling) computes this exact tuple; any moved commit, tree, lockfile,
/// topology, package, or shared row changes the binding and mismatches.
pub fn release_authorization_denominator_binding_v1(
    freeze: &ReleaseAuthorizationFreezeV1,
) -> Result<String, serde_json::Error> {
    content_digest(&(
        freeze.topology_id.as_str(),
        freeze.commit.as_str(),
        freeze.tree.as_str(),
        freeze.lock_digest.as_str(),
        freeze.packages.as_slice(),
        freeze.shared_prerequisites.as_slice(),
    ))
}

/// Checked one-use transition for the authorization lifecycle.
pub fn transition_authorization_consumption(
    current: ReleaseAuthorizationConsumptionV1,
    next: ReleaseAuthorizationConsumptionV1,
) -> Result<ReleaseAuthorizationConsumptionV1, &'static str> {
    use ReleaseAuthorizationConsumptionV1 as Consumption;
    match (current, next) {
        (Consumption::Available, Consumption::SelectedForRun)
        | (
            Consumption::SelectedForRun,
            Consumption::IrreversibleOperationStarted,
        )
        | (
            Consumption::IrreversibleOperationStarted,
            Consumption::ConsumedComplete,
        )
        | (
            Consumption::IrreversibleOperationStarted,
            Consumption::ConsumedIncident,
        )
        | (_, Consumption::Revoked) => Ok(next),
        (Consumption::Available, Consumption::Expired)
        | (Consumption::SelectedForRun, Consumption::Expired) => Ok(next),
        _ => Err("invalid authorization consumption transition"),
    }
}

/// Pure compilation. No adapter is invoked, no credential is read, no tag is
/// created, and no supplied evidence is authenticated.
pub fn compile_release_authorization_v1(
    input: &ReleaseAuthorizationInputV1,
) -> CargoAllowReleaseAuthorizationV1 {
    let mut findings = Vec::new();
    let mut caveats = Vec::new();
    if input.schema_id != RELEASE_AUTHORIZATION_SCHEMA_ID
        || input.schema_version != RELEASE_AUTHORIZATION_SCHEMA_VERSION
    {
        finding(
            &mut findings,
            ResultState::Unsupported,
            "non-current authorization generation",
        );
    }
    let operation = &input.operation;
    let known_operation = operation.name == RELEASE_AUTHORIZATION_FINAL_OPERATION
        || operation.name == RELEASE_AUTHORIZATION_RECOVERY_OPERATION;
    if !known_operation {
        finding(
            &mut findings,
            ResultState::Mismatch,
            "operation is not the selected final or recovery operation",
        );
    }
    match ReleaseIdentityV1::parse(
        &operation.version,
        &operation.tag,
        operation.github_prerelease,
    ) {
        Ok(identity) => {
            if operation.name == RELEASE_AUTHORIZATION_FINAL_OPERATION
                && (identity.version().as_str() != RELEASE_AUTHORIZATION_FINAL_VERSION
                    || operation.tag != RELEASE_AUTHORIZATION_FINAL_TAG
                    || operation.channel != RELEASE_AUTHORIZATION_STABLE_CHANNEL
                    || operation.github_prerelease)
            {
                finding(
                    &mut findings,
                    ResultState::Mismatch,
                    "final operation must bind exact 0.2.0 stable identity",
                );
            }
            if operation.channel.trim().is_empty() {
                finding(
                    &mut findings,
                    ResultState::Malformed,
                    "release channel must be non-blank",
                );
            }
        }
        Err(_) => finding(
            &mut findings,
            ResultState::Malformed,
            "operation version/tag/prerelease identity is malformed",
        ),
    }
    let clean = operation.authority_kind == ReleaseAuthorizationAuthorityKindV1::Clean;
    if operation.name == RELEASE_AUTHORIZATION_FINAL_OPERATION && !clean {
        finding(
            &mut findings,
            ResultState::Mismatch,
            "final operation requires clean publication authority",
        );
    }
    if operation.name == RELEASE_AUTHORIZATION_RECOVERY_OPERATION && clean {
        finding(
            &mut findings,
            ResultState::Mismatch,
            "recovery operation requires recovery authority",
        );
    }
    let freeze = &input.freeze;
    for (field, value) in [
        ("freeze receipt_digest", freeze.receipt_digest.as_str()),
        ("freeze candidate_digest", freeze.candidate_digest.as_str()),
        (
            "freeze denominator_digest",
            freeze.denominator_digest.as_str(),
        ),
        ("freeze lock_digest", freeze.lock_digest.as_str()),
    ] {
        if !digest(value) {
            finding(
                &mut findings,
                ResultState::Malformed,
                format!("{field} is malformed/missing"),
            );
        }
    }
    if !git_sha(&freeze.commit) {
        finding(
            &mut findings,
            ResultState::Malformed,
            "freeze commit is not a canonical commit SHA",
        );
    }
    if !git_sha(&freeze.tree) {
        finding(
            &mut findings,
            ResultState::Malformed,
            "freeze tree is not a canonical tree SHA",
        );
    }
    if freeze.topology_id != "CARGO-ALLOW-PKG-TOPOLOGY-V2-0001" {
        finding(
            &mut findings,
            ResultState::Malformed,
            "freeze topology is not the selected final generation",
        );
    }
    if freeze.packages.len() != 10 || freeze.shared_prerequisites.len() != 3 {
        finding(
            &mut findings,
            ResultState::Malformed,
            "freeze must carry exactly ten final and three shared rows",
        );
    }
    let mut shared_index = 0;
    for (index, (logical, package, version, shared)) in
        RELEASE_AUTHORIZATION_SELECTION.into_iter().enumerate()
    {        if shared {
            let row = freeze.shared_prerequisites.get(shared_index);
            shared_index += 1;
            let Some(row) = row else {
                finding(
                    &mut findings,
                    ResultState::Malformed,
                    "shared prerequisite row is missing",
                );
                continue;
            };
            if row.logical_id != logical
                || row.package_name != package
                || row.package_version != version
            {
                finding(
                    &mut findings,
                    ResultState::Mismatch,
                    format!("shared row {index} differs from the frozen denominator"),
                );
            }
            if !digest(&row.expected_checksum) || !digest(&row.authority_digest) {
                finding(
                    &mut findings,
                    ResultState::Malformed,
                    format!("shared row {package} checksum authority is malformed"),
                );
            }
            continue;
        }
        let row = freeze.packages.get(index - (shared_index));
        let Some(row) = row else {
            finding(
                &mut findings,
                ResultState::Malformed,
                "final package row is missing",
            );
            continue;
        };
        if row.logical_id != logical
            || row.package_name != package
            || row.package_version != version
        {
            finding(
                &mut findings,
                ResultState::Mismatch,
                format!("final row {index} differs from the frozen denominator"),
            );
        }
        if !digest(&row.package_digest) {
            finding(
                &mut findings,
                ResultState::Malformed,
                format!("final row {package} digest is malformed"),
            );
        }
    }
    match release_authorization_denominator_binding_v1(freeze) {
        Ok(binding) => {
            if !binding.eq_ignore_ascii_case(&freeze.denominator_digest) {
                finding(
                    &mut findings,
                    ResultState::Mismatch,
                    "frozen denominator rows do not match the denominator binding",
                );
            }
        }
        Err(error) => finding(
            &mut findings,
            ResultState::InstrumentFailure,
            format!("denominator binding serialization: {error}"),
        ),
    }
    let evidence = &input.evidence;
    for (field, value) in [
        ("package_docs_digest", evidence.package_docs_digest.as_str()),
        ("support_digest", evidence.support_digest.as_str()),
        ("manifest_digest", evidence.manifest_digest.as_str()),
        ("rehearsal_digest", evidence.rehearsal_digest.as_str()),
        (
            "source_controls_digest",
            evidence.source_controls_digest.as_str(),
        ),
        ("live_controls_digest", evidence.live_controls_digest.as_str()),
        ("workflow_digest", evidence.workflow_digest.as_str()),
        (
            "action_inventory_digest",
            evidence.action_inventory_digest.as_str(),
        ),
    ] {
        if !digest(value) {
            finding(
                &mut findings,
                ResultState::Malformed,
                format!("{field} is malformed/missing"),
            );
        }
    }
    if !evidence.rehearsal_complete_except_authorization {
        finding(
            &mut findings,
            ResultState::Mismatch,
            "zero-upload rehearsal must be complete except the authorization hold",
        );
    }
    if evidence.preflight_maximum_age_seconds == 0 {
        finding(
            &mut findings,
            ResultState::Malformed,
            "preflight freshness window must be positive",
        );
    }
    match input
        .evaluated_at_unix_seconds
        .checked_sub(evidence.preflight_evaluated_at_unix_seconds)
    {
        Some(age)
            if evidence.preflight_maximum_age_seconds > 0
                && age <= evidence.preflight_maximum_age_seconds => {}
        _ => finding(
            &mut findings,
            ResultState::Stale,
            "registry preflight observation is future-dated or expired",
        ),
    }
    match evidence.preflight_result {
        Preflight::Complete => {}
        Preflight::CompleteWithResidualAuthorityRisk => caveats.push(
            "registry permission remains unproven; residual authority risk travels with this authorization"
                .to_string(),
        ),
        Preflight::Conflict => finding(
            &mut findings,
            ResultState::Unauthorized,
            "immutable registry checksum conflict cannot be authorized",
        ),
        Preflight::Stale | Preflight::Incomplete | Preflight::ProviderUnavailable => finding(
            &mut findings,
            ResultState::Stale,
            "registry preflight is not current; refresh observation before authorization",
        ),
        Preflight::InstrumentFailure
        | Preflight::Malformed
        | Preflight::UnsupportedGeneration => finding(
            &mut findings,
            ResultState::Malformed,
            "registry preflight evidence is malformed",
        ),
    }
    if evidence.observed_context_digest.trim().is_empty()
        || evidence.current_context_digest.trim().is_empty()
    {
        finding(
            &mut findings,
            ResultState::Malformed,
            "evidence context digests must be non-blank",
        );
    } else if !evidence
        .observed_context_digest
        .eq_ignore_ascii_case(&evidence.current_context_digest)
    {
        finding(
            &mut findings,
            ResultState::Stale,
            "workflow/principal/environment/control movement invalidates evidence",
        );
    }
    let authority = &input.authority;
    if authority.selected_auth_class != RELEASE_AUTHORIZATION_AUTH_CLASS {
        finding(
            &mut findings,
            ResultState::Unauthorized,
            "selected auth class cannot carry the clean final operation",
        );
    }
    if !authority.secret_availability.redacted {
        finding(
            &mut findings,
            ResultState::Malformed,
            "secret values must never travel in authorization documents",
        );
    }
    if authority.secret_availability.state
        == ReleaseAuthorizationSecretStateV1::Unknown
    {
        caveats.push(
            "repository-secret availability is unproven; the token boundary rechecks it"
                .to_string(),
        );
    }
    if authority.maintainer_actor.trim().is_empty()
        || authority.maintainer_role.trim().is_empty()
    {
        finding(
            &mut findings,
            ResultState::Unauthorized,
            "maintainer actor and role are required",
        );
    }
    let source = &authority.source;
    if source.repository.trim().is_empty()
        || source.reference.trim().is_empty()
        || source.author.trim().is_empty()
        || !digest(&source.body_digest)
    {
        finding(
            &mut findings,
            ResultState::Malformed,
            "structured source reference is malformed",
        );
    }
    if source.statement.trim().is_empty()
        || source.statement.len() > RELEASE_AUTHORIZATION_MAX_STATEMENT_LEN
    {
        finding(
            &mut findings,
            ResultState::Malformed,
            "maintainer statement must be bounded prose",
        );
    }
    if authority.created_at_unix_seconds > input.evaluated_at_unix_seconds {
        finding(
            &mut findings,
            ResultState::Malformed,
            "authorization creation is future-dated",
        );
    }
    if authority.expires_at_unix_seconds <= authority.created_at_unix_seconds {
        finding(
            &mut findings,
            ResultState::Malformed,
            "authorization expiry must follow creation",
        );
    } else if input.evaluated_at_unix_seconds > authority.expires_at_unix_seconds {
        finding(
            &mut findings,
            ResultState::Expired,
            "authorization expired before evaluation",
        );
    }
    if authority.nonce.trim().is_empty() {
        finding(
            &mut findings,
            ResultState::Malformed,
            "one-use nonce is required",
        );
    } else if authority
        .prior_consumptions
        .iter()
        .any(|seen| seen == &authority.nonce)
    {
        finding(
            &mut findings,
            ResultState::Reused,
            "authorization nonce was already consumed",
        );
    }
    use ReleaseAuthorizationConsumptionV1 as Consumption;
    match authority.consumption {
        Consumption::Available => {}
        Consumption::SelectedForRun
        | Consumption::IrreversibleOperationStarted
        | Consumption::ConsumedComplete
        | Consumption::ConsumedIncident => finding(
            &mut findings,
            ResultState::Reused,
            "authorization was already selected or consumed",
        ),
        Consumption::Expired => finding(
            &mut findings,
            ResultState::Expired,
            "authorization is expired",
        ),
        Consumption::Revoked => finding(
            &mut findings,
            ResultState::Unauthorized,
            "authorization was revoked",
        ),
    }
    let authorization_digest = match authorization_statement_digest(input) {
        Ok(value) => value,
        Err(error) => {
            finding(
                &mut findings,
                ResultState::InstrumentFailure,
                format!("authorization digest serialization: {error}"),
            );
            String::new()
        }
    };
    if !authorization_digest.is_empty()
        && input.frozen_file_digests.iter().any(|entry| {
            entry.eq_ignore_ascii_case(&authorization_digest)
        })
    {
        finding(
            &mut findings,
            ResultState::Malformed,
            "authorization artifact lives inside the frozen tree",
        );
    }
    let result = findings
        .iter()
        .map(|finding| finding.result)
        .min()
        .unwrap_or(ResultState::Complete);
    CargoAllowReleaseAuthorizationV1 {
        schema_id: RELEASE_AUTHORIZATION_SCHEMA_ID.to_string(),
        schema_version: RELEASE_AUTHORIZATION_SCHEMA_VERSION,
        result,
        findings,
        caveats,
        authorization_digest,
        evaluated_at_unix_seconds: input.evaluated_at_unix_seconds,
    }
}
