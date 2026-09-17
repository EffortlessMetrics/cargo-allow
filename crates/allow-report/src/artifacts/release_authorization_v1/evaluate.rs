use super::*;
use crate::FinalRegistryPreflightResultV1 as Preflight;
use crate::artifacts::release_identity_v1::ReleaseIdentityV1;
use std::collections::BTreeSet;

use ReleaseAuthorizationResultV1 as ResultState;

const CLAIM_BOUNDARY: &str = "This receipt records deterministic eligibility of one immutable final-release authorization decision against an independently supplied trusted freeze, evidence, control, provider, and one-use context. It does not authenticate the source provider, create or move a tag, read a credential, upload a package, grant recovery authority, or execute publication.";

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
    (value.len() == 40 || value.len() == 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn decimal(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
}

fn content_digest<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    let bytes = serde_json::to_vec(value)?;
    Ok(allow_core::sha256_v1_bytes(&bytes).replacen("sha256:v1:", "sha256:", 1))
}

/// Canonical digest of the immutable authorization decision. Evaluation time,
/// provider observations, and mutable use state are deliberately excluded.
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
    ))
}

/// Canonical digest of the independently supplied expected context. Set-like
/// inventories are sorted so provider enumeration order is not authority.
fn expected_context_digest(
    context: &ReleaseAuthorizationExpectedContextV1,
) -> Result<String, serde_json::Error> {
    let mut consumed_nonces = context.use_observation.consumed_nonces.clone();
    consumed_nonces.sort();
    consumed_nonces.dedup();
    let mut frozen_file_digests = context.frozen_file_digests.clone();
    frozen_file_digests.sort();
    frozen_file_digests.dedup();
    content_digest(&(
        context.schema_id.as_str(),
        context.schema_version,
        context.repository.as_str(),
        &context.freeze,
        &context.evidence,
        &context.secret_availability,
        context.use_observation.state,
        consumed_nonces.as_slice(),
        frozen_file_digests.as_slice(),
        context.evaluated_at_unix_seconds,
    ))
}

/// Canonical binding over the full frozen denominator. The trusted #2501
/// producer computes this exact tuple; any moved commit, tree, lockfile,
/// topology, package, package size, or shared row changes the binding.
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

/// Checked transition for an append-only authorization-use record. The
/// immutable authorization decision itself is never rewritten.
pub fn transition_authorization_consumption(
    current: ReleaseAuthorizationConsumptionV1,
    next: ReleaseAuthorizationConsumptionV1,
) -> Result<ReleaseAuthorizationConsumptionV1, &'static str> {
    use ReleaseAuthorizationConsumptionV1 as Consumption;
    match (current, next) {
        (Consumption::Available, Consumption::SelectedForRun)
        | (Consumption::SelectedForRun, Consumption::IrreversibleOperationStarted)
        | (Consumption::IrreversibleOperationStarted, Consumption::ConsumedComplete)
        | (Consumption::IrreversibleOperationStarted, Consumption::ConsumedIncident)
        | (_, Consumption::Revoked) => Ok(next),
        (Consumption::Available, Consumption::Expired)
        | (Consumption::SelectedForRun, Consumption::Expired) => Ok(next),
        _ => Err("invalid authorization consumption transition"),
    }
}

fn validate_operation(
    operation: &ReleaseAuthorizationOperationV1,
    findings: &mut Vec<ReleaseAuthorizationFindingV1>,
) {
    if operation.name != RELEASE_AUTHORIZATION_FINAL_OPERATION {
        finding(
            findings,
            ResultState::Mismatch,
            "operation is not the selected clean final publication operation",
        );
    }
    match ReleaseIdentityV1::parse(
        &operation.version,
        &operation.tag,
        operation.github_prerelease,
    ) {
        Ok(identity) => {
            if identity.version().as_str() != RELEASE_AUTHORIZATION_FINAL_VERSION
                || operation.tag != RELEASE_AUTHORIZATION_FINAL_TAG
                || operation.channel != RELEASE_AUTHORIZATION_STABLE_CHANNEL
                || operation.github_prerelease
            {
                finding(
                    findings,
                    ResultState::Mismatch,
                    "operation must bind exact 0.2.0 stable identity",
                );
            }
        }
        Err(_) => finding(
            findings,
            ResultState::Malformed,
            "operation version/tag/prerelease identity is malformed",
        ),
    }
    if operation.authority_kind != ReleaseAuthorizationAuthorityKindV1::Clean {
        finding(
            findings,
            ResultState::Mismatch,
            "final operation requires clean publication authority",
        );
    }
}

