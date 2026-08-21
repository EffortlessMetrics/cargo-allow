use crate::error::{SnapshotError, SnapshotErrorKind, SnapshotResult};
use crate::git::{git_tracked_files_at_revision, read_file_at_revision};
use crate::inventory::{
    SourceInventory, SourceInventoryCompleteness, SourceInventorySource, default_source_inventory,
    source_inventory_path_is_ignored,
};
use crate::revision_identity::{ResolvedRevisionIdentity, resolve_revision_identity};
use crate::staged_index::{
    StagedEntryKind, StagedPathRead, StagedRepositorySnapshot, StagedSnapshotCompleteness,
    read_staged_path, staged_repository_snapshot,
};
use crate::util::{SOURCE_FILE_READ_MAX_BYTES, read_file_capped};
use std::path::{Component, Path, PathBuf};

type RustSourceInputs = (Vec<(PathBuf, String)>, Vec<(PathBuf, String)>);

/// Load-bearing source bytes for repository snapshot consumers.
///
/// A staged view owns both the candidate inventory and candidate bytes. It never
/// falls back to the ordinary worktree for a path absent from the index.
#[derive(Debug)]
pub enum RepositorySourceView {
    Filesystem {
        root: PathBuf,
        inventory: SourceInventory,
    },
    StagedIndex {
        snapshot: StagedRepositorySnapshot,
        inventory: SourceInventory,
    },
    CommittedTree {
        root: PathBuf,
        revision: String,
        identity: ResolvedRevisionIdentity,
        inventory: SourceInventory,
    },
}

impl RepositorySourceView {
    pub fn filesystem(root: impl AsRef<Path>) -> SnapshotResult<Self> {
        let root = root.as_ref();
        Ok(Self::Filesystem {
            root: root.to_path_buf(),
            inventory: default_source_inventory(root)?,
        })
    }

    pub fn staged(root: impl AsRef<Path>) -> SnapshotResult<Self> {
        let snapshot = staged_repository_snapshot(root)?;
        let inventory = staged_inventory(&snapshot);
        Ok(Self::StagedIndex {
            snapshot,
            inventory,
        })
    }

    pub fn committed(root: impl AsRef<Path>, revision: &str) -> SnapshotResult<Self> {
        let root = root.as_ref();
        let identity = resolve_revision_identity(root, revision)?;
        let files = git_tracked_files_at_revision(root, &identity.commit)?;
        let mut files = files
            .into_iter()
            .filter(|path| !source_inventory_path_is_ignored(path))
            .collect::<Vec<_>>();
        files.sort();
        files.dedup();
        let completeness = SourceInventoryCompleteness::Scoped;
        Ok(Self::CommittedTree {
            root: root.to_path_buf(),
            revision: identity.commit.clone(),
            identity,
            inventory: SourceInventory {
                empty_git_tracked: files.is_empty(),
                files,
                source: SourceInventorySource::GitTracked,
                completeness,
                deleted_tracked: Vec::new(),
                git_error: None,
                skipped_paths: Vec::new(),
                submodule_paths: Vec::new(),
            },
        })
    }

    pub fn inventory(&self) -> &SourceInventory {
        match self {
            Self::Filesystem { inventory, .. }
            | Self::StagedIndex { inventory, .. }
            | Self::CommittedTree { inventory, .. } => inventory,
        }
    }

    pub fn source_identity(&self) -> Option<&str> {
        match self {
            Self::Filesystem { .. } => None,
            Self::StagedIndex { snapshot, .. } => Some(&snapshot.identity.semantic_hash),
            Self::CommittedTree { identity, .. } => Some(&identity.commit),
        }
    }

    pub fn revision_identity(&self) -> Option<&ResolvedRevisionIdentity> {
        match self {
            Self::CommittedTree { identity, .. } => Some(identity),
            Self::Filesystem { .. } | Self::StagedIndex { .. } => None,
        }
    }

    pub fn limitations(&self) -> &[String] {
        match self {
            Self::Filesystem { .. } => &[],
            Self::StagedIndex { snapshot, .. } => &snapshot.limitations,
            Self::CommittedTree { .. } => &[],
        }
    }

    pub fn read_text(&self, path: &Path) -> SnapshotResult<String> {
        let bytes = self.read_bytes(path)?;
        String::from_utf8(bytes).map_err(|source| {
            SnapshotError::with_kind(
                SnapshotErrorKind::Scan,
                format!("source file {} is not valid UTF-8", path.display()),
            )
            .with_cause(&source)
        })
    }

