use std::{fs, path::Path};

use super::*;

#[test]
fn saved_migrate_output_covers_policy_migration_summary_contract() {
    let fixture = SourceTreeFixture::new("saved-migrate-summary");
    let legacy_dir = fixture.root.join("legacy-policy");
    fs::create_dir_all(&legacy_dir)
        .unwrap_or_else(|err| panic!("create legacy policy dir: {err}"));
    fs::write(
        legacy_dir.join("process-allowlist.toml"),
        process_policy_fixture_text(),
    )
    .unwrap_or_else(|err| panic!("write process policy fixture: {err}"));
    fs::write(
        legacy_dir.join("unsafe-allowlist.toml"),
        unsafe_policy_fixture_text(),
    )
    .unwrap_or_else(|err| panic!("write unsafe policy fixture: {err}"));

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let migrated_policy = artifact_dir.join("allow.migrated.toml");
    let migrate_summary = artifact_dir.join("migrate-summary.json");

    run_cargo_allow(&[
        "migrate",
        "--root",
        fixture.root_str(),
        "--repo-policy",
        path_arg(&legacy_dir),
        "--out",
        path_arg(&migrated_policy),
        "--summary-format",
        "json",
        "--summary-output",
        path_arg(&migrate_summary),
    ]);

    assert_policy_output(&migrated_policy);
    let value = assert_policy_migration_artifact_with_inventory(
        &migrate_summary,
        allow_report::MIGRATE_SCHEMA_ID,
        "migrate",
        "filesystem_fallback",
    );
    let receipt = value.pointer("/mutation_receipt").unwrap_or_else(|| {
        std::panic::panic_any("migrate summary should include mutation receipt")
    });
    assert_eq!(
        receipt.get("operation").and_then(serde_json::Value::as_str),
        Some("migrate"),
        "migrate receipt operation"
    );
    assert_eq!(
        receipt.get("result").and_then(serde_json::Value::as_str),
        Some("written"),
        "migrate receipt result"
    );
    let changed_ids = receipt
        .get("changed_allow_ids")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| {
            std::panic::panic_any("migrate receipt should include changed allow IDs")
        });
    assert_eq!(changed_ids.len(), 2, "migrate receipt changed ID count");
    assert_eq!(
        changed_ids[0].as_str(),
        Some("proc-bash-package-proof"),
        "migrate receipt IDs should be sorted deterministically"
    );
    assert_eq!(
        changed_ids[1].as_str(),
        Some("unsafe-ffi-boundary"),
        "migrate receipt IDs should identify the migrated entries"
    );
    assert_eq!(
        receipt
            .get("before_fingerprints")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(changed_ids.len()),
        "migrate receipt before fingerprints align with IDs"
    );
    assert_eq!(
        receipt
            .get("after_fingerprints")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(changed_ids.len()),
        "migrate receipt after fingerprints align with IDs"
    );
    assert!(
        receipt
            .pointer("/next_commands/1")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|command| command == "cargo-allow check --mode no-new"),
        "migrate receipt should route post-migration validation"
    );
    assert_eq!(
        value
            .pointer("/summary/allow_entries")
            .and_then(serde_json::Value::as_u64),
        Some(2),
        "migrate summary allow entry count"
    );
    assert_eq!(
        value
            .pointer("/summary/unsafe_entries")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "migrate summary unsafe entry count"
    );
    assert_eq!(
        value
            .pointer("/summary/evidence_entries")
            .and_then(serde_json::Value::as_u64),
        Some(7),
        "migrate summary evidence reference count"
    );
    assert_eq!(
        value
            .pointer("/summary/unsafe_broken_evidence_links")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "migrate summary unsafe broken evidence count"
    );
    assert_eq!(
        value
            .pointer("/summary/unsafe_weak_evidence_references")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "migrate summary unsafe weak evidence count"
    );
    let queues = value
        .pointer("/evidence_repair_queues")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| {
            std::panic::panic_any("migrate summary should route evidence repair queues")
        });
    let broken = migrate_queue(queues, "broken_evidence_link");
    assert_eq!(
        broken.get("signal").and_then(serde_json::Value::as_str),
        Some("broken_evidence_links"),
        "migrate broken evidence queue signal"
    );
    assert_eq!(
        broken.get("route_kind").and_then(serde_json::Value::as_str),
        Some("worklist_item_kind"),
        "migrate broken evidence queue route kind"
    );
    assert_eq!(
        broken.get("count").and_then(serde_json::Value::as_u64),
        Some(1),
        "migrate broken evidence queue count"
    );
    assert_eq!(
        broken
            .get("unsafe_count")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "migrate broken evidence unsafe queue count"
    );
    assert_eq!(
        broken.get("command").and_then(serde_json::Value::as_str),
        Some("cargo-allow worklist --item-kind broken_evidence_link --format json"),
        "migrate broken evidence queue command"
    );
    assert_eq!(
        broken
            .get("unsafe_command")
            .and_then(serde_json::Value::as_str),
        Some("cargo-allow worklist --item-kind broken_evidence_link --kind unsafe --format json"),
        "migrate broken evidence unsafe queue command"
    );

    let weak = migrate_queue(queues, "weak_evidence_reference");
    assert_eq!(
        weak.get("signal").and_then(serde_json::Value::as_str),
        Some("weak_evidence_references"),
        "migrate weak evidence queue signal"
    );
    assert_eq!(
        weak.get("route_kind").and_then(serde_json::Value::as_str),
        Some("worklist_item_kind"),
        "migrate weak evidence queue route kind"
    );
    assert_eq!(
        weak.get("count").and_then(serde_json::Value::as_u64),
        Some(5),
        "migrate weak evidence queue count"
    );
    assert_eq!(
        weak.get("unsafe_count").and_then(serde_json::Value::as_u64),
        Some(1),
        "migrate weak evidence unsafe queue count"
    );
    assert_eq!(
        weak.get("command").and_then(serde_json::Value::as_str),
        Some("cargo-allow worklist --item-kind weak_evidence_reference --format json"),
        "migrate weak evidence queue command"
    );
    assert_eq!(
        weak.get("unsafe_command")
            .and_then(serde_json::Value::as_str),
        Some(
            "cargo-allow worklist --item-kind weak_evidence_reference --kind unsafe --format json"
        ),
        "migrate weak evidence unsafe queue command"
    );
    let actual_policy_output = value
        .pointer("/output/path")
        .and_then(serde_json::Value::as_str)
        .map(|path| path.replace('\\', "/"));
    let expected_policy_output = path_arg(&migrated_policy).replace('\\', "/");
    assert_eq!(
        actual_policy_output.as_deref(),
        Some(expected_policy_output.as_str()),
        "migrate policy output path"
    );
}

