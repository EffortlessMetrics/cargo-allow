use std::path::Path;

pub struct LegacyPolicySource {
    pub file_name: String,
    pub compat_kind: &'static str,
}

pub fn legacy_compat_kind(file_name: &str) -> Option<&'static str> {
    match file_name {
        "non-rust-allowlist.toml" => Some("non-rust"),
        "generated-allowlist.toml" => Some("generated"),
        "no-panic-allowlist.toml" => Some("no-panic-allowlist"),
        "no-panic-baseline.toml" => Some("panic"),
        "clippy-exceptions.toml" => Some("lint-exception"),
        "unsafe-allowlist.toml" => Some("unsafe"),
        "executable-allowlist.toml" => Some("executable"),
        "workflow-allowlist.toml" => Some("workflow"),
        "dependency-surface-allowlist.toml" => Some("dependency-surface"),
        "process-allowlist.toml" => Some("process"),
        "network-allowlist.toml" => Some("network"),
        _ => None,
    }
}

pub fn legacy_policy_source_for_path(path: &Path) -> Option<LegacyPolicySource> {
    let file_name = path.file_name()?.to_str()?;
    let compat_kind = legacy_compat_kind(file_name)?;
    Some(LegacyPolicySource {
        file_name: file_name.to_string(),
        compat_kind,
    })
}

pub fn list_legacy_policy_sources_in_dir(dir: &Path) -> Vec<LegacyPolicySource> {
    crate::loader_policy_dir::legacy_policy_file_names()
        .iter()
        .filter_map(|file_name| {
            let path = dir.join(file_name);
            if !path.is_file() {
                return None;
            }
            let compat_kind = legacy_compat_kind(file_name)?;
            Some(LegacyPolicySource {
                file_name: (*file_name).to_string(),
                compat_kind,
            })
        })
        .collect()
}
