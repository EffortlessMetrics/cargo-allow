use super::*;
use crate::{PackageCandidateFamilyV2, PackageCandidatePayloadV2, PackageCandidateRowV2};

type TestResult = Result<(), Box<dyn std::error::Error>>;
use FinalRegistryPreflightResultV1 as State;
use FinalRegistryVersionResponseV1 as Response;
use FinalRegistryVersionStateV1 as Version;

#[test]
fn final_registry_preflight_malformed_provenance_preserves_freshness() -> TestResult {
    for dimension in ["version", "owner", "authority"] {
        for malformed_field in ["provider", "source", "digest"] {
            for observed_at in [99, 111] {
                let mut input = fixture()?;
                let observation = first(&mut input)?;
                let provenance = match dimension {
                    "version" => observation.version_provenance.as_mut(),
                    "owner" => observation.owner_provenance.as_mut(),
                    _ => observation.authority_provenance.as_mut(),
                }
                .ok_or("fixture provenance absent")?;
                match malformed_field {
                    "provider" => provenance.provider.clear(),
                    "source" => provenance.source.clear(),
                    _ => provenance.evidence_digest = "invalid".to_string(),
                }
                provenance.observed_at_unix_seconds = observed_at;
                let receipt = evaluate_final_registry_preflight_v1(&input);
                require(
                    receipt.result == State::Malformed,
                    "malformed precedence changed",
                )?;
                let row = receipt.upload_rows.first().ok_or("row absent")?;
                for (state, reason) in [
                    (
                        State::Malformed,
                        format!("malformed {dimension} provenance"),
                    ),
                    (
                        State::Stale,
                        format!("{dimension} observation is future-dated or expired"),
                    ),
                ] {
                    require(
                        row.findings
                            .iter()
                            .any(|finding| finding.result == state && finding.reason == reason),
                        format!(
                            "missing {state:?} for {dimension}/{malformed_field}/{observed_at}"
                        ),
                    )?;
                }
            }
        }
    }
    Ok(())
}

#[test]
fn final_registry_preflight_malformed_checksum_keeps_independent_yanked_fact() -> TestResult {
    let mut input = fixture()?;
    first(&mut input)?.version = Response::Found {
        checksum: "malformed".to_string(),
        yanked: true,
    };
    let receipt = evaluate_final_registry_preflight_v1(&input);
    require(
        receipt.result == State::Malformed,
        "malformed checksum must stay non-clean",
    )?;
    let row = receipt.upload_rows.first().ok_or("row absent")?;
    require(
        row.findings.iter().any(|finding| {
            finding.result == State::Conflict
                && finding.reason == "selected registry version is yanked"
        }),
        "independent yanked finding was lost",
    )
}

#[test]
fn final_registry_preflight_expired_failure_preserves_each_failure_dimension() -> TestResult {
    for (response, expected, underlying) in [
        (
            Response::Timeout {},
            State::Stale,
            State::ProviderUnavailable,
        ),
        (
            Response::MalformedResponse {},
            State::InstrumentFailure,
            State::InstrumentFailure,
        ),
    ] {
        let mut input = fixture()?;
        input.evaluated_at_unix_seconds = 111;
        first(&mut input)?.version = response;
        let receipt = evaluate_final_registry_preflight_v1(&input);
        require(receipt.result == expected, "failure precedence changed")?;
        let row = receipt.upload_rows.first().ok_or("row absent")?;
        require(
            row.findings
                .iter()
                .any(|finding| finding.result == underlying)
                && row
                    .findings
                    .iter()
                    .any(|finding| finding.result == State::Stale),
            "underlying failure or expiry lost",
        )?;
    }
    Ok(())
}

fn require(condition: bool, message: impl Into<String>) -> TestResult {
    if !condition {
        return Err(std::io::Error::other(message.into()).into());
    }
    Ok(())
}

fn checksum(value: u32) -> String {
    format!("sha256:{value:064x}")
}

