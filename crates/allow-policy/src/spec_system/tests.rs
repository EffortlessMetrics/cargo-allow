use super::*;
use allow_core::CargoAllowErrorKind;
use std::path::Path;

#[test]
fn parses_minimal_spec_system_config() {
    let cfg_result = parse_spec_system_config(
        r#"
            schema_version = "1.0"
            profile = "spec-system"
            mode = "advisory"

            [roots]
            proposals = "docs/proposals"
            specs = "docs/specs"
            adrs = "docs/adr"
            plans = "plans"
            goals = ".allow/goals"
            support_tiers = "docs/status/SUPPORT_TIERS.md"
            artifact_ledger = ".allow/artifacts/doc-artifacts.toml"

            [requirements]
            ledger_required = true
            templates_required = true
            support_tiers_required = true
            active_goal_required = true
            closeout_required_for_done_items = true
        "#,
    );
    assert!(
        cfg_result.is_ok(),
        "config should parse: {:?}",
        cfg_result.err()
    );
    let Ok(cfg) = cfg_result else {
        return;
    };

    assert_eq!(cfg.schema_version, "1.0");
    assert_eq!(cfg.profile, "spec-system");
    assert_eq!(cfg.mode, SpecSystemMode::Advisory);
    assert_eq!(cfg.generation, SpecSystemGeneration::LegacyV1);
    assert_eq!(
        cfg.roots.artifact_ledger,
        ".allow/artifacts/doc-artifacts.toml"
    );
    assert!(cfg.requirements.ledger_required);
    assert!(cfg.requirements.closeout_required_for_done_items);
}

#[test]
fn parses_current_spec_system_config_without_active_goal() -> Result<(), String> {
    let cfg = parse_spec_system_config(
        r#"
            schema_version = "1.0"
            profile = "spec-system"
            mode = "advisory"
            generation = "current-v2"

            [roots]
            proposals = "docs/proposals"
            specs = "docs/specs"
            adrs = "docs/adr"
            plans = "plans"
            support_tiers = "docs/status/SUPPORT_TIERS.md"
            artifact_ledger = ".allow/artifacts/doc-artifacts.toml"
        "#,
    )
    .map_err(|error| error.to_string())?;

    assert_eq!(cfg.generation, SpecSystemGeneration::CurrentV2);
    assert!(cfg.roots.goals.is_none());
    assert!(!cfg.requirements.active_goal_required);
    Ok(())
}

#[test]
fn rejects_current_spec_system_config_with_legacy_goal_fields() -> Result<(), String> {
    let err = match parse_spec_system_config(
        r#"
            schema_version = "1.0"
            profile = "spec-system"
            mode = "advisory"
            generation = "current-v2"

            [roots]
            proposals = "docs/proposals"
            specs = "docs/specs"
            adrs = "docs/adr"
            plans = "plans"
            goals = ".allow/goals"
            support_tiers = "docs/status/SUPPORT_TIERS.md"
            artifact_ledger = ".allow/artifacts/doc-artifacts.toml"
        "#,
    ) {
        Ok(_) => return Err("current profile accepted a legacy goals root".to_string()),
        Err(error) => error,
    };

    assert!(err.to_string().contains("current-v2"));
    Ok(())
}

#[test]
fn rejects_unknown_spec_system_generation() -> Result<(), String> {
    let err = match parse_spec_system_config(
        r#"
            schema_version = "1.0"
            profile = "spec-system"
            mode = "advisory"
            generation = "future-v3"

            [roots]
            proposals = "docs/proposals"
            specs = "docs/specs"
            adrs = "docs/adr"
            plans = "plans"
            support_tiers = "docs/status/SUPPORT_TIERS.md"
            artifact_ledger = ".allow/artifacts/doc-artifacts.toml"
        "#,
    ) {
        Ok(_) => return Err("unknown profile generation was accepted".to_string()),
        Err(error) => error,
    };

    assert!(err.to_string().contains("unknown variant") || err.to_string().contains("future-v3"));
    Ok(())
}

#[test]
fn parse_spec_system_config_at_preserves_location() -> Result<(), String> {
    let err = match parse_spec_system_config_at(
        Some(Path::new(".allow/profiles/spec-system.toml")),
        "mode = [",
    ) {
        Ok(_) => return Err("invalid spec-system TOML unexpectedly parsed".to_string()),
        Err(err) => err,
    };
    assert_eq!(err.kind(), allow_core::CargoAllowErrorKind::InvalidConfig);
    let location = err
        .location()
        .ok_or_else(|| "spec-system parse error should have a location".to_string())?;
    assert_eq!(
        location.path.as_deref(),
        Some(".allow/profiles/spec-system.toml")
    );
    assert_eq!(location.line, 1);
    assert!(location.column > 0);
    Ok(())
}

#[test]
fn parses_spec_system_config_with_import_roots() {
    let cfg_result = parse_spec_system_config(
        r#"
            schema_version = "1.0"
            profile = "spec-system"
            mode = "advisory"

            [roots]
            proposals = "docs/proposals"
            specs = "docs/specs"
            adrs = "docs/adr"
            plans = "plans"
            goals = ".allow/goals"
            support_tiers = "docs/status/SUPPORT_TIERS.md"
            artifact_ledger = ".allow/artifacts/doc-artifacts.toml"

            [import_roots]
            owned = ".allow/imports"

            [[import_roots.entries]]
            id = "kiro"
            path = ".kiro"
            ecosystem = "kiro"
            role = "imported"
        "#,
    );
    assert!(
        cfg_result.is_ok(),
        "config with import roots should parse: {:?}",
        cfg_result.err()
    );
    let Ok(cfg) = cfg_result else {
        return;
    };
    let Some(import_roots) = cfg.import_roots.as_ref() else {
        std::panic::panic_any("expected import_roots section");
    };
    assert_eq!(import_roots.owned.as_deref(), Some(".allow/imports"));
    assert_eq!(import_roots.entries.len(), 1);
    assert_eq!(import_roots.entries[0].role, ImportNodeRole::Imported);
}

#[test]
fn parses_current_repository_doc_artifact_ledger() {
    let ledger_result = parse_doc_artifact_ledger(include_str!(
        "../../../../.allow/artifacts/doc-artifacts.toml"
    ));
    assert!(
        ledger_result.is_ok(),
        "ledger should parse: {:?}",
        ledger_result.err()
    );
    let Ok(ledger) = ledger_result else {
        return;
    };

    assert_eq!(ledger.schema_version, "1.0");
    assert_eq!(ledger.status, SpecSystemMode::Advisory);
    assert!(
        ledger
            .artifact
            .iter()
            .any(|artifact| artifact.id == "CARGO-ALLOW-SPEC-0001"
                && artifact.kind == ArtifactKind::Spec
                && artifact.status == ArtifactStatus::Accepted)
    );
}

#[test]
fn loads_doc_artifacts_from_path() {
    let path = std::env::temp_dir().join(format!(
        "cargo-allow-doc-artifacts-{}-{}.toml",
        std::process::id(),
        unique_stamp()
    ));
    let text = r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-PROP-9999"
            kind = "proposal"
            path = "docs/proposals/CARGO-ALLOW-PROP-9999-example.md"
            status = "accepted"
            owner = "repo-infra"
            created = "2026-06-12"
        "#;
    let write_result = std::fs::write(&path, text);
    assert!(
        write_result.is_ok(),
        "fixture ledger should be written: {:?}",
        write_result.err()
    );

    let loaded = load_doc_artifacts(&path);
    let remove_result = std::fs::remove_file(&path);
    assert!(
        remove_result.is_ok(),
        "fixture ledger should be removed: {:?}",
        remove_result.err()
    );

    assert!(loaded.is_ok(), "ledger should load: {:?}", loaded.err());
    let Ok(ledger) = loaded else {
        return;
    };
    assert_eq!(ledger.artifact.len(), 1);
}

#[test]
fn accepts_minimal_doc_artifacts() {
    let ledger_result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-PROP-0001"
            kind = "proposal"
            path = "docs/proposals/CARGO-ALLOW-PROP-0001-spec-system-profile.md"
            status = "accepted"
            owner = "repo-infra"
            created = "2026-06-12"
        "#,
    );
    assert!(
        ledger_result.is_ok(),
        "minimal doc artifact ledger should parse: {:?}",
        ledger_result.err()
    );
    let Ok(ledger) = ledger_result else {
        return;
    };

    assert_eq!(ledger.artifact.len(), 1);
    assert_eq!(ledger.artifact[0].kind, ArtifactKind::Proposal);
}