#[test]
fn saved_migrate_output_preserves_legacy_evidence_and_links() {
    let fixture = SourceTreeFixture::new("saved-migrate-evidence-preserved");
    let legacy_dir = fixture.root.join("legacy-policy");
    fs::create_dir_all(&legacy_dir)
        .unwrap_or_else(|err| panic!("create legacy policy dir: {err}"));
    fs::write(
        legacy_dir.join("generated-allowlist.toml"),
        generated_policy_with_evidence_fixture_text(),
    )
    .unwrap_or_else(|err| panic!("write generated policy fixture: {err}"));
    fs::write(
        legacy_dir.join("executable-allowlist.toml"),
        executable_policy_with_covered_by_fixture_text(),
    )
    .unwrap_or_else(|err| panic!("write executable policy fixture: {err}"));

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let migrated_policy = artifact_dir.join("allow.migrated.toml");
    let migrate_summary = artifact_dir.join("migrate-summary.json");

    run_cargo_allow(&[
        "migrate",
        "--root",
        fixture.root_str(),
        "--repo-policy",
        path_arg(&legacy_dir),
        "--out",
        path_arg(&migrated_policy),
        "--summary-format",
        "json",
        "--summary-output",
        path_arg(&migrate_summary),
    ]);

    assert_policy_output(&migrated_policy);
    let value = assert_policy_migration_artifact_with_inventory(
        &migrate_summary,
        allow_report::MIGRATE_SCHEMA_ID,
        "migrate",
        "filesystem_fallback",
    );
    assert_eq!(
        value
            .pointer("/summary/allow_entries")
            .and_then(serde_json::Value::as_u64),
        Some(2),
        "migrate evidence preservation allow entry count"
    );
    assert_eq!(
        value
            .pointer("/summary/evidence_entries")
            .and_then(serde_json::Value::as_u64),
        Some(8),
        "migrate evidence preservation evidence count"
    );
    assert_eq!(
        value
            .pointer("/summary/entries_with_links")
            .and_then(serde_json::Value::as_u64),
        Some(2),
        "migrate evidence preservation link-entry count"
    );
    assert_eq!(
        value
            .pointer("/summary/link_entries")
            .and_then(serde_json::Value::as_u64),
        Some(2),
        "migrate evidence preservation link count"
    );

    let cfg = allow_policy::load_policy(&migrated_policy).unwrap_or_else(|err| {
        panic!(
            "load migrated policy {}: {err}",
            migrated_policy.display()
        )
    });
    let generated = migrated_entry(&cfg, "saved-generated-evidence");
    assert_eq!(generated.kind, allow_core::FindingKind::GeneratedCode);
    assert_eq!(generated.family.as_deref(), Some("generated_code"));
    assert_entry_metadata(
        generated,
        "policy",
        "generated_code",
        "Generated schema fixture.",
        Some("2026-05-10"),
        Some("2026-05-10"),
        Some("never"),
    );
    assert_eq!(
        generated.path.as_deref(),
        Some(Path::new("docs/generated/schema.json"))
    );
    assert_entry_evidence(
        generated,
        &[
            "doc:docs/generated/schema.md",
            "issue:#314",
            "legacy-policy:saved-generated-evidence",
            "generator:cargo xtask schema",
            "cargo:cargo xtask schema",
        ],
    );
    assert_entry_links(generated, &["legacy-policy:saved-generated-evidence"]);

    let executable = migrated_entry(&cfg, "saved-executable-covered");
    assert_eq!(executable.kind, allow_core::FindingKind::PolicyException);
    assert_eq!(executable.family.as_deref(), Some("executable_file"));
    assert_entry_metadata(
        executable,
        "release",
        "executable_file",
        "Package helper fixture.",
        Some("2026-05-09"),
        Some("2026-05-09"),
        Some("never"),
    );
    assert_eq!(
        executable.path.as_deref(),
        Some(Path::new("scripts/package.sh"))
    );
    assert_entry_evidence(
        executable,
        &[
            "doc:docs/release/package.md",
            "legacy-policy:saved-executable-covered",
            "interpreter:bash",
        ],
    );
    assert_entry_links(executable, &["legacy-policy:saved-executable-covered"]);
}

#[test]
fn saved_migrate_output_preserves_non_rust_evidence_matrix() {
    let fixture = SourceTreeFixture::new("saved-migrate-non-rust-evidence-matrix");
    let legacy_dir = fixture.root.join("legacy-policy");
    fs::create_dir_all(&legacy_dir)
        .unwrap_or_else(|err| panic!("create legacy policy dir: {err}"));
    write_fixture_doc(&fixture.root, "docs/source-exception-ledger.md");
    write_fixture_doc(&fixture.root, "docs/source-exception-ledger-evidence.md");
    write_fixture_doc(&fixture.root, "docs/source-exception-ledger-review.md");
    write_fixture_doc(&fixture.root, "docs/ci-evidence.md");
    write_fixture_doc(&fixture.root, ".github/workflows/ci.yml");
    fs::write(
        legacy_dir.join("non-rust-allowlist.toml"),
        non_rust_policy_with_evidence_fixture_text(),
    )
    .unwrap_or_else(|err| panic!("write non-rust policy fixture: {err}"));

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let compat_audit = artifact_dir.join("non-rust-compat-audit.json");
    let migrated_policy = artifact_dir.join("allow.migrated.toml");
    let migrate_summary = artifact_dir.join("migrate-summary.json");

    run_cargo_allow(&[
        "audit",
        "--root",
        fixture.root_str(),
        "--compat",
        "--kind",
        "non-rust",
        "--config",
        path_arg(&legacy_dir.join("non-rust-allowlist.toml")),
        "--format",
        "json",
        "--output",
        path_arg(&compat_audit),
    ]);

    let compat_report = assert_source_syntax_artifact_with_inventory(
        &compat_audit,
        allow_report::REPORT_SCHEMA_ID,
        "audit",
        "filesystem_fallback",
    );
    assert_eq!(
        compat_report
            .pointer("/summary/findings")
            .and_then(serde_json::Value::as_u64),
        Some(6),
        "legacy non-rust audit should report the complete fixture inventory"
    );
    let compat_paths = compat_report
        .pointer("/findings")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|finding| finding.get("path"))
        .filter_map(serde_json::Value::as_str)
        .map(str::to_owned)
        .collect::<Vec<_>>();

    run_cargo_allow(&[
        "migrate",
        "--root",
        fixture.root_str(),
        "--repo-policy",
        path_arg(&legacy_dir),
        "--out",
        path_arg(&migrated_policy),
        "--summary-format",
        "json",
        "--summary-output",
        path_arg(&migrate_summary),
    ]);

    assert_policy_output(&migrated_policy);
    let value = assert_policy_migration_artifact_with_inventory(
        &migrate_summary,
        allow_report::MIGRATE_SCHEMA_ID,
        "migrate",
        "filesystem_fallback",
    );
    assert_eq!(
        value
            .pointer("/summary/allow_entries")
            .and_then(serde_json::Value::as_u64),
        Some(2),
        "migrate non-rust evidence matrix allow entry count"
    );
    assert_eq!(
        value
            .pointer("/summary/entries_with_evidence")
            .and_then(serde_json::Value::as_u64),
        Some(2),
        "migrate non-rust evidence matrix entries-with-evidence count"
    );
    assert_eq!(
        value
            .pointer("/summary/evidence_entries")
            .and_then(serde_json::Value::as_u64),
        Some(3),
        "migrate non-rust evidence matrix evidence count"
    );
    assert_eq!(
        value
            .pointer("/summary/entries_with_links")
            .and_then(serde_json::Value::as_u64),
        Some(2),
        "migrate non-rust evidence matrix link-entry count"
    );
    assert_eq!(
        value
            .pointer("/summary/link_entries")
            .and_then(serde_json::Value::as_u64),
        Some(2),
        "migrate non-rust evidence matrix link count"
    );
    assert!(
        value.pointer("/summary/broken_evidence_links").is_none(),
        "fixture should preserve non-rust local doc evidence without broken links"
    );
    assert!(
        value.pointer("/summary/weak_evidence_references").is_none(),
        "fixture should preserve non-rust evidence without weak references"
    );

    let cfg = allow_policy::load_policy(&migrated_policy).unwrap_or_else(|err| {
        panic!(
            "load migrated policy {}: {err}",
            migrated_policy.display()
        )
    });
    let docs = migrated_entry_by_path(&cfg, Path::new("docs/source-exception-ledger.md"));
    assert!(docs.id.starts_with("saved-non-rust-doc--"), "docs id");
    assert_eq!(docs.kind, allow_core::FindingKind::NonRustFile);
    assert_eq!(docs.family, None);
    assert_entry_metadata(
        docs,
        "docs",
        "documentation",
        "Repository policy prose fixture.",
        Some("2026-05-09"),
        Some("2026-09-09"),
        None,
    );
    assert_entry_evidence(
        docs,
        &[
            "doc:docs/source-exception-ledger-evidence.md",
            "doc:docs/source-exception-ledger-review.md",
        ],
    );
    assert_entry_links(docs, &["legacy-policy:saved-non-rust-doc"]);

    let workflow = migrated_entry_by_path(&cfg, Path::new(".github/workflows/ci.yml"));
    assert!(
        workflow.id.starts_with("saved-non-rust-workflow--"),
        "workflow id"
    );
    assert_eq!(workflow.kind, allow_core::FindingKind::NonRustFile);
    assert_eq!(workflow.family, None);
    assert_entry_metadata(
        workflow,
        "release/ci",
        "ci_declarative",
        "Workflow file fixture.",
        Some("2026-05-09"),
        Some("2026-05-09"),
        Some("never"),
    );
    assert_entry_evidence(workflow, &["doc:docs/ci-evidence.md"]);
    assert_entry_links(workflow, &["legacy-policy:saved-non-rust-workflow"]);

    let canonical_audit = artifact_dir.join("non-rust-canonical-audit.json");
    run_cargo_allow(&[
        "audit",
        "--root",
        fixture.root_str(),
        "--config",
        path_arg(&migrated_policy),
        "--format",
        "json",
        "--output",
        path_arg(&canonical_audit),
    ]);
    let canonical_report = assert_source_syntax_artifact_with_inventory(
        &canonical_audit,
        allow_report::REPORT_SCHEMA_ID,
        "audit",
        "filesystem_fallback",
    );
    assert_eq!(
        canonical_report
            .pointer("/summary/findings")
            .and_then(serde_json::Value::as_u64),
        Some(6),
        "canonical non-rust audit should report the complete fixture inventory"
    );
    let canonical_paths = canonical_report
        .pointer("/findings")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|finding| finding.get("path"))
        .filter_map(serde_json::Value::as_str)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    assert_eq!(
        compat_paths, canonical_paths,
        "legacy and canonical audits should preserve the same non-rust paths"
    );
}

