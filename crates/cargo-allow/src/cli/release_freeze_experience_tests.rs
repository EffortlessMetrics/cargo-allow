//! Original-bundle admission controls. All provider records and opaque pilot/
//! docs bytes here are explicit fixtures, never evidence of an executed pilot.

use super::super::super as command;
use super::*;
use allow_report::{
    CargoAllowReleaseDocsExecutionEvaluationV1, CargoAllowReleaseExperienceV1,
    ExactCandidatePayloadV2, ExactCandidateResultV2, FinalEvidenceCurrentnessV1,
    FinalEvidenceNodeResultV1, FinalEvidencePackageRoleV1, FinalEvidencePackageSubjectV1,
    FinalReadinessVerdictV1, IsolatedInstallPayloadV2, IsolatedInstallResultV2,
    PackageCandidatePayloadV2, PackageCandidateResultV2, RELEASE_EXPERIENCE_REQUIRED_DOCS,
    ReleaseExperienceBrownfieldPostureV1, ReleaseExperienceDocsCheckKindV1,
    ReleaseExperienceDocsCheckOutcomeV1, ReleaseExperienceDocsCheckV1,
    ReleaseExperienceDocsExecutionClaimV1, ReleaseExperienceDocsExecutionV1,
    ReleaseExperienceDocsIdentityV1, ReleaseExperienceFrictionDispositionV1,
    ReleaseExperienceFrictionV1, ReleaseExperienceInputV1, ReleaseExperiencePilotResultV1,
    ReleaseExperiencePilotV1, ReleaseExperienceResultV1,
    evaluate_release_experience_docs_execution_v1, evaluate_release_experience_v1,
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
        "platform":"x86_64-unknown-linux-gnu", "toolchain":"1.95.0",
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
        "toolchain":"1.95.0", "support_matrix_generation":"selected-support",
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
            docs_execution: None,
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
        // This already-admitted synthetic peer connects the required registry
        // node through the existing SupportsOnly edge. It supplies no producer
        // evidence for the experience admission below.
        let role = FreezeEvidenceRole::PackageDocs;
        evidence.push(EvidenceInput {
            role,
            path: std::path::PathBuf::from("synthetic-peer.json"),
            sha256: sha256_v1_bytes(role.label().as_bytes()),
            value: serde_json::Value::Null,
            binding_notes: Vec::new(),
        });
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
        let baseline = aggregate_final_readiness(&graph, &decisions);
        if baseline.verdict != FinalReadinessVerdictV1::ReadyForFreeze {
            return Err(format!(
                "diagnostic unrelated-gate baseline was not ready: {:?}",
                baseline.rows
            )
            .into());
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

// ---- Installed executed-documentation admission (#4706) ----

/// Receipt-free positional accessor over recorded docs checks: no bracket
/// indexing, no `unwrap`.
fn docs_check_at<'a>(
    checks: &'a mut [ReleaseExperienceDocsCheckV1],
    index: usize,
    label: &'static str,
) -> &'a mut ReleaseExperienceDocsCheckV1 {
    checks
        .get_mut(index)
        .unwrap_or_else(|| std::panic::panic_any(label))
}

fn docs_check(
    id: &str,
    docs_name: &str,
    kind: ReleaseExperienceDocsCheckKindV1,
) -> ReleaseExperienceDocsCheckV1 {
    ReleaseExperienceDocsCheckV1 {
        id: id.to_string(),
        docs_name: docs_name.to_string(),
        kind,
        command: None,
        argv: Vec::new(),
        exit_code: None,
        expected_exit_code: None,
        stdout_excerpt: String::new(),
        stderr_excerpt: String::new(),
        declared_writes: Vec::new(),
        observed_writes: Vec::new(),
        documented_claim: "the shipped docs advertise this surface".to_string(),
        observed_summary: "observed agreement on the selected installed product".to_string(),
        outcome: ReleaseExperienceDocsCheckOutcomeV1::Matched,
    }
}