#[test]
fn rejects_duplicate_artifact_id() {
    let result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-PROP-0001"
            kind = "proposal"
            path = "docs/proposals/CARGO-ALLOW-PROP-0001-spec-system-profile.md"
            status = "accepted"
            owner = "repo-infra"
            created = "2026-06-12"

            [[artifact]]
            id = "CARGO-ALLOW-PROP-0001"
            kind = "proposal"
            path = "docs/proposals/CARGO-ALLOW-PROP-0001-copy.md"
            status = "accepted"
            owner = "repo-infra"
            created = "2026-06-12"
        "#,
    );

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("duplicate doc artifact id"));
    assert!(err.to_string().contains("CARGO-ALLOW-PROP-0001"));
}

#[test]
fn rejects_duplicate_artifact_id_with_whitespace() {
    let result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-PROP-0001"
            kind = "proposal"
            path = "docs/proposals/CARGO-ALLOW-PROP-0001-spec-system-profile.md"
            status = "accepted"
            owner = "repo-infra"
            created = "2026-06-12"

            [[artifact]]
            id = " CARGO-ALLOW-PROP-0001 "
            kind = "proposal"
            path = "docs/proposals/CARGO-ALLOW-PROP-0001-copy.md"
            status = "accepted"
            owner = "repo-infra"
            created = "2026-06-12"
        "#,
    );

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("doc artifact id"));
    assert!(err.to_string().contains("whitespace"));
}

#[test]
fn rejects_empty_ledger_schema_version() {
    let result = parse_doc_artifact_ledger(
        r#"
            schema_version = ""
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"
        "#,
    );

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("schema_version"));
}

#[test]
fn rejects_empty_ledger_policy() {
    let result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = ""
            owner = "repo-infra"
            status = "advisory"
        "#,
    );

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("policy"));
}

#[test]
fn rejects_empty_ledger_owner() {
    let result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = ""
            status = "advisory"
        "#,
    );

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("owner"));
}

#[test]
fn rejects_empty_artifact_id() {
    let result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = ""
            kind = "proposal"
            path = "docs/proposals/CARGO-ALLOW-PROP-0001-spec-system-profile.md"
            status = "accepted"
            owner = "repo-infra"
            created = "2026-06-12"
        "#,
    );

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("doc artifact id"));
}

#[test]
fn rejects_empty_artifact_created() {
    let result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-PROP-0001"
            kind = "proposal"
            path = "docs/proposals/CARGO-ALLOW-PROP-0001-spec-system-profile.md"
            status = "accepted"
            owner = "repo-infra"
            created = ""
        "#,
    );

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("created"));
}

#[test]
fn rejects_empty_artifact_path() {
    let result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-PROP-0001"
            kind = "proposal"
            path = ""
            status = "accepted"
            owner = "repo-infra"
            created = "2026-06-12"
        "#,
    );

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("CARGO-ALLOW-PROP-0001 path"));
}

#[test]
fn rejects_missing_artifact_path() {
    let result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-PROP-0001"
            kind = "proposal"
            status = "accepted"
            owner = "repo-infra"
            created = "2026-06-12"
        "#,
    );

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("path"));
}

#[test]
fn rejects_empty_artifact_owner() {
    let result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-PROP-0001"
            kind = "proposal"
            path = "docs/proposals/CARGO-ALLOW-PROP-0001-spec-system-profile.md"
            status = "accepted"
            owner = ""
            created = "2026-06-12"
        "#,
    );

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("CARGO-ALLOW-PROP-0001 owner"));
}

#[test]
fn rejects_missing_artifact_owner() {
    let result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-PROP-0001"
            kind = "proposal"
            path = "docs/proposals/CARGO-ALLOW-PROP-0001-spec-system-profile.md"
            status = "accepted"
            created = "2026-06-12"
        "#,
    );

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("owner"));
}

#[test]
fn rejects_unknown_artifact_kind() {
    let result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-RUNBOOK-0001"
            kind = "runbook"
            path = "docs/runbooks/example.md"
            status = "accepted"
            owner = "repo-infra"
            created = "2026-06-12"
        "#,
    );
    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };

    assert!(
        err.to_string()
            .contains("failed to parse doc artifact ledger TOML")
    );
    assert!(err.to_string().contains("runbook"));
}

#[test]
fn rejects_unknown_artifact_status() {
    let result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-SPEC-0001"
            kind = "spec"
            path = "docs/specs/CARGO-ALLOW-SPEC-0001-spec-system-profile.md"
            status = "retired"
            owner = "repo-infra"
            created = "2026-06-12"
        "#,
    );
    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };

    assert!(
        err.to_string()
            .contains("failed to parse doc artifact ledger TOML")
    );
    assert!(err.to_string().contains("retired"));
}

#[test]
fn validates_current_repository_artifact_files() {
    let ledger_result = parse_doc_artifact_ledger(include_str!(
        "../../../../.allow/artifacts/doc-artifacts.toml"
    ));
    assert!(
        ledger_result.is_ok(),
        "ledger should parse: {:?}",
        ledger_result.err()
    );
    let Ok(ledger) = ledger_result else {
        return;
    };

    let result = validate_doc_artifact_files(repo_root(), &ledger, &test_roots());

    assert!(
        result.is_ok(),
        "repo artifacts validate: {:?}",
        result.err()
    );
}

#[test]
fn validates_current_repository_artifact_links() {
    let ledger_result = parse_doc_artifact_ledger(include_str!(
        "../../../../.allow/artifacts/doc-artifacts.toml"
    ));
    assert!(
        ledger_result.is_ok(),
        "ledger should parse: {:?}",
        ledger_result.err()
    );
    let Ok(ledger) = ledger_result else {
        return;
    };

    let result = validate_doc_artifact_links(&ledger);

    assert!(
        result.is_ok(),
        "repo artifact links validate: {:?}",
        result.err()
    );
}

#[test]
fn validates_current_repository_support_tier_claims() {
    let result =
        validate_support_tier_claims(include_str!("../../../../docs/status/SUPPORT_TIERS.md"));

    assert!(
        result.is_ok(),
        "support-tier claims should validate: {:?}",
        result.err()
    );
    let Ok(rows) = result else {
        return;
    };

    assert_eq!(rows.len(), 14);
    for (surface, tier) in [
        (
            "cargo-allow published source-exception ledger",
            SupportTierLevel::Stable,
        ),
        (
            "cargo-allow 0.2 source candidate",
            SupportTierLevel::Stabilizing,
        ),
        ("cargo-intent", SupportTierLevel::Experimental),
        ("cargo-proof", SupportTierLevel::Experimental),
        (
            "Historical spec-system artifacts",
            SupportTierLevel::Compatibility,
        ),
        (
            "current 22-package workspace topology",
            SupportTierLevel::Advisory,
        ),
        (
            "historical 27-package extraction scaffold",
            SupportTierLevel::Compatibility,
        ),
        (
            "physical repository extraction",
            SupportTierLevel::NotIncluded,
        ),
    ] {
        assert!(
            rows.iter()
                .any(|row| row.surface == surface && row.tier == tier),
            "missing support-tier row {surface} = {tier:?}"
        );
    }
}

#[test]
fn accepts_advisory_support_tier_without_proof_command() {
    let result = validate_support_tier_claims(
        r#"
            # Support Tiers

            | Surface | Tier | Claim | Proof command | Notes |
            | --- | --- | --- | --- | --- |
            | Planned profile | Advisory | The repo documents a planned opt-in profile. | | Current behavior is not implemented yet. |
        "#,
    );

    assert!(
        result.is_ok(),
        "advisory rows may omit proof commands: {:?}",
        result.err()
    );
}

#[test]
fn parses_claims_table_after_other_tables() {
    let result = validate_support_tier_claims(
        r#"
            # Support Tiers

            | Tier | Meaning |
            | --- | --- |
            | Stable | Current behavior with proof. |

            | Surface | Tier | Claim | Proof command | Notes |
            | --- | --- | --- | --- | --- |
            | Source exception ledger | Stable | The scanner reports source-tree posture. | cargo-allow check --mode no-new | Claims table appears second. |
        "#,
    );

    assert!(
        result.is_ok(),
        "claims table should be found after vocabulary table: {:?}",
        result.err()
    );
    let Ok(rows) = result else {
        return;
    };
    assert_eq!(rows.len(), 1);
}

