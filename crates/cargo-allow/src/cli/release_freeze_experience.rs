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

use crate::core_command_migration::{
    ADMISSION_SCHEMA, Admission as NativeCaseAdmission, CATALOGUE_DIGEST, CATALOGUE_SCHEMA,
    CONTEXT_SCHEMA, Catalogue, ExpectedContext, SemanticValidity,
};

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

    fn retain_dependencies(&mut self, command_case: CommandCaseJoin) {
        self.record(
            State::NotProven,
            "#2466: the clean-pilot target/steps/friction producer and semantic reader are absent; retained opaque bytes do not prove a pilot",
        );
        match command_case {
            CommandCaseJoin::Absent => self.record(
                State::NotProven,
                "#3149: the executed supported-case denominator and human/machine parity producer are absent",
            ),
            // A retained projection is not a replay of its native bundle and
            // members. Its claimed coverage cannot discharge this hold.
            CommandCaseJoin::Partial(note) => self.record(State::NotProven, note),
            CommandCaseJoin::Defect(result, note) => self.record(result, note),
        }
        self.record(
            State::NotProven,
            "#3151: installed help/reference/man/completion and selected documentation coherence production are absent",
        );
    }
}

/// Outcome of joining the retained command-case originals into the #3149
/// executed-denominator hold. Every note names the owning hold so the claim
/// boundary distinguishes a bound declaration from replayed semantic proof.
enum CommandCaseJoin {
    /// No retained originals: the delivered absence hold stands unchanged.
    Absent,
    /// Retained projection/context metadata binds, but the underlying native
    /// bundle/member replay is absent. Every selected case stays unproven.
    Partial(String),
    /// The retained originals carry a binding defect; the hold remains.
    Defect(State, String),
}

