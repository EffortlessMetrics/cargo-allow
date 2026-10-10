//! Required admission of retained installed-experience records (#3151).
//!
//! This consumes original records and references; it executes no candidate.
//! Matching a caller's input/result pair proves neither an external pilot nor
//! installed documentation/parity observations. Their absent producer contracts
//! remain explicit required holds, even for a model-level Complete result.

use std::collections::{BTreeMap, BTreeSet};

use allow_core::sha256_v1_bytes;
use allow_report::{
    CargoAllowReleaseExperienceV1, ExactCandidatePayloadV2, ExactCandidateResultV2,
    FinalEvidenceCurrentnessV1, FinalEvidenceGraphV1, FinalEvidenceNodeResultV1,
    FinalEvidencePackageRoleV1, FinalEvidencePackageSubjectV1, IsolatedInstallPayloadV2,
    IsolatedInstallResultV2, PackageCandidateFamilyV2, PackageCandidatePayloadV2,
    PackageCandidateResultV2, ReleaseExperienceInputV1, ReleaseExperienceResultV1,
    evaluate_release_experience_v1, validate_exact_candidate_v2, validate_isolated_install_v2,
    validate_package_candidate_v2,
};
use serde::de::DeserializeOwned;

use super::{EvidenceInput, FreezeEvidenceRole, SubjectIdentity, qualification::ReadbackInput};

type State = ReleaseExperienceResultV1;

/// Private computed state, never a serialized authority or supplied proof flag.
pub(super) struct Admission {
    pub(super) result: State,
    pub(super) currentness: FinalEvidenceCurrentnessV1,
    pub(super) notes: Vec<String>,
}

impl Admission {
    fn issue(result: State, detail: impl Into<String>) -> Self {
        Self {
            result,
            currentness: FinalEvidenceCurrentnessV1::ProviderUnavailable,
            notes: vec![detail.into()],
        }
    }

    fn record(&mut self, result: State, detail: impl Into<String>) {
        self.result = self.result.min(result);
        self.notes.push(detail.into());
    }

    pub(super) fn graph_result(&self) -> FinalEvidenceNodeResultV1 {
        match self.result {
            State::Unsupported => FinalEvidenceNodeResultV1::Unsupported,
            State::Malformed => FinalEvidenceNodeResultV1::Malformed,
            State::InstrumentFailure => FinalEvidenceNodeResultV1::InstrumentFailure,
            State::Mismatch => FinalEvidenceNodeResultV1::Mismatch,
            State::Stale => FinalEvidenceNodeResultV1::Stale,
            State::Incomplete => FinalEvidenceNodeResultV1::Incomplete,
            State::NotProven => FinalEvidenceNodeResultV1::NotProven,
            State::Complete => FinalEvidenceNodeResultV1::Complete,
        }
    }

    fn retain_dependencies(&mut self) {
        for note in [
            "#2466: the clean-pilot target/steps/friction producer and semantic reader are absent; retained opaque bytes do not prove a pilot",
            "#3149: the executed supported-case denominator and human/machine parity producer are absent",
            "#3151: installed help/reference/man/completion and selected documentation coherence production are absent",
        ] {
            self.record(State::NotProven, note);
        }
    }
}

fn same_digest(left: &str, right: &str) -> bool {
    let left = left.to_ascii_lowercase();
    let right = right.to_ascii_lowercase();
    match (super::hex_payload(&left), super::hex_payload(&right)) {
        (Some(left), Some(right)) => left == right,
        _ => false,
    }
}

fn original_pair(
    evidence: &[EvidenceInput],
) -> Result<(ReleaseExperienceInputV1, CargoAllowReleaseExperienceV1), Admission> {
    let one = |role| {
        let mut matching = evidence.iter().filter(|input| input.role == role);
        let first = matching.next();
        if matching.next().is_some() {
            return Err(Admission::issue(
                State::Malformed,
                format!("duplicate original experience role {}", role.label()),
            ));
        }
        first.ok_or_else(|| {
            Admission::issue(
                State::Incomplete,
                format!("missing original experience role {}", role.label()),
            )
        })
    };
    let input = one(FreezeEvidenceRole::ReleaseExperienceInput)?;
    let receipt = one(FreezeEvidenceRole::ReleaseExperience)?;
    let input = serde_json::from_value(input.value.clone()).map_err(|error| {
        Admission::issue(State::Malformed, format!("experience input contract: {error}"))
    })?;
    let receipt = serde_json::from_value(receipt.value.clone()).map_err(|error| {
        Admission::issue(State::Malformed, format!("experience result contract: {error}"))
    })?;
    Ok((input, receipt))
}