#[test]
fn saved_migrate_output_is_deterministic_for_shuffled_non_rust_rows() {
    let fixture = SourceTreeFixture::new("saved-migrate-non-rust-shuffled-rows");
    let legacy_dir = fixture.root.join("legacy-policy");
    fs::create_dir_all(&legacy_dir)
        .unwrap_or_else(|err| panic!("create legacy policy dir: {err}"));
    write_fixture_doc(&fixture.root, "docs/guide.md");
    write_fixture_doc(&fixture.root, "docs/reference.md");
    write_fixture_doc(&fixture.root, "migration-evidence.md");

    let legacy_policy = legacy_dir.join("non-rust-allowlist.toml");
    let migrated_policy = fixture.root.join("target/cargo-allow/allow.migrated.toml");
    let migrate_summary = fixture.root.join("target/cargo-allow/migrate-summary.json");

    let forward = non_rust_policy_with_equal_specificity_rows_fixture_text(false);
    let reverse = non_rust_policy_with_equal_specificity_rows_fixture_text(true);
    assert_ne!(
        forward, reverse,
        "fixture should actually reorder legacy rows"
    );

    fs::write(&legacy_policy, &forward).unwrap_or_else(|err| {
        panic!("write forward policy fixture: {err}")
    });
    run_cargo_allow(&[
        "migrate",
        "--root",
        fixture.root_str(),
        "--repo-policy",
        path_arg(&legacy_dir),
        "--out",
        path_arg(&migrated_policy),
        "--summary-format",
        "json",
        "--summary-output",
        path_arg(&migrate_summary),
    ]);
    assert_policy_output(&migrated_policy);
    let first_summary = assert_policy_migration_artifact_with_inventory(
        &migrate_summary,
        allow_report::MIGRATE_SCHEMA_ID,
        "migrate",
        "filesystem_fallback",
    );
    let first_policy_text = fs::read_to_string(&migrated_policy).unwrap_or_else(|err| {
        panic!("read forward migrated policy: {err}")
    });
    let first_summary_text = fs::read_to_string(&migrate_summary).unwrap_or_else(|err| {
        panic!("read forward migrate summary: {err}")
    });
    let first_cfg = allow_policy::load_policy(&migrated_policy).unwrap_or_else(|err| {
        panic!("load forward migrated policy: {err}")
    });
    assert_eq!(
        first_cfg
            .allow
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>(),
        vec!["a-broad--0001", "a-broad--0002"],
        "equal-specificity rows should select the stable rule ID"
    );
    assert_eq!(
        first_summary
            .pointer("/summary/allow_entries")
            .and_then(serde_json::Value::as_u64),
        Some(2),
        "shuffled-row fixture should migrate both governed files"
    );

    fs::remove_file(&migrated_policy).unwrap_or_else(|err| {
        panic!("remove first migrated policy: {err}")
    });
    fs::remove_file(&migrate_summary).unwrap_or_else(|err| {
        panic!("remove first migrate summary: {err}")
    });
    fs::write(&legacy_policy, &reverse).unwrap_or_else(|err| {
        panic!("write reverse policy fixture: {err}")
    });
    run_cargo_allow(&[
        "migrate",
        "--root",
        fixture.root_str(),
        "--repo-policy",
        path_arg(&legacy_dir),
        "--out",
        path_arg(&migrated_policy),
        "--summary-format",
        "json",
        "--summary-output",
        path_arg(&migrate_summary),
    ]);
    assert_policy_output(&migrated_policy);
    assert_policy_migration_artifact_with_inventory(
        &migrate_summary,
        allow_report::MIGRATE_SCHEMA_ID,
        "migrate",
        "filesystem_fallback",
    );
    let second_policy_text = fs::read_to_string(&migrated_policy).unwrap_or_else(|err| {
        panic!("read reverse migrated policy: {err}")
    });
    let second_summary_text = fs::read_to_string(&migrate_summary).unwrap_or_else(|err| {
        panic!("read reverse migrate summary: {err}")
    });

    assert_eq!(
        first_policy_text, second_policy_text,
        "shuffling equal-specificity legacy rows must not change canonical policy bytes"
    );
    assert_eq!(
        first_summary_text, second_summary_text,
        "shuffling equal-specificity legacy rows must not change migration summary bytes"
    );
}

#[test]
fn saved_migrate_non_rust_negative_controls_fail_closed_or_surface_debt() {
    for (fixture_name, diagnostic, line) in [
        ("non-rust-malformed.toml", "invalid basic string", Some(6)),
        (
            "non-rust-unsupported.toml",
            "unsupported field `approval_ticket`",
            Some(4),
        ),
        (
            "non-rust-conflict.toml",
            "defines both path and glob",
            Some(5),
        ),
        (
            "non-rust-duplicate.toml",
            "non-rust allow entry 1 duplicates earlier id `negative-duplicate`",
            Some(10),
        ),
        (
            "non-rust-invalid-owner.toml",
            "field `owner` must be a string",
            Some(4),
        ),
        (
            "non-rust-invalid-evidence.toml",
            "field `evidence` must be a string or string array",
            Some(4),
        ),
        (
            "non-rust-invalid-covered-by.toml",
            "field `covered_by` must be a string or string array",
            Some(4),
        ),
    ] {
        assert_non_rust_fixture_rejected(fixture_name, diagnostic, line);
    }

    assert_non_rust_dual_evidence_preserved();

    let fixture = SourceTreeFixture::new("saved-migrate-non-rust-missing-evidence");
    let legacy_dir = fixture.root.join("legacy-policy");
    fs::create_dir_all(&legacy_dir)
        .unwrap_or_else(|err| panic!("create legacy policy dir: {err}"));
    write_fixture_doc(&fixture.root, "docs/missing-evidence.md");
    let legacy_policy = legacy_dir.join("non-rust-allowlist.toml");
    copy_non_rust_negative_fixture("non-rust-missing-evidence.toml", &legacy_policy);

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let compat_audit = artifact_dir.join("compat-audit.json");
    let migrated_policy = artifact_dir.join("allow.migrated.toml");
    let migrate_summary = artifact_dir.join("migrate-summary.json");
    run_cargo_allow(&[
        "audit",
        "--root",
        fixture.root_str(),
        "--compat",
        "--kind",
        "non-rust",
        "--config",
        path_arg(&legacy_policy),
        "--format",
        "json",
        "--output",
        path_arg(&compat_audit),
    ]);
    let compat = assert_source_syntax_artifact_with_inventory(
        &compat_audit,
        allow_report::REPORT_SCHEMA_ID,
        "audit",
        "filesystem_fallback",
    );

    let migrate_args = [
        "migrate",
        "--root",
        fixture.root_str(),
        "--repo-policy",
        path_arg(&legacy_dir),
        "--out",
        path_arg(&migrated_policy),
        "--summary-format",
        "json",
        "--summary-output",
        path_arg(&migrate_summary),
    ];
    run_cargo_allow(&migrate_args);
    assert_policy_output(&migrated_policy);
    let summary = assert_policy_migration_artifact_with_inventory(
        &migrate_summary,
        allow_report::MIGRATE_SCHEMA_ID,
        "migrate",
        "filesystem_fallback",
    );
    assert!(
        summary
            .pointer("/summary/weak_evidence_references")
            .and_then(serde_json::Value::as_u64)
            .is_some_and(|count| count >= 1),
        "missing legacy evidence must remain visible in the migration summary: {summary}"
    );
    let policy_text = fs::read_to_string(&migrated_policy)
        .unwrap_or_else(|err| panic!("read migrated policy: {err}"));
    let summary_text = fs::read_to_string(&migrate_summary)
        .unwrap_or_else(|err| panic!("read migrate summary: {err}"));
    let cfg = allow_policy::load_policy(&migrated_policy)
        .unwrap_or_else(|err| panic!("load migrated policy: {err}"));
    let entry = migrated_entry_by_path(&cfg, Path::new("docs/missing-evidence.md"));
    assert!(
        entry
            .evidence
            .contains(&"legacy-policy:negative-missing-evidence".to_string())
            && entry
                .evidence
                .contains(&"TODO: add non-rust migration evidence".to_string()),
        "missing evidence must become explicit provenance plus TODO debt: {:?}",
        entry.evidence
    );

    let canonical_audit = artifact_dir.join("canonical-audit.json");
    run_cargo_allow(&[
        "audit",
        "--root",
        fixture.root_str(),
        "--config",
        path_arg(&migrated_policy),
        "--format",
        "json",
        "--output",
        path_arg(&canonical_audit),
    ]);
    let canonical = assert_source_syntax_artifact_with_inventory(
        &canonical_audit,
        allow_report::REPORT_SCHEMA_ID,
        "audit",
        "filesystem_fallback",
    );
    let finding_paths = |value: &serde_json::Value| {
        value
            .pointer("/findings")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|finding| finding.get("path"))
            .filter_map(serde_json::Value::as_str)
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        finding_paths(&compat),
        finding_paths(&canonical),
        "missing-evidence downgrade must not alter compat/canonical governed paths"
    );

    let worklist = artifact_dir.join("worklist.json");
    run_cargo_allow(&[
        "worklist",
        "--root",
        fixture.root_str(),
        "--config",
        path_arg(&migrated_policy),
        "--format",
        "json",
        "--output",
        path_arg(&worklist),
    ]);
    let worklist =
        assert_source_syntax_artifact(&worklist, allow_report::WORKLIST_SCHEMA_ID, "worklist");
    assert!(
        worklist
            .pointer("/work_items")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|items| items.iter().any(|item| {
                item.get("allow_id")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|id| id.starts_with("negative-missing-evidence--"))
            })),
        "canonical worklist must retain missing-evidence downgrade debt: {worklist}"
    );

    fs::remove_file(&migrated_policy)
        .unwrap_or_else(|err| panic!("remove migrated policy: {err}"));
    fs::remove_file(&migrate_summary)
        .unwrap_or_else(|err| panic!("remove migrate summary: {err}"));
    run_cargo_allow(&migrate_args);
    assert_eq!(
        fs::read_to_string(&migrated_policy)
            .unwrap_or_else(|err| panic!("read rerun policy: {err}")),
        policy_text,
        "unchanged missing-evidence migration must reproduce canonical policy bytes"
    );
    assert_eq!(
        fs::read_to_string(&migrate_summary)
            .unwrap_or_else(|err| panic!("read rerun summary: {err}")),
        summary_text,
        "unchanged missing-evidence migration must reproduce summary bytes"
    );
}

