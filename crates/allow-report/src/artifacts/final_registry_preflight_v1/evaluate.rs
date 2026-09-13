use super::*;
use crate::artifacts::package_candidate_v2::{
    PackageCandidateFamilyV2, PackageCandidatePayloadV2, PackageCandidateResultV2,
    validate_package_candidate_v2,
};

use FinalRegistryPreflightResultV1 as ResultState;

// This generation owns the final 0.2 denominator, not arbitrary future releases.
const SELECTION: [(&str, &str, u32); 13] = [
    ("allow-core", "allow-core", 10),
    ("allow-policy", "allow-policy", 20),
    ("allow-inventory", "allow-inventory", 30),
    ("allow-files", "allow-files", 40),
    ("allow-rust", "allow-rust", 50),
    ("allow-match", "allow-match", 60),
    ("allow-report", "allow-report", 70),
    ("allow-policy-legacy", "allow-policy-legacy", 75),
    ("repo-protocol", "effortless-repo-protocol", 80),
    ("repo-snapshot", "effortless-repo-snapshot", 85),
    ("repo-edit", "effortless-repo-edit", 90),
    ("allow-diff", "allow-diff", 95),
    ("cargo-allow", "cargo-allow", 100),
];

fn finding(
    findings: &mut Vec<FinalRegistryPreflightFindingV1>,
    result: ResultState,
    reason: impl Into<String>,
) {
    findings.push(FinalRegistryPreflightFindingV1 {
        result,
        reason: reason.into(),
    });
}

fn digest(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn content_digest<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    let bytes = serde_json::to_vec(value)?;
    Ok(allow_core::sha256_v1_bytes(&bytes).replacen("sha256:v1:", "sha256:", 1))
}

/// Compute bindings from the actual candidate and retained shared authorities.
/// These bind inputs, not their authenticity. Validation still belongs to the evaluator.
pub fn final_registry_bindings_v1(
    candidate: &PackageCandidatePayloadV2,
    shared: &[FinalRegistrySharedAuthorityV1],
) -> Result<(String, String), serde_json::Error> {
    Ok((
        content_digest(candidate)?,
        content_digest(&(candidate.rows.as_slice(), shared))?,
    ))
}

