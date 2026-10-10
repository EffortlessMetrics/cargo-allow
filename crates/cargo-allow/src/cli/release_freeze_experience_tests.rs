//! Original-bundle admission controls. All provider records and opaque pilot/
//! docs bytes here are explicit fixtures, never evidence of an executed pilot.

use super::super::super as command;
use super::*;
use allow_report::{
    CargoAllowReleaseExperienceV1, ExactCandidatePayloadV2, ExactCandidateResultV2,
    FinalEvidenceCurrentnessV1, FinalEvidenceNodeResultV1, FinalEvidencePackageRoleV1,
    FinalEvidencePackageSubjectV1, FinalReadinessVerdictV1, IsolatedInstallPayloadV2,
    IsolatedInstallResultV2, PackageCandidatePayloadV2, PackageCandidateResultV2,
    RELEASE_EXPERIENCE_REQUIRED_DOCS, ReleaseExperienceBrownfieldPostureV1,
    ReleaseExperienceDocsIdentityV1, ReleaseExperienceFrictionDispositionV1,
    ReleaseExperienceFrictionV1, ReleaseExperienceInputV1, ReleaseExperiencePilotResultV1,
    ReleaseExperiencePilotV1, ReleaseExperienceResultV1, evaluate_release_experience_v1,
    validate_exact_candidate_v2, validate_isolated_install_v2, validate_package_candidate_v2,
};

type ExperienceResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
type State = ReleaseExperienceResultV1;

struct OriginalBundle {
    subject: SubjectIdentity,
    readback: ReadbackInput,
    packages: Vec<FinalEvidencePackageSubjectV1>,
    input: ReleaseExperienceInputV1,
}

fn digest(bytes: &[u8]) -> String {
    sha256_v1_bytes(bytes).replacen("sha256:v1:", "sha256:", 1)
}

impl OriginalBundle {
    fn put(&mut self, id: &str, role: &str, bytes: Vec<u8>) -> ExperienceResult<String> {
        let sha = digest(&bytes);
        let artifact = self
            .readback
            .artifacts
            .first_mut()
            .ok_or("original artifact absent")?;
        if let Some(member) = artifact
            .members
            .iter_mut()
            .find(|member| member.logical_id == id)
        {
            member.bytes = bytes;
            member.role = role.to_string();
        } else {
            artifact.members.push(MemberReadback {
                logical_id: id.to_string(),
                role: role.to_string(),
                path: format!("{}.json", id.replace(':', "-")),
                bytes,
            });
        }
        self.refresh()?;
        Ok(sha)
    }

    fn refresh(&mut self) -> ExperienceResult {
        let artifact = self
            .readback
            .artifacts
            .first_mut()
            .ok_or("original artifact absent")?;
        artifact.transfer.files = artifact
            .members
            .iter()
            .map(|member| ArtifactTransferFileV1 {
                path: member.path.clone(),
                size_bytes: member.bytes.len() as u64,
                sha256: digest(&member.bytes),
            })
            .collect();
        Ok(())
    }

    fn pair(&mut self) -> ExperienceResult {
        self.put(
            "evidence:experience-input",
            "Evidence:experience-input",
            serde_json::to_vec(&self.input)?,
        )?;
        self.put(
            "evidence:release-experience",
            "Evidence:release-experience",
            serde_json::to_vec(&evaluate_release_experience_v1(&self.input))?,
        )?;
        Ok(())
    }

    fn evidence(&self) -> ExperienceResult<Vec<EvidenceInput>> {
        let mut evidence = Vec::new();
        for (role, id) in [
            (
                FreezeEvidenceRole::ReleaseExperienceInput,
                "evidence:experience-input",
            ),
            (
                FreezeEvidenceRole::ReleaseExperience,
                "evidence:release-experience",
            ),
        ] {
            if let Some((_, member)) = self.readback.member(id)? {
                evidence.push(EvidenceInput {
                    role,
                    path: std::path::PathBuf::from(&member.path),
                    sha256: sha256_v1_bytes(&member.bytes),
                    value: serde_json::from_slice(&member.bytes)?,
                    binding_notes: Vec::new(),
                });
            }
        }
        Ok(evidence)
    }

    fn admission(&self) -> ExperienceResult<command::experience::Admission> {
        Ok(command::experience::reconcile(
            &self.subject,
            &self.packages,
            &self.evidence()?,
            Some(&self.readback),
        ))
    }
}