fn assert_non_rust_dual_evidence_preserved() {
    let fixture = SourceTreeFixture::new("saved-migrate-non-rust-dual-evidence");
    let legacy_dir = fixture.root.join("legacy-policy");
    fs::create_dir_all(&legacy_dir)
        .unwrap_or_else(|err| panic!("create legacy policy dir: {err}"));
    let legacy_policy = legacy_dir.join("non-rust-allowlist.toml");
    copy_non_rust_negative_fixture("non-rust-dual-evidence.toml", &legacy_policy);
    write_fixture_doc(&fixture.root, "docs/dual-evidence.md");

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let migrated_policy = artifact_dir.join("allow.migrated.toml");
    let migrate_summary = artifact_dir.join("migrate-summary.json");
    run_cargo_allow(&[
        "migrate",
        "--root",
        fixture.root_str(),
        "--repo-policy",
        path_arg(&legacy_dir),
        "--out",
        path_arg(&migrated_policy),
        "--summary-format",
        "json",
        "--summary-output",
        path_arg(&migrate_summary),
    ]);
    assert_policy_output(&migrated_policy);
    assert_policy_migration_artifact_with_inventory(
        &migrate_summary,
        allow_report::MIGRATE_SCHEMA_ID,
        "migrate",
        "filesystem_fallback",
    );
    let cfg = allow_policy::load_policy(&migrated_policy)
        .unwrap_or_else(|err| panic!("load migrated policy: {err}"));
    let entry = migrated_entry_by_path(&cfg, Path::new("docs/dual-evidence.md"));
    assert_eq!(
        entry.evidence,
        vec![
            "test:explicit".to_string(),
            "issue:#2469".to_string(),
            "spec:NON-RUST-DUAL".to_string(),
        ],
        "dual legacy evidence fields must merge stably without duplicates"
    );
}

fn assert_non_rust_fixture_rejected(
    fixture_name: &str,
    expected_diagnostic: &str,
    expected_line: Option<usize>,
) {
    let fixture = SourceTreeFixture::new(&format!("saved-migrate-{fixture_name}"));
    let legacy_dir = fixture.root.join("legacy-policy");
    fs::create_dir_all(&legacy_dir)
        .unwrap_or_else(|err| panic!("create legacy policy dir: {err}"));
    let legacy_policy = legacy_dir.join("non-rust-allowlist.toml");
    copy_non_rust_negative_fixture(fixture_name, &legacy_policy);
    let artifact_dir = fixture.root.join("target/cargo-allow");
    let compat_output = artifact_dir.join("compat.json");
    let migrated_output = artifact_dir.join("allow.migrated.toml");
    let summary_output = artifact_dir.join("migrate-summary.json");

    for (command, args) in [
        (
            "compat",
            vec![
                "audit",
                "--root",
                fixture.root_str(),
                "--compat",
                "--kind",
                "non-rust",
                "--config",
                path_arg(&legacy_policy),
                "--format",
                "json",
                "--output",
                path_arg(&compat_output),
            ],
        ),
        (
            "migrate",
            vec![
                "migrate",
                "--root",
                fixture.root_str(),
                "--repo-policy",
                path_arg(&legacy_dir),
                "--out",
                path_arg(&migrated_output),
                "--summary-format",
                "json",
                "--summary-output",
                path_arg(&summary_output),
            ],
        ),
    ] {
        let output = super::support::cargo_allow_command()
            .args(&args)
            .output()
            .unwrap_or_else(|err| {
                panic!("run {command} for {fixture_name}: {err}")
            });
        assert!(
            !output.status.success(),
            "{command} unexpectedly accepted {fixture_name}"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("non-rust-allowlist.toml") && stderr.contains(expected_diagnostic),
            "{command} diagnostic for {fixture_name} lost source or cause: {stderr}"
        );
        if let Some(line) = expected_line {
            assert!(
                stderr.contains(&format!("non-rust-allowlist.toml:{line}")),
                "{command} diagnostic for {fixture_name} lost entry line {line}: {stderr}"
            );
        }
    }
}

fn copy_non_rust_negative_fixture(name: &str, destination: &Path) {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/migration")
        .join(name);
    fs::copy(&source, destination).unwrap_or_else(|err| {
        panic!(
            "copy negative migration fixture {} to {}: {err}",
            source.display(),
            destination.display()
        )
    });
}

