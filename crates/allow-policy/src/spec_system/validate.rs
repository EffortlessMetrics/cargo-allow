use allow_core::{CargoAllowError, CargoAllowResult};
use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use super::{ArtifactKind, ArtifactStatus, DocArtifact, DocArtifactLedger, SpecSystemRoots};

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

        let text = fs::read_to_string(&source_path).map_err(|e| {
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
        ArtifactKind::ActiveGoal => path_has_prefix(&source_path, &roots.goals),
        ArtifactKind::SupportTier => source_path == normalize_source_path(&roots.support_tiers),
        ArtifactKind::PolicyLedger => {
            let policy_root = match source_parent(&roots.artifact_ledger) {
                Some(root) => root,
                None => "policy".to_string(),
            };
            path_has_prefix(&source_path, &policy_root)
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

fn source_parent(path: &str) -> Option<String> {
    let path = normalize_source_path(path);
    path.rsplit_once('/').map(|(parent, _)| parent.to_string())
}

fn normalize_source_path(path: &str) -> String {
    path.trim_matches('/').replace('\\', "/")
}

fn contains_artifact_id(text: &str, id: &str) -> bool {
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
