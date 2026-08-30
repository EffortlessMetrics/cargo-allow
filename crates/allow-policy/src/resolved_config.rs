use std::path::{Component, Path, PathBuf};

use allow_core::{
    CargoAllowError, CargoAllowErrorKind, CargoAllowResult, normalize_path, read_text_file_capped,
    sha256_v1_bytes,
};
use serde::{Deserialize, Serialize};

use crate::discovery::DiscoverConfigResult;
use crate::discovery::skipped_metadata_candidate_source;
use crate::federation::{
    FEDERATION_CONFIG_REL_PATH, FederationDiagnosticKind, FederationEvaluation,
    FederationLoadOutcome, PrecedenceTier, load_federation_config,
};
use crate::{
    DISCOVERY_REL_PATHS, SOURCE_CONVENTIONAL_PATH, SOURCE_PACKAGE_METADATA,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigCandidateV1 {
    pub source: ConfigCandidateSourceV1,
    pub path: Option<String>,
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
    pub path: String,
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
    pub config_path: String,
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
    pub resolved_repository_root: String,
    pub status: ConfigResolutionStatusV1,
    pub completeness: ConfigCompletenessV1,
    pub selected_policy: Option<ResolvedPolicyV1>,
    pub selection_source: Option<ConfigCandidateSourceV1>,
    pub precedence_tier: Option<ConfigPrecedenceTierV1>,
    pub explicit_cli_values: Vec<String>,
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

    Ok(compile_resolution(CompileResolutionInput {
        root: &resolved_root,
        cli_config,
        source_subject,
        discovery,
        federation_observation,
        evaluation,
    }))
}

struct FederationObservation {
    participation: ConfigFederationParticipationV1,
    ambiguous: bool,
    error: Option<ConfigDiagnosticV1>,
}

struct CompileResolutionInput<'a> {
    root: &'a Path,
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
            let diagnostic = diagnostic_from_error(input.root, &error);
            fallback.considered = true;
            fallback.selected = input.discovery.selected.is_some();
            fallback.reason = Some(diagnostic.code.clone());
            diagnostics.push(diagnostic);
            (
                input.discovery.selected.clone(),
                None,
                input.discovery.selected_source.and_then(source_from_label),
                Some(error.kind()),
            )
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
    if input.federation_observation.participation.posture != ConfigFederationPostureV1::Missing {
        let federation_path = if precedence == Some(ConfigPrecedenceTierV1::FederationRegistry) {
            selected_path
                .as_deref()
                .and_then(|path| portable_path(input.root, path))
        } else {
            Some(FEDERATION_CONFIG_REL_PATH.to_string())
        };
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
    sort_candidates(&mut candidates);

    let policy_observation = selected_path
        .as_deref()
        .map(|path| observe_policy(input.root, path))
        .unwrap_or_default();
    if let Some(error) = policy_observation.error.clone() {
        diagnostics.push(error);
    }

    let ambiguous = input.federation_observation.ambiguous;
    let status = resolution_status(
        policy_observation.policy.as_ref(),
        evaluation_error,
        fallback.selected,
        ambiguous,
        federation.posture,
        input.cli_config.is_none()
            && input.discovery.selected.is_none()
            && input.discovery.skipped.is_empty()
            && federation.posture == ConfigFederationPostureV1::Missing,
        &diagnostics,
    );

    ResolvedCargoAllowConfigV1 {
        schema_id: RESOLVED_CARGO_ALLOW_CONFIG_SCHEMA_ID.to_string(),
        schema_version: RESOLVED_CARGO_ALLOW_CONFIG_SCHEMA_VERSION,
        producer_generation: 1,
        source_subject: input.source_subject.to_string(),
        requested_root: ".".to_string(),
        resolved_repository_root: ".".to_string(),
        status,
        completeness: ConfigCompletenessV1::Partial,
        selected_policy: policy_observation.policy,
        selection_source,
        precedence_tier: precedence,
        explicit_cli_values: input
            .cli_config
            .and_then(|path| portable_joined_path(input.root, path))
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
        inventory_mode: None,
        ignored_scopes: policy_observation.ignored_scopes,
        generated_scopes: policy_observation.generated_scopes,
        selected_sensor_families: Vec::new(),
        diagnostics,
        limitations: vec![
            CURRENT_ADAPTER_LIMITATION.to_string(),
            CANDIDATE_ENUMERATION_LIMITATION.to_string(),
            SENSOR_OBSERVATION_LIMITATION.to_string(),
            ROOT_RELATIONSHIP_LIMITATION.to_string(),
        ],
        claim_boundary: RESOLVED_CARGO_ALLOW_CONFIG_CLAIM_BOUNDARY.to_string(),
    }
}

fn observe_federation(root: &Path) -> FederationObservation {
    match load_federation_config(root) {
        Ok(loaded) => match loaded.outcome {
            FederationLoadOutcome::Missing => FederationObservation {
                participation: ConfigFederationParticipationV1 {
                    config_path: loaded.path,
                    posture: ConfigFederationPostureV1::Missing,
                    selected_for_source_exception: false,
                    configured_ledgers: Vec::new(),
                    diagnostics: Vec::new(),
                },
                ambiguous: false,
                error: None,
            },
            FederationLoadOutcome::Parsed(validated) => {
                let ambiguous = validated.diagnostics.iter().any(|diagnostic| {
                    matches!(
                        diagnostic.kind,
                        FederationDiagnosticKind::DuplicateCanonicalLane
                            | FederationDiagnosticKind::PriorityTie
                    )
                });
                FederationObservation {
                    participation: ConfigFederationParticipationV1 {
                        config_path: loaded.path,
                        posture: if validated.valid {
                            ConfigFederationPostureV1::Valid
                        } else {
                            ConfigFederationPostureV1::Invalid
                        },
                        selected_for_source_exception: false,
                        configured_ledgers: validated
                            .config
                            .ledgers
                            .iter()
                            .map(|ledger| ledger.id.clone())
                            .collect(),
                        diagnostics: validated
                            .diagnostics
                            .iter()
                            .map(|diagnostic| diagnostic.kind.as_str().to_string())
                            .collect(),
                    },
                    ambiguous,
                    error: None,
                }
            }
        },
        Err(error) => FederationObservation {
            participation: ConfigFederationParticipationV1 {
                config_path: FEDERATION_CONFIG_REL_PATH.to_string(),
                posture: ConfigFederationPostureV1::Unreadable,
                selected_for_source_exception: false,
                configured_ledgers: Vec::new(),
                diagnostics: vec![error.code().to_string()],
            },
            ambiguous: false,
            error: Some(diagnostic_from_error(root, &error)),
        },
    }
}

fn candidates_from_discovery(
    root: &Path,
    discovery: &DiscoverConfigResult,
) -> Vec<ConfigCandidateV1> {
    let mut candidates = Vec::new();
    if let (Some(path), Some(source)) = (&discovery.selected, discovery.selected_source) {
        candidates.push(ConfigCandidateV1 {
            source: source_from_label(source).unwrap_or(ConfigCandidateSourceV1::ConventionalPath),
            path: portable_path(root, path),
            disposition: ConfigCandidateDispositionV1::Available,
            reason: None,
        });
    }
    candidates.extend(discovery.skipped.iter().map(|candidate| {
        ConfigCandidateV1 {
            source: skipped_metadata_candidate_source(root, &candidate.path)
                .and_then(source_from_label)
                .unwrap_or_else(|| {
                    if is_conventional_candidate(root, &candidate.path) {
                        ConfigCandidateSourceV1::ConventionalPath
                    } else {
                        ConfigCandidateSourceV1::LegacyDiscovery
                    }
                }),
            path: portable_path(root, &candidate.path),
            disposition: ConfigCandidateDispositionV1::Skipped,
            reason: Some(portable_message(root, &candidate.reason)),
        }
    }));
    candidates
}

fn candidate_from_cli(root: &Path, config: &Path, cli_selected: bool) -> ConfigCandidateV1 {
    let joined = root.join(config);
    let portable = portable_path(root, &joined);
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
            .then(|| "absolute or parent-traversing CLI path is not portable".to_string()),
    }
}