#[test]
fn saved_migrate_output_preserves_policy_exception_evidence_matrix() {
    let fixture = SourceTreeFixture::new("saved-migrate-policy-exception-evidence-matrix");
    let legacy_dir = fixture.root.join("legacy-policy");
    fs::create_dir_all(&legacy_dir)
        .unwrap_or_else(|err| panic!("create legacy policy dir: {err}"));
    write_fixture_doc(&fixture.root, "docs/ci.md");
    write_fixture_doc(&fixture.root, "docs/dependencies.md");
    write_fixture_doc(&fixture.root, "docs/release/process.md");
    write_fixture_doc(&fixture.root, "docs/network.md");
    fs::write(
        legacy_dir.join("workflow-allowlist.toml"),
        workflow_policy_with_evidence_fixture_text(),
    )
    .unwrap_or_else(|err| panic!("write workflow policy fixture: {err}"));
    fs::write(
        legacy_dir.join("dependency-surface-allowlist.toml"),
        dependency_policy_with_evidence_fixture_text(),
    )
    .unwrap_or_else(|err| panic!("write dependency policy fixture: {err}"));
    fs::write(
        legacy_dir.join("process-allowlist.toml"),
        process_policy_with_evidence_fixture_text(),
    )
    .unwrap_or_else(|err| panic!("write process policy fixture: {err}"));
    fs::write(
        legacy_dir.join("network-allowlist.toml"),
        network_policy_with_evidence_fixture_text(),
    )
    .unwrap_or_else(|err| panic!("write network policy fixture: {err}"));

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let migrated_policy = artifact_dir.join("allow.migrated.toml");
    let migrate_summary = artifact_dir.join("migrate-summary.json");

    run_cargo_allow(&[
        "migrate",
        "--root",
        fixture.root_str(),
        "--repo-policy",
        path_arg(&legacy_dir),
        "--out",
        path_arg(&migrated_policy),
        "--summary-format",
        "json",
        "--summary-output",
        path_arg(&migrate_summary),
    ]);

    assert_policy_output(&migrated_policy);
    let value = assert_policy_migration_artifact_with_inventory(
        &migrate_summary,
        allow_report::MIGRATE_SCHEMA_ID,
        "migrate",
        "filesystem_fallback",
    );
    assert_eq!(
        value
            .pointer("/summary/allow_entries")
            .and_then(serde_json::Value::as_u64),
        Some(5),
        "migrate policy-exception evidence matrix allow entry count"
    );
    assert_eq!(
        value
            .pointer("/summary/evidence_entries")
            .and_then(serde_json::Value::as_u64),
        Some(28),
        "migrate policy-exception evidence matrix evidence count"
    );
    assert_eq!(
        value
            .pointer("/summary/entries_with_links")
            .and_then(serde_json::Value::as_u64),
        Some(5),
        "migrate policy-exception evidence matrix link-entry count"
    );
    assert_eq!(
        value
            .pointer("/summary/link_entries")
            .and_then(serde_json::Value::as_u64),
        Some(5),
        "migrate policy-exception evidence matrix link count"
    );
    assert_eq!(
        value
            .pointer("/summary/weak_evidence_references")
            .and_then(serde_json::Value::as_u64),
        Some(13),
        "migrate policy-exception evidence matrix weak evidence count"
    );
    assert!(
        value.pointer("/summary/broken_evidence_links").is_none(),
        "fixture should preserve local doc evidence without broken links"
    );
    let queues = value
        .pointer("/evidence_repair_queues")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| {
            std::panic::panic_any("migrate summary should route weak evidence repair queues")
        });
    let weak = migrate_queue(queues, "weak_evidence_reference");
    assert_eq!(
        weak.get("count").and_then(serde_json::Value::as_u64),
        Some(13),
        "migrate policy-exception evidence matrix weak queue count"
    );
    assert_eq!(
        weak.get("command").and_then(serde_json::Value::as_str),
        Some("cargo-allow worklist --item-kind weak_evidence_reference --format json"),
        "migrate policy-exception evidence matrix weak queue command"
    );

    let cfg = allow_policy::load_policy(&migrated_policy).unwrap_or_else(|err| {
        panic!(
            "load migrated policy {}: {err}",
            migrated_policy.display()
        )
    });
    let workflow = migrated_entry(
        &cfg,
        "workflow-file-github-workflows-release-yml-2702d30def7815f5",
    );
    assert_eq!(workflow.kind, allow_core::FindingKind::PolicyException);
    assert_eq!(workflow.family.as_deref(), Some("github_workflow"));
    assert_entry_metadata(
        workflow,
        "release/ci",
        "github_workflow",
        "Release workflow fixture.",
        Some("2026-05-09"),
        Some("2026-05-09"),
        Some("never"),
    );
    assert_entry_evidence(
        workflow,
        &[
            "doc:docs/ci.md",
            "issue:#456",
            "legacy-policy:workflow:.github/workflows/release.yml",
            "permission:contents:read",
            "secret:RELEASE_TOKEN",
        ],
    );
    assert_entry_links(
        workflow,
        &["legacy-policy:workflow:.github/workflows/release.yml"],
    );

    let action = migrated_entry(
        &cfg,
        "workflow-action-github-workflows-release-yml-2702d30def7815f5--actions-checkout-v4",
    );
    assert_eq!(action.family.as_deref(), Some("workflow_external_action"));
    assert_entry_metadata(
        action,
        "release/ci",
        "workflow_external_action",
        "Release workflow fixture.",
        Some("2026-05-09"),
        Some("2026-05-09"),
        Some("never"),
    );
    assert_entry_evidence(
        action,
        &[
            "doc:docs/ci.md",
            "issue:#456",
            "legacy-policy:workflow:.github/workflows/release.yml",
            "external_action:actions/checkout@v4",
        ],
    );
    assert_entry_links(
        action,
        &["legacy-policy:workflow:.github/workflows/release.yml"],
    );

    let dependency = migrated_entry(&cfg, "saved-dependency-evidence");
    assert_eq!(dependency.family.as_deref(), Some("dependency_surface"));
    assert_entry_metadata(
        dependency,
        "release",
        "workspace_manifest",
        "Workspace dependency block fixture.",
        Some("2026-05-09"),
        Some("2026-05-09"),
        Some("never"),
    );
    assert_entry_evidence(
        dependency,
        &[
            "doc:docs/dependencies.md",
            "issue:#234",
            "legacy-policy:saved-dependency-evidence",
            "surface:workspace_manifest",
            "dep_count_at_baseline:22",
        ],
    );
    assert_entry_links(dependency, &["legacy-policy:saved-dependency-evidence"]);

    let process = migrated_entry(&cfg, "saved-process-evidence");
    assert_eq!(process.family.as_deref(), Some("process_spawn"));
    assert_entry_metadata(
        process,
        "release",
        "local_process",
        "Release helper fixture.",
        Some("2026-05-09"),
        Some("2026-05-09"),
        Some("never"),
    );
    assert_entry_evidence(
        process,
        &[
            "doc:docs/release/process.md",
            "issue:#789",
            "legacy-policy:saved-process-evidence",
            "binary:bash",
            "argv_shape:scripts/release.sh",
            "network_reach:false",
            "called_by:.github/workflows/release.yml",
        ],
    );
    assert_entry_links(process, &["legacy-policy:saved-process-evidence"]);

    let network = migrated_entry(&cfg, "saved-network-evidence");
    assert_eq!(network.family.as_deref(), Some("network_destination"));
    assert_entry_metadata(
        network,
        "release/ci",
        "authenticated_network",
        "Release API fixture.",
        Some("2026-05-09"),
        Some("2026-05-09"),
        Some("never"),
    );
    assert_entry_evidence(
        network,
        &[
            "doc:docs/network.md",
            "issue:#345",
            "legacy-policy:saved-network-evidence",
            "destination:api.github.com",
            "lane:release",
            "auth_required:true",
            "auth_secret:GITHUB_TOKEN",
        ],
    );
    assert_entry_links(network, &["legacy-policy:saved-network-evidence"]);
}

