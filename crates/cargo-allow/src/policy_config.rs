use allow_core::{AllowConfig, CargoAllowError, CargoAllowResult};
use allow_policy::{find_config, load_policy};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EvidenceValidationMode {
    Abort,
    ReportOnly,
}

impl EvidenceValidationMode {
    pub(crate) fn aborts_on_broken_local_evidence(self) -> bool {
        matches!(self, Self::Abort)
    }
}

pub(crate) fn load_config_required(
    root: &Path,
    config: Option<&Path>,
) -> CargoAllowResult<AllowConfig> {
    let path = config_path(root, config).ok_or_else(|| {
        CargoAllowError::new("no policy config found; run `cargo-allow init` or pass --config")
    })?;
    load_policy_for_root(path)
}

pub(crate) fn load_config_optional(
    root: &Path,
    config: Option<&Path>,
) -> CargoAllowResult<Option<AllowConfig>> {
    match config_path(root, config) {
        Some(path) => Ok(Some(load_policy_for_root(path)?)),
        None => Ok(None),
    }
}

fn load_policy_for_root(path: PathBuf) -> CargoAllowResult<AllowConfig> {
    let cfg = load_policy(path)?;
    Ok(cfg)
}

pub(crate) fn config_path(root: &Path, config: Option<&Path>) -> Option<PathBuf> {
    config
        .map(|path| root_relative_path(root, path))
        .or_else(|| find_config(root))
}

pub(crate) fn root_relative_path(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

pub(crate) fn git_relative_config_path(
    root: &Path,
    config: Option<&Path>,
) -> CargoAllowResult<PathBuf> {
    let path = config_path(root, config).ok_or_else(|| {
        CargoAllowError::new("no policy config found; run `cargo-allow init` or pass --config")
    })?;
    let root = root.canonicalize().map_err(|e| {
        CargoAllowError::new(format!("failed to canonicalize {}: {e}", root.display()))
    })?;
    let path = path.canonicalize().map_err(|e| {
        CargoAllowError::new(format!("failed to canonicalize {}: {e}", path.display()))
    })?;
    path.strip_prefix(&root).map(PathBuf::from).map_err(|_| {
        CargoAllowError::new(format!(
            "policy config {} is not inside source tree {}",
            path.display(),
            root.display()
        ))
    })
}
