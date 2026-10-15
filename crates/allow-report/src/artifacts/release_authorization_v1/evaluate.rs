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

pub(crate) fn authorization_statement_digest(
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

fn malformed_context_receipt(
    input: &ReleaseAuthorizationInputV1,
    reason: String,
) -> CargoAllowReleaseAuthorizationV1 {
    let authorization_digest = authorization_statement_digest(input).unwrap_or_default();
    CargoAllowReleaseAuthorizationV1 {
        schema_id: RELEASE_AUTHORIZATION_SCHEMA_ID.to_string(),
        schema_version: RELEASE_AUTHORIZATION_SCHEMA_VERSION,
        result: ResultState::Malformed,
        findings: vec![ReleaseAuthorizationFindingV1 {
            result: ResultState::Malformed,
            reason,
        }],
        caveats: Vec::new(),
        authorization_digest,
        expected_context_digest: String::new(),
        evaluated_at_unix_seconds: 0,
        claim_boundary: CLAIM_BOUNDARY.to_string(),
    }
}

/// Pure two-sided compilation from an immutable decision and independently
/// retained expected-context bytes. The context bytes must deserialize into the
/// closed typed contract; the compiler never copies expected values from the
/// decision document.
pub fn compile_release_authorization_v1(
    input: &ReleaseAuthorizationInputV1,
    expected_context_json: &[u8],
) -> CargoAllowReleaseAuthorizationV1 {
    let expected: ReleaseAuthorizationExpectedContextV1 =
        match serde_json::from_slice(expected_context_json) {
            Ok(value) => value,
            Err(error) => {
                return malformed_context_receipt(
                    input,
                    format!("expected-context artifact is malformed: {error}"),
                );
            }
        };
    compile_with_context(input, &expected, true)
}

fn compile_with_context(
    input: &ReleaseAuthorizationInputV1,
    expected: &ReleaseAuthorizationExpectedContextV1,
    require_available: bool,
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
    if require_available {
        validate_use_observation(&input.authority, &expected.use_observation, &mut findings);
    }

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

/// Revalidate an already selected operation without creating fresh authority.
/// The original independently assembled context must still compile as an
/// Available decision, and all non-use eligibility checks run against the
/// current independent context. Mutable custody is replayed from its retained
/// Available birth; neither the record nor its consumed nonce is reset.
/// Provider authentication and byte custody belong to the caller's transport.
pub fn validate_release_authorization_continuation_v1(
    input: &ReleaseAuthorizationInputV1,
    original_expected_json: &[u8],
    current_expected_json: &[u8],
    identity: &crate::CargoAllowReleaseOperationIdentityV1,
    birth: &crate::CargoAllowReleaseAuthorizationCustodyV1,
    custody: &crate::CargoAllowReleaseAuthorizationCustodyV1,
) -> Result<(), &'static str> {
    use crate::{
        AUTHORIZATION_CUSTODY_COMPLETE_REPLAY_RESULTS, AUTHORIZATION_CUSTODY_SCHEMA_ID,
        AUTHORIZATION_CUSTODY_SCHEMA_VERSION, CargoAllowReleaseOperationClassV1,
        authorization_evidence_digest_v1, note_irreversible_start_v1,
        release_operation_identity_digest_v1, select_authorization_for_operation_v1,
        validate_release_operation_identity_v1,
    };
    use ReleaseAuthorizationConsumptionV1 as Consumption;

    let original: ReleaseAuthorizationExpectedContextV1 =
        serde_json::from_slice(original_expected_json)
            .map_err(|_| "original context is malformed")?;
    let current: ReleaseAuthorizationExpectedContextV1 =
        serde_json::from_slice(current_expected_json)
            .map_err(|_| "current context is malformed")?;
    let initial_receipt = compile_with_context(input, &original, true);
    if initial_receipt.result != ResultState::Complete
        || compile_with_context(input, &current, false).result != ResultState::Complete
        || current.evaluated_at_unix_seconds < original.evaluated_at_unix_seconds
        || current.repository != original.repository
        || current.freeze != original.freeze
        || current.evidence != original.evidence
        || current.frozen_file_digests != original.frozen_file_digests
    {
        return Err("continuation requires original eligibility and current independent evidence");
    }
    validate_release_operation_identity_v1(identity)?;
    let operation_digest = release_operation_identity_digest_v1(identity)
        .map_err(|_| "operation identity digest failed")?;
    let evidence_digest = authorization_evidence_digest_v1(&input.evidence)
        .map_err(|_| "authorization evidence digest failed")?;
    if identity.operation_class != CargoAllowReleaseOperationClassV1::CleanFinalPublication
        || identity.authorization_digest != initial_receipt.authorization_digest
        || identity.freeze_digest != input.freeze.receipt_digest
        || identity.custody_digest != birth.mint.candidate_custody_digest
        || identity.replay_digest != birth.mint.replay_digest
        || identity.cargo_lock_digest != input.freeze.lock_digest
        || identity.support_digest != input.evidence.support_digest
        || identity.workflow_digest != input.evidence.workflow_digest
        || identity.action_inventory_digest != input.evidence.action_inventory_digest
        || identity.live_controls_digest != input.evidence.live_controls_digest
        || identity.version != input.operation.version
        || identity.tag != input.operation.tag
        || identity.channel != input.operation.channel
        || identity.github_prerelease != input.operation.github_prerelease
        || !identity.one_run_scope
        || identity.expires_at_unix_seconds > birth.expires_at_unix_seconds
        || current.evaluated_at_unix_seconds > identity.expires_at_unix_seconds
        || birth.schema_id != AUTHORIZATION_CUSTODY_SCHEMA_ID
        || birth.schema_version != AUTHORIZATION_CUSTODY_SCHEMA_VERSION
        || birth.authorization_id.trim().is_empty()
        || birth.authorization_digest != initial_receipt.authorization_digest
        || birth.operation != input.operation
        || birth.freeze != input.freeze
        || birth.evidence_digest != evidence_digest
        || birth.mint.freeze_receipt_digest != input.freeze.receipt_digest
        || !AUTHORIZATION_CUSTODY_COMPLETE_REPLAY_RESULTS
            .contains(&birth.mint.replay_result.as_str())
        || birth.mint.minted_by != input.authority.maintainer_actor
        || birth.mint.minted_at_unix_seconds < input.authority.created_at_unix_seconds
        || birth.mint.minted_at_unix_seconds > original.evaluated_at_unix_seconds
        || birth.storage.retention_expiry_unix_seconds < birth.expires_at_unix_seconds
        || birth.valid_from_unix_seconds < input.authority.created_at_unix_seconds
        || birth.expires_at_unix_seconds > input.authority.expires_at_unix_seconds
        || birth.valid_from_unix_seconds >= birth.expires_at_unix_seconds
        || current.evaluated_at_unix_seconds < birth.valid_from_unix_seconds
        || current.evaluated_at_unix_seconds > birth.expires_at_unix_seconds
        || !birth.one_run_scope
        || !birth.redacted
        || !birth.readback_verified
        || birth
            .readback_digest
            .as_deref()
            .is_none_or(|value| !digest(value))
        || birth.nonce != input.authority.nonce
        || birth.state != Consumption::Available
        || birth.selected_operation_identity_digest.is_some()
        || !birth.consumed_nonces.is_empty()
        || !birth.transitions.is_empty()
        || original.use_observation.state != birth.state
        || original.use_observation.consumed_nonces != birth.consumed_nonces
        || current.use_observation.state != custody.state
        || current.use_observation.consumed_nonces != custody.consumed_nonces
        || custody.selected_operation_identity_digest.as_deref() != Some(operation_digest.as_str())
        || custody.consumed_nonces.as_slice() != [input.authority.nonce.clone()]
        || !matches!(
            custody.state,
            Consumption::SelectedForRun | Consumption::IrreversibleOperationStarted
        )
    {
        return Err("continuation requires exact original custody and the selected live operation");
    }
    if identity.packages.len() != input.freeze.packages.len()
        || !identity
            .packages
            .iter()
            .zip(&input.freeze.packages)
            .all(|(actual, expected)| {
                actual.logical_id == expected.logical_id
                    && actual.package_name == expected.package_name
                    && actual.package_version == expected.package_version
                    && actual.package_digest == expected.package_digest
            })
    {
        return Err("continuation package denominator differs from the authorized freeze");
    }
    let selected = custody
        .transitions
        .first()
        .ok_or("selection transition is missing")?;
    if selected.at_unix_seconds < original.evaluated_at_unix_seconds
        || selected.at_unix_seconds > current.evaluated_at_unix_seconds
    {
        return Err("selection is outside the independently observed context window");
    }
    let mut replayed = birth.clone();
    select_authorization_for_operation_v1(
        identity,
        &mut replayed,
        &input.authority.nonce,
        selected.at_unix_seconds,
        &evidence_digest,
        true,
    )?;
    if custody.state == Consumption::IrreversibleOperationStarted {
        let started = custody
            .transitions
            .get(1)
            .ok_or("start transition is missing")?;
        if started.at_unix_seconds > current.evaluated_at_unix_seconds {
            return Err("irreversible start is future-dated");
        }
        note_irreversible_start_v1(&mut replayed, started.at_unix_seconds)?;
    }
    if &replayed != custody {
        return Err("mutable custody differs from its exact reducer replay");
    }
    Ok(())
}