fn fixture() -> Result<FinalRegistryPreflightInputV1, Box<dyn std::error::Error>> {
    let identities = [
        ("allow-core", 10),
        ("allow-policy", 20),
        ("allow-inventory", 30),
        ("allow-files", 40),
        ("allow-rust", 50),
        ("allow-match", 60),
        ("allow-report", 70),
        ("allow-policy-legacy", 75),
        ("repo-protocol", 80),
        ("repo-snapshot", 85),
        ("repo-edit", 90),
        ("allow-diff", 95),
        ("cargo-allow", 100),
    ];
    let rows: Vec<_> = identities
        .into_iter()
        .map(|(logical, release_order)| {
            let shared = logical.starts_with("repo-");
            let package = if shared {
                format!("effortless-{logical}")
            } else {
                logical.to_string()
            };
            let version = if shared { "0.1.0" } else { "0.2.0" };
            PackageCandidateRowV2 {
                logical_id: logical.to_string(),
                cargo_package_name: package.clone(),
                cargo_package_version: version.to_string(),
                rust_library_name: logical.replace('-', "_"),
                workspace_source_path: format!("crates/{logical}"),
                product_family: if shared {
                    PackageCandidateFamilyV2::Shared01
                } else {
                    PackageCandidateFamilyV2::CargoAllow02
                },
                publication_state: "UnpublishedInternal".to_string(),
                publish: true,
                support_tier: "supported".to_string(),
                release_order,
                selected_features: Vec::new(),
                expected_manifest_identity: format!("{package}:{version}"),
                expected_dependency_rows: Vec::new(),
                required_assets: Vec::new(),
                crate_digest: Some(checksum(release_order)),
                crate_size_bytes: Some(100),
            }
        })
        .collect();
    let shared_authorities: Vec<_> = rows
        .iter()
        .filter(|row| row.product_family == PackageCandidateFamilyV2::Shared01)
        .map(|row| FinalRegistrySharedAuthorityV1 {
            package_name: row.cargo_package_name.clone(),
            package_version: row.cargo_package_version.clone(),
            expected_checksum: checksum(row.release_order + 1),
            authority_digest: checksum(700),
        })
        .collect();
    let candidate = PackageCandidatePayloadV2 {
        schema_id: "cargo-allow.package-candidate.v2".to_string(),
        schema_version: 2,
        topology_id: "CARGO-ALLOW-PKG-TOPOLOGY-V2-0001".to_string(),
        topology_digest: Some(checksum(2)),
        repository_commit: "fixture-commit".to_string(),
        repository_tree: "fixture-tree".to_string(),
        cargo_lock_digest: checksum(3),
        candidate_product_id: "cargo-allow-0.2".to_string(),
        root_logical_id: "cargo-allow".to_string(),
        root_package_name: "cargo-allow".to_string(),
        root_package_version: "0.2.0".to_string(),
        target_class: "fixture".to_string(),
        feature_set_id: "default".to_string(),
        rows,
        known_exclusions: Vec::new(),
        limitations: vec!["fixture observations only".to_string()],
        claim_boundary: "no registry contact".to_string(),
    };
    let (candidate_digest, denominator_digest) =
        final_registry_bindings_v1(&candidate, &shared_authorities)?;
    let context = FinalRegistryContextV1 {
        candidate_digest,
        denominator_digest,
        workflow_digest: checksum(4),
        principal: "fixture-principal".to_string(),
        environment: "fixture-environment".to_string(),
        owner_team_digest: checksum(5),
        release_controls_digest: checksum(6),
        provider_state_digest: checksum(7),
    };
    let provenance = FinalRegistryProvenanceV1 {
        origin: FinalRegistryObservationOriginV1::TestFixture,
        provider: "deterministic-fixture".to_string(),
        source: "fixture://preflight".to_string(),
        evidence_digest: checksum(8),
        observed_at_unix_seconds: 100,
    };
    let observations = candidate
        .rows
        .iter()
        .map(|row| FinalRegistryObservationV1 {
            package_name: row.cargo_package_name.clone(),
            package_version: row.cargo_package_version.clone(),
            version: if row.product_family == PackageCandidateFamilyV2::Shared01 {
                Response::Found {
                    checksum: checksum(row.release_order + 1),
                    yanked: false,
                }
            } else {
                Response::Missing {}
            },
            version_provenance: Some(provenance.clone()),
            owner: FinalRegistryOwnerStateV1::OwnedByExpectedPrincipal,
            owner_provenance: Some(provenance.clone()),
            publish_authority: FinalRegistryPublishAuthorityV1::NotProven,
            authority_provenance: Some(provenance.clone()),
        })
        .collect();
    Ok(FinalRegistryPreflightInputV1 {
        schema_id: FINAL_REGISTRY_PREFLIGHT_SCHEMA_ID.to_string(),
        schema_version: 1,
        candidate,
        shared_authorities,
        observed_context: context.clone(),
        current_context: context,
        evaluated_at_unix_seconds: 110,
        maximum_age_seconds: 10,
        observations,
    })
}