fn bundle() -> ExperienceResult<OriginalBundle> {
    let (subject, mut readback) = fixture()?;
    let now = readback.experience_observed_at(&subject)?;
    readback
        .artifacts
        .first_mut()
        .ok_or("original artifact absent")?
        .members
        .clear();
    let names = [
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
        "effortless-repo-edit",
        "effortless-repo-snapshot",
        "effortless-rust-source-index",
    ];
    let mut packages = Vec::new();
    let mut candidate_rows = Vec::new();
    let mut install_rows = Vec::new();
    let mut journey_rows = Vec::new();
    for (index, name) in names.into_iter().enumerate() {
        let upload = index < 10;
        let version = if upload { "0.2.0" } else { "0.1.0" };
        let archive = format!("selected isolated archive {name}").into_bytes();
        let archive_digest = digest(&archive);
        packages.push(FinalEvidencePackageSubjectV1 {
            logical_id: name.to_string(),
            package_name: name.to_string(),
            version: version.to_string(),
            role: if upload {
                FinalEvidencePackageRoleV1::UploadCandidate
            } else {
                FinalEvidencePackageRoleV1::ExistingSharedPrerequisite
            },
            expected_digest: if upload {
                archive_digest.clone()
            } else {
                digest(format!("public registry {name}").as_bytes())
            },
            observed_digest: None,
        });
        candidate_rows.push(serde_json::json!({
            "logical_id":name, "cargo_package_name":name, "cargo_package_version":version,
            "rust_library_name":name.replace('-', "_"), "workspace_source_path":format!("crates/{name}"),
            "product_family":if upload { "cargo-allow-0.2" } else { "shared-0.1" },
            "publication_state":"selected", "publish":true, "support_tier":"fixture",
            "release_order":index + 1, "selected_features":[],
            "expected_manifest_identity":format!("{name}:{version}"), "expected_dependency_rows":[],
            "required_assets":[], "crate_digest":archive_digest, "crate_size_bytes":archive.len(),
        }));
        install_rows.push(serde_json::json!({
            "package_name":name, "package_version":version, "crate_digest":archive_digest,
            "index_checksum":archive_digest, "resolved_version":version,
        }));
        journey_rows.push(serde_json::json!({
            "logical_id":name, "package_name":name, "package_version":version,
            "crate_digest":archive_digest,
        }));
    }
    let candidate = serde_json::json!({
        "schema_id":"cargo-allow.package-candidate.v2", "schema_version":2,
        "topology_id":"fixture-topology", "topology_digest":digest(b"normalized topology"),
        "repository_commit":subject.commit, "repository_tree":subject.tree,
        "cargo_lock_digest":digest(b"LF-normalized workspace lock"), "candidate_product_id":"cargo-allow",
        "root_logical_id":"cargo-allow", "root_package_name":"cargo-allow", "root_package_version":"0.2.0",
        "target_class":"linux-gnu", "feature_set_id":"default", "rows":candidate_rows,
        "known_exclusions":[], "limitations":["synthetic structural fixture"],
        "claim_boundary":"existing predecessor shape only; no execution is claimed",
    });
    let candidate_bytes = serde_json::to_vec(&candidate)?;
    let candidate_digest = digest(&candidate_bytes);
    let binary_digest = digest(b"selected executable identity; binary is not bundled");
    let install = serde_json::json!({
        "schema_id":"cargo-allow.isolated-install.v2", "schema_version":2,
        "candidate_artifact_digest":candidate_digest, "repository_commit":subject.commit,
        "repository_tree":subject.tree, "cargo_lock_digest":digest(b"packaged root lock"),
        "registry_index_digest":digest(b"isolated index"), "external_cache_identity":"selected-cache",
        "source_checkout_denied":true, "install_root_identity":digest(b"install root identity"),
        "cargo_home_identity":digest(b"isolated cargo home identity"),
        "installed_executable_digest":binary_digest, "installed_version_output":"cargo-allow 0.2.0",
        "platform":"x86_64-unknown-linux-gnu", "toolchain":"selected-toolchain",
        "package_rows":install_rows,
        "graph_comparison":{"expected_packages":13, "matched_packages":13, "unexpected_packages":[],
            "missing_packages":[], "version_mismatches":[], "path_sources":[]},
        "limitations":["synthetic structural fixture"], "claim_boundary":"existing install contract only",
    });
    let install_bytes = serde_json::to_vec(&install)?;
    let install_digest = digest(&install_bytes);
    let journey = serde_json::json!({
        "schema_id":"cargo-allow.exact-candidate.v2", "schema_version":2,
        "candidate_artifact_digest":candidate_digest, "isolated_install_receipt_digest":install_digest,
        "repository_commit":subject.commit, "repository_tree":subject.tree,
        "cargo_lock_digest":subject.cargo_lock_digest, "installed_executable_digest":binary_digest,
        "installed_version_output":"cargo-allow 0.2.0", "platform":"x86_64-unknown-linux-gnu",
        "toolchain":"selected-toolchain", "support_matrix_generation":"selected-support",
        "package_rows":journey_rows, "journey_steps":[{"id":"existing-synthetic-step", "exit_code":0}],
        "artifact_schema_results":[], "scanner_completeness":"complete", "diff_base_identity":"selected-base",
        "limitations":["not the missing #3149 execution catalogue"], "not_included":[],
        "claim_boundary":"existing journey structural contract only",
    });
    let journey_bytes = serde_json::to_vec(&journey)?;
    let mut original = OriginalBundle {
        subject,
        readback,
        packages,
        input: ReleaseExperienceInputV1 {
            schema_id: "cargo-allow.release-experience.v1".to_string(),
            schema_version: 1,
            candidate_digest,
            install_digest,
            journey_digest: digest(&journey_bytes),
            binary_digest,
            invocation_path: "installed/bin/cargo-allow".to_string(),
            support_matrix_generation: "selected-support".to_string(),
            command_registry_generation: "selected-registry".to_string(),
            migration_denominator_digest: digest(b"opaque migration catalogue"),
            migration_schema_id: "cargo-allow.core-command-summary.v1".to_string(),
            clean_pilot: None,
            brownfield_posture:
                ReleaseExperienceBrownfieldPostureV1::NotIncludedPendingPublishedPilot,
            brownfield_receipt_digest: None,
            docs_identities: Vec::new(),
            frictions: Vec::new(),
            claimed_result: State::NotProven,
            not_proven_reason: "No clean external pilot was executed".to_string(),
            narrowed_claims: vec!["No low-friction external adoption claim".to_string()],
            observed_at_unix_seconds: now - 20,
            evaluated_at_unix_seconds: now - 10,
            maximum_age_seconds: 600,
        },
    };
    for (id, bytes) in [
        ("experience:package-candidate", candidate_bytes),
        ("experience:isolated-install", install_bytes),
        ("experience:exact-candidate", journey_bytes),
        (
            "experience:migration-denominator",
            b"opaque migration catalogue".to_vec(),
        ),
    ] {
        original.put(id, "ExperienceReference", bytes)?;
    }
    for name in RELEASE_EXPERIENCE_REQUIRED_DOCS {
        let sha = original.put(
            &format!("experience:docs:{name}"),
            "ExperienceReference",
            format!("opaque selected documentation {name}").into_bytes(),
        )?;
        original
            .input
            .docs_identities
            .push(ReleaseExperienceDocsIdentityV1 {
                name: name.to_string(),
                digest: sha,
            });
    }
    original.pair()?;
    Ok(original)
}

