use allow_core::{
    CargoAllowError, CargoAllowErrorKind, CargoAllowResult, SOURCE_FILE_READ_MAX_BYTES,
    read_file_capped, source_tree_path_is_ignored,
};
use allow_diff::{
    StagedEntryKind, StagedPathRead, StagedRepositorySnapshot, StagedSnapshotCompleteness,
    read_staged_path, staged_repository_snapshot,
};
use allow_inventory::{
    Inventory, InventoryCompleteness, InventoryOptions, InventorySource, inventory,
};
use std::path::{Component, Path, PathBuf};

type RustSourceInputs = (Vec<(PathBuf, String)>, Vec<(PathBuf, String)>);

/// Load-bearing source bytes for the retained spec-system compilation path.
///
/// A staged view owns both the candidate inventory and candidate bytes. It
/// never falls back to the ordinary worktree for a path absent from the index.
#[derive(Debug)]
pub(crate) enum RepositorySourceView {
    Filesystem {
        root: PathBuf,
        inventory: Inventory,
    },
    StagedIndex {
        snapshot: StagedRepositorySnapshot,
        inventory: Inventory,
    },
}

impl RepositorySourceView {
    pub(crate) fn filesystem(root: impl AsRef<Path>) -> CargoAllowResult<Self> {
        let root = root.as_ref();
        Ok(Self::Filesystem {
            root: root.to_path_buf(),
            inventory: inventory(root, &InventoryOptions::default())?,
        })
    }

    pub fn staged(root: impl AsRef<Path>) -> CargoAllowResult<Self> {
        let snapshot = staged_repository_snapshot(root)?;
        let inventory = staged_inventory(&snapshot);
        Ok(Self::StagedIndex {
            snapshot,
            inventory,
        })
    }

    pub(crate) fn inventory(&self) -> &Inventory {
        match self {
            Self::Filesystem { inventory, .. } | Self::StagedIndex { inventory, .. } => inventory,
        }
    }

    pub(crate) fn source_identity(&self) -> Option<&str> {
        match self {
            Self::Filesystem { .. } => None,
            Self::StagedIndex { snapshot, .. } => Some(&snapshot.identity.semantic_hash),
        }
    }

    pub(crate) fn limitations(&self) -> &[String] {
        match self {
            Self::Filesystem { .. } => &[],
            Self::StagedIndex { snapshot, .. } => &snapshot.limitations,
        }
    }

    pub(crate) fn read_text(&self, path: &Path) -> CargoAllowResult<String> {
        let bytes = self.read_bytes(path)?;
        String::from_utf8(bytes).map_err(|source| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::Scan,
                format!("source file {} is not valid UTF-8", path.display()),
            )
            .with_cause(&source)
        })
    }

    pub(crate) fn rust_inputs(&self) -> CargoAllowResult<RustSourceInputs> {
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

    fn read_bytes(&self, path: &Path) -> CargoAllowResult<Vec<u8>> {
        validate_relative_path(path)?;
        match self {
            Self::Filesystem { root, .. } => read_file_capped(&root.join(path)).map_err(|source| {
                CargoAllowError::with_kind(
                    CargoAllowErrorKind::Scan,
                    format!("failed to read source file {}", path.display()),
                )
                .with_cause(&source)
            }),
            Self::StagedIndex { snapshot, .. } => {
                let read = read_staged_path(snapshot, path)?;
                match read {
                    StagedPathRead::Regular(bytes) => capped_staged_bytes(path, bytes),
                    StagedPathRead::Missing => Err(CargoAllowError::with_kind(
                        CargoAllowErrorKind::Inventory,
                        format!(
                            "staged source file {} is absent from the candidate",
                            path.display()
                        ),
                    )),
                    StagedPathRead::Unsupported { kind, .. } => Err(CargoAllowError::with_kind(
                        CargoAllowErrorKind::Inventory,
                        format!(
                            "staged source file {} has unsupported entry kind {kind:?}",
                            path.display()
                        ),
                    )),
                }
            }
        }
    }
}

