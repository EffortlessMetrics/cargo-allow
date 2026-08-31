//! #3919 contract tests: the final freeze replays from retained artifacts
//! alone, without moving or reading `main`.
//!
//! Controls proven here: a removed archive stays `MissingArtifact` even
//! though current source could rebuild it; a modified retained receipt is
//! caught by digest recomputation; an earlier custody aggregate with the
//! same version is rejected; altered evidence-graph edges are caught even
//! with leaf bytes preserved; an expired registry observation forces
//! `Stale`, never `CompleteEquivalent`; the source checkout and ambient
//! caches can never satisfy a missing retained artifact because the replay
//! consumes no input outside its retained set; RC.1 custody replayed as
//! final is a `Mismatch`; omitted remaining irreversible operations force
//! `Incomplete`; and the replay exposes no tag/upload/release/authorization
//! capability.
//!
//! Repair controls: a foreign-generation receipt schema fails closed even
//! with a self-consistent retained-byte chain; a same-version transfer
//! envelope produced from a different commit never counts as coverage; and
//! an envelope set that omits a required artifact forces `MissingArtifact`.
//!
//! The numbered-control fixtures live in the module doc of
//! `final_freeze_replay_v1.rs`.
use allow_core::sha256_v1_bytes;
use allow_report::{
    ArtifactTransferFileV1, ArtifactTransferInitV1, CandidateCustodyInitV1,
    CargoAllowFinalFreezeReceiptV1, CargoAllowFinalFreezeReplayInputsV1,
    CargoAllowFrozenCandidateCustodyV1, CargoAllowReleaseArtifactTransferV1,
    ConfidentialityClassV1, CustodyDispositionV1, CustodyFileV1, FinalEvidenceAuthorityScopeV1,
    FinalEvidenceCurrentnessV1, FinalEvidenceEdgeKindV1, FinalEvidenceEdgeV1,
    FinalEvidenceGraphModeV1, FinalEvidenceGraphV1, FinalEvidenceInvalidationDimensionV1,
    FinalEvidenceNodeClassV1, FinalEvidenceNodeResultV1, FinalEvidenceNodeV1,
    FinalEvidenceOriginV1, FinalEvidencePackageRoleV1, FinalEvidencePackageSubjectV1,
    FinalEvidenceProducerV1, FinalEvidenceReleaseIdentityV1, FinalEvidenceSelectedSubjectV1,
    FinalEvidenceSubjectBindingV1, FinalFreezeManifestBindingV1, FinalFreezeManifestResultV1,
    FinalFreezeReceiptInitV1, FinalFreezeReplayResultV1, ObservationFreshnessV1,
    ObservationReadingV1, ProducerIdentityV1, RefreshableObservationAdapterV1,
    RefreshableObservationKindV1, RefreshableObservationV1, RetainedArtifactBytesV1,
    RetainedCustodyItemV1, RetainedExactArtifactV1, TrustClassV1, UntrustedInputPostureV1,
    replay_final_freeze,
};
use std::io;

fn require(condition: bool, message: &str) -> Result<(), io::Error> {
    if !condition {
        return Err(io::Error::other(message));
    }
    Ok(())
}

const REPOSITORY: &str = "EffortlessMetrics/cargo-allow";
const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
const TREE: &str = "fedcba9876543210fedcba9876543210fedcba98";
const VERSION: &str = "0.2.0";
const TAG: &str = "v0.2.0";
const CUSTODY_ID: &str = "candidate-custody-0.2.0-final";
const RECEIPT_ARTIFACT_ID: &str = "final-freeze-receipt";
const MANIFEST_ARTIFACT_ID: &str = "release-manifest-v2";
const INCIDENT_HANDOFF_ID: &str = "incident-handoff";
const REPLAYED_AT: &str = "2026-08-28T00:00:00Z";

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
    format!("exact-manifest-bytes:{VERSION}").into_bytes()
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
    for (index, name) in [
        "effortless-repo-edit",
        "effortless-repo-protocol",
        "effortless-repo-snapshot",
    ]
    .into_iter()
    .enumerate()
    {
        rows.push(FinalEvidencePackageSubjectV1 {
            logical_id: name.to_string(),
            package_name: name.to_string(),
            version: "0.1.0".to_string(),
            role: FinalEvidencePackageRoleV1::ExistingSharedPrerequisite,
            expected_digest: digest(300 + index as u64),
            observed_digest: Some(digest(300 + index as u64)),
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
            workflow_run_id: Some(7),
            workflow_attempt: Some(1),
            job: Some(evidence_id.to_string()),
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
            frozen_at_utc: "2026-08-26T12:00:00Z".to_string(),
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
        storage_locator: format!("s3://release-custody-2026/0.2.0/{artifact_id}"),
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
        created_at_utc: "2026-08-26T12:00:00Z".to_string(),
    })
}

