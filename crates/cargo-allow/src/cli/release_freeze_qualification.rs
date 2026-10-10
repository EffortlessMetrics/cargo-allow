//! Computational half of the authenticated freeze qualifier (#2501).
//!
//! The Python producer performs provider reads. This private transport shape
//! contains existing records and exact bytes; it is not an authority schema or
//! a reusable provider attestation. Invoking this hidden command directly can
//! only compute a result. A saved successful result cannot replace fresh reads.

use std::collections::BTreeSet;
use std::path::Path;

use allow_core::{CargoAllowResult, SimpleDate, sha256_v1_bytes};
use allow_report::{
    ActualDownloadedFileV1, ArtifactTransferDispositionV1, CandidateCustodyInitV1,
    CargoAllowFinalFreezeReplayInputsV1, CargoAllowFrozenCandidateCustodyV1,
    CargoAllowPostMergeQualificationV1, CargoAllowReleaseArtifactTransferV1,
    ConfidentialityClassV1, ConsumerContextV1, CustodyDispositionV1, CustodyFileV1,
    FinalEvidenceGraphV1, FinalEvidenceProducerExpectationV1, FinalEvidenceProducerV1,
    FinalReadinessCustodyPostureV1, FinalReadinessPostMergePostureV1,
    FinalReadinessQualificationPostureV1, FinalReadinessVerdictV1,
    PostMergeEquivalenceVerdictV1, ProducerIdentityV1, RetainedArtifactBytesV1,
    RetainedCustodyItemV1, RetainedExactArtifactV1, TrustClassV1, UntrustedInputPostureV1,
    aggregate_final_readiness, final_evidence_graph_digest, replay_final_freeze,
};
use serde::{Deserialize, Serialize};

use super::{EvidenceInput, FreezeEvidenceRole, ReleaseFreezeComposeArgs, SubjectIdentity,
    FreezeObservationAdapter, REPOSITORY, instrument};