fn assert_admission(bundle: &OriginalBundle, expected: State, reason: &str) -> ExperienceResult {
    let actual = bundle.admission()?;
    if actual.result != expected || !actual.notes.iter().any(|note| note.contains(reason)) {
        return Err(format!(
            "expected {expected:?}/{reason:?}, got {:?}/{:?}",
            actual.result, actual.notes
        )
        .into());
    }
    Ok(())
}

#[test]
fn exact_originals_keep_not_proven_and_the_three_owner_specific_locks() -> ExperienceResult {
    let original = bundle()?;
    original.readback.validate(&original.subject)?;
    for (id, expected) in [
        ("experience:package-candidate", "package"),
        ("experience:isolated-install", "install"),
        ("experience:exact-candidate", "journey"),
    ] {
        let (_, member) = original.readback.member(id)?.ok_or("predecessor missing")?;
        let valid = match expected {
            "package" => {
                validate_package_candidate_v2(&serde_json::from_slice::<PackageCandidatePayloadV2>(
                    &member.bytes,
                )?)
                .result
                    == PackageCandidateResultV2::Complete
            }
            "install" => {
                validate_isolated_install_v2(&serde_json::from_slice::<IsolatedInstallPayloadV2>(
                    &member.bytes,
                )?)
                .result
                    == IsolatedInstallResultV2::Complete
            }
            _ => {
                validate_exact_candidate_v2(&serde_json::from_slice::<ExactCandidatePayloadV2>(
                    &member.bytes,
                )?)
                .result
                    == ExactCandidateResultV2::Complete
            }
        };
        if !valid {
            return Err(format!("existing {expected} predecessor fixture is invalid").into());
        }
    }
    let admission = original.admission()?;
    let local = command::experience::reconcile(
        &original.subject,
        &original.packages,
        &original.evidence()?,
        None,
    );
    if local.currentness != FinalEvidenceCurrentnessV1::ProviderUnavailable
        || admission.currentness != FinalEvidenceCurrentnessV1::Current
    {
        return Err("unknown observation time acquired a factual currentness projection".into());
    }
    if admission.result != State::NotProven || admission.notes.len() != 3 {
        return Err(format!(
            "byte/subject admission added an unexpected defect: {:?}",
            admission.notes
        )
        .into());
    }
    for owner in ["#2466", "#3149", "#3151"] {
        assert_admission(&original, State::NotProven, owner)?;
    }
    let evidence = original.evidence()?;
    let result = evidence
        .iter()
        .find(|item| item.role == FreezeEvidenceRole::ReleaseExperience)
        .ok_or("original result missing")?;
    let result: CargoAllowReleaseExperienceV1 = serde_json::from_value(result.value.clone())?;
    if result != evaluate_release_experience_v1(&original.input)
        || result.result != State::NotProven
    {
        return Err("original model result was rewritten".into());
    }
    Ok(())
}

#[test]
fn consistently_forged_complete_pair_and_opaque_references_cannot_discharge_producers()
-> ExperienceResult {
    let mut original = bundle()?;
    let pilot = original.put(
        "experience:clean-pilot",
        "ExperienceReference",
        b"arbitrary pilot bytes".to_vec(),
    )?;
    let friction = original.put(
        "experience:clean-pilot-friction",
        "ExperienceReference",
        b"arbitrary friction bytes".to_vec(),
    )?;
    original.input.clean_pilot = Some(ReleaseExperiencePilotV1 {
        receipt_digest: pilot,
        result: ReleaseExperiencePilotResultV1::Complete,
        friction_digest: friction,
    });
    original.input.claimed_result = State::Complete;
    original.input.not_proven_reason.clear();
    original.input.narrowed_claims.clear();
    original.pair()?;
    if evaluate_release_experience_v1(&original.input).result != State::Complete {
        return Err(
            "forged model-level Complete control did not reach the intended boundary".into(),
        );
    }
    for owner in ["#2466", "#3149", "#3151"] {
        assert_admission(&original, State::NotProven, owner)?;
    }
    Ok(())
}