    pub fn rust_inputs(&self) -> SnapshotResult<RustSourceInputs> {
        let mut manifests = Vec::new();
        let mut sources = Vec::new();
        for path in &self.inventory().files {
            if path.file_name().and_then(|name| name.to_str()) == Some("Cargo.toml") {
                manifests.push((path.clone(), self.read_text(path)?));
            } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
                sources.push((path.clone(), self.read_text(path)?));
            }
        }
        Ok((manifests, sources))
    }

    pub fn read_bytes(&self, path: &Path) -> SnapshotResult<Vec<u8>> {
        validate_relative_path(path)?;
        match self {
            Self::Filesystem { root, .. } => read_file_capped(&root.join(path)).map_err(|source| {
                SnapshotError::with_kind(
                    SnapshotErrorKind::Scan,
                    format!("failed to read source file {}", path.display()),
                )
                .with_cause(&source)
            }),
            Self::StagedIndex { snapshot, .. } => {
                let read = read_staged_path(snapshot, path)?;
                match read {
                    StagedPathRead::Regular(bytes) => capped_staged_bytes(path, bytes),
                    StagedPathRead::Missing => Err(SnapshotError::with_kind(
                        SnapshotErrorKind::Inventory,
                        format!(
                            "staged source file {} is absent from the candidate",
                            path.display()
                        ),
                    )),
                    StagedPathRead::Unsupported { kind, .. } => Err(SnapshotError::with_kind(
                        SnapshotErrorKind::Inventory,
                        format!(
                            "staged source file {} has unsupported entry kind {kind:?}",
                            path.display()
                        ),
                    )),
                }
            }
            Self::CommittedTree { root, revision, .. } => {
                match read_file_at_revision(root, revision, path)? {
                    Some(text) => capped_committed_text(path, text),
                    None => Err(SnapshotError::with_kind(
                        SnapshotErrorKind::Inventory,
                        format!(
                            "committed source file {} is absent or unsupported in the parent tree",
                            path.display()
                        ),
                    )),
                }
            }
        }
    }
}

fn staged_inventory(snapshot: &StagedRepositorySnapshot) -> SourceInventory {
    let mut files = snapshot
        .entries
        .iter()
        .filter(|entry| entry.stage == 0)
        .filter_map(|entry| entry.path.clone())
        .filter(|path| !source_inventory_path_is_ignored(path))
        .filter(|path| {
            snapshot
                .entries
                .iter()
                .find(|entry| entry.stage == 0 && entry.path.as_ref() == Some(path))
                .is_some_and(|entry| {
                    matches!(
                        entry.kind,
                        StagedEntryKind::RegularFile | StagedEntryKind::ExecutableFile
                    )
                })
        })
        .collect::<Vec<_>>();
    files.sort();
    files.dedup();
    let completeness = if snapshot.completeness == StagedSnapshotCompleteness::Partial {
        SourceInventoryCompleteness::Partial
    } else {
        SourceInventoryCompleteness::Scoped
    };
    SourceInventory {
        files,
        source: SourceInventorySource::GitIndexStagedCandidate,
        completeness,
        empty_git_tracked: snapshot.entries.is_empty(),
        deleted_tracked: Vec::new(),
        git_error: None,
        skipped_paths: Vec::new(),
        submodule_paths: Vec::new(),
    }
}

fn capped_staged_bytes(path: &Path, bytes: Vec<u8>) -> SnapshotResult<Vec<u8>> {
    if (bytes.len() as u64) > SOURCE_FILE_READ_MAX_BYTES {
        return Err(SnapshotError::with_kind(
            SnapshotErrorKind::Scan,
            format!(
                "staged source file {} exceeds the {}-byte source-read limit",
                path.display(),
                SOURCE_FILE_READ_MAX_BYTES
            ),
        ));
    }
    Ok(bytes)
}

fn capped_committed_text(path: &Path, text: String) -> SnapshotResult<Vec<u8>> {
    if (text.len() as u64) > SOURCE_FILE_READ_MAX_BYTES {
        return Err(SnapshotError::with_kind(
            SnapshotErrorKind::Scan,
            format!(
                "committed source file {} exceeds the {}-byte source-read limit",
                path.display(),
                SOURCE_FILE_READ_MAX_BYTES
            ),
        ));
    }
    Ok(text.into_bytes())
}

fn validate_relative_path(path: &Path) -> SnapshotResult<()> {
    if path.as_os_str().is_empty()
        || path.components().any(|component| {
            matches!(
                component,
                Component::Prefix(_) | Component::RootDir | Component::ParentDir
            )
        })
    {
        return Err(SnapshotError::with_kind(
            SnapshotErrorKind::InvalidConfig,
            format!(
                "source view path must be repository-relative: {}",
                path.display()
            ),
        ));
    }
    Ok(())
}
