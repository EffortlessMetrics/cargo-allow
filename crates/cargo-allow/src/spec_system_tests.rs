use super::*;
use allow_policy::spec_system::ResolvedProfileConfig;

fn test_loaded_spec_system_config(cfg: SpecSystemConfig) -> LoadedSpecSystemConfig {
    LoadedSpecSystemConfig {
        cfg,
        source: "built-in".to_string(),
        provenance: ProfileConfigProvenance::BuiltInDefault,
        path: DEFAULT_PROFILE_CONFIG.to_string(),
        found: true,
        valid: Some(true),
        diagnostic: None,
        resolved: ResolvedProfileConfig {
            path: None,
            provenance: ProfileConfigProvenance::BuiltInDefault,
            legacy_conflict_path: None,
        },
    }
}

fn legacy_test_config() -> SpecSystemConfig {
    let mut cfg = default_spec_system_config();
    cfg.generation = SpecSystemGeneration::LegacyV1;
    cfg.roots.goals = Some(".codex/goals".to_string());
    cfg.requirements.active_goal_required = true;
    cfg
}

#[test]
fn spec_system_name_helpers_cover_all_variants() {
    assert_eq!(artifact_kind_name(ArtifactKind::Proposal), "proposal");
    assert_eq!(artifact_kind_name(ArtifactKind::Spec), "spec");
    assert_eq!(artifact_kind_name(ArtifactKind::Adr), "adr");
    assert_eq!(
        artifact_kind_name(ArtifactKind::ImplementationPlan),
        "implementation_plan"
    );
    assert_eq!(artifact_kind_name(ArtifactKind::PlanItem), "plan_item");
    assert_eq!(artifact_kind_name(ArtifactKind::ActiveGoal), "active_goal");
    assert_eq!(
        artifact_kind_name(ArtifactKind::SupportTier),
        "support_tier"
    );
    assert_eq!(
        artifact_kind_name(ArtifactKind::PolicyLedger),
        "policy_ledger"
    );
    assert_eq!(artifact_kind_name(ArtifactKind::Closeout), "closeout");
    assert_eq!(
        artifact_kind_name(ArtifactKind::ReleaseRecord),
        "release_record"
    );

    assert_eq!(artifact_status_name(ArtifactStatus::Draft), "draft");
    assert_eq!(artifact_status_name(ArtifactStatus::Proposed), "proposed");
    assert_eq!(artifact_status_name(ArtifactStatus::Accepted), "accepted");
    assert_eq!(artifact_status_name(ArtifactStatus::Active), "active");
    assert_eq!(artifact_status_name(ArtifactStatus::Done), "done");
    assert_eq!(
        artifact_status_name(ArtifactStatus::Superseded),
        "superseded"
    );

    assert_eq!(spec_system_mode_name(&SpecSystemMode::Advisory), "advisory");
    assert_eq!(spec_system_mode_name(&SpecSystemMode::Shadow), "shadow");
    assert_eq!(spec_system_mode_name(&SpecSystemMode::Blocking), "blocking");

    assert_eq!(support_tier_level_name(SupportTierLevel::Stable), "stable");
    assert_eq!(
        support_tier_level_name(SupportTierLevel::Stabilizing),
        "stabilizing"
    );
    assert_eq!(
        support_tier_level_name(SupportTierLevel::Advisory),
        "advisory"
    );
}

#[test]
fn spec_system_json_helpers_escape_values_and_optional_bools() {
    assert_eq!(
        json_escape("quote: \" slash: \\ newline:\n tab:\t return:\r bell:\u{0007}"),
        "quote: \\\" slash: \\\\ newline:\\n tab:\\t return:\\r bell: "
    );
    assert_eq!(json_escape("plain"), "plain");

    assert_eq!(optional_bool_json(Some(true)), "true");
    assert_eq!(optional_bool_json(Some(false)), "false");
    assert_eq!(optional_bool_json(None), "null");
}