fn executed_docs_check(
    id: &str,
    docs_name: &str,
    argv: &[&str],
    stdout: &str,
) -> ReleaseExperienceDocsCheckV1 {
    let command_head = argv
        .first()
        .unwrap_or_else(|| std::panic::panic_any("executed docs check requires an argv head"))
        .to_string();
    ReleaseExperienceDocsCheckV1 {
        kind: ReleaseExperienceDocsCheckKindV1::SurfaceGeneration,
        command: Some(command_head),
        argv: argv.iter().map(|argument| argument.to_string()).collect(),
        exit_code: Some(0),
        expected_exit_code: Some(0),
        stdout_excerpt: stdout.to_string(),
        ..docs_check(
            id,
            docs_name,
            ReleaseExperienceDocsCheckKindV1::SurfaceGeneration,
        )
    }
}

impl OriginalBundle {
    /// Internally consistent observations carrying this bundle's installed
    /// identity. The v1 record still lacks a checked execution oracle.
    fn docs_execution_record(&self) -> ReleaseExperienceDocsExecutionV1 {
        let mut checks = vec![
            executed_docs_check(
                "E.help.entrypoint",
                "help",
                &["--help"],
                "Usage: cargo-allow help [COMMAND] init audit check",
            ),
            executed_docs_check(
                "E.reference.entrypoint",
                "command-registry",
                &["reference", "--format", "json"],
                r#"{"schema":"cargo-allow.cli-reference.v1","commands":["reference"]}"#,
            ),
            executed_docs_check(
                "E.man.entrypoint",
                "manpage",
                &["reference", "--format", "man"],
                ".TH generated by reference; .B cargo-allow check",
            ),
            executed_docs_check(
                "E.completions.entrypoint",
                "completion",
                &["completions", "bash"],
                "cargo__allow__subcmd__completions() { ... }",
            ),
            ReleaseExperienceDocsCheckV1 {
                kind: ReleaseExperienceDocsCheckKindV1::SnippetExecution,
                command: Some("check".to_string()),
                argv: vec![
                    "check".to_string(),
                    "--mode".to_string(),
                    "no-new".to_string(),
                ],
                exit_code: Some(0),
                expected_exit_code: Some(0),
                stdout_excerpt: "**Result:** passed (enforcing)".to_string(),
                declared_writes: vec![
                    "policy/allow.toml".to_string(),
                    "target/cargo-allow/check.receipt.json".to_string(),
                ],
                observed_writes: vec!["policy/allow.toml".to_string()],
                documented_claim: "the shipped docs advertise this command".to_string(),
                observed_summary: "executed the documented journey to its green gate".to_string(),
                outcome: ReleaseExperienceDocsCheckOutcomeV1::Matched,
                id: "docs.first-hour.journey-green".to_string(),
                docs_name: "getting-started".to_string(),
                stderr_excerpt: String::new(),
            },
            docs_check(
                "docs.channel.version-claim",
                "channel",
                ReleaseExperienceDocsCheckKindV1::ClaimInspection,
            ),
            docs_check(
                "docs.support.rust-floor",
                "support-matrix",
                ReleaseExperienceDocsCheckKindV1::ClaimInspection,
            ),
            docs_check(
                "E.package.entrypoint.readme",
                "readme",
                ReleaseExperienceDocsCheckKindV1::ClaimInspection,
            ),
        ];
        for claim_check in checks.iter_mut().skip(5).take(3) {
            claim_check.observed_summary =
                "observed installed metadata agrees with the documented claim".to_string();
        }
        // The root help surface's argv head is `--help`; the advertised
        // command the output must name is `help`.
        docs_check_at(&mut checks, 0, "help surface check present").command =
            Some("help".to_string());
        ReleaseExperienceDocsExecutionV1 {
            schema_id: "cargo-allow.release-experience-docs-execution.v1".to_string(),
            schema_version: 1,
            binary_digest: self.input.binary_digest.clone(),
            installed_version_output: format!("cargo-allow {}", self.subject.version),
            invocation_path: self.input.invocation_path.clone(),
            platform: "x86_64-unknown-linux-gnu".to_string(),
            toolchain: "1.95.0".to_string(),
            channel_claim: "stable".to_string(),
            version_claim: format!("cargo-allow {}", self.subject.version),
            rust_floor_claim: "1.95".to_string(),
            checks,
            limitations: Vec::new(),
            claim_boundary: "installed documentation observations only; no release authorization"
                .to_string(),
            observed_at_unix_seconds: self.input.observed_at_unix_seconds,
            maximum_age_seconds: self.input.maximum_age_seconds,
        }
    }

