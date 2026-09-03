//! Pure typed semantic plan for preparing one cargo-allow release candidate
//! (#3831).
//!
//! The plan binds one exact input identity to one exact prospective release
//! projection and classifies the transition as ready, decision-required, or
//! explicitly non-ready. It determines what must move together and which
//! human/repository decisions remain unresolved. It never writes, qualifies,
//! authorizes, or publishes anything, and it never invents a second package
//! list: every selected row comes from the caller-supplied V2 topology rows.
//!
//! Determinism law: equal semantic inputs produce equal plan digests across
//! checkout roots. The semantic identity carries relative repository paths,
//! content digests, and typed identities only — no credentials, no absolute
//! paths, no volatile timestamps.

use std::collections::BTreeMap;

use allow_core::sha256_v1_bytes;
use serde::{Deserialize, Serialize};

use super::release_identity_v1::{ReleaseChannelV1, ReleaseIdentityV1, ReleaseVersionV1};

pub const CANDIDATE_PREPARATION_PLAN_SCHEMA_V1: &str = "cargo-allow.candidate-preparation-plan.v1";
pub const CANDIDATE_PREPARATION_RESULT_SCHEMA_V1: &str =
    "cargo-allow.candidate-preparation-result.v1";

/// The only product-package-topology generation this contract understands.
pub const SUPPORTED_TOPOLOGY_GENERATION_V1: u32 = 2;

pub const CANDIDATE_PREPARATION_CLAIM_BOUNDARY_V1: &str = "A pure typed semantic plan for the exact cargo-allow release-candidate transition. It determines what must move together and which decisions remain unresolved; it does not write, qualify, authorize, or publish the candidate.";

/// Product families allowed inside the release closure. Every other family
/// (cargo-intent, cargo-proof) stays outside the cargo-allow closure.
const CLOSURE_FAMILIES: &[&str] = &["cargo-allow", "shared"];

/// Classification of one candidate-preparation transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidatePreparationReadinessV1 {
    Ready,
    DecisionRequired,
    Stale,
    Conflict,
    Unsupported,
    InstrumentFailure,
}

/// Working-tree state class bound into the plan identity. Dirty facts are
/// never normalized to clean: the type has no implicit default and the
/// classifier refuses `Unknown` instruments outright.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "class", rename_all = "snake_case")]
pub enum CandidatePreparationDirtyStateV1 {
    Clean,
    Dirty {
        modified_paths: u32,
        untracked_paths: u32,
    },
    Unknown,
}

/// One release-corpus source bound by repository-relative path and digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateCorpusSourceV1 {
    pub path: String,
    pub digest: String,
}

/// Exact input identity binding the plan to one repository state (#3831).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidatePreparationInputIdentityV1 {
    pub repository: String,
    pub branch: String,
    pub head_commit: String,
    pub tree: String,
    pub dirty_state: CandidatePreparationDirtyStateV1,
    pub cargo_lock_digest: String,
    pub workspace_manifest_digest: String,
    /// Workspace-relative member path → manifest digest.
    pub member_manifest_digests: BTreeMap<String, String>,
    pub topology_generation: u32,
    pub topology_digest: String,
    pub source_release_identity_digest: String,
    pub support_selection_digest: String,
    pub changie_config_digest: String,
    pub changie_history_digest: String,
    pub release_record: Option<CandidateCorpusSourceV1>,
    pub github_release_note: Option<CandidateCorpusSourceV1>,
    pub source_exception_policy_schema_version: String,
    pub source_exception_policy_digest: String,
}

impl CandidatePreparationPlanV1 {
    /// True when the stored plan digest equals the canonical digest of the
    /// plan's own content. Apply-time gate against tampering.
    pub fn digest_is_authentic(&self) -> bool {
        let mut draft = self.clone();
        draft.plan_digest = String::new();
        match serde_json::to_string(&draft) {
            Ok(canonical) => allow_core::sha256_v1_bytes(canonical.as_bytes()) == self.plan_digest,
            Err(_) => false,
        }
    }
}

impl CandidatePreparationInputIdentityV1 {
    /// Digest over the canonical JSON of the full identity. Relative paths
    /// and content digests only, so equal repository states digest equally
    /// from any checkout root.
    pub fn identity_digest(&self) -> String {
        let canonical = serde_json::to_string(self).unwrap_or_else(|_| {
            panic!("candidate preparation input identity must serialize canonically")
        });
        sha256_v1_bytes(canonical.as_bytes())
    }
}

/// Topology row projected into the plan vocabulary. The CLI adapter copies
/// these fields from the typed V2 topology authority; the model never parses
/// the topology file itself and never invents rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidatePackageRowV1 {
    pub logical_id: String,
    pub cargo_package_name: String,
    pub product_family: String,
    pub posture: String,
    pub package_version: String,
    pub version_line: String,
    pub version_source: String,
    pub publication_state: String,
    pub candidate_inclusion: bool,
    pub publish: bool,
    pub release_order: u32,
    pub support_tier: String,
}

/// One closure row selected by the plan, with its role in the release graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateSelectedRowV1 {
    pub row: CandidatePackageRowV1,
    /// "product" for the cargo-allow family closure, "shared_prerequisite"
    /// for the exact shared inputs.
    pub role: String,
    /// Prospective package version after the transition. Equal to the
    /// current version for held shared prerequisites.
    pub prospective_version: String,
}

/// One atomic semantic change the future apply slice must perform.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CandidatePreparationOperationV1 {
    SetPackageVersion {
        package: String,
        release_order: u32,
        from: String,
        to: String,
    },
    SetInternalRequirement {
        dependency: String,
        from: String,
        to: String,
    },
    HoldExactVersion {
        package: String,
        release_order: u32,
        version: String,
        reason: String,
    },
}