#[test]
fn missing_duplicate_and_changed_original_records_have_precise_results() -> ExperienceResult {
    let original = bundle()?;
    let mut evidence = original.evidence()?;
    evidence.retain(|item| item.role != FreezeEvidenceRole::ReleaseExperienceInput);
    let admission = command::experience::reconcile(
        &original.subject,
        &original.packages,
        &evidence,
        Some(&original.readback),
    );
    if admission.result != State::Incomplete {
        return Err("missing original input was not Incomplete".into());
    }
    let mut evidence = original.evidence()?;
    let duplicate = original.evidence()?.pop().ok_or("result missing")?;
    evidence.push(duplicate);
    if command::experience::reconcile(
        &original.subject,
        &original.packages,
        &evidence,
        Some(&original.readback),
    )
    .result
        != State::Malformed
    {
        return Err("duplicate original result was not Malformed".into());
    }
    let mut original = bundle()?;
    let mut changed = evaluate_release_experience_v1(&original.input);
    changed.retained_evidence.clear();
    original.put(
        "evidence:release-experience",
        "Evidence:release-experience",
        serde_json::to_vec(&changed)?,
    )?;
    assert_admission(&original, State::Mismatch, "complete canonical evaluation")?;
    Ok(())
}

#[test]
fn original_reference_missing_digest_role_foreign_subject_and_provider_fail_separately()
-> ExperienceResult {
    for defect in 0..6 {
        let mut original = bundle()?;
        match defect {
            0 => {
                let artifact = original
                    .readback
                    .artifacts
                    .first_mut()
                    .ok_or("artifact missing")?;
                artifact
                    .members
                    .retain(|member| member.logical_id != "experience:docs:help");
                original.refresh()?;
            }
            1 => {
                original.put(
                    "experience:docs:help",
                    "ExperienceReference",
                    b"changed original help".to_vec(),
                )?;
            }
            2 => {
                let (_, bytes) = original
                    .readback
                    .experience_member("experience:docs:help")?
                    .ok_or("help missing")?;
                let bytes = bytes.to_vec();
                original.put("experience:docs:help", "EvidenceGraph", bytes)?;
            }
            3 => {
                let (_, bytes) = original
                    .readback
                    .experience_member("experience:exact-candidate")?
                    .ok_or("journey missing")?;
                let mut value: ExactCandidatePayloadV2 = serde_json::from_slice(bytes)?;
                value.repository_commit = "f".repeat(40);
                original.input.journey_digest = original.put(
                    "experience:exact-candidate",
                    "ExperienceReference",
                    serde_json::to_vec(&value)?,
                )?;
                original.pair()?;
            }
            4 => {
                original
                    .readback
                    .artifacts
                    .first_mut()
                    .ok_or("artifact missing")?
                    .expected_producer
                    .run_attempt = 2;
            }
            _ => {
                original
                    .packages
                    .first_mut()
                    .ok_or("package missing")?
                    .expected_digest = digest(b"other selected upload");
            }
        }
        let (state, message) = match defect {
            0 => (State::Incomplete, "missing original experience member"),
            1 | 2 => (State::Mismatch, "role or exact byte digest"),
            3 => (State::Mismatch, "selected source"),
            4 => (State::InstrumentFailure, "original readback context"),
            _ => (State::Mismatch, "selected upload bytes"),
        };
        assert_admission(&original, state, message)?;
    }
    Ok(())
}

#[test]
fn current_observation_rejects_expired_and_future_originals_without_rewriting_them()
-> ExperienceResult {
    for future in [false, true] {
        let mut original = bundle()?;
        let now = original
            .readback
            .experience_observed_at(&original.subject)?;
        let observed = if future { now + 1 } else { now - 1_000 };
        original.input.observed_at_unix_seconds = observed;
        original.input.evaluated_at_unix_seconds = observed;
        original.pair()?;
        if evaluate_release_experience_v1(&original.input).result != State::NotProven {
            return Err(
                "original timestamp control was already denied by the historical model".into(),
            );
        }
        assert_admission(&original, State::Stale, "checked provider observation")?;
    }
    Ok(())
}

#[test]
fn duplicate_docs_and_open_friction_preserve_the_existing_evaluator_priority() -> ExperienceResult {
    let mut original = bundle()?;
    let duplicate = original
        .input
        .docs_identities
        .first()
        .ok_or("docs fixture missing")?
        .clone();
    original.input.docs_identities.push(duplicate);
    original.pair()?;
    assert_admission(
        &original,
        State::Malformed,
        "duplicate experience documentation",
    )?;
    let mut original = bundle()?;
    original.input.frictions.push(ReleaseExperienceFrictionV1 {
        id: "unresolved".to_string(),
        disposition: ReleaseExperienceFrictionDispositionV1::Open,
        note: "The actual owner has not closed this fixture friction".to_string(),
    });
    original.pair()?;
    assert_admission(&original, State::Incomplete, "open friction blockers")?;
    Ok(())
}