#[test]
fn accepts_support_tier_table_with_extra_columns_and_without_notes() {
    let result = validate_support_tier_claims(
        r#"
            # Support Tiers

            | Surface | Tier | Claim | Proof command | Owner |
            | --- | --- | --- | --- | --- |
            | Source exception ledger | Stable | The scanner reports source-tree posture. | cargo-allow check --mode no-new | repo-infra |
        "#,
    );

    assert!(
        result.is_ok(),
        "claims table should allow extra columns: {:?}",
        result.err()
    );
    let Ok(rows) = result else {
        return;
    };
    assert_eq!(rows.len(), 1);
    let first = rows.first();
    assert!(first.is_some());
    let Some(row) = first else {
        return;
    };
    assert_eq!(row.notes, "");
}

#[test]
fn accepts_nonexistent_support_tier_proof_command_text_without_execution() {
    let result = validate_support_tier_claims(
        r#"
            # Support Tiers

            | Surface | Tier | Claim | Proof command | Notes |
            | --- | --- | --- | --- | --- |
            | Fictional proof surface | Stable | A command string is recorded. | definitely-not-a-real-command --flag | Presence-only validation. |
        "#,
    );

    assert!(
        result.is_ok(),
        "proof command strings are not executed or resolved: {:?}",
        result.err()
    );
}

#[test]
fn rejects_stable_support_tier_without_proof_command() {
    let result = validate_support_tier_claims(
        r#"
            # Support Tiers

            | Surface | Tier | Claim | Proof command | Notes |
            | --- | --- | --- | --- | --- |
            | Source exception ledger | Stable | The scanner reports source-tree posture. | | Missing proof. |
        "#,
    );

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("proof command"));
    assert!(err.to_string().contains("must not be empty"));
}

#[test]
fn rejects_stabilizing_support_tier_without_proof_command() {
    let result = validate_support_tier_claims(
        r#"
            # Support Tiers

            | Surface | Tier | Claim | Proof command | Notes |
            | --- | --- | --- | --- | --- |
            | PR posture | Stabilizing | Pull request posture is reported. | | Missing proof. |
        "#,
    );

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("proof command"));
    assert!(err.to_string().contains("must not be empty"));
}

#[test]
fn rejects_support_tier_row_without_claim() {
    let result = validate_support_tier_claims(
        r#"
            # Support Tiers

            | Surface | Tier | Claim | Proof command | Notes |
            | --- | --- | --- | --- | --- |
            | Worklist routing | Advisory | | cargo-allow worklist --format json | Missing claim. |
        "#,
    );

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("support-tier claim"));
}

#[test]
fn rejects_support_tier_row_without_surface() {
    let result = validate_support_tier_claims(
        r#"
            # Support Tiers

            | Surface | Tier | Claim | Proof command | Notes |
            | --- | --- | --- | --- | --- |
            | | Advisory | Worklists exist. | cargo-allow worklist --format json | Missing surface. |
        "#,
    );

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("support-tier surface"));
}

#[test]
fn rejects_support_tier_row_without_tier() {
    let result = validate_support_tier_claims(
        r#"
            # Support Tiers

            | Surface | Tier | Claim | Proof command | Notes |
            | --- | --- | --- | --- | --- |
            | Worklist routing | | Worklists exist. | cargo-allow worklist --format json | Missing tier. |
        "#,
    );

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("support-tier tier"));
}

#[test]
fn rejects_unknown_support_tier_level() {
    let result = validate_support_tier_claims(
        r#"
            # Support Tiers

            | Surface | Tier | Claim | Proof command | Notes |
            | --- | --- | --- | --- | --- |
            | Worklist routing | Unsupported | Worklists exist. | cargo-allow worklist --format json | Unknown tier. |
        "#,
    );

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("unknown support-tier level"));
}

#[test]
fn rejects_missing_support_tier_claims_table() {
    let result = validate_support_tier_claims(
        r#"
            # Support Tiers

            No support-tier claim table here.
        "#,
    );

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("claims table"));
    assert!(err.to_string().contains("not found"));
}

#[test]
fn rejects_support_tier_table_missing_required_column() {
    let result = validate_support_tier_claims(
        r#"
            # Support Tiers

            | Surface | Tier | Claim | Notes |
            | --- | --- | --- | --- |
            | Worklist routing | Advisory | Worklists exist. | Missing proof-command column. |
        "#,
    );

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(
        err.to_string()
            .contains("missing required column Proof command or Proof or evidence")
    );
}

#[test]
fn rejects_support_tier_table_without_claim_rows() {
    let result = validate_support_tier_claims(
        r#"
            # Support Tiers

            | Surface | Tier | Claim | Proof command | Notes |
            | --- | --- | --- | --- | --- |

            ## Claim Boundary
        "#,
    );

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("at least one claim row"));
}

#[test]
fn rejects_support_tier_row_with_wrong_cell_count() {
    let result = validate_support_tier_claims(
        r#"
            # Support Tiers

            | Surface | Tier | Claim | Proof command | Notes |
            | --- | --- | --- | --- | --- |
            | Worklist routing | Advisory | Worklists exist. |
        "#,
    );

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("cells"));
    assert!(err.to_string().contains("expected"));
}

#[test]
fn rejects_support_tier_table_with_invalid_separator() {
    let result = validate_support_tier_claims(
        r#"
            # Support Tiers

            | Surface | Tier | Claim | Proof command | Notes |
            | === | === | === | === | === |
            | Worklist routing | Advisory | Worklists exist. | cargo-allow worklist --format json | Invalid separator. |
        "#,
    );

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("separator row is invalid"));
}

#[test]
fn direct_error_discriminators_match_support_tier_parse_messages() {
    let err = parse_support_tier_claims("No support-tier claim table here.")
        .expect_err("missing claims table should fail parsing");
    assert_eq!(err.kind(), CargoAllowErrorKind::Unknown);

    let err = parse_support_tier_claims(
        r#"
            | Surface | Tier | Claim | Notes |
            | --- | --- | --- | --- |
            | Worklist routing | Advisory | Worklists exist. | Missing proof-command column. |
        "#,
    )
    .expect_err("missing proof-command column should fail parsing");
    assert_eq!(err.kind(), CargoAllowErrorKind::Unknown);

    let err = parse_support_tier_claims("| Surface | Tier | Claim | Proof command | Notes |")
        .expect_err("header without separator should fail parsing");
    assert_eq!(err.kind(), CargoAllowErrorKind::Unknown);

    let err = parse_support_tier_claims(
        r#"
            | Surface | Tier | Claim | Proof command | Notes |
            | === | === | === | === | === |
            | Worklist routing | Advisory | Worklists exist. | cargo-allow worklist --format json | Invalid separator. |
        "#,
    )
    .expect_err("invalid separator should fail parsing");
    assert_eq!(err.kind(), CargoAllowErrorKind::Unknown);

    let err = parse_support_tier_claims(
        r#"
            | Surface | Tier | Claim | Proof command | Notes |
            | --- | --- | --- | --- | --- |
            | Worklist routing | Advisory | Worklists exist. |
        "#,
    )
    .expect_err("short row should fail parsing");
    assert_eq!(err.kind(), CargoAllowErrorKind::Unknown);

    let err = parse_support_tier_claims(
        r#"
            | Surface | Tier | Claim | Proof command | Notes |
            | --- | --- | --- | --- | --- |

            ## Claim Boundary
        "#,
    )
    .expect_err("empty claim rows should fail parsing");
    assert_eq!(err.kind(), CargoAllowErrorKind::Unknown);

    let err = parse_support_tier_claims(
        r#"
            | Surface | Tier | Claim | Proof command | Notes |
            | --- | --- | --- | --- | --- |
            | Worklist routing | Advisory | Claim text | cargo cmd |
        "#,
    )
    .expect_err("missing notes cell should fail parsing");
    assert_eq!(err.kind(), CargoAllowErrorKind::Unknown);

    let err = validate_support_tier_claims(
        r#"
            | Surface | Tier | Claim | Proof command | Notes |
            | --- | --- | --- | --- | --- |
            | Worklist routing | | Worklists exist. | cargo-allow worklist --format json | Missing tier. |
        "#,
    )
    .expect_err("empty tier should fail validation");
    assert_eq!(err.kind(), CargoAllowErrorKind::Unknown);

    let err = validate_support_tier_claims(
        r#"
            | Surface | Tier | Claim | Proof command | Notes |
            | --- | --- | --- | --- | --- |
            | Worklist routing | Unsupported | Worklists exist. | cargo-allow worklist --format json | Unknown tier. |
        "#,
    )
    .expect_err("unknown tier should fail validation");
    assert_eq!(err.kind(), CargoAllowErrorKind::Unknown);

    let err = validate_support_tier_claims(
        r#"
            | Surface | Tier | Claim | Proof command | Notes |
            | --- | --- | --- | --- | --- |
            | | Advisory | Worklists exist. | cargo-allow worklist --format json | Missing surface. |
        "#,
    )
    .expect_err("empty surface should fail validation");
    assert_eq!(err.kind(), CargoAllowErrorKind::Unknown);

    let err = validate_support_tier_claims(
        r#"
            | Surface | Tier | Claim | Proof command | Notes |
            | --- | --- | --- | --- | --- |
            | Worklist routing | Advisory | | cargo-allow worklist --format json | Missing claim. |
        "#,
    )
    .expect_err("empty claim should fail validation");
    assert_eq!(err.kind(), CargoAllowErrorKind::Unknown);

    let err = validate_support_tier_claims(
        r#"
            | Surface | Tier | Claim | Proof command | Notes |
            | --- | --- | --- | --- | --- |
            | Source exception ledger | Stable | The scanner reports source-tree posture. | | Missing proof. |
        "#,
    )
    .expect_err("stable tier without proof should fail validation");
    assert_eq!(err.kind(), CargoAllowErrorKind::Unknown);

    let err = validate_support_tier_claims(
        r#"
            | Surface | Tier | Claim | Proof command | Notes |
            | --- | --- | --- | --- | --- |
            | PR posture | Stabilizing | Pull request posture is reported. | | Missing proof. |
        "#,
    )
    .expect_err("stabilizing tier without proof should fail validation");
    assert_eq!(err.kind(), CargoAllowErrorKind::Unknown);
}

