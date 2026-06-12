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

fn unique_stamp() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos())
}