#[test]
fn spec_system_finding_blocking_reasons_are_discriminated() {
    assert_eq!(
        spec_system_blocking_reason(
            "profile_config",
            "failed to parse spec-system config TOML: invalid type"
        ),
        Some("profile_config_parse_failure")
    );
    assert_eq!(
        spec_system_blocking_reason("profile_config", "policy/spec-system.toml does not exist"),
        None
    );
    assert_eq!(
        spec_system_blocking_reason(
            "profile_config",
            "both owned profile config `.allow/profiles/spec-system.toml` and legacy `policy/spec-system.toml` exist"
        ),
        None
    );

    assert_eq!(
        spec_system_blocking_reason("doc_artifact_ledger", "failed to read doc artifact ledger"),
        Some("doc_artifact_ledger_missing")
    );
    assert_eq!(
        spec_system_blocking_reason(
            "doc_artifact_ledger",
            "failed to parse doc artifact ledger TOML: unknown variant `bad_kind`"
        ),
        Some("invalid_artifact_kind_or_status")
    );
    assert_eq!(
        spec_system_blocking_reason(
            "doc_artifact_ledger",
            "failed to parse doc artifact ledger TOML"
        ),
        Some("doc_artifact_ledger_parse_failure")
    );
    assert_eq!(
        spec_system_blocking_reason(
            "doc_artifact_ledger",
            "duplicate doc artifact id CARGO-ALLOW-SPEC-0001"
        ),
        Some("duplicate_id")
    );

    assert_eq!(
        spec_system_blocking_reason(
            "artifact_file",
            "CARGO-ALLOW-SPEC-0001 artifact file missing: docs/specs/missing.md"
        ),
        Some("artifact_file_missing")
    );
    assert_eq!(
        spec_system_blocking_reason("artifact_file", "failed to read artifact CARGO"),
        Some("artifact_file_unreadable")
    );
    assert_eq!(
        spec_system_blocking_reason(
            "artifact_file",
            "CARGO-ALLOW-SPEC-0001 not found in artifact file docs/specs/spec.md"
        ),
        Some("artifact_id_not_in_file")
    );

    assert_eq!(
        spec_system_blocking_reason(
            "artifact_link",
            "CARGO-ALLOW-SPEC-0001 linked_proposal target CARGO-ALLOW-PROP-9999 is not registered"
        ),
        Some("unknown_link_target")
    );
    assert_eq!(
        spec_system_blocking_reason("active_goal", "stale goal"),
        None
    );
}

#[test]
fn spec_system_work_item_blocking_reasons_are_discriminated() {
    assert_eq!(
        spec_system_work_item_blocking_reason(&work_item(
            "artifact_file_missing",
            "registered artifact file is missing"
        )),
        Some("artifact_file_missing")
    );
    assert_eq!(
        spec_system_work_item_blocking_reason(&work_item(
            "artifact_file_unreadable",
            "registered artifact file is unreadable"
        )),
        Some("artifact_file_unreadable")
    );
    assert_eq!(
        spec_system_work_item_blocking_reason(&work_item(
            "artifact_id_not_in_file",
            "registered artifact file does not contain its id"
        )),
        Some("artifact_id_not_in_file")
    );
    assert_eq!(
        spec_system_work_item_blocking_reason(&work_item(
            "unknown_link_target",
            "linked target is unknown"
        )),
        Some("unknown_link_target")
    );
    assert_eq!(
        spec_system_work_item_blocking_reason(&work_item(
            "missing_node",
            "spec-system profile config failed to parse"
        )),
        Some("profile_config_parse_failure")
    );
    assert_eq!(
        spec_system_work_item_blocking_reason(&work_item(
            "missing_node",
            "doc artifact ledger failed to parse doc artifact ledger TOML: unknown variant"
        )),
        Some("invalid_artifact_kind_or_status")
    );
    assert_eq!(
        spec_system_work_item_blocking_reason(&work_item(
            "missing_node",
            "doc artifact ledger duplicate doc artifact id CARGO-ALLOW-SPEC-0001"
        )),
        Some("duplicate_id")
    );
    assert_eq!(
        spec_system_work_item_blocking_reason(&work_item(
            "missing_closeout",
            "done work item has no closeout"
        )),
        None
    );
}

#[test]
fn validate_active_goal_file_reports_source_path_read_errors() -> std::io::Result<()> {
    let root = temp_root("missing-active-goal")?;
    let cfg = legacy_test_config();
    let ledger = empty_doc_artifact_ledger();

    let err = match validate_active_goal_file(&root, &cfg, &ledger) {
        Ok(()) => {
            return Err(std::io::Error::other(
                "missing active goal file should be reported",
            ));
        }
        Err(err) => err,
    };
    let _ = std::fs::remove_dir_all(&root);

    assert!(
        err.to_string()
            .contains("failed to read active goal manifest .codex/goals/active.toml"),
        "unexpected active goal read error: {err}"
    );
    Ok(())
}

