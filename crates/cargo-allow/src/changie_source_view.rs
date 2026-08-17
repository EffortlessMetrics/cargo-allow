//! Exact repository source-view adapter for the Changie sensor (#3622).
#![expect(
    dead_code,
    reason = "policy:allow-11122: changie source-view adapter awaits the #3623 projection leaf"
)]
//!
//! One adapter turns every supported exact cargo-allow source view —
//! saved worktree, staged index, or committed tree — into the same pure
//! `allow_files::changie` request/result. Acquisition and selection
//! live here; the sensor stays process/filesystem/Git-neutral. Config
//! and fragment bytes/facts are provably from one source generation
//! because every read goes through the single selected view handle:
//! staged and committed analysis structurally cannot fall back to dirty
//! worktree bytes.

use allow_files::changie::ChangieRepoPath;
use allow_files::changie::ChangieSourceDocument;
use allow_files::changie_lint::ChangieLintCandidate;
use allow_files::changie_lint::ChangieLintReport;
use allow_files::changie_lint::sensor::ChangieSensor;
use allow_files::changie_lint::{ChangieCandidateEntry, ChangieEntryState};
use effortless_repo_snapshot::RepositorySourceView;
use effortless_repo_snapshot::StagedPathStatus;

/// How the Changie configuration was selected (#3622). No ambient
/// `CHANGIE_CONFIG_PATH` is inherited: selection is explicit input or
/// the default-name precedence of the modeled generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangieConfigSelectionV1 {
    /// The caller named an exact repository-relative path.
    Explicit(String),
    /// Default-name discovery: `.changie.yaml` first, then
    /// `.changie.yml`, mirroring the modeled generation's lookup order.
    DefaultNames,
}

/// The retained record of what config selection found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangieConfigSelectionRecord {
    pub mode: ChangieConfigSelectionV1,
    /// The repository-relative path actually selected.
    pub selected_path: String,
    /// Default-name candidates present in the view (for ambiguity).
    pub candidates_present: Vec<String>,
    /// True when both default names exist; precedence still applies but
    /// the conflict stays visible instead of being silently resolved.
    pub ambiguous: bool,
}

/// One candidate fragment entry as observed in the selected view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangiePopulationEntry {
    pub repo_path: String,
    pub state: ChangieEntryState,
    /// Present when the entry's bytes were read and parsed; None for
    /// entries the population reports without a parseable document.
    pub content_identity: Option<allow_files::changie::ChangieContentIdentity>,
}

/// Internal assembly bundle for population enumeration.
struct PopulationAssembly {
    root: String,
    population: Vec<ChangiePopulationEntry>,
    entries: Vec<ChangieCandidateEntry>,
    omitted: Vec<(String, String)>,
}

/// The fragment population derived from the selected config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangieSourcePopulationV1 {
    /// Normalized `<changesDir>/<unreleasedDir>` root the config
    /// declared, rediscovered inside the same view whenever the config
    /// path fields change.
    pub root: String,
    /// Entries under the root the adapter inspected (any extension or
    /// nesting the view exposes — classification is the sensor's).
    pub inspected: Vec<ChangiePopulationEntry>,
    /// Entries considered but omitted (for example: bytes that could
    /// not be read as UTF-8), with the reason retained.
    pub omitted: Vec<(String, String)>,
}

/// View kinds the adapter supports. Anything else must be rejected
/// explicitly — the adapter never gains a Changie-only reader for an
/// unsupported view (falsifier 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangieSourceViewKind {
    SavedWorktree,
    StagedIndex,
    CommittedTree,
}

impl ChangieSourceViewKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SavedWorktree => "saved_worktree",
            Self::StagedIndex => "staged_index",
            Self::CommittedTree => "committed_tree",
        }
    }
}

/// Acquisition completeness for the analysis result: non-clean on any
/// partial, unsupported, or instrument-failed state (falsifier 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangieAcquisitionCompleteness {
    Complete,
    Partial,
    NotProven,
}

impl ChangieAcquisitionCompleteness {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::NotProven => "not_proven",
        }
    }
}

/// The canonical analysis result: the pure sensor report plus the
/// exact-source identities that prove which bytes and population were
/// analyzed. It never flattens to `clean: bool`.
#[derive(Debug, Clone)]
pub struct ChangieAnalysisResultV1 {
    pub generation: &'static str,
    pub view_kind: ChangieSourceViewKind,
    /// Exact view identity where the view provides one (staged semantic
    /// hash or committed revision); None for the saved worktree.
    pub view_identity: Option<String>,
    pub config_selection: ChangieConfigSelectionRecord,
    pub config_content_identity: Option<allow_files::changie::ChangieContentIdentity>,
    pub population: ChangieSourcePopulationV1,
    pub report: ChangieLintReport,
    pub completeness: ChangieAcquisitionCompleteness,
    /// Adapter and view limitations retained for the consumer.
    pub limitations: Vec<String>,
    /// Deterministic analysis identity over view identity, config
    /// identity, and population identities — traversal-order
    /// independent (falsifier 9).
    pub analysis_identity: String,
}