    /// Install the record as an original member, point the input's claim at
    /// it, and reseal the original pair.
    fn install_docs_execution(
        &mut self,
        record: &ReleaseExperienceDocsExecutionV1,
        claim_result: State,
    ) -> ExperienceResult<()> {
        let bytes = serde_json::to_vec(record)?;
        let record_digest = self.put("experience:docs-execution", "ExperienceReference", bytes)?;
        self.input.docs_execution = Some(ReleaseExperienceDocsExecutionClaimV1 {
            record_digest,
            result: claim_result,
        });
        self.pair()?;
        Ok(())
    }
}

#[test]
fn consistent_docs_observations_retain_all_required_holds() -> ExperienceResult {
    let mut original = bundle()?;
    let record = original.docs_execution_record();
    let evaluation: CargoAllowReleaseDocsExecutionEvaluationV1 =
        evaluate_release_experience_docs_execution_v1(&record);
    if evaluation.result != State::NotProven {
        return Err(format!(
            "consistent fixture observations became proof: {:?}",
            evaluation.gaps
        )
        .into());
    }
    original.install_docs_execution(&record, State::NotProven)?;
    let admission = original.admission()?;
    if admission.result != State::NotProven {
        return Err(format!(
            "missing producers must still keep NotProven: {:?}",
            admission.notes
        )
        .into());
    }
    for owner in ["#2466", "#3149", "#3151"] {
        if !admission.notes.iter().any(|note| note.contains(owner)) {
            return Err(format!("independent hold {owner} was silently dropped").into());
        }
    }
    if !admission
        .notes
        .iter()
        .any(|note| note.contains("selected-case producer"))
    {
        return Err(format!(
            "missing documentation proof lost its reason: {:?}",
            admission.notes
        )
        .into());
    }
    Ok(())
}