#[test]
fn collect_spec_system_readiness_discriminates_invalid_inputs() -> std::io::Result<()> {
    let root = temp_root("invalid-readiness")?;
    let cfg = legacy_test_config();
    for path in [
        Some(cfg.roots.proposals.as_str()),
        Some(cfg.roots.specs.as_str()),
        Some(cfg.roots.adrs.as_str()),
        Some(cfg.roots.plans.as_str()),
        cfg.roots.goals.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        std::fs::create_dir_all(root.join(path))?;
    }
    write_fixture_file(&root, &cfg.roots.artifact_ledger, "not = valid = toml")?;

    let readiness = collect_spec_system_readiness(&root, &test_loaded_spec_system_config(cfg));
    let _ = std::fs::remove_dir_all(&root);

    let ledger = readiness_check_by_kind(&readiness, "artifact_ledger");
    assert!(
        ledger.is_some(),
        "missing artifact_ledger check: {readiness:?}"
    );
    let Some(ledger) = ledger else {
        return Ok(());
    };
    assert!(ledger.found);
    assert_eq!(ledger.valid, Some(false));
    assert_eq!(ledger.status, "invalid");
    assert!(
        ledger
            .message
            .contains("failed to parse doc artifact ledger TOML"),
        "unexpected ledger message: {}",
        ledger.message
    );

    let support_tiers = readiness_check_by_kind(&readiness, "support_tiers");
    assert!(
        support_tiers.is_some(),
        "missing support_tiers check: {readiness:?}"
    );
    let Some(support_tiers) = support_tiers else {
        return Ok(());
    };
    assert!(!support_tiers.found);
    assert_eq!(support_tiers.valid, Some(false));
    assert_eq!(support_tiers.status, "missing");
    assert!(
        support_tiers
            .message
            .contains("failed to read support-tier file docs/status/SUPPORT_TIERS.md"),
        "unexpected support-tier message: {}",
        support_tiers.message
    );

    let active_goal = readiness_check_by_kind(&readiness, "active_goal");
    assert!(
        active_goal.is_some(),
        "missing active_goal check: {readiness:?}"
    );
    let Some(active_goal) = active_goal else {
        return Ok(());
    };
    assert!(!active_goal.found);
    assert_eq!(active_goal.valid, Some(false));
    assert_eq!(active_goal.status, "missing");
    assert!(
        active_goal
            .message
            .contains("active goal manifest cannot be validated until doc artifact ledger parses"),
        "unexpected active-goal message: {}",
        active_goal.message
    );
    Ok(())
}

#[test]
fn collect_spec_system_readiness_discriminates_invalid_active_goal() -> std::io::Result<()> {
    let root = temp_root("invalid-active-goal")?;
    for file in spec_system_bootstrap_files(Path::new(DEFAULT_PROFILE_CONFIG), false) {
        write_fixture_file(&root, &file.path.display().to_string(), &file.contents)?;
    }
    write_fixture_file(
        &root,
        ".codex/goals/active.toml",
        "schema_version = 1\nstatus = []\n",
    )?;

    let readiness =
        collect_spec_system_readiness(&root, &test_loaded_spec_system_config(legacy_test_config()));
    let _ = std::fs::remove_dir_all(&root);

    let active_goal = readiness_check_by_kind(&readiness, "active_goal");
    assert!(
        active_goal.is_some(),
        "missing active_goal check: {readiness:?}"
    );
    let Some(active_goal) = active_goal else {
        return Ok(());
    };
    assert!(active_goal.found);
    assert_eq!(active_goal.valid, Some(false));
    assert_eq!(active_goal.status, "invalid");
    assert!(
        active_goal.message.contains("active goal")
            || active_goal.message.contains("failed to parse"),
        "unexpected active-goal message: {}",
        active_goal.message
    );
    Ok(())
}

fn work_item(kind: &'static str, message: &'static str) -> SpecSystemWorkItem {
    SpecSystemWorkItem {
        kind,
        artifact_id: None,
        path: None,
        owner: None,
        status: None,
        message: message.to_string(),
        suggested_actions: Vec::new(),
        proof_commands: Vec::new(),
        ledger_id: None,
        ledger_path: None,
        lane: None,
        mode: None,
        role: None,
    }
}

fn empty_doc_artifact_ledger() -> DocArtifactLedger {
    DocArtifactLedger {
        schema_version: "1.0".to_string(),
        policy: "cargo-allow-doc-artifacts".to_string(),
        owner: "repo-infra".to_string(),
        status: SpecSystemMode::Advisory,
        artifact: Vec::new(),
    }
}

fn readiness_check_by_kind<'a>(
    readiness: &'a SpecSystemReadiness,
    kind: &str,
) -> Option<&'a SpecSystemReadinessCheck> {
    readiness.checks.iter().find(|check| check.kind == kind)
}

