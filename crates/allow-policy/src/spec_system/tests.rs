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

#[test]
fn validates_current_repository_artifact_files() {
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

    let result = validate_doc_artifact_files(repo_root(), &ledger, &test_roots());

    assert!(
        result.is_ok(),
        "repo artifacts validate: {:?}",
        result.err()
    );
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
        "policy/spec-system.toml",
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
            path = "policy/spec-system.toml"
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
        goals: ".codex/goals".to_string(),
        support_tiers: "docs/status/SUPPORT_TIERS.md".to_string(),
        artifact_ledger: "policy/doc-artifacts.toml".to_string(),
    }
}