#[test]
fn required_experience_rows_block_readiness_with_all_unrelated_gates_fixed() -> ExperienceResult {
    let original = bundle()?;
    let mut evidence = original.evidence()?;
    // These synthetic already-admitted peers isolate the new required rows;
    // no actual producer success is asserted by this construction.
    for role in [
        FreezeEvidenceRole::CandidatePreparation,
        FreezeEvidenceRole::PackageSet,
        FreezeEvidenceRole::PackageDocs,
        FreezeEvidenceRole::Rehearsal,
        FreezeEvidenceRole::InstallJourney,
        FreezeEvidenceRole::UpgradeRollback,
        FreezeEvidenceRole::Controls,
    ] {
        evidence.push(EvidenceInput {
            role,
            path: std::path::PathBuf::from("synthetic-peer.json"),
            sha256: sha256_v1_bytes(role.label().as_bytes()),
            value: serde_json::Value::Null,
            binding_notes: Vec::new(),
        });
    }
    let selection = command::tests::selection();
    let current = allow_report::ObservationReadingV1 {
        freshness: allow_report::ObservationFreshnessV1::Current,
        detail: "isolated fixture gate".to_string(),
    };
    let mut graph = command::build_evidence_graph(
        &original.subject,
        &selection,
        &evidence,
        &original.packages,
        None,
        (FinalEvidenceNodeResultV1::Complete, &current),
    );
    let mut decisions =
        command::readiness_decision_inputs(&original.subject, &selection, &evidence);
    decisions.remaining_reversible_work.clear();
    for node in &mut graph.nodes {
        node.result = FinalEvidenceNodeResultV1::Complete;
        node.currentness = FinalEvidenceCurrentnessV1::Current;
    }
    let positive = aggregate_final_readiness(&graph, &decisions);
    if positive.verdict != FinalReadinessVerdictV1::ReadyForFreeze {
        return Err(format!("unrelated-gate control is not ready: {:?}", positive.rows).into());
    }
    command::experience::apply(&mut graph, &original.admission()?);
    let denied = aggregate_final_readiness(&graph, &decisions);
    if denied.verdict == FinalReadinessVerdictV1::ReadyForFreeze {
        return Err("required NotProven experience was waived".into());
    }
    for id in ["release-experience-input", "release-experience"] {
        if !graph.required_node_ids.iter().any(|value| value == id)
            || !denied
                .required_evidence
                .iter()
                .any(|row| row.evidence_id == id)
            || !denied
                .rows
                .iter()
                .any(|row| row.evidence_id.as_deref() == Some(id))
        {
            return Err(format!("required admission lost direct readiness row {id}").into());
        }
    }
    if decisions
        .root_decisions
        .iter()
        .any(|decision| decision.decision_id.starts_with("pilot-"))
    {
        return Err("composer fabricated a pilot applicability decision".into());
    }
    Ok(())
}

#[test]
fn exact_small_original_members_survive_existing_custody_serialization() -> ExperienceResult {
    let original = bundle()?;
    let artifact = original
        .readback
        .artifacts
        .first()
        .ok_or("artifact missing")?;
    let envelope_before = serde_json::to_vec(&artifact.transfer)?;
    let (custody, retained) = original
        .readback
        .custody(&original.subject, "selected-experience-custody")?;
    let bytes = serde_json::to_vec(&(custody.clone(), retained.clone()))?;
    let (decoded_custody, decoded_retained): (
        CargoAllowFrozenCandidateCustodyV1,
        Vec<RetainedExactArtifactV1>,
    ) = serde_json::from_slice(&bytes)?;
    if custody != decoded_custody
        || retained != decoded_retained
        || envelope_before != serde_json::to_vec(&artifact.transfer)?
        || retained.len() != 14
        || custody
            .items
            .iter()
            .any(|item| item.files.len() != 1 || !item.storage_locator.contains("/505/"))
        || original
            .readback
            .artifacts
            .iter()
            .flat_map(|artifact| &artifact.members)
            .any(|member| member.bytes.len() > MAX_MEMBER_BYTES || member.role == "InstalledBinary")
    {
        return Err("existing exact member custody lost identity or duplicated a binary".into());
    }
    for evidence in original.evidence()? {
        let id = format!("evidence:{}", evidence.role.label());
        let item = retained
            .iter()
            .find(|item| item.artifact_id == id)
            .ok_or("original result/input not retained")?;
        if item.declared_sha256 != evidence.sha256 {
            return Err("retained original digest changed".into());
        }
    }
    Ok(())
}

#[test]
fn every_existing_experience_state_maps_without_a_success_fallback() -> ExperienceResult {
    for (result, expected) in [
        (State::Unsupported, FinalEvidenceNodeResultV1::Unsupported),
        (State::Malformed, FinalEvidenceNodeResultV1::Malformed),
        (
            State::InstrumentFailure,
            FinalEvidenceNodeResultV1::InstrumentFailure,
        ),
        (State::Mismatch, FinalEvidenceNodeResultV1::Mismatch),
        (State::Stale, FinalEvidenceNodeResultV1::Stale),
        (State::Incomplete, FinalEvidenceNodeResultV1::Incomplete),
        (State::NotProven, FinalEvidenceNodeResultV1::NotProven),
        (State::Complete, FinalEvidenceNodeResultV1::Complete),
    ] {
        let actual = command::experience::Admission {
            result,
            currentness: FinalEvidenceCurrentnessV1::ProviderUnavailable,
            notes: Vec::new(),
        };
        if actual.graph_result() != expected {
            return Err(format!("experience state {result:?} changed in projection").into());
        }
    }
    Ok(())
}