#[test]
fn rejects_accepted_spec_without_proposal_or_standalone_reason() {
    let ledger_result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-SPEC-0001"
            kind = "spec"
            path = "docs/specs/CARGO-ALLOW-SPEC-0001-example.md"
            status = "accepted"
            owner = "repo-infra"
            created = "2026-06-12"
        "#,
    );
    assert!(
        ledger_result.is_ok(),
        "fixture ledger should parse: {:?}",
        ledger_result.err()
    );
    let Ok(ledger) = ledger_result else {
        return;
    };

    let result = validate_doc_artifact_links(&ledger);

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("requires linked_proposal"));
    assert!(err.to_string().contains("standalone_reason"));
}

#[test]
fn accepts_standalone_spec_with_reason() {
    let ledger_result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-SPEC-0001"
            kind = "spec"
            path = "docs/specs/CARGO-ALLOW-SPEC-0001-example.md"
            status = "accepted"
            owner = "repo-infra"
            created = "2026-06-12"
            standalone_reason = "Small internal contract that does not need a proposal."
        "#,
    );
    assert!(
        ledger_result.is_ok(),
        "fixture ledger should parse: {:?}",
        ledger_result.err()
    );
    let Ok(ledger) = ledger_result else {
        return;
    };

    let result = validate_doc_artifact_links(&ledger);

    assert!(
        result.is_ok(),
        "standalone spec should validate: {:?}",
        result.err()
    );
}

#[test]
fn rejects_blank_standalone_reason_for_accepted_spec() {
    let ledger_result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-SPEC-0001"
            kind = "spec"
            path = "docs/specs/CARGO-ALLOW-SPEC-0001-example.md"
            status = "accepted"
            owner = "repo-infra"
            created = "2026-06-12"
            standalone_reason = "  "
        "#,
    );
    assert!(
        ledger_result.is_ok(),
        "fixture ledger should parse: {:?}",
        ledger_result.err()
    );
    let Ok(ledger) = ledger_result else {
        return;
    };

    let result = validate_doc_artifact_links(&ledger);

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("requires linked_proposal"));
}

#[test]
fn rejects_unknown_linked_spec_target() {
    let ledger_result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-SUPPORT-0001"
            kind = "support_tier"
            path = "docs/status/SUPPORT_TIERS.md"
            status = "active"
            owner = "repo-infra"
            created = "2026-06-12"
            linked_spec = "CARGO-ALLOW-SPEC-9999"
        "#,
    );
    assert!(
        ledger_result.is_ok(),
        "fixture ledger should parse: {:?}",
        ledger_result.err()
    );
    let Ok(ledger) = ledger_result else {
        return;
    };

    let result = validate_doc_artifact_links(&ledger);

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("linked_spec target"));
    assert!(err.to_string().contains("is not registered"));
}

#[test]
fn rejects_linked_proposal_kind_mismatch() {
    let ledger_result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-SPEC-0001"
            kind = "spec"
            path = "docs/specs/CARGO-ALLOW-SPEC-0001-example.md"
            status = "accepted"
            owner = "repo-infra"
            created = "2026-06-12"
            linked_proposal = "CARGO-ALLOW-SPEC-0002"

            [[artifact]]
            id = "CARGO-ALLOW-SPEC-0002"
            kind = "spec"
            path = "docs/specs/CARGO-ALLOW-SPEC-0002-target.md"
            status = "draft"
            owner = "repo-infra"
            created = "2026-06-12"
        "#,
    );
    assert!(
        ledger_result.is_ok(),
        "fixture ledger should parse: {:?}",
        ledger_result.err()
    );
    let Ok(ledger) = ledger_result else {
        return;
    };

    let result = validate_doc_artifact_links(&ledger);

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("linked_proposal target"));
    assert!(err.to_string().contains("expected Proposal"));
}

#[test]
fn accepts_active_goal_linked_plan_by_path() {
    let ledger_result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-PROP-0001"
            kind = "proposal"
            path = "docs/proposals/CARGO-ALLOW-PROP-0001-example.md"
            status = "accepted"
            owner = "repo-infra"
            created = "2026-06-12"

            [[artifact]]
            id = "CARGO-ALLOW-SPEC-0001"
            kind = "spec"
            path = "docs/specs/CARGO-ALLOW-SPEC-0001-example.md"
            status = "accepted"
            owner = "repo-infra"
            created = "2026-06-12"
            linked_proposal = "CARGO-ALLOW-PROP-0001"

            [[artifact]]
            id = "CARGO-ALLOW-PLAN-0001"
            kind = "implementation_plan"
            path = "plans/spec-system/implementation-plan.md"
            status = "active"
            owner = "repo-infra"
            created = "2026-06-12"
            linked_spec = "CARGO-ALLOW-SPEC-0001"

            [[artifact]]
            id = "CARGO-ALLOW-GOAL-0001"
            kind = "active_goal"
            path = ".allow/goals/active.toml"
            status = "active"
            owner = "codex"
            created = "2026-06-12"
            linked_proposal = "CARGO-ALLOW-PROP-0001"
            linked_spec = "CARGO-ALLOW-SPEC-0001"
            linked_plan = "plans/spec-system/implementation-plan.md"
        "#,
    );
    assert!(
        ledger_result.is_ok(),
        "fixture ledger should parse: {:?}",
        ledger_result.err()
    );
    let Ok(ledger) = ledger_result else {
        return;
    };

    let result = validate_doc_artifact_links(&ledger);

    assert!(
        result.is_ok(),
        "active goal should resolve plan by path: {:?}",
        result.err()
    );
}

#[test]
fn rejects_active_goal_unknown_plan() {
    let ledger_result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-PROP-0001"
            kind = "proposal"
            path = "docs/proposals/CARGO-ALLOW-PROP-0001-example.md"
            status = "accepted"
            owner = "repo-infra"
            created = "2026-06-12"

            [[artifact]]
            id = "CARGO-ALLOW-SPEC-0001"
            kind = "spec"
            path = "docs/specs/CARGO-ALLOW-SPEC-0001-example.md"
            status = "accepted"
            owner = "repo-infra"
            created = "2026-06-12"
            linked_proposal = "CARGO-ALLOW-PROP-0001"

            [[artifact]]
            id = "CARGO-ALLOW-GOAL-0001"
            kind = "active_goal"
            path = ".allow/goals/active.toml"
            status = "active"
            owner = "codex"
            created = "2026-06-12"
            linked_proposal = "CARGO-ALLOW-PROP-0001"
            linked_spec = "CARGO-ALLOW-SPEC-0001"
            linked_plan = "plans/spec-system/missing.md"
        "#,
    );
    assert!(
        ledger_result.is_ok(),
        "fixture ledger should parse: {:?}",
        ledger_result.err()
    );
    let Ok(ledger) = ledger_result else {
        return;
    };

    let result = validate_doc_artifact_links(&ledger);

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("linked_plan target"));
    assert!(err.to_string().contains("not registered by id or path"));
}

#[test]
fn parses_current_repository_historical_goal_manifest() {
    let ledger_result = parse_doc_artifact_ledger(include_str!(
        "../../../../.allow/artifacts/doc-artifacts.toml"
    ));
    assert!(
        ledger_result.is_ok(),
        "ledger should parse: {:?}",
        ledger_result.err()
    );
    let Ok(ledger) = ledger_result else {
        return;
    };

    let result = validate_active_goal_manifest_text(
        include_str!(
            "../../../../.allow/goals/archive/CARGO-ALLOW-GOAL-0004-core-exception-ledger.toml"
        ),
        &ledger,
    );

    assert!(
        result.is_ok(),
        "active goal manifest should validate: {:?}",
        result.err()
    );
}

