use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub(crate) fn normalize_local_evidence_path(path: &Path) -> PathBuf {
    PathBuf::from(path.to_string_lossy().replace('\\', "/"))
}

#[derive(Debug)]
pub(crate) struct EvidencePathInspectionError {
    component: PathBuf,
    source: io::Error,
}

impl EvidencePathInspectionError {
    pub(crate) fn component(&self) -> &Path {
        &self.component
    }

    pub(crate) fn kind(&self) -> io::ErrorKind {
        self.source.kind()
    }
}

impl fmt::Display for EvidencePathInspectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "unable to inspect source-tree component {}: {}",
            self.component.display(),
            self.source
        )
    }
}

impl std::error::Error for EvidencePathInspectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EvidencePathComponentKind {
    Symlink,
    Other,
}

pub(crate) fn first_symlink_component(
    root: &Path,
    relative: &Path,
) -> Result<Option<PathBuf>, EvidencePathInspectionError> {
    first_symlink_component_with(root, relative, |path| {
        fs::symlink_metadata(path).map(|metadata| {
            if metadata.file_type().is_symlink() {
                EvidencePathComponentKind::Symlink
            } else {
                EvidencePathComponentKind::Other
            }
        })
    })
}

fn first_symlink_component_with(
    root: &Path,
    relative: &Path,
    mut inspect: impl FnMut(&Path) -> io::Result<EvidencePathComponentKind>,
) -> Result<Option<PathBuf>, EvidencePathInspectionError> {
    let mut current = root.to_path_buf();
    let mut source_tree_component = PathBuf::new();
    for component in relative.components() {
        current.push(component.as_os_str());
        source_tree_component.push(component.as_os_str());
        match inspect(&current) {
            Ok(EvidencePathComponentKind::Symlink) => {
                return Ok(Some(source_tree_component));
            }
            Ok(EvidencePathComponentKind::Other) => {}
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(source) => {
                return Err(EvidencePathInspectionError {
                    component: source_tree_component,
                    source,
                });
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_backslashes_for_source_tree_paths() {
        assert_eq!(
            normalize_local_evidence_path(Path::new("docs\\safety\\ffi.md")),
            PathBuf::from("docs/safety/ffi.md")
        );
    }

    #[test]
    fn returns_none_when_component_is_missing() {
        let root = std::env::temp_dir().join(format!(
            "cargo-allow-missing-evidence-component-{}",
            std::process::id()
        ));
        assert_eq!(
            first_symlink_component(&root, Path::new("docs/safety.md"))
                .unwrap_or_else(|err| panic!("missing path should not be an inspection error: {err}")),
            None
        );
    }

    #[test]
    fn returns_first_symlink_component() {
        let root = Path::new("repo-root");
        let result = first_symlink_component_with(
            root,
            Path::new("docs/link/safety.md"),
            |path| {
                if path.ends_with(Path::new("docs/link")) {
                    Ok(EvidencePathComponentKind::Symlink)
                } else {
                    Ok(EvidencePathComponentKind::Other)
                }
            },
        )
        .unwrap_or_else(|err| panic!("simulated symlink inspection should succeed: {err}"));

        assert_eq!(result, Some(PathBuf::from("docs/link")));
    }

    #[test]
    fn preserves_non_not_found_inspection_failure() {
        let root = Path::new("repo-root");
        let err = first_symlink_component_with(
            root,
            Path::new("docs/private/safety.md"),
            |path| {
                if path.ends_with(Path::new("docs/private")) {
                    Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "fixture permission denied",
                    ))
                } else {
                    Ok(EvidencePathComponentKind::Other)
                }
            },
        )
        .err()
        .unwrap_or_else(|| panic!("permission failure should be retained"));

        assert_eq!(err.component(), Path::new("docs/private"));
        assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
        assert!(err.to_string().contains("fixture permission denied"));
    }
}
