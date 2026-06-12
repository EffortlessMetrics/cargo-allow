use super::*;

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
            goals = ".codex/goals"
            support_tiers = "docs/status/SUPPORT_TIERS.md"
            artifact_ledger = "policy/doc-artifacts.toml"

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
    assert_eq!(cfg.roots.artifact_ledger, "policy/doc-artifacts.toml");
    assert!(cfg.requirements.ledger_required);
    assert!(cfg.requirements.closeout_required_for_done_items);
}

#[test]
fn parses_current_repository_doc_artifact_ledger() {
    let ledger_result =
        parse_doc_artifact_ledger(include_str!("../../../../policy/doc-artifacts.toml"));
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