#[test]
fn saved_migrate_output_preserves_source_exception_evidence_matrix() {
    let fixture = SourceTreeFixture::new("saved-migrate-source-exception-evidence-matrix");
    let legacy_dir = fixture.root.join("legacy-policy");
    fs::create_dir_all(&legacy_dir)
        .unwrap_or_else(|err| panic!("create legacy policy dir: {err}"));
    write_fixture_doc(&fixture.root, "docs/evidence/unsafe/read.json");
    fs::write(
        legacy_dir.join("clippy-exceptions.toml"),
        clippy_policy_with_evidence_fixture_text(),
    )
    .unwrap_or_else(|err| panic!("write clippy policy fixture: {err}"));
    fs::write(
        legacy_dir.join("no-panic-allowlist.toml"),
        no_panic_allowlist_with_covered_by_fixture_text(),
    )
    .unwrap_or_else(|err| {
        panic!("write no-panic allowlist fixture: {err}")
    });
    fs::write(
        legacy_dir.join("unsafe-allowlist.toml"),
        unsafe_policy_with_evidence_fixture_text(),
    )
    .unwrap_or_else(|err| panic!("write unsafe policy fixture: {err}"));

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let migrated_policy = artifact_dir.join("allow.migrated.toml");
    let migrate_summary = artifact_dir.join("migrate-summary.json");

    run_cargo_allow(&[
        "migrate",
        "--root",
        fixture.root_str(),
        "--repo-policy",
        path_arg(&legacy_dir),
        "--out",
        path_arg(&migrated_policy),
        "--summary-format",
        "json",
        "--summary-output",
        path_arg(&migrate_summary),
    ]);

    assert_policy_output(&migrated_policy);
    let value = assert_policy_migration_artifact_with_inventory(
        &migrate_summary,
        allow_report::MIGRATE_SCHEMA_ID,
        "migrate",
        "filesystem_fallback",
    );
    assert_eq!(
        value
            .pointer("/summary/allow_entries")
            .and_then(serde_json::Value::as_u64),
        Some(3),
        "migrate source-exception evidence matrix allow entry count"
    );
    assert_eq!(
        value
            .pointer("/summary/unsafe_entries")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "migrate source-exception evidence matrix unsafe entry count"
    );
    assert_eq!(
        value
            .pointer("/summary/entries_with_evidence")
            .and_then(serde_json::Value::as_u64),
        Some(3),
        "migrate source-exception evidence matrix entries-with-evidence count"
    );
    assert_eq!(
        value
            .pointer("/summary/evidence_entries")
            .and_then(serde_json::Value::as_u64),
        Some(4),
        "migrate source-exception evidence matrix evidence count"
    );
    assert_eq!(
        value
            .pointer("/summary/entries_with_links")
            .and_then(serde_json::Value::as_u64),
        Some(3),
        "migrate source-exception evidence matrix link-entry count"
    );
    assert_eq!(
        value
            .pointer("/summary/link_entries")
            .and_then(serde_json::Value::as_u64),
        Some(3),
        "migrate source-exception evidence matrix link count"
    );
    assert!(
        value.pointer("/summary/broken_evidence_links").is_none(),
        "fixture should preserve source-exception local evidence without broken links"
    );
    assert!(
        value.pointer("/summary/weak_evidence_references").is_none(),
        "fixture should preserve source-exception evidence without weak references"
    );

    let cfg = allow_policy::load_policy(&migrated_policy).unwrap_or_else(|err| {
        panic!(
            "load migrated policy {}: {err}",
            migrated_policy.display()
        )
    });
    let clippy = migrated_entry(&cfg, "saved-clippy-evidence");
    assert_eq!(clippy.kind, allow_core::FindingKind::LintException);
    assert_eq!(clippy.family.as_deref(), Some("expect_attribute"));
    assert_entry_metadata(
        clippy,
        "runtime",
        "reviewed_lint_exception",
        "Intentional unwrap fixture for migration evidence parity.",
        Some("2026-05-09"),
        Some("2026-09-09"),
        None,
    );
    assert_entry_evidence(clippy, &["test:lint_policy_is_linked", "issue:#123"]);
    assert_entry_links(clippy, &["legacy-policy:saved-clippy-evidence"]);

    let no_panic = migrated_entry(&cfg, "saved-no-panic-covered");
    assert_eq!(no_panic.kind, allow_core::FindingKind::Panic);
    assert_eq!(no_panic.family.as_deref(), Some("unwrap"));
    assert_entry_metadata(
        no_panic,
        "runtime",
        "reviewed_panic_exception",
        "Parser validates optional value before unwrap.",
        Some("2026-05-09"),
        Some("2026-09-09"),
        None,
    );
    assert_entry_evidence(no_panic, &["test:parser_validates_optional_value"]);
    assert_entry_links(no_panic, &["legacy-policy:no-panic-allowlist"]);

    let unsafe_entry = migrated_entry(&cfg, "saved-unsafe-evidence");
    assert_eq!(unsafe_entry.kind, allow_core::FindingKind::Unsafe);
    assert_eq!(unsafe_entry.family.as_deref(), Some("unsafe_block"));
    assert_entry_metadata(
        unsafe_entry,
        "runtime",
        "reviewed_unsafe_boundary",
        "Caller validates pointer before read.",
        Some("2026-05-09"),
        Some("2026-09-09"),
        None,
    );
    assert_entry_evidence(
        unsafe_entry,
        &["unsafe-review:docs/evidence/unsafe/read.json"],
    );
    assert_entry_links(unsafe_entry, &["legacy-policy:saved-unsafe-evidence"]);
}

#[test]
fn saved_migrate_output_routes_unsafe_baseline_debt_closeout() {
    let fixture = SourceTreeFixture::new("saved-migrate-unsafe-baseline-debt-closeout");
    let legacy_dir = fixture.root.join("legacy-policy");
    fs::create_dir_all(&legacy_dir)
        .unwrap_or_else(|err| panic!("create legacy policy dir: {err}"));
    fs::write(
        legacy_dir.join("unsafe-allowlist.toml"),
        unsafe_policy_fixture_text(),
    )
    .unwrap_or_else(|err| panic!("write unsafe policy fixture: {err}"));

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let migrated_policy = artifact_dir.join("allow.migrated.toml");
    let migrate_summary = artifact_dir.join("migrate-summary.json");

    run_cargo_allow(&[
        "migrate",
        "--root",
        fixture.root_str(),
        "--repo-policy",
        path_arg(&legacy_dir),
        "--out",
        path_arg(&migrated_policy),
        "--summary-format",
        "json",
        "--summary-output",
        path_arg(&migrate_summary),
    ]);

    assert_policy_output(&migrated_policy);
    let value = assert_policy_migration_artifact_with_inventory(
        &migrate_summary,
        allow_report::MIGRATE_SCHEMA_ID,
        "migrate",
        "filesystem_fallback",
    );
    assert_eq!(
        value
            .pointer("/summary/allow_entries")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "migrate unsafe baseline-debt allow entry count"
    );
    assert_eq!(
        value
            .pointer("/summary/baseline_debt")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "migrate unsafe baseline-debt summary count"
    );
    assert_eq!(
        value
            .pointer("/summary/unsafe_entries")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "migrate unsafe baseline-debt unsafe entry count"
    );
    assert_eq!(
        value
            .pointer("/summary/evidence_entries")
            .and_then(serde_json::Value::as_u64),
        Some(2),
        "migrate unsafe baseline-debt evidence count"
    );
    assert_eq!(
        value
            .pointer("/summary/entries_with_links")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "migrate unsafe baseline-debt link-entry count"
    );
    assert_eq!(
        value
            .pointer("/summary/link_entries")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "migrate unsafe baseline-debt link count"
    );
    assert_eq!(
        value
            .pointer("/summary/broken_evidence_links")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "migrate unsafe baseline-debt broken evidence count"
    );
    assert_eq!(
        value
            .pointer("/summary/unsafe_broken_evidence_links")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "migrate unsafe baseline-debt unsafe broken evidence count"
    );
    assert_eq!(
        value
            .pointer("/summary/weak_evidence_references")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "migrate unsafe baseline-debt weak evidence count"
    );
    assert_eq!(
        value
            .pointer("/summary/unsafe_weak_evidence_references")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "migrate unsafe baseline-debt unsafe weak evidence count"
    );

    let follow_up_queues = value
        .pointer("/follow_up_queues")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| {
            std::panic::panic_any(
                "migrate summary should route unsafe baseline-debt follow-up queues",
            )
        });
    let [baseline_debt] = follow_up_queues.as_slice() else {
        panic!(
            "expected one migrate unsafe baseline-debt follow-up queue, got {}",
            follow_up_queues.len()
        );
    };
    assert_eq!(
        baseline_debt
            .get("signal")
            .and_then(serde_json::Value::as_str),
        Some("baseline_debt"),
        "migrate unsafe baseline-debt queue signal"
    );
    assert_eq!(
        baseline_debt
            .get("item_kind")
            .and_then(serde_json::Value::as_str),
        Some("baseline_debt"),
        "migrate unsafe baseline-debt queue item kind"
    );
    assert_eq!(
        baseline_debt
            .get("count")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "migrate unsafe baseline-debt queue count"
    );
    assert_eq!(
        baseline_debt
            .get("command")
            .and_then(serde_json::Value::as_str),
        Some("cargo-allow worklist --item-kind baseline_debt --format json"),
        "migrate unsafe baseline-debt queue command"
    );

    let queues = value
        .pointer("/evidence_repair_queues")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| {
            std::panic::panic_any("migrate summary should route unsafe evidence repair queues")
        });
    let broken = migrate_queue(queues, "broken_evidence_link");
    assert_eq!(
        broken
            .get("unsafe_count")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "migrate unsafe baseline-debt broken queue unsafe count"
    );
    assert_eq!(
        broken
            .get("unsafe_command")
            .and_then(serde_json::Value::as_str),
        Some("cargo-allow worklist --item-kind broken_evidence_link --kind unsafe --format json"),
        "migrate unsafe baseline-debt broken queue unsafe command"
    );
    let weak = migrate_queue(queues, "weak_evidence_reference");
    assert_eq!(
        weak.get("unsafe_count").and_then(serde_json::Value::as_u64),
        Some(1),
        "migrate unsafe baseline-debt weak queue unsafe count"
    );
    assert_eq!(
        weak.get("unsafe_command")
            .and_then(serde_json::Value::as_str),
        Some(
            "cargo-allow worklist --item-kind weak_evidence_reference --kind unsafe --format json"
        ),
        "migrate unsafe baseline-debt weak queue unsafe command"
    );

    let cfg = allow_policy::load_policy(&migrated_policy).unwrap_or_else(|err| {
        panic!(
            "load migrated policy {}: {err}",
            migrated_policy.display()
        )
    });
    let unsafe_entry = migrated_entry(&cfg, "unsafe-ffi-boundary");
    assert_eq!(unsafe_entry.kind, allow_core::FindingKind::Unsafe);
    assert_eq!(unsafe_entry.family.as_deref(), Some("unsafe_fn"));
    assert_eq!(unsafe_entry.owner, "unowned", "unsafe baseline-debt owner");
    assert_eq!(
        unsafe_entry.classification, "baseline_debt",
        "unsafe baseline-debt classification"
    );
    assert!(
        unsafe_entry
            .reason
            .contains("Generated from legacy unsafe allowlist; requires human review."),
        "unsafe baseline-debt reason should preserve generated review marker: {}",
        unsafe_entry.reason
    );
    assert!(
        unsafe_entry.lifecycle.created.is_some(),
        "unsafe baseline-debt migration should keep generated lifecycle creation date"
    );
    assert!(
        unsafe_entry.lifecycle.review_after.is_none(),
        "unsafe baseline-debt migration should not invent review_after"
    );
    assert!(
        unsafe_entry.lifecycle.expires.is_some(),
        "unsafe baseline-debt migration should keep generated expiry"
    );
    assert_eq!(unsafe_entry.path.as_deref(), Some(Path::new("src/lib.rs")));
    assert_eq!(
        unsafe_entry.selector.ast_kind.as_deref(),
        Some("unsafe_fn"),
        "unsafe baseline-debt selector ast kind"
    );
    assert_entry_evidence(
        unsafe_entry,
        &[
            "doc:docs/safety/missing-ffi.md",
            "TODO: add unsafe-review or boundary-test evidence",
        ],
    );
    assert_entry_links(unsafe_entry, &["legacy-policy:unsafe-ffi-boundary"]);
}