/// Evaluate original bytes and current reference availability without rewriting
/// the retained input's evaluation time, result, findings, or claim boundary.
pub(super) fn reconcile(
    subject: &SubjectIdentity,
    selected_packages: &[FinalEvidencePackageSubjectV1],
    evidence: &[EvidenceInput],
    readback: Option<&ReadbackInput>,
) -> Admission {
    let (input, retained) = match original_pair(evidence) {
        Ok(pair) => pair,
        Err(mut admission) => {
            admission.retain_dependencies();
            return admission;
        }
    };
    let evaluated = evaluate_release_experience_v1(&input);
    let mut admission = Admission {
        result: evaluated.result,
        currentness: FinalEvidenceCurrentnessV1::ProviderUnavailable,
        notes: evaluated
            .findings
            .iter()
            .map(|finding| finding.reason.clone())
            .collect(),
    };
    if evaluated != retained {
        admission.record(
            State::Mismatch,
            "retained experience result differs from the complete canonical evaluation of its original input",
        );
    }
    let mut docs = BTreeSet::new();
    if input
        .docs_identities
        .iter()
        .any(|identity| !docs.insert(&identity.name))
    {
        admission.record(
            State::Malformed,
            "duplicate experience documentation identity",
        );
    }
    if let Some(readback) = readback {
        match readback.experience_observed_at(subject) {
            Ok(now) => {
                admission.currentness = FinalEvidenceCurrentnessV1::Current;
                if input.evaluated_at_unix_seconds > now
                    || !now
                        .checked_sub(input.observed_at_unix_seconds)
                        .is_some_and(|age| {
                            input.maximum_age_seconds > 0 && age <= input.maximum_age_seconds
                        })
                {
                    admission.record(
                        State::Stale,
                        "experience is future-dated or expired at the checked provider observation",
                    );
                }
                if let Err(problem) = bind_references(subject, selected_packages, &input, readback) {
                    admission.record(problem.result, problem.notes.join("; "));
                }
            }
            Err(error) => admission.record(
                State::InstrumentFailure,
                format!("experience original readback context: {error}"),
            ),
        }
    } else {
        admission.record(
            State::NotProven,
            "experience original member readback and current evaluation clock are absent",
        );
    }
    // An authentic download or matching model output is not a semantic proof.
    // No supported producer/reader currently discharges these three obligations.
    admission.retain_dependencies();
    admission
}

pub(super) fn apply(graph: &mut FinalEvidenceGraphV1, admission: &Admission) {
    for node in graph.nodes.iter_mut().filter(|node| {
        matches!(
            node.evidence_id.as_str(),
            "release-experience-input" | "release-experience"
        )
    }) {
        node.result = admission.graph_result();
        node.currentness = match admission.result {
            State::Stale => FinalEvidenceCurrentnessV1::Stale,
            State::Mismatch => FinalEvidenceCurrentnessV1::Mismatch,
            State::InstrumentFailure => FinalEvidenceCurrentnessV1::InstrumentFailure,
            _ => admission.currentness,
        };
        node.rerun_owner = Some("#3151".to_string());
        node.claim_boundary = admission.notes.join("; ");
    }
}

fn reference<'a>(
    readback: &'a ReadbackInput,
    id: &str,
    digest: &str,
) -> Result<&'a [u8], Admission> {
    let member = readback.experience_member(id).map_err(|error| {
        Admission::issue(
            State::InstrumentFailure,
            format!("experience member {id}: {error}"),
        )
    })?;
    let (role, bytes) = member.ok_or_else(|| {
        Admission::issue(
            State::Incomplete,
            format!("missing original experience member {id}"),
        )
    })?;
    if role != "ExperienceReference" || !same_digest(digest, &sha256_v1_bytes(bytes)) {
        return Err(Admission::issue(
            State::Mismatch,
            format!("experience member {id} role or exact byte digest differs"),
        ));
    }
    Ok(bytes)
}

fn decode<T: DeserializeOwned>(bytes: &[u8], label: &str) -> Result<T, Admission> {
    let value = super::rehearsal::decode(bytes).map_err(|error| {
        Admission::issue(State::Malformed, format!("{label} original JSON: {error}"))
    })?;
    serde_json::from_value(value).map_err(|error| {
        Admission::issue(State::Malformed, format!("{label} original contract: {error}"))
    })
}

