use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

use allow_core::{
    CargoAllowError, CargoAllowErrorKind, CargoAllowResult, normalize_path, read_text_file_capped,
    sha256_v1_bytes,
};
use serde::{Deserialize, Serialize};

use crate::discovery::DiscoverConfigResult;
use crate::federation::{
    FEDERATION_CONFIG_REL_PATH, FederationEvaluation, FederationLoadOutcome, LedgerRole,
    PrecedenceTier, SOURCE_EXCEPTION_LANE, load_federation_config,
    resolve_canonical_ledger_for_lane,
};
use crate::{
    SOURCE_CARGO_METADATA, SOURCE_CONVENTIONAL_PATH, SOURCE_PACKAGE_METADATA,
    SOURCE_WORKSPACE_METADATA, discover_config, evaluate_source_exception_policy,
    parse_policy_with_reportable_evidence_at,
};

pub const RESOLVED_CARGO_ALLOW_CONFIG_SCHEMA_ID: &str = "cargo-allow.resolved-config.v1";
pub const RESOLVED_CARGO_ALLOW_CONFIG_SCHEMA_VERSION: u32 = 1;
pub const RESOLVED_CARGO_ALLOW_CONFIG_CLAIM_BOUNDARY: &str = "Describes cargo-allow's read-only source-exception configuration selection from source text and repository files. It does not approve policy contents, execute Cargo metadata, evaluate findings, authorize writes, or merge profile and federation semantics.";

const CURRENT_ADAPTER_LIMITATION: &str =
    "current_multi_pass_adapter_does_not_prove_atomic_resolution";
const CANDIDATE_ENUMERATION_LIMITATION: &str =
    "current_discovery_stops_after_the_selected_candidate";
const SENSOR_OBSERVATION_LIMITATION: &str =
    "sensor_and_inventory_selection_not_observed_by_policy_adapter";
const ROOT_RELATIONSHIP_LIMITATION: &str =
    "requested_and_repository_root_share_the_callers_resolved_root_input";
const ROOT_RELATIONSHIP_UNKNOWN_LIMITATION: &str =
    "requested_root_relationship_could_not_be_represented_portably";
