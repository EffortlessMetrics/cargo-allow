use allow_core::FindingKind;
use allow_policy_legacy::{
    all_legacy_lane_descriptors, generated_findings_from_gitattributes_text,
    legacy_policy_source_for_path, migration_debt_classes, workflow_findings_from_sources,
    DebtPolicy, MigrationDebtClass,
};
use std::path::{Path, PathBuf};

#[test]
fn public_finding_facade_keeps_file_surface_families_available() {
    let generated = generated_findings_from_gitattributes_text(
        "generated/schema.json linguist-generated=true\nREADME.md linguist-documentation=true\n",
    );
    assert_eq!(generated.len(), 1);
    assert_eq!(generated[0].kind, FindingKind::GeneratedCode);
    assert_eq!(generated[0].path, PathBuf::from("generated/schema.json"));

    let workflow = workflow_findings_from_sources(vec![
        (
            PathBuf::from(".github/workflows/ci.yml"),
            "steps:\n  - uses: actions/checkout@v4\n".to_string(),
        ),
    ]);
    assert!(workflow.iter().any(|finding| {
        finding.family.as_deref() == Some("github_workflow")
            && finding.path == Path::new(".github/workflows/ci.yml")
    }));
    assert!(workflow.iter().any(|finding| {
        finding.family.as_deref() == Some("workflow_external_action")
            && finding.identity.target_fingerprint.as_deref() == Some("action:actions/checkout@v4")
    }));
}

#[test]
fn public_legacy_source_lookup_rejects_unknown_files() {
    let source = legacy_policy_source_for_path(Path::new("policy/allowlist.toml"));
    assert!(source.is_none());

    let source = legacy_policy_source_for_path(Path::new("policy/generated-allowlist.toml"))
        .unwrap_or_else(|| std::panic::panic_any("generated legacy source should be recognized"));
    assert_eq!(source.file_name, "generated-allowlist.toml");
    assert_eq!(source.compat_kind, "generated");
}

#[test]
fn public_lane_descriptors_have_unique_ids_and_explicit_debt_policy() {
    let descriptors = all_legacy_lane_descriptors();
    assert!(!descriptors.is_empty());

    for (index, descriptor) in descriptors.iter().enumerate() {
        assert!(!descriptor.compat_kind_id().is_empty());
        assert!(!descriptor.legacy_filename.is_empty());
        assert_eq!(
            migration_debt_classes(descriptor).is_empty(),
            descriptor.debt_policy == DebtPolicy::None
        );

        for other in descriptors.iter().skip(index + 1) {
            assert_ne!(
                descriptor.compat_kind_id(),
                other.compat_kind_id(),
                "compatibility kind ids must remain unique"
            );
        }
    }

    let has_missing_evidence_lane = descriptors.iter().any(|descriptor| {
        migration_debt_classes(descriptor) == [MigrationDebtClass::MissingEvidence]
    });
    assert!(has_missing_evidence_lane);
}