fn bind_references(
    subject: &SubjectIdentity,
    selected_packages: &[FinalEvidencePackageSubjectV1],
    input: &ReleaseExperienceInputV1,
    readback: &ReadbackInput,
) -> Result<(), Admission> {
    let candidate_bytes = reference(readback, "experience:package-candidate", &input.candidate_digest)?;
    let install_bytes = reference(readback, "experience:isolated-install", &input.install_digest)?;
    let journey_bytes = reference(readback, "experience:exact-candidate", &input.journey_digest)?;
    let candidate: PackageCandidatePayloadV2 = decode(candidate_bytes, "package candidate")?;
    let install: IsolatedInstallPayloadV2 = decode(install_bytes, "isolated install")?;
    let journey: ExactCandidatePayloadV2 = decode(journey_bytes, "exact candidate")?;
    validate_predecessors(&candidate, &install, &journey)?;
    if !same_digest(&install.candidate_artifact_digest, &input.candidate_digest)
        || !same_digest(&journey.candidate_artifact_digest, &input.candidate_digest)
        || !same_digest(&journey.isolated_install_receipt_digest, &input.install_digest)
        || !same_digest(&install.installed_executable_digest, &input.binary_digest)
        || !same_digest(&journey.installed_executable_digest, &input.binary_digest)
    {
        return Err(Admission::issue(
            State::Mismatch,
            "experience predecessor or installed executable digests disagree",
        ));
    }
    let expected_version = format!("cargo-allow {}", subject.version);
    if candidate.repository_commit != subject.commit
        || candidate.repository_tree != subject.tree
        || install.repository_commit != subject.commit
        || install.repository_tree != subject.tree
        || journey.repository_commit != subject.commit
        || journey.repository_tree != subject.tree
        || candidate.root_package_name != "cargo-allow"
        || candidate.root_package_version != subject.version
        || install.installed_version_output != expected_version
        || journey.installed_version_output != expected_version
        || install.platform != journey.platform
        || install.toolchain != journey.toolchain
        || journey.support_matrix_generation != input.support_matrix_generation
        || !same_digest(&journey.cargo_lock_digest, &subject.cargo_lock_digest)
    {
        return Err(Admission::issue(
            State::Mismatch,
            "experience predecessors differ from the selected source, version, platform, toolchain, support generation or raw journey lock",
        ));
    }
    // The other locks deliberately mean LF-normalized workspace and packaged
    // root locks. Their original validators/producers own those meanings; do
    // not equate either with the raw workspace lock or rewrite a receipt.
    bind_packages(selected_packages, &candidate, &install, &journey)?;
    reference(
        readback,
        "experience:migration-denominator",
        &input.migration_denominator_digest,
    )?;
    for identity in &input.docs_identities {
        reference(
            readback,
            &format!("experience:docs:{}", identity.name),
            &identity.digest,
        )?;
    }
    if let Some(pilot) = &input.clean_pilot {
        reference(readback, "experience:clean-pilot", &pilot.receipt_digest)?;
        reference(readback, "experience:clean-pilot-friction", &pilot.friction_digest)?;
    }
    if let Some(digest) = &input.brownfield_receipt_digest {
        reference(readback, "experience:brownfield-pilot", digest)?;
    }
    Ok(())
}

