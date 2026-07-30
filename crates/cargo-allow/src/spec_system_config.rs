use super::*;

pub(super) fn load_spec_system_config(
    root: &Path,
    config: Option<&Path>,
) -> LoadedSpecSystemConfig {
    let resolved = resolve_profile_config(root, PROFILE_NAME, config);
    let provenance = resolved.provenance;
    let config_path_text = resolved
        .path
        .clone()
        .unwrap_or_else(|| DEFAULT_PROFILE_CONFIG.to_string());
    let config_path = root_relative_path(root, Path::new(&config_path_text));
    let source = match provenance {
        ProfileConfigProvenance::BuiltInDefault => "default spec-system roots".to_string(),
        _ => config_path_text.clone(),
    };

    if provenance == ProfileConfigProvenance::BuiltInDefault {
        return LoadedSpecSystemConfig {
            cfg: default_spec_system_config(),
            source,
            provenance,
            path: config_path_text,
            found: false,
            valid: None,
            diagnostic: Some(format!(
                "spec-system profile config {} does not exist",
                config_path.display()
            )),
            resolved,
        };
    }

    if !config_path.exists() {
        return LoadedSpecSystemConfig {
            cfg: default_spec_system_config(),
            source: "default spec-system roots".to_string(),
            provenance,
            path: config_path_text,
            found: false,
            valid: None,
            diagnostic: Some(format!(
                "spec-system profile config {} does not exist",
                config_path.display()
            )),
            resolved,
        };
    }

    match read_text_file_capped(&config_path) {
        Ok(text) => match parse_spec_system_config_at(Some(&config_path), &text) {
            Ok(cfg) => LoadedSpecSystemConfig {
                cfg,
                source: config_path_text.clone(),
                provenance,
                path: config_path_text,
                found: true,
                valid: Some(true),
                diagnostic: None,
                resolved,
            },
            Err(err) => LoadedSpecSystemConfig {
                cfg: default_spec_system_config(),
                source: "default spec-system roots".to_string(),
                provenance,
                path: config_path_text,
                found: true,
                valid: Some(false),
                diagnostic: Some(err.to_string()),
                resolved,
            },
        },
        Err(err) => LoadedSpecSystemConfig {
            cfg: default_spec_system_config(),
            source: "default spec-system roots".to_string(),
            provenance,
            path: config_path_text,
            found: true,
            valid: Some(false),
            diagnostic: Some(format!(
                "failed to read spec-system profile config {}: {err}",
                config_path.display()
            )),
            resolved,
        },
    }
}

pub(super) fn profile_config_findings(
    loaded: &LoadedSpecSystemConfig,
    explicit_config: bool,
) -> Vec<SpecSystemFinding> {
    if loaded.valid == Some(false) || (explicit_config && !loaded.found) {
        return vec![SpecSystemFinding::new(
            "profile_config",
            loaded
                .diagnostic
                .clone()
                .unwrap_or_else(|| "spec-system profile config is invalid".to_string()),
        )];
    }
    Vec::new()
}