fn validate_freeze(
    freeze: &ReleaseAuthorizationFreezeV1,
    subject: &str,
    invalid_state: ResultState,
    mismatch_state: ResultState,
    findings: &mut Vec<ReleaseAuthorizationFindingV1>,
) {
    for (field, value) in [
        ("receipt_digest", freeze.receipt_digest.as_str()),
        ("candidate_digest", freeze.candidate_digest.as_str()),
        ("denominator_digest", freeze.denominator_digest.as_str()),
        ("lock_digest", freeze.lock_digest.as_str()),
    ] {
        if !digest(value) {
            finding(
                findings,
                invalid_state,
                format!("{subject} freeze {field} is malformed/missing"),
            );
        }
    }
    if !git_sha(&freeze.commit) {
        finding(
            findings,
            invalid_state,
            format!("{subject} freeze commit is not a canonical commit SHA"),
        );
    }
    if !git_sha(&freeze.tree) {
        finding(
            findings,
            invalid_state,
            format!("{subject} freeze tree is not a canonical tree SHA"),
        );
    }
    if freeze.topology_id != "CARGO-ALLOW-PKG-TOPOLOGY-V2-0001" {
        finding(
            findings,
            mismatch_state,
            format!("{subject} freeze topology is not the selected final generation"),
        );
    }
    if freeze.packages.len() != 10 || freeze.shared_prerequisites.len() != 3 {
        finding(
            findings,
            invalid_state,
            format!("{subject} freeze must carry exactly ten final and three shared rows"),
        );
    }

    let mut upload_index = 0;
    let mut shared_index = 0;
    for (release_index, (logical, package, version, shared)) in
        RELEASE_AUTHORIZATION_SELECTION.into_iter().enumerate()
    {
        if shared {
            let Some(row) = freeze.shared_prerequisites.get(shared_index) else {
                finding(
                    findings,
                    invalid_state,
                    format!("{subject} shared prerequisite row is missing"),
                );
                shared_index += 1;
                continue;
            };
            shared_index += 1;
            if row.logical_id != logical
                || row.package_name != package
                || row.package_version != version
            {
                finding(
                    findings,
                    mismatch_state,
                    format!(
                        "{subject} shared row {release_index} differs from the selected denominator"
                    ),
                );
            }
            if !digest(&row.expected_checksum) || !digest(&row.authority_digest) {
                finding(
                    findings,
                    invalid_state,
                    format!("{subject} shared row {package} checksum authority is malformed"),
                );
            }
            continue;
        }

        let Some(row) = freeze.packages.get(upload_index) else {
            finding(
                findings,
                invalid_state,
                format!("{subject} final package row is missing"),
            );
            upload_index += 1;
            continue;
        };
        upload_index += 1;
        if row.logical_id != logical
            || row.package_name != package
            || row.package_version != version
        {
            finding(
                findings,
                mismatch_state,
                format!(
                    "{subject} final row {release_index} differs from the selected denominator"
                ),
            );
        }
        if !digest(&row.package_digest) || row.package_size_bytes == 0 {
            finding(
                findings,
                invalid_state,
                format!("{subject} final row {package} digest or size is malformed"),
            );
        }
    }

    match release_authorization_denominator_binding_v1(freeze) {
        Ok(binding) => {
            if !binding.eq_ignore_ascii_case(&freeze.denominator_digest) {
                finding(
                    findings,
                    mismatch_state,
                    format!("{subject} denominator rows do not match their binding"),
                );
            }
        }
        Err(error) => finding(
            findings,
            ResultState::InstrumentFailure,
            format!("{subject} denominator binding serialization failed: {error}"),
        ),
    }
}