#[test]
fn saved_migrate_output_routes_baseline_debt_follow_up() {
    let fixture = SourceTreeFixture::new("saved-migrate-baseline-debt-follow-up");
    let legacy_dir = fixture.root.join("legacy-policy");
    fs::create_dir_all(&legacy_dir)
        .unwrap_or_else(|err| panic!("create legacy policy dir: {err}"));
    fs::write(
        legacy_dir.join("no-panic-baseline.toml"),
        no_panic_baseline_policy_fixture_text(),
    )
    .unwrap_or_else(|err| {
        panic!("write no-panic baseline policy fixture: {err}")
    });

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let migrated_policy = artifact_dir.join("allow.migrated.toml");
    let migrate_summary = artifact_dir.join("migrate-summary.json");

    run_cargo_allow(&[
        "migrate",
        "--root",
        fixture.root_str(),
        "--repo-policy",
        path_arg(&legacy_dir),
        "--out",
        path_arg(&migrated_policy),
        "--summary-format",
        "json",
        "--summary-output",
        path_arg(&migrate_summary),
    ]);

    assert_policy_output(&migrated_policy);
    let value = assert_policy_migration_artifact_with_inventory(
        &migrate_summary,
        allow_report::MIGRATE_SCHEMA_ID,
        "migrate",
        "filesystem_fallback",
    );
    assert_eq!(
        value
            .pointer("/summary/allow_entries")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "migrate baseline-debt allow entry count"
    );
    assert_eq!(
        value
            .pointer("/summary/baseline_debt")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "migrate baseline-debt summary count"
    );
    assert_eq!(
        value
            .pointer("/summary/unsafe_entries")
            .and_then(serde_json::Value::as_u64),
        Some(0),
        "migrate baseline-debt unsafe entry count"
    );
    assert_eq!(
        value
            .pointer("/summary/evidence_entries")
            .and_then(serde_json::Value::as_u64),
        Some(3),
        "migrate baseline-debt evidence count"
    );
    assert_eq!(
        value
            .pointer("/summary/entries_with_links")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "migrate baseline-debt link-entry count"
    );
    assert_eq!(
        value
            .pointer("/summary/link_entries")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "migrate baseline-debt link count"
    );
    let follow_up_queues = value
        .pointer("/follow_up_queues")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| {
            std::panic::panic_any("migrate summary should route baseline-debt follow-up queues")
        });
    let [baseline_debt] = follow_up_queues.as_slice() else {
        panic!(
            "expected one migrate baseline-debt follow-up queue, got {}",
            follow_up_queues.len()
        );
    };
    assert_eq!(
        baseline_debt
            .get("signal")
            .and_then(serde_json::Value::as_str),
        Some("baseline_debt"),
        "migrate baseline-debt queue signal"
    );
    assert_eq!(
        baseline_debt
            .get("route_kind")
            .and_then(serde_json::Value::as_str),
        Some("worklist_item_kind"),
        "migrate baseline-debt queue route kind"
    );
    assert_eq!(
        baseline_debt
            .get("item_kind")
            .and_then(serde_json::Value::as_str),
        Some("baseline_debt"),
        "migrate baseline-debt queue item kind"
    );
    assert_eq!(
        baseline_debt
            .get("count")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "migrate baseline-debt queue count"
    );
    assert_eq!(
        baseline_debt
            .get("command")
            .and_then(serde_json::Value::as_str),
        Some("cargo-allow worklist --item-kind baseline_debt --format json"),
        "migrate baseline-debt queue command"
    );

    let cfg = allow_policy::load_policy(&migrated_policy).unwrap_or_else(|err| {
        panic!(
            "load migrated policy {}: {err}",
            migrated_policy.display()
        )
    });
    let baseline = migrated_entry(&cfg, "panic-baseline-0001");
    assert_eq!(baseline.kind, allow_core::FindingKind::Panic);
    assert_eq!(baseline.family.as_deref(), Some("unwrap"));
    assert_eq!(baseline.owner, "unowned");
    assert_eq!(baseline.classification, "baseline_debt");
    assert_eq!(baseline.occurrence_limit, Some(2));
    assert_entry_evidence(
        baseline,
        &[
            "legacy_policy:no-panic-baseline",
            "legacy_selector_callee:Option/Result::unwrap",
            "baseline_count:2",
        ],
    );
    assert_entry_links(baseline, &["legacy-policy:no-panic-baseline"]);
}

fn process_policy_fixture_text() -> &'static str {
    r#"schema_version = 1
policy = "process-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "proc-bash-package-proof"
binary = "bash"
argv_shape = ["scripts/package-proof.sh"]
network_reach = false
called_by = [".github/workflows/release.yml"]
owner = "release"
reason = "Release preflight package proof; pure local checks."
created = "2026-05-09"
expires = "permanent"
"#
}

fn no_panic_baseline_policy_fixture_text() -> &'static str {
    r#"schema_version = 1
policy = "no-panic-baseline"
owner = "EffortlessMetrics"
status = "advisory"

[policy_config]
mode = "no-new-debt"

[[entry]]
path = "src/lib.rs"
family = "unwrap"
selector_kind = "method-call"
selector_callee = "Option/Result::unwrap"
snippet = "let value = maybe.unwrap();"
count = 2
"#
}

fn generated_policy_with_evidence_fixture_text() -> &'static str {
    r#"schema_version = 1
policy = "generated-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "saved-generated-evidence"
path = "docs/generated/schema.json"
generator = "cargo xtask schema"
regenerate_command = "cargo xtask schema"
owner = "policy"
reason = "Generated schema fixture."
evidence = ["doc:docs/generated/schema.md", "issue:#314"]
created = "2026-05-10"
expires = "permanent"
"#
}

fn executable_policy_with_covered_by_fixture_text() -> &'static str {
    r#"schema_version = 1
policy = "executable-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "saved-executable-covered"
path = "scripts/package.sh"
interpreter = "bash"
owner = "release"
reason = "Package helper fixture."
covered_by = "doc:docs/release/package.md"
created = "2026-05-09"
expires = "permanent"
"#
}

fn workflow_policy_with_evidence_fixture_text() -> &'static str {
    r#"schema_version = 1
policy = "workflow-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[entry]]
path = ".github/workflows/release.yml"
owner = "release/ci"
reason = "Release workflow fixture."
permissions = ["contents:read"]
secrets_used = ["RELEASE_TOKEN"]
external_actions = ["actions/checkout@v4"]
evidence = ["doc:docs/ci.md", "issue:#456"]
created = "2026-05-09"
expires = "permanent"
"#
}

fn dependency_policy_with_evidence_fixture_text() -> &'static str {
    r#"schema_version = 1
policy = "dependency-surface-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "saved-dependency-evidence"
path = "Cargo.toml"
surface = "workspace_manifest"
owner = "release"
reason = "Workspace dependency block fixture."
dep_count_at_baseline = 22
evidence = ["doc:docs/dependencies.md", "issue:#234"]
created = "2026-05-09"
expires = "permanent"
"#
}