/// Deterministic observation adapter with per-kind freshness overrides.
struct FixtureAdapter {
    source: ObservationFreshnessV1,
    registry: ObservationFreshnessV1,
}

impl FixtureAdapter {
    fn current() -> Self {
        Self {
            source: ObservationFreshnessV1::Current,
            registry: ObservationFreshnessV1::Current,
        }
    }
}

impl RefreshableObservationAdapterV1 for FixtureAdapter {
    fn refresh(&self, observation: &RefreshableObservationV1) -> ObservationReadingV1 {
        let freshness = match observation.kind {
            RefreshableObservationKindV1::SourceLiveControl => self.source,
            RefreshableObservationKindV1::RegistryFeasibility => self.registry,
            RefreshableObservationKindV1::AmbientCache => ObservationFreshnessV1::Current,
        };
        ObservationReadingV1 {
            freshness,
            detail: "fixture reading".to_string(),
        }
    }
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
            created_at_utc: "2026-08-26T06:00:00Z".to_string(),
        }),
        evidence_graph: graph,
        freeze_receipt: receipt,
        retained_transfers,
        retained_artifacts: artifacts,
        observations: vec![
            RefreshableObservationV1 {
                observation_id: "obs:source-live-control".to_string(),
                kind: RefreshableObservationKindV1::SourceLiveControl,
                observed_at_utc: "2026-08-27T00:00:00Z".to_string(),
            },
            RefreshableObservationV1 {
                observation_id: "obs:registry-feasibility".to_string(),
                kind: RefreshableObservationKindV1::RegistryFeasibility,
                observed_at_utc: "2026-08-27T00:00:00Z".to_string(),
            },
            RefreshableObservationV1 {
                observation_id: "obs:ambient-cache".to_string(),
                kind: RefreshableObservationKindV1::AmbientCache,
                observed_at_utc: "2026-08-27T00:00:00Z".to_string(),
            },
        ],
        replayed_at_utc: REPLAYED_AT.to_string(),
    })
}

fn fixture() -> Result<CargoAllowFinalFreezeReplayInputsV1, io::Error> {
    fixture_with(evidence_graph(), |_| {})
}

fn row_with<'a>(
    replay: &'a allow_report::CargoAllowFinalFreezeReplayV1,
    needle: &str,
) -> Option<&'a allow_report::FinalFreezeReplayRowV1> {
    replay.rows.iter().find(|row| row.message.contains(needle))
}

#[test]
fn final_freeze_replay_complete_equivalent_from_retained_inputs_alone() -> Result<(), io::Error> {
    let inputs = fixture()?;
    let first = replay_final_freeze(&inputs, &FixtureAdapter::current());
    let second = replay_final_freeze(&inputs, &FixtureAdapter::current());
    require(
        first.result == FinalFreezeReplayResultV1::CompleteEquivalent,
        "the complete retained set must replay complete_equivalent",
    )?;
    require(
        first == second,
        "the replay must be deterministic for the same retained set",
    )?;
    require(first.retained_bytes_verified, "retained bytes must verify")?;
    require(
        first.selected_upload_rows == 10 && first.selected_shared_rows == 3,
        "the 10+3 denominator must be reconstructed",
    )?;
    require(
        first.custody_disposition == CustodyDispositionV1::Complete,
        "custody must re-evaluate to complete",
    )?;
    require(
        first.rc1_excluded && first.incident_handoff_present,
        "the RC.1 exclusion and incident handoff must be reconstructed",
    )?;
    require(
        first.remaining_irreversible_operations.len() == 3,
        "the remaining irreversible operations must be echoed",
    )?;
    Ok(())
}

#[test]
fn final_freeze_replay_isolation_removed_archive_cannot_be_rebuilt() -> Result<(), io::Error> {
    let mut inputs = fixture()?;
    inputs
        .retained_artifacts
        .retain(|artifact| artifact.artifact_id != "allow-core");
    inputs
        .custody
        .items
        .retain(|item| item.artifact_id != "allow-core");

    // The ambient cache and source-control observations both report current:
    // neither may satisfy the missing retained archive, and the current
    // source checkout is never consulted because it is not an input.
    let replayed = replay_final_freeze(&inputs, &FixtureAdapter::current());
    require(
        replayed.result == FinalFreezeReplayResultV1::MissingArtifact,
        "a removed archive must stay missing_artifact, got {:?}",
    )?;
    require(
        replayed
            .rows
            .iter()
            .any(|row| row.subject.as_deref() == Some("allow-core")),
        "the missing-artifact row must name the removed archive",
    )?;
    let ambient = replayed
        .observation_readings
        .iter()
        .find(|reading| reading.observation_id == "obs:ambient-cache")
        .ok_or_else(|| io::Error::other("the ambient cache reading is missing"))?;
    require(
        !ambient.authoritative,
        "an ambient cache reading must never be authoritative",
    )?;
    Ok(())
}