/// Join the retained command-case admission and its expected context into the
/// executed-denominator hold. Metadata checks cannot authenticate a native
/// evaluation. The small-record freeze transport does not carry the original
/// executable, snapshots and case members required by the native reader, so
/// even a self-consistent full-coverage projection must remain NotProven.
fn command_case_join(
    subject: &SubjectIdentity,
    input: &ReleaseExperienceInputV1,
    evidence: &[EvidenceInput],
    catalogue_bytes: Option<&[u8]>,
) -> CommandCaseJoin {
    let original = |role| {
        let mut matching = evidence.iter().filter(|item| item.role == role);
        let first = matching.next();
        (first, matching.next().is_none())
    };
    let (admission_input, admission_unique) = original(FreezeEvidenceRole::CommandCaseAdmission);
    let (context_input, context_unique) = original(FreezeEvidenceRole::CommandCaseContext);
    if !admission_unique || !context_unique {
        return CommandCaseJoin::Defect(
            State::Malformed,
            "#3149: duplicate command-case admission or expected-context original".to_string(),
        );
    }
    let (Some(admission_input), Some(context_input)) = (admission_input, context_input) else {
        return match (admission_input, context_input) {
            (None, None) => CommandCaseJoin::Absent,
            _ => CommandCaseJoin::Defect(
                State::Malformed,
                "#3149: the native command-case admission and its expected context must be retained together".to_string(),
            ),
        };
    };
    let admission: NativeCaseAdmission = match serde_json::from_value(admission_input.value.clone())
    {
        Ok(value) => value,
        Err(error) => {
            return CommandCaseJoin::Defect(
                State::Malformed,
                format!("#3149: command-case admission original contract: {error}"),
            );
        }
    };
    let context: ExpectedContext = match serde_json::from_value(context_input.value.clone()) {
        Ok(value) => value,
        Err(error) => {
            return CommandCaseJoin::Defect(
                State::Malformed,
                format!("#3149: command-case expected-context original contract: {error}"),
            );
        }
    };
    if admission.schema_id != ADMISSION_SCHEMA || admission.schema_version != 1 {
        return CommandCaseJoin::Defect(
            State::Malformed,
            format!(
                "#3149: retained command-case admission is generation {} ({}), not the current native admission",
                admission.schema_version, admission.schema_id
            ),
        );
    }
    if context.schema_id != CONTEXT_SCHEMA || context.schema_version != 1 {
        return CommandCaseJoin::Defect(
            State::Malformed,
            format!(
                "#3149: retained command-case expected context is generation {} ({}), not the current context generation",
                context.schema_version, context.schema_id
            ),
        );
    }
    if admission.collection_id.trim().is_empty() || admission.collection_id != context.collection_id
    {
        return CommandCaseJoin::Defect(
            State::Mismatch,
            "#3149: retained command-case projection and expected context name different or empty collections".to_string(),
        );
    }
    if super::hex_payload(&admission.bundle_digest).is_none() {
        return CommandCaseJoin::Defect(
            State::Malformed,
            "#3149: retained command-case projection has a malformed bundle digest".to_string(),
        );
    }
    if !same_digest(&admission.context_digest, &context_input.sha256) {
        return CommandCaseJoin::Defect(
            State::Mismatch,
            "#3149: retained command-case admission does not bind the retained expected-context bytes".to_string(),
        );
    }
    if !same_digest(&input.migration_denominator_digest, CATALOGUE_DIGEST)
        || !same_digest(&context.catalogue_digest, CATALOGUE_DIGEST)
        || !same_digest(&admission.catalogue_digest, CATALOGUE_DIGEST)
    {
        return CommandCaseJoin::Defect(
            State::Mismatch,
            "#3149: retained command-case originals or the experience denominator reference a different case catalogue".to_string(),
        );
    }
    // The catalogue member bytes are checked only in a readback-bound
    // admission; the compose-time pass conservatively keeps the hold.
    let Some(catalogue_bytes) = catalogue_bytes else {
        return CommandCaseJoin::Defect(
            State::NotProven,
            "#3149: retained command-case originals were not checked against the referenced accepted catalogue in this admission; the executed-denominator hold remains".to_string(),
        );
    };
    let catalogue: Catalogue = match super::rehearsal::decode(catalogue_bytes) {
        Ok(value) => match serde_json::from_value(value) {
            Ok(catalogue) => catalogue,
            Err(error) => {
                return CommandCaseJoin::Defect(
                    State::Malformed,
                    format!("#3149: referenced accepted case catalogue original: {error}"),
                );
            }
        },
        Err(error) => {
            return CommandCaseJoin::Defect(
                State::Malformed,
                format!("#3149: referenced accepted case catalogue original: {error}"),
            );
        }
    };
    if catalogue.schema_id != CATALOGUE_SCHEMA || catalogue.schema_version != 1 {
        return CommandCaseJoin::Defect(
            State::Malformed,
            "#3149: referenced case catalogue is not the accepted generation".to_string(),
        );
    }
    if !same_digest(&context.binary.digest, &input.binary_digest) {
        return CommandCaseJoin::Defect(
            State::Mismatch,
            "#3149: retained command-case observations were executed by a different executable"
                .to_string(),
        );
    }
    if context.binary.source_generation != subject.commit {
        return CommandCaseJoin::Defect(
            State::Mismatch,
            "#3149: retained command-case observations bind a different source generation"
                .to_string(),
        );
    }
    if context.binary.path != input.invocation_path
        || context.binary.tool_version != subject.version
        || context.binary.size_bytes == 0
    {
        return CommandCaseJoin::Defect(
            State::Mismatch,
            "#3149: retained command-case executable path, version or size differs from the selected candidate".to_string(),
        );
    }
    let installed = match context.binary.provenance {
        crate::core_command_migration::BinaryProvenance::SourceBuild => {
            if context.binary.candidate_identity.is_some()
                || context.binary.install_identity.is_some()
            {
                return CommandCaseJoin::Defect(
                    State::Malformed,
                    "#3149: source-build command-case context claims installed identities"
                        .to_string(),
                );
            }
            false
        }
        crate::core_command_migration::BinaryProvenance::SuppliedInstalledCandidate => {
            if context
                .binary
                .candidate_identity
                .as_deref()
                .is_none_or(|value| super::hex_payload(value).is_none())
                || context
                    .binary
                    .install_identity
                    .as_deref()
                    .is_none_or(|value| super::hex_payload(value).is_none())
            {
                return CommandCaseJoin::Defect(
                    State::Malformed,
                    "#3149: installed command-case context lacks well-formed candidate/install identities".to_string(),
                );
            }
            true
        }
    };
    let total = catalogue.cases.len();
    let mut admitted_ids = BTreeSet::new();
    for case in &admission.cases {
        if !admitted_ids.insert(case.case_id.as_str()) {
            return CommandCaseJoin::Defect(
                State::Malformed,
                format!(
                    "#3149: duplicate command-case admission row {}",
                    case.case_id
                ),
            );
        }
        if !catalogue.cases.iter().any(|spec| spec.id == case.case_id) {
            return CommandCaseJoin::Defect(
                State::Mismatch,
                format!(
                    "#3149: command-case admission row {} is not an accepted catalogue case",
                    case.case_id
                ),
            );
        }
    }
    let claimed = admission
        .cases
        .iter()
        .filter(|case| case.semantic_validity == SemanticValidity::Valid)
        .count();
    let collection: String = admission.collection_id.chars().take(100).collect();
    let errors = if admission.errors.is_empty() {
        String::new()
    } else {
        format!(
            "; the retained projection reports {} collection errors",
            admission.errors.len()
        )
    };
    let provenance = if installed {
        "declared installed-candidate"
    } else {
        "source-build"
    };
    CommandCaseJoin::Partial(format!(
        "#3149: retained {provenance} projection claims {claimed}/{total} valid selected cases (collection {collection}){errors}; the native bundle/member originals have not been replayed and candidate/install authority has not been reconciled, so all {total} selected case executions remain unproven at this boundary; installed-candidate execution stays required (#4705)"
    ))
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
        Admission::issue(
            State::Malformed,
            format!("experience input contract: {error}"),
        )
    })?;
    let receipt = serde_json::from_value(receipt.value.clone()).map_err(|error| {
        Admission::issue(
            State::Malformed,
            format!("experience result contract: {error}"),
        )
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
    let mut admission = Admission {
        result: State::Complete,
        currentness: FinalEvidenceCurrentnessV1::ProviderUnavailable,
        notes: Vec::new(),
    };
    // Provider availability is independent of whether an original record is
    // present or well-formed. Keep the specific record defect when a checked
    // context exists; absence of that context still cannot acquire Current.
    let now = match readback {
        Some(readback) => match readback.experience_observed_at(subject) {
            Ok(now) => {
                admission.currentness = FinalEvidenceCurrentnessV1::Current;
                Some(now)
            }
            Err(error) => {
                admission.currentness = FinalEvidenceCurrentnessV1::InstrumentFailure;
                admission.record(
                    State::InstrumentFailure,
                    format!("experience original readback context: {error}"),
                );
                None
            }
        },
        None => {
            admission.record(
                State::NotProven,
                "experience original member readback and current evaluation clock are absent",
            );
            None
        }
    };
    let (input, retained) = match original_pair(evidence) {
        Ok(pair) => pair,
        Err(problem) => {
            admission.record(problem.result, problem.notes.join("; "));
            admission.retain_dependencies(CommandCaseJoin::Absent);
            return admission;
        }
    };
    let evaluated = evaluate_release_experience_v1(&input);
    admission.result = admission.result.min(evaluated.result);
    admission.notes.extend(
        evaluated
            .findings
            .iter()
            .map(|finding| finding.reason.clone()),
    );
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
    let mut catalogue_bytes = None;
    if let (Some(readback), Some(now)) = (readback, now) {
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
        catalogue_bytes = match bind_references(subject, selected_packages, &input, readback) {
            Ok(bytes) => Some(bytes),
            Err(problem) => {
                admission.record(problem.result, problem.notes.join("; "));
                None
            }
        };
    }
    // An authentic download or matching model output is not a semantic proof.
    // The pilot and installed-documentation producers still have no supported
    // discharge; the command-case join is bounded to its own retained pair.
    let command_case = command_case_join(subject, &input, evidence, catalogue_bytes.as_deref());
    admission.retain_dependencies(command_case);
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

/// Close only this admission boundary over the current public V2 fields.
/// Serde retains value, type and optional-field semantics; original bytes are
/// never compared with or replaced by a re-serialized predecessor.
fn object_fields(
    value: &serde_json::Value,
    allowed: &[&str],
    label: &str,
) -> Result<(), Admission> {
    if let Some(fields) = value.as_object()
        && let Some(field) = fields
            .keys()
            .find(|field| !allowed.contains(&field.as_str()))
    {
        return Err(Admission::issue(
            State::Malformed,
            format!("{label} original has unknown field {field}"),
        ));
    }
    Ok(())
}

fn candidate_fields(value: &serde_json::Value) -> Result<(), Admission> {
    object_fields(
        value,
        &[
            "schema_id",
            "schema_version",
            "topology_id",
            "topology_digest",
            "repository_commit",
            "repository_tree",
            "cargo_lock_digest",
            "candidate_product_id",
            "root_logical_id",
            "root_package_name",
            "root_package_version",
            "target_class",
            "feature_set_id",
            "rows",
            "known_exclusions",
            "limitations",
            "claim_boundary",
        ],
        "package candidate",
    )?;
    if let Some(rows) = value.get("rows").and_then(serde_json::Value::as_array) {
        for (index, row) in rows.iter().enumerate() {
            let label = format!("package candidate rows[{index}]");
            object_fields(
                row,
                &[
                    "logical_id",
                    "cargo_package_name",
                    "cargo_package_version",
                    "rust_library_name",
                    "workspace_source_path",
                    "product_family",
                    "publication_state",
                    "publish",
                    "support_tier",
                    "release_order",
                    "selected_features",
                    "expected_manifest_identity",
                    "expected_dependency_rows",
                    "required_assets",
                    "crate_digest",
                    "crate_size_bytes",
                ],
                &label,
            )?;
            if let Some(dependencies) = row
                .get("expected_dependency_rows")
                .and_then(serde_json::Value::as_array)
            {
                for (dependency, value) in dependencies.iter().enumerate() {
                    object_fields(
                        value,
                        &["package_name", "package_version", "dependency_kind"],
                        &format!("{label} expected_dependency_rows[{dependency}]"),
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn install_fields(value: &serde_json::Value) -> Result<(), Admission> {
    object_fields(
        value,
        &[
            "schema_id",
            "schema_version",
            "candidate_artifact_digest",
            "repository_commit",
            "repository_tree",
            "cargo_lock_digest",
            "registry_index_digest",
            "external_cache_identity",
            "source_checkout_denied",
            "install_root_identity",
            "cargo_home_identity",
            "installed_executable_digest",
            "installed_version_output",
            "platform",
            "toolchain",
            "package_rows",
            "graph_comparison",
            "limitations",
            "claim_boundary",
        ],
        "isolated install",
    )?;
    if let Some(rows) = value
        .get("package_rows")
        .and_then(serde_json::Value::as_array)
    {
        for (index, row) in rows.iter().enumerate() {
            object_fields(
                row,
                &[
                    "package_name",
                    "package_version",
                    "crate_digest",
                    "index_checksum",
                    "resolved_version",
                ],
                &format!("isolated install package_rows[{index}]"),
            )?;
        }
    }
    if let Some(graph) = value.get("graph_comparison") {
        object_fields(
            graph,
            &[
                "expected_packages",
                "matched_packages",
                "unexpected_packages",
                "missing_packages",
                "version_mismatches",
                "path_sources",
            ],
            "isolated install graph_comparison",
        )?;
    }
    Ok(())
}

fn journey_fields(value: &serde_json::Value) -> Result<(), Admission> {
    object_fields(
        value,
        &[
            "schema_id",
            "schema_version",
            "candidate_artifact_digest",
            "isolated_install_receipt_digest",
            "repository_commit",
            "repository_tree",
            "cargo_lock_digest",
            "installed_executable_digest",
            "installed_version_output",
            "platform",
            "toolchain",
            "support_matrix_generation",
            "package_rows",
            "journey_steps",
            "artifact_schema_results",
            "scanner_completeness",
            "diff_base_identity",
            "limitations",
            "not_included",
            "claim_boundary",
        ],
        "exact candidate",
    )?;
    if let Some(rows) = value
        .get("package_rows")
        .and_then(serde_json::Value::as_array)
    {
        for (index, row) in rows.iter().enumerate() {
            object_fields(
                row,
                &[
                    "logical_id",
                    "package_name",
                    "package_version",
                    "crate_digest",
                ],
                &format!("exact candidate package_rows[{index}]"),
            )?;
        }
    }
    if let Some(steps) = value
        .get("journey_steps")
        .and_then(serde_json::Value::as_array)
    {
        for (index, step) in steps.iter().enumerate() {
            object_fields(
                step,
                &["id", "exit_code", "artifact_schema_id"],
                &format!("exact candidate journey_steps[{index}]"),
            )?;
        }
    }
    Ok(())
}

fn decode<T: DeserializeOwned>(
    bytes: &[u8],
    label: &str,
    check_fields: fn(&serde_json::Value) -> Result<(), Admission>,
) -> Result<T, Admission> {
    let value = super::rehearsal::decode(bytes).map_err(|error| {
        Admission::issue(State::Malformed, format!("{label} original JSON: {error}"))
    })?;
    check_fields(&value)?;
    serde_json::from_value(value).map_err(|error| {
        Admission::issue(
            State::Malformed,
            format!("{label} original contract: {error}"),
        )
    })
}

/// Bind every referenced original member and predecessor, returning the
/// migration-denominator member bytes for the command-case join.
fn bind_references(
    subject: &SubjectIdentity,
    selected_packages: &[FinalEvidencePackageSubjectV1],
    input: &ReleaseExperienceInputV1,
    readback: &ReadbackInput,
) -> Result<Vec<u8>, Admission> {
    let candidate_bytes = reference(
        readback,
        "experience:package-candidate",
        &input.candidate_digest,
    )?;
    let install_bytes = reference(
        readback,
        "experience:isolated-install",
        &input.install_digest,
    )?;
    let journey_bytes = reference(
        readback,
        "experience:exact-candidate",
        &input.journey_digest,
    )?;
    let candidate: PackageCandidatePayloadV2 =
        decode(candidate_bytes, "package candidate", candidate_fields)?;
    let install: IsolatedInstallPayloadV2 =
        decode(install_bytes, "isolated install", install_fields)?;
    let journey: ExactCandidatePayloadV2 =
        decode(journey_bytes, "exact candidate", journey_fields)?;
    validate_predecessors(&candidate, &install, &journey)?;
    if !same_digest(&install.candidate_artifact_digest, &input.candidate_digest)
        || !same_digest(&journey.candidate_artifact_digest, &input.candidate_digest)
        || !same_digest(
            &journey.isolated_install_receipt_digest,
            &input.install_digest,
        )
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
    let migration_catalogue = reference(
        readback,
        "experience:migration-denominator",
        &input.migration_denominator_digest,
    )?
    .to_vec();
    for identity in &input.docs_identities {
        reference(
            readback,
            &format!("experience:docs:{}", identity.name),
            &identity.digest,
        )?;
    }
    if let Some(pilot) = &input.clean_pilot {
        reference(readback, "experience:clean-pilot", &pilot.receipt_digest)?;
        reference(
            readback,
            "experience:clean-pilot-friction",
            &pilot.friction_digest,
        )?;
    }
    if let Some(digest) = &input.brownfield_receipt_digest {
        reference(readback, "experience:brownfield-pilot", digest)?;
    }
    Ok(migration_catalogue)
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