fn staged_inventory(snapshot: &StagedRepositorySnapshot) -> Inventory {
    let options = InventoryOptions::default();
    let mut files = snapshot
        .entries
        .iter()
        .filter(|entry| entry.stage == 0)
        .filter_map(|entry| entry.path.clone())
        .filter(|path| !source_tree_path_is_ignored(path, &options.ignored))
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
        InventoryCompleteness::Partial
    } else if !options.ignored.is_empty() || !options.generated.is_empty() {
        InventoryCompleteness::Scoped
    } else {
        InventoryCompleteness::Complete
    };
    Inventory {
        files,
        source: InventorySource::GitIndexStagedCandidate,
        completeness,
        empty_git_tracked: snapshot.entries.is_empty(),
        deleted_tracked: Vec::new(),
        git_error: None,
        skipped_paths: Vec::new(),
        submodule_paths: Vec::new(),
    }
}

fn capped_staged_bytes(path: &Path, bytes: Vec<u8>) -> CargoAllowResult<Vec<u8>> {
    if (bytes.len() as u64) > SOURCE_FILE_READ_MAX_BYTES {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Scan,
            format!(
                "staged source file {} exceeds the {}-byte source-read limit",
                path.display(),
                SOURCE_FILE_READ_MAX_BYTES
            ),
        ));
    }
    Ok(bytes)
}

fn validate_relative_path(path: &Path) -> CargoAllowResult<()> {
    if path.as_os_str().is_empty()
        || path.components().any(|component| {
            matches!(
                component,
                Component::Prefix(_) | Component::RootDir | Component::ParentDir
            )
        })
    {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!(
                "source view path must be repository-relative: {}",
                path.display()
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_repo(label: &str) -> Result<PathBuf, String> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "cargo-allow-source-view-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        git(&root, &["init", "-q"])?;
        git(&root, &["config", "user.name", "Cargo Allow"])?;
        git(
            &root,
            &["config", "user.email", "cargo-allow@example.invalid"],
        )?;
        Ok(root)
    }

    fn git(root: &Path, args: &[&str]) -> Result<String, String> {
        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .map_err(|error| error.to_string())?;
        if output.status.success() {
            String::from_utf8(output.stdout).map_err(|error| error.to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).into_owned())
        }
    }

    #[test]
    fn staged_view_reads_indexed_bytes_and_inventory() -> Result<(), String> {
        let root = test_repo("indexed")?;
        fs::write(root.join("value.txt"), "staged\n").map_err(|error| error.to_string())?;
        git(&root, &["add", "value.txt"])?;
        fs::write(root.join("value.txt"), "worktree\n").map_err(|error| error.to_string())?;

        let view = RepositorySourceView::staged(&root).map_err(|error| error.to_string())?;
        assert_eq!(
            view.inventory().source,
            InventorySource::GitIndexStagedCandidate
        );
        assert_eq!(
            view.read_text(Path::new("value.txt")),
            Ok("staged\n".to_string())
        );
        assert!(view.source_identity().is_some());
        fs::remove_dir_all(root).map_err(|error| error.to_string())
    }

    #[test]
    fn staged_view_does_not_fall_back_for_deleted_candidate_path() -> Result<(), String> {
        let root = test_repo("deleted")?;
        fs::write(root.join("value.txt"), "base\n").map_err(|error| error.to_string())?;
        git(&root, &["add", "value.txt"])?;
        git(&root, &["commit", "-qm", "base"])?;
        git(&root, &["rm", "-q", "value.txt"])?;
        fs::write(root.join("value.txt"), "dirty worktree\n").map_err(|error| error.to_string())?;

        let view = RepositorySourceView::staged(&root).map_err(|error| error.to_string())?;
        let error = view
            .read_text(Path::new("value.txt"))
            .err()
            .ok_or_else(|| "deleted staged path unexpectedly read from worktree".to_string())?;
        assert!(error.to_string().contains("absent from the candidate"));
        fs::remove_dir_all(root).map_err(|error| error.to_string())
    }
}