/// The typed release identity one side of the transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateReleaseIdentityProjectionV1 {
    pub version: String,
    pub tag: String,
    pub channel: String,
    pub rc_ordinal: Option<u32>,
    pub github_prerelease: bool,
}

impl CandidateReleaseIdentityProjectionV1 {
    pub fn from_version(version: &ReleaseVersionV1) -> Self {
        let (channel, rc_ordinal) = match version.channel() {
            ReleaseChannelV1::Stable => ("stable", None),
            ReleaseChannelV1::ReleaseCandidate { ordinal } => ("release_candidate", Some(ordinal)),
        };
        Self {
            version: version.as_str().to_string(),
            tag: version.tag(),
            channel: channel.to_string(),
            rc_ordinal,
            github_prerelease: version.channel().github_prerelease(),
        }
    }

    /// Canonical provenance digest binding one role ("source", "target")
    /// to this identity projection.
    pub fn canonical_digest(&self, role: &str) -> String {
        let subject = format!(
            "cargo-allow.candidate-release-identity.v1\x1f{role}\x1f{}\x1f{}\x1f{}\x1f{}",
            self.version, self.tag, self.channel, self.github_prerelease
        );
        sha256_v1_bytes(subject.as_bytes())
    }
}

/// Support/channel posture projected onto the closure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateSupportChannelPostureV1 {
    pub target_channel: String,
    pub github_prerelease: bool,
    /// product_family → support tier, copied from the topology rows.
    pub product_support_tiers: BTreeMap<String, String>,
}

/// Expected role of one release-corpus artifact. Roles are semantic; the
/// bytes belong to the next child (#3832).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateCorpusRoleV1 {
    pub role: String,
    pub path: String,
    pub expected_semantic_change: String,
}

/// One governed-file class the transition must respect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateGovernedFileClassV1 {
    pub class: String,
    pub expectation: String,
}

/// One unresolved human/repository judgment. Never synthesized away.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidatePreparationDecisionV1 {
    pub decision_id: String,
    pub question: String,
    pub owner: String,
}

/// One validation obligation the candidate must satisfy before any later
/// qualification or publication claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateValidationObligationV1 {
    pub obligation_id: String,
    pub description: String,
}

/// A known external observation retained as input only. Observations never
/// change the projection and never authorize anything.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateExternalObservationV1 {
    pub observation_id: String,
    pub subject: String,
    pub detail: String,
}

/// The complete typed semantic plan (#3831).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidatePreparationPlanV1 {
    pub schema: String,
    pub input_identity: CandidatePreparationInputIdentityV1,
    pub source_release_identity: CandidateReleaseIdentityProjectionV1,
    pub target_release_identity: CandidateReleaseIdentityProjectionV1,
    /// Always "projected_not_public": a prepared candidate has no public
    /// existence until a separately authorized release executes.
    pub target_publication_posture: String,
    pub selected_rows: Vec<CandidateSelectedRowV1>,
    pub operations: Vec<CandidatePreparationOperationV1>,
    pub support_channel_posture: CandidateSupportChannelPostureV1,
    pub expected_corpus_roles: Vec<CandidateCorpusRoleV1>,
    pub expected_governed_file_classes: Vec<CandidateGovernedFileClassV1>,
    pub required_decisions: Vec<CandidatePreparationDecisionV1>,
    pub external_observations: Vec<CandidateExternalObservationV1>,
    pub validation_obligations: Vec<CandidateValidationObligationV1>,
    pub plan_digest: String,
    pub claim_boundary: String,
}

/// The classified outcome of one preparation attempt. Non-ready classes
/// carry reasons and, when the inputs themselves were collectible, the input
/// identity that was bound.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidatePreparationResultV1 {
    pub schema: String,
    pub readiness: CandidatePreparationReadinessV1,
    pub reasons: Vec<String>,
    pub input_identity: Option<CandidatePreparationInputIdentityV1>,
    pub plan: Option<CandidatePreparationPlanV1>,
    /// The compiled file-operation and decision plan (#3832), present when
    /// the semantic plan projected and the caller gathered surfaces.
    #[serde(default)]
    pub operations: Option<super::candidate_preparation_operations_v1::CandidateOperationPlanV1>,
    pub human_summary: String,
}

/// Everything the pure classifier needs. The caller (CLI or test) gathers
/// these from live authorities; the model performs no I/O.
#[derive(Debug, Clone)]
pub struct CandidateProjectionInput<'a> {
    pub target_version_text: &'a str,
    pub input_identity: CandidatePreparationInputIdentityV1,
    pub topology_rows: &'a [CandidatePackageRowV1],
    /// product_id → posture from the product support matrix, for the
    /// topology cross-check.
    pub support_matrix_postures: BTreeMap<String, String>,
    /// Workspace dependency key → version requirement (path dependencies).
    pub internal_requirements: BTreeMap<String, String>,
    pub external_observations: Vec<CandidateExternalObservationV1>,
}