#[test]
fn parse_spec_system_mode_override_maps_audit_and_fails_closed() {
    // #1941: `audit` (the report-only source-tree mode and the documented
    // proof command) maps to advisory, case-insensitive. The enforcing
    // source-tree modes have no spec-system meaning and must fail closed
    // instead of being silently dropped.
    for value in ["audit", "AUDIT", "  audit  "] {
        assert_eq!(
            parse_spec_system_mode_override(value).unwrap_or_else(|err| {
                std::panic::panic_any(format!("{value} should parse: {err}"))
            }),
            SpecSystemMode::Advisory,
            "{value} should map to advisory"
        );
    }
    for value in ["no-new", "strict", "release", "blocking"] {
        let err = parse_spec_system_mode_override(value)
            .expect_err("an unsupported spec-system mode must fail closed");
        assert!(
            err.to_string()
                .contains(&format!("--mode `{value}` is not supported")),
            "error should name the rejected value: {err}"
        );
    }
}

#[test]
fn spec_system_check_mode_override_reaches_report_mode() -> std::io::Result<()> {
    // #1941: an explicit --mode must reach the spec-system evaluation and
    // override the config mode, instead of being silently dropped.
    let root = temp_root("mode-override")?;
    for file in spec_system_bootstrap_files(Path::new(DEFAULT_PROFILE_CONFIG), false) {
        write_fixture_file(&root, &file.path.display().to_string(), &file.contents)?;
    }
    let root_args = RootArgs {
        root: Some(root.clone()),
    };

    let build = |mode: Option<SpecSystemMode>| {
        super::spec_system_report::build_spec_system_report(
            "check", &root_args, None, false, false, mode,
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("report builds: {err}")))
    };

    // `--mode blocking` forces blocking even though the bootstrap config is
    // not blocking; `--mode audit` maps to advisory; no override keeps the
    // config mode.
    let blocking = build(Some(SpecSystemMode::Blocking));
    assert_eq!(blocking.mode, SpecSystemMode::Blocking);
    let advisory = build(Some(SpecSystemMode::Advisory));
    assert_eq!(advisory.mode, SpecSystemMode::Advisory);
    let default_mode = build(None).mode;
    assert_ne!(
        default_mode,
        SpecSystemMode::Blocking,
        "override must be what forced blocking, not the config default"
    );

    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

fn temp_root(name: &str) -> std::io::Result<PathBuf> {
    let mut root = std::env::temp_dir();
    let nanos = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos(),
        Err(_) => 0,
    };
    root.push(format!(
        "cargo-allow-spec-system-{name}-{}-{}",
        std::process::id(),
        nanos
    ));
    std::fs::create_dir_all(&root)?;
    Ok(root)
}

fn write_fixture_file(root: &Path, relative: &str, contents: &str) -> std::io::Result<()> {
    let path = root_relative_path(root, Path::new(relative));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, contents)
}

#[test]
fn typed_blocking_classification_is_stable_under_message_text_change() {
    // #1942: blocking classification should not depend on rendered error
    // message text. Two findings with the same diagnostic_kind but different
    // messages should have the same blocking eligibility.
    use crate::spec_system::SpecSystemFinding;

    let finding_a = SpecSystemFinding::new_typed(
        "profile_config",
        "failed to parse spec-system config TOML at line 5".to_string(),
        "profile_config_parse_failure",
    );
    let finding_b = SpecSystemFinding::new_typed(
        "profile_config",
        "completely different error message wording".to_string(),
        "profile_config_parse_failure",
    );

    assert!(
        finding_a.blocking_eligible,
        "profile_config_parse_failure should be blocking"
    );
    assert_eq!(
        finding_a.blocking_eligible, finding_b.blocking_eligible,
        "blocking eligibility must be stable under message text change"
    );
    assert_eq!(
        finding_a.blocking_reason, finding_b.blocking_reason,
        "blocking reason must be stable under message text change"
    );
}

#[test]
fn owned_and_legacy_profile_config_conflict_stays_advisory() {
    // #3269 migrated this finding to `new_typed` but reused the parse-failure
    // diagnostic kind, which silently escalated an ambiguity advisory into a
    // blocking finding. Profile resolution *succeeds* here: one file is
    // selected deterministically and the operator is asked to remove the
    // unused one. It must never block.
    use crate::spec_system::SpecSystemFinding;

    let finding = SpecSystemFinding::new_typed(
        "profile_config",
        "both owned profile config `.allow/profiles/spec-system.toml` and legacy \
         `policy/spec-system.toml` exist; using `.allow/profiles/spec-system.toml` \
         — remove or migrate the unused file to avoid ambiguity"
            .to_string(),
        "profile_config_legacy_conflict",
    );

    assert!(
        !finding.blocking_eligible,
        "an owned/legacy profile-config conflict resolves deterministically and must stay advisory"
    );
    assert_eq!(
        finding.blocking_reason, None,
        "an advisory conflict must not carry a blocking reason"
    );
}