fn validate_evidence_shape(
    evidence: &ReleaseAuthorizationEvidenceV1,
    subject: &str,
    invalid_state: ResultState,
    findings: &mut Vec<ReleaseAuthorizationFindingV1>,
) {
    for (field, value) in [
        ("package_docs_digest", evidence.package_docs_digest.as_str()),
        ("support_digest", evidence.support_digest.as_str()),
        ("manifest_digest", evidence.manifest_digest.as_str()),
        ("rehearsal_digest", evidence.rehearsal_digest.as_str()),
        (
            "source_controls_digest",
            evidence.source_controls_digest.as_str(),
        ),
        (
            "live_controls_digest",
            evidence.live_controls_digest.as_str(),
        ),
        ("workflow_digest", evidence.workflow_digest.as_str()),
        (
            "action_inventory_digest",
            evidence.action_inventory_digest.as_str(),
        ),
        (
            "observed_context_digest",
            evidence.observed_context_digest.as_str(),
        ),
        (
            "current_context_digest",
            evidence.current_context_digest.as_str(),
        ),
    ] {
        if !digest(value) {
            finding(
                findings,
                invalid_state,
                format!("{subject} evidence {field} is malformed/missing"),
            );
        }
    }
    if evidence.preflight_maximum_age_seconds == 0 {
        finding(
            findings,
            invalid_state,
            format!("{subject} preflight freshness window must be positive"),
        );
    }
}

fn validate_expected_evidence(
    evidence: &ReleaseAuthorizationEvidenceV1,
    evaluated_at_unix_seconds: u64,
    findings: &mut Vec<ReleaseAuthorizationFindingV1>,
    caveats: &mut Vec<String>,
) {
    if !evidence.rehearsal_complete_except_authorization {
        finding(
            findings,
            ResultState::Mismatch,
            "trusted zero-upload rehearsal is not complete except the authorization hold",
        );
    }
    match evaluated_at_unix_seconds.checked_sub(evidence.preflight_evaluated_at_unix_seconds) {
        Some(age)
            if evidence.preflight_maximum_age_seconds > 0
                && age <= evidence.preflight_maximum_age_seconds => {}
        _ => finding(
            findings,
            ResultState::Stale,
            "trusted registry preflight observation is future-dated or expired",
        ),
    }
    match evidence.preflight_result {
        Preflight::Complete => {}
        Preflight::CompleteWithResidualAuthorityRisk => caveats.push(
            "registry permission remains unproven; residual authority risk travels with this authorization"
                .to_string(),
        ),
        Preflight::Conflict => finding(
            findings,
            ResultState::Unauthorized,
            "immutable registry checksum conflict cannot be authorized",
        ),
        Preflight::Stale | Preflight::Incomplete | Preflight::ProviderUnavailable => finding(
            findings,
            ResultState::Stale,
            "registry preflight is not current; refresh observation before authorization",
        ),
        Preflight::InstrumentFailure => finding(
            findings,
            ResultState::InstrumentFailure,
            "registry preflight observation instrument failed",
        ),
        Preflight::Malformed | Preflight::UnsupportedGeneration => finding(
            findings,
            ResultState::Malformed,
            "registry preflight evidence is malformed or unsupported",
        ),
    }
    if !evidence
        .observed_context_digest
        .eq_ignore_ascii_case(&evidence.current_context_digest)
    {
        finding(
            findings,
            ResultState::Stale,
            "workflow/principal/environment/control movement invalidates evidence",
        );
    }
}

fn valid_source_reference(kind: ReleaseAuthorizationSourceKindV1, reference: &str) -> bool {
    match kind {
        ReleaseAuthorizationSourceKindV1::IssueComment => reference
            .strip_prefix("issue:")
            .and_then(|rest| rest.split_once("#comment:"))
            .is_some_and(|(issue, comment)| decimal(issue) && decimal(comment)),
        ReleaseAuthorizationSourceKindV1::WorkflowDispatch => reference
            .strip_prefix("workflow:")
            .and_then(|rest| rest.split_once("#run:"))
            .and_then(|(workflow, rest)| {
                rest.split_once("#attempt:")
                    .map(|(run, attempt)| (workflow, run, attempt))
            })
            .is_some_and(|(workflow, run, attempt)| {
                !workflow.trim().is_empty() && decimal(run) && decimal(attempt)
            }),
    }
}

