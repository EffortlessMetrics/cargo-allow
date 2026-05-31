use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn normalize_local_evidence_path(path: &Path) -> PathBuf {
    PathBuf::from(path.to_string_lossy().replace('\\', "/"))
}

pub(crate) fn first_symlink_component(root: &Path, relative: &Path) -> Option<PathBuf> {
    let mut current = root.to_path_buf();
    let mut source_tree_component = PathBuf::new();
    for component in relative.components() {
        current.push(component.as_os_str());
        source_tree_component.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Some(source_tree_component);
            }
            Ok(_) => {}
            Err(_) => return None,
        }
    }
    None
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
            first_symlink_component(&root, Path::new("docs/safety.md")),
            None
        );
    }
}