fn context_fields(context: &FinalRegistryContextV1) -> [(&'static str, &str); 8] {
    [
        ("candidate_digest", &context.candidate_digest),
        ("denominator_digest", &context.denominator_digest),
        ("workflow_digest", &context.workflow_digest),
        ("principal", &context.principal),
        ("environment", &context.environment),
        ("owner_team_digest", &context.owner_team_digest),
        ("release_controls_digest", &context.release_controls_digest),
        ("provider_state_digest", &context.provider_state_digest),
    ]
}

/// Pure reconciliation. No adapter is invoked and no supplied evidence is authenticated.
pub fn evaluate_final_registry_preflight_v1(
    input: &FinalRegistryPreflightInputV1,
) -> CargoAllowFinalRegistryPreflightV1 {
    let mut findings = Vec::new();
    if input.schema_id != FINAL_REGISTRY_PREFLIGHT_SCHEMA_ID
        || input.schema_version != FINAL_REGISTRY_PREFLIGHT_SCHEMA_VERSION
    {
        finding(
            &mut findings,
            ResultState::UnsupportedGeneration,
            "non-current preflight generation",
        );
    }
    let validation = validate_package_candidate_v2(&input.candidate);
    let state = if validation.result == PackageCandidateResultV2::UnsupportedGeneration {
        ResultState::UnsupportedGeneration
    } else {
        ResultState::Malformed
    };
    for gap in validation.gaps {
        finding(&mut findings, state, format!("candidate: {gap}"));
    }
    if input.candidate.topology_id != "CARGO-ALLOW-PKG-TOPOLOGY-V2-0001"
        || input.candidate.candidate_product_id != "cargo-allow-0.2"
        || input.candidate.root_logical_id != "cargo-allow"
        || input.candidate.root_package_name != "cargo-allow"
        || input.candidate.root_package_version != "0.2.0"
    {
        finding(
            &mut findings,
            ResultState::Malformed,
            "candidate topology/product/root is not the selected final cargo-allow 0.2.0 generation",
        );
    }
    if input.candidate.rows.len() != SELECTION.len()
        || input.observations.len() != SELECTION.len()
        || input.shared_authorities.len() != 3
    {
        finding(
            &mut findings,
            ResultState::Malformed,
            "expected exactly 13 candidate/observation rows and 3 shared authorities",
        );
    }
    for ((name, observed), (_, current)) in context_fields(&input.observed_context)
        .into_iter()
        .zip(context_fields(&input.current_context))
    {
        if observed.trim().is_empty()
            || current.trim().is_empty()
            || (name.ends_with("digest") && (!digest(observed) || !digest(current)))
        {
            finding(
                &mut findings,
                ResultState::Malformed,
                format!("invalid context {name}"),
            );
        }
        if observed != current {
            finding(
                &mut findings,
                ResultState::Stale,
                format!("context moved: {name}"),
            );
        }
    }
    match final_registry_bindings_v1(&input.candidate, &input.shared_authorities) {
        Ok((candidate, denominator)) => {
            if candidate != input.current_context.candidate_digest {
                finding(
                    &mut findings,
                    ResultState::Stale,
                    "candidate content does not match current binding",
                );
            }
            if denominator != input.current_context.denominator_digest {
                finding(
                    &mut findings,
                    ResultState::Stale,
                    "selected denominator/authorities do not match current binding",
                );
            }
        }
        Err(error) => finding(
            &mut findings,
            ResultState::InstrumentFailure,
            format!("binding serialization: {error}"),
        ),
    }
    if input.maximum_age_seconds == 0 {
        finding(
            &mut findings,
            ResultState::Malformed,
            "freshness window must be positive",
        );
    }
    let mut upload_rows = Vec::new();
    let mut shared_prerequisites = Vec::new();
    let mut shared_index = 0;
    for (index, candidate) in input.candidate.rows.iter().enumerate() {
        let shared = candidate.product_family == PackageCandidateFamilyV2::Shared01;
        let mut row_findings = Vec::new();
        if SELECTION
            .get(index)
            .is_none_or(|(logical, package, order)| {
                candidate.logical_id != *logical
                    || candidate.cargo_package_name != *package
                    || candidate.release_order != *order
            })
            || candidate.cargo_package_version != if shared { "0.1.0" } else { "0.2.0" }
            || shared != candidate.cargo_package_name.starts_with("effortless-repo-")
            || !candidate.publish
        {
            finding(
                &mut row_findings,
                ResultState::Malformed,
                "row differs from final generation selection/order",
            );
        }
        let local = candidate.crate_digest.clone();
        let (expected_checksum, checksum_authority_digest) = if shared {
            let authority = input.shared_authorities.get(shared_index);
            shared_index += 1;
            match authority {
                Some(authority)
                    if authority.package_name == candidate.cargo_package_name
                        && authority.package_version == candidate.cargo_package_version =>
                {
                    (
                        authority.expected_checksum.clone(),
                        authority.authority_digest.clone(),
                    )
                }
                _ => {
                    finding(
                        &mut row_findings,
                        ResultState::Malformed,
                        "missing/reordered shared checksum authority",
                    );
                    (String::new(), String::new())
                }
            }
        } else {
            (
                local.clone().unwrap_or_default(),
                input.current_context.candidate_digest.clone(),
            )
        };
        if !digest(&expected_checksum) || !digest(&checksum_authority_digest) {
            finding(
                &mut row_findings,
                ResultState::Malformed,
                "expected checksum or its authority is malformed/missing",
            );
        }
        let expected = FinalRegistryExpectedRowV1 {
            logical_id: candidate.logical_id.clone(),
            package_name: candidate.cargo_package_name.clone(),
            package_version: candidate.cargo_package_version.clone(),
            release_order: candidate.release_order,
            role: if shared {
                FinalRegistryRowRoleV1::SharedPrerequisite
            } else {
                FinalRegistryRowRoleV1::FinalUploadCandidate
            },
            expected_checksum,
            checksum_authority_digest,
            diagnostic_local_checksum: if shared { local } else { None },
        };
        let observation = input.observations.get(index).cloned();
        let version_state = match &observation {
            Some(observation) => {
                reconcile_observation(&expected, observation, input, &mut row_findings)
            }
            None => {
                finding(
                    &mut row_findings,
                    ResultState::Malformed,
                    "missing observation",
                );
                FinalRegistryVersionStateV1::Unknown
            }
        };
        let result = row_findings
            .iter()
            .chain(&findings)
            .map(|finding| finding.result)
            .min()
            .unwrap_or(ResultState::Complete);
        let next_action = next_action(result, version_state, shared);
        let row = FinalRegistryPreflightRowV1 {
            expected,
            observation,
            version_state,
            findings: row_findings,
            next_action,
        };
        if shared {
            shared_prerequisites.push(row);
        } else {
            upload_rows.push(row);
        }
    }
    let result = findings
        .iter()
        .chain(
            upload_rows
                .iter()
                .chain(&shared_prerequisites)
                .flat_map(|row| &row.findings),
        )
        .map(|finding| finding.result)
        .min()
        .unwrap_or(ResultState::Complete);
    CargoAllowFinalRegistryPreflightV1 {
        schema_id: FINAL_REGISTRY_PREFLIGHT_SCHEMA_ID.to_string(),
        schema_version: FINAL_REGISTRY_PREFLIGHT_SCHEMA_VERSION,
        observed_context: input.observed_context.clone(),
        current_context: input.current_context.clone(),
        evaluated_at_unix_seconds: input.evaluated_at_unix_seconds,
        maximum_age_seconds: input.maximum_age_seconds,
        result,
        findings,
        upload_rows,
        shared_prerequisites,
    }
}

fn provenance_valid(
    provenance: Option<&FinalRegistryProvenanceV1>,
    dimension: &str,
    input: &FinalRegistryPreflightInputV1,
    findings: &mut Vec<FinalRegistryPreflightFindingV1>,
) -> bool {
    let Some(provenance) = provenance else {
        finding(
            findings,
            ResultState::Malformed,
            format!("missing {dimension} provenance"),
        );
        return false;
    };
    let shape_valid = !provenance.provider.trim().is_empty()
        && !provenance.source.trim().is_empty()
        && digest(&provenance.evidence_digest);
    if !shape_valid {
        finding(
            findings,
            ResultState::Malformed,
            format!("malformed {dimension} provenance"),
        );
    }
    match input
        .evaluated_at_unix_seconds
        .checked_sub(provenance.observed_at_unix_seconds)
    {
        Some(age) if input.maximum_age_seconds > 0 && age <= input.maximum_age_seconds => {
            shape_valid
        }
        _ => {
            finding(
                findings,
                ResultState::Stale,
                format!("{dimension} observation is future-dated or expired"),
            );
            false
        }
    }
}

fn reconcile_observation(
    expected: &FinalRegistryExpectedRowV1,
    observation: &FinalRegistryObservationV1,
    input: &FinalRegistryPreflightInputV1,
    findings: &mut Vec<FinalRegistryPreflightFindingV1>,
) -> FinalRegistryVersionStateV1 {
    use FinalRegistryVersionResponseV1 as Response;
    use FinalRegistryVersionStateV1 as Version;
    let identity = observation.package_name == expected.package_name
        && observation.package_version == expected.package_version;
    if !identity {
        finding(
            findings,
            ResultState::Malformed,
            "observation requested identity differs from selected exact version",
        );
    }
    let version_proven = provenance_valid(
        observation.version_provenance.as_ref(),
        "version",
        input,
        findings,
    );
    let mut version = match &observation.version {
        Response::Found { checksum, yanked } => {
            if *yanked {
                finding(
                    findings,
                    ResultState::Conflict,
                    "selected registry version is yanked",
                );
            }
            if !digest(checksum) {
                finding(
                    findings,
                    ResultState::Malformed,
                    "malformed observed checksum",
                );
                Version::Unknown
            } else {
                let exact = checksum == &expected.expected_checksum;
                if !exact {
                    finding(
                        findings,
                        ResultState::Conflict,
                        "immutable registry checksum conflict",
                    );
                }
                if *yanked {
                    Version::Yanked
                } else if exact {
                    Version::AlreadyPublishedExact
                } else {
                    Version::AlreadyPublishedConflict
                }
            }
        }
        Response::Missing {} => {
            if expected.role == FinalRegistryRowRoleV1::SharedPrerequisite {
                finding(
                    findings,
                    ResultState::Incomplete,
                    "shared prerequisite is absent",
                );
            }
            Version::Missing
        }
        Response::NameUnavailable {} => {
            finding(
                findings,
                ResultState::Incomplete,
                "name unavailable does not establish exact-version absence",
            );
            Version::NameUnavailable
        }
        Response::VisibilityPending {} => {
            finding(
                findings,
                ResultState::Incomplete,
                "exact-version visibility pending",
            );
            Version::Unknown
        }
        Response::MalformedResponse {} => {
            finding(
                findings,
                ResultState::InstrumentFailure,
                "malformed provider response",
            );
            Version::Unknown
        }
        Response::Timeout {} | Response::RateLimited {} | Response::ProviderUnavailable {} => {
            finding(
                findings,
                ResultState::ProviderUnavailable,
                "version provider unavailable",
            );
            Version::Unknown
        }
    };
    if !identity || !version_proven {
        version = Version::Unknown;
    }
    provenance_valid(
        observation.owner_provenance.as_ref(),
        "owner",
        input,
        findings,
    );
    match observation.owner {
        FinalRegistryOwnerStateV1::UnexpectedOwner => {
            finding(findings, ResultState::Conflict, "unexpected package owner")
        }
        FinalRegistryOwnerStateV1::ProviderUnavailable => finding(
            findings,
            ResultState::ProviderUnavailable,
            "owner endpoint unavailable",
        ),
        FinalRegistryOwnerStateV1::PermissionNotProven => finding(
            findings,
            ResultState::CompleteWithResidualAuthorityRisk,
            "owner permission remains unproven",
        ),
        FinalRegistryOwnerStateV1::OwnedByExpectedPrincipal => {}
    }
    // Even exact prior publication or membership does not prove current permission.
    provenance_valid(
        observation.authority_provenance.as_ref(),
        "authority",
        input,
        findings,
    );
    match observation.publish_authority {
        FinalRegistryPublishAuthorityV1::Proven => {}
        FinalRegistryPublishAuthorityV1::SupportingEvidenceOnly
        | FinalRegistryPublishAuthorityV1::NotProven => finding(
            findings,
            ResultState::CompleteWithResidualAuthorityRisk,
            "publication authority remains unproven",
        ),
        FinalRegistryPublishAuthorityV1::Conflict => finding(
            findings,
            ResultState::Conflict,
            "publication authority conflict",
        ),
        FinalRegistryPublishAuthorityV1::ProviderUnavailable => finding(
            findings,
            ResultState::ProviderUnavailable,
            "authority provider unavailable",
        ),
        FinalRegistryPublishAuthorityV1::InstrumentFailure => finding(
            findings,
            ResultState::InstrumentFailure,
            "authority instrument failure",
        ),
    }
    version
}

fn next_action(
    result: ResultState,
    version: FinalRegistryVersionStateV1,
    shared: bool,
) -> FinalRegistryNextActionV1 {
    use FinalRegistryNextActionV1 as Action;
    match result {
        ResultState::UnsupportedGeneration
        | ResultState::Malformed
        | ResultState::InstrumentFailure => Action::RepairInput,
        ResultState::Conflict => Action::ResolveConflict,
        ResultState::Stale => Action::RefreshObservation,
        ResultState::ProviderUnavailable => Action::RestoreProvider,
        ResultState::Incomplete if shared && version == FinalRegistryVersionStateV1::Missing => {
            Action::ObtainPrerequisite
        }
        ResultState::Incomplete => Action::AwaitVisibility,
        ResultState::CompleteWithResidualAuthorityRisk => Action::ObtainAuthorityEvidence,
        ResultState::Complete if shared => Action::RetainExactPrerequisite,
        ResultState::Complete => Action::AwaitSeparateAuthorization,
    }
}