const MAX_MEMBER_BYTES: usize = 2 * 1024 * 1024;
const MAX_TOTAL_BYTES: usize = 8 * 1024 * 1024;
const MAX_WIRE_BYTES: u64 = 48 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReadbackInput {
    observed_at_utc: String,
    authorization_window_end_utc: String,
    qualification: CargoAllowPostMergeQualificationV1,
    reviewed_evidence_graph: FinalEvidenceGraphV1,
    artifacts: Vec<ArtifactReadback>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactReadback {
    transfer: CargoAllowReleaseArtifactTransferV1,
    expected_producer: ProducerIdentityV1,
    created_at_utc: String,
    retention_expiry_utc: String,
    members: Vec<MemberReadback>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MemberReadback {
    logical_id: String,
    role: String,
    path: String,
    bytes: Vec<u8>,
}

pub(super) fn write_json<T: Serialize + ?Sized>(path: &Path, value: &T) -> CargoAllowResult<()> {
    let bytes = serde_json::to_vec(value).map_err(|error| instrument(format!("serialize retained input: {error}")))?;
    std::fs::write(path, bytes).map_err(|error| instrument(format!("write retained input: {error}")))
}

fn read_input(path: &Path) -> CargoAllowResult<ReadbackInput> {
    use std::io::Read as _;
    let file = std::fs::File::open(path).map_err(|error| instrument(format!("readback input: {error}")))?;
    let mut bytes = Vec::new();
    file.take(MAX_WIRE_BYTES + 1).read_to_end(&mut bytes)
        .map_err(|error| instrument(format!("readback input: {error}")))?;
    if bytes.len() as u64 > MAX_WIRE_BYTES { return Err(instrument("readback input exceeds its bound")); }
    let value = super::rehearsal::decode(&bytes).map_err(|error| instrument(format!("readback JSON: {error}")))?;
    serde_json::from_value(value).map_err(|error| instrument(format!("readback input contract: {error}")))
}

fn utc(value: &str) -> CargoAllowResult<&str> {
    if value.len() != 20 || !value.is_ascii() || &value[10..11] != "T"
        || &value[13..14] != ":" || &value[16..17] != ":" || &value[19..] != "Z"
    { return Err(instrument("canonical UTC evaluation/retention time is required")); }
    let date = SimpleDate::parse(&value[..10]).ok_or_else(|| instrument("invalid UTC date"))?;
    let time = [&value[11..13], &value[14..16], &value[17..19]];
    let values = time.iter().map(|part| part.parse::<u32>()).collect::<Result<Vec<_>, _>>()
        .map_err(|_| instrument("invalid UTC time"))?;
    if format!("{date}") != value[..10] || values[0] > 23 || values[1] > 59 || values[2] > 59
        || !time.iter().all(|part| part.bytes().all(|byte| byte.is_ascii_digit()))
    { return Err(instrument("invalid canonical UTC time")); }
    Ok(value)
}

fn token(value: &str) -> bool {
    !value.is_empty() && value.len() <= 200 && value.bytes().all(|byte| {
        byte.is_ascii_alphanumeric() || b"._:/#-".contains(&byte)
    })
}

fn path_valid(value: &str) -> bool {
    !value.is_empty() && !value.starts_with('/') && value.len() <= 512
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"._-/".contains(&byte))
        && value.split('/').all(|part| !part.is_empty() && part != "." && part != "..")
}

fn digest_matches(expected: &str, bytes: &[u8]) -> bool {
    super::hex_payload(expected) == super::hex_payload(&sha256_v1_bytes(bytes))
}

fn producer(producer: &ProducerIdentityV1) -> CargoAllowResult<FinalEvidenceProducerV1> {
    let generation = u32::try_from(producer.producer_generation)
        .map_err(|_| instrument("producer generation exceeds graph contract"))?;
    let attempt = u32::try_from(producer.run_attempt)
        .map_err(|_| instrument("producer attempt exceeds graph contract"))?;
    let identity = serde_json::to_vec(producer).map_err(|error| instrument(format!("producer identity: {error}")))?;
    Ok(FinalEvidenceProducerV1 {
        producer_id: format!("github-actions:{}:{}:{}", producer.run_id, producer.run_attempt, producer.job_id),
        tool: producer.tool_name.clone(), generation, identity_digest: sha256_v1_bytes(&identity),
        workflow_path: Some(producer.workflow_path.clone()), workflow_run_id: Some(producer.run_id),
        workflow_attempt: Some(attempt), job: Some(producer.job_id.clone()),
    })
}

impl ReadbackInput {
    pub(super) fn preparation_time(&self) -> CargoAllowResult<String> {
        let observed = utc(&self.observed_at_utc)?;
        if let Some((_, member)) = self.member("final-freeze-receipt")? {
            let receipt: allow_report::CargoAllowFinalFreezeReceiptV1 = serde_json::from_slice(&member.bytes)
                .map_err(|error| instrument(format!("prepared receipt: {error}")))?;
            if utc(&receipt.frozen_at_utc)? > observed {
                return Err(instrument("prepared receipt is from the future"));
            }
            Ok(receipt.frozen_at_utc)
        } else { Ok(observed.to_string()) }
    }

    fn member(&self, id: &str) -> CargoAllowResult<Option<(&ArtifactReadback, &MemberReadback)>> {
        let mut found = self.artifacts.iter().flat_map(|artifact| artifact.members.iter().map(move |member| (artifact, member)))
            .filter(|(_, member)| member.logical_id == id);
        let first = found.next();
        if found.next().is_some() { return Err(instrument("duplicate selected logical member")); }
        Ok(first)
    }

    fn validate(&self, subject: &SubjectIdentity) -> CargoAllowResult<()> {
        let now = utc(&self.observed_at_utc)?;
        let window = utc(&self.authorization_window_end_utc)?;
        if window <= now { return Err(instrument("selected authorization window is absent or already ended")); }
        let record = &self.qualification;
        if record.evaluate_verdict() != PostMergeEquivalenceVerdictV1::EquivalentTree
            || record.merged.merge_commit_sha != subject.commit || record.merged.merge_tree_sha != subject.tree
            || utc(&record.created_at_utc)? > now
        { return Err(instrument("current exact-tree post-merge qualification is required")); }
        let reviewed = &self.reviewed_evidence_graph;
        let reviewed_digest = final_evidence_graph_digest(reviewed)
            .map_err(|error| instrument(format!("reviewed graph digest: {error}")))?;
        let required: BTreeSet<_> = reviewed.required_node_ids.iter().chain(
            reviewed.nodes.iter().filter(|node| node.required).map(|node| &node.evidence_id)).cloned().collect();
        let preserved: BTreeSet<_> = record.preserved_evidence_nodes.iter().cloned().collect();
        if reviewed.mode != allow_report::FinalEvidenceGraphModeV1::Production
            || reviewed.repository != REPOSITORY || reviewed.selected_subject.repository != REPOSITORY
            || reviewed.selected_subject.commit != record.reviewed.head_sha
            || reviewed.selected_subject.tree != record.reviewed.tree_sha
            || reviewed_digest != record.premerge_evidence_digest
            || required.is_empty() || required != preserved || preserved.len() != record.preserved_evidence_nodes.len()
            || record.semantic_owners.is_empty()
        { return Err(instrument("qualification does not preserve the exact authenticated reviewed graph denominator")); }
        let mut objects = BTreeSet::new();
        let mut logical = BTreeSet::new();
        let mut total = 0usize;
        let mut count = 0usize;
        if self.artifacts.is_empty() || self.artifacts.len() > 64 { return Err(instrument("selected artifact object set is absent or oversized")); }
        for artifact in &self.artifacts {
            let envelope = &artifact.transfer;
            let identity = &envelope.producer;
            let numeric = &envelope.stable_artifact_id;
            let created = utc(&artifact.created_at_utc)?;
            let expiry = utc(&artifact.retention_expiry_utc)?;
            if envelope.provider_id != "github-actions-artifact" || !objects.insert(numeric)
                || numeric.is_empty() || numeric.len() > 128 || numeric.starts_with('0')
                || !numeric.bytes().all(|byte| byte.is_ascii_digit())
                || identity != &artifact.expected_producer || identity.repository != REPOSITORY
                || identity.commit_sha != subject.commit || identity.tree_sha != subject.tree
                || identity.release_version != subject.version || identity.run_id == 0 || identity.run_attempt == 0
                || identity.producer_generation == 0 || identity.workflow_path.is_empty()
                || identity.job_id.is_empty() || identity.job_id.starts_with('0')
                || !identity.job_id.bytes().all(|byte| byte.is_ascii_digit())
                || created > now || expiry <= now || created >= expiry
                || !matches!(envelope.trust_class, TrustClassV1::ManualDispatch | TrustClassV1::TagWorkflow | TrustClassV1::CleanRelease)
                || envelope.untrusted_input_posture != UntrustedInputPostureV1::StrictByteMatch
                || artifact.members.is_empty() || envelope.files.len() != artifact.members.len()
            { return Err(instrument("selected artifact identity, producer, interval, trust or inventory differs")); }
            let mut paths = BTreeSet::new();
            let mut actual = Vec::new();
            for member in &artifact.members {
                total = total.checked_add(member.bytes.len()).ok_or_else(|| instrument("retained byte total overflow"))?;
                count += 1;
                if !token(&member.logical_id) || !token(&member.role) || !path_valid(&member.path)
                    || !logical.insert(&member.logical_id) || !paths.insert(&member.path)
                    || member.bytes.len() > MAX_MEMBER_BYTES || total > MAX_TOTAL_BYTES || count > 64
                { return Err(instrument("selected logical member is malformed, duplicated, or exceeds provider bounds")); }
                let mut matching = envelope.files.iter().filter(|file| file.path == member.path);
                let file = matching.next().ok_or_else(|| instrument("member path absent from original envelope"))?;
                if matching.next().is_some() || file.size_bytes != member.bytes.len() as u64 || !digest_matches(&file.sha256, &member.bytes) {
                    return Err(instrument("member path, size or exact bytes differ from original envelope"));
                }
                actual.push(ActualDownloadedFileV1 { path: member.path.clone(), size_bytes: member.bytes.len() as u64, sha256: file.sha256.clone() });
            }
            let consumer = ConsumerContextV1 { workflow_path: "release-freeze/qualify".to_string(), run_id: 0,
                job_id: "pure-computation".to_string(), requested_role: envelope.role.clone(), is_credential_bearing: true };
            if envelope.evaluate_transfer(&consumer, &subject.commit, &subject.version, &actual) != ArtifactTransferDispositionV1::Complete {
                return Err(instrument("existing transfer evaluator rejected the downloaded inventory"));
            }
        }
        Ok(())
    }

    pub(super) fn bind_original_evidence(&self, subject: &SubjectIdentity, evidence: &[EvidenceInput], graph: &mut FinalEvidenceGraphV1) -> CargoAllowResult<()> {
        self.validate(subject)?;
        for input in evidence {
            let id = if input.role == FreezeEvidenceRole::ReleaseManifest { "release-manifest-v2".to_string() }
                else { format!("evidence:{}", input.role.label()) };
            let (artifact, member) = self.member(&id)?.ok_or_else(|| instrument(format!("original evidence member {id} is absent")))?;
            let expected_role = if input.role == FreezeEvidenceRole::ReleaseManifest { "ReleaseManifest".to_string() }
                else { format!("Evidence:{}", input.role.label()) };
            if member.role != expected_role || sha256_v1_bytes(&member.bytes) != input.sha256
                || std::fs::read(&input.path).map_err(|error| instrument(format!("evidence reread: {error}")))? != member.bytes
            { return Err(instrument("evidence parser input differs from the selected original member bytes/role")); }
            let id = input.role.graph_shape().2;
            let node = graph.nodes.iter_mut().find(|node| node.evidence_id == id)
                .ok_or_else(|| instrument("selected evidence has no graph node"))?;
            node.producer = producer(&artifact.transfer.producer)?;
            let expected = producer(&artifact.expected_producer)?;
            node.producer_expectation = Some(FinalEvidenceProducerExpectationV1 {
                producer_id: expected.producer_id, generation: expected.generation, identity_digest: Some(expected.identity_digest),
            });
            node.artifact_digest = Some(sha256_v1_bytes(&member.bytes));
            node.expected_artifact_digest = Some(input.sha256.clone());
        }
        for row in graph.selected_subject.package_rows.iter().filter(|row| row.role == allow_report::FinalEvidencePackageRoleV1::UploadCandidate) {
            let (_, member) = self.member(&row.package_name)?.ok_or_else(|| instrument("selected package archive member is absent"))?;
            if member.role != "PackageArchive" || !digest_matches(&row.expected_digest, &member.bytes) {
                return Err(instrument("selected archive logical role or bytes differ from the package denominator"));
            }
        }
        Ok(())
    }

    fn custody(&self, subject: &SubjectIdentity, custody_id: &str) -> CargoAllowResult<(CargoAllowFrozenCandidateCustodyV1, Vec<RetainedExactArtifactV1>)> {
        self.validate(subject)?;
        let mut items = Vec::new();
        let mut retained = Vec::new();
        for artifact in &self.artifacts {
            for member in &artifact.members {
                let digest = sha256_v1_bytes(&member.bytes);
                items.push(RetainedCustodyItemV1 {
                    role: member.role.clone(), artifact_id: member.logical_id.clone(),
                    files: vec![CustodyFileV1 { path: member.path.clone(), size_bytes: member.bytes.len() as u64, sha256: digest.clone() }],
                    storage_locator: format!("github-actions-artifact://{REPOSITORY}/{}/{}", artifact.transfer.stable_artifact_id, member.path),
                    retention_expiry_utc: artifact.retention_expiry_utc.clone(), readback_verified: true,
                    readback_sha256: Some(digest.clone()), confidentiality_class: ConfidentialityClassV1::Public,
                });
                retained.push(RetainedExactArtifactV1 { role: member.role.clone(), artifact_id: member.logical_id.clone(),
                    declared_sha256: digest, bytes: RetainedArtifactBytesV1::new(member.bytes.clone()) });
            }
        }
        Ok((CargoAllowFrozenCandidateCustodyV1::new(CandidateCustodyInitV1 {
            custody_id: custody_id.to_string(), candidate_version: subject.version.clone(),
            git_commit: subject.commit.clone(), git_tree: subject.tree.clone(), items,
            created_at_utc: self.observed_at_utc.clone(),
        }), retained))
    }
}

pub(super) fn prepare(root: &Path, args: &ReleaseFreezeComposeArgs, input_path: &Path) -> CargoAllowResult<()> {
    let readback = read_input(input_path)?;
    if readback.member("final-freeze-receipt")?.is_some() || readback.member("final-freeze-evidence-graph")?.is_some() {
        return Err(instrument("preparation cannot select an existing prepared receipt or graph"));
    }
    let prepared = super::prepare_inputs(root, args, Some(&readback))?;
    let out = if args.out_dir.is_absolute() { args.out_dir.clone() } else { root.join(&args.out_dir) };
    std::fs::create_dir_all(&out).map_err(|error| instrument(format!("prepare out directory: {error}")))?;
    std::fs::write(out.join("final-freeze.receipt.json"), &prepared.receipt_bytes)
        .map_err(|error| instrument(format!("prepared receipt write: {error}")))?;
    write_json(&out.join("final-freeze.evidence-graph.json"), &prepared.graph)?;
    println!("{}", serde_json::json!({"stage":"prepared", "freeze_state":"Incomplete",
        "receipt_sha256":sha256_v1_bytes(&prepared.receipt_bytes),
        "claim_boundary":"immutable inputs prepared; qualification, custody, readiness and replay remain separate"}));
    Ok(())
}

pub(super) fn qualify(root: &Path, args: &ReleaseFreezeComposeArgs, input_path: &Path) -> CargoAllowResult<()> {
    let out = if args.out_dir.is_absolute() { args.out_dir.clone() } else { root.join(&args.out_dir) };
    let readback = read_input(input_path)?;
    let prepared = super::prepare_inputs(root, args, Some(&readback))?;
    let (receipt_object, receipt_member) = readback.member("final-freeze-receipt")?.ok_or_else(|| instrument("retained prepared receipt is absent"))?;
    let (graph_object, graph_member) = readback.member("final-freeze-evidence-graph")?.ok_or_else(|| instrument("retained prepared graph is absent"))?;
    let graph_bytes = serde_json::to_vec(&prepared.graph).map_err(|error| instrument(format!("prepared graph render: {error}")))?;
    if receipt_member.role != "FreezeReceipt" || receipt_member.bytes != prepared.receipt_bytes
        || graph_member.role != "EvidenceGraph" || graph_member.bytes != graph_bytes
        || receipt_object.transfer.stable_artifact_id != graph_object.transfer.stable_artifact_id
        || receipt_object.members.len() != 2
    { return Err(instrument("retained prepared receipt/graph differ from their exact unchanged original input computation")); }
    let (custody, retained_artifacts) = readback.custody(&prepared.subject, &prepared.receipt.frozen_custody_id)?;
    let custody_disposition = custody.evaluate_custody(&prepared.subject.commit, &prepared.subject.version, &readback.observed_at_utc);
    let expiry = custody.items.iter().any(|item| item.retention_expiry_utc <= readback.authorization_window_end_utc);
    let input = CargoAllowFinalFreezeReplayInputsV1 {
        custody, evidence_graph: prepared.graph, freeze_receipt: prepared.receipt,
        retained_transfers: readback.artifacts.iter().map(|artifact| artifact.transfer.clone()).collect(),
        retained_artifacts, observations: super::observation_set(&prepared.evidence, prepared.registry.1),
        replayed_at_utc: readback.observed_at_utc.clone(),
    };
    // Exercise the actual persisted representation before producing completion.
    let wire = serde_json::to_vec(&input).map_err(|error| instrument(format!("replay input render: {error}")))?;
    let replay_input: CargoAllowFinalFreezeReplayInputsV1 = serde_json::from_slice(&wire)
        .map_err(|error| instrument(format!("replay input roundtrip: {error}")))?;
    if input != replay_input { return Err(instrument("serialized replay input changed its typed meaning")); }
    let replay = replay_final_freeze(&replay_input, &FreezeObservationAdapter {
        source_current: false, registry_reading: prepared.registry.2,
    });
    let post_merge = FinalReadinessPostMergePostureV1 {
        merge_commit: readback.qualification.merged.merge_commit_sha.clone(),
        merge_subject_current: readback.qualification.merged.current_main_commit_sha == prepared.subject.commit
            && readback.qualification.merged.current_main_tree_sha == prepared.subject.tree,
        qualification: FinalReadinessQualificationPostureV1::Current, owner: "#2501".to_string(),
    };
    let posture = FinalReadinessCustodyPostureV1 {
        // Feasibility describes complete available replay inputs, independently
        // of semantic evidence blockers in the replay's resulting verdict.
        replay_feasible: custody_disposition == CustodyDispositionV1::Complete,
        expires_before_authorization_window: expiry, owner: "#2501".to_string(),
    };
    let decisions = super::readiness_decision_inputs_observed(&prepared.subject, &prepared.selection,
        &prepared.evidence, Some(post_merge), Some(posture));
    let readiness = decisions.map(|inputs| aggregate_final_readiness(&replay_input.evidence_graph, &inputs));
    super::write_outputs(args, root, &replay_input, &receipt_member.bytes, &replay, &readiness)?;
    write_json(&out.join("final-freeze.qualification.json"), &readback.qualification)?;
    write_json(&out.join("final-freeze.reviewed-evidence-graph.json"), &readback.reviewed_evidence_graph)?;
    let complete = readiness.as_ref().is_ok_and(|value| value.verdict == FinalReadinessVerdictV1::ReadyForFreeze)
        && replay.result == allow_report::FinalFreezeReplayResultV1::CompleteEquivalent;
    let summary = serde_json::json!({
        "stage":"qualified-computation", "freeze_state":if complete { "Complete" } else { "Incomplete" },
        "post_merge_qualification":readback.qualification.evaluate_verdict(),
        "custody_disposition":custody_disposition, "replay_retained_bytes_verified":replay.retained_bytes_verified,
        "receipt_sha256":sha256_v1_bytes(&receipt_member.bytes), "replay_result":replay.result,
        "readiness":readiness.as_ref().ok(), "blocking_rows":super::readiness_rows(&readiness),
        "claim_boundary":"computed from freshly supplied records and bytes; this output is not provider authentication or release authorization"
    });
    write_json(&out.join("final-freeze.composition.json"), &summary)?;
    println!("{summary}");
    // A successful computation can faithfully retain an Incomplete freeze.
    // The production driver checks both the stage and the result before use.
    Ok(())
}

#[cfg(test)]
#[path = "release_freeze_qualification_tests.rs"]
mod tests;