#[test]
fn final_freeze_replay_stale_observation_registry_expiry_forces_stale() -> Result<(), io::Error> {
    let inputs = fixture()?;
    let adapter = FixtureAdapter {
        source: ObservationFreshnessV1::Current,
        registry: ObservationFreshnessV1::Stale,
    };
    let replayed = replay_final_freeze(&inputs, &adapter);
    require(
        replayed.result == FinalFreezeReplayResultV1::Stale,
        "an expired registry observation must force stale, got {:?}",
    )?;
    require(
        row_with(
            &replayed,
            "a required refreshable observation is not current",
        )
        .is_some(),
        "the stale-observation row is missing",
    )?;
    Ok(())
}

#[test]
fn final_freeze_replay_modified_retained_receipt_is_caught() -> Result<(), io::Error> {
    let mut inputs = fixture()?;
    // Tamper with the retained receipt without changing its filename or
    // artifact identity: the recomputed canonical digest must diverge from
    // both the retained bytes and the custody record.
    inputs.freeze_receipt.expected_shared_rows = 4;
    let replayed = replay_final_freeze(&inputs, &FixtureAdapter::current());
    require(
        replayed.result == FinalFreezeReplayResultV1::Mismatch,
        "a modified retained receipt must mismatch, got {:?}",
    )?;
    require(
        row_with(&replayed, "no longer hashes to its retained receipt bytes").is_some(),
        "the modified receipt was not caught by digest recomputation",
    )?;
    Ok(())
}

#[test]
fn final_freeze_replay_earlier_custody_aggregate_is_rejected() -> Result<(), io::Error> {
    let mut inputs = fixture()?;
    inputs.custody.custody_id = "candidate-custody-0.2.0-earlier".to_string();
    let replayed = replay_final_freeze(&inputs, &FixtureAdapter::current());
    require(
        replayed.result == FinalFreezeReplayResultV1::Mismatch,
        "an earlier custody aggregate with the same version must mismatch, got {:?}",
    )?;
    require(
        row_with(
            &replayed,
            "not the custody aggregate bound by the freeze receipt",
        )
        .is_some(),
        "the earlier custody aggregate was not rejected",
    )?;
    Ok(())
}

#[test]
fn final_freeze_replay_altered_graph_edges_are_caught() -> Result<(), io::Error> {
    let mut inputs = fixture()?;
    // Alter an edge while every leaf node digest stays byte-identical: the
    // canonical graph digest diverges from the receipt's recorded digest.
    let first_edge = match inputs.evidence_graph.edges.first_mut() {
        Some(edge) => edge,
        None => return Err(io::Error::other("fixture lost its first edge")),
    };
    first_edge.kind = FinalEvidenceEdgeKindV1::SupportsOnly;
    let replayed = replay_final_freeze(&inputs, &FixtureAdapter::current());
    require(
        replayed.result == FinalFreezeReplayResultV1::Mismatch,
        "altered graph edges must mismatch even with leaf bytes preserved, got {:?}",
    )?;
    require(
        row_with(
            &replayed,
            "no longer hashes to the freeze receipt's recorded graph digest",
        )
        .is_some(),
        "the altered edges were not caught by the graph digest",
    )?;
    Ok(())
}

#[test]
fn final_freeze_replay_rc1_custody_replayed_as_final_mismatches() -> Result<(), io::Error> {
    let mut inputs = fixture()?;
    inputs.custody.candidate_version = "0.2.0-rc.1".to_string();
    let replayed = replay_final_freeze(&inputs, &FixtureAdapter::current());
    require(
        replayed.result == FinalFreezeReplayResultV1::Mismatch,
        "rc.1 custody replayed as final must mismatch, got {:?}",
    )?;
    require(
        row_with(
            &replayed,
            "does not bind the freeze receipt's exact candidate identity",
        )
        .is_some(),
        "the rc.1 custody row did not name the identity binding",
    )?;
    Ok(())
}