/// Validate one candidate operation set against the closure and target.
///
/// Law enforced here (see #3831 negative controls 3-6):
/// - only closure rows appear, and only cargo-allow rows move;
/// - every moved row lands exactly on the target version;
/// - every internal requirement lands on the exact `=<target>` form;
/// - shared prerequisites are held at their exact stable current version.
pub fn validate_candidate_operation_set(
    selected_rows: &[CandidateSelectedRowV1],
    target: &ReleaseVersionV1,
    operations: &[CandidatePreparationOperationV1],
) -> Result<(), String> {
    let products: BTreeMap<&str, &CandidateSelectedRowV1> = selected_rows
        .iter()
        .filter(|selected| selected.row.product_family == "cargo-allow")
        .map(|selected| (selected.row.cargo_package_name.as_str(), selected))
        .collect();
    let shared: BTreeMap<&str, &CandidateSelectedRowV1> = selected_rows
        .iter()
        .filter(|selected| selected.role == "shared_prerequisite")
        .map(|selected| (selected.row.cargo_package_name.as_str(), selected))
        .collect();

    for operation in operations {
        match operation {
            CandidatePreparationOperationV1::SetPackageVersion { package, to, .. } => {
                let Some(selected) = products.get(package.as_str()) else {
                    return Err(format!(
                        "set-package-version targets `{package}` which is not a cargo-allow closure row; only product rows move"
                    ));
                };
                if to != target.as_str() {
                    return Err(format!(
                        "set-package-version for `{package}` lands on `{to}` instead of the exact target `{}`",
                        target.as_str()
                    ));
                }
                if to == &selected.row.package_version {
                    return Err(format!(
                        "set-package-version for `{package}` reuses the current version `{to}` instead of moving to the target"
                    ));
                }
            }
            CandidatePreparationOperationV1::SetInternalRequirement { dependency, to, .. } => {
                let expected = format!("={}", target.as_str());
                if *to != expected {
                    return Err(format!(
                        "internal requirement for `{dependency}` lands on `{to}` instead of the exact `{expected}`"
                    ));
                }
                if !products.contains_key(dependency.as_str()) {
                    return Err(format!(
                        "internal requirement `{dependency}` does not name a cargo-allow closure row"
                    ));
                }
            }
            CandidatePreparationOperationV1::HoldExactVersion {
                package, version, ..
            } => {
                let Some(selected) = shared.get(package.as_str()) else {
                    return Err(format!(
                        "hold-exact-version names `{package}` which is not a shared prerequisite closure row"
                    ));
                };
                if version != &selected.row.package_version {
                    return Err(format!(
                        "hold-exact-version for `{package}` claims `{version}` but the closure binds `{}`",
                        selected.row.package_version
                    ));
                }
                let held = ReleaseVersionV1::parse(version)
                    .map_err(|error| format!("held shared version `{version}`: {error}"))?;
                if held.channel() != ReleaseChannelV1::Stable {
                    return Err(format!(
                        "held shared prerequisite `{package}` must stay on an exact stable line, found `{version}`"
                    ));
                }
            }
        }
    }
    Ok(())
}

