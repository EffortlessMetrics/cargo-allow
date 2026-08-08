use allow_core::{CargoAllowError, CargoAllowResult, read_text_file_capped};
use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use super::{ArtifactKind, ArtifactStatus, DocArtifact, DocArtifactLedger, SpecSystemRoots};

pub fn validate_doc_artifact_links(ledger: &DocArtifactLedger) -> CargoAllowResult<()> {
    let index = ArtifactIndex::new(ledger)?;

    for artifact in &ledger.artifact {
        validate_required_edges(artifact)?;
        validate_artifact_links(artifact, &index)?;
        validate_lifecycle_links(artifact, &index)?;
    }

    Ok(())
}

pub fn validate_doc_artifact_files(
    repo_root: impl AsRef<Path>,
    ledger: &DocArtifactLedger,
    roots: &SpecSystemRoots,
) -> CargoAllowResult<()> {
    let canonical_root = fs::canonicalize(repo_root.as_ref()).map_err(|e| {
        CargoAllowError::new(format!(
            "failed to canonicalize source tree root {}: {e}",
            repo_root.as_ref().display()
        ))
    })?;
    let statuses = ledger
        .artifact
        .iter()
        .map(|artifact| (artifact.id.as_str(), artifact.status))
        .collect::<HashMap<_, _>>();

    for artifact in &ledger.artifact {
        let source_path = source_tree_path(&canonical_root, &artifact.path)?;
        validate_artifact_source_path(artifact, roots)?;
        validate_superseded_replacement(artifact, &statuses)?;

        if !source_path.is_file() {
            return Err(CargoAllowError::new(format!(
                "{} artifact file missing: {}",
                artifact.id, artifact.path
            )));
        }

        let canonical_source_path = fs::canonicalize(&source_path).map_err(|e| {
            CargoAllowError::new(format!(
                "failed to canonicalize artifact {}: {e}",
                artifact.path
            ))
        })?;
        if !canonical_source_path.starts_with(&canonical_root) {
            return Err(CargoAllowError::new(format!(
                "{} artifact path escapes source tree: {}",
                artifact.id, artifact.path
            )));
        }

        let text = read_text_file_capped(&source_path).map_err(|e| {
            CargoAllowError::new(format!("failed to read artifact {}: {e}", artifact.path))
        })?;
        if !contains_artifact_id(&text, &artifact.id) {
            return Err(CargoAllowError::new(format!(
                "{} not found in artifact file {}",
                artifact.id, artifact.path
            )));
        }
    }

    Ok(())
}

struct ArtifactIndex<'a> {
    by_id: HashMap<&'a str, &'a DocArtifact>,
    by_path: HashMap<String, &'a DocArtifact>,
}

impl<'a> ArtifactIndex<'a> {
    fn new(ledger: &'a DocArtifactLedger) -> CargoAllowResult<Self> {
        let by_id = ledger
            .artifact
            .iter()
            .map(|artifact| (artifact.id.as_str(), artifact))
            .collect();
        let mut by_path = HashMap::new();
        for artifact in &ledger.artifact {
            let path = normalize_source_path(&artifact.path);
            if by_path.insert(path.clone(), artifact).is_some() {
                return Err(CargoAllowError::new(format!(
                    "duplicate doc artifact path {path}"
                )));
            }
        }
        Ok(Self { by_id, by_path })
    }

    fn resolve_id(&self, id: &str) -> Option<&'a DocArtifact> {
        self.by_id.get(id).copied()
    }

    fn resolve_id_or_path(&self, value: &str) -> Option<&'a DocArtifact> {
        self.resolve_id(value)
            .or_else(|| self.by_path.get(&normalize_source_path(value)).copied())
    }
}

fn validate_required_edges(artifact: &DocArtifact) -> CargoAllowResult<()> {
    match artifact.kind {
        ArtifactKind::Spec if artifact.status == ArtifactStatus::Accepted => {
            require_link_or_reason(
                artifact,
                "linked_proposal",
                artifact.linked_proposal.as_deref(),
            )
        }
        ArtifactKind::Adr if artifact.status == ArtifactStatus::Accepted => {
            require_link_or_reason(artifact, "linked_spec", artifact.linked_spec.as_deref())
        }
        ArtifactKind::ImplementationPlan | ArtifactKind::PlanItem
            if artifact.status == ArtifactStatus::Active =>
        {
            if has_value(artifact.linked_proposal.as_deref())
                || has_value(artifact.linked_spec.as_deref())
            {
                Ok(())
            } else {
                Err(CargoAllowError::new(format!(
                    "{} active plan requires linked_proposal or linked_spec",
                    artifact.id
                )))
            }
        }
        ArtifactKind::ActiveGoal if artifact.status == ArtifactStatus::Active => {
            require_link(
                artifact,
                "linked_proposal",
                artifact.linked_proposal.as_deref(),
            )?;
            require_link(artifact, "linked_spec", artifact.linked_spec.as_deref())?;
            require_link(artifact, "linked_plan", artifact.linked_plan.as_deref())
        }
        ArtifactKind::Closeout => {
            require_link(artifact, "linked_plan", artifact.linked_plan.as_deref())
        }
        _ => Ok(()),
    }
}