fn is_conventional_candidate(root: &Path, candidate: &Path) -> bool {
    let mut dir = root.to_path_buf();
    loop {
        if DISCOVERY_REL_PATHS
            .iter()
            .any(|relative| dir.join(relative) == candidate)
        {
            return true;
        }
        if !dir.pop() {
            return false;
        }
    }
}

fn mark_selected_candidate(
    candidates: &mut [ConfigCandidateV1],
    selected: Option<&Path>,
    selected_source: Option<ConfigCandidateSourceV1>,
    root: &Path,
) {
    let selected = selected.and_then(|path| portable_path(root, path));
    if let (Some(selected), Some(selected_source)) = (selected, selected_source) {
        for candidate in candidates {
            if candidate.source == selected_source
                && candidate.path.as_deref() == Some(selected.as_str())
            {
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
        ConfigCandidateSourceV1::ConventionalPath => 4,
        ConfigCandidateSourceV1::LegacyDiscovery => 5,
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
    ignored_scopes: Vec<String>,
    generated_scopes: Vec<String>,
    error: Option<ConfigDiagnosticV1>,
}

fn observe_policy(root: &Path, path: &Path) -> PolicyObservation {
    let Some(portable) = portable_path(root, path) else {
        return PolicyObservation {
            error: Some(ConfigDiagnosticV1 {
                code: CargoAllowErrorKind::InvalidConfig.code().to_string(),
                kind: CargoAllowErrorKind::InvalidConfig.as_str().to_string(),
                message: "selected policy is outside the repository root".to_string(),
            }),
            ..PolicyObservation::default()
        };
    };
    let text = match read_text_file_capped(path) {
        Ok(text) => text,
        Err(error) => {
            return PolicyObservation {
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
                    message: bounded_message(&format!(
                        "selected policy could not be read: {error}"
                    )),
                }),
                ..PolicyObservation::default()
            };
        }
    };
    let digest = sha256_v1_bytes(text.as_bytes());
    match parse_policy_with_reportable_evidence_at(path, &text) {
        Ok(policy) => PolicyObservation {
            ignored_scopes: policy.workspace.ignored.clone(),
            generated_scopes: policy.workspace.generated.clone(),
            policy: Some(ResolvedPolicyV1 {
                path: portable,
                digest: Some(digest),
                schema_version: Some(policy.schema_version),
                policy: Some(policy.policy),
                status: policy.status,
            }),
            error: None,
        },
        Err(error) => PolicyObservation {
            policy: Some(ResolvedPolicyV1 {
                path: portable,
                digest: Some(digest),
                schema_version: None,
                policy: None,
                status: None,
            }),
            error: Some(diagnostic_from_error(root, &error)),
            ..PolicyObservation::default()
        },
    }
}

fn resolution_status(
    selected_policy: Option<&ResolvedPolicyV1>,
    evaluation_error: Option<CargoAllowErrorKind>,
    fallback_selected: bool,
    ambiguous: bool,
    federation_posture: ConfigFederationPostureV1,
    no_policy_observed: bool,
    diagnostics: &[ConfigDiagnosticV1],
) -> ConfigResolutionStatusV1 {
    if ambiguous {
        return ConfigResolutionStatusV1::Ambiguous;
    }
    if let Some(kind) = evaluation_error {
        if fallback_selected {
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
            CargoAllowErrorKind::InvalidConfig if no_policy_observed => {
                ConfigResolutionStatusV1::NoPolicy
            }
            _ => ConfigResolutionStatusV1::Invalid,
        };
    }
    if selected_policy.is_none() {
        if diagnostics.is_empty() {
            ConfigResolutionStatusV1::NoPolicy
        } else {
            ConfigResolutionStatusV1::Invalid
        }
    } else if matches!(
        federation_posture,
        ConfigFederationPostureV1::Invalid | ConfigFederationPostureV1::Unreadable
    ) {
        ConfigResolutionStatusV1::Partial
    } else if diagnostics.is_empty() {
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
        SOURCE_CONVENTIONAL_PATH => Some(ConfigCandidateSourceV1::ConventionalPath),
        _ => None,
    }
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

fn diagnostic_from_error(root: &Path, error: &CargoAllowError) -> ConfigDiagnosticV1 {
    ConfigDiagnosticV1 {
        code: error.code().to_string(),
        kind: error.kind().as_str().to_string(),
        message: portable_message(root, &error.to_string()),
    }
}

fn portable_joined_path(root: &Path, path: &Path) -> Option<String> {
    if path.is_absolute() {
        portable_path(root, path)
    } else {
        portable_path(root, &root.join(path))
    }
}

fn validate_source_subject(root: &Path, source_subject: &str) -> CargoAllowResult<()> {
    const MAX_SOURCE_SUBJECT_CHARS: usize = 1_024;
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
    if source_subject.is_empty()
        || source_subject.chars().count() > MAX_SOURCE_SUBJECT_CHARS
        || !source_subject.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, '-' | '_' | '.' | ':' | '@' | '+')
        })
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

fn portable_path(root: &Path, path: &Path) -> Option<String> {
    if let Ok(relative) = path.strip_prefix(root) {
        return is_safe_relative_path(relative).then(|| normalize_path(relative));
    }
    let canonical = path.canonicalize().ok()?;
    let relative = canonical.strip_prefix(root).ok()?;
    is_safe_relative_path(relative).then(|| normalize_path(relative))
}

fn is_safe_relative_path(path: &Path) -> bool {
    !path.is_absolute()
        && !path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
}

fn portable_message(root: &Path, message: &str) -> String {
    let root_text = root.display().to_string();
    let normalized_root = normalize_path(root);
    let without_native = message.replace(&root_text, ".");
    let without_normalized = without_native.replace(&normalized_root, ".");
    bounded_message(&without_normalized)
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
        requested_root: ".".to_string(),
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
            config_path: FEDERATION_CONFIG_REL_PATH.to_string(),
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
        limitations: vec![CURRENT_ADAPTER_LIMITATION.to_string()],
        claim_boundary: RESOLVED_CARGO_ALLOW_CONFIG_CLAIM_BOUNDARY.to_string(),
    }
}

#[cfg(test)]
mod tests;