/// Classify one candidate-preparation transition. Always returns a typed
/// result; malformed or unsupported inputs are explicit non-ready classes,
/// never panics or silent greens.
pub fn prepare_candidate_plan(input: CandidateProjectionInput<'_>) -> CandidatePreparationResultV1 {
    let unsupported = |reasons: Vec<String>| CandidatePreparationResultV1 {
        schema: CANDIDATE_PREPARATION_RESULT_SCHEMA_V1.to_string(),
        readiness: CandidatePreparationReadinessV1::Unsupported,
        reasons,
        input_identity: Some(input.input_identity.clone()),
        plan: None,
        operations: None,
        human_summary: "candidate preparation is unsupported for the requested inputs".to_string(),
    };

    let target = match ReleaseVersionV1::parse(input.target_version_text) {
        Ok(target) => target,
        Err(error) => {
            return unsupported(vec![format!(
                "malformed or unsupported target version: {error}"
            )]);
        }
    };

    // Control 2: the release identity contract itself rejects a stable
    // target paired with a prerelease posture; the projection can only be
    // built through that contract.
    let target_identity = match ReleaseIdentityV1::parse(
        target.as_str(),
        &target.tag(),
        target.channel().github_prerelease(),
    ) {
        Ok(identity) => identity,
        Err(error) => {
            return unsupported(vec![format!("target release identity rejected: {error}")]);
        }
    };

    if input.input_identity.topology_generation != SUPPORTED_TOPOLOGY_GENERATION_V1 {
        return CandidatePreparationResultV1 {
            schema: CANDIDATE_PREPARATION_RESULT_SCHEMA_V1.to_string(),
            readiness: CandidatePreparationReadinessV1::Unsupported,
            reasons: vec![format!(
                "product package topology generation {} is unsupported; this contract understands generation {SUPPORTED_TOPOLOGY_GENERATION_V1}",
                input.input_identity.topology_generation
            )],
            input_identity: Some(input.input_identity.clone()),
            plan: None,
            operations: None,
            human_summary: "candidate preparation is unsupported for the requested inputs"
                .to_string(),
        };
    }

    let instrument_failure = |reasons: Vec<String>| CandidatePreparationResultV1 {
        schema: CANDIDATE_PREPARATION_RESULT_SCHEMA_V1.to_string(),
        readiness: CandidatePreparationReadinessV1::InstrumentFailure,
        reasons,
        input_identity: Some(input.input_identity.clone()),
        plan: None,
        operations: None,
        human_summary: "candidate preparation inputs could not be trusted".to_string(),
    };

    if matches!(
        input.input_identity.dirty_state,
        CandidatePreparationDirtyStateV1::Unknown
    ) {
        return instrument_failure(vec![
            "working-tree state could not be determined; dirty-state facts must never be guessed or normalized".to_string(),
        ]);
    }

    let conflict = |reasons: Vec<String>| CandidatePreparationResultV1 {
        schema: CANDIDATE_PREPARATION_RESULT_SCHEMA_V1.to_string(),
        readiness: CandidatePreparationReadinessV1::Conflict,
        reasons,
        input_identity: Some(input.input_identity.clone()),
        plan: None,
        operations: None,
        human_summary: "candidate preparation found conflicting authorities".to_string(),
    };

    // Closure derivation: membership comes from the topology rows only.
    let mut closure: Vec<&CandidatePackageRowV1> = input
        .topology_rows
        .iter()
        .filter(|row| row.candidate_inclusion)
        .collect();
    closure.sort_by_key(|row| (row.release_order, row.logical_id.clone()));

    if closure.is_empty() {
        return conflict(vec![
            "the topology selects an empty release closure".to_string(),
        ]);
    }
    for row in &closure {
        if !CLOSURE_FAMILIES.contains(&row.product_family.as_str()) {
            return conflict(vec![format!(
                "closure row `{}` carries product family `{}`, which stays outside the cargo-allow release closure",
                row.logical_id, row.product_family
            )]);
        }
        if !row.publish {
            return conflict(vec![format!(
                "closure row `{}` is marked candidate_inclusion but not publishable",
                row.logical_id
            )]);
        }
    }

    let product_rows: Vec<&&CandidatePackageRowV1> = closure
        .iter()
        .filter(|row| row.product_family == "cargo-allow")
        .collect();
    let shared_rows: Vec<&&CandidatePackageRowV1> = closure
        .iter()
        .filter(|row| row.product_family == "shared")
        .collect();

    if product_rows.is_empty() {
        return conflict(vec![
            "the release closure selects no cargo-allow product rows".to_string(),
        ]);
    }

    // Mixed-version ownership law: all product rows move as one line.
    let mut source_versions: BTreeMap<&str, &str> = BTreeMap::new();
    for row in &product_rows {
        source_versions.insert(row.package_version.as_str(), row.logical_id.as_str());
    }
    if source_versions.len() != 1 {
        return conflict(vec![format!(
            "cargo-allow closure rows disagree on the source line: {}",
            source_versions
                .iter()
                .map(|(version, logical)| format!("{logical}={version}"))
                .collect::<Vec<_>>()
                .join(", ")
        )]);
    }
    let source_version_text = *source_versions.keys().next().expect("checked above");
    let product_version_line = product_rows[0].version_line.clone();
    for row in &product_rows {
        if row.version_line != product_version_line {
            return conflict(vec![format!(
                "cargo-allow closure row `{}` carries version_line `{}` while the closure binds `{product_version_line}`",
                row.logical_id, row.version_line
            )]);
        }
    }

    let source_version = match ReleaseVersionV1::parse(source_version_text) {
        Ok(version) => version,
        Err(error) => {
            return conflict(vec![format!(
                "topology source line `{source_version_text}` is not a typed release identity: {error}"
            )]);
        }
    };
    if let Err(error) = ReleaseIdentityV1::parse(
        source_version.as_str(),
        &source_version.tag(),
        source_version.channel().github_prerelease(),
    ) {
        return conflict(vec![format!(
            "topology source release identity rejected: {error}"
        )]);
    }

    if target.as_str() == source_version.as_str() {
        return CandidatePreparationResultV1 {
            schema: CANDIDATE_PREPARATION_RESULT_SCHEMA_V1.to_string(),
            readiness: CandidatePreparationReadinessV1::Stale,
            reasons: vec![format!(
                "target version {} equals the current source line; there is no transition to prepare",
                target.as_str()
            )],
            input_identity: Some(input.input_identity.clone()),
            plan: None,
            operations: None,
            human_summary: "candidate preparation is stale for the requested inputs".to_string(),
        };
    }
    if target.precedence() <= source_version.precedence() {
        return conflict(vec![format!(
            "target version {} does not outrank the current source line {}; refreezing an older line is not a preparation transition",
            target.as_str(),
            source_version.as_str()
        )]);
    }

    // Support selection cross-check: the product support matrix may not
    // disagree with the topology postures. Each matrix product maps to a
    // topology family (legacy-migration mirrors the legacy rows inside the
    // cargo-allow family), and its posture must be present among that
    // family's rows. Shared rows legitimately mix protocol and
    // implementation postures, so presence — not uniformity — is the law.
    let mut family_postures: BTreeMap<&str, std::collections::BTreeSet<&str>> =
        std::collections::BTreeMap::new();
    for row in input.topology_rows {
        family_postures
            .entry(row.product_family.as_str())
            .or_default()
            .insert(row.posture.as_str());
    }
    let mut posture_conflicts: Vec<String> = Vec::new();
    for (product_id, posture) in &input.support_matrix_postures {
        let family = match product_id.as_str() {
            "cargo-allow" | "legacy-migration" => Some("cargo-allow"),
            "cargo-intent" => Some("cargo-intent"),
            "cargo-proof" => Some("cargo-proof"),
            "shared-protocols" => Some("shared"),
            _ => None,
        };
        if let Some(family) = family {
            match family_postures.get(family) {
                Some(topology_postures) => {
                    if !topology_postures.contains(posture.as_str()) {
                        posture_conflicts.push(format!(
                            "product support matrix records posture `{posture}` for `{product_id}` while the topology binds {topology_postures:?}"
                        ));
                    }
                }
                None => posture_conflicts.push(format!(
                    "product support matrix records `{product_id}` but the topology binds no `{family}` rows"
                )),
            }
        }
    }
    if !posture_conflicts.is_empty() {
        return conflict(posture_conflicts);
    }

    // Shared prerequisites stay on their exact stable lines.
    let mut shared_conflicts: Vec<String> = Vec::new();
    for row in &shared_rows {
        match ReleaseVersionV1::parse(&row.package_version) {
            Ok(shared_version) => {
                if shared_version.channel() != ReleaseChannelV1::Stable {
                    shared_conflicts.push(format!(
                        "shared prerequisite `{}` is bound to non-stable line `{}`",
                        row.logical_id, row.package_version
                    ));
                }
            }
            Err(error) => shared_conflicts.push(format!(
                "shared prerequisite `{}` carries malformed version `{}`: {error}",
                row.logical_id, row.package_version
            )),
        }
    }
    if !shared_conflicts.is_empty() {
        return conflict(shared_conflicts);
    }

    // Selected rows in dependency order.
    let selected_rows: Vec<CandidateSelectedRowV1> = closure
        .iter()
        .map(|row| {
            let role = if row.product_family == "cargo-allow" {
                "product"
            } else {
                "shared_prerequisite"
            };
            let prospective_version = if role == "product" {
                target.as_str().to_string()
            } else {
                row.package_version.clone()
            };
            CandidateSelectedRowV1 {
                row: (*row).clone(),
                role: role.to_string(),
                prospective_version,
            }
        })
        .collect();

    // Operations: move every product row, exact the requirements, hold the
    // shared prerequisites.
    let mut operations: Vec<CandidatePreparationOperationV1> = Vec::new();
    for row in &closure {
        if row.product_family != "cargo-allow" {
            continue;
        }
        operations.push(CandidatePreparationOperationV1::SetPackageVersion {
            package: row.cargo_package_name.clone(),
            release_order: row.release_order,
            from: row.package_version.clone(),
            to: target.as_str().to_string(),
        });
    }
    for (dependency, requirement) in &input.internal_requirements {
        let closure_binds_dependency = closure.iter().any(|row| {
            row.product_family == "cargo-allow" && &row.cargo_package_name == dependency
        });
        if !closure_binds_dependency {
            continue;
        }
        let prospective = format!("={}", target.as_str());
        if requirement != &prospective {
            operations.push(CandidatePreparationOperationV1::SetInternalRequirement {
                dependency: dependency.clone(),
                from: requirement.clone(),
                to: prospective,
            });
        }
    }
    for row in &shared_rows {
        operations.push(CandidatePreparationOperationV1::HoldExactVersion {
            package: row.cargo_package_name.clone(),
            release_order: row.release_order,
            version: row.package_version.clone(),
            reason: "exact public shared prerequisite input stays fixed across the transition"
                .to_string(),
        });
    }
    if let Err(error) = validate_candidate_operation_set(&selected_rows, &target, &operations) {
        return conflict(vec![format!(
            "generated operation set violated the transition law: {error}"
        )]);
    }

    // Expected release-corpus roles for the target (semantic roles only).
    let expected_corpus_roles = vec![
        CandidateCorpusRoleV1 {
            role: "release_record".to_string(),
            path: format!("docs/release/{}.md", target.as_str()),
            expected_semantic_change: "final release record for the target identity, authored under the frozen candidate basis".to_string(),
        },
        CandidateCorpusRoleV1 {
            role: "github_release_note".to_string(),
            path: format!("docs/release/github/{}.md", target.tag()),
            expected_semantic_change: "GitHub release note projected from the final release record".to_string(),
        },
        CandidateCorpusRoleV1 {
            role: "changie_corpus".to_string(),
            path: ".changes/".to_string(),
            expected_semantic_change: "Changie change entries reconciled into the release corpus before the record is finalized".to_string(),
        },
    ];
    let mut reasons: Vec<String> = Vec::new();
    let mut absent_corpus = false;
    if input.input_identity.release_record.is_none() {
        absent_corpus = true;
        reasons.push(format!(
            "expected release-record source docs/release/{}.md is absent from the bound inputs",
            target.as_str()
        ));
    }
    if input.input_identity.github_release_note.is_none() {
        absent_corpus = true;
        reasons.push(format!(
            "expected GitHub-note source docs/release/github/{}.md is absent from the bound inputs",
            target.tag()
        ));
    }

    // Required decisions. Structural for every final candidate: the frozen
    // basis and the publication authorization are human judgments the plan
    // must never synthesize.
    let mut required_decisions = vec![
        CandidatePreparationDecisionV1 {
            decision_id: "confirm-frozen-candidate-basis".to_string(),
            question: "Confirm the exact frozen candidate basis this projection will be applied against (#2501 refreeze authority).".to_string(),
            owner: "release-operator".to_string(),
        },
        CandidatePreparationDecisionV1 {
            decision_id: "publication-authorization".to_string(),
            question: "Publication remains gated on separate explicit authorization (#3760); preparation never authorizes it.".to_string(),
            owner: "release-operator".to_string(),
        },
    ];
    match &input.input_identity.dirty_state {
        CandidatePreparationDirtyStateV1::Dirty {
            modified_paths,
            untracked_paths,
        } => {
            required_decisions.push(CandidatePreparationDecisionV1 {
                decision_id: "clean-working-tree".to_string(),
                question: format!(
                    "Resolve the dirty working tree ({modified_paths} modified, {untracked_paths} untracked) before any future apply slice binds to this plan."
                ),
                owner: "repository-maintainer".to_string(),
            });
        }
        CandidatePreparationDirtyStateV1::Unknown => {
            return instrument_failure(vec![
                "working-tree state could not be determined; dirty-state facts must never be guessed or normalized".to_string(),
            ]);
        }
        CandidatePreparationDirtyStateV1::Clean => {}
    }

    let validation_obligations = vec![
        CandidateValidationObligationV1 {
            obligation_id: "no-new-guard".to_string(),
            description: "cargo-allow check --mode no-new stays green across the applied transition".to_string(),
        },
        CandidateValidationObligationV1 {
            obligation_id: "change-note-gate".to_string(),
            description: "git diff --require-change-note proves every ledger-adjacent change is receipted".to_string(),
        },
        CandidateValidationObligationV1 {
            obligation_id: "full-binary-suite".to_string(),
            description: "the full cargo-allow binary test suite passes before the candidate is qualified".to_string(),
        },
        CandidateValidationObligationV1 {
            obligation_id: "release-rehearsal".to_string(),
            description: "release rehearsal phases (#3751) must qualify the prepared candidate".to_string(),
        },
        CandidateValidationObligationV1 {
            obligation_id: "package-candidate-smoke".to_string(),
            description: "the packaged candidate must pass the pre-publication package smoke before any publication claim".to_string(),
        },
    ];

    let expected_governed_file_classes = vec![
        CandidateGovernedFileClassV1 {
            class: "source_exception_policy".to_string(),
            expectation: "policy/allow.toml rows change only through the receipted add flow; release preparation never authors ledger entries".to_string(),
        },
        CandidateGovernedFileClassV1 {
            class: "product_support_matrix".to_string(),
            expectation: "support matrix postures must keep mirroring the topology rows across the transition".to_string(),
        },
        CandidateGovernedFileClassV1 {
            class: "package_topology".to_string(),
            expectation: "topology generation and digest are bound into the plan identity; applied changes require a fresh plan".to_string(),
        },
    ];

    let source_projection = CandidateReleaseIdentityProjectionV1::from_version(&source_version);
    let target_projection = CandidateReleaseIdentityProjectionV1::from_version(&target);

    let mut support_tiers: BTreeMap<String, String> = BTreeMap::new();
    for row in &closure {
        support_tiers
            .entry(row.product_family.clone())
            .or_insert_with(|| row.support_tier.clone());
    }

    let draft = CandidatePreparationPlanV1 {
        schema: CANDIDATE_PREPARATION_PLAN_SCHEMA_V1.to_string(),
        input_identity: input.input_identity.clone(),
        source_release_identity: source_projection,
        target_release_identity: target_projection,
        target_publication_posture: "projected_not_public".to_string(),
        selected_rows,
        operations,
        support_channel_posture: CandidateSupportChannelPostureV1 {
            target_channel: if target.channel() == ReleaseChannelV1::Stable {
                "stable".to_string()
            } else {
                "release_candidate".to_string()
            },
            github_prerelease: target_identity.github_prerelease(),
            product_support_tiers: support_tiers,
        },
        expected_corpus_roles,
        expected_governed_file_classes,
        required_decisions,
        external_observations: input.external_observations.clone(),
        validation_obligations,
        plan_digest: String::new(),
        claim_boundary: CANDIDATE_PREPARATION_CLAIM_BOUNDARY_V1.to_string(),
    };

    let digest = sha256_v1_bytes(
        serde_json::to_string(&draft)
            .expect("candidate preparation plan must serialize canonically")
            .as_bytes(),
    );
    let plan = CandidatePreparationPlanV1 {
        plan_digest: digest,
        ..draft
    };

    // Classification: absent expected corpus sources make the plan stale
    // against the target. Otherwise the always-present structural decisions
    // (frozen basis, publication authorization) keep every projected plan
    // at DecisionRequired; `Ready` is reserved for a state where no human
    // judgment remains open, which this contract cannot assert by itself.
    let readiness = if absent_corpus {
        CandidatePreparationReadinessV1::Stale
    } else {
        CandidatePreparationReadinessV1::DecisionRequired
    };
    let decision_count = plan.required_decisions.len();
    let operation_count = plan.operations.len();
    let product_count = plan
        .selected_rows
        .iter()
        .filter(|selected| selected.role == "product")
        .count();
    let shared_count = plan
        .selected_rows
        .iter()
        .filter(|selected| selected.role == "shared_prerequisite")
        .count();

    let human_summary = format!(
        "candidate preparation: {} product rows -> {}, {} shared prerequisites held, {operation_count} operations, {decision_count} decisions required; source {} -> target {}; readiness {:?}; plan {}",
        product_count,
        target.as_str(),
        shared_count,
        source_version.as_str(),
        target.as_str(),
        readiness,
        plan.plan_digest,
    );

    CandidatePreparationResultV1 {
        schema: CANDIDATE_PREPARATION_RESULT_SCHEMA_V1.to_string(),
        readiness,
        reasons,
        input_identity: Some(input.input_identity),
        plan: Some(plan),
        operations: None,
        human_summary,
    }
}

