//! Intercepted production final-tag consumer controls for #3930/#3925.
//!
//! The fixture is built by the existing typed freeze, preflight, authorization
//! and custody owners. It contains synthetic bytes and intercepted provider
//! provenance only. It is never release evidence or an authorization act.
//! The Python tests execute the actual compiled CLI for every semantic gate;
//! no Python fixture computes eligibility or reconstructs a push permit.

use allow_core::sha256_v1_bytes;
use allow_report::*;
use serde::Serialize;
use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::Command;

const NOW: u64 = 1_786_000_200;
type TestResult<T = ()> = Result<T, Box<dyn Error>>;

fn require(condition: bool, message: &str) -> Result<(), io::Error> {
    if !condition {
        return Err(io::Error::other(message));
    }
    Ok(())
}

const REPOSITORY: &str = "EffortlessMetrics/cargo-allow";
const COMMIT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const TREE: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const VERSION: &str = "0.2.0";
const TAG: &str = "v0.2.0";
const CUSTODY_ID: &str = "candidate-custody-0.2.0-final";
const RECEIPT_ARTIFACT_ID: &str = "final-freeze-receipt";
const MANIFEST_ARTIFACT_ID: &str = "release-manifest-v2";
const INCIDENT_HANDOFF_ID: &str = "incident-handoff";
const REPLAYED_AT: &str = "2026-08-06T07:10:00Z";

fn digest(seed: u64) -> String {
    format!("sha256:v1:{seed:064x}")
}

fn upload_names() -> Vec<String> {
    [
        "allow-core",
        "allow-policy",
        "allow-policy-legacy",
        "allow-inventory",
        "allow-files",
        "allow-rust",
        "allow-match",
        "allow-report",
        "allow-diff",
        "cargo-allow",
    ]
    .iter()
    .map(|name| (*name).to_string())
    .collect()
}

fn archive_bytes(name: &str) -> Vec<u8> {
    format!("exact-archive-bytes:{name}:{VERSION}").into_bytes()
}

fn archive_digest(name: &str) -> String {
    sha256_v1_bytes(&archive_bytes(name))
}

fn manifest_bytes() -> Vec<u8> {
    br#"{"schema_id":"cargo-allow.release-manifest.v2","synthetic_intercepted_fixture":true,"version":"0.2.0"}"#.to_vec()
}

fn package_rows() -> Vec<FinalEvidencePackageSubjectV1> {
    let mut rows = upload_names()
        .iter()
        .map(|name| FinalEvidencePackageSubjectV1 {
            logical_id: name.clone(),
            package_name: name.clone(),
            version: VERSION.to_string(),
            role: FinalEvidencePackageRoleV1::UploadCandidate,
            expected_digest: archive_digest(name),
            observed_digest: Some(archive_digest(name)),
        })
        .collect::<Vec<_>>();
    for name in [
        "effortless-repo-edit",
        "effortless-repo-protocol",
        "effortless-repo-snapshot",
    ]
    .into_iter()
    {
        rows.push(FinalEvidencePackageSubjectV1 {
            logical_id: name.trim_start_matches("effortless-").to_string(),
            package_name: name.to_string(),
            version: "0.1.0".to_string(),
            role: FinalEvidencePackageRoleV1::ExistingSharedPrerequisite,
            expected_digest: archive_digest(name),
            observed_digest: Some(archive_digest(name)),
        });
    }
    rows
}

fn release_identity() -> FinalEvidenceReleaseIdentityV1 {
    FinalEvidenceReleaseIdentityV1 {
        version: VERSION.to_string(),
        tag: TAG.to_string(),
        github_prerelease: false,
    }
}

fn binding() -> FinalEvidenceSubjectBindingV1 {
    FinalEvidenceSubjectBindingV1 {
        repository: REPOSITORY.to_string(),
        commit: Some(COMMIT.to_string()),
        tree: Some(TREE.to_string()),
        cargo_lock_digest: Some(digest(1)),
        topology_digest: Some(digest(2)),
        release_identity: Some(release_identity()),
        package_rows: Vec::new(),
    }
}

