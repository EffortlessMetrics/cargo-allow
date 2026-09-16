use std::collections::HashMap;
use std::error::Error;
use std::io;

use allow_report::{
    FINAL_REGISTRY_PREFLIGHT_SCHEMA_ID, FinalRegistryContextV1, FinalRegistryExpectedRowV1,
    FinalRegistryObservationOriginV1, FinalRegistryObservationV1, FinalRegistryOwnerStateV1,
    FinalRegistryPreflightInputV1, FinalRegistryPreflightResultV1, FinalRegistryProvenanceV1,
    FinalRegistryProviderV1, FinalRegistryPublishAuthorityV1, FinalRegistryRowRoleV1,
    FinalRegistrySharedAuthorityV1, FinalRegistryVersionResponseV1, FinalRegistryVersionStateV1,
    PackageCandidatePayloadV2, evaluate_final_registry_preflight_v1, final_registry_bindings_v1,
};

type TestResult = Result<(), Box<dyn Error>>;
use FinalRegistryPreflightResultV1 as State;
use FinalRegistryVersionResponseV1 as Response;
use FinalRegistryVersionStateV1 as Version;

/// Final denominator in evaluator order:
/// (logical_id, package_name, version, release_order, shared).
const DENOMINATOR: [(&str, &str, &str, u32, bool); 13] = [
    ("allow-core", "allow-core", "0.2.0", 10, false),
    ("allow-policy", "allow-policy", "0.2.0", 20, false),
    ("allow-inventory", "allow-inventory", "0.2.0", 30, false),
    ("allow-files", "allow-files", "0.2.0", 40, false),
    ("allow-rust", "allow-rust", "0.2.0", 50, false),
    ("allow-match", "allow-match", "0.2.0", 60, false),
    ("allow-report", "allow-report", "0.2.0", 70, false),
    (
        "allow-policy-legacy",
        "allow-policy-legacy",
        "0.2.0",
        75,
        false,
    ),
    (
        "repo-protocol",
        "effortless-repo-protocol",
        "0.1.0",
        80,
        true,
    ),
    (
        "repo-snapshot",
        "effortless-repo-snapshot",
        "0.1.0",
        85,
        true,
    ),
    ("repo-edit", "effortless-repo-edit", "0.1.0", 90, true),
    ("allow-diff", "allow-diff", "0.2.0", 95, false),
    ("cargo-allow", "cargo-allow", "0.2.0", 100, false),
];

fn digest(n: u64) -> String {
    format!("sha256:{n:064x}")
}

fn require(ok: bool, message: impl Into<String>) -> TestResult {
    if ok {
        Ok(())
    } else {
        Err(io::Error::other(message.into()).into())
    }
}

#[derive(Clone)]
struct Script {
    version: Response,
    owner: FinalRegistryOwnerStateV1,
    authority: FinalRegistryPublishAuthorityV1,
    /// False models a copied checksum without provider evidence.
    provenance: bool,
}

/// Deterministic stand-in for the #3850 external adapter: every observation
/// traverses the production `FinalRegistryProviderV1` boundary, never the
/// evaluator's internals. No network access, no credentials.
struct StubProvider {
    scripts: HashMap<(String, String), Script>,
    default_final: Script,
    default_shared: Script,
}

impl StubProvider {
    fn missing() -> Script {
        Script {
            version: Response::Missing {},
            owner: FinalRegistryOwnerStateV1::PermissionNotProven,
            authority: FinalRegistryPublishAuthorityV1::NotProven,
            provenance: true,
        }
    }

    fn shared_exact() -> Script {
        Script {
            // The harness pins the observed checksum to the retained
            // namespace authority after observation; conflict cases override
            // this script entry instead.
            version: Response::Found {
                checksum: digest(1),
                yanked: false,
            },
            owner: FinalRegistryOwnerStateV1::OwnedByExpectedPrincipal,
            authority: FinalRegistryPublishAuthorityV1::NotProven,
            provenance: true,
        }
    }

    fn script_for(&self, package_name: &str, package_version: &str) -> &Script {
        self.scripts
            .get(&(package_name.to_string(), package_version.to_string()))
            .unwrap_or(if package_name.starts_with("effortless-") {
                &self.default_shared
            } else {
                &self.default_final
            })
    }
}