fn validate_authority(
    authority: &ReleaseAuthorizationAuthorityV1,
    context: &ReleaseAuthorizationExpectedContextV1,
    findings: &mut Vec<ReleaseAuthorizationFindingV1>,
) {
    if authority.selected_auth_class != RELEASE_AUTHORIZATION_AUTH_CLASS {
        finding(
            findings,
            ResultState::Unauthorized,
            "selected auth class cannot carry the clean final operation",
        );
    }
    if authority.maintainer_actor.trim().is_empty() || authority.maintainer_role.trim().is_empty() {
        finding(
            findings,
            ResultState::Unauthorized,
            "maintainer actor and role are required",
        );
    }
    if !authority.one_run_scope {
        finding(
            findings,
            ResultState::Unauthorized,
            "authorization must be explicitly scoped to one run",
        );
    }
    if authority.nonce.trim().is_empty() {
        finding(
            findings,
            ResultState::Malformed,
            "one-use nonce is required",
        );
    }
    if authority.created_at_unix_seconds > context.evaluated_at_unix_seconds {
        finding(
            findings,
            ResultState::Malformed,
            "authorization creation is future-dated",
        );
    }
    if authority.expires_at_unix_seconds <= authority.created_at_unix_seconds {
        finding(
            findings,
            ResultState::Malformed,
            "authorization expiry must follow creation",
        );
    } else if context.evaluated_at_unix_seconds > authority.expires_at_unix_seconds {
        finding(
            findings,
            ResultState::Expired,
            "authorization expired before evaluation",
        );
    }

    let source = &authority.source;
    if source.repository != context.repository
        || source.repository != RELEASE_AUTHORIZATION_REPOSITORY
        || source.author != authority.maintainer_actor
        || !digest(&source.body_digest)
        || !valid_source_reference(source.kind, &source.reference)
    {
        finding(
            findings,
            ResultState::Unauthorized,
            "structured source identity is not bound to the selected repository, actor, and object",
        );
    }
    if source.statement != RELEASE_AUTHORIZATION_EXACT_STATEMENT {
        finding(
            findings,
            ResultState::Unauthorized,
            "maintainer statement does not exactly select the final operation and tag",
        );
    }
}

fn validate_use_observation(
    authority: &ReleaseAuthorizationAuthorityV1,
    observation: &ReleaseAuthorizationUseObservationV1,
    findings: &mut Vec<ReleaseAuthorizationFindingV1>,
) {
    let mut seen = BTreeSet::new();
    for nonce in &observation.consumed_nonces {
        if nonce.trim().is_empty() || !seen.insert(nonce) {
            finding(
                findings,
                ResultState::InstrumentFailure,
                "authorization-use observation contains an empty or duplicate nonce",
            );
        }
    }
    if observation
        .consumed_nonces
        .iter()
        .any(|nonce| nonce == &authority.nonce)
    {
        finding(
            findings,
            ResultState::Reused,
            "authorization nonce was already consumed",
        );
    }
    use ReleaseAuthorizationConsumptionV1 as Consumption;
    match observation.state {
        Consumption::Available => {}
        Consumption::SelectedForRun
        | Consumption::IrreversibleOperationStarted
        | Consumption::ConsumedComplete
        | Consumption::ConsumedIncident => finding(
            findings,
            ResultState::Reused,
            "authorization was already selected or consumed",
        ),
        Consumption::Expired => finding(
            findings,
            ResultState::Expired,
            "authorization-use observation is expired",
        ),
        Consumption::Revoked => finding(
            findings,
            ResultState::Unauthorized,
            "authorization-use observation is revoked",
        ),
    }
}

