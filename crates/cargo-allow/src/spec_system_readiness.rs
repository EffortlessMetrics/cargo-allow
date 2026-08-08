use super::*;

fn federation_ledgers_readiness_check(root: &Path) -> SpecSystemReadinessCheck {
    let path = allow_policy::federation::FEDERATION_CONFIG_REL_PATH.to_string();
    match load_federation_config(root) {
        Ok(loaded) => match loaded.outcome {
            FederationLoadOutcome::Missing => SpecSystemReadinessCheck {
                kind: "federation_ledgers",
                path: Some(path),
                found: false,
                valid: None,
                status: "ready",
                message: "federation ledger registry `.allow/config.toml` is not configured"
                    .to_string(),
            },
            FederationLoadOutcome::Parsed(validated) => {
                let count = validated.config.ledgers.len();
                readiness_check(
                    "federation_ledgers",
                    Some(path.clone()),
                    true,
                    Some(validated.valid),
                    if validated.valid {
                        format!("federation registry parsed with {count} configured ledger(s)")
                    } else {
                        format!(
                            "federation registry has {} validation issue(s)",
                            validated.diagnostics.len()
                        )
                    },
                )
            }
        },
        Err(err) => {
            let config_path = allow_policy::federation::FEDERATION_CONFIG_REL_PATH;
            readiness_check(
                "federation_ledgers",
                Some(config_path.to_string()),
                root.join(config_path).is_file(),
                Some(false),
                err.to_string(),
            )
        }
    }
}

pub(super) fn active_goal_manifest_source_path(cfg: &SpecSystemConfig) -> Option<String> {
    let goals = cfg.roots.goals.as_deref()?.trim_end_matches(['/', '\\']);
    Some(format!("{goals}/active.toml"))
}

pub(super) fn validate_active_goal_file(
    root: &Path,
    cfg: &SpecSystemConfig,
    ledger: &DocArtifactLedger,
) -> CargoAllowResult<()> {
    let source_path = active_goal_manifest_source_path(cfg).ok_or_else(|| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Artifact,
            "legacy active-goal validation requires an explicit legacy goals root",
        )
    })?;
    let active_goal_path = root_relative_path(root, Path::new(&source_path));
    let text = read_text_file_capped(&active_goal_path).map_err(|err| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Artifact,
            format!("failed to read active goal manifest {source_path}: {err}"),
        )
    })?;
    validate_active_goal_manifest_text_at(Some(&active_goal_path), &text, ledger).map(|_| ())
}