#[test]
fn docs_execution_defects_keep_the_hold_with_named_reasons() -> ExperienceResult {
    for (label, defect, expected, fragment) in [
        (
            "missing record member",
            "missing_record",
            State::Incomplete,
            "missing original experience member experience:docs-execution",
        ),
        (
            "changed record bytes",
            "changed_bytes",
            State::Mismatch,
            "role or exact byte digest",
        ),
        (
            "forged complete claim over an honest disagreement",
            "forged_claim",
            State::Mismatch,
            "differs from the computed reader result",
        ),
        (
            "honest incomplete claim stays below Complete",
            "honest_incomplete",
            State::Incomplete,
            "docs execution:",
        ),
        (
            "undeclared write",
            "undeclared_write",
            State::Mismatch,
            "undeclared write",
        ),
        (
            "foreign installed executable",
            "foreign_binary",
            State::Mismatch,
            "different installed executable",
        ),
        (
            "wrong subject version claim",
            "wrong_version",
            State::Mismatch,
            "version or channel claim",
        ),
        (
            "wrong subject channel claim",
            "wrong_channel",
            State::Mismatch,
            "channel claim differs",
        ),
        (
            "unparseable Rust floor",
            "bad_floor",
            State::Malformed,
            "rust floor claim is not a comparable version",
        ),
        (
            "Cargo version does not identify a compiler",
            "bad_toolchain",
            State::Malformed,
            "toolchain does not identify a comparable Rust compiler",
        ),
        (
            "compiler older than claimed floor",
            "old_toolchain",
            State::Mismatch,
            "exceeds the installed toolchain",
        ),
        (
            "foreign installed platform",
            "foreign_platform",
            State::Mismatch,
            "platform or toolchain differs from the selected install",
        ),
        (
            "foreign installed compiler",
            "foreign_toolchain",
            State::Mismatch,
            "platform or toolchain differs from the selected install",
        ),
        (
            "stale record",
            "stale_record",
            State::Stale,
            "docs execution record is future-dated or expired",
        ),
        (
            "unbound docs identity",
            "unbound_identity",
            State::Mismatch,
            "does not bind a selected documentation identity",
        ),
        (
            "unknown record field",
            "unknown_field",
            State::Malformed,
            "docs execution original has unknown field extra_observation",
        ),
    ] {
        let mut original = bundle()?;
        match defect {
            "missing_record" => {
                let record = original.docs_execution_record();
                original.input.docs_execution = Some(ReleaseExperienceDocsExecutionClaimV1 {
                    record_digest: digest(&serde_json::to_vec(&record)?),
                    result: State::Complete,
                });
                original.pair()?;
            }
            "changed_bytes" => {
                let mut record = original.docs_execution_record();
                record.platform = "changed-platform".to_string();
                original.install_docs_execution(&record, State::NotProven)?;
                let (_, bytes) = original
                    .readback
                    .experience_member("experience:docs-execution")?
                    .ok_or("record missing")?;
                let mut value: serde_json::Value = serde_json::from_slice(bytes)?;
                let Some(object) = value.as_object_mut() else {
                    std::panic::panic_any("changed record bytes must stay an object");
                };
                object.insert(
                    "toolchain".to_string(),
                    serde_json::json!("changed-toolchain"),
                );
                original.put(
                    "experience:docs-execution",
                    "ExperienceReference",
                    serde_json::to_vec(&value)?,
                )?;
            }
            "forged_claim" => {
                let mut record = original.docs_execution_record();
                docs_check_at(&mut record.checks, 0, "help surface check present").outcome =
                    ReleaseExperienceDocsCheckOutcomeV1::Mismatched;
                docs_check_at(&mut record.checks, 0, "help surface check present")
                    .observed_summary =
                    "the documented group headers are absent from the installed help".to_string();
                original.install_docs_execution(&record, State::Complete)?;
            }
            "honest_incomplete" => {
                let mut record = original.docs_execution_record();
                docs_check_at(&mut record.checks, 7, "readme claim check present").outcome =
                    ReleaseExperienceDocsCheckOutcomeV1::NotExecuted;
                docs_check_at(&mut record.checks, 7, "readme claim check present")
                    .observed_summary =
                    "rustdoc cannot be executed by the docs producer".to_string();
                original.install_docs_execution(&record, State::Incomplete)?;
            }
            "undeclared_write" => {
                let mut record = original.docs_execution_record();
                docs_check_at(&mut record.checks, 4, "journey check present")
                    .observed_writes
                    .push("untracked/leak.txt".to_string());
                original.install_docs_execution(&record, State::Complete)?;
            }
            "foreign_binary" => {
                let mut record = original.docs_execution_record();
                record.binary_digest = digest(b"a different installed executable");
                original.install_docs_execution(&record, State::Complete)?;
            }
            "wrong_version" => {
                let mut record = original.docs_execution_record();
                record.version_claim = "cargo-allow 9.9.9".to_string();
                original.install_docs_execution(&record, State::Complete)?;
            }
            "wrong_channel" | "bad_floor" | "bad_toolchain" | "old_toolchain"
            | "foreign_platform" | "foreign_toolchain" => {
                let mut record = original.docs_execution_record();
                match defect {
                    "wrong_channel" => record.channel_claim = "public-prerelease".to_string(),
                    "bad_floor" => record.rust_floor_claim = "anything".to_string(),
                    "bad_toolchain" => record.toolchain = "cargo 1.50.0 stable-host".to_string(),
                    "old_toolchain" => record.toolchain = "1.94.0".to_string(),
                    "foreign_platform" => record.platform = "another-platform".to_string(),
                    _ => record.toolchain = "1.96.0".to_string(),
                }
                original.install_docs_execution(&record, State::NotProven)?;
            }
            "stale_record" => {
                let mut record = original.docs_execution_record();
                record.observed_at_unix_seconds = original.input.observed_at_unix_seconds - 10_000;
                original.install_docs_execution(&record, State::NotProven)?;
            }
            "unbound_identity" => {
                let mut record = original.docs_execution_record();
                docs_check_at(&mut record.checks, 0, "help surface check present").docs_name =
                    "blog".to_string();
                original.install_docs_execution(&record, State::Complete)?;
            }
            _ => {
                let record = original.docs_execution_record();
                let mut value = serde_json::to_value(&record)?;
                let Some(object) = value.as_object_mut() else {
                    std::panic::panic_any("record JSON must stay an object");
                };
                object.insert("extra_observation".to_string(), serde_json::json!(true));
                let record_digest = original.put(
                    "experience:docs-execution",
                    "ExperienceReference",
                    serde_json::to_vec(&value)?,
                )?;
                original.input.docs_execution = Some(ReleaseExperienceDocsExecutionClaimV1 {
                    record_digest,
                    result: State::Complete,
                });
                original.pair()?;
            }
        }
        original.readback.validate(&original.subject)?;
        let admission = original.admission()?;
        if !admission.notes.iter().any(|note| note.contains("#3151")) {
            return Err(format!(
                "defect {label} discharged the documentation hold: {:?}",
                admission.notes
            )
            .into());
        }
        if !admission.notes.iter().any(|note| note.contains(fragment)) {
            return Err(format!(
                "defect {label} lost its named reason: {:?}",
                admission.notes
            )
            .into());
        }
        if admission.result > expected {
            return Err(format!(
                "defect {label} read {:?} but needed at most {expected:?}",
                admission.result
            )
            .into());
        }
    }
    Ok(())
}