#[test]
fn validates_active_goal_toml_links_and_work_items() {
    let ledger = active_goal_manifest_test_ledger();
    let result = validate_active_goal_manifest_text(valid_active_goal_manifest_toml(), &ledger);

    assert!(
        result.is_ok(),
        "active goal TOML should validate: {:?}",
        result.err()
    );
}

#[test]
fn rejects_active_goal_toml_unknown_linked_plan() {
    let ledger = active_goal_manifest_test_ledger();
    let manifest = valid_active_goal_manifest_toml().replace(
        "plans/spec-system/implementation-plan.md",
        "plans/spec-system/missing.md",
    );

    let result = validate_active_goal_manifest_text(&manifest, &ledger);

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("active goal linked_plan target"));
    assert!(err.to_string().contains("not registered by id or path"));
}

#[test]
fn rejects_active_goal_ready_work_item_without_proof_commands() {
    let ledger = active_goal_manifest_test_ledger();
    let manifest = valid_active_goal_manifest_toml().replace(
        r#"proof_commands = [
  "cargo-allow check --profile spec-system --mode audit",
]"#,
        "proof_commands = []",
    );

    let result = validate_active_goal_manifest_text(&manifest, &ledger);

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("requires proof_commands"));
}

#[test]
fn rejects_active_goal_done_work_item_without_closeout() {
    let ledger = active_goal_manifest_test_ledger();
    let manifest =
        valid_active_goal_manifest_toml().replace("status = \"ready\"", "status = \"done\"");

    let result = validate_active_goal_manifest_text(&manifest, &ledger);

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("linked_closeout"));
}

#[test]
fn rejects_active_goal_ready_work_item_unknown_closeout() {
    let ledger = active_goal_manifest_test_ledger();
    let manifest = valid_active_goal_manifest_toml().replace(
        "linked_plan = \"plans/spec-system/implementation-plan.md\"\nproof_commands",
        "linked_plan = \"plans/spec-system/implementation-plan.md\"\nlinked_closeout = \"CARGO-ALLOW-CLOSEOUT-MISSING\"\nproof_commands",
    );

    let result = validate_active_goal_manifest_text(&manifest, &ledger);

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("linked_closeout target"));
    assert!(err.to_string().contains("not registered by id or path"));
}

#[test]
fn rejects_active_goal_missing_required_spec_link() {
    let ledger_result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-PROP-0001"
            kind = "proposal"
            path = "docs/proposals/CARGO-ALLOW-PROP-0001-example.md"
            status = "accepted"
            owner = "repo-infra"
            created = "2026-06-12"

            [[artifact]]
            id = "CARGO-ALLOW-PLAN-0001"
            kind = "implementation_plan"
            path = "plans/spec-system/implementation-plan.md"
            status = "active"
            owner = "repo-infra"
            created = "2026-06-12"
            linked_proposal = "CARGO-ALLOW-PROP-0001"

            [[artifact]]
            id = "CARGO-ALLOW-GOAL-0001"
            kind = "active_goal"
            path = ".allow/goals/active.toml"
            status = "active"
            owner = "codex"
            created = "2026-06-12"
            linked_proposal = "CARGO-ALLOW-PROP-0001"
            linked_plan = "CARGO-ALLOW-PLAN-0001"
        "#,
    );
    assert!(
        ledger_result.is_ok(),
        "fixture ledger should parse: {:?}",
        ledger_result.err()
    );
    let Ok(ledger) = ledger_result else {
        return;
    };

    let result = validate_doc_artifact_links(&ledger);

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("requires linked_spec"));
}

#[test]
fn rejects_active_plan_without_proposal_or_spec() {
    let ledger_result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-PLAN-0001"
            kind = "implementation_plan"
            path = "plans/spec-system/implementation-plan.md"
            status = "active"
            owner = "repo-infra"
            created = "2026-06-12"
        "#,
    );
    assert!(
        ledger_result.is_ok(),
        "fixture ledger should parse: {:?}",
        ledger_result.err()
    );
    let Ok(ledger) = ledger_result else {
        return;
    };

    let result = validate_doc_artifact_links(&ledger);

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(
        err.to_string()
            .contains("active plan requires linked_proposal or linked_spec")
    );
}

#[test]
fn rejects_accepted_adr_without_spec_or_standalone_reason() {
    let ledger_result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-ADR-0001"
            kind = "adr"
            path = "docs/adr/CARGO-ALLOW-ADR-0001-example.md"
            status = "accepted"
            owner = "repo-infra"
            created = "2026-06-12"
        "#,
    );
    assert!(
        ledger_result.is_ok(),
        "fixture ledger should parse: {:?}",
        ledger_result.err()
    );
    let Ok(ledger) = ledger_result else {
        return;
    };

    let result = validate_doc_artifact_links(&ledger);

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("requires linked_spec"));
    assert!(err.to_string().contains("standalone_reason"));
}

#[test]
fn accepts_standalone_adr_with_reason() {
    let ledger_result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-ADR-0001"
            kind = "adr"
            path = "docs/adr/CARGO-ALLOW-ADR-0001-example.md"
            status = "accepted"
            owner = "repo-infra"
            created = "2026-06-12"
            standalone_reason = "Repository-wide architecture decision."
        "#,
    );
    assert!(
        ledger_result.is_ok(),
        "fixture ledger should parse: {:?}",
        ledger_result.err()
    );
    let Ok(ledger) = ledger_result else {
        return;
    };

    let result = validate_doc_artifact_links(&ledger);

    assert!(
        result.is_ok(),
        "standalone ADR should validate: {:?}",
        result.err()
    );
}

#[test]
fn rejects_closeout_without_linked_plan() {
    let ledger_result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-CLOSEOUT-0001"
            kind = "closeout"
            path = "plans/spec-system/closeout.md"
            status = "done"
            owner = "repo-infra"
            created = "2026-06-12"
        "#,
    );
    assert!(
        ledger_result.is_ok(),
        "fixture ledger should parse: {:?}",
        ledger_result.err()
    );
    let Ok(ledger) = ledger_result else {
        return;
    };

    let result = validate_doc_artifact_links(&ledger);

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("requires linked_plan"));
}

#[test]
fn accepts_closeout_linked_plan_by_id() {
    let ledger_result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-PROP-0001"
            kind = "proposal"
            path = "docs/proposals/CARGO-ALLOW-PROP-0001-example.md"
            status = "accepted"
            owner = "repo-infra"
            created = "2026-06-12"

            [[artifact]]
            id = "CARGO-ALLOW-PLAN-0001"
            kind = "implementation_plan"
            path = "plans/spec-system/implementation-plan.md"
            status = "active"
            owner = "repo-infra"
            created = "2026-06-12"
            linked_proposal = "CARGO-ALLOW-PROP-0001"

            [[artifact]]
            id = "CARGO-ALLOW-CLOSEOUT-0001"
            kind = "closeout"
            path = "plans/spec-system/closeout.md"
            status = "done"
            owner = "repo-infra"
            created = "2026-06-12"
            linked_plan = "CARGO-ALLOW-PLAN-0001"
        "#,
    );
    assert!(
        ledger_result.is_ok(),
        "fixture ledger should parse: {:?}",
        ledger_result.err()
    );
    let Ok(ledger) = ledger_result else {
        return;
    };

    let result = validate_doc_artifact_links(&ledger);

    assert!(
        result.is_ok(),
        "closeout should resolve plan by id: {:?}",
        result.err()
    );
}

#[test]
fn rejects_replaces_unknown_target() {
    let ledger_result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-SPEC-0002"
            kind = "spec"
            path = "docs/specs/CARGO-ALLOW-SPEC-0002-example.md"
            status = "draft"
            owner = "repo-infra"
            created = "2026-06-12"
            replaces = "CARGO-ALLOW-SPEC-0001"
        "#,
    );
    assert!(
        ledger_result.is_ok(),
        "fixture ledger should parse: {:?}",
        ledger_result.err()
    );
    let Ok(ledger) = ledger_result else {
        return;
    };

    let result = validate_doc_artifact_links(&ledger);

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("replaces target"));
    assert!(err.to_string().contains("is not registered"));
}