/// Errors the adapter can fail closed with. Acquisition failures are
/// errors, never an empty clean population.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangieSourceViewError {
    /// The config selection found nothing readable in the view.
    ConfigNotFound { mode: String, view: &'static str },
    /// The selected config exists in the view but its bytes could not
    /// be read or decoded. Malformed *content* is not an error — it
    /// flows into the sensor as a malformed document — but unreadable
    /// bytes are.
    ConfigUnreadable { path: String, reason: String },
    /// A config path field is not a safe repository-relative path
    /// (falsifier 8).
    ConfigPathUnsafe { path: String, reason: String },
    /// The caller requested a view kind this adapter does not support.
    UnsupportedView(&'static str),
}

impl std::fmt::Display for ChangieSourceViewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConfigNotFound { mode, view } => {
                write!(f, "no Changie config found for {mode} in the {view} view")
            }
            Self::ConfigUnreadable { path, reason } => {
                write!(f, "config {path} could not be read from the view: {reason}")
            }
            Self::ConfigPathUnsafe { path, reason } => {
                write!(f, "config path {path} is not repository-relative: {reason}")
            }
            Self::UnsupportedView(kind) => {
                write!(f, "source view kind {kind} has no exact adapter")
            }
        }
    }
}

/// Run one exact analysis: select the config in the view, parse it from
/// the view's bytes, derive the fragment population from the parsed
/// config inside the same view, and lint through the pure sensor.
pub fn analyze_source_view(
    view: &RepositorySourceView,
    selection: &ChangieConfigSelectionV1,
) -> Result<ChangieAnalysisResultV1, ChangieSourceViewError> {
    let view_kind = view_kind_of(view);
    let view_identity = view.source_identity().map(str::to_string);
    let mut limitations: Vec<String> = view.limitations().to_vec();
    let mut completeness = ChangieAcquisitionCompleteness::Complete;

    // --- config selection ---------------------------------------------------
    let selection_record = select_config(view, selection)?;
    let config_bytes = view
        .read_bytes(std::path::Path::new(&selection_record.selected_path))
        .map_err(|err| ChangieSourceViewError::ConfigUnreadable {
            path: selection_record.selected_path.clone(),
            reason: err.to_string(),
        })?;
    if selection_record.ambiguous {
        limitations.push(format!(
            "both .changie.yaml and .changie.yml exist; precedence selected {}",
            selection_record.selected_path
        ));
    }
    let config_content_identity = allow_files::changie::ChangieContentIdentity::of(&config_bytes);
    let config_repo_path = ChangieRepoPath::from_repo_relative(&selection_record.selected_path)
        .map_err(|err| ChangieSourceViewError::ConfigPathUnsafe {
            path: selection_record.selected_path.clone(),
            reason: err,
        })?;
    let config_source = ChangieSourceDocument::from_bytes(
        config_repo_path,
        config_bytes,
        Some(subject_token(view_kind)),
    )
    .map_err(|err| ChangieSourceViewError::ConfigUnreadable {
        path: selection_record.selected_path.clone(),
        reason: err,
    })?;
    let sensor = ChangieSensor;
    let config = sensor.parse_config(config_source);

    // --- population from the same view -------------------------------------
    let PopulationAssembly {
        root,
        population,
        entries,
        omitted,
    } = population_from_view(
        view,
        &config,
        view_kind,
        &mut completeness,
        &mut limitations,
    )?;

    // --- pure sensor --------------------------------------------------------
    let report = sensor.lint(ChangieLintCandidate {
        config,
        entries: entries.clone(),
    });

    // Deterministic analysis identity: sorted population identities
    // over the view and config identities — equal subjects, equal ids.
    let mut identity_input = String::new();
    identity_input.push_str(view_kind.as_str());
    identity_input.push('\n');
    identity_input.push_str(view_identity.as_deref().unwrap_or("worktree"));
    identity_input.push('\n');
    identity_input.push_str(&selection_record.selected_path);
    identity_input.push('\n');
    identity_input.push_str(&config_content_identity.to_string());
    identity_input.push('\n');
    let mut population_identities: Vec<String> = population
        .iter()
        .map(|entry| {
            format!(
                "{}:{:?}:{}",
                entry.repo_path,
                entry.state,
                entry
                    .content_identity
                    .map(|identity| identity.to_string())
                    .unwrap_or_default()
            )
        })
        .collect();
    population_identities.sort();
    for identity in population_identities {
        identity_input.push_str(&identity);
        identity_input.push('\n');
    }
    for (path, reason) in &omitted {
        identity_input.push_str(&format!("omitted:{path}:{reason}"));
        identity_input.push('\n');
    }
    let analysis_identity = format!(
        "changie.analysis.v1:{}",
        allow_files::changie::ChangieContentIdentity::of(identity_input.as_bytes())
    );

    Ok(ChangieAnalysisResultV1 {
        generation: sensor.generation(),
        view_kind,
        view_identity,
        config_selection: selection_record,
        config_content_identity: Some(config_content_identity),
        population: ChangieSourcePopulationV1 {
            root,
            inspected: population,
            omitted,
        },
        report,
        completeness,
        limitations,
        analysis_identity,
    })
}