fn validate_predecessors(
    candidate: &PackageCandidatePayloadV2,
    install: &IsolatedInstallPayloadV2,
    journey: &ExactCandidatePayloadV2,
) -> Result<(), Admission> {
    let candidate = validate_package_candidate_v2(candidate);
    let result = match candidate.result {
        PackageCandidateResultV2::Complete => None,
        PackageCandidateResultV2::UnsupportedGeneration => Some(State::Unsupported),
        PackageCandidateResultV2::MalformedArtifact => Some(State::Malformed),
        PackageCandidateResultV2::StaleInput => Some(State::Stale),
        PackageCandidateResultV2::MissingAsset => Some(State::Incomplete),
        PackageCandidateResultV2::IdentityConflict
        | PackageCandidateResultV2::DependencyConflict => Some(State::Mismatch),
    };
    if let Some(result) = result {
        return Err(Admission::issue(
            result,
            format!("package candidate: {}", candidate.gaps.join("; ")),
        ));
    }
    let install = validate_isolated_install_v2(install);
    let result = match install.result {
        IsolatedInstallResultV2::Complete => None,
        IsolatedInstallResultV2::UnsupportedGeneration => Some(State::Unsupported),
        IsolatedInstallResultV2::MalformedArtifact => Some(State::Malformed),
        IsolatedInstallResultV2::StaleInput => Some(State::Stale),
        IsolatedInstallResultV2::PackageMissing => Some(State::Incomplete),
        IsolatedInstallResultV2::SourceFallbackDetected
        | IsolatedInstallResultV2::ChecksumMismatch
        | IsolatedInstallResultV2::IndexMismatch
        | IsolatedInstallResultV2::GraphMismatch
        | IsolatedInstallResultV2::AmbientShadow
        | IsolatedInstallResultV2::PathLeakInReceipt => Some(State::Mismatch),
    };
    if let Some(result) = result {
        return Err(Admission::issue(
            result,
            format!("isolated install: {}", install.gaps.join("; ")),
        ));
    }
    let journey = validate_exact_candidate_v2(journey);
    let result = match journey.result {
        ExactCandidateResultV2::Complete => None,
        ExactCandidateResultV2::Incomplete => Some(State::Incomplete),
        ExactCandidateResultV2::Stale => Some(State::Stale),
        ExactCandidateResultV2::Mismatch => Some(State::Mismatch),
        ExactCandidateResultV2::Unsupported => Some(State::Unsupported),
        ExactCandidateResultV2::InstrumentFailure => Some(State::InstrumentFailure),
    };
    if let Some(result) = result {
        return Err(Admission::issue(
            result,
            format!("exact candidate: {}", journey.gaps.join("; ")),
        ));
    }
    Ok(())
}

fn bind_packages(
    selected: &[FinalEvidencePackageSubjectV1],
    candidate: &PackageCandidatePayloadV2,
    install: &IsolatedInstallPayloadV2,
    journey: &ExactCandidatePayloadV2,
) -> Result<(), Admission> {
    let installed: BTreeMap<_, _> = install
        .package_rows
        .iter()
        .map(|row| (row.package_name.as_str(), row))
        .collect();
    let qualified: BTreeMap<_, _> = journey
        .package_rows
        .iter()
        .map(|row| (row.package_name.as_str(), row))
        .collect();
    if selected.len() != candidate.rows.len()
        || installed.len() != candidate.rows.len()
        || qualified.len() != candidate.rows.len()
        || usize::try_from(install.graph_comparison.expected_packages).ok()
            != Some(candidate.rows.len())
        || usize::try_from(install.graph_comparison.matched_packages).ok()
            != Some(candidate.rows.len())
    {
        return Err(Admission::issue(
            State::Mismatch,
            "experience package denominator differs from the selected freeze or predecessors",
        ));
    }
    for row in &candidate.rows {
        let selected = selected
            .iter()
            .find(|selected| selected.package_name == row.cargo_package_name);
        let installed = installed.get(row.cargo_package_name.as_str());
        let qualified = qualified.get(row.cargo_package_name.as_str());
        let (Some(selected), Some(installed), Some(qualified), Some(digest)) =
            (selected, installed, qualified, row.crate_digest.as_deref())
        else {
            return Err(Admission::issue(
                State::Mismatch,
                "experience package identity or original archive digest is absent",
            ));
        };
        let family_matches = matches!(
            (selected.role, row.product_family),
            (
                FinalEvidencePackageRoleV1::UploadCandidate,
                PackageCandidateFamilyV2::CargoAllow02
            ) | (
                FinalEvidencePackageRoleV1::ExistingSharedPrerequisite,
                PackageCandidateFamilyV2::Shared01
            )
        );
        if selected.version != row.cargo_package_version
            || installed.package_version != row.cargo_package_version
            || installed
                .resolved_version
                .as_ref()
                .is_some_and(|version| version != &row.cargo_package_version)
            || qualified.package_version != row.cargo_package_version
            || qualified.logical_id != row.logical_id
            || !family_matches
            || !row.crate_size_bytes.is_some_and(|size| size > 0)
            || !same_digest(digest, &installed.crate_digest)
            || !same_digest(digest, &installed.index_checksum)
            || !same_digest(digest, &qualified.crate_digest)
            || (selected.role == FinalEvidencePackageRoleV1::UploadCandidate
                && !same_digest(digest, &selected.expected_digest))
        {
            return Err(Admission::issue(
                State::Mismatch,
                format!(
                    "experience package {} disagrees across original producers or selected upload bytes",
                    row.cargo_package_name
                ),
            ));
        }
        // Shared archive bytes in this isolated candidate are not the public
        // registry checksum authority; that remains the selected registry row.
    }
    Ok(())
}