fn first(
    input: &mut FinalRegistryPreflightInputV1,
) -> Result<&mut FinalRegistryObservationV1, Box<dyn std::error::Error>> {
    input
        .observations
        .first_mut()
        .ok_or_else(|| std::io::Error::other("fixture lacks first observation").into())
}

fn check_result(
    input: &FinalRegistryPreflightInputV1,
    state: State,
    version: Version,
) -> TestResult {
    let receipt = evaluate_final_registry_preflight_v1(input);
    require(
        receipt.result == state,
        format!("expected {state:?}, got {:?}: {receipt:?}", receipt.result),
    )?;
    require(
        receipt
            .upload_rows
            .first()
            .is_some_and(|row| row.version_state == version),
        format!("expected version {version:?}"),
    )
}

#[test]
fn final_registry_preflight_missing_final_exact_shared_is_feasibility_only() -> TestResult {
    let input = fixture()?;
    let receipt = evaluate_final_registry_preflight_v1(&input);
    require(
        receipt.result == State::CompleteWithResidualAuthorityRisk,
        format!("{receipt:?}"),
    )?;
    require(
        receipt.upload_rows.len() == 10 && receipt.shared_prerequisites.len() == 3,
        "wrong row roles/counts",
    )?;
    require(
        receipt
            .upload_rows
            .iter()
            .all(|row| row.version_state == Version::Missing),
        "final row not missing",
    )?;
    require(
        receipt.shared_prerequisites.iter().all(|row| {
            row.version_state == Version::AlreadyPublishedExact
                && row.expected.diagnostic_local_checksum.as_ref()
                    != Some(&row.expected.expected_checksum)
        }),
        "shared local repackaging became authority",
    )?;
    let mut proven = input;
    for observation in &mut proven.observations {
        observation.publish_authority = FinalRegistryPublishAuthorityV1::Proven;
    }
    check_result(&proven, State::Complete, Version::Missing)
}

#[test]
fn final_registry_preflight_exact_conflicting_and_yanked_versions() -> TestResult {
    for (observed, yanked, expected, version) in [
        (
            10,
            false,
            State::CompleteWithResidualAuthorityRisk,
            Version::AlreadyPublishedExact,
        ),
        (
            11,
            false,
            State::Conflict,
            Version::AlreadyPublishedConflict,
        ),
        (10, true, State::Conflict, Version::Yanked),
    ] {
        let mut input = fixture()?;
        first(&mut input)?.version = Response::Found {
            checksum: checksum(observed),
            yanked,
        };
        check_result(&input, expected, version)?;
    }
    Ok(())
}

#[test]
fn final_registry_preflight_rc_visibility_does_not_satisfy_final_query() -> TestResult {
    let mut input = fixture()?;
    first(&mut input)?.version = Response::Found {
        checksum: checksum(10),
        yanked: false,
    };
    first(&mut input)?.package_version = "0.2.0-rc.1".to_string();
    check_result(&input, State::Malformed, Version::Unknown)?;
    first(&mut input)?.package_version = "0.2.0".to_string();
    first(&mut input)?.version = Response::Missing {};
    check_result(
        &input,
        State::CompleteWithResidualAuthorityRisk,
        Version::Missing,
    )
}