fn view_kind_of(view: &RepositorySourceView) -> ChangieSourceViewKind {
    match view {
        RepositorySourceView::Filesystem { .. } => ChangieSourceViewKind::SavedWorktree,
        RepositorySourceView::StagedIndex { .. } => ChangieSourceViewKind::StagedIndex,
        RepositorySourceView::CommittedTree { .. } => ChangieSourceViewKind::CommittedTree,
    }
}

fn subject_token(kind: ChangieSourceViewKind) -> String {
    format!("changie-adapter:{}", kind.as_str())
}

fn select_config(
    view: &RepositorySourceView,
    selection: &ChangieConfigSelectionV1,
) -> Result<ChangieConfigSelectionRecord, ChangieSourceViewError> {
    let inventory = view.inventory();
    let contains = |path: &str| {
        inventory
            .files
            .iter()
            .any(|file| file.to_string_lossy() == path)
    };
    match selection {
        ChangieConfigSelectionV1::Explicit(path) => {
            if !contains(path) {
                return Err(ChangieSourceViewError::ConfigNotFound {
                    mode: format!("explicit {path:?}"),
                    view: view_kind_of(view).as_str(),
                });
            }
            Ok(ChangieConfigSelectionRecord {
                mode: selection.clone(),
                selected_path: path.clone(),
                candidates_present: Vec::new(),
                ambiguous: false,
            })
        }
        ChangieConfigSelectionV1::DefaultNames => {
            let yaml = ".changie.yaml";
            let yml = ".changie.yml";
            let yaml_present = contains(yaml);
            let yml_present = contains(yml);
            // Precedence: .changie.yaml before .changie.yml, mirroring
            // the modeled generation. A malformed nearer config does
            // NOT fall through to the other name (falsifier 3): if the
            // nearer name exists, it is selected and its malformed
            // content flows into the sensor.
            let selected = if yaml_present {
                Some(yaml)
            } else if yml_present {
                Some(yml)
            } else {
                None
            };
            let Some(selected) = selected else {
                return Err(ChangieSourceViewError::ConfigNotFound {
                    mode: "default names".into(),
                    view: view_kind_of(view).as_str(),
                });
            };
            let mut candidates_present = Vec::new();
            if yaml_present {
                candidates_present.push(yaml.to_string());
            }
            if yml_present {
                candidates_present.push(yml.to_string());
            }
            Ok(ChangieConfigSelectionRecord {
                mode: selection.clone(),
                selected_path: selected.to_string(),
                candidates_present,
                ambiguous: yaml_present && yml_present,
            })
        }
    }
}

fn safe_repo_relative(raw: &str) -> Result<(), String> {
    if raw.is_empty() {
        return Err("empty path".into());
    }
    if raw.starts_with('/') || raw.starts_with('\\') {
        return Err("absolute path".into());
    }
    if raw.contains(':') {
        return Err("drive letter or scheme separator".into());
    }
    let mut depth: i64 = 0;
    for segment in raw.replace('\\', "/").split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                depth -= 1;
                if depth < 0 {
                    return Err("escapes the repository root".into());
                }
            }
            _ => depth += 1,
        }
    }
    Ok(())
}

fn normalize_repo_relative(raw: &str) -> String {
    let owned = raw.replace('\\', "/");
    let mut parts: Vec<&str> = Vec::new();
    for segment in owned.split('/') {
        match segment {
            "" | "." | ".." => {}
            other => parts.push(other),
        }
    }
    parts.join("/")
}