fn node(
    evidence_id: &str,
    class: FinalEvidenceNodeClassV1,
    origin: FinalEvidenceOriginV1,
    authority: FinalEvidenceAuthorityScopeV1,
    required: bool,
) -> FinalEvidenceNodeV1 {
    FinalEvidenceNodeV1 {
        schema_id: "cargo-allow.final-evidence-node.v1".to_string(),
        schema_version: 1,
        evidence_id: evidence_id.to_string(),
        class,
        origin,
        authority_scope: authority,
        required,
        producer: FinalEvidenceProducerV1 {
            producer_id: format!("producer:{evidence_id}"),
            tool: "cargo-allow".to_string(),
            generation: 1,
            identity_digest: digest(9_000),
            workflow_path: Some(".github/workflows/release.yml".to_string()),
            workflow_run_id: Some(101),
            workflow_attempt: Some(1),
            job: Some("303".to_string()),
        },
        producer_expectation: None,
        subject: binding(),
        semantic_digest: digest(3_000),
        expected_semantic_digest: Some(digest(3_000)),
        artifact_digest: Some(digest(4_000)),
        expected_artifact_digest: Some(digest(4_000)),
        result: FinalEvidenceNodeResultV1::Complete,
        currentness: FinalEvidenceCurrentnessV1::Current,
        invalidation_dimensions: vec![FinalEvidenceInvalidationDimensionV1::Source],
        rerun_owner: Some(format!("owner:{evidence_id}")),
        limitations: Vec::new(),
        claim_boundary: format!("Exact bounded evidence for {evidence_id}."),
    }
}

fn edge(from: &str, to: &str, kind: FinalEvidenceEdgeKindV1) -> FinalEvidenceEdgeV1 {
    FinalEvidenceEdgeV1 {
        schema_id: "cargo-allow.final-evidence-edge.v1".to_string(),
        schema_version: 1,
        from: from.to_string(),
        to: to.to_string(),
        kind,
        claim_boundary: format!("{from} supplies the selected {kind:?} relationship to {to}."),
    }
}

fn evidence_graph() -> FinalEvidenceGraphV1 {
    let nodes = vec![
        node(
            "package-archive",
            FinalEvidenceNodeClassV1::PackageArchive,
            FinalEvidenceOriginV1::CandidateBytes,
            FinalEvidenceAuthorityScopeV1::FinalExact,
            true,
        ),
        node(
            "installed-journey",
            FinalEvidenceNodeClassV1::InstalledJourney,
            FinalEvidenceOriginV1::WorkflowArtifact,
            FinalEvidenceAuthorityScopeV1::FinalExact,
            true,
        ),
        node(
            "support-selection",
            FinalEvidenceNodeClassV1::SupportSelection,
            FinalEvidenceOriginV1::SourceAuthority,
            FinalEvidenceAuthorityScopeV1::FinalExact,
            true,
        ),
        node(
            "manifest-result",
            FinalEvidenceNodeClassV1::ManifestResult,
            FinalEvidenceOriginV1::WorkflowArtifact,
            FinalEvidenceAuthorityScopeV1::FinalExact,
            true,
        ),
        node(
            INCIDENT_HANDOFF_ID,
            FinalEvidenceNodeClassV1::IncidentHandoff,
            FinalEvidenceOriginV1::HistoricalObservation,
            FinalEvidenceAuthorityScopeV1::HistoricalIncident,
            false,
        ),
    ];
    let required = nodes
        .iter()
        .filter(|node| node.required)
        .map(|node| node.evidence_id.clone())
        .collect();
    FinalEvidenceGraphV1 {
        schema_id: "cargo-allow.final-evidence-graph.v1".to_string(),
        schema_version: 1,
        mode: FinalEvidenceGraphModeV1::Production,
        repository: REPOSITORY.to_string(),
        selected_subject: FinalEvidenceSelectedSubjectV1 {
            repository: REPOSITORY.to_string(),
            commit: COMMIT.to_string(),
            tree: TREE.to_string(),
            cargo_lock_digest: digest(1),
            topology_digest: digest(2),
            release_identity: release_identity(),
            expected_upload_rows: 10,
            expected_shared_rows: 3,
            package_rows: package_rows(),
        },
        required_node_ids: required,
        nodes,
        edges: vec![
            edge(
                "package-archive",
                "installed-journey",
                FinalEvidenceEdgeKindV1::ProducedFrom,
            ),
            edge(
                "support-selection",
                "installed-journey",
                FinalEvidenceEdgeKindV1::Projects,
            ),
            edge(
                "manifest-result",
                "installed-journey",
                FinalEvidenceEdgeKindV1::ConsumedBy,
            ),
        ],
        limitations: Vec::new(),
        claim_boundary: "Exact final-release evidence fixture.".to_string(),
    }
}