/// Pure two-sided compilation. No adapter is invoked, no credential is read,
/// no tag is created, and no supplied evidence is authenticated by this
/// function. Callers must assemble `expected` from trusted retained objects and
/// current readbacks independently of `input`.
pub fn compile_release_authorization_v1(
    input: &ReleaseAuthorizationInputV1,
    expected: &ReleaseAuthorizationExpectedContextV1,
) -> CargoAllowReleaseAuthorizationV1 {
    let mut findings = Vec::new();
    let mut caveats = Vec::new();

    if input.schema_id != RELEASE_AUTHORIZATION_SCHEMA_ID
        || input.schema_version != RELEASE_AUTHORIZATION_SCHEMA_VERSION
    {
        finding(
            &mut findings,
            ResultState::Unsupported,
            "non-current authorization decision generation",
        );
    }
    if expected.schema_id != RELEASE_AUTHORIZATION_EXPECTED_CONTEXT_SCHEMA_ID
        || expected.schema_version != RELEASE_AUTHORIZATION_EXPECTED_CONTEXT_SCHEMA_VERSION
    {
        finding(
            &mut findings,
            ResultState::Unsupported,
            "non-current expected-context generation",
        );
    }
    if expected.repository != RELEASE_AUTHORIZATION_REPOSITORY {
        finding(
            &mut findings,
            ResultState::InstrumentFailure,
            "trusted context names the wrong repository",
        );
    }

    validate_operation(&input.operation, &mut findings);
    validate_freeze(
        &input.freeze,
        "authorization",
        ResultState::Malformed,
        ResultState::Mismatch,
        &mut findings,
    );
    validate_freeze(
        &expected.freeze,
        "trusted context",
        ResultState::InstrumentFailure,
        ResultState::InstrumentFailure,
        &mut findings,
    );
    if input.freeze != expected.freeze {
        finding(
            &mut findings,
            ResultState::Mismatch,
            "authorization freeze differs from the independently retained freeze",
        );
    }

    validate_evidence_shape(
        &input.evidence,
        "authorization",
        ResultState::Malformed,
        &mut findings,
    );
    validate_evidence_shape(
        &expected.evidence,
        "trusted context",
        ResultState::InstrumentFailure,
        &mut findings,
    );
    validate_expected_evidence(
        &expected.evidence,
        expected.evaluated_at_unix_seconds,
        &mut findings,
        &mut caveats,
    );
    if input.evidence != expected.evidence {
        finding(
            &mut findings,
            ResultState::Mismatch,
            "authorization evidence differs from the independently retained evidence",
        );
    }

    validate_authority(&input.authority, expected, &mut findings);

    if !expected.secret_availability.redacted {
        finding(
            &mut findings,
            ResultState::InstrumentFailure,
            "trusted secret-availability observation retained secret material",
        );
    }
    if expected.secret_availability.state == ReleaseAuthorizationSecretStateV1::Unknown {
        caveats.push(
            "repository-secret availability is unproven; the token boundary must recheck it"
                .to_string(),
        );
    }
    validate_use_observation(
        &input.authority,
        &expected.use_observation,
        &mut findings,
    );

    let authorization_digest = match authorization_statement_digest(input) {
        Ok(value) => value,
        Err(error) => {
            finding(
                &mut findings,
                ResultState::InstrumentFailure,
                format!("authorization digest serialization failed: {error}"),
            );
            String::new()
        }
    };
    let context_digest = match expected_context_digest(expected) {
        Ok(value) => value,
        Err(error) => {
            finding(
                &mut findings,
                ResultState::InstrumentFailure,
                format!("expected-context digest serialization failed: {error}"),
            );
            String::new()
        }
    };

    let mut inventory = BTreeSet::new();
    for entry in &expected.frozen_file_digests {
        if !digest(entry) || !inventory.insert(entry.to_ascii_lowercase()) {
            finding(
                &mut findings,
                ResultState::InstrumentFailure,
                "trusted frozen-file inventory contains a malformed or duplicate digest",
            );
        }
    }
    if !authorization_digest.is_empty()
        && expected
            .frozen_file_digests
            .iter()
            .any(|entry| entry.eq_ignore_ascii_case(&authorization_digest))
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
        expected_context_digest: context_digest,
        evaluated_at_unix_seconds: expected.evaluated_at_unix_seconds,
        claim_boundary: CLAIM_BOUNDARY.to_string(),
    }
}