fn population_from_view(
    view: &RepositorySourceView,
    config: &allow_files::changie::ChangieConfigDocument,
    view_kind: ChangieSourceViewKind,
    completeness: &mut ChangieAcquisitionCompleteness,
    limitations: &mut Vec<String>,
) -> Result<PopulationAssembly, ChangieSourceViewError> {
    // Derive the root only from the parsed config's own fields.
    let string_field = |key: &str| -> Option<String> {
        config
            .root
            .as_ref()
            .and_then(|node| match &node.value {
                allow_files::changie::ChangieValue::Mapping(mapping) => {
                    mapping.first(key).map(|node| &node.value)
                }
                _ => None,
            })
            .and_then(|value| match value {
                allow_files::changie::ChangieValue::String(text) => Some(text.clone()),
                _ => None,
            })
    };
    // A malformed or non-mapping config carries no derivable path
    // fields: its malformed content flows into the sensor with an empty
    // population instead of failing before the sensor can report it.
    let config_root_is_mapping = matches!(
        config.root.as_ref().map(|node| &node.value),
        Some(allow_files::changie::ChangieValue::Mapping(_))
    );
    if !config_root_is_mapping {
        return Ok(PopulationAssembly {
            root: String::new(),
            population: Vec::new(),
            entries: Vec::new(),
            omitted: Vec::new(),
        });
    }
    let changes_dir = string_field("changesDir").unwrap_or_default();
    let unreleased_dir = string_field("unreleasedDir").unwrap_or_default();
    for (field, value) in [
        ("changesDir", &changes_dir),
        ("unreleasedDir", &unreleased_dir),
    ] {
        if let Err(reason) = safe_repo_relative(value) {
            return Err(ChangieSourceViewError::ConfigPathUnsafe {
                path: format!("{field}={value}"),
                reason,
            });
        }
    }
    let root = normalize_repo_relative(&format!("{changes_dir}/{unreleased_dir}"));
    let root_prefix = format!("{root}/");

    // Enumerate candidates from the view's inventory — the same view
    // that produced the config. Sorted for determinism regardless of
    // the view's internal traversal order (falsifier 9).
    let mut candidates: Vec<String> = view
        .inventory()
        .files
        .iter()
        .map(|path| normalize_repo_relative(&path.to_string_lossy()))
        .filter(|path| path.starts_with(&root_prefix))
        .collect();
    candidates.sort();
    candidates.dedup();

    // Deleted/renamed-away tracked entries stay visible where the view
    // exposes them, so a disappearing fragment cannot vanish silently
    // (falsifier 5).
    let mut deleted: Vec<String> = view
        .inventory()
        .deleted_tracked
        .iter()
        .map(|path| normalize_repo_relative(&path.to_string_lossy()))
        .filter(|path| path.starts_with(&root_prefix))
        .collect();
    if let RepositorySourceView::StagedIndex { snapshot, .. } = view {
        // Staged deletions and renames-away are change records, not
        // entries: enumerate them so a disappearing fragment stays
        // typed instead of vanishing.
        for change in &snapshot.changes {
            if !matches!(
                change.status,
                effortless_repo_snapshot::StagedPathStatus::Deleted
                    | effortless_repo_snapshot::StagedPathStatus::Renamed
            ) {
                continue;
            }
            if let Some(path) = &change.path {
                let normalized = normalize_repo_relative(&path.to_string_lossy());
                if normalized.starts_with(&root_prefix) {
                    deleted.push(normalized);
                }
            }
        }
    }
    deleted.sort();
    deleted.dedup();

    let staged_status_of = |path: &str| -> Option<(ChangieEntryState, bool)> {
        if let RepositorySourceView::StagedIndex { snapshot, .. } = view {
            let change = snapshot.changes.iter().find(|change| {
                change.path.as_ref().is_some_and(|candidate| {
                    normalize_repo_relative(&candidate.to_string_lossy()) == path
                })
            });
            let deleted = change.is_some_and(|change| {
                matches!(
                    change.status,
                    StagedPathStatus::Deleted | StagedPathStatus::Renamed
                )
            });
            let entry = snapshot.entries.iter().find(|entry| {
                entry.path.as_ref().is_some_and(|candidate| {
                    normalize_repo_relative(&candidate.to_string_lossy()) == path
                })
            });
            let kind_state = entry.map(|entry| match entry.kind {
                effortless_repo_snapshot::StagedEntryKind::RegularFile
                | effortless_repo_snapshot::StagedEntryKind::ExecutableFile => {
                    ChangieEntryState::File
                }
                effortless_repo_snapshot::StagedEntryKind::Symlink => ChangieEntryState::Symlink,
                effortless_repo_snapshot::StagedEntryKind::Gitlink => ChangieEntryState::Gitlink,
                effortless_repo_snapshot::StagedEntryKind::SparseDirectory => {
                    ChangieEntryState::Directory
                }
                effortless_repo_snapshot::StagedEntryKind::Unsupported => {
                    ChangieEntryState::UnsupportedMode
                }
            });
            return kind_state.map(|state| (state, deleted));
        }
        None
    };

    let mut population: Vec<ChangiePopulationEntry> = Vec::new();
    let mut entries: Vec<ChangieCandidateEntry> = Vec::new();
    let mut omitted: Vec<(String, String)> = Vec::new();

    for path in deleted {
        // Deleted/renamed entries keep their typed state even though no
        // document can be parsed from them.
        population.push(ChangiePopulationEntry {
            repo_path: path.clone(),
            state: ChangieEntryState::DeletedTracked,
            content_identity: None,
        });
        entries.push(ChangieCandidateEntry {
            repo_path: path,
            state: ChangieEntryState::DeletedTracked,
            fragment: None,
        });
    }

    for path in candidates {
        let (state, staged_deleted) =
            staged_status_of(&path).unwrap_or((ChangieEntryState::File, false));
        if staged_deleted {
            // A staged deletion: the view's inventory still lists the
            // tracked path, but the staged state says it is gone.
            population.push(ChangiePopulationEntry {
                repo_path: path.clone(),
                state: ChangieEntryState::DeletedTracked,
                content_identity: None,
            });
            entries.push(ChangieCandidateEntry {
                repo_path: path,
                state: ChangieEntryState::DeletedTracked,
                fragment: None,
            });
            continue;
        }
        let bytes = match view.read_bytes(std::path::Path::new(&path)) {
            Ok(bytes) => bytes,
            Err(err) => {
                *completeness = ChangieAcquisitionCompleteness::Partial;
                omitted.push((path.clone(), err.to_string()));
                population.push(ChangiePopulationEntry {
                    repo_path: path,
                    state,
                    content_identity: None,
                });
                continue;
            }
        };
        let content_identity = allow_files::changie::ChangieContentIdentity::of(&bytes);
        let repo_path = match ChangieRepoPath::from_repo_relative(&path) {
            Ok(repo_path) => repo_path,
            Err(err) => {
                *completeness = ChangieAcquisitionCompleteness::Partial;
                omitted.push((path.clone(), err));
                population.push(ChangiePopulationEntry {
                    repo_path: path,
                    state,
                    content_identity: Some(content_identity),
                });
                continue;
            }
        };
        let document =
            ChangieSourceDocument::from_bytes(repo_path, bytes, Some(subject_token(view_kind)))
                .unwrap_or_else(|err| std::panic::panic_any(format!("source document: {err}")));
        let fragment = sensor_parse_fragment(&document);
        population.push(ChangiePopulationEntry {
            repo_path: path.clone(),
            state: state.clone(),
            content_identity: Some(content_identity),
        });
        entries.push(ChangieCandidateEntry {
            repo_path: path,
            state,
            fragment: Some(fragment),
        });
    }

    match view.inventory().completeness {
        // Complete and Scoped are exact enumerations of their subject
        // (tracked files, staged entries, or a committed tree).
        effortless_repo_snapshot::SourceInventoryCompleteness::Complete
        | effortless_repo_snapshot::SourceInventoryCompleteness::Scoped => {}
        other => {
            *completeness = ChangieAcquisitionCompleteness::Partial;
            limitations.push(format!(
                "view inventory completeness is {}; population may be narrower than the truth",
                other.as_str()
            ));
        }
    }
    if view_kind == ChangieSourceViewKind::SavedWorktree {
        limitations.push(
            "saved-worktree view does not expose entry types; every present entry is reported as a regular file".into(),
        );
    }

    Ok(PopulationAssembly {
        root,
        population,
        entries,
        omitted,
    })
}

fn sensor_parse_fragment(
    document: &ChangieSourceDocument,
) -> allow_files::changie::ChangieFragmentDocument {
    ChangieSensor.parse_fragment(
        ChangieSourceDocument::from_bytes(
            ChangieRepoPath::from_repo_relative(document.repo_path())
                .unwrap_or_else(|err| std::panic::panic_any(err)),
            document.bytes().to_vec(),
            document.subject().map(str::to_string),
        )
        .unwrap_or_else(|err| std::panic::panic_any(err)),
    )
}

#[cfg(test)]
#[path = "changie_source_view_tests.rs"]
mod changie_source_view_tests;