#[test]
fn rejects_lifecycle_self_reference() {
    let ledger_result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-SPEC-0001"
            kind = "spec"
            path = "docs/specs/CARGO-ALLOW-SPEC-0001-example.md"
            status = "draft"
            owner = "repo-infra"
            created = "2026-06-12"
            replaces = "CARGO-ALLOW-SPEC-0001"
        "#,
    );
    assert!(
        ledger_result.is_ok(),
        "fixture ledger should parse: {:?}",
        ledger_result.err()
    );
    let Ok(ledger) = ledger_result else {
        return;
    };

    let result = validate_doc_artifact_links(&ledger);

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("must not reference itself"));
}

#[test]
fn rejects_lifecycle_wrong_kind_target() {
    let ledger_result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-PROP-0001"
            kind = "proposal"
            path = "docs/proposals/CARGO-ALLOW-PROP-0001-example.md"
            status = "accepted"
            owner = "repo-infra"
            created = "2026-06-12"

            [[artifact]]
            id = "CARGO-ALLOW-SPEC-0001"
            kind = "spec"
            path = "docs/specs/CARGO-ALLOW-SPEC-0001-example.md"
            status = "draft"
            owner = "repo-infra"
            created = "2026-06-12"
            supersedes = "CARGO-ALLOW-PROP-0001"
        "#,
    );
    assert!(
        ledger_result.is_ok(),
        "fixture ledger should parse: {:?}",
        ledger_result.err()
    );
    let Ok(ledger) = ledger_result else {
        return;
    };

    let result = validate_doc_artifact_links(&ledger);

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("supersedes target"));
    assert!(err.to_string().contains("expected Spec"));
}

#[test]
fn rejects_empty_link_field() {
    let ledger_result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-SUPPORT-0001"
            kind = "support_tier"
            path = "docs/status/SUPPORT_TIERS.md"
            status = "active"
            owner = "repo-infra"
            created = "2026-06-12"
            linked_spec = ""
        "#,
    );
    assert!(
        ledger_result.is_ok(),
        "fixture ledger should parse: {:?}",
        ledger_result.err()
    );
    let Ok(ledger) = ledger_result else {
        return;
    };

    let result = validate_doc_artifact_links(&ledger);

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("linked_spec must not be empty"));
}

#[test]
fn rejects_duplicate_artifact_path_for_graph_links() {
    let ledger_result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-PROP-0001"
            kind = "proposal"
            path = "docs/proposals/CARGO-ALLOW-DUPLICATE.md"
            status = "draft"
            owner = "repo-infra"
            created = "2026-06-12"

            [[artifact]]
            id = "CARGO-ALLOW-PROP-0002"
            kind = "proposal"
            path = "docs/proposals/CARGO-ALLOW-DUPLICATE.md"
            status = "draft"
            owner = "repo-infra"
            created = "2026-06-12"
        "#,
    );
    assert!(
        ledger_result.is_ok(),
        "fixture ledger should parse: {:?}",
        ledger_result.err()
    );
    let Ok(ledger) = ledger_result else {
        return;
    };

    let result = validate_doc_artifact_links(&ledger);

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("duplicate doc artifact path"));
}

#[test]
fn rejects_missing_artifact_file() {
    let root = temp_root("missing-file");
    let ledger_result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-PROP-0001"
            kind = "proposal"
            path = "docs/proposals/CARGO-ALLOW-PROP-0001-missing.md"
            status = "accepted"
            owner = "repo-infra"
            created = "2026-06-12"
        "#,
    );
    assert!(
        ledger_result.is_ok(),
        "fixture ledger should parse: {:?}",
        ledger_result.err()
    );
    let Ok(ledger) = ledger_result else {
        return;
    };

    let result = validate_doc_artifact_files(&root, &ledger, &test_roots());
    let _ = std::fs::remove_dir_all(&root);

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("artifact file missing"));
}

#[test]
fn rejects_id_missing_from_file() {
    let root = temp_root("missing-id");
    write_file(
        &root,
        "docs/proposals/CARGO-ALLOW-PROP-0001-missing-id.md",
        "# Proposal\n\nNo visible artifact ID here.\n",
    );
    let ledger_result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-PROP-0001"
            kind = "proposal"
            path = "docs/proposals/CARGO-ALLOW-PROP-0001-missing-id.md"
            status = "accepted"
            owner = "repo-infra"
            created = "2026-06-12"
        "#,
    );
    assert!(
        ledger_result.is_ok(),
        "fixture ledger should parse: {:?}",
        ledger_result.err()
    );
    let Ok(ledger) = ledger_result else {
        return;
    };

    let result = validate_doc_artifact_files(&root, &ledger, &test_roots());
    let _ = std::fs::remove_dir_all(&root);

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("not found in artifact file"));
}

#[test]
fn rejects_kind_path_mismatch() {
    let root = temp_root("kind-path-mismatch");
    write_file(
        &root,
        "docs/proposals/CARGO-ALLOW-SPEC-0001-wrong-root.md",
        "CARGO-ALLOW-SPEC-0001\n",
    );
    let ledger_result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-SPEC-0001"
            kind = "spec"
            path = "docs/proposals/CARGO-ALLOW-SPEC-0001-wrong-root.md"
            status = "accepted"
            owner = "repo-infra"
            created = "2026-06-12"
        "#,
    );
    assert!(
        ledger_result.is_ok(),
        "fixture ledger should parse: {:?}",
        ledger_result.err()
    );
    let Ok(ledger) = ledger_result else {
        return;
    };

    let result = validate_doc_artifact_files(&root, &ledger, &test_roots());
    let _ = std::fs::remove_dir_all(&root);

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("does not match artifact path"));
}

#[test]
fn rejects_artifact_path_with_parent_segment() {
    let root = temp_root("parent-path");
    let ledger_result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-SPEC-0001"
            kind = "spec"
            path = "docs/specs/../CARGO-ALLOW-SPEC-0001.md"
            status = "accepted"
            owner = "repo-infra"
            created = "2026-06-12"
        "#,
    );
    assert!(
        ledger_result.is_ok(),
        "fixture ledger should parse: {:?}",
        ledger_result.err()
    );
    let Ok(ledger) = ledger_result else {
        return;
    };

    let result = validate_doc_artifact_files(&root, &ledger, &test_roots());
    let _ = std::fs::remove_dir_all(&root);

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("must stay under the source tree"));
}

#[test]
fn rejects_absolute_artifact_path() {
    let root = temp_root("absolute-path");
    let absolute = root
        .join("docs/specs/CARGO-ALLOW-SPEC-0001.md")
        .to_string_lossy()
        .replace('\\', "\\\\");
    let ledger_text = format!(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-SPEC-0001"
            kind = "spec"
            path = "{absolute}"
            status = "accepted"
            owner = "repo-infra"
            created = "2026-06-12"
        "#
    );
    let ledger_result = parse_doc_artifact_ledger(&ledger_text);
    assert!(
        ledger_result.is_ok(),
        "fixture ledger should parse: {:?}",
        ledger_result.err()
    );
    let Ok(ledger) = ledger_result else {
        return;
    };

    let result = validate_doc_artifact_files(&root, &ledger, &test_roots());
    let _ = std::fs::remove_dir_all(&root);

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("must be relative"));
}

#[test]
fn rejects_directory_at_artifact_path() {
    let root = temp_root("directory-path");
    let directory_result =
        std::fs::create_dir_all(root.join("docs/specs/CARGO-ALLOW-SPEC-0001.md"));
    assert!(
        directory_result.is_ok(),
        "artifact directory should be created: {:?}",
        directory_result.err()
    );
    let ledger_result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-SPEC-0001"
            kind = "spec"
            path = "docs/specs/CARGO-ALLOW-SPEC-0001.md"
            status = "accepted"
            owner = "repo-infra"
            created = "2026-06-12"
        "#,
    );
    assert!(
        ledger_result.is_ok(),
        "fixture ledger should parse: {:?}",
        ledger_result.err()
    );
    let Ok(ledger) = ledger_result else {
        return;
    };

    let result = validate_doc_artifact_files(&root, &ledger, &test_roots());
    let _ = std::fs::remove_dir_all(&root);

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("artifact file missing"));
}

#[test]
fn rejects_artifact_id_substring_match() {
    let root = temp_root("id-substring");
    write_file(
        &root,
        "docs/specs/CARGO-ALLOW-SPEC-0001.md",
        "CARGO-ALLOW-SPEC-00010\n",
    );
    let ledger_result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-SPEC-0001"
            kind = "spec"
            path = "docs/specs/CARGO-ALLOW-SPEC-0001.md"
            status = "accepted"
            owner = "repo-infra"
            created = "2026-06-12"
        "#,
    );
    assert!(
        ledger_result.is_ok(),
        "fixture ledger should parse: {:?}",
        ledger_result.err()
    );
    let Ok(ledger) = ledger_result else {
        return;
    };

    let result = validate_doc_artifact_files(&root, &ledger, &test_roots());
    let _ = std::fs::remove_dir_all(&root);

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("not found in artifact file"));
}