impl FinalRegistryProviderV1 for StubProvider {
    fn observe(&self, row: &FinalRegistryExpectedRowV1) -> FinalRegistryObservationV1 {
        let script = self.script_for(&row.package_name, &row.package_version);
        let provenance = script.provenance.then(|| FinalRegistryProvenanceV1 {
            origin: FinalRegistryObservationOriginV1::TestFixture,
            provider: "stub-provider".to_string(),
            source: "stub://final-registry-provider".to_string(),
            evidence_digest: digest(900),
            observed_at_unix_seconds: 100,
        });
        FinalRegistryObservationV1 {
            package_name: row.package_name.clone(),
            package_version: row.package_version.clone(),
            version: script.version.clone(),
            version_provenance: provenance.clone(),
            owner: script.owner,
            owner_provenance: provenance.clone(),
            publish_authority: script.authority,
            authority_provenance: provenance,
        }
    }
}

fn candidate_rows() -> Result<serde_json::Value, Box<dyn Error>> {
    let mut rows = Vec::new();
    for (logical, package, version, order, shared) in DENOMINATOR {
        rows.push(serde_json::json!({
            "logical_id": logical, "cargo_package_name": package,
            "cargo_package_version": version,
            "rust_library_name": logical.replace('-', "_"),
            "workspace_source_path": format!("crates/{logical}"),
            "product_family": if shared { "shared-0.1" } else { "cargo-allow-0.2" },
            "publication_state": "UnpublishedInternal", "publish": true,
            "support_tier": "supported", "release_order": order,
            "selected_features": [],
            "expected_manifest_identity": format!("{package}:{version}"),
            "expected_dependency_rows": [], "required_assets": [],
            "crate_digest": digest(u64::from(order)),
            "crate_size_bytes": 100,
        }));
    }
    Ok(serde_json::Value::Array(rows))
}

fn harness(
    scripts: HashMap<(String, String), Script>,
) -> Result<(FinalRegistryPreflightInputV1, StubProvider), Box<dyn Error>> {
    let rows = candidate_rows()?;
    let candidate: PackageCandidatePayloadV2 = serde_json::from_value(serde_json::json!({
        "schema_id": "cargo-allow.package-candidate.v2", "schema_version": 2,
        "topology_id": "CARGO-ALLOW-PKG-TOPOLOGY-V2-0001",
        "repository_commit": "fixture", "repository_tree": "fixture",
        "cargo_lock_digest": digest(3), "candidate_product_id": "cargo-allow-0.2",
        "root_logical_id": "cargo-allow", "root_package_name": "cargo-allow",
        "root_package_version": "0.2.0", "target_class": "fixture",
        "feature_set_id": "default", "rows": rows,
        "known_exclusions": [], "limitations": ["provider stub only"],
        "claim_boundary": "stub provider integration only",
    }))?;
    let shared_authorities: Vec<FinalRegistrySharedAuthorityV1> = DENOMINATOR
        .iter()
        .filter(|row| row.4)
        .map(|row| FinalRegistrySharedAuthorityV1 {
            package_name: row.1.to_string(),
            package_version: row.2.to_string(),
            expected_checksum: digest(u64::from(row.3) + 1),
            authority_digest: digest(700),
        })
        .collect();
    let provider = StubProvider {
        scripts,
        default_final: StubProvider::missing(),
        default_shared: StubProvider::shared_exact(),
    };
    // Route every observation through the provider trait, in denominator
    // order, exactly as the external adapter must.
    let mut expected_rows = Vec::new();
    for (logical, package, version, order, shared) in DENOMINATOR {
        expected_rows.push(FinalRegistryExpectedRowV1 {
            logical_id: logical.to_string(),
            package_name: package.to_string(),
            package_version: version.to_string(),
            release_order: order,
            role: if shared {
                FinalRegistryRowRoleV1::SharedPrerequisite
            } else {
                FinalRegistryRowRoleV1::FinalUploadCandidate
            },
            expected_checksum: String::new(),
            checksum_authority_digest: String::new(),
            diagnostic_local_checksum: None,
        });
    }
    let mut observations = Vec::new();
    for expected in &expected_rows {
        observations.push(provider.observe(expected));
    }
    // Shared exactness is bound to retained namespace authority (#3744), so
    // the harness pins exact shared observations to the same authority the
    // input retains; conflict cases override the script instead.
    for observation in &mut observations {
        if observation.package_name.starts_with("effortless-")
            && matches!(observation.version, Response::Found { yanked: false, .. })
        {
            let authority = shared_authorities
                .iter()
                .find(|authority| authority.package_name == observation.package_name)
                .ok_or_else(|| io::Error::other("shared authority absent"))?;
            observation.version = Response::Found {
                checksum: authority.expected_checksum.clone(),
                yanked: false,
            };
        }
    }
    let (candidate_digest, denominator_digest) =
        final_registry_bindings_v1(&candidate, &shared_authorities)?;
    let context = FinalRegistryContextV1 {
        candidate_digest,
        denominator_digest,
        workflow_digest: digest(4),
        principal: "fixture".to_string(),
        environment: "fixture".to_string(),
        owner_team_digest: digest(5),
        release_controls_digest: digest(6),
        provider_state_digest: digest(7),
    };
    let input = FinalRegistryPreflightInputV1 {
        schema_id: FINAL_REGISTRY_PREFLIGHT_SCHEMA_ID.to_string(),
        schema_version: 1,
        candidate,
        shared_authorities,
        observations,
        observed_context: context.clone(),
        current_context: context,
        evaluated_at_unix_seconds: 105,
        maximum_age_seconds: 10,
    };
    Ok((input, provider))
}

