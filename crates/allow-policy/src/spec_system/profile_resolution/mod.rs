use allow_core::normalize_path;
use std::path::Path;

/// Relative path for a profile-specific config under `.allow/profiles/`.
pub fn allow_profile_rel_path(profile: &str) -> String {
    format!(".allow/profiles/{profile}.toml")
}

/// Relative path for the shared `.allow/` profile config.
pub const ALLOW_CONFIG_REL_PATH: &str = ".allow/config.toml";

/// Relative path for a legacy `policy/<profile>.toml` profile config.
pub fn legacy_profile_rel_path(profile: &str) -> String {
    format!("policy/{profile}.toml")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileConfigProvenance {
    ExplicitConfig,
    AllowProfiles,
    AllowConfig,
    LegacyPolicy,
    BuiltInDefault,
}

impl ProfileConfigProvenance {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ExplicitConfig => "explicit_config",
            Self::AllowProfiles => "allow_profiles",
            Self::AllowConfig => "allow_config",
            Self::LegacyPolicy => "legacy_policy",
            Self::BuiltInDefault => "built_in_default",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedProfileConfig {
    /// Repo-relative path of the selected config, when one exists on disk.
    pub path: Option<String>,
    pub provenance: ProfileConfigProvenance,
    /// Legacy `policy/<profile>.toml` present alongside an owned `.allow/` config.
    pub legacy_conflict_path: Option<String>,
}

pub fn resolve_profile_config(
    root: &Path,
    profile: &str,
    explicit_config: Option<&Path>,
) -> ResolvedProfileConfig {
    if let Some(explicit) = explicit_config {
        let path = normalize_repo_relative_path(explicit);
        return ResolvedProfileConfig {
            path: Some(path),
            provenance: ProfileConfigProvenance::ExplicitConfig,
            legacy_conflict_path: None,
        };
    }

    let allow_profile = allow_profile_rel_path(profile);
    let legacy = legacy_profile_rel_path(profile);
    let allow_profile_exists = root.join(&allow_profile).is_file();
    let allow_config_exists = root.join(ALLOW_CONFIG_REL_PATH).is_file();
    let legacy_exists = root.join(&legacy).is_file();

    let (path, provenance) = if allow_profile_exists {
        (
            Some(allow_profile.clone()),
            ProfileConfigProvenance::AllowProfiles,
        )
    } else if allow_config_exists {
        (
            Some(ALLOW_CONFIG_REL_PATH.to_string()),
            ProfileConfigProvenance::AllowConfig,
        )
    } else if legacy_exists {
        (Some(legacy.clone()), ProfileConfigProvenance::LegacyPolicy)
    } else {
        (None, ProfileConfigProvenance::BuiltInDefault)
    };

    let legacy_conflict_path = if matches!(
        provenance,
        ProfileConfigProvenance::AllowProfiles | ProfileConfigProvenance::AllowConfig
    ) && legacy_exists
    {
        Some(legacy)
    } else {
        None
    };

    ResolvedProfileConfig {
        path,
        provenance,
        legacy_conflict_path,
    }
}

pub fn profile_config_conflict_message(resolved: &ResolvedProfileConfig) -> Option<String> {
    let selected = resolved.path.as_deref()?;
    let legacy = resolved.legacy_conflict_path.as_deref()?;
    Some(format!(
        "both owned profile config `{selected}` and legacy `{legacy}` exist; using `{selected}` — remove or migrate the unused file to avoid ambiguity"
    ))
}

fn normalize_repo_relative_path(path: &Path) -> String {
    normalize_path(path)
}

#[cfg(test)]
mod tests;