fn freeze_receipt(
    graph: &FinalEvidenceGraphV1,
) -> Result<CargoAllowFinalFreezeReceiptV1, io::Error> {
    let recorded_graph_digest = match allow_report::final_evidence_graph_digest(graph) {
        Ok(value) => value,
        Err(error) => return Err(io::Error::other(error)),
    };
    Ok(CargoAllowFinalFreezeReceiptV1::new(
        FinalFreezeReceiptInitV1 {
            freeze_id: "freeze-0.2.0-final".to_string(),
            frozen_custody_id: CUSTODY_ID.to_string(),
            frozen_at_utc: "2026-08-06T07:09:00Z".to_string(),
            release_identity: release_identity(),
            repository: REPOSITORY.to_string(),
            commit: COMMIT.to_string(),
            tree: TREE.to_string(),
            cargo_lock_digest: digest(1),
            topology_digest: digest(2),
            expected_upload_rows: 10,
            expected_shared_rows: 3,
            package_rows: package_rows(),
            prepublication_manifest: FinalFreezeManifestBindingV1 {
                result: FinalFreezeManifestResultV1::Exact,
                artifact_id: MANIFEST_ARTIFACT_ID.to_string(),
                payload_sha256: sha256_v1_bytes(&manifest_bytes()),
            },
            rc1_excluded: true,
            rc1_version: Some("0.2.0-rc.1".to_string()),
            incident_handoff_id: Some(INCIDENT_HANDOFF_ID.to_string()),
            recorded_graph_digest,
            remaining_irreversible_operations: vec![
                "push tag v0.2.0".to_string(),
                "upload 10 package rows to crates.io".to_string(),
                "publish the GitHub release".to_string(),
            ],
        },
    ))
}

fn custody_item(
    role: &str,
    artifact_id: &str,
    path: &str,
    payload: &[u8],
) -> RetainedCustodyItemV1 {
    let sha256 = sha256_v1_bytes(payload);
    RetainedCustodyItemV1 {
        role: role.to_string(),
        artifact_id: artifact_id.to_string(),
        files: vec![CustodyFileV1 {
            path: path.to_string(),
            size_bytes: payload.len() as u64,
            sha256: sha256.clone(),
        }],
        storage_locator: format!("https://fixture.invalid/custody/{artifact_id}"),
        retention_expiry_utc: "2027-01-01T00:00:00Z".to_string(),
        readback_verified: true,
        readback_sha256: Some(sha256),
        confidentiality_class: ConfidentialityClassV1::Public,
    }
}

fn retained_artifact(role: &str, artifact_id: &str, payload: Vec<u8>) -> RetainedExactArtifactV1 {
    RetainedExactArtifactV1 {
        role: role.to_string(),
        artifact_id: artifact_id.to_string(),
        declared_sha256: sha256_v1_bytes(&payload),
        bytes: RetainedArtifactBytesV1::new(payload),
    }
}

fn transfer_envelope(artifact_id: &str, payload: &[u8]) -> CargoAllowReleaseArtifactTransferV1 {
    CargoAllowReleaseArtifactTransferV1::new(ArtifactTransferInitV1 {
        transfer_id: format!("transfer:{artifact_id}"),
        role: "PackageArchive".to_string(),
        stable_artifact_id: artifact_id.to_string(),
        producer: ProducerIdentityV1 {
            repository: REPOSITORY.to_string(),
            workflow_path: ".github/workflows/release.yml".to_string(),
            git_ref: format!("refs/tags/{TAG}"),
            run_id: 7,
            run_attempt: 1,
            job_id: format!("job:{artifact_id}"),
            commit_sha: COMMIT.to_string(),
            tree_sha: TREE.to_string(),
            release_version: VERSION.to_string(),
            tool_name: "cargo-allow".to_string(),
            schema_id: "cargo-allow.release-artifact-transfer.v1".to_string(),
            producer_generation: 1,
        },
        provider_id: "github-actions".to_string(),
        provider_artifact_name: artifact_id.to_string(),
        files: vec![ArtifactTransferFileV1 {
            path: format!("{artifact_id}.bin"),
            size_bytes: payload.len() as u64,
            sha256: sha256_v1_bytes(payload),
        }],
        semantic_payload_digest: None,
        trust_class: TrustClassV1::TagWorkflow,
        untrusted_input_posture: UntrustedInputPostureV1::StrictByteMatch,
        created_at_utc: "2026-08-06T07:09:00Z".to_string(),
    })
}