fn key(name: &str, version: &str) -> (String, String) {
    (name.to_string(), version.to_string())
}

#[test]
fn final_registry_preflight_provider_missing_final_exact_shared_is_residual_risk() -> TestResult {
    let (input, _) = harness(HashMap::new())?;
    let receipt = evaluate_final_registry_preflight_v1(&input);
    require(
        receipt.result == State::CompleteWithResidualAuthorityRisk,
        format!("expected residual authority risk: {receipt:?}"),
    )?;
    require(
        receipt.upload_rows.len() == 10 && receipt.shared_prerequisites.len() == 3,
        "wrong row roles/counts",
    )?;
    require(
        receipt.surplus_observations.is_empty(),
        "valid input must render no surplus",
    )?;
    require(
        receipt
            .upload_rows
            .iter()
            .all(|row| row.version_state == Version::Missing),
        "final rows must stay missing, never invented",
    )?;
    Ok(())
}

#[test]
fn final_registry_preflight_provider_checksum_conflict_is_conflict() -> TestResult {
    let mut scripts = HashMap::new();
    scripts.insert(
        key("allow-core", "0.2.0"),
        Script {
            version: Response::Found {
                // Valid shape but unequal to the candidate checksum.
                checksum: digest(999),
                yanked: false,
            },
            owner: FinalRegistryOwnerStateV1::OwnedByExpectedPrincipal,
            authority: FinalRegistryPublishAuthorityV1::NotProven,
            provenance: true,
        },
    );
    let (input, _) = harness(scripts)?;
    let receipt = evaluate_final_registry_preflight_v1(&input);
    require(
        receipt.result == State::Conflict,
        format!("expected immutable conflict: {receipt:?}"),
    )?;
    let row = receipt
        .upload_rows
        .first()
        .ok_or_else(|| io::Error::other("upload row absent"))?;
    require(
        row.version_state == Version::AlreadyPublishedConflict,
        format!("expected conflict state: {row:?}"),
    )?;
    Ok(())
}

#[test]
fn final_registry_preflight_provider_yanked_exact_is_conflict() -> TestResult {
    let mut scripts = HashMap::new();
    scripts.insert(
        key("cargo-allow", "0.2.0"),
        Script {
            version: Response::Found {
                checksum: digest(100),
                yanked: true,
            },
            owner: FinalRegistryOwnerStateV1::OwnedByExpectedPrincipal,
            authority: FinalRegistryPublishAuthorityV1::NotProven,
            provenance: true,
        },
    );
    let (input, _) = harness(scripts)?;
    let receipt = evaluate_final_registry_preflight_v1(&input);
    require(
        receipt.result == State::Conflict,
        format!("expected yank conflict: {receipt:?}"),
    )?;
    let row = receipt
        .upload_rows
        .last()
        .ok_or_else(|| io::Error::other("upload row absent"))?;
    require(
        row.version_state == Version::Yanked,
        format!("expected yanked state: {row:?}"),
    )?;
    Ok(())
}