#[cfg(test)]
mod tests {
    use super::super::release_identity_v1::ReleaseIdentityErrorV1;
    use super::*;

    fn product_row(name: &str, order: u32, version: &str) -> CandidatePackageRowV1 {
        CandidatePackageRowV1 {
            logical_id: name.to_string(),
            cargo_package_name: name.to_string(),
            product_family: "cargo-allow".to_string(),
            posture: "CargoAllowSupported".to_string(),
            package_version: version.to_string(),
            version_line: "cargo-allow-0.2".to_string(),
            version_source: "WorkspaceProduct".to_string(),
            publication_state: "UnpublishedInternal".to_string(),
            candidate_inclusion: true,
            publish: true,
            release_order: order,
            support_tier: "supported".to_string(),
        }
    }

    fn shared_row(name: &str, order: u32) -> CandidatePackageRowV1 {
        CandidatePackageRowV1 {
            logical_id: name.to_string(),
            cargo_package_name: name.to_string(),
            product_family: "shared".to_string(),
            posture: "SharedProtocolInternalOrStabilizing".to_string(),
            package_version: "0.1.0".to_string(),
            version_line: "shared-0.1".to_string(),
            version_source: "Explicit".to_string(),
            publication_state: "UnpublishedInternal".to_string(),
            candidate_inclusion: true,
            publish: true,
            release_order: order,
            support_tier: "internal-stabilizing".to_string(),
        }
    }