const EXTERNAL_CLI_LIMITATION: &str =
    "external_cli_policy_identity_is_redacted_and_reported_unsupported";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigResolutionStatusV1 {
    Complete,
    NoPolicy,
    Invalid,
    Partial,
    Ambiguous,
    Unsupported,
    InstrumentFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigCompletenessV1 {
    Complete,
    Partial,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigCandidateSourceV1 {
    CliOverride,
    FederationRegistry,
    PackageMetadata,
    WorkspaceMetadata,
    CargoMetadata,
    ConventionalPath,
    LegacyDiscovery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigCandidateDispositionV1 {
    Selected,
    Available,
    Skipped,
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigPrecedenceTierV1 {
    CliOverride,
    FederationRegistry,
    DiscoveryFallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigFederationPostureV1 {
    Missing,
    Valid,
    Invalid,
    Unreadable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigPathAnchorV1 {
    ResolvedRepositoryRoot,
    DiscoveryAncestor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigRootRelationV1 {
    Same,
    Descendant,
    External,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortableConfigPathV1 {
    pub anchor: ConfigPathAnchorV1,
    pub ancestor_depth: u32,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigCandidateV1 {
    pub source: ConfigCandidateSourceV1,
    pub path: Option<PortableConfigPathV1>,
    pub disposition: ConfigCandidateDispositionV1,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigDiagnosticV1 {
    pub code: String,
    pub kind: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedPolicyV1 {
    pub path: PortableConfigPathV1,
    pub digest: Option<String>,
    pub schema_version: Option<String>,
    pub policy: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigFallbackV1 {
    pub considered: bool,
    pub selected: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigFederationParticipationV1 {
    pub config_path: PortableConfigPathV1,
    pub posture: ConfigFederationPostureV1,
    pub selected_for_source_exception: bool,
    pub configured_ledgers: Vec<String>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigProfileParticipationV1 {
    pub observed: bool,
    pub selected_profile: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedCargoAllowConfigV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub producer_generation: u32,
    pub source_subject: String,
    pub requested_root: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_root_relation: Option<ConfigRootRelationV1>,
    pub resolved_repository_root: String,
    pub status: ConfigResolutionStatusV1,
    pub completeness: ConfigCompletenessV1,
    pub selected_policy: Option<ResolvedPolicyV1>,
    pub selection_source: Option<ConfigCandidateSourceV1>,
    pub precedence_tier: Option<ConfigPrecedenceTierV1>,
    pub explicit_cli_values: Vec<PortableConfigPathV1>,
    pub candidates: Vec<ConfigCandidateV1>,
    pub fallback: ConfigFallbackV1,
    pub federation: ConfigFederationParticipationV1,
    pub profile: ConfigProfileParticipationV1,
    pub inventory_mode: Option<String>,
    pub ignored_scopes: Vec<String>,
    pub generated_scopes: Vec<String>,
    pub selected_sensor_families: Vec<String>,
    pub diagnostics: Vec<ConfigDiagnosticV1>,
    pub limitations: Vec<String>,
    pub claim_boundary: String,
}

/// Observe the current cargo-allow configuration selectors and project their
/// result into a portable v1 contract.
///
/// This is deliberately an adapter over the current selectors. It does not
/// change precedence or make their multi-pass reads atomic. Consumers migrate
/// to a single resolved object separately under issue #3876.
///
/// `root` must already be the resolved source-tree root used by the invoking
/// command. This policy-layer adapter does not invoke Git or rediscover a
/// repository root.
pub fn resolve_cargo_allow_config_v1(
    root: &Path,
    cli_config: Option<&Path>,
    source_subject: &str,
) -> CargoAllowResult<ResolvedCargoAllowConfigV1> {
    resolve_cargo_allow_config_v1_with_requested_root(root, root, cli_config, source_subject)
}

/// Resolve configuration when the caller's requested root is distinct from
/// the repository root selected by its surrounding command.
pub fn resolve_cargo_allow_config_v1_with_requested_root(
    requested_root: &Path,
    root: &Path,
    cli_config: Option<&Path>,
    source_subject: &str,
) -> CargoAllowResult<ResolvedCargoAllowConfigV1> {
    validate_source_subject(root, source_subject)?;
    let resolved_root = match root.canonicalize() {
        Ok(path) => path,
        Err(error) => {
            return Ok(unavailable_resolution(
                source_subject,
                ConfigDiagnosticV1 {
                    code: CargoAllowErrorKind::InstrumentFailure.code().to_string(),
                    kind: CargoAllowErrorKind::InstrumentFailure.as_str().to_string(),
                    message: portable_message(
                        root,
                        &format!("repository root could not be resolved: {error}"),
                    ),
                },
            ));
        }
    };
    validate_source_subject(&resolved_root, source_subject)?;
    let discovery = discover_config(&resolved_root);
    let federation_observation = observe_federation(&resolved_root);
    let evaluation = evaluate_source_exception_policy(&resolved_root, cli_config);

    let (requested_root_identity, requested_root_relation) =
        portable_root_identity(requested_root, &resolved_root);
    Ok(compile_resolution(CompileResolutionInput {
        root: &resolved_root,
        requested_root_identity,
        requested_root_relation: Some(requested_root_relation),
        cli_config,
        source_subject,
        discovery,
        federation_observation,
        evaluation,
    }))
}

struct FederationObservation {
    participation: ConfigFederationParticipationV1,
    source_exception_path: Option<PathBuf>,
    source_exception_ambiguous: bool,
    error: Option<ConfigDiagnosticV1>,
}

struct CompileResolutionInput<'a> {
    root: &'a Path,
    requested_root_identity: String,
    requested_root_relation: Option<ConfigRootRelationV1>,
    cli_config: Option<&'a Path>,
    source_subject: &'a str,
    discovery: DiscoverConfigResult,
    federation_observation: FederationObservation,
    evaluation: Result<(PathBuf, FederationEvaluation), CargoAllowError>,
}

fn compile_resolution(input: CompileResolutionInput<'_>) -> ResolvedCargoAllowConfigV1 {
    let mut federation = input.federation_observation.participation.clone();
    let mut diagnostics = Vec::new();
    if let Some(error) = input.federation_observation.error.clone() {
        diagnostics.push(error);
    }
    let mut candidates = candidates_from_discovery(input.root, &input.discovery);
    let mut fallback = ConfigFallbackV1 {
        considered: false,
        selected: false,
        reason: None,
    };

    let (selected_path, precedence, selection_source, evaluation_error) = match input.evaluation {
        Ok((path, evaluation)) => {
            let source = source_from_evaluation(&evaluation, &input.discovery);
            (
                Some(path),
                Some(ConfigPrecedenceTierV1::from(evaluation.precedence_applied)),
                source,
                None,
            )
        }
        Err(error) => {
            let diagnostic = diagnostic_from_error(&error);
            diagnostics.push(diagnostic);
            if let Some(config) = input.cli_config {
                (
                    Some(input.root.join(config)),
                    Some(ConfigPrecedenceTierV1::CliOverride),
                    Some(ConfigCandidateSourceV1::CliOverride),
                    None,
                )
            } else {
                fallback.considered = true;
                fallback.selected = input.discovery.selected.is_some();
                fallback.reason = Some(error.code().to_string());
                (
                    input.discovery.selected.clone(),
                    None,
                    input.discovery.selected_source.and_then(source_from_label),
                    Some(error.kind()),
                )
            }
        }
    };

    if precedence == Some(ConfigPrecedenceTierV1::DiscoveryFallback)
        && matches!(
            federation.posture,
            ConfigFederationPostureV1::Invalid | ConfigFederationPostureV1::Unreadable
        )
    {
        fallback.considered = true;
        fallback.selected = selected_path.is_some();
        fallback.reason = federation.diagnostics.first().cloned();
    }

    if let Some(config) = input.cli_config {
        candidates.push(candidate_from_cli(
            input.root,
            config,
            selection_source == Some(ConfigCandidateSourceV1::CliOverride),
        ));
    }
    if input.federation_observation.participation.posture != ConfigFederationPostureV1::Missing
        && (input.federation_observation.participation.posture != ConfigFederationPostureV1::Valid
            || input.federation_observation.source_exception_path.is_some())
    {
        let federation_path = input
            .federation_observation
            .source_exception_path
            .as_deref()
            .and_then(|path| portable_config_path(input.root, path, false))
            .or_else(|| {
                (input.federation_observation.participation.posture
                    != ConfigFederationPostureV1::Valid)
                    .then(|| root_relative_config_path(FEDERATION_CONFIG_REL_PATH))
            });
        candidates.push(ConfigCandidateV1 {
            source: ConfigCandidateSourceV1::FederationRegistry,
            path: federation_path,
            disposition: if precedence == Some(ConfigPrecedenceTierV1::FederationRegistry) {
                ConfigCandidateDispositionV1::Selected
            } else if input.federation_observation.participation.posture
                == ConfigFederationPostureV1::Valid
            {
                ConfigCandidateDispositionV1::Available
            } else {
                ConfigCandidateDispositionV1::Invalid
            },
            reason: input
                .federation_observation
                .participation
                .diagnostics
                .first()
                .cloned(),
        });
    }
    federation.selected_for_source_exception =
        precedence == Some(ConfigPrecedenceTierV1::FederationRegistry);
    mark_selected_candidate(
        &mut candidates,
        selected_path.as_deref(),
        selection_source,
        input.root,
    );

    let policy_observation = selected_path
        .as_deref()
        .map(|path| {
            observe_policy(
                input.root,
                path,
                selection_source.is_some_and(source_allows_ancestor),
                selection_source == Some(ConfigCandidateSourceV1::CliOverride)
                    && input.cli_config.is_some_and(|config| {
                        config
                            .components()
                            .any(|component| component == Component::ParentDir)
                    }),
            )
        })
        .unwrap_or_default();
    if selection_source == Some(ConfigCandidateSourceV1::CliOverride)
        && policy_observation.reject_selected_candidate
    {
        for candidate in &mut candidates {
            if candidate.source == ConfigCandidateSourceV1::CliOverride
                && candidate.disposition == ConfigCandidateDispositionV1::Selected
            {
                candidate.disposition = ConfigCandidateDispositionV1::Invalid;
                candidate.reason = policy_observation
                    .error
                    .as_ref()
                    .map(|diagnostic| diagnostic.message.clone());
            }
        }
    }
    sort_candidates(&mut candidates);
    if let Some(error) = policy_observation.error.clone() {
        diagnostics.push(error);
    }

    let ambiguous = input.federation_observation.source_exception_ambiguous
        && precedence != Some(ConfigPrecedenceTierV1::CliOverride);
    let status = resolution_status(ResolutionStatusInput {
        selected_policy: policy_observation.policy.as_ref(),
        status_override: policy_observation.status_override,
        evaluation_error,
        fallback_selected: fallback.selected,
        ambiguous,
        federation_posture: federation.posture,
        no_policy_observed: input.cli_config.is_none()
            && input.discovery.selected.is_none()
            && input.discovery.skipped.is_empty()
            && matches!(
                federation.posture,
                ConfigFederationPostureV1::Missing | ConfigFederationPostureV1::Valid
            ),
        diagnostics: &diagnostics,
    });

    let mut limitations = vec![
        CURRENT_ADAPTER_LIMITATION.to_string(),
        CANDIDATE_ENUMERATION_LIMITATION.to_string(),
        SENSOR_OBSERVATION_LIMITATION.to_string(),
        EXTERNAL_CLI_LIMITATION.to_string(),
    ];
    match input.requested_root_relation {
        Some(ConfigRootRelationV1::Same) => {
            limitations.push(ROOT_RELATIONSHIP_LIMITATION.to_string())
        }
        None | Some(ConfigRootRelationV1::Unknown) => {
            limitations.push(ROOT_RELATIONSHIP_UNKNOWN_LIMITATION.to_string())
        }
        Some(ConfigRootRelationV1::Descendant | ConfigRootRelationV1::External) => {}
    }

    ResolvedCargoAllowConfigV1 {
        schema_id: RESOLVED_CARGO_ALLOW_CONFIG_SCHEMA_ID.to_string(),
        schema_version: RESOLVED_CARGO_ALLOW_CONFIG_SCHEMA_VERSION,
        producer_generation: 1,
        source_subject: input.source_subject.to_string(),
        requested_root: input.requested_root_identity.clone(),
        requested_root_relation: input.requested_root_relation,
        resolved_repository_root: ".".to_string(),
        status,
        completeness: ConfigCompletenessV1::Partial,
        selected_policy: policy_observation.policy,
        selection_source,
        precedence_tier: precedence,
        explicit_cli_values: input
            .cli_config
            .and_then(|path| portable_joined_path(input.root, path, false))
            .into_iter()
            .collect(),
        candidates,
        fallback,
        federation,
        profile: ConfigProfileParticipationV1 {
            observed: false,
            selected_profile: None,
            reason: "profile resolution is a separate current consumer".to_string(),
        },
        inventory_mode: policy_observation.inventory_mode,
        ignored_scopes: policy_observation.ignored_scopes,
        generated_scopes: policy_observation.generated_scopes,
        selected_sensor_families: policy_observation.selected_sensor_families,
        diagnostics,
        limitations,
        claim_boundary: RESOLVED_CARGO_ALLOW_CONFIG_CLAIM_BOUNDARY.to_string(),
    }
}

fn portable_root_identity(
    requested_root: &Path,
    resolved_root: &Path,
) -> (String, ConfigRootRelationV1) {
    let requested = requested_root.canonicalize().ok();
    let resolved = resolved_root.canonicalize().ok();
    let Some((requested, resolved)) = requested.as_deref().zip(resolved.as_deref()) else {
        return ("unknown".to_string(), ConfigRootRelationV1::Unknown);
    };
    if requested == resolved {
        return (".".to_string(), ConfigRootRelationV1::Same);
    }
    if let Some(relative) = lexical_relative_path(resolved, requested) {
        return (
            if relative.is_empty() {
                ".".to_string()
            } else {
                relative
            },
            ConfigRootRelationV1::Descendant,
        );
    }
    ("external".to_string(), ConfigRootRelationV1::External)
}

fn observe_federation(root: &Path) -> FederationObservation {
    match load_federation_config(root) {
        Ok(loaded) => match loaded.outcome {
            FederationLoadOutcome::Missing => FederationObservation {
                participation: ConfigFederationParticipationV1 {
                    config_path: root_relative_config_path(&loaded.path),
                    posture: ConfigFederationPostureV1::Missing,
                    selected_for_source_exception: false,
                    configured_ledgers: Vec::new(),
                    diagnostics: Vec::new(),
                },
                source_exception_path: None,
                source_exception_ambiguous: false,
                error: None,
            },
            FederationLoadOutcome::Parsed(validated) => {
                let has_non_portable_ledger_id = validated.config.ledgers.iter().any(|ledger| {
                    !ledger.id.trim().is_empty() && !is_portable_ledger_identity(&ledger.id)
                });
                let source_exception_path = validated
                    .valid
                    .then(|| {
                        resolve_canonical_ledger_for_lane(&validated.config, SOURCE_EXCEPTION_LANE)
                    })
                    .flatten()
                    .map(|ledger| root.join(&ledger.path));
                let source_exception_ambiguous = validated
                    .config
                    .ledgers
                    .iter()
                    .filter(|ledger| {
                        ledger.role == LedgerRole::Canonical
                            && ledger
                                .lanes
                                .iter()
                                .any(|lane| lane == SOURCE_EXCEPTION_LANE)
                    })
                    .count()
                    > 1;
                FederationObservation {
                    participation: ConfigFederationParticipationV1 {
                        config_path: root_relative_config_path(&loaded.path),
                        posture: if validated.valid && !has_non_portable_ledger_id {
                            ConfigFederationPostureV1::Valid
                        } else {
                            ConfigFederationPostureV1::Invalid
                        },
                        selected_for_source_exception: false,
                        configured_ledgers: validated
                            .config
                            .ledgers
                            .iter()
                            .filter(|ledger| is_portable_ledger_identity(&ledger.id))
                            .map(|ledger| ledger.id.clone())
                            .collect(),
                        diagnostics: validated
                            .diagnostics
                            .iter()
                            .map(|diagnostic| diagnostic.kind.as_str().to_string())
                            .chain(
                                has_non_portable_ledger_id.then(|| "invalid_ledger_id".to_string()),
                            )
                            .collect(),
                    },
                    source_exception_path,
                    source_exception_ambiguous,
                    error: None,
                }
            }
        },
        Err(error) => unreadable_federation_observation(error),
    }
}

fn unreadable_federation_observation(error: CargoAllowError) -> FederationObservation {
    FederationObservation {
        participation: ConfigFederationParticipationV1 {
            config_path: root_relative_config_path(FEDERATION_CONFIG_REL_PATH),
            posture: ConfigFederationPostureV1::Unreadable,
            selected_for_source_exception: false,
            configured_ledgers: Vec::new(),
            diagnostics: vec![error.code().to_string()],
        },
        source_exception_path: None,
        source_exception_ambiguous: false,
        error: Some(diagnostic_from_error(&error)),
    }
}

fn candidates_from_discovery(
    root: &Path,
    discovery: &DiscoverConfigResult,
) -> Vec<ConfigCandidateV1> {
    let mut candidates = Vec::new();
    if let (Some(path), Some(source)) = (&discovery.selected, discovery.selected_source) {
        let portable = portable_config_path(root, path, true);
        candidates.push(ConfigCandidateV1 {
            source: source_from_label(source).unwrap_or(ConfigCandidateSourceV1::ConventionalPath),
            path: portable.clone(),
            disposition: ConfigCandidateDispositionV1::Available,
            reason: portable.is_none().then(|| {
                "selected current winner is outside the portable repository boundary".to_string()
            }),
        });
    }
    candidates.extend(discovery.skipped.iter().map(|candidate| ConfigCandidateV1 {
        source:
            source_from_label(candidate.source).unwrap_or(ConfigCandidateSourceV1::LegacyDiscovery),
        path: portable_config_path(root, &candidate.path, true),
        disposition: ConfigCandidateDispositionV1::Skipped,
        reason: Some(portable_message(root, &candidate.reason)),
    }));
    candidates
}

fn candidate_from_cli(root: &Path, config: &Path, cli_selected: bool) -> ConfigCandidateV1 {
    let joined = root.join(config);
    let portable = portable_config_path(root, &joined, false);
    let is_safe = portable.is_some();
    ConfigCandidateV1 {
        source: ConfigCandidateSourceV1::CliOverride,
        path: portable,
        disposition: if cli_selected && is_safe {
            ConfigCandidateDispositionV1::Selected
        } else if is_safe {
            ConfigCandidateDispositionV1::Available
        } else {
            ConfigCandidateDispositionV1::Invalid
        },
        reason: (!is_safe)
            .then(|| "CLI path cannot be represented as a portable identity".to_string()),
    }
}

fn mark_selected_candidate(
    candidates: &mut [ConfigCandidateV1],
    selected: Option<&Path>,
    selected_source: Option<ConfigCandidateSourceV1>,
    root: &Path,
) {
    let allow_ancestor = selected_source.is_some_and(source_allows_ancestor);
    let selected = selected.and_then(|path| portable_config_path(root, path, allow_ancestor));
    if let (Some(selected), Some(selected_source)) = (selected, selected_source) {
        for candidate in candidates {
            if candidate.source == selected_source && candidate.path.as_ref() == Some(&selected) {
                candidate.disposition = ConfigCandidateDispositionV1::Selected;
            }
        }
    }
}

fn sort_candidates(candidates: &mut [ConfigCandidateV1]) {
    candidates.sort_by(|left, right| {
        candidate_source_key(left.source)
            .cmp(&candidate_source_key(right.source))
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| {
                candidate_disposition_key(left.disposition)
                    .cmp(&candidate_disposition_key(right.disposition))
            })
    });
}

fn candidate_source_key(source: ConfigCandidateSourceV1) -> u8 {
    match source {
        ConfigCandidateSourceV1::CliOverride => 0,
        ConfigCandidateSourceV1::FederationRegistry => 1,
        ConfigCandidateSourceV1::PackageMetadata => 2,
        ConfigCandidateSourceV1::WorkspaceMetadata => 3,
        ConfigCandidateSourceV1::CargoMetadata => 4,
        ConfigCandidateSourceV1::ConventionalPath => 5,
        ConfigCandidateSourceV1::LegacyDiscovery => 6,
    }
}

fn candidate_disposition_key(disposition: ConfigCandidateDispositionV1) -> u8 {
    match disposition {
        ConfigCandidateDispositionV1::Selected => 0,
        ConfigCandidateDispositionV1::Available => 1,
        ConfigCandidateDispositionV1::Skipped => 2,
        ConfigCandidateDispositionV1::Invalid => 3,
    }
}

#[derive(Default)]
struct PolicyObservation {
    policy: Option<ResolvedPolicyV1>,
    inventory_mode: Option<String>,
    ignored_scopes: Vec<String>,
    generated_scopes: Vec<String>,
    selected_sensor_families: Vec<String>,
    error: Option<ConfigDiagnosticV1>,
    status_override: Option<ConfigResolutionStatusV1>,
    reject_selected_candidate: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectedPathContainment {
    Contained,
    Rejected,
    RejectedParent,
}

fn unreadable_policy_observation(portable: PortableConfigPathV1) -> PolicyObservation {
    PolicyObservation {
        policy: Some(ResolvedPolicyV1 {
            path: portable,
            digest: None,
            schema_version: None,
            policy: None,
            status: None,
        }),
        error: Some(ConfigDiagnosticV1 {
            code: CargoAllowErrorKind::InvalidConfig.code().to_string(),
            kind: CargoAllowErrorKind::InvalidConfig.as_str().to_string(),
            message: "selected policy could not be read".to_string(),
        }),
        status_override: Some(ConfigResolutionStatusV1::Invalid),
        ..PolicyObservation::default()
    }
}

fn observe_policy(
    root: &Path,
    path: &Path,
    allow_ancestor: bool,
    invalid_unportable_cli: bool,
) -> PolicyObservation {
    let Some(portable) = portable_config_path(root, path, allow_ancestor) else {
        let kind = if invalid_unportable_cli {
            CargoAllowErrorKind::InvalidConfig
        } else {
            CargoAllowErrorKind::Unsupported
        };
        return PolicyObservation {
            error: Some(ConfigDiagnosticV1 {
                code: kind.code().to_string(),
                kind: kind.as_str().to_string(),
                message: "selected current policy cannot be represented inside the portable repository boundary"
                    .to_string(),
            }),
            status_override: Some(if invalid_unportable_cli {
                ConfigResolutionStatusV1::Invalid
            } else {
                ConfigResolutionStatusV1::Unsupported
            }),
            ..PolicyObservation::default()
        };
    };
    let containment = selected_path_containment(root, path, &portable);
    if containment != SelectedPathContainment::Contained {
        return PolicyObservation {
            error: Some(ConfigDiagnosticV1 {
                code: CargoAllowErrorKind::Unsupported.code().to_string(),
                kind: CargoAllowErrorKind::Unsupported.as_str().to_string(),
                message: "selected policy target is outside its authorized portable anchor or is a dangling link"
                    .to_string(),
            }),
            status_override: Some(ConfigResolutionStatusV1::Unsupported),
            reject_selected_candidate:
                containment == SelectedPathContainment::RejectedParent,
            ..PolicyObservation::default()
        };
    }
    // Do not pass directories, FIFOs, sockets, or devices to the text reader.
    // `read_text_file_capped` opens its input synchronously and a non-regular
    // target (notably a FIFO) could otherwise block the whole resolution.
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => {
            return unreadable_policy_observation(portable);
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => {
            return unreadable_policy_observation(portable);
        }
    }
    let text = match read_text_file_capped(path) {
        Ok(text) => text,
        Err(_) => {
            return unreadable_policy_observation(portable);
        }
    };
    let digest = sha256_v1_bytes(text.as_bytes());
    match parse_policy_with_reportable_evidence_at(path, &text) {
        Ok(policy) => {
            let mut selected_sensor_families =
                vec!["rust_source".to_string(), "non_rust_file".to_string()];
            selected_sensor_families.extend(
                policy
                    .workspace
                    .file_families
                    .iter()
                    .map(|family| family.family.clone()),
            );
            selected_sensor_families.sort();
            selected_sensor_families.dedup();
            PolicyObservation {
                inventory_mode: Some(policy.workspace.inventory.clone()),
                ignored_scopes: policy.workspace.ignored.clone(),
                generated_scopes: policy.workspace.generated.clone(),
                selected_sensor_families,
                policy: Some(ResolvedPolicyV1 {
                    path: portable,
                    digest: Some(digest),
                    schema_version: Some(policy.schema_version),
                    policy: Some(policy.policy),
                    status: policy.status,
                }),
                error: None,
                status_override: None,
                reject_selected_candidate: false,
            }
        }
        Err(error) => PolicyObservation {
            policy: Some(ResolvedPolicyV1 {
                path: portable,
                digest: Some(digest),
                schema_version: None,
                policy: None,
                status: None,
            }),
            error: Some(diagnostic_from_error(&error)),
            status_override: Some(ConfigResolutionStatusV1::Invalid),
            ..PolicyObservation::default()
        },
    }
}

struct ResolutionStatusInput<'a> {
    selected_policy: Option<&'a ResolvedPolicyV1>,
    status_override: Option<ConfigResolutionStatusV1>,
    evaluation_error: Option<CargoAllowErrorKind>,
    fallback_selected: bool,
    ambiguous: bool,
    federation_posture: ConfigFederationPostureV1,
    no_policy_observed: bool,
    diagnostics: &'a [ConfigDiagnosticV1],
}

fn resolution_status(input: ResolutionStatusInput<'_>) -> ConfigResolutionStatusV1 {
    if let Some(status) = input.status_override {
        return status;
    }
    if input.ambiguous {
        return ConfigResolutionStatusV1::Ambiguous;
    }
    if let Some(kind) = input.evaluation_error {
        if input.fallback_selected {
            return ConfigResolutionStatusV1::Partial;
        }
        return match kind {
            CargoAllowErrorKind::Unsupported => ConfigResolutionStatusV1::Unsupported,
            CargoAllowErrorKind::InstrumentFailure
            | CargoAllowErrorKind::Inventory
            | CargoAllowErrorKind::Scan
            | CargoAllowErrorKind::Artifact
            | CargoAllowErrorKind::Internal
            | CargoAllowErrorKind::Unknown => ConfigResolutionStatusV1::InstrumentFailure,
            CargoAllowErrorKind::InvalidConfig if input.no_policy_observed => {
                ConfigResolutionStatusV1::NoPolicy
            }
            _ => ConfigResolutionStatusV1::Invalid,
        };
    }
    if input.selected_policy.is_none() {
        if input.diagnostics.is_empty() {
            ConfigResolutionStatusV1::NoPolicy
        } else {
            ConfigResolutionStatusV1::Invalid
        }
    } else if matches!(
        input.federation_posture,
        ConfigFederationPostureV1::Invalid | ConfigFederationPostureV1::Unreadable
    ) {
        ConfigResolutionStatusV1::Partial
    } else if input.diagnostics.is_empty() {
        ConfigResolutionStatusV1::Complete
    } else {
        ConfigResolutionStatusV1::Invalid
    }
}

fn source_from_evaluation(
    evaluation: &FederationEvaluation,
    discovery: &DiscoverConfigResult,
) -> Option<ConfigCandidateSourceV1> {
    match evaluation.precedence_applied {
        PrecedenceTier::CliOverride => Some(ConfigCandidateSourceV1::CliOverride),
        PrecedenceTier::FederationRegistry => Some(ConfigCandidateSourceV1::FederationRegistry),
        PrecedenceTier::DiscoveryFallback => discovery.selected_source.and_then(source_from_label),
    }
}

fn source_from_label(label: &str) -> Option<ConfigCandidateSourceV1> {
    match label {
        SOURCE_PACKAGE_METADATA => Some(ConfigCandidateSourceV1::PackageMetadata),
        SOURCE_WORKSPACE_METADATA => Some(ConfigCandidateSourceV1::WorkspaceMetadata),
        SOURCE_CARGO_METADATA => Some(ConfigCandidateSourceV1::CargoMetadata),
        SOURCE_CONVENTIONAL_PATH => Some(ConfigCandidateSourceV1::ConventionalPath),
        _ => None,
    }
}

fn source_allows_ancestor(source: ConfigCandidateSourceV1) -> bool {
    matches!(
        source,
        ConfigCandidateSourceV1::PackageMetadata
            | ConfigCandidateSourceV1::WorkspaceMetadata
            | ConfigCandidateSourceV1::ConventionalPath
            | ConfigCandidateSourceV1::LegacyDiscovery
    )
}

impl From<PrecedenceTier> for ConfigPrecedenceTierV1 {
    fn from(value: PrecedenceTier) -> Self {
        match value {
            PrecedenceTier::CliOverride => Self::CliOverride,
            PrecedenceTier::FederationRegistry => Self::FederationRegistry,
            PrecedenceTier::DiscoveryFallback => Self::DiscoveryFallback,
        }
    }
}

fn diagnostic_from_error(error: &CargoAllowError) -> ConfigDiagnosticV1 {
    ConfigDiagnosticV1 {
        code: error.code().to_string(),
        kind: error.kind().as_str().to_string(),
        message: format!(
            "cargo-allow configuration resolution reported {}",
            error.kind().as_str()
        ),
    }
}

fn portable_joined_path(
    root: &Path,
    path: &Path,
    allow_ancestor: bool,
) -> Option<PortableConfigPathV1> {
    if path.is_absolute() {
        portable_config_path(root, path, allow_ancestor)
    } else {
        portable_config_path(root, &root.join(path), allow_ancestor)
    }
}

fn validate_source_subject(root: &Path, source_subject: &str) -> CargoAllowResult<()> {
    let root_text = root.display().to_string();
    let normalized_root = normalize_path(root);
    let embeds_private_root = if root.is_absolute() && cfg!(windows) {
        let folded_subject = source_subject.to_lowercase();
        folded_subject.contains(&root_text.to_lowercase())
            || folded_subject.contains(&normalized_root.to_lowercase())
    } else {
        root.is_absolute()
            && (source_subject.contains(&root_text) || source_subject.contains(&normalized_root))
    };
    if !is_portable_opaque_identity(source_subject)
        || Path::new(source_subject).is_absolute()
        || embeds_private_root
    {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            "source subject must be a non-empty portable opaque identity of at most 1024 ASCII identity characters",
        ));
    }
    Ok(())
}

fn is_portable_opaque_identity(value: &str) -> bool {
    const MAX_PORTABLE_IDENTITY_CHARS: usize = 1_024;
    !value.is_empty()
        && value.chars().count() <= MAX_PORTABLE_IDENTITY_CHARS
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, '-' | '_' | '.' | ':' | '@' | '+')
        })
}

fn is_portable_ledger_identity(value: &str) -> bool {
    const MAX_PORTABLE_IDENTITY_CHARS: usize = 1_024;
    !value.is_empty()
        && value.chars().count() <= MAX_PORTABLE_IDENTITY_CHARS
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | '@' | '+')
        })
}

fn portable_config_path(
    root: &Path,
    path: &Path,
    allow_ancestor: bool,
) -> Option<PortableConfigPathV1> {
    let mut anchor = root.to_path_buf();
    let mut ancestor_depth = 0u32;
    loop {
        let relative = lexical_relative_path(&anchor, path).or_else(|| {
            let resolved_anchor = anchor.canonicalize().ok()?;
            let resolved_path = path.canonicalize().ok()?;
            lexical_relative_path(&resolved_anchor, &resolved_path)
        });
        if let Some(relative) = relative {
            let path = if relative.is_empty() {
                ".".to_string()
            } else {
                relative
            };
            if !is_safe_portable_path_text(&path) {
                return None;
            }
            return Some(PortableConfigPathV1 {
                anchor: if ancestor_depth == 0 {
                    ConfigPathAnchorV1::ResolvedRepositoryRoot
                } else {
                    ConfigPathAnchorV1::DiscoveryAncestor
                },
                ancestor_depth,
                path,
            });
        }
        if !allow_ancestor || !anchor.pop() {
            return None;
        }
        ancestor_depth = ancestor_depth.checked_add(1)?;
    }
}

fn lexical_relative_path(anchor: &Path, path: &Path) -> Option<String> {
    if anchor.to_str().is_none() || path.to_str().is_none() {
        return None;
    }
    if path
        .components()
        .any(|component| component == Component::ParentDir)
    {
        return None;
    }
    if let Ok(relative) = path.strip_prefix(anchor) {
        return lossless_relative_path(relative);
    }
    if !cfg!(windows) {
        return None;
    }

    let anchor_components = anchor.components().collect::<Vec<_>>();
    let path_components = path.components().collect::<Vec<_>>();
    if path_components.len() < anchor_components.len()
        || !anchor_components
            .iter()
            .zip(&path_components)
            .all(|(anchor, path)| {
                anchor
                    .as_os_str()
                    .to_str()
                    .zip(path.as_os_str().to_str())
                    .is_some_and(|(anchor, path)| anchor.to_lowercase() == path.to_lowercase())
            })
    {
        return None;
    }
    let relative = path_components.iter().skip(anchor_components.len()).fold(
        PathBuf::new(),
        |mut relative, component| {
            relative.push(component.as_os_str());
            relative
        },
    );
    lossless_relative_path(&relative)
}

fn lossless_relative_path(path: &Path) -> Option<String> {
    if !is_safe_relative_path(path) {
        return None;
    }
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_str()?),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(parts.join("/"))
}

fn root_relative_config_path(path: &str) -> PortableConfigPathV1 {
    let normalized = normalize_path(Path::new(path));
    PortableConfigPathV1 {
        anchor: ConfigPathAnchorV1::ResolvedRepositoryRoot,
        ancestor_depth: 0,
        path: if normalized.is_empty() {
            ".".to_string()
        } else {
            normalized
        },
    }
}

fn selected_path_containment(
    root: &Path,
    path: &Path,
    portable: &PortableConfigPathV1,
) -> SelectedPathContainment {
    let mut anchor = root.to_path_buf();
    for _ in 0..portable.ancestor_depth {
        if !anchor.pop() {
            return SelectedPathContainment::Rejected;
        }
    }
    match path.canonicalize() {
        Ok(target) => {
            if anchor
                .canonicalize()
                .ok()
                .and_then(|resolved_anchor| lexical_relative_path(&resolved_anchor, &target))
                .is_some()
            {
                SelectedPathContainment::Contained
            } else {
                SelectedPathContainment::Rejected
            }
        }
        Err(_) => match fs::symlink_metadata(path) {
            Ok(_) => SelectedPathContainment::Rejected,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                if missing_selected_path_has_authorized_parent(&anchor, path) {
                    SelectedPathContainment::Contained
                } else {
                    SelectedPathContainment::RejectedParent
                }
            }
            Err(_) => SelectedPathContainment::Contained,
        },
    }
}

fn missing_selected_path_has_authorized_parent(anchor: &Path, path: &Path) -> bool {
    let Some(mut current) = path.parent() else {
        return false;
    };
    let Ok(resolved_anchor) = anchor.canonicalize() else {
        return false;
    };
    loop {
        match fs::symlink_metadata(current) {
            Ok(_) => {
                return current
                    .canonicalize()
                    .ok()
                    .and_then(|resolved| lexical_relative_path(&resolved_anchor, &resolved))
                    .is_some();
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(_) => return false,
        }
        let Some(parent) = current.parent() else {
            return false;
        };
        current = parent;
    }
}

fn is_safe_relative_path(path: &Path) -> bool {
    path.to_str().is_some()
        && !(cfg!(unix) && path.to_string_lossy().contains('\\'))
        && !path.is_absolute()
        && !path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
}

fn is_safe_portable_path_text(path: &str) -> bool {
    if path.is_empty() || path.starts_with('/') || path.starts_with('\\') {
        return false;
    }
    let bytes = path.as_bytes();
    if bytes.first().is_some_and(|byte| byte.is_ascii_alphabetic()) && bytes.get(1) == Some(&b':') {
        return false;
    }
    !path.split(['/', '\\']).any(|segment| segment == "..")
}

fn portable_message(root: &Path, message: &str) -> String {
    let mut portable = message.to_string();
    for (depth, anchor) in root
        .ancestors()
        .take_while(|anchor| anchor.parent().is_some())
        .enumerate()
    {
        let replacement = if depth == 0 {
            ".".to_string()
        } else {
            format!("<discovery_ancestor:{depth}>")
        };
        portable = portable.replace(&anchor.display().to_string(), &replacement);
        portable = portable.replace(&normalize_path(anchor), &replacement);
    }
    bounded_message(&portable)
}

fn bounded_message(message: &str) -> String {
    const MAX_CHARS: usize = 512;
    let mut bounded = message.chars().take(MAX_CHARS).collect::<String>();
    if message.chars().count() > MAX_CHARS {
        bounded.push_str("...");
    }
    bounded
}

fn unavailable_resolution(
    source_subject: &str,
    diagnostic: ConfigDiagnosticV1,
) -> ResolvedCargoAllowConfigV1 {
    ResolvedCargoAllowConfigV1 {
        schema_id: RESOLVED_CARGO_ALLOW_CONFIG_SCHEMA_ID.to_string(),
        schema_version: RESOLVED_CARGO_ALLOW_CONFIG_SCHEMA_VERSION,
        producer_generation: 1,
        source_subject: source_subject.to_string(),
        requested_root: "unknown".to_string(),
        requested_root_relation: Some(ConfigRootRelationV1::Unknown),
        resolved_repository_root: ".".to_string(),
        status: ConfigResolutionStatusV1::InstrumentFailure,
        completeness: ConfigCompletenessV1::Unavailable,
        selected_policy: None,
        selection_source: None,
        precedence_tier: None,
        explicit_cli_values: Vec::new(),
        candidates: Vec::new(),
        fallback: ConfigFallbackV1 {
            considered: false,
            selected: false,
            reason: None,
        },
        federation: ConfigFederationParticipationV1 {
            config_path: root_relative_config_path(FEDERATION_CONFIG_REL_PATH),
            posture: ConfigFederationPostureV1::Unreadable,
            selected_for_source_exception: false,
            configured_ledgers: Vec::new(),
            diagnostics: vec![diagnostic.code.clone()],
        },
        profile: ConfigProfileParticipationV1 {
            observed: false,
            selected_profile: None,
            reason: "repository root was unavailable".to_string(),
        },
        inventory_mode: None,
        ignored_scopes: Vec::new(),
        generated_scopes: Vec::new(),
        selected_sensor_families: Vec::new(),
        diagnostics: vec![diagnostic],
        limitations: vec![
            CURRENT_ADAPTER_LIMITATION.to_string(),
            ROOT_RELATIONSHIP_UNKNOWN_LIMITATION.to_string(),
        ],
        claim_boundary: RESOLVED_CARGO_ALLOW_CONFIG_CLAIM_BOUNDARY.to_string(),
    }
}

#[cfg(test)]
mod tests;