#[test]
fn serialized_replay_recomputes_an_experience_reference_digest() -> ExperienceResult {
    let original = bundle()?;
    let evidence = original.evidence()?;
    let reading = allow_report::ObservationReadingV1 {
        freshness: allow_report::ObservationFreshnessV1::ProviderUnavailable,
        detail: "unrelated refreshable observation is deliberately unproven".to_string(),
    };
    let graph = command::build_evidence_graph(
        &original.subject,
        &command::tests::selection(),
        &evidence,
        &original.packages,
        None,
        (FinalEvidenceNodeResultV1::NotProven, &reading),
    );
    let (custody, retained_artifacts) = original
        .readback
        .custody(&original.subject, "replay-experience")?;
    let receipt =
        allow_report::CargoAllowFinalFreezeReceiptV1::new(allow_report::FinalFreezeReceiptInitV1 {
            freeze_id: "synthetic-experience-replay".to_string(),
            frozen_custody_id: custody.custody_id.clone(),
            frozen_at_utc: original.readback.observed_at_utc.clone(),
            release_identity: original.subject.release_identity(),
            repository: REPOSITORY.to_string(),
            commit: original.subject.commit.clone(),
            tree: original.subject.tree.clone(),
            cargo_lock_digest: original.subject.cargo_lock_digest.clone(),
            topology_digest: original.subject.topology_digest.clone(),
            expected_upload_rows: 10,
            expected_shared_rows: 3,
            package_rows: original.packages.clone(),
            prepublication_manifest: allow_report::FinalFreezeManifestBindingV1 {
                result: allow_report::FinalFreezeManifestResultV1::NotRun,
                artifact_id: "release-manifest-v2".to_string(),
                payload_sha256: String::new(),
            },
            rc1_excluded: true,
            rc1_version: Some("0.2.0-rc.1".to_string()),
            incident_handoff_id: None,
            recorded_graph_digest: final_evidence_graph_digest(&graph)?,
            remaining_irreversible_operations: command::REMAINING_IRREVERSIBLE_OPERATIONS
                .iter()
                .map(|operation| (*operation).to_string())
                .collect(),
        });
    let input = CargoAllowFinalFreezeReplayInputsV1 {
        custody,
        evidence_graph: graph,
        freeze_receipt: receipt,
        retained_transfers: original
            .readback
            .artifacts
            .iter()
            .map(|artifact| artifact.transfer.clone())
            .collect(),
        retained_artifacts,
        observations: Vec::new(),
        replayed_at_utc: original.readback.observed_at_utc.clone(),
    };
    let bytes = serde_json::to_vec(&input)?;
    let mut retained: CargoAllowFinalFreezeReplayInputsV1 = serde_json::from_slice(&bytes)?;
    if retained != input {
        return Err("existing replay input serialization changed original members".into());
    }
    let adapter = FreezeObservationAdapter {
        source_current: false,
        registry_reading: reading,
    };
    let id = "experience:docs:help";
    let before = replay_final_freeze(&retained, &adapter);
    if before
        .rows
        .iter()
        .any(|row| row.subject.as_deref() == Some(id))
    {
        return Err("original help reference already failed its specific replay boundary".into());
    }
    retained
        .retained_artifacts
        .iter_mut()
        .find(|item| item.artifact_id == id)
        .ok_or("retained help reference absent")?
        .bytes = RetainedArtifactBytesV1::new(b"changed after serialization".to_vec());
    let after = replay_final_freeze(&retained, &adapter);
    if !after.rows.iter().any(|row| {
        row.subject.as_deref() == Some(id)
            && row.kind == allow_report::FinalFreezeReplayRowKindV1::Mismatch
    }) {
        return Err("replay did not independently recompute the changed experience member".into());
    }
    Ok(())
}

impl OriginalBundle {
    fn predecessors(&self) -> ExperienceResult<[serde_json::Value; 3]> {
        let read = |id| -> ExperienceResult<serde_json::Value> {
            let (_, member) = self.readback.member(id)?.ok_or("predecessor missing")?;
            Ok(serde_json::from_slice(&member.bytes)?)
        };
        Ok([
            read("experience:package-candidate")?,
            read("experience:isolated-install")?,
            read("experience:exact-candidate")?,
        ])
    }

