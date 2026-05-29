use std::path::{Path, PathBuf};

pub(super) fn compat_policy_path(
    config: Option<&Path>,
    root: &Path,
    default_policy_file: &str,
) -> PathBuf {
    config
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join(default_policy_file))
}