/// Build the retained input set around a customized graph, applying an
/// optional receipt customization before every derived digest is computed.
fn fixture_with(
    graph: FinalEvidenceGraphV1,
    customize_receipt: impl FnOnce(&mut CargoAllowFinalFreezeReceiptV1),
) -> Result<CargoAllowFinalFreezeReplayInputsV1, io::Error> {
    let mut receipt = freeze_receipt(&graph)?;
    customize_receipt(&mut receipt);
    let receipt_payload = serde_json::to_vec(&receipt).map_err(io::Error::other)?;

    let mut artifacts = upload_names()
        .iter()
        .map(|name| retained_artifact("PackageArchive", name, archive_bytes(name)))
        .collect::<Vec<_>>();
    artifacts.push(retained_artifact(
        "FreezeReceipt",
        RECEIPT_ARTIFACT_ID,
        receipt_payload.clone(),
    ));
    artifacts.push(retained_artifact(
        "ReleaseManifest",
        MANIFEST_ARTIFACT_ID,
        manifest_bytes(),
    ));

    let retained_transfers = upload_names()
        .iter()
        .map(|name| transfer_envelope(name, &archive_bytes(name)))
        .chain([
            transfer_envelope(RECEIPT_ARTIFACT_ID, &receipt_payload),
            transfer_envelope(MANIFEST_ARTIFACT_ID, &manifest_bytes()),
        ])
        .collect::<Vec<_>>();

    let mut items = upload_names()
        .iter()
        .map(|name| {
            custody_item(
                "PackageArchive",
                name,
                &format!("packages/{name}-{VERSION}.crate"),
                &archive_bytes(name),
            )
        })
        .collect::<Vec<_>>();
    items.push(custody_item(
        "FreezeReceipt",
        RECEIPT_ARTIFACT_ID,
        "candidate-freeze.receipt.json",
        &receipt_payload,
    ));
    items.push(custody_item(
        "ReleaseManifest",
        MANIFEST_ARTIFACT_ID,
        "release-manifest.v2.json",
        &manifest_bytes(),
    ));

    Ok(CargoAllowFinalFreezeReplayInputsV1 {
        custody: CargoAllowFrozenCandidateCustodyV1::new(CandidateCustodyInitV1 {
            custody_id: CUSTODY_ID.to_string(),
            candidate_version: VERSION.to_string(),
            git_commit: COMMIT.to_string(),
            git_tree: TREE.to_string(),
            items,
            created_at_utc: "2026-08-06T07:08:30Z".to_string(),
        }),
        evidence_graph: graph,
        freeze_receipt: receipt,
        retained_transfers,
        retained_artifacts: artifacts,
        observations: vec![
            RefreshableObservationV1 {
                observation_id: "obs:source-live-control".to_string(),
                kind: RefreshableObservationKindV1::SourceLiveControl,
                observed_at_utc: "2026-08-06T07:09:50Z".to_string(),
            },
            RefreshableObservationV1 {
                observation_id: "obs:registry-feasibility".to_string(),
                kind: RefreshableObservationKindV1::RegistryFeasibility,
                observed_at_utc: "2026-08-06T07:09:50Z".to_string(),
            },
            RefreshableObservationV1 {
                observation_id: "obs:ambient-cache".to_string(),
                kind: RefreshableObservationKindV1::AmbientCache,
                observed_at_utc: "2026-08-06T07:09:50Z".to_string(),
            },
        ],
        replayed_at_utc: REPLAYED_AT.to_string(),
    })
}




fn canonical(value: &str) -> String {
    value.replacen("sha256:v1:", "sha256:", 1)
}

fn plain_digest(seed: u32) -> String {
    format!("sha256:{seed:064x}")
}

fn raw_digest(raw: &[u8]) -> String {
    canonical(&sha256_v1_bytes(raw))
}