    fn identity() -> CandidatePreparationInputIdentityV1 {
        CandidatePreparationInputIdentityV1 {
            repository: "EffortlessMetrics/cargo-allow".to_string(),
            branch: "main".to_string(),
            head_commit: "0".repeat(40),
            tree: "1".repeat(40),
            dirty_state: CandidatePreparationDirtyStateV1::Clean,
            cargo_lock_digest: "sha256:v1:aa".to_string(),
            workspace_manifest_digest: "sha256:v1:bb".to_string(),
            member_manifest_digests: BTreeMap::new(),
            topology_generation: SUPPORTED_TOPOLOGY_GENERATION_V1,
            topology_digest: "sha256:v1:cc".to_string(),
            source_release_identity_digest: "sha256:v1:dd".to_string(),
            support_selection_digest: "sha256:v1:ee".to_string(),
            changie_config_digest: "sha256:v1:ff".to_string(),
            changie_history_digest: "sha256:v1:00".to_string(),
            release_record: None,
            github_release_note: None,
            source_exception_policy_schema_version: "0.1".to_string(),
            source_exception_policy_digest: "sha256:v1:11".to_string(),
        }
    }

    fn fixture_input<'a>(
        target: &'a str,
        rows: &'a [CandidatePackageRowV1],
    ) -> CandidateProjectionInput<'a> {
        CandidateProjectionInput {
            target_version_text: target,
            input_identity: identity(),
            topology_rows: rows,
            support_matrix_postures: BTreeMap::new(),
            internal_requirements: BTreeMap::new(),
            external_observations: Vec::new(),
        }
    }

    #[test]
    fn malformed_targets_fail_closed_as_unsupported() {
        for malformed in ["0.2", "0.2.0-beta.1", "0.2.0+build", " 0.2.0"] {
            let rows = vec![product_row("allow-core", 10, "0.2.0-rc.1")];
            let result = prepare_candidate_plan(fixture_input(malformed, &rows));
            assert_eq!(
                result.readiness,
                CandidatePreparationReadinessV1::Unsupported,
                "malformed target {malformed} was not rejected"
            );
            assert!(result.plan.is_none());
        }
    }

    #[test]
    fn stable_target_with_prerelease_posture_is_rejected_by_the_typed_authority() {
        let error = ReleaseIdentityV1::parse("0.2.0", "v0.2.0", true)
            .expect_err("stable target with prerelease posture must fail");
        assert!(matches!(
            error,
            ReleaseIdentityErrorV1::GithubPrereleaseMismatch { .. }
        ));
    }

    #[test]
    fn operation_reuse_of_the_rc_line_is_rejected() {
        let selected = vec![CandidateSelectedRowV1 {
            row: product_row("allow-core", 10, "0.2.0-rc.1"),
            role: "product".to_string(),
            prospective_version: "0.2.0".to_string(),
        }];
        let target = ReleaseVersionV1::parse("0.2.0").expect("target parses");
        let operations = vec![CandidatePreparationOperationV1::SetPackageVersion {
            package: "allow-core".to_string(),
            release_order: 10,
            from: "0.2.0-rc.1".to_string(),
            to: "0.2.0-rc.1".to_string(),
        }];
        let error = validate_candidate_operation_set(&selected, &target, &operations)
            .expect_err("rc.1 identity reuse must be rejected");
        assert!(error.contains("instead of the exact target"), "{error}");
    }

    #[test]
    fn non_exact_internal_requirements_are_rejected() {
        let selected = vec![CandidateSelectedRowV1 {
            row: product_row("allow-core", 10, "0.2.0-rc.1"),
            role: "product".to_string(),
            prospective_version: "0.2.0".to_string(),
        }];
        let target = ReleaseVersionV1::parse("0.2.0").expect("target parses");
        let operations = vec![CandidatePreparationOperationV1::SetInternalRequirement {
            dependency: "allow-core".to_string(),
            from: "=0.2.0-rc.1".to_string(),
            to: "0.2.0".to_string(),
        }];
        let error = validate_candidate_operation_set(&selected, &target, &operations)
            .expect_err("non-exact internal requirement must be rejected");
        assert!(error.contains("exact `=0.2.0`"), "{error}");
    }

    #[test]
    fn operations_may_not_move_shared_or_outside_rows() {
        let shared = CandidateSelectedRowV1 {
            row: shared_row("effortless-repo-protocol", 80),
            role: "shared_prerequisite".to_string(),
            prospective_version: "0.1.0".to_string(),
        };
        let outside = CandidateSelectedRowV1 {
            row: CandidatePackageRowV1 {
                product_family: "cargo-intent".to_string(),
                candidate_inclusion: false,
                ..shared_row("cargo-intent", 360)
            },
            role: "product".to_string(),
            prospective_version: "0.2.0".to_string(),
        };
        let selected = vec![shared, outside];
        let target = ReleaseVersionV1::parse("0.2.0").expect("target parses");
        let move_shared = vec![CandidatePreparationOperationV1::SetPackageVersion {
            package: "effortless-repo-protocol".to_string(),
            release_order: 80,
            from: "0.1.0".to_string(),
            to: "0.2.0".to_string(),
        }];
        assert!(validate_candidate_operation_set(&selected, &target, &move_shared).is_err());
        let move_outside = vec![CandidatePreparationOperationV1::SetPackageVersion {
            package: "cargo-intent".to_string(),
            release_order: 360,
            from: "0.1.0".to_string(),
            to: "0.2.0".to_string(),
        }];
        assert!(validate_candidate_operation_set(&selected, &target, &move_outside).is_err());
    }

    #[test]
    fn outside_families_never_enter_the_closure() {
        let rows = vec![
            product_row("allow-core", 10, "0.2.0-rc.1"),
            CandidatePackageRowV1 {
                product_family: "cargo-intent".to_string(),
                candidate_inclusion: true,
                ..shared_row("cargo-intent", 360)
            },
        ];
        let result = prepare_candidate_plan(fixture_input("0.2.0", &rows));
        assert_eq!(result.readiness, CandidatePreparationReadinessV1::Conflict);
        assert!(result.reasons[0].contains("outside the cargo-allow release closure"));
    }

    #[test]
    fn unsupported_topology_generation_is_explicit() {
        let rows = vec![product_row("allow-core", 10, "0.2.0-rc.1")];
        let mut fixture = fixture_input("0.2.0", &rows);
        fixture.input_identity.topology_generation = 3;
        let result = prepare_candidate_plan(fixture);
        assert_eq!(
            result.readiness,
            CandidatePreparationReadinessV1::Unsupported
        );
        assert!(result.reasons[0].contains("generation 3 is unsupported"));
    }

    #[test]
    fn unknown_dirty_state_is_an_instrument_failure() {
        let rows = vec![product_row("allow-core", 10, "0.2.0-rc.1")];
        let mut fixture = fixture_input("0.2.0", &rows);
        fixture.input_identity.dirty_state = CandidatePreparationDirtyStateV1::Unknown;
        let result = prepare_candidate_plan(fixture);
        assert_eq!(
            result.readiness,
            CandidatePreparationReadinessV1::InstrumentFailure
        );
    }

    #[test]
    fn dirty_state_is_surfaced_as_a_required_decision_not_normalized() {
        let rows = vec![
            product_row("allow-core", 10, "0.2.0-rc.1"),
            product_row("allow-policy", 20, "0.2.0-rc.1"),
            shared_row("effortless-repo-protocol", 80),
        ];
        let mut fixture = fixture_input("0.2.0", &rows);
        fixture.input_identity.dirty_state = CandidatePreparationDirtyStateV1::Dirty {
            modified_paths: 2,
            untracked_paths: 1,
        };
        let result = prepare_candidate_plan(fixture);
        let plan = result.plan.expect("plan projects");
        assert!(
            plan.required_decisions
                .iter()
                .any(|decision| decision.decision_id == "clean-working-tree")
        );
    }

    #[test]
    fn stale_target_equal_to_source_is_explicit() {
        let rows = vec![product_row("allow-core", 10, "0.2.0")];
        let result = prepare_candidate_plan(fixture_input("0.2.0", &rows));
        assert_eq!(result.readiness, CandidatePreparationReadinessV1::Stale);
    }

    #[test]
    fn equal_inputs_produce_equal_plan_digests() {
        let rows = vec![
            product_row("allow-core", 10, "0.2.0-rc.1"),
            product_row("allow-policy", 20, "0.2.0-rc.1"),
            shared_row("effortless-repo-protocol", 80),
        ];
        let first = prepare_candidate_plan(fixture_input("0.2.0", &rows));
        let second = prepare_candidate_plan(fixture_input("0.2.0", &rows));
        let first_digest = first.plan.as_ref().expect("plan").plan_digest.clone();
        let second_digest = second.plan.as_ref().expect("plan").plan_digest.clone();
        assert_eq!(first_digest, second_digest);
    }

    #[test]
    fn projection_moves_products_holds_shared_and_exacts_requirements() {
        let rows = vec![
            product_row("allow-core", 10, "0.2.0-rc.1"),
            product_row("allow-policy", 20, "0.2.0-rc.1"),
            shared_row("effortless-repo-protocol", 80),
            shared_row("effortless-repo-edit", 90),
        ];
        let mut fixture = fixture_input("0.2.0", &rows);
        fixture
            .internal_requirements
            .insert("allow-core".to_string(), "=0.2.0-rc.1".to_string());
        let result = prepare_candidate_plan(fixture);
        let plan = result.plan.expect("plan projects");
        assert_eq!(plan.source_release_identity.version, "0.2.0-rc.1");
        assert_eq!(plan.target_release_identity.version, "0.2.0");
        assert_eq!(plan.target_publication_posture, "projected_not_public");
        let moved: Vec<&str> = plan
            .operations
            .iter()
            .filter_map(|operation| match operation {
                CandidatePreparationOperationV1::SetPackageVersion { package, to, .. } => {
                    assert_eq!(to, "0.2.0");
                    Some(package.as_str())
                }
                _ => None,
            })
            .collect();
        assert_eq!(moved, vec!["allow-core", "allow-policy"]);
        let held: Vec<&str> = plan
            .operations
            .iter()
            .filter_map(|operation| match operation {
                CandidatePreparationOperationV1::HoldExactVersion { package, .. } => {
                    Some(package.as_str())
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            held,
            vec!["effortless-repo-protocol", "effortless-repo-edit"]
        );
        let exacted: Vec<&CandidatePreparationOperationV1> = plan
            .operations
            .iter()
            .filter(|operation| {
                matches!(
                    operation,
                    CandidatePreparationOperationV1::SetInternalRequirement { .. }
                )
            })
            .collect();
        assert_eq!(exacted.len(), 1);
    }
}