#[test]
fn rejects_support_tier_path_mismatch() {
    let root = temp_root("support-tier-mismatch");
    write_file(
        &root,
        "docs/status/CARGO-ALLOW-SUPPORT-0001.md",
        "CARGO-ALLOW-SUPPORT-0001\n",
    );
    let ledger_result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-SUPPORT-0001"
            kind = "support_tier"
            path = "docs/status/CARGO-ALLOW-SUPPORT-0001.md"
            status = "active"
            owner = "repo-infra"
            created = "2026-06-12"
        "#,
    );
    assert!(
        ledger_result.is_ok(),
        "fixture ledger should parse: {:?}",
        ledger_result.err()
    );
    let Ok(ledger) = ledger_result else {
        return;
    };

    let result = validate_doc_artifact_files(&root, &ledger, &test_roots());
    let _ = std::fs::remove_dir_all(&root);

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("does not match artifact path"));
}

#[test]
fn rejects_active_goal_wrong_root() {
    let root = temp_root("goal-wrong-root");
    write_file(
        &root,
        "plans/CARGO-ALLOW-GOAL-0001.toml",
        "CARGO-ALLOW-GOAL-0001\n",
    );
    let ledger_result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-GOAL-0001"
            kind = "active_goal"
            path = "plans/CARGO-ALLOW-GOAL-0001.toml"
            status = "active"
            owner = "codex"
            created = "2026-06-12"
        "#,
    );
    assert!(
        ledger_result.is_ok(),
        "fixture ledger should parse: {:?}",
        ledger_result.err()
    );
    let Ok(ledger) = ledger_result else {
        return;
    };

    let result = validate_doc_artifact_files(&root, &ledger, &test_roots());
    let _ = std::fs::remove_dir_all(&root);

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("does not match artifact path"));
}

#[test]
fn accepts_policy_ledger_under_policy_root() {
    let root = temp_root("policy-ledger-root");
    write_file(
        &root,
        ".allow/profiles/spec-system.toml",
        "CARGO-ALLOW-POLICY-0001\n",
    );
    let ledger_result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-POLICY-0001"
            kind = "policy_ledger"
            path = ".allow/profiles/spec-system.toml"
            status = "draft"
            owner = "repo-infra"
            created = "2026-06-12"
        "#,
    );
    assert!(
        ledger_result.is_ok(),
        "fixture ledger should parse: {:?}",
        ledger_result.err()
    );
    let Ok(ledger) = ledger_result else {
        return;
    };

    let result = validate_doc_artifact_files(&root, &ledger, &test_roots());
    let _ = std::fs::remove_dir_all(&root);

    assert!(
        result.is_ok(),
        "policy ledger should validate: {:?}",
        result.err()
    );
}

#[test]
fn accepts_superseded_with_replacement() {
    let root = temp_root("superseded-ok");
    write_file(
        &root,
        "docs/specs/CARGO-ALLOW-SPEC-0001-old.md",
        "CARGO-ALLOW-SPEC-0001\n",
    );
    write_file(
        &root,
        "docs/specs/CARGO-ALLOW-SPEC-0002-new.md",
        "CARGO-ALLOW-SPEC-0002\n",
    );
    let ledger_result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-SPEC-0001"
            kind = "spec"
            path = "docs/specs/CARGO-ALLOW-SPEC-0001-old.md"
            status = "superseded"
            owner = "repo-infra"
            created = "2026-06-12"
            superseded_by = "CARGO-ALLOW-SPEC-0002"

            [[artifact]]
            id = "CARGO-ALLOW-SPEC-0002"
            kind = "spec"
            path = "docs/specs/CARGO-ALLOW-SPEC-0002-new.md"
            status = "accepted"
            owner = "repo-infra"
            created = "2026-06-12"
        "#,
    );
    assert!(
        ledger_result.is_ok(),
        "fixture ledger should parse: {:?}",
        ledger_result.err()
    );
    let Ok(ledger) = ledger_result else {
        return;
    };

    let result = validate_doc_artifact_files(&root, &ledger, &test_roots());
    let _ = std::fs::remove_dir_all(&root);

    assert!(
        result.is_ok(),
        "superseded replacement should validate: {:?}",
        result.err()
    );
}

#[test]
fn rejects_superseded_missing_replacement() {
    let root = temp_root("superseded-missing");
    write_file(
        &root,
        "docs/specs/CARGO-ALLOW-SPEC-0001-old.md",
        "CARGO-ALLOW-SPEC-0001\n",
    );
    let ledger_result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-SPEC-0001"
            kind = "spec"
            path = "docs/specs/CARGO-ALLOW-SPEC-0001-old.md"
            status = "superseded"
            owner = "repo-infra"
            created = "2026-06-12"
            superseded_by = "CARGO-ALLOW-SPEC-9999"
        "#,
    );
    assert!(
        ledger_result.is_ok(),
        "fixture ledger should parse: {:?}",
        ledger_result.err()
    );
    let Ok(ledger) = ledger_result else {
        return;
    };

    let result = validate_doc_artifact_files(&root, &ledger, &test_roots());
    let _ = std::fs::remove_dir_all(&root);

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("superseded_by target"));
}

#[test]
fn rejects_superseded_without_replacement() {
    let root = temp_root("superseded-none");
    write_file(
        &root,
        "docs/specs/CARGO-ALLOW-SPEC-0001-old.md",
        "CARGO-ALLOW-SPEC-0001\n",
    );
    let ledger_result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-SPEC-0001"
            kind = "spec"
            path = "docs/specs/CARGO-ALLOW-SPEC-0001-old.md"
            status = "superseded"
            owner = "repo-infra"
            created = "2026-06-12"
        "#,
    );
    assert!(
        ledger_result.is_ok(),
        "fixture ledger should parse: {:?}",
        ledger_result.err()
    );
    let Ok(ledger) = ledger_result else {
        return;
    };

    let result = validate_doc_artifact_files(&root, &ledger, &test_roots());
    let _ = std::fs::remove_dir_all(&root);

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("requires superseded_by"));
}

#[test]
fn rejects_superseded_self_replacement() {
    let root = temp_root("superseded-self");
    write_file(
        &root,
        "docs/specs/CARGO-ALLOW-SPEC-0001-old.md",
        "CARGO-ALLOW-SPEC-0001\n",
    );
    let ledger_result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-SPEC-0001"
            kind = "spec"
            path = "docs/specs/CARGO-ALLOW-SPEC-0001-old.md"
            status = "superseded"
            owner = "repo-infra"
            created = "2026-06-12"
            superseded_by = "CARGO-ALLOW-SPEC-0001"
        "#,
    );
    assert!(
        ledger_result.is_ok(),
        "fixture ledger should parse: {:?}",
        ledger_result.err()
    );
    let Ok(ledger) = ledger_result else {
        return;
    };

    let result = validate_doc_artifact_files(&root, &ledger, &test_roots());
    let _ = std::fs::remove_dir_all(&root);

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("must not supersede itself"));
}

#[test]
fn rejects_superseded_replacement_that_is_also_superseded() {
    let root = temp_root("superseded-chain");
    write_file(
        &root,
        "docs/specs/CARGO-ALLOW-SPEC-0001-old.md",
        "CARGO-ALLOW-SPEC-0001\n",
    );
    write_file(
        &root,
        "docs/specs/CARGO-ALLOW-SPEC-0002-old.md",
        "CARGO-ALLOW-SPEC-0002\n",
    );
    let ledger_result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-SPEC-0001"
            kind = "spec"
            path = "docs/specs/CARGO-ALLOW-SPEC-0001-old.md"
            status = "superseded"
            owner = "repo-infra"
            created = "2026-06-12"
            superseded_by = "CARGO-ALLOW-SPEC-0002"

            [[artifact]]
            id = "CARGO-ALLOW-SPEC-0002"
            kind = "spec"
            path = "docs/specs/CARGO-ALLOW-SPEC-0002-old.md"
            status = "superseded"
            owner = "repo-infra"
            created = "2026-06-12"
            superseded_by = "CARGO-ALLOW-SPEC-0003"
        "#,
    );
    assert!(
        ledger_result.is_ok(),
        "fixture ledger should parse: {:?}",
        ledger_result.err()
    );
    let Ok(ledger) = ledger_result else {
        return;
    };

    let result = validate_doc_artifact_files(&root, &ledger, &test_roots());
    let _ = std::fs::remove_dir_all(&root);

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("is also superseded"));
}