fn json_bytes<T: Serialize + ?Sized>(value: &T) -> TestResult<Vec<u8>> {
    Ok(serde_json::to_vec(value)?)
}

fn preflight_fixture() -> Result<FinalRegistryPreflightInputV1, Box<dyn std::error::Error>> {
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
                crate_digest: Some(canonical(&archive_digest(&package))),
                crate_size_bytes: Some(archive_bytes(&package).len() as u64),
            }
        })
        .collect();
    let shared_authorities: Vec<_> = rows
        .iter()
        .filter(|row| row.product_family == PackageCandidateFamilyV2::Shared01)
        .map(|row| FinalRegistrySharedAuthorityV1 {
            package_name: row.cargo_package_name.clone(),
            package_version: row.cargo_package_version.clone(),
            expected_checksum: canonical(&archive_digest(&row.cargo_package_name)),
            authority_digest: plain_digest(700),
        })
        .collect();
    let candidate = PackageCandidatePayloadV2 {
        schema_id: "cargo-allow.package-candidate.v2".to_string(),
        schema_version: 2,
        topology_id: "CARGO-ALLOW-PKG-TOPOLOGY-V2-0001".to_string(),
        topology_digest: Some(plain_digest(2)),
        repository_commit: COMMIT.to_string(),
        repository_tree: TREE.to_string(),
        cargo_lock_digest: plain_digest(1),
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
        workflow_digest: plain_digest(4),
        principal: "fixture-principal".to_string(),
        environment: "fixture-environment".to_string(),
        owner_team_digest: plain_digest(5),
        release_controls_digest: plain_digest(6),
        provider_state_digest: plain_digest(7),
    };
    let provenance = FinalRegistryProvenanceV1 {
        origin: FinalRegistryObservationOriginV1::ExternalProvider,
        provider: "intercepted-provider-fixture".to_string(),
        source: "https://fixture.invalid/preflight".to_string(),
        evidence_digest: plain_digest(8),
        observed_at_unix_seconds: NOW - 10,
    };
    let observations = candidate
        .rows
        .iter()
        .map(|row| FinalRegistryObservationV1 {
            package_name: row.cargo_package_name.clone(),
            package_version: row.cargo_package_version.clone(),
            version: if row.product_family == PackageCandidateFamilyV2::Shared01 {
                FinalRegistryVersionResponseV1::Found {
                    checksum: canonical(&archive_digest(&row.cargo_package_name)),
                    yanked: false,
                }
            } else {
                FinalRegistryVersionResponseV1::Missing {}
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
        evaluated_at_unix_seconds: NOW,
        maximum_age_seconds: 3600,
        observations,
    })
}

/// This test producer calls the same domain ports as a retained freeze
/// producer. Freshness is derived from selected bytes and the real preflight
/// evaluator; it is not an alternate release eligibility model.
struct FixtureReadings<'a> {
    controls_digest: &'a str,
    preflight: &'a FinalRegistryPreflightInputV1,
}

impl RefreshableObservationAdapterV1 for FixtureReadings<'_> {
    fn refresh(&self, observation: &RefreshableObservationV1) -> ObservationReadingV1 {
        let current = match observation.kind {
            RefreshableObservationKindV1::SourceLiveControl => {
                canonical(&observation.observed_at_utc) == self.controls_digest
            }
            RefreshableObservationKindV1::RegistryFeasibility => matches!(
                evaluate_final_registry_preflight_v1(self.preflight).result,
                FinalRegistryPreflightResultV1::Complete
                    | FinalRegistryPreflightResultV1::CompleteWithResidualAuthorityRisk
            ),
            RefreshableObservationKindV1::AmbientCache => false,
        };
        ObservationReadingV1 {
            freshness: if current {
                ObservationFreshnessV1::Current
            } else {
                ObservationFreshnessV1::Stale
            },
            detail: "intercepted typed producer; no live provider claim".to_string(),
        }
    }
}

fn selected<'a>(files: &'a BTreeMap<String, Vec<u8>>, path: &str) -> TestResult<&'a [u8]> {
    files.get(path).map(Vec::as_slice).ok_or_else(|| io::Error::other("selected fixture file missing").into())
}