fn validate_artifact_links(
    artifact: &DocArtifact,
    index: &ArtifactIndex<'_>,
) -> CargoAllowResult<()> {
    validate_id_link(
        artifact,
        "linked_proposal",
        artifact.linked_proposal.as_deref(),
        &[ArtifactKind::Proposal],
        index,
    )?;
    validate_id_link(
        artifact,
        "linked_spec",
        artifact.linked_spec.as_deref(),
        &[ArtifactKind::Spec],
        index,
    )?;
    validate_id_link(
        artifact,
        "linked_adr",
        artifact.linked_adr.as_deref(),
        &[ArtifactKind::Adr],
        index,
    )?;
    validate_plan_link(artifact, artifact.linked_plan.as_deref(), index)?;
    validate_id_link(
        artifact,
        "linked_goal",
        artifact.linked_goal.as_deref(),
        &[ArtifactKind::ActiveGoal],
        index,
    )?;
    validate_id_link(
        artifact,
        "linked_support_tier",
        artifact.linked_support_tier.as_deref(),
        &[ArtifactKind::SupportTier],
        index,
    )?;
    validate_id_link(
        artifact,
        "linked_closeout",
        artifact.linked_closeout.as_deref(),
        &[ArtifactKind::Closeout],
        index,
    )
}

fn validate_lifecycle_links(
    artifact: &DocArtifact,
    index: &ArtifactIndex<'_>,
) -> CargoAllowResult<()> {
    validate_same_kind_link(
        artifact,
        "supersedes",
        artifact.supersedes.as_deref(),
        index,
    )?;
    validate_same_kind_link(
        artifact,
        "superseded_by",
        artifact.superseded_by.as_deref(),
        index,
    )?;
    validate_same_kind_link(artifact, "replaces", artifact.replaces.as_deref(), index)
}

fn require_link_or_reason(
    artifact: &DocArtifact,
    field: &str,
    value: Option<&str>,
) -> CargoAllowResult<()> {
    if has_value(value) || has_value(artifact.standalone_reason.as_deref()) {
        return Ok(());
    }

    Err(CargoAllowError::new(format!(
        "{} accepted {:?} requires {field} or standalone_reason",
        artifact.id, artifact.kind
    )))
}

fn require_link(artifact: &DocArtifact, field: &str, value: Option<&str>) -> CargoAllowResult<()> {
    if has_value(value) {
        return Ok(());
    }

    Err(CargoAllowError::new(format!(
        "{} {:?} requires {field}",
        artifact.id, artifact.kind
    )))
}

fn validate_id_link(
    artifact: &DocArtifact,
    field: &str,
    value: Option<&str>,
    expected_kinds: &[ArtifactKind],
    index: &ArtifactIndex<'_>,
) -> CargoAllowResult<()> {
    let Some(value) = optional_link_value(artifact, field, value)? else {
        return Ok(());
    };
    let Some(target) = index.resolve_id(value) else {
        return Err(unknown_target_error(artifact, field, value));
    };
    validate_target_kind(artifact, field, value, target.kind, expected_kinds)
}

fn validate_plan_link(
    artifact: &DocArtifact,
    value: Option<&str>,
    index: &ArtifactIndex<'_>,
) -> CargoAllowResult<()> {
    let Some(value) = optional_link_value(artifact, "linked_plan", value)? else {
        return Ok(());
    };
    let Some(target) = index.resolve_id_or_path(value) else {
        return Err(CargoAllowError::new(format!(
            "{} linked_plan target {} is not registered by id or path",
            artifact.id, value
        )));
    };
    validate_target_kind(
        artifact,
        "linked_plan",
        value,
        target.kind,
        &[ArtifactKind::ImplementationPlan, ArtifactKind::PlanItem],
    )
}

fn validate_same_kind_link(
    artifact: &DocArtifact,
    field: &str,
    value: Option<&str>,
    index: &ArtifactIndex<'_>,
) -> CargoAllowResult<()> {
    let Some(value) = optional_link_value(artifact, field, value)? else {
        return Ok(());
    };
    if value == artifact.id {
        return Err(CargoAllowError::new(format!(
            "{} {field} must not reference itself",
            artifact.id
        )));
    }
    let Some(target) = index.resolve_id(value) else {
        return Err(unknown_target_error(artifact, field, value));
    };
    validate_target_kind(artifact, field, value, target.kind, &[artifact.kind])
}

fn optional_link_value<'a>(
    artifact: &DocArtifact,
    field: &str,
    value: Option<&'a str>,
) -> CargoAllowResult<Option<&'a str>> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.trim().is_empty() {
        return Err(CargoAllowError::new(format!(
            "{} {field} must not be empty",
            artifact.id
        )));
    }
    if value.trim() != value {
        return Err(CargoAllowError::new(format!(
            "{} {field} must not have leading or trailing whitespace",
            artifact.id
        )));
    }
    Ok(Some(value))
}