fn process_policy_with_evidence_fixture_text() -> &'static str {
    r#"schema_version = 1
policy = "process-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "saved-process-evidence"
binary = "bash"
argv_shape = ["scripts/release.sh"]
network_reach = false
called_by = [".github/workflows/release.yml"]
owner = "release"
reason = "Release helper fixture."
evidence = ["doc:docs/release/process.md", "issue:#789"]
created = "2026-05-09"
expires = "permanent"
"#
}

fn network_policy_with_evidence_fixture_text() -> &'static str {
    r#"schema_version = 1
policy = "network-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "saved-network-evidence"
destination = "api.github.com"
auth_required = true
auth_secret = "GITHUB_TOKEN"
lane = "release"
owner = "release/ci"
reason = "Release API fixture."
evidence = ["doc:docs/network.md", "issue:#345"]
created = "2026-05-09"
expires = "permanent"
"#
}

fn non_rust_policy_with_evidence_fixture_text() -> &'static str {
    r#"schema_version = 1
policy = "non-rust-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "saved-non-rust-doc"
path = "docs/source-exception-ledger.md"
category = "documentation"
owner = "docs"
reason = "Repository policy prose fixture."
evidence = [
  "doc:docs/source-exception-ledger-evidence.md",
  "doc:docs/source-exception-ledger-review.md"
]
created = "2026-05-09"
review_after = "2026-09-09"

[[allow]]
id = "saved-non-rust-workflow"
path = ".github/workflows/ci.yml"
category = "ci_declarative"
owner = "release/ci"
reason = "Workflow file fixture."
covered_by = "doc:docs/ci-evidence.md"
created = "2026-05-09"
expires = "permanent"
"#
}

fn non_rust_policy_with_equal_specificity_rows_fixture_text(reverse: bool) -> String {
    let rows = if reverse {
        r#"[[allow]]
id = "z-broad"
glob = "docs/**"
category = "documentation"
owner = "docs-z"
reason = "Later duplicate row."
broad_glob_reason = "The docs tree is intentionally governed."
evidence = ["doc:migration-evidence.md"]
created = "2026-05-09"
review_after = "2026-09-09"

[[allow]]
id = "a-broad"
glob = "docs/**"
category = "documentation"
owner = "docs-a"
reason = "Stable duplicate row."
broad_glob_reason = "The docs tree is intentionally governed."
evidence = ["doc:migration-evidence.md"]
created = "2026-05-09"
review_after = "2026-09-09"
"#
    } else {
        r#"[[allow]]
id = "a-broad"
glob = "docs/**"
category = "documentation"
owner = "docs-a"
reason = "Stable duplicate row."
broad_glob_reason = "The docs tree is intentionally governed."
evidence = ["doc:migration-evidence.md"]
created = "2026-05-09"
review_after = "2026-09-09"

[[allow]]
id = "z-broad"
glob = "docs/**"
category = "documentation"
owner = "docs-z"
reason = "Later duplicate row."
broad_glob_reason = "The docs tree is intentionally governed."
evidence = ["doc:migration-evidence.md"]
created = "2026-05-09"
review_after = "2026-09-09"
"#
    };
    format!(
        r#"schema_version = 1
policy = "non-rust-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

{rows}"#
    )
}

fn clippy_policy_with_evidence_fixture_text() -> &'static str {
    r#"schema_version = 1
policy = "clippy-exceptions"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "saved-clippy-evidence"
path = "src/lib.rs"
lint = "clippy::unwrap_used"
owner = "runtime"
classification = "reviewed_lint_exception"
reason = "Intentional unwrap fixture for migration evidence parity."
evidence = ["test:lint_policy_is_linked", "issue:#123"]
created = "2026-05-09"
review_after = "2026-09-09"
"#
}

fn no_panic_allowlist_with_covered_by_fixture_text() -> &'static str {
    r#"schema_version = 1
policy = "no-panic-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "saved-no-panic-covered"
path = "src/lib.rs"
family = "unwrap"
owner = "runtime"
classification = "reviewed_panic_exception"
reason = "Parser validates optional value before unwrap."
covered_by = "test:parser_validates_optional_value"
created = "2026-05-09"
review_after = "2026-09-09"

[allow.selector]
kind = "method-call"
callee = "unwrap"
container = "load"
"#
}

fn unsafe_policy_with_evidence_fixture_text() -> &'static str {
    r#"schema_version = 1
policy = "unsafe-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "saved-unsafe-evidence"
path = "src/lib.rs"
family = "unsafe_block"
owner = "runtime"
classification = "reviewed_unsafe_boundary"
reason = "Caller validates pointer before read."
evidence = ["unsafe-review:docs/evidence/unsafe/read.json"]
created = "2026-05-09"
review_after = "2026-09-09"

[allow.selector]
kind = "unsafe-block"
container = "read"
"#
}

fn write_fixture_doc(root: &Path, relative_path: &str) {
    let path = root.join(relative_path);
    let parent = path
        .parent()
        .unwrap_or_else(|| panic!("fixture doc parent: {relative_path}"));
    fs::create_dir_all(parent)
        .unwrap_or_else(|err| panic!("create fixture doc dir: {err}"));
    fs::write(&path, "fixture evidence\n")
        .unwrap_or_else(|err| panic!("write fixture doc: {err}"));
}

fn migrated_entry<'a>(cfg: &'a allow_core::AllowConfig, id: &str) -> &'a allow_core::AllowEntry {
    cfg.allow
        .iter()
        .find(|entry| entry.id == id)
        .unwrap_or_else(|| panic!("missing migrated entry {id}"))
}

fn migrated_entry_by_path<'a>(
    cfg: &'a allow_core::AllowConfig,
    path: &Path,
) -> &'a allow_core::AllowEntry {
    cfg.allow
        .iter()
        .find(|entry| entry.path.as_deref() == Some(path))
        .unwrap_or_else(|| {
            panic!(
                "missing migrated entry for path {}; got {:?}",
                path.display(),
                cfg.allow
                    .iter()
                    .map(|entry| (&entry.id, entry.path.as_deref()))
                    .collect::<Vec<_>>()
            )
        })
}

fn migrate_queue<'a>(queues: &'a [serde_json::Value], item_kind: &str) -> &'a serde_json::Value {
    queues
        .iter()
        .find(|queue| queue.get("item_kind").and_then(serde_json::Value::as_str) == Some(item_kind))
        .unwrap_or_else(|| {
            panic!(
                "missing migrate evidence repair queue {item_kind}; got {queues:?}"
            )
        })
}

fn assert_entry_metadata(
    entry: &allow_core::AllowEntry,
    owner: &str,
    classification: &str,
    reason_fragment: &str,
    created: Option<&str>,
    review_after: Option<&str>,
    expires: Option<&str>,
) {
    assert_eq!(entry.owner, owner, "{} owner", entry.id);
    assert_eq!(
        entry.classification, classification,
        "{} classification",
        entry.id
    );
    assert!(
        entry.reason.contains(reason_fragment),
        "{} should preserve reason fragment `{}` in `{}`",
        entry.id,
        reason_fragment,
        entry.reason
    );
    assert_eq!(
        entry.lifecycle.created.as_deref(),
        created,
        "{} created",
        entry.id
    );
    assert_eq!(
        entry.lifecycle.review_after.as_deref(),
        review_after,
        "{} review_after",
        entry.id
    );
    assert_eq!(
        entry.lifecycle.expires.as_deref(),
        expires,
        "{} expires",
        entry.id
    );
}

fn assert_entry_evidence(entry: &allow_core::AllowEntry, expected: &[&str]) {
    for value in expected {
        assert!(
            entry.evidence.iter().any(|item| item == value),
            "{} should preserve evidence `{value}` in {:?}",
            entry.id,
            entry.evidence
        );
    }
}

fn assert_entry_links(entry: &allow_core::AllowEntry, expected: &[&str]) {
    for value in expected {
        assert!(
            entry.links.iter().any(|item| item == value),
            "{} should preserve link `{value}` in {:?}",
            entry.id,
            entry.links
        );
    }
}

fn unsafe_policy_fixture_text() -> &'static str {
    r#"schema_version = 1
policy = "unsafe-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "unsafe-ffi-boundary"
path = "src/lib.rs"
family = "unsafe_fn"
kind = "unsafe-fn"
evidence = [
  "doc:docs/safety/missing-ffi.md",
  "TODO: add unsafe-review or boundary-test evidence"
]

[allow.selector]
kind = "unsafe-fn"
"#
}