    // Fixture-only mutation reseals every downstream reference so the actual
    // admission defect, rather than an unrelated stale digest, is the oracle.
    fn reseal_predecessors(
        &mut self,
        [candidate, mut install, mut journey]: [serde_json::Value; 3],
    ) -> ExperienceResult {
        let bytes = |value: &serde_json::Value| -> ExperienceResult<Vec<u8>> {
            let mut bytes = serde_json::to_vec_pretty(value)?;
            bytes.push(b'\n');
            Ok(bytes)
        };
        self.input.candidate_digest = self.put(
            "experience:package-candidate",
            "ExperienceReference",
            bytes(&candidate)?,
        )?;
        install
            .as_object_mut()
            .ok_or("install object missing")?
            .insert(
                "candidate_artifact_digest".to_string(),
                serde_json::Value::String(self.input.candidate_digest.clone()),
            );
        self.input.install_digest = self.put(
            "experience:isolated-install",
            "ExperienceReference",
            bytes(&install)?,
        )?;
        let journey_object = journey.as_object_mut().ok_or("journey object missing")?;
        journey_object.insert(
            "candidate_artifact_digest".to_string(),
            serde_json::Value::String(self.input.candidate_digest.clone()),
        );
        journey_object.insert(
            "isolated_install_receipt_digest".to_string(),
            serde_json::Value::String(self.input.install_digest.clone()),
        );
        self.input.journey_digest = self.put(
            "experience:exact-candidate",
            "ExperienceReference",
            bytes(&journey)?,
        )?;
        self.pair()
    }
}

#[test]
fn predecessor_unknown_fields_are_refused_at_every_existing_object_shape() -> ExperienceResult {
    for (label, predecessor, pointer) in [
        ("package candidate", 0, ""),
        ("package candidate rows[0]", 0, "/rows/0"),
        (
            "package candidate rows[0] expected_dependency_rows[0]",
            0,
            "/rows/0/expected_dependency_rows/0",
        ),
        ("isolated install", 1, ""),
        ("isolated install package_rows[0]", 1, "/package_rows/0"),
        ("isolated install graph_comparison", 1, "/graph_comparison"),
        ("exact candidate", 2, ""),
        ("exact candidate package_rows[0]", 2, "/package_rows/0"),
        ("exact candidate journey_steps[0]", 2, "/journey_steps/0"),
    ] {
        let mut original = bundle()?;
        let mut predecessors = original.predecessors()?;
        let candidate_row = predecessors
            .get_mut(0)
            .and_then(|candidate| candidate.get_mut("rows"))
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|rows| rows.first_mut())
            .and_then(serde_json::Value::as_object_mut)
            .ok_or("selected candidate row object missing")?;
        candidate_row.insert(
            "expected_dependency_rows".to_string(),
            serde_json::json!([{
                "package_name": "serde", "package_version": "1", "dependency_kind": "external"
            }]),
        );
        original.reseal_predecessors(predecessors)?;
        let before = original.admission()?;
        if before.result != State::NotProven || before.notes.len() != 3 {
            return Err(format!("valid field-control baseline failed: {:?}", before.notes).into());
        }
        let mut predecessors = original.predecessors()?;
        predecessors
            .get_mut(predecessor)
            .and_then(|value| value.pointer_mut(pointer))
            .and_then(serde_json::Value::as_object_mut)
            .ok_or("selected predecessor object missing")?
            .insert(
                "unrecognized_observation".to_string(),
                serde_json::json!(true),
            );
        original.reseal_predecessors(predecessors)?;
        original.readback.validate(&original.subject)?;
        assert_admission(
            &original,
            State::Malformed,
            &format!("{label} original has unknown field unrecognized_observation"),
        )?;
    }
    Ok(())
}

#[test]
fn predecessor_optional_null_empty_and_omitted_fields_keep_existing_semantics() -> ExperienceResult
{
    for explicit in [false, true] {
        let mut original = bundle()?;
        let mut predecessors = original.predecessors()?;
        for (predecessor, pointer, field) in [
            (0, "", "topology_digest"),
            (1, "/package_rows/0", "resolved_version"),
            (2, "/journey_steps/0", "artifact_schema_id"),
        ] {
            let object = predecessors
                .get_mut(predecessor)
                .and_then(|value| value.pointer_mut(pointer))
                .and_then(serde_json::Value::as_object_mut)
                .ok_or("selected optional-field object missing")?;
            if explicit {
                object.insert(field.to_string(), serde_json::Value::Null);
            } else {
                object.remove(field);
            }
        }
        let journey = predecessors
            .get_mut(2)
            .and_then(serde_json::Value::as_object_mut)
            .ok_or("journey object missing")?;
        if explicit {
            journey.insert("not_included".to_string(), serde_json::json!([]));
        } else {
            journey.remove("not_included");
        }
        // These existing graph arrays allow empty values but have no serde
        // default. Keep them present; omission is not an accepted alternative.
        let graph = predecessors
            .get_mut(1)
            .and_then(|install| install.get_mut("graph_comparison"))
            .and_then(serde_json::Value::as_object_mut)
            .ok_or("install graph comparison object missing")?;
        for field in [
            "unexpected_packages",
            "missing_packages",
            "version_mismatches",
            "path_sources",
        ] {
            graph.insert(field.to_string(), serde_json::json!([]));
        }
        original.reseal_predecessors(predecessors)?;
        let before = original
            .readback
            .custody(&original.subject, "optional-originals")?;
        let admission = original.admission()?;
        let after = original
            .readback
            .custody(&original.subject, "optional-originals")?;
        if admission.result != State::NotProven || admission.notes.len() != 3 || before != after {
            return Err(format!(
                "allowed optional values or original bytes changed: {:?}",
                admission.notes
            )
            .into());
        }
        let (custody, retained) = after;
        for id in [
            "experience:package-candidate",
            "experience:isolated-install",
            "experience:exact-candidate",
        ] {
            let (_, member) = original.readback.member(id)?.ok_or("original missing")?;
            let retained = retained
                .iter()
                .find(|item| item.artifact_id == id)
                .ok_or("original not retained")?;
            let item = custody
                .items
                .iter()
                .find(|item| item.artifact_id == id)
                .ok_or("original custody missing")?;
            if retained.declared_sha256 != sha256_v1_bytes(&member.bytes)
                || retained.bytes != RetainedArtifactBytesV1::new(member.bytes.clone())
                || !item.storage_locator.contains("/505/")
            {
                return Err("noncanonical original bytes lost their exact custody identity".into());
            }
        }
    }
    Ok(())
}