fn unique_stamp() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos())
}

fn active_goal_manifest_test_ledger() -> DocArtifactLedger {
    let ledger_result = parse_doc_artifact_ledger(
        r#"
            schema_version = "1.0"
            policy = "cargo-allow-doc-artifacts"
            owner = "repo-infra"
            status = "advisory"

            [[artifact]]
            id = "CARGO-ALLOW-PROP-0001"
            kind = "proposal"
            path = "docs/proposals/CARGO-ALLOW-PROP-0001-example.md"
            status = "accepted"
            owner = "repo-infra"
            created = "2026-06-12"

            [[artifact]]
            id = "CARGO-ALLOW-SPEC-0001"
            kind = "spec"
            path = "docs/specs/CARGO-ALLOW-SPEC-0001-example.md"
            status = "accepted"
            owner = "repo-infra"
            created = "2026-06-12"
            linked_proposal = "CARGO-ALLOW-PROP-0001"

            [[artifact]]
            id = "CARGO-ALLOW-SUPPORT-0001"
            kind = "support_tier"
            path = "docs/status/SUPPORT_TIERS.md"
            status = "active"
            owner = "repo-infra"
            created = "2026-06-12"
            linked_proposal = "CARGO-ALLOW-PROP-0001"
            linked_spec = "CARGO-ALLOW-SPEC-0001"

            [[artifact]]
            id = "CARGO-ALLOW-GOAL-0001"
            kind = "active_goal"
            path = ".allow/goals/active.toml"
            status = "active"
            owner = "codex"
            created = "2026-06-12"
            linked_proposal = "CARGO-ALLOW-PROP-0001"
            linked_spec = "CARGO-ALLOW-SPEC-0001"
            linked_support_tier = "CARGO-ALLOW-SUPPORT-0001"
            linked_plan = "plans/spec-system/implementation-plan.md"

            [[artifact]]
            id = "CARGO-ALLOW-PLAN-0001"
            kind = "implementation_plan"
            path = "plans/spec-system/implementation-plan.md"
            status = "active"
            owner = "repo-infra"
            created = "2026-06-12"
            linked_proposal = "CARGO-ALLOW-PROP-0001"
            linked_spec = "CARGO-ALLOW-SPEC-0001"
            linked_support_tier = "CARGO-ALLOW-SUPPORT-0001"
            linked_goal = "CARGO-ALLOW-GOAL-0001"

            [[artifact]]
            id = "CARGO-ALLOW-CLOSEOUT-0001"
            kind = "closeout"
            path = "plans/spec-system/closeout.md"
            status = "draft"
            owner = "repo-infra"
            created = "2026-06-12"
            linked_plan = "CARGO-ALLOW-PLAN-0001"
        "#,
    );
    assert!(
        ledger_result.is_ok(),
        "active goal fixture ledger should parse: {:?}",
        ledger_result.err()
    );
    let Ok(ledger) = ledger_result else {
        return DocArtifactLedger {
            schema_version: String::new(),
            policy: String::new(),
            owner: String::new(),
            status: SpecSystemMode::Advisory,
            artifact: Vec::new(),
        };
    };
    ledger
}

fn valid_active_goal_manifest_toml() -> &'static str {
    r#"
schema_version = "1.0"
id = "CARGO-ALLOW-GOAL-0001"
title = "Spec-system profile"
status = "active"
owner = "codex"
created = "2026-06-12"
linked_proposal = "CARGO-ALLOW-PROP-0001"
linked_spec = "CARGO-ALLOW-SPEC-0001"
linked_support_tier = "CARGO-ALLOW-SUPPORT-0001"
linked_plan = "plans/spec-system/implementation-plan.md"
linked_plan_status = "active"

[[work_item]]
id = "spec-system-pr-001"
status = "ready"
title = "Keep graph valid"
linked_spec = "CARGO-ALLOW-SPEC-0001"
linked_plan = "plans/spec-system/implementation-plan.md"
proof_commands = [
  "cargo-allow check --profile spec-system --mode audit",
]
"#
}

fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn temp_root(label: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-spec-system-{label}-{}-{}",
        std::process::id(),
        unique_stamp()
    ));
    let create_result = std::fs::create_dir_all(&root);
    assert!(
        create_result.is_ok(),
        "temp root should be created: {:?}",
        create_result.err()
    );
    root
}

fn write_file(root: &std::path::Path, relative_path: &str, text: &str) {
    let path = root.join(relative_path);
    if let Some(parent) = path.parent() {
        let create_result = std::fs::create_dir_all(parent);
        assert!(
            create_result.is_ok(),
            "artifact parent should be created: {:?}",
            create_result.err()
        );
    }
    let write_result = std::fs::write(&path, text);
    assert!(
        write_result.is_ok(),
        "artifact file should be written: {:?}",
        write_result.err()
    );
}

fn test_roots() -> SpecSystemRoots {
    SpecSystemRoots {
        proposals: "docs/proposals".to_string(),
        specs: "docs/specs".to_string(),
        adrs: "docs/adr".to_string(),
        plans: "plans".to_string(),
        goals: Some(".allow/goals".to_string()),
        support_tiers: "docs/status/SUPPORT_TIERS.md".to_string(),
        artifact_ledger: ".allow/artifacts/doc-artifacts.toml".to_string(),
    }
}

#[derive(serde::Deserialize)]
struct ThreeProductCollapseRow {
    target_module: String,
    disposition: String,
}

#[derive(serde::Deserialize)]
struct ThreeProductDispositionMap {
    schema_version: String,
    authority_generation: u32,
    design_package_proposal: String,
    ownership_adr: String,
    package_identity_adr: String,
    historical_spec: String,
    current_spec: String,
    design_package_plan: String,
    historical_scaffold_package_count: usize,
    current_package_count: usize,
    retained_package_count: usize,
    repository_extraction_authorized: bool,
    release_authorized: bool,
    collapse: Vec<ThreeProductCollapseRow>,
}

#[test]
fn spec_system_design_package() -> Result<(), String> {
    let root = repo_root();
    let disposition_map = root.join("tests/fixtures/three-product-design/disposition-map.toml");
    if !disposition_map.is_file() {
        return Err(format!(
            "three-product reconstruction fixture missing: {}",
            disposition_map.display()
        ));
    }
    let disposition_text = std::fs::read_to_string(&disposition_map)
        .map_err(|error| format!("disposition map should be readable: {error}"))?;
    let disposition = toml::from_str::<ThreeProductDispositionMap>(&disposition_text)
        .map_err(|error| format!("disposition map should parse as TOML: {error}"))?;

    if disposition.schema_version != "2.0"
        || disposition.authority_generation != 2
        || disposition.design_package_proposal != "CARGO-ALLOW-PROP-0010"
        || disposition.ownership_adr != "CARGO-ALLOW-ADR-0002"
        || disposition.package_identity_adr != "CARGO-ALLOW-ADR-0003"
        || disposition.historical_spec != "CARGO-ALLOW-SPEC-0010"
        || disposition.current_spec != "CARGO-ALLOW-SPEC-0011"
        || disposition.design_package_plan != "CARGO-ALLOW-PLAN-0010"
        || disposition.historical_scaffold_package_count != 27
        || disposition.current_package_count != 22
        || disposition.retained_package_count != 22
        || disposition.repository_extraction_authorized
        || disposition.release_authorized
    {
        return Err("generation-2 disposition authority fields do not match".to_string());
    }
    if disposition.collapse.len() != 5
        || disposition.collapse.iter().any(|row| {
            row.target_module.trim().is_empty() || row.disposition != "CompletedAbsorption"
        })
    {
        return Err("generation-2 absorption projection is incomplete".to_string());
    }

    let ledger = parse_doc_artifact_ledger(include_str!(
        "../../../../.allow/artifacts/doc-artifacts.toml"
    ))
    .map_err(|error| error.to_string())?;
    for id in [
        "CARGO-ALLOW-PROP-0010",
        "CARGO-ALLOW-ADR-0002",
        "CARGO-ALLOW-ADR-0003",
        "CARGO-ALLOW-SPEC-0010",
        "CARGO-ALLOW-SPEC-0011",
        "CARGO-ALLOW-PLAN-0010",
    ] {
        if !ledger.artifact.iter().any(|artifact| artifact.id == id) {
            return Err(format!("artifact ledger missing {id}"));
        }
    }
    validate_doc_artifact_links(&ledger).map_err(|error| error.to_string())?;
    validate_doc_artifact_files(repo_root(), &ledger, &test_roots())
        .map_err(|error| error.to_string())?;
    Ok(())
}