#[test]
fn docs_execution_compound_defects_preserve_severity_and_all_reasons() -> ExperienceResult {
    for (label, duplicate, mismatch, freshness, claim, expected) in [
        (
            "malformed with understated claim",
            true,
            false,
            "current",
            State::NotProven,
            State::Malformed,
        ),
        (
            "malformed with Complete claim",
            true,
            false,
            "current",
            State::Complete,
            State::Malformed,
        ),
        (
            "malformed freshness window",
            false,
            false,
            "zero_window",
            State::Malformed,
            State::Malformed,
        ),
        (
            "mismatch and expired",
            false,
            true,
            "expired",
            State::Mismatch,
            State::Mismatch,
        ),
        (
            "mismatch and future-dated",
            false,
            true,
            "future",
            State::Mismatch,
            State::Mismatch,
        ),
        (
            "forged Complete claim and expired",
            false,
            false,
            "expired",
            State::Complete,
            State::Mismatch,
        ),
        (
            "malformed with Complete claim and expired",
            true,
            false,
            "expired",
            State::Complete,
            State::Malformed,
        ),
    ] {
        let mut original = bundle()?;
        let mut record = original.docs_execution_record();
        if duplicate {
            let check = record.checks.first().ok_or("docs check absent")?.clone();
            record.checks.push(check);
        }
        if mismatch {
            let check = record.checks.first_mut().ok_or("docs check absent")?;
            check.outcome = ReleaseExperienceDocsCheckOutcomeV1::Mismatched;
            check.observed_summary = "installed help disagrees with the documentation".to_string();
        }
        match freshness {
            "zero_window" => record.maximum_age_seconds = 0,
            "expired" => record.observed_at_unix_seconds -= 10_000,
            "future" => record.observed_at_unix_seconds += 10_000,
            _ => {}
        }
        let evaluated = evaluate_release_experience_docs_execution_v1(&record);
        original.install_docs_execution(&record, claim)?;
        original.readback.validate(&original.subject)?;
        let admission = original.admission()?;
        if admission.result != expected {
            return Err(format!(
                "{label}: compound docs defects became {:?}, expected {expected:?}: {:?}",
                admission.result, admission.notes
            )
            .into());
        }
        let mut reasons = vec!["#3151"];
        if duplicate {
            reasons.push("duplicate documentation check");
        }
        if mismatch {
            reasons.push("installed help disagrees with the documentation");
        }
        if freshness == "zero_window" {
            reasons.push("positive observation time and freshness window");
        }
        if freshness != "current" {
            reasons.push("docs execution record is future-dated or expired");
        }
        if evaluated.result != claim {
            reasons.push("differs from the computed reader result");
        }
        for reason in reasons {
            if !admission.notes.iter().any(|note| note.contains(reason)) {
                return Err(format!("{label}: lost {reason:?}: {:?}", admission.notes).into());
            }
        }
    }
    Ok(())
}