fn validate_target_kind(
    artifact: &DocArtifact,
    field: &str,
    value: &str,
    actual_kind: ArtifactKind,
    expected_kinds: &[ArtifactKind],
) -> CargoAllowResult<()> {
    if expected_kinds.contains(&actual_kind) {
        return Ok(());
    }

    Err(CargoAllowError::new(format!(
        "{} {field} target {} has kind {:?}, expected {}",
        artifact.id,
        value,
        actual_kind,
        expected_kind_list(expected_kinds)
    )))
}

fn unknown_target_error(artifact: &DocArtifact, field: &str, value: &str) -> CargoAllowError {
    CargoAllowError::new(format!(
        "{} {field} target {} is not registered",
        artifact.id, value
    ))
}

fn expected_kind_list(kinds: &[ArtifactKind]) -> String {
    kinds
        .iter()
        .map(|kind| format!("{kind:?}"))
        .collect::<Vec<_>>()
        .join(" or ")
}

fn has_value(value: Option<&str>) -> bool {
    value.is_some_and(|value| !value.trim().is_empty())
}

fn validate_artifact_source_path(
    artifact: &DocArtifact,
    roots: &SpecSystemRoots,
) -> CargoAllowResult<()> {
    let source_path = normalize_source_path(&artifact.path);
    let valid = match artifact.kind {
        ArtifactKind::Proposal => path_has_prefix(&source_path, &roots.proposals),
        ArtifactKind::Spec => path_has_prefix(&source_path, &roots.specs),
        ArtifactKind::Adr => path_has_prefix(&source_path, &roots.adrs),
        ArtifactKind::ImplementationPlan | ArtifactKind::PlanItem | ArtifactKind::Closeout => {
            path_has_prefix(&source_path, &roots.plans)
        }
        ArtifactKind::ActiveGoal => roots
            .goals
            .as_deref()
            .is_some_and(|goals| path_has_prefix(&source_path, goals)),
        ArtifactKind::SupportTier => source_path == normalize_source_path(&roots.support_tiers),
        ArtifactKind::PolicyLedger => {
            path_has_prefix(&source_path, "policy")
                || path_has_prefix(&source_path, ".allow/profiles")
        }
        ArtifactKind::ReleaseRecord => path_has_prefix(&source_path, "docs/release"),
    };

    if !valid {
        return Err(CargoAllowError::new(format!(
            "{} kind {:?} does not match artifact path {}",
            artifact.id, artifact.kind, artifact.path
        )));
    }

    Ok(())
}

fn validate_superseded_replacement(
    artifact: &DocArtifact,
    statuses: &HashMap<&str, ArtifactStatus>,
) -> CargoAllowResult<()> {
    if artifact.status != ArtifactStatus::Superseded {
        return Ok(());
    }

    let Some(replacement) = artifact.superseded_by.as_deref() else {
        return Err(CargoAllowError::new(format!(
            "{} superseded artifact requires superseded_by",
            artifact.id
        )));
    };

    if replacement == artifact.id {
        return Err(CargoAllowError::new(format!(
            "{} superseded artifact must not supersede itself",
            artifact.id
        )));
    }

    let Some(replacement_status) = statuses.get(replacement) else {
        return Err(CargoAllowError::new(format!(
            "{} superseded_by target {} is not registered",
            artifact.id, replacement
        )));
    };

    if *replacement_status == ArtifactStatus::Superseded {
        return Err(CargoAllowError::new(format!(
            "{} superseded_by target {} is also superseded",
            artifact.id, replacement
        )));
    }

    Ok(())
}

fn source_tree_path(repo_root: impl AsRef<Path>, source_path: &str) -> CargoAllowResult<PathBuf> {
    let path = Path::new(source_path);
    if path.is_absolute() {
        return Err(CargoAllowError::new(format!(
            "artifact path {source_path} must be relative"
        )));
    }
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(CargoAllowError::new(format!(
            "artifact path {source_path} must stay under the source tree"
        )));
    }
    Ok(repo_root.as_ref().join(path))
}

fn path_has_prefix(path: &str, prefix: &str) -> bool {
    let prefix = normalize_source_path(prefix);
    path == prefix || path.starts_with(&format!("{prefix}/"))
}

fn normalize_source_path(path: &str) -> String {
    path.trim_matches('/').replace('\\', "/")
}

pub fn contains_artifact_id(text: &str, id: &str) -> bool {
    text.match_indices(id)
        .any(|(index, _)| has_id_boundaries(text.as_bytes(), index, index + id.len()))
}

fn has_id_boundaries(bytes: &[u8], start: usize, end: usize) -> bool {
    let before = start == 0
        || bytes
            .get(start.saturating_sub(1))
            .is_none_or(|byte| !is_artifact_id_byte(*byte));
    let after = bytes
        .get(end)
        .is_none_or(|byte| !is_artifact_id_byte(*byte));
    before && after
}

fn is_artifact_id_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'
}