#[test]
fn checked_readback_keeps_pair_defects_distinct_from_provider_unavailability() -> ExperienceResult {
    for (case, expected) in [
        ("missing", State::Incomplete),
        ("duplicate", State::Malformed),
        ("malformed", State::Malformed),
    ] {
        let mut original = bundle()?;
        if case == "malformed" {
            let mut input = serde_json::to_value(&original.input)?;
            input
                .as_object_mut()
                .ok_or("experience input object missing")?
                .insert(
                    "maximum_age_seconds".to_string(),
                    serde_json::json!("not an integer"),
                );
            original.put(
                "evidence:experience-input",
                "Evidence:experience-input",
                serde_json::to_vec(&input)?,
            )?;
        }
        original.readback.validate(&original.subject)?;
        let mut evidence = original.evidence()?;
        match case {
            "missing" => {
                evidence.retain(|item| item.role != FreezeEvidenceRole::ReleaseExperienceInput);
            }
            "duplicate" => {
                evidence.push(original.evidence()?.pop().ok_or("result missing")?);
            }
            _ => {}
        }
        let selection = command::tests::selection();
        let current = allow_report::ObservationReadingV1 {
            freshness: allow_report::ObservationFreshnessV1::Current,
            detail: "explicit diagnostic control".to_string(),
        };
        let mut graph = command::build_evidence_graph(
            &original.subject,
            &selection,
            &evidence,
            &original.packages,
            None,
            (FinalEvidenceNodeResultV1::Complete, &current),
        );
        let mut decisions =
            command::readiness_decision_inputs(&original.subject, &selection, &evidence);
        decisions.remaining_reversible_work.clear();
        // Explicit unit fixtures fix unrelated gates and establish the positive
        // baseline. The actual admission below restores both required defects.
        for node in &mut graph.nodes {
            node.result = FinalEvidenceNodeResultV1::Complete;
            node.currentness = FinalEvidenceCurrentnessV1::Current;
        }
        if aggregate_final_readiness(&graph, &decisions).verdict
            != FinalReadinessVerdictV1::ReadyForFreeze
        {
            return Err("diagnostic unrelated-gate baseline was not ready".into());
        }
        for readback in [Some(&original.readback), None] {
            let admission = command::experience::reconcile(
                &original.subject,
                &original.packages,
                &evidence,
                readback,
            );
            let expected_currentness = if readback.is_some() {
                FinalEvidenceCurrentnessV1::Current
            } else {
                FinalEvidenceCurrentnessV1::ProviderUnavailable
            };
            if admission.result != expected || admission.currentness != expected_currentness {
                return Err(
                    format!("pair defect lost its independent readback context: {case}").into(),
                );
            }
            let mut affected = graph.clone();
            command::experience::apply(&mut affected, &admission);
            let readiness = aggregate_final_readiness(&affected, &decisions);
            for id in ["release-experience-input", "release-experience"] {
                let node = affected
                    .nodes
                    .iter()
                    .find(|node| node.evidence_id == id)
                    .ok_or("required experience node missing")?;
                if !node.required
                    || node.authority_scope
                        != allow_report::FinalEvidenceAuthorityScopeV1::FinalExact
                    || !affected
                        .required_node_ids
                        .iter()
                        .any(|required| required == id)
                    || node.result != admission.graph_result()
                    || node.currentness != expected_currentness
                    || (readback.is_some()
                        && readiness.rows.iter().any(|row| {
                            row.evidence_id.as_deref() == Some(id)
                                && row.kind
                                    == allow_report::FinalReadinessRowKindV1::ProviderUnavailable
                        }))
                {
                    return Err(format!(
                        "required row acquired a false provider outage: {case}/{id}"
                    )
                    .into());
                }
            }
            let input = readiness
                .rows
                .iter()
                .find(|row| row.evidence_id.as_deref() == Some("release-experience-input"))
                .ok_or("direct input readiness row missing")?;
            let (kind, action) = if readback.is_some() {
                (
                    allow_report::FinalReadinessRowKindV1::MissingEvidence,
                    "produce the exact evidence result on the selected subject",
                )
            } else {
                (
                    allow_report::FinalReadinessRowKindV1::ProviderUnavailable,
                    "restore provider access and re-observe through the exact producer",
                )
            };
            if input.kind != kind || input.next_action != action {
                return Err(format!("wrong direct pair-defect action: {case}/{input:?}").into());
            }
            // The dependent result may remain transitively Stale. Do not
            // change aggregate precedence or graph dependency semantics.
        }
    }
    Ok(())
}