#[test]
fn final_registry_preflight_provider_errors_never_become_absence() -> TestResult {
    for (response, failure) in [
        (Response::Timeout {}, State::ProviderUnavailable),
        (Response::RateLimited {}, State::ProviderUnavailable),
        (Response::ProviderUnavailable {}, State::ProviderUnavailable),
        (Response::MalformedResponse {}, State::InstrumentFailure),
    ] {
        let mut scripts = HashMap::new();
        scripts.insert(
            key("allow-diff", "0.2.0"),
            Script {
                version: response,
                owner: FinalRegistryOwnerStateV1::ProviderUnavailable,
                authority: FinalRegistryPublishAuthorityV1::ProviderUnavailable,
                provenance: true,
            },
        );
        let (input, _) = harness(scripts)?;
        let receipt = evaluate_final_registry_preflight_v1(&input);
        require(
            receipt.result == failure,
            format!("provider failure became clean: {receipt:?}"),
        )?;
        let row = receipt
            .upload_rows
            .iter()
            .find(|row| row.expected.package_name == "allow-diff")
            .ok_or_else(|| io::Error::other("upload row absent"))?;
        require(
            row.version_state == Version::Unknown,
            format!("provider failure established version state: {row:?}"),
        )?;
    }
    Ok(())
}

#[test]
fn final_registry_preflight_provider_name_unavailable_is_not_absence() -> TestResult {
    let mut scripts = HashMap::new();
    scripts.insert(
        key("allow-files", "0.2.0"),
        Script {
            version: Response::NameUnavailable {},
            owner: FinalRegistryOwnerStateV1::PermissionNotProven,
            authority: FinalRegistryPublishAuthorityV1::NotProven,
            provenance: true,
        },
    );
    let (input, _) = harness(scripts)?;
    let receipt = evaluate_final_registry_preflight_v1(&input);
    let row = receipt
        .upload_rows
        .iter()
        .find(|row| row.expected.package_name == "allow-files")
        .ok_or_else(|| io::Error::other("upload row absent"))?;
    require(
        row.version_state == Version::NameUnavailable,
        format!("name unavailability became absence: {row:?}"),
    )?;
    require(
        receipt.result == State::Incomplete,
        format!("expected incomplete: {receipt:?}"),
    )?;
    Ok(())
}

#[test]
fn final_registry_preflight_provider_foreign_identity_is_malformed() -> TestResult {
    let (mut input, _) = harness(HashMap::new())?;
    let observation = input
        .observations
        .first_mut()
        .ok_or_else(|| io::Error::other("observation absent"))?;
    // RC substitution: the requested exact version is replaced by a
    // release-candidate response. It must never describe the final row.
    observation.package_version = "0.2.0-rc.1".to_string();
    let raw = observation.clone();
    let receipt = evaluate_final_registry_preflight_v1(&input);
    require(
        receipt.result == State::Malformed,
        format!("foreign observation became clean: {receipt:?}"),
    )?;
    let row = receipt
        .upload_rows
        .first()
        .ok_or_else(|| io::Error::other("upload row absent"))?;
    require(
        row.observation.as_ref() == Some(&raw),
        "foreign observation was not retained",
    )?;
    require(
        row.version_state == Version::Unknown,
        "foreign observation established version state",
    )?;
    Ok(())
}

#[test]
fn final_registry_preflight_provider_copied_checksum_without_evidence_is_malformed() -> TestResult {
    // Negative control 7: the candidate checksum is copied into the observed
    // field without provider evidence. Equality alone must not establish
    // exact publication.
    let mut scripts = HashMap::new();
    scripts.insert(
        key("allow-core", "0.2.0"),
        Script {
            version: Response::Found {
                checksum: digest(10),
                yanked: false,
            },
            owner: FinalRegistryOwnerStateV1::OwnedByExpectedPrincipal,
            authority: FinalRegistryPublishAuthorityV1::NotProven,
            provenance: false,
        },
    );
    let (input, _) = harness(scripts)?;
    let receipt = evaluate_final_registry_preflight_v1(&input);
    require(
        receipt.result == State::Malformed,
        format!("unevidenced equality became clean: {receipt:?}"),
    )?;
    let row = receipt
        .upload_rows
        .first()
        .ok_or_else(|| io::Error::other("upload row absent"))?;
    require(
        row.version_state == Version::Unknown,
        "unevidenced equality established version state",
    )?;
    Ok(())
}