#[test]
fn final_registry_preflight_missing_duplicate_reordered_rows_fail_closed() -> TestResult {
    for index in [0, 8] {
        let mut omitted = fixture()?;
        omit(&mut omitted.observations, index)?;
        require(
            evaluate_final_registry_preflight_v1(&omitted).result == State::Malformed,
            "omitted observation accepted",
        )?;
        let mut duplicated = fixture()?;
        let copied = duplicated
            .observations
            .get(index)
            .cloned()
            .ok_or("fixture index absent")?;
        *duplicated
            .observations
            .get_mut(index + 1)
            .ok_or("duplicate target absent")? = copied;
        require(
            evaluate_final_registry_preflight_v1(&duplicated).result == State::Malformed,
            "duplicate observation accepted",
        )?;
        let mut reordered = fixture()?;
        reordered
            .observations
            .get_mut(index..index + 2)
            .ok_or("reorder pair absent")?
            .rotate_left(1);
        require(
            evaluate_final_registry_preflight_v1(&reordered).result == State::Malformed,
            "reordered observation accepted",
        )?;
        let mut candidate = fixture()?;
        omit(&mut candidate.candidate.rows, index)?;
        require(
            evaluate_final_registry_preflight_v1(&candidate).result == State::Malformed,
            "omitted candidate accepted",
        )?;
    }
    let mut input = fixture()?;
    input
        .shared_authorities
        .get_mut(..2)
        .ok_or("authority pair absent")?
        .rotate_left(1);
    require(
        evaluate_final_registry_preflight_v1(&input).result == State::Malformed,
        "reordered shared authorities accepted",
    )
}

fn omit<T>(rows: &mut Vec<T>, index: usize) -> TestResult {
    rows.get(index).ok_or("omission index absent")?;
    *rows = std::mem::take(rows)
        .into_iter()
        .enumerate()
        .filter_map(|(position, row)| (position != index).then_some(row))
        .collect();
    Ok(())
}

#[test]
fn final_registry_preflight_malformed_and_missing_checksum_or_authority() -> TestResult {
    for checksum_value in ["", "sha256:short", "garbage"] {
        let mut observed = fixture()?;
        first(&mut observed)?.version = Response::Found {
            checksum: checksum_value.to_string(),
            yanked: false,
        };
        check_result(&observed, State::Malformed, Version::Unknown)?;
        let mut expected = fixture()?;
        expected
            .candidate
            .rows
            .first_mut()
            .ok_or("fixture candidate absent")?
            .crate_digest = Some(checksum_value.to_string());
        require(
            evaluate_final_registry_preflight_v1(&expected).result == State::Malformed,
            "malformed candidate checksum accepted",
        )?;
        let mut shared = fixture()?;
        shared
            .shared_authorities
            .first_mut()
            .ok_or("fixture authority absent")?
            .authority_digest = checksum_value.to_string();
        require(
            evaluate_final_registry_preflight_v1(&shared).result == State::Malformed,
            "malformed shared authority accepted",
        )?;
    }
    let mut input = fixture()?;
    input
        .candidate
        .rows
        .first_mut()
        .ok_or("fixture candidate absent")?
        .crate_digest = None;
    require(
        evaluate_final_registry_preflight_v1(&input).result == State::Malformed,
        "missing candidate checksum accepted",
    )
}

#[test]
fn final_registry_preflight_provider_failures_remain_distinct() -> TestResult {
    for (response, result, version) in [
        (
            Response::NameUnavailable {},
            State::Incomplete,
            Version::NameUnavailable,
        ),
        (
            Response::Timeout {},
            State::ProviderUnavailable,
            Version::Unknown,
        ),
        (
            Response::RateLimited {},
            State::ProviderUnavailable,
            Version::Unknown,
        ),
        (
            Response::ProviderUnavailable {},
            State::ProviderUnavailable,
            Version::Unknown,
        ),
        (
            Response::VisibilityPending {},
            State::Incomplete,
            Version::Unknown,
        ),
        (
            Response::MalformedResponse {},
            State::InstrumentFailure,
            Version::Unknown,
        ),
    ] {
        let mut input = fixture()?;
        first(&mut input)?.version = response.clone();
        check_result(&input, result, version)?;
        require(
            evaluate_final_registry_preflight_v1(&input)
                .upload_rows
                .first()
                .and_then(|row| row.observation.as_ref())
                .is_some_and(|observation| observation.version == response),
            "provider response was lost",
        )?;
    }
    Ok(())
}

#[test]
fn final_registry_preflight_owner_failure_does_not_erase_exact_version() -> TestResult {
    let mut input = fixture()?;
    first(&mut input)?.version = Response::Found {
        checksum: checksum(10),
        yanked: false,
    };
    first(&mut input)?.owner = FinalRegistryOwnerStateV1::ProviderUnavailable;
    check_result(
        &input,
        State::ProviderUnavailable,
        Version::AlreadyPublishedExact,
    )
}

