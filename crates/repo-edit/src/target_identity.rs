use std::path::{Component, Path, PathBuf};

use crate::containment::strip_verbatim_prefix;

/// Lexically normalize a path by stripping the Windows verbatim prefix and
/// folding `.`/`..` components. This is not a filesystem canonicalize — it does
/// not resolve symlinks.
pub fn canonicalize_lexically(path: &Path) -> PathBuf {
    let stripped = strip_verbatim_prefix(path);
    let mut components = Vec::new();
    for component in stripped.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => match components.last() {
                Some(Component::Normal(_)) => {
                    components.pop();
                }
                _ => components.push(component),
            },
            other => components.push(other),
        }
    }
    let mut result = PathBuf::new();
    for component in &components {
        result.push(component.as_os_str());
    }
    if result.as_os_str().is_empty() {
        stripped
    } else {
        result
    }
}
