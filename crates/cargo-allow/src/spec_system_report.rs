use super::*;

pub(super) fn build_spec_system_report(
    command: &str,
    root_args: &RootArgs,
    config: Option<&Path>,
    include_work_items: bool,
    include_readiness: bool,
    mode_override: Option<SpecSystemMode>,
) -> CargoAllowResult<SpecSystemReport> {
    let cwd = current_dir()?;
    let root = resolve_source_tree_root(root_args.root.as_deref(), cwd)?;
    let loaded_config = load_spec_system_config(&root, config);
    let mut cfg = loaded_config.cfg.clone();
    // An explicit `--mode` overrides the config mode (mirrors source-tree
    // `check`), so `--mode blocking`/`--mode audit` are honored instead of
    // silently dropped (#1941).
    if let Some(mode) = mode_override {
        cfg.mode = mode;
    }
    let config_source = loaded_config.source.clone();
    let config_provenance = loaded_config.provenance.as_str().to_string();
    let mut findings = profile_config_findings(&loaded_config, config.is_some());
    if let Some(message) = profile_config_conflict_message(&loaded_config.resolved) {
        findings.push(SpecSystemFinding::new("profile_config", message));
    }
    findings.extend(federation_config_findings(&root));
    let mut artifacts = Vec::new();
    let mut links = Vec::new();
    let mut support_tier_rows = 0;
    let mut work_items = Vec::new();

    if matches!(cfg.generation, SpecSystemGeneration::CurrentV2) {
        let legacy_active_goal = root.join(".allow/goals/active.toml");
        if legacy_active_goal.is_file() {
            let message = format!(
                "legacy active goal manifest {} is historical-only; it cannot select current work, authorize mutation, or promote implementation/support state",
                legacy_active_goal
                    .strip_prefix(&root)
                    .unwrap_or(&legacy_active_goal)
                    .display()
            );
            findings.push(SpecSystemFinding::new(
                "legacy_active_goal_present",
                message.clone(),
            ));
            if include_work_items {
                work_items.push(SpecSystemWorkItem {
                    kind: "legacy_goal_historical_only",
                    artifact_id: None,
                    path: Some(".allow/goals/active.toml".to_string()),
                    owner: None,
                    status: Some("historical_only".to_string()),
                    message,
                    suggested_actions: vec![
                        "archive or remove the legacy active-goal file after preserving its closeout history"
                            .to_string(),
                        "do not use the legacy file as a current issue, implementation, or mutation authority"
                            .to_string(),
                    ],
                    proof_commands: spec_system_proof_commands(),
                    ledger_id: None,
                    ledger_path: None,
                    lane: Some("migration".to_string()),
                    mode: Some("advisory".to_string()),
                    role: Some("legacy".to_string()),
                });
            }
        }
    }

    let ledger_path = root_relative_path(&root, Path::new(&cfg.roots.artifact_ledger));
    match load_doc_artifacts(&ledger_path) {
        Ok(ledger) => {
            artifacts = collect_artifacts(&ledger);
            links = collect_links(&ledger);
            if include_work_items {
                work_items.extend(work_items_from_artifact_files(&root, &ledger));
                work_items.extend(work_items_from_artifact_links(&ledger));
                work_items.extend(work_items_from_missing_closeouts(
                    &ledger,
                    cfg.requirements.closeout_required_for_done_items,
                ));
            }
            collect_validation(
                &mut findings,
                "artifact_file",
                validate_doc_artifact_files(&root, &ledger, &cfg.roots),
            );
            collect_validation(
                &mut findings,
                "artifact_link",
                validate_doc_artifact_links(&ledger),
            );
            if cfg.requirements.active_goal_required {
                let active_goal_result = validate_active_goal_file(&root, &cfg, &ledger);
                if let Err(err) = active_goal_result {
                    let message = err.to_string();
                    findings.push(SpecSystemFinding::new("active_goal", message.clone()));
                    if include_work_items {
                        let active_goal_path = active_goal_manifest_source_path(&cfg)
                            .unwrap_or_else(|| ".allow/goals/active.toml".to_string());
                        work_items.push(active_goal_work_item(&active_goal_path, &message));
                    }
                }
            }
        }
        Err(err) => {
            let message = err.to_string();
            findings.push(SpecSystemFinding::new(
                "doc_artifact_ledger",
                message.clone(),
            ));
            if include_work_items && cfg.requirements.ledger_required {
                work_items.push(missing_node_work_item(
                    "doc artifact ledger",
                    &cfg.roots.artifact_ledger,
                    &message,
                    vec![
                        format!(
                            "create {} with registered source-of-truth artifacts",
                            cfg.roots.artifact_ledger
                        ),
                        "or correct the configured artifact_ledger path in the spec-system profile config"
                            .to_string(),
                    ],
                ));
            }
        }
    }

    let support_tiers_path = root_relative_path(&root, Path::new(&cfg.roots.support_tiers));
    match read_text_file_capped(&support_tiers_path) {
        Ok(text) => match parse_support_tier_claims(&text) {
            Ok(rows) => {
                support_tier_rows = rows.len();
                if include_work_items {
                    work_items.extend(work_items_from_support_tiers(
                        &cfg.roots.support_tiers,
                        &rows,
                    ));
                }
                if let Err(err) = validate_support_tier_claims(&text) {
                    findings.push(SpecSystemFinding::new("support_tier", err.to_string()));
                }
            }
            Err(err) => {
                findings.push(SpecSystemFinding::new("support_tier", err.to_string()));
                if include_work_items && cfg.requirements.support_tiers_required {
                    work_items.push(SpecSystemWorkItem {
                        kind: "missing_support_tier",
                        artifact_id: None,
                        path: Some(cfg.roots.support_tiers.clone()),
                        owner: None,
                        status: None,
                        message: "support-tier claims table is missing or invalid".to_string(),
                        suggested_actions: vec![
                            "add a support-tier table with Surface, Tier, Claim, Proof command, and Notes columns"
                                .to_string(),
                            "or correct the configured support_tiers path in the spec-system profile config"
                                .to_string(),
                        ],
                        proof_commands: spec_system_proof_commands(),
                        ledger_id: None,
                        ledger_path: None,
                        lane: None,
                        mode: None,
                        role: None,
                    });
                }
            }
        },
        Err(err) => {
            let message = format!(
                "failed to read support-tier file {}: {err}",
                cfg.roots.support_tiers
            );
            findings.push(SpecSystemFinding::new("support_tier", message.clone()));
            if include_work_items && cfg.requirements.support_tiers_required {
                work_items.push(SpecSystemWorkItem {
                    kind: "missing_support_tier",
                    artifact_id: None,
                    path: Some(cfg.roots.support_tiers.clone()),
                    owner: None,
                    status: None,
                    message,
                    suggested_actions: vec![
                        "create docs/status/SUPPORT_TIERS.md with claim-to-proof rows".to_string(),
                        "or correct the configured support_tiers path in the spec-system profile config"
                            .to_string(),
                    ],
                    proof_commands: spec_system_proof_commands(),
                    ledger_id: None,
                    ledger_path: None,
                    lane: None,
                    mode: None,
                    role: None,
                });
            }
        }
    }

    if include_work_items {
        work_items.extend(work_items_from_config_findings(&findings));
    }
    let import_graph = discover_spec_system_import_graph(&root, cfg.import_roots.as_ref());
    findings.extend(import_graph_findings(&import_graph));
    if include_work_items {
        work_items.extend(work_items_from_import_graph(&import_graph));
    }
    let import_graph_summary = Some(import_graph_summary_from_graph(&import_graph));
    let federation = spec_system_federation_summary(&root, &mut work_items);
    let readiness = if include_readiness {
        Some(collect_spec_system_readiness(&root, &loaded_config))
    } else {
        None
    };

    Ok(SpecSystemReport {
        command: command.to_string(),
        root,
        config_source,
        config_provenance,
        mode: cfg.mode,
        artifacts,
        links,
        support_tier_rows,
        findings,
        work_items,
        readiness,
        federation,
        import_graph: import_graph_summary,
    })
}