#[test]
fn final_registry_preflight_membership_and_prior_publication_are_only_supporting() -> TestResult {
    let mut input = fixture()?;
    for observation in &mut input.observations {
        observation.publish_authority = FinalRegistryPublishAuthorityV1::SupportingEvidenceOnly;
    }
    first(&mut input)?.version = Response::Found {
        checksum: checksum(10),
        yanked: false,
    };
    check_result(
        &input,
        State::CompleteWithResidualAuthorityRisk,
        Version::AlreadyPublishedExact,
    )
}

#[test]
fn final_registry_preflight_copied_candidate_without_provider_provenance_is_unknown() -> TestResult
{
    let mut input = fixture()?;
    first(&mut input)?.version = Response::Found {
        checksum: checksum(10),
        yanked: false,
    };
    first(&mut input)?.version_provenance = None;
    check_result(&input, State::Malformed, Version::Unknown)?;
    require(
        serde_json::from_str::<FinalRegistryVersionResponseV1>(
            r#"{"status":"missing","checksum":"copied"}"#,
        )
        .is_err(),
        "contradictory response accepted",
    )
}

#[test]
fn final_registry_preflight_each_context_movement_invalidates_replay() -> TestResult {
    let baseline = fixture()?;
    for field in [
        "candidate_digest",
        "denominator_digest",
        "workflow_digest",
        "principal",
        "environment",
        "owner_team_digest",
        "release_controls_digest",
        "provider_state_digest",
    ] {
        let mut input = baseline.clone();
        let mut context = serde_json::to_value(&input.current_context)?;
        *context.get_mut(field).ok_or("context field absent")? =
            serde_json::Value::String(if field.ends_with("digest") {
                checksum(999)
            } else {
                "moved".to_string()
            });
        input.current_context = serde_json::from_value(context)?;
        require(
            evaluate_final_registry_preflight_v1(&input).result == State::Stale,
            format!("replay accepted after {field} movement"),
        )?;
    }
    let mut moved_bytes = baseline;
    moved_bytes
        .candidate
        .rows
        .first_mut()
        .ok_or("candidate absent")?
        .crate_digest = Some(checksum(999));
    require(
        evaluate_final_registry_preflight_v1(&moved_bytes).result == State::Stale,
        "candidate byte movement accepted under unchanged context",
    )
}

#[test]
fn final_registry_preflight_time_window_boundary_future_expiry_and_refresh() -> TestResult {
    let mut input = fixture()?;
    check_result(
        &input,
        State::CompleteWithResidualAuthorityRisk,
        Version::Missing,
    )?;
    input.evaluated_at_unix_seconds = 111;
    check_result(&input, State::Stale, Version::Unknown)?;
    input.evaluated_at_unix_seconds = 99;
    check_result(&input, State::Stale, Version::Unknown)?;
    input.maximum_age_seconds = 0;
    check_result(&input, State::Malformed, Version::Unknown)?;
    input.maximum_age_seconds = 10;
    input.evaluated_at_unix_seconds = 120;
    input.current_context.principal = "replacement-principal".to_string();
    // New collection replaces all three endpoint observations and evidence identities.
    for observation in &mut input.observations {
        for provenance in [
            &mut observation.version_provenance,
            &mut observation.owner_provenance,
            &mut observation.authority_provenance,
        ] {
            let provenance = provenance.as_mut().ok_or("fixture provenance absent")?;
            provenance.observed_at_unix_seconds = 120;
            provenance.evidence_digest = checksum(120);
        }
    }
    require(
        evaluate_final_registry_preflight_v1(&input).result == State::Stale,
        "new endpoint times erased context movement",
    )?;
    input.observed_context = input.current_context.clone();
    check_result(
        &input,
        State::CompleteWithResidualAuthorityRisk,
        Version::Missing,
    )?;
    Ok(())
}