pub(super) fn collect_spec_system_readiness(
    root: &Path,
    loaded: &LoadedSpecSystemConfig,
) -> SpecSystemReadiness {
    let cfg = &loaded.cfg;
    let mut checks = Vec::new();
    checks.push(readiness_check(
        "profile_config",
        Some(loaded.path.clone()),
        loaded.found,
        loaded.valid,
        loaded.diagnostic.clone().unwrap_or_else(|| {
            if loaded.found {
                format!(
                    "spec-system profile config parsed (provenance: {})",
                    loaded.provenance.as_str()
                )
            } else {
                format!(
                    "spec-system profile config is missing; built-in roots are in use (provenance: {})",
                    loaded.provenance.as_str()
                )
            }
        }),
    ));

    for (label, path) in [
        ("artifact_root", Some(cfg.roots.proposals.as_str())),
        ("artifact_root", Some(cfg.roots.specs.as_str())),
        ("artifact_root", Some(cfg.roots.adrs.as_str())),
        ("artifact_root", Some(cfg.roots.plans.as_str())),
        ("artifact_root", cfg.roots.goals.as_deref()),
    ] {
        let Some(path) = path else {
            continue;
        };
        let full_path = root_relative_path(root, Path::new(path));
        checks.push(readiness_check(
            label,
            Some(path.to_string()),
            full_path.is_dir(),
            Some(full_path.is_dir()),
            if full_path.is_dir() {
                format!("artifact root {path} exists")
            } else {
                format!("artifact root {path} is missing")
            },
        ));
    }

    let ledger_path = root_relative_path(root, Path::new(&cfg.roots.artifact_ledger));
    let ledger_result = load_doc_artifacts(&ledger_path);
    let ledger_valid = ledger_result.is_ok();
    checks.push(readiness_check(
        "artifact_ledger",
        Some(cfg.roots.artifact_ledger.clone()),
        ledger_path.is_file(),
        Some(ledger_valid),
        match &ledger_result {
            Ok(_) => format!("doc artifact ledger {} parsed", cfg.roots.artifact_ledger),
            Err(err) => err.to_string(),
        },
    ));

    let support_tiers_path = root_relative_path(root, Path::new(&cfg.roots.support_tiers));
    let support_tiers_result = read_text_file_capped(&support_tiers_path)
        .map_err(|err| {
            format!(
                "failed to read support-tier file {}: {err}",
                cfg.roots.support_tiers
            )
        })
        .and_then(|text| validate_support_tier_claims(&text).map_err(|err| err.to_string()));
    checks.push(readiness_check(
        "support_tiers",
        Some(cfg.roots.support_tiers.clone()),
        support_tiers_path.is_file(),
        Some(support_tiers_result.is_ok()),
        match support_tiers_result {
            Ok(_) => format!("support-tier file {} parsed", cfg.roots.support_tiers),
            Err(err) => err,
        },
    ));

    if matches!(cfg.generation, SpecSystemGeneration::LegacyV1) {
        let active_goal = active_goal_manifest_source_path(cfg)
            .unwrap_or_else(|| ".allow/goals/active.toml".to_string());
        let active_goal_path = root_relative_path(root, Path::new(&active_goal));
        if cfg.requirements.active_goal_required {
            let active_goal_result = match &ledger_result {
                Ok(ledger) => {
                    validate_active_goal_file(root, cfg, ledger).map_err(|err| err.to_string())
                }
                Err(err) => Err(format!(
                    "active goal manifest cannot be validated until doc artifact ledger parses: {err}"
                )),
            };
            let active_goal_valid = active_goal_result.is_ok();
            checks.push(readiness_check(
                "active_goal",
                Some(active_goal.clone()),
                active_goal_path.is_file(),
                Some(active_goal_valid),
                match active_goal_result {
                    Ok(()) => format!("active goal manifest {active_goal} parsed"),
                    Err(err) => err,
                },
            ));
        } else {
            checks.push(SpecSystemReadinessCheck {
                kind: "active_goal",
                path: Some(active_goal.clone()),
                found: active_goal_path.is_file(),
                valid: None,
                status: "ready",
                message: "active goal validation is optional because active_goal_required = false"
                    .to_string(),
            });
        }
    }

    let missing_templates = EXPECTED_TEMPLATE_FILES
        .iter()
        .filter(|path| !root_relative_path(root, Path::new(path)).is_file())
        .copied()
        .collect::<Vec<_>>();
    checks.push(readiness_check(
        "templates",
        Some("docs/templates".to_string()),
        missing_templates.is_empty(),
        Some(missing_templates.is_empty()),
        if missing_templates.is_empty() {
            "all spec-system templates exist".to_string()
        } else {
            format!(
                "missing spec-system templates: {}",
                missing_templates.join(", ")
            )
        },
    ));

    if matches!(
        loaded.provenance,
        ProfileConfigProvenance::AllowProfiles | ProfileConfigProvenance::AllowConfig
    ) {
        let imports_path = root_relative_path(root, Path::new(DEFAULT_OWNED_IMPORTS_ROOT));
        checks.push(readiness_check(
            "allow_imports",
            Some(DEFAULT_OWNED_IMPORTS_ROOT.to_string()),
            imports_path.is_dir(),
            Some(imports_path.is_dir()),
            if imports_path.is_dir() {
                format!("owned import root {DEFAULT_OWNED_IMPORTS_ROOT} exists")
            } else {
                format!("owned import root {DEFAULT_OWNED_IMPORTS_ROOT} is missing")
            },
        ));
    }

    checks.push(federation_ledgers_readiness_check(root));

    SpecSystemReadiness {
        ready: checks.iter().all(|check| check.status == "ready"),
        mode: spec_system_mode_name(&cfg.mode),
        checks,
    }
}

fn readiness_check(
    kind: &'static str,
    path: Option<String>,
    found: bool,
    valid: Option<bool>,
    message: String,
) -> SpecSystemReadinessCheck {
    let status = match (found, valid) {
        (false, _) => "missing",
        (true, Some(false)) => "invalid",
        (true, _) => "ready",
    };
    SpecSystemReadinessCheck {
        kind,
        path,
        found,
        valid,
        status,
        message,
    }
}