#[test]
fn final_freeze_replay_omitted_irreversible_operations_force_incomplete() -> Result<(), io::Error> {
    let inputs = fixture_with(evidence_graph(), |receipt| {
        receipt.remaining_irreversible_operations = Vec::new();
    })?;
    let replayed = replay_final_freeze(&inputs, &FixtureAdapter::current());
    require(
        replayed.result == FinalFreezeReplayResultV1::Incomplete,
        "omitted remaining irreversible operations must force incomplete, got {:?}",
    )?;
    require(
        row_with(&replayed, "records no remaining irreversible operations").is_some(),
        "the empty operation list was not flagged",
    )?;
    Ok(())
}

#[test]
fn final_freeze_replay_foreign_receipt_schema_fails_closed() -> Result<(), io::Error> {
    // The schema customization lands before every derived digest is computed,
    // so the retained-byte chain is fully self-consistent: only the declared
    // schema generation may reject this receipt, and serde's willingness to
    // deserialize an unknown generation into the V1 type must not help it.
    let inputs = fixture_with(evidence_graph(), |receipt| {
        receipt.schema_id = "cargo-allow.final-freeze-receipt.v2".to_string();
    })?;
    let replayed = replay_final_freeze(&inputs, &FixtureAdapter::current());
    require(
        replayed.result != FinalFreezeReplayResultV1::CompleteEquivalent,
        "a foreign-generation receipt must not replay complete_equivalent, got {:?}",
    )?;
    require(
        row_with(&replayed, "the replay only consumes schema").is_some(),
        "the schema failure row is missing",
    )?;
    require(
        !replayed.retained_bytes_verified,
        "a schema failure must leave the retained bytes unverified",
    )?;
    Ok(())
}

#[test]
fn final_freeze_replay_same_version_envelope_from_other_commit_is_not_coverage()
-> Result<(), io::Error> {
    let mut inputs = fixture()?;
    let envelope = match inputs.retained_transfers.first_mut() {
        Some(envelope) => envelope,
        None => return Err(io::Error::other("fixture lost its first envelope")),
    };
    // Correct release version, different build: producer identity must be
    // bound to the frozen subject, so this envelope cannot supply coverage.
    envelope.producer.commit_sha = "9999999999999999999999999999999999999999".to_string();
    let replayed = replay_final_freeze(&inputs, &FixtureAdapter::current());
    require(
        replayed.result == FinalFreezeReplayResultV1::Mismatch,
        "a same-version envelope from a different commit must mismatch, got {:?}",
    )?;
    require(
        row_with(
            &replayed,
            "produced from a different commit than the frozen subject",
        )
        .is_some(),
        "the commit binding row did not name the mismatch",
    )?;
    Ok(())
}

#[test]
fn final_freeze_replay_envelope_missing_required_artifact_is_missing() -> Result<(), io::Error> {
    let mut inputs = fixture()?;
    inputs
        .retained_transfers
        .retain(|envelope| envelope.stable_artifact_id != "allow-diff");
    let replayed = replay_final_freeze(&inputs, &FixtureAdapter::current());
    require(
        replayed.result == FinalFreezeReplayResultV1::MissingArtifact,
        "an envelope set that omits a required artifact must force missing_artifact, got {:?}",
    )?;
    require(
        replayed.rows.iter().any(|row| {
            row.subject.as_deref() == Some("allow-diff")
                && row
                    .message
                    .contains("not covered by any retained transfer envelope")
        }),
        "the coverage row must name the uncovered artifact",
    )?;
    Ok(())
}

#[test]
fn final_freeze_replay_has_no_mutation_capability() -> Result<(), io::Error> {
    let inputs = fixture()?;
    let replayed = replay_final_freeze(&inputs, &FixtureAdapter::current());
    let json = serde_json::to_string(&replayed).map_err(io::Error::other)?;
    // The replay output is inert data: it echoes the receipt's operation list
    // and carries no handle, token, command, or authorization output of any
    // kind. The claim boundary states the structural no-mutation guarantee.
    require(
        replayed
            .claim_boundary
            .contains("never tags, uploads, publishes, authorizes"),
        "the replay claim boundary must state the no-mutation guarantee",
    )?;
    require(
        !json.contains("credential") && !json.contains("token"),
        "the replay output must not carry credential or token surfaces",
    )?;
    let expected = [
        "publish the GitHub release".to_string(),
        "push tag v0.2.0".to_string(),
        "upload 10 package rows to crates.io".to_string(),
    ];
    require(
        replayed.remaining_irreversible_operations == expected,
        "the echoed operations must be the receipt's list, canonically ordered",
    )?;
    Ok(())
}