#[test]
fn final_registry_preflight_authority_claim_requires_independent_dimension_provenance() -> TestResult
{
    let mut input = fixture()?;
    for observation in &mut input.observations {
        observation.publish_authority = FinalRegistryPublishAuthorityV1::Proven;
    }
    first(&mut input)?.owner = FinalRegistryOwnerStateV1::PermissionNotProven;
    check_result(
        &input,
        State::CompleteWithResidualAuthorityRisk,
        Version::Missing,
    )?;
    first(&mut input)?.owner = FinalRegistryOwnerStateV1::OwnedByExpectedPrincipal;
    first(&mut input)?.authority_provenance = None;
    check_result(&input, State::Malformed, Version::Missing)?;
    let mut substituted = fixture()?;
    substituted.candidate.candidate_product_id = "cargo-intent".to_string();
    require(
        evaluate_final_registry_preflight_v1(&substituted).result == State::Malformed,
        "foreign product subject accepted",
    )
}

#[test]
fn final_registry_preflight_foreign_topology_is_rejected_with_fresh_bindings() -> TestResult {
    let mut input = fixture()?;
    input.candidate.topology_id = "FOREIGN-TOPOLOGY".to_string();
    let (candidate_digest, denominator_digest) =
        final_registry_bindings_v1(&input.candidate, &input.shared_authorities)?;
    input.current_context.candidate_digest = candidate_digest;
    input.current_context.denominator_digest = denominator_digest;
    input.observed_context = input.current_context.clone();
    check_result(&input, State::Malformed, Version::Missing)
}

#[test]
fn final_registry_preflight_reuses_exact_internal_dependency_version_validation() -> TestResult {
    let mut input = fixture()?;
    input
        .candidate
        .rows
        .iter_mut()
        .find(|row| row.logical_id == "allow-policy")
        .ok_or("policy row absent")?
        .expected_dependency_rows
        .push(crate::PackageCandidateDependencyRowV2 {
            package_name: "allow-core".to_string(),
            package_version: "0.2.0-rc.1".to_string(),
            dependency_kind: crate::PackageCandidateDependencyKindV2::Internal,
        });
    let (candidate_digest, denominator_digest) =
        final_registry_bindings_v1(&input.candidate, &input.shared_authorities)?;
    input.current_context.candidate_digest = candidate_digest;
    input.current_context.denominator_digest = denominator_digest;
    input.observed_context = input.current_context.clone();
    check_result(&input, State::Malformed, Version::Missing)
}

#[test]
fn final_registry_preflight_canonical_roundtrip_retains_all_negative_dimensions() -> TestResult {
    let mut input = fixture()?;
    first(&mut input)?.version = Response::Found {
        checksum: checksum(999),
        yanked: true,
    };
    first(&mut input)?.owner = FinalRegistryOwnerStateV1::ProviderUnavailable;
    first(&mut input)?.publish_authority = FinalRegistryPublishAuthorityV1::InstrumentFailure;
    let receipt = evaluate_final_registry_preflight_v1(&input);
    require(
        receipt.result == State::InstrumentFailure,
        "severity precedence changed",
    )?;
    let row = receipt.upload_rows.first().ok_or("row absent")?;
    require(
        row.findings
            .iter()
            .any(|finding| finding.result == State::Conflict)
            && row
                .findings
                .iter()
                .any(|finding| finding.result == State::ProviderUnavailable),
        "secondary failures lost",
    )?;
    let json = render_final_registry_preflight_v1(&receipt)?;
    let decoded: CargoAllowFinalRegistryPreflightV1 = serde_json::from_str(&json)?;
    require(
        decoded == receipt && render_final_registry_preflight_v1(&decoded)? == json,
        "noncanonical roundtrip",
    )?;
    require(
        json.contains("test_fixture") && !json.contains("external_provider"),
        "fixture provenance laundered",
    )?;
    input.schema_version = 99;
    require(
        evaluate_final_registry_preflight_v1(&input).result == State::UnsupportedGeneration,
        "unsupported generation accepted",
    )
}

#[test]
fn final_registry_preflight_shared_absence_is_not_upload_feasibility() -> TestResult {
    let mut input = fixture()?;
    input
        .observations
        .iter_mut()
        .find(|row| row.package_name == "effortless-repo-protocol")
        .ok_or("shared observation absent")?
        .version = Response::Missing {};
    require(
        evaluate_final_registry_preflight_v1(&input).result == State::Incomplete,
        "missing shared prerequisite became feasible",
    )
}