fn current_producer() -> ProducerIdentityV1 {
    ProducerIdentityV1 {
        repository: REPOSITORY.to_string(),
        workflow_path: ".github/workflows/release.yml".to_string(),
        git_ref: "refs/heads/main".to_string(),
        run_id: 101,
        run_attempt: 1,
        job_id: "303".to_string(),
        commit_sha: COMMIT.to_string(),
        tree_sha: TREE.to_string(),
        release_version: VERSION.to_string(),
        tool_name: "cargo-allow".to_string(),
        schema_id: RELEASE_OPERATION_HEAD_SCHEMA_ID.to_string(),
        producer_generation: 1,
    }
}

fn typed_fixture() -> TestResult<serde_json::Value> {
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let controls = serde_json::json!({
        "schema": "cargo-allow.live-release-controls-observation.v1", "state": "Feasible",
        "repository": REPOSITORY, "commit": COMMIT, "tree": TREE,
        "checks": {"synthetic_intercepted_readback": true}
    });
    let rehearsal = serde_json::json!({"commit_sha": COMMIT, "phases": {
        "release_identity": "Complete", "candidate_package_set": "Complete",
        "shared_prerequisites": "Complete", "publisher_state_machine": "Complete",
        "docs_and_support_identity": "Complete", "manifest_and_assets": "Complete",
        "workflow_graph_permissions": "Complete", "authorization_boundary": "Incomplete"
    }});
    files.insert("live-controls.json".to_string(), json_bytes(&controls)?);
    files.insert("rehearsal.json".to_string(), json_bytes(&rehearsal)?);
    files.insert("package-docs.json".to_string(), br#"{"synthetic_docs_result":"Complete"}"#.to_vec());
    files.insert("support.toml".to_string(), b"schema = 'intercepted-support-fixture'\n".to_vec());
    files.insert("source-controls.json".to_string(), br#"{"synthetic_source_controls":true}"#.to_vec());
    files.insert("workflow.yml".to_string(), b"name: synthetic-intercepted-workflow\non: workflow_dispatch\n".to_vec());
    files.insert("action-inventory.json".to_string(), br#"{"synthetic_pinned_actions":[]}"#.to_vec());
    files.insert("channel.json".to_string(), br#"{"channel":"stable","synthetic":true}"#.to_vec());
    for (_, asset_name) in RELEASE_OPERATION_ASSET_SELECTION {
        let raw = if asset_name == "release-manifest-v2.json" {
            manifest_bytes()
        } else if asset_name.ends_with(".json") {
            json_bytes(&serde_json::json!({"synthetic_asset": asset_name}))?
        } else {
            format!("exact synthetic asset:{asset_name}\n").into_bytes()
        };
        files.insert(asset_name.to_string(), raw);
    }
    let preflight = preflight_fixture()?;
    let preflight_result = evaluate_final_registry_preflight_v1(&preflight);
    require(
        preflight_result.result == FinalRegistryPreflightResultV1::CompleteWithResidualAuthorityRisk,
        "actual fixture preflight must preserve residual permission risk",
    )?;
    let mut inputs = fixture_with(evidence_graph(), |_| {})?;
    let controls_digest = raw_digest(selected(&files, "live-controls.json")?);
    for observation in &mut inputs.observations {
        if observation.kind == RefreshableObservationKindV1::SourceLiveControl {
            observation.observed_at_utc = controls_digest.clone();
        }
    }
    let replay = replay_final_freeze(&inputs, &FixtureReadings {
        controls_digest: &controls_digest,
        preflight: &preflight,
    });
    require(replay.result == FinalFreezeReplayResultV1::CompleteEquivalent
        && replay.retained_bytes_verified, "actual retained freeze fixture must replay completely")?;
    files.insert("freeze-inputs.json".to_string(), json_bytes(&inputs)?);
    files.insert("freeze-receipt.json".to_string(), json_bytes(&inputs.freeze_receipt)?);
    files.insert("candidate-custody.json".to_string(), json_bytes(&inputs.custody)?);
    files.insert("freeze-replay.json".to_string(), json_bytes(&replay)?);
    files.insert("preflight-inputs.json".to_string(), json_bytes(&preflight)?);

    let (candidate_digest, _) = final_registry_bindings_v1(&preflight.candidate, &preflight.shared_authorities)?;
    let mut freeze = ReleaseAuthorizationFreezeV1 {
        receipt_digest: raw_digest(selected(&files, "freeze-receipt.json")?),
        candidate_digest,
        denominator_digest: String::new(),
        commit: COMMIT.to_string(), tree: TREE.to_string(), lock_digest: plain_digest(1),
        topology_id: preflight.candidate.topology_id.clone(), packages: Vec::new(), shared_prerequisites: Vec::new(),
    };
    for (logical, package, version, shared) in RELEASE_AUTHORIZATION_SELECTION {
        if shared {
            let authority = preflight.shared_authorities.iter().find(|row| row.package_name == package)
                .ok_or_else(|| io::Error::other("fixture shared authority missing"))?;
            freeze.shared_prerequisites.push(ReleaseAuthorizationSharedRowV1 {
                logical_id: logical.to_string(), package_name: package.to_string(), package_version: version.to_string(),
                expected_checksum: authority.expected_checksum.clone(), authority_digest: authority.authority_digest.clone(),
            });
        } else {
            freeze.packages.push(ReleaseAuthorizationPackageRowV1 {
                logical_id: logical.to_string(), package_name: package.to_string(), package_version: version.to_string(),
                package_digest: canonical(&archive_digest(package)), package_size_bytes: archive_bytes(package).len() as u64,
            });
        }
    }
    freeze.denominator_digest = release_authorization_denominator_binding_v1(&freeze)?;
    let evidence = ReleaseAuthorizationEvidenceV1 {
        package_docs_digest: raw_digest(selected(&files, "package-docs.json")?),
        preflight_result: preflight_result.result,
        preflight_evaluated_at_unix_seconds: preflight.evaluated_at_unix_seconds,
        preflight_maximum_age_seconds: preflight.maximum_age_seconds,
        support_digest: raw_digest(selected(&files, "support.toml")?),
        manifest_digest: raw_digest(selected(&files, "release-manifest-v2.json")?),
        rehearsal_complete_except_authorization: true,
        rehearsal_digest: raw_digest(selected(&files, "rehearsal.json")?),
        source_controls_digest: raw_digest(selected(&files, "source-controls.json")?),
        live_controls_digest: controls_digest,
        workflow_digest: raw_digest(selected(&files, "workflow.yml")?),
        action_inventory_digest: raw_digest(selected(&files, "action-inventory.json")?),
        observed_context_digest: raw_digest(&json_bytes(&preflight.observed_context)?),
        current_context_digest: raw_digest(&json_bytes(&preflight.current_context)?),
    };
    // Synthetic frozen-tree inventory produced independently of the decision.
    let frozen_file_digests: Vec<String> = files.values().map(|raw| raw_digest(raw)).collect();
    files.insert("frozen-file-digests.json".to_string(), json_bytes(&frozen_file_digests)?);
    let expected = ReleaseAuthorizationExpectedContextV1 {
        schema_id: RELEASE_AUTHORIZATION_EXPECTED_CONTEXT_SCHEMA_ID.to_string(), schema_version: 1,
        repository: REPOSITORY.to_string(), freeze: freeze.clone(), evidence: evidence.clone(),
        secret_availability: ReleaseAuthorizationSecretAvailabilityV1 { redacted: true, state: ReleaseAuthorizationSecretStateV1::Unknown },
        use_observation: ReleaseAuthorizationUseObservationV1 { state: ReleaseAuthorizationConsumptionV1::Available, consumed_nonces: Vec::new() },
        frozen_file_digests, evaluated_at_unix_seconds: NOW,
    };
    let source = ReleaseAuthorizationSourceV1 {
        kind: ReleaseAuthorizationSourceKindV1::IssueComment, repository: REPOSITORY.to_string(),
        reference: "issue:3760#comment:202".to_string(), author: "release-operator".to_string(),
        body_digest: raw_digest(RELEASE_AUTHORIZATION_EXACT_STATEMENT.as_bytes()),
        statement: RELEASE_AUTHORIZATION_EXACT_STATEMENT.to_string(),
    };
    let decision = ReleaseAuthorizationInputV1 {
        schema_id: RELEASE_AUTHORIZATION_SCHEMA_ID.to_string(), schema_version: 1,
        operation: ReleaseAuthorizationOperationV1 {
            name: RELEASE_AUTHORIZATION_FINAL_OPERATION.to_string(), version: VERSION.to_string(), tag: TAG.to_string(),
            channel: "stable".to_string(), github_prerelease: false, authority_kind: ReleaseAuthorizationAuthorityKindV1::Clean,
        },
        freeze, evidence,
        authority: ReleaseAuthorizationAuthorityV1 {
            selected_auth_class: RELEASE_AUTHORIZATION_AUTH_CLASS.to_string(), maintainer_actor: "release-operator".to_string(),
            maintainer_role: "release-maintainer".to_string(), source: source.clone(),
            created_at_unix_seconds: NOW - 30, expires_at_unix_seconds: NOW + 3600,
            one_run_scope: true, nonce: "synthetic-final-tag-authorization-0001".to_string(),
        },
    };
    let authorization = compile_release_authorization_v1(&decision, &json_bytes(&expected)?);
    require(authorization.result == ReleaseAuthorizationResultV1::Complete,
        "actual independently assembled expected context must compile in the synthetic fixture")?;
    let birth = mint_authorization_custody_v1(AuthorizationCustodyMintInitV1 {
        authorization_id: "intercepted-final-tag-mint".to_string(), decision: decision.clone(),
        freeze_receipt_digest: decision.freeze.receipt_digest.clone(), replay_digest: raw_digest(selected(&files, "freeze-replay.json")?),
        replay_result: "CompleteEquivalent".to_string(), candidate_custody_digest: raw_digest(selected(&files, "candidate-custody.json")?),
        freeze_complete: replay.retained_bytes_verified, replay_complete: replay.result == FinalFreezeReplayResultV1::CompleteEquivalent,
        storage_locator: "https://fixture.invalid/independent-mint".to_string(), repository_root: "/synthetic/frozen/tree".to_string(),
        storage_access_policy: "synthetic-intercepted-operator".to_string(), storage_retention_expiry_unix_seconds: NOW + 7200,
        storage_provider_available: true, valid_from_unix_seconds: NOW - 20, expires_at_unix_seconds: NOW + 3600,
        minted_by: "release-operator".to_string(), minted_at_unix_seconds: NOW - 10,
    }).map_err(io::Error::other)?;
    files.insert("expected-context.json".to_string(), json_bytes(&expected)?);
    files.insert("authorization.json".to_string(), json_bytes(&decision)?);
    files.insert("authorization-custody.json".to_string(), json_bytes(&birth)?);
    let configuration = serde_json::json!({
        "repository_id": 41, "anchor_commit": COMMIT, "anchor_tree": TREE,
        "control_prefix": "refs/heads/cargo-allow-release-control/", "download_hosts": ["artifacts.example.test"],
        "producer": current_producer(), "operation_nonce": "synthetic-final-tag-operation-0001",
        "expires_at_unix_seconds": NOW + 3600, "approved_actor_id": 404, "approved_actor_login": "release-operator",
        "source": source, "artifacts": [], "tagger_name": "Release Fixture",
        "tagger_email": "release@example.invalid", "tag_message": "Synthetic intercepted final-tag fixture.\n"
    });
    Ok(serde_json::json!({"configuration": configuration, "selected": files,
        "source_bytes": RELEASE_AUTHORIZATION_EXACT_STATEMENT.as_bytes(), "now_unix_seconds": NOW}))
}

#[test]
fn final_tag_driver_executes_real_typed_bridge_with_intercepted_provider_io() -> TestResult {
    let fixture = typed_fixture()?;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = std::env::temp_dir().join(format!("cargo-allow-final-tag-driver-{}", std::process::id()));
    fs::create_dir(&directory)?;
    let fixture_path = directory.join("typed-fixture.json");
    fs::write(&fixture_path, json_bytes(&fixture)?)?;
    require(fs::read(&fixture_path)? == json_bytes(&fixture)?,
        "exact typed fixture bytes must reach the intercepted production consumer")?;
    let output = Command::new("python3")
        .args(["-B", "scripts/test-release-final-tag.py", "--bridge", env!("CARGO_BIN_EXE_cargo-allow"), "--fixture"])
        .arg(&fixture_path)
        .current_dir(&root)
        .env_remove("CARGO_ALLOW_ROOT")
        .env_remove("CARGO_ALLOW_CONFIG")
        .output()?;
    fs::remove_dir_all(&directory)?;
    require(output.status.success(), &format!("intercepted actual typed driver failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr)))?;
    Ok(())
}