#[test]
fn final_registry_preflight_provider_owner_failure_preserves_version_evidence() -> TestResult {
    let mut scripts = HashMap::new();
    scripts.insert(
        key("allow-match", "0.2.0"),
        Script {
            version: Response::Found {
                checksum: digest(60),
                yanked: false,
            },
            owner: FinalRegistryOwnerStateV1::ProviderUnavailable,
            authority: FinalRegistryPublishAuthorityV1::NotProven,
            provenance: true,
        },
    );
    let (input, _) = harness(scripts)?;
    let receipt = evaluate_final_registry_preflight_v1(&input);
    let row = receipt
        .upload_rows
        .iter()
        .find(|row| row.expected.package_name == "allow-match")
        .ok_or_else(|| io::Error::other("upload row absent"))?;
    require(
        row.version_state == Version::AlreadyPublishedExact,
        format!("owner failure erased version evidence: {row:?}"),
    )?;
    require(
        receipt.result == State::ProviderUnavailable,
        format!("expected provider-unavailable aggregate: {receipt:?}"),
    )?;
    Ok(())
}

#[test]
fn final_registry_preflight_provider_membership_never_proves_authority() -> TestResult {
    // Owner membership and prior publication stay supporting evidence; they
    // cannot promote authority to Proven, so the ceiling remains residual
    // authority risk.
    let mut scripts = HashMap::new();
    scripts.insert(
        key("allow-rust", "0.2.0"),
        Script {
            version: Response::Missing {},
            owner: FinalRegistryOwnerStateV1::OwnedByExpectedPrincipal,
            authority: FinalRegistryPublishAuthorityV1::SupportingEvidenceOnly,
            provenance: true,
        },
    );
    let (input, _) = harness(scripts)?;
    let receipt = evaluate_final_registry_preflight_v1(&input);
    require(
        receipt.result == State::CompleteWithResidualAuthorityRisk,
        format!("membership became clean permission: {receipt:?}"),
    )?;
    Ok(())
}

#[test]
fn final_registry_preflight_provider_surplus_is_preserved_without_authority() -> TestResult {
    let (mut input, _) = harness(HashMap::new())?;
    let mut surplus = input
        .observations
        .first()
        .ok_or_else(|| io::Error::other("observation absent"))?
        .clone();
    surplus.version = Response::MalformedResponse {};
    input.observations.push(surplus);
    let receipt = evaluate_final_registry_preflight_v1(&input);
    require(
        receipt.surplus_observations.len() == 1,
        "surplus observation was normalized away",
    )?;
    require(
        receipt
            .findings
            .iter()
            .any(|finding| finding.reason.starts_with("surplus observation[13]:")),
        format!("surplus health lacks independent provenance: {receipt:?}"),
    )?;
    Ok(())
}

#[test]
fn final_registry_preflight_provider_omitted_duplicated_reordered_are_malformed() -> TestResult {
    let (input, _) = harness(HashMap::new())?;
    let mut omitted = input.clone();
    omitted.observations.pop();
    require(
        evaluate_final_registry_preflight_v1(&omitted).result == State::Malformed,
        "omitted row was accepted",
    )?;
    let mut duplicated = input.clone();
    let first = duplicated
        .observations
        .first()
        .ok_or_else(|| io::Error::other("observation absent"))?
        .clone();
    duplicated.observations.insert(1, first);
    duplicated.observations.pop();
    require(
        evaluate_final_registry_preflight_v1(&duplicated).result == State::Malformed,
        "duplicated row was accepted",
    )?;
    let mut reordered = input.clone();
    reordered.observations.swap(0, 1);
    require(
        evaluate_final_registry_preflight_v1(&reordered).result == State::Malformed,
        "reordered rows were accepted",
    )?;
    Ok(())
}

#[test]
fn final_registry_preflight_provider_stale_context_is_stale() -> TestResult {
    let (mut input, _) = harness(HashMap::new())?;
    input.current_context.provider_state_digest = digest(701);
    let receipt = evaluate_final_registry_preflight_v1(&input);
    require(
        receipt.result == State::Stale,
        format!("moved provider state stayed current: {receipt:?}"),
    )?;
    Ok(())
}