#[test]
fn complete_model_input_cannot_override_a_failed_docs_observation() -> ExperienceResult {
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
    let mut record = original.docs_execution_record();
    docs_check_at(&mut record.checks, 0, "help surface check present").outcome =
        ReleaseExperienceDocsCheckOutcomeV1::Mismatched;
    docs_check_at(&mut record.checks, 0, "help surface check present").observed_summary =
        "the installed first screen is an ungrouped flat command list".to_string();
    original.install_docs_execution(&record, State::Complete)?;
    if evaluate_release_experience_v1(&original.input).result != State::Complete {
        return Err("forged model-level Complete control did not reach the boundary".into());
    }
    let admission = original.admission()?;
    for owner in ["#2466", "#3151"] {
        if !admission.notes.iter().any(|note| note.contains(owner)) {
            return Err(format!(
                "model Complete waived the {owner} hold over failed docs evidence"
            )
            .into());
        }
    }
    if !admission
        .notes
        .iter()
        .any(|note| note.contains("differs from the computed reader result"))
    {
        return Err(format!("forged docs claim was not caught: {:?}", admission.notes).into());
    }
    Ok(())
}

#[test]
fn docs_execution_claim_without_a_checked_readback_keeps_the_hold() -> ExperienceResult {
    let mut original = bundle()?;
    let record = original.docs_execution_record();
    original.install_docs_execution(&record, State::Complete)?;
    let local = command::experience::reconcile(
        &original.subject,
        &original.packages,
        &original.evidence()?,
        None,
    );
    if local.result != State::NotProven
        || !local.notes.iter().any(|note| note.contains("#3151"))
        || local.currentness != FinalEvidenceCurrentnessV1::ProviderUnavailable
    {
        return Err("an unchecked readback admitted docs execution without a clock".into());
    }
    Ok(())
}

#[test]
fn docs_execution_prose_only_or_consistent_complete_claims_keep_the_hold() -> ExperienceResult {
    for prose_only in [false, true] {
        let mut original = bundle()?;
        let mut record = original.docs_execution_record();
        if prose_only {
            record.checks = RELEASE_EXPERIENCE_REQUIRED_DOCS
                .iter()
                .map(|name| {
                    docs_check(
                        &format!("unexecuted-{name}"),
                        name,
                        ReleaseExperienceDocsCheckKindV1::ClaimInspection,
                    )
                })
                .collect();
        }
        original.install_docs_execution(&record, State::Complete)?;
        let admission = original.admission()?;
        if admission.result != State::Mismatch
            || !admission.notes.iter().any(|note| note.contains("#3151"))
            || !admission
                .notes
                .iter()
                .any(|note| note.contains("differs from the computed reader result"))
        {
            return Err(format!("unproved Complete claim escaped: {:?}", admission.notes).into());
        }
    }
    Ok(())
}

#[test]
fn docs_execution_reference_or_freshness_defects_cannot_discharge_the_hold() -> ExperienceResult {
    for defect in [
        "changed_docs",
        "replaced_docs",
        "stale_input",
        "missing_predecessor",
    ] {
        let mut original = bundle()?;
        let record = original.docs_execution_record();
        let expected = match defect {
            "changed_docs" | "replaced_docs" => {
                let digest = original.put(
                    "experience:docs:help",
                    "ExperienceReference",
                    b"different arbitrary documentation bytes".to_vec(),
                )?;
                if defect == "replaced_docs" {
                    original
                        .input
                        .docs_identities
                        .iter_mut()
                        .find(|identity| identity.name == "help")
                        .ok_or("help identity absent")?
                        .digest = digest;
                    State::NotProven
                } else {
                    State::Mismatch
                }
            }
            "stale_input" => {
                original.input.observed_at_unix_seconds -= 10_000;
                State::Stale
            }
            _ => {
                original
                    .readback
                    .artifacts
                    .first_mut()
                    .ok_or("artifact absent")?
                    .members
                    .retain(|member| member.logical_id != "experience:package-candidate");
                original.refresh()?;
                State::Incomplete
            }
        };
        original.install_docs_execution(&record, State::NotProven)?;
        let admission = original.admission()?;
        if admission.result != expected
            || !admission.notes.iter().any(|note| note.contains("#3151"))
        {
            return Err(format!(
                "{defect} lost its result or docs hold: {:?}",
                admission.notes
            )
            .into());
        }
    }
    Ok(())
}

#[test]
fn docs_execution_source_candidate_is_not_a_published_channel_claim() -> ExperienceResult {
    let mut original = bundle()?;
    let mut record = original.docs_execution_record();
    record.channel_claim = "source-candidate".to_string();
    original.install_docs_execution(&record, State::NotProven)?;
    let admission = original.admission()?;
    if admission.result != State::NotProven
        || !admission.notes.iter().any(|note| note.contains("#3151"))
    {
        return Err(format!(
            "valid source-candidate channel was rejected: {:?}",
            admission.notes
        )
        .into());
    }
    Ok(())
}

#[test]
fn retained_rc1_installed_docs_execution_admits_its_named_disagreements() -> ExperienceResult {
    // The retained public-prerelease execution record: one selected isolated
    // install of the published 0.2.0-rc.1 crate, executed by this lane, with
    // the exact binary identity below. Retaining it here keeps the executed
    // evidence reviewable with its exact source; it is not final installed
    // qualification and grants no release authority.
    const RECORD: &str =
        include_str!("../../../../docs/release/evidence/rc1-installed-docs-execution.v1.json");
    let record: ReleaseExperienceDocsExecutionV1 = serde_json::from_str(RECORD)?;
    if record.binary_digest
        != "sha256:71bfd25a06dfa8fb14c7ee820ed9bce7f30ee84feef71a483c09274e3e7df86c"
        || record.installed_version_output != "cargo-allow 0.2.0-rc.1"
        || record.version_claim != "cargo-allow 0.2.0-rc.1"
        || record.rust_floor_claim != "1.95"
    {
        return Err("retained rc.1 record lost its exact installed identity".into());
    }
    let evaluation = evaluate_release_experience_docs_execution_v1(&record);
    // The incident-bearing prerelease honestly fails seven E-family checks
    // and leaves the package rustdoc surface unexecuted, so the record can
    // never read Complete and the documentation hold stays for this channel.
    if evaluation.result != State::Mismatch {
        return Err(format!(
            "retained rc.1 docs execution read {:?} instead of an honest Mismatch: {:?}",
            evaluation.result, evaluation.gaps
        )
        .into());
    }
    for fragment in [
        "E.help.entrypoint",
        "E.completions.entrypoint",
        "E.help.extraction_leakage",
        "E.help.package_help_mismatch",
        "E.package.entrypoint",
        "was not executed",
    ] {
        if !evaluation.gaps.iter().any(|gap| gap.contains(fragment)) {
            return Err(format!(
                "retained rc.1 record lost the named disagreement {fragment}: {:?}",
                evaluation.gaps
            )
            .into());
        }
    }
    let executed = record
        .checks
        .iter()
        .filter(|check| check.outcome == ReleaseExperienceDocsCheckOutcomeV1::Matched)
        .count();
    if executed < 20 {
        return Err(format!(
            "retained rc.1 record lost its positive matched executions: {executed}"
        )
        .into());
    }
    Ok(())
}
