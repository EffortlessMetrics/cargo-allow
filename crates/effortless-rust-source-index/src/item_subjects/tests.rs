use super::*;

fn item(id: &str, module: &[&str]) -> RustItemSubjectV1 {
    RustItemSubjectV1 {
        schema_version: RUST_ITEM_SUBJECT_SCHEMA_VERSION.into(),
        subject_id: RustItemSubjectIdV1::new(id),
        repository_id: "repo".into(),
        snapshot_id: "tree:abc".into(),
        generation_identity: "generation:1".into(),
        target: RustItemTargetIdentityV1 {
            package: "demo".into(),
            crate_name: "demo".into(),
            kind: RustItemTargetKindV1::Library,
            name: "demo".into(),
        },
        module_path: module.iter().map(|segment| (*segment).into()).collect(),
        item_path: vec!["run".into()],
        definition_kind: RustItemDefinitionKindV1::Function,
        source: Some(RustItemSourceIdentityV1 {
            source_path: format!("src/{}.rs", module.join("_")),
            declaration_range: RustSourceRangeV1 {
                start_line: 1,
                start_column: 1,
                end_line: 3,
                end_column: 2,
            },
            identifier_range: None,
            declaration_identity: format!("decl:{id}"),
            signature_identity: Some(format!("sig:{id}")),
            body_identity: Some(format!("body:{id}")),
        }),
        container_subject_id: None,
        visibility: RustVisibilityShapeV1::Private,
        cfg_expressions: Vec::new(),
        lint_declarations: Vec::new(),
        source_available: true,
        generated_or_macro_owned: false,
        limitations: Vec::new(),
    }
}

fn lint(id: &str, item_id: &str) -> RustLintDeclarationSubjectV1 {
    RustLintDeclarationSubjectV1 {
        schema_version: RUST_ITEM_SUBJECT_SCHEMA_VERSION.into(),
        subject_id: RustLintDeclarationSubjectIdV1::new(id),
        item_subject_id: RustItemSubjectIdV1::new(item_id),
        family: RustLintDeclarationFamilyV1::Expect,
        lint_names: vec!["dead_code".into()],
        source_range: RustSourceRangeV1 {
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 20,
        },
        cfg_expression: None,
        conditional_source_only: false,
    }
}

fn inventory(
    status: RustItemInventoryStatusV1,
    subjects: Vec<RustItemSubjectV1>,
) -> RustItemInventoryV1 {
    RustItemInventoryV1 {
        schema_version: RUST_ITEM_SUBJECT_SCHEMA_VERSION.into(),
        repository_id: "repo".into(),
        snapshot_id: "tree:abc".into(),
        generation_identity: "generation:1".into(),
        status,
        subjects,
        diagnostics: Vec::new(),
    }
}

fn selector() -> RustItemSelectorV1 {
    RustItemSelectorV1 {
        repository_id: Some("repo".into()),
        snapshot_id: Some("tree:abc".into()),
        package: Some("demo".into()),
        crate_name: Some("demo".into()),
        target_kind: Some(RustItemTargetKindV1::Library),
        target_name: Some("demo".into()),
        item_path: Some(vec!["run".into()]),
        module_path: Some(vec!["left".into()]),
        definition_kind: Some(RustItemDefinitionKindV1::Function),
        generation_identity: Some("generation:1".into()),
        ..Default::default()
    }
}

#[test]
fn exact_resolution_requires_structural_identity() {
    let inventory = inventory(
        RustItemInventoryStatusV1::Complete,
        vec![item("left", &["left"]), item("right", &["right"])],
    );
    let ambiguous = resolve_rust_item_subject(
        &inventory,
        &RustItemSelectorV1 {
            item_path: Some(vec!["run".into()]),
            definition_kind: Some(RustItemDefinitionKindV1::Function),
            generation_identity: Some("generation:1".into()),
            ..Default::default()
        },
    );
    assert_eq!(
        ambiguous.class,
        RustItemResolutionClassV1::MalformedSelector
    );

    let exact = resolve_rust_item_subject(
        &inventory,
        &RustItemSelectorV1 {
            module_path: Some(vec!["right".into()]),
            item_path: Some(vec!["run".into()]),
            definition_kind: Some(RustItemDefinitionKindV1::Function),
            generation_identity: Some("generation:1".into()),
            repository_id: Some("repo".into()),
            snapshot_id: Some("tree:abc".into()),
            package: Some("demo".into()),
            crate_name: Some("demo".into()),
            target_kind: Some(RustItemTargetKindV1::Library),
            target_name: Some("demo".into()),
            ..Default::default()
        },
    );
    assert_eq!(exact.class, RustItemResolutionClassV1::Exact);
    assert_eq!(exact.candidate_ids, vec![RustItemSubjectIdV1::new("right")]);
}

#[test]
fn inventory_status_controls_missing_authority() {
    let partial = resolve_rust_item_subject(
        &inventory(RustItemInventoryStatusV1::Partial, Vec::new()),
        &RustItemSelectorV1 {
            repository_id: Some("repo".into()),
            snapshot_id: Some("tree:abc".into()),
            package: Some("demo".into()),
            crate_name: Some("demo".into()),
            target_kind: Some(RustItemTargetKindV1::Library),
            target_name: Some("demo".into()),
            item_path: Some(vec!["missing".into()]),
            module_path: Some(vec!["missing".into()]),
            definition_kind: Some(RustItemDefinitionKindV1::Function),
            generation_identity: Some("generation:1".into()),
            ..Default::default()
        },
    );
    assert_eq!(partial.class, RustItemResolutionClassV1::Partial);

    let complete = resolve_rust_item_subject(
        &inventory(RustItemInventoryStatusV1::Complete, Vec::new()),
        &RustItemSelectorV1 {
            repository_id: Some("repo".into()),
            snapshot_id: Some("tree:abc".into()),
            package: Some("demo".into()),
            crate_name: Some("demo".into()),
            target_kind: Some(RustItemTargetKindV1::Library),
            target_name: Some("demo".into()),
            item_path: Some(vec!["missing".into()]),
            module_path: Some(vec!["missing".into()]),
            definition_kind: Some(RustItemDefinitionKindV1::Function),
            generation_identity: Some("generation:1".into()),
            ..Default::default()
        },
    );
    assert_eq!(
        complete.class,
        RustItemResolutionClassV1::MissingWithinCompleteScope
    );
}

#[test]
fn partial_inventory_never_resolves_an_existing_item_exactly() {
    let subject = item("partial", &["partial"]);
    let result = resolve_rust_item_subject(
        &inventory(RustItemInventoryStatusV1::Partial, vec![subject]),
        &selector(),
    );
    assert_eq!(result.class, RustItemResolutionClassV1::Partial);
    assert!(result.subjects.is_empty());
}

#[test]
fn container_links_require_existing_noncyclic_subjects() {
    let mut child = item("child", &["child"]);
    child.container_subject_id = Some(RustItemSubjectIdV1::new("missing"));
    assert!(!inventory(RustItemInventoryStatusV1::Complete, vec![child]).validate());

    let mut self_link = item("self", &["self"]);
    self_link.container_subject_id = Some(RustItemSubjectIdV1::new("self"));
    assert!(!inventory(RustItemInventoryStatusV1::Complete, vec![self_link]).validate());

    let mut first = item("first", &["first"]);
    let mut second = item("second", &["second"]);
    let mut third = item("third", &["third"]);
    first.container_subject_id = Some(RustItemSubjectIdV1::new("third"));
    second.container_subject_id = Some(RustItemSubjectIdV1::new("first"));
    third.container_subject_id = Some(RustItemSubjectIdV1::new("second"));
    assert!(
        !inventory(
            RustItemInventoryStatusV1::Complete,
            vec![first, second, third]
        )
        .validate()
    );
}

#[test]
fn malformed_selector_fields_fail_closed() {
    let malformed = [
        RustItemSelectorV1 {
            item_path: Some(Vec::new()),
            ..selector()
        },
        RustItemSelectorV1 {
            item_path: Some(vec![" ".into()]),
            ..selector()
        },
        RustItemSelectorV1 {
            module_path: Some(vec!["".into()]),
            ..selector()
        },
        RustItemSelectorV1 {
            package: Some(" ".into()),
            ..selector()
        },
        RustItemSelectorV1 {
            source_path: Some(String::new()),
            ..selector()
        },
    ];
    let inventory = inventory(
        RustItemInventoryStatusV1::Complete,
        vec![item("one", &["one"])],
    );
    for selector in malformed {
        assert_eq!(
            resolve_rust_item_subject(&inventory, &selector).class,
            RustItemResolutionClassV1::MalformedSelector
        );
    }
}

#[test]
fn exact_selector_binds_target_identity() {
    let mut binary = item("bin", &["same"]);
    binary.target.kind = RustItemTargetKindV1::Binary;
    binary.target.name = "demo-bin".into();
    let candidate = inventory(
        RustItemInventoryStatusV1::Complete,
        vec![item("lib", &["left"]), {
            binary.module_path = vec!["left".into()];
            binary
        }],
    );
    let under_targeted = resolve_rust_item_subject(
        &candidate,
        &RustItemSelectorV1 {
            target_name: None,
            ..selector()
        },
    );
    assert_eq!(
        under_targeted.class,
        RustItemResolutionClassV1::MalformedSelector
    );
    let exact = resolve_rust_item_subject(
        &candidate,
        &RustItemSelectorV1 {
            target_kind: Some(RustItemTargetKindV1::Binary),
            target_name: Some("demo-bin".into()),
            crate_name: Some("demo".into()),
            ..selector()
        },
    );
    assert_eq!(exact.class, RustItemResolutionClassV1::Exact);
    let stale = inventory(
        RustItemInventoryStatusV1::Complete,
        vec![item("stale", &["same"])],
    );
    let mismatch = resolve_rust_item_subject(
        &stale,
        &RustItemSelectorV1 {
            generation_identity: Some("generation:2".into()),
            ..selector()
        },
    );
    assert_eq!(mismatch.class, RustItemResolutionClassV1::NotProven);
}

#[test]
fn inventory_rejects_subject_generation_substitution() {
    let mut stale = inventory(
        RustItemInventoryStatusV1::Complete,
        vec![item("stale", &["same"])],
    );
    if let Some(subject) = stale.subjects.first_mut() {
        subject.generation_identity = "generation:2".into();
    }
    assert!(!stale.validate());
    assert_eq!(
        resolve_rust_item_subject(&stale, &selector()).class,
        RustItemResolutionClassV1::NotProven
    );
}

#[test]
fn selector_inventory_identity_mismatch_is_not_proven() {
    let inventory = inventory(
        RustItemInventoryStatusV1::Complete,
        vec![item("current", &["left"])],
    );
    for mismatched in [
        RustItemSelectorV1 {
            repository_id: Some("other-repo".into()),
            ..selector()
        },
        RustItemSelectorV1 {
            snapshot_id: Some("tree:def".into()),
            ..selector()
        },
        RustItemSelectorV1 {
            generation_identity: Some("generation:2".into()),
            ..selector()
        },
    ] {
        assert_eq!(
            resolve_rust_item_subject(&inventory, &mismatched).class,
            RustItemResolutionClassV1::NotProven
        );
    }
}

#[test]
fn one_record_target_substitution_is_never_exact() {
    let inventory = inventory(
        RustItemInventoryStatusV1::Complete,
        vec![item("current", &["left"])],
    );
    for substituted in [
        RustItemSelectorV1 {
            package: Some("other-package".into()),
            ..selector()
        },
        RustItemSelectorV1 {
            crate_name: Some("other_crate".into()),
            ..selector()
        },
        RustItemSelectorV1 {
            target_kind: Some(RustItemTargetKindV1::Binary),
            ..selector()
        },
        RustItemSelectorV1 {
            target_name: Some("other-target".into()),
            ..selector()
        },
    ] {
        assert_ne!(
            resolve_rust_item_subject(&inventory, &substituted).class,
            RustItemResolutionClassV1::Exact
        );
    }
}

#[test]
fn generated_cfg_and_source_unavailable_items_are_not_exact() {
    let mut generated = item("generated", &["generated"]);
    generated.generated_or_macro_owned = true;
    generated.source = None;
    assert_eq!(
        resolve_rust_item_subject(
            &inventory(RustItemInventoryStatusV1::Complete, vec![generated]),
            &RustItemSelectorV1 {
                module_path: Some(vec!["generated".into()]),
                ..selector()
            },
        )
        .class,
        RustItemResolutionClassV1::GeneratedOrMacroOwned
    );

    let mut cfg = item("cfg", &["cfg"]);
    cfg.cfg_expressions.push("feature = \"special\"".into());
    assert_eq!(
        resolve_rust_item_subject(
            &inventory(RustItemInventoryStatusV1::Complete, vec![cfg]),
            &RustItemSelectorV1 {
                module_path: Some(vec!["cfg".into()]),
                ..selector()
            },
        )
        .class,
        RustItemResolutionClassV1::CfgOrFeatureUnknown
    );

    let mut unavailable = item("unavailable", &["unavailable"]);
    unavailable.source_available = false;
    unavailable.source = None;
    assert_eq!(
        resolve_rust_item_subject(
            &inventory(RustItemInventoryStatusV1::Complete, vec![unavailable]),
            &RustItemSelectorV1 {
                module_path: Some(vec!["unavailable".into()]),
                ..selector()
            },
        )
        .class,
        RustItemResolutionClassV1::SourceUnavailable
    );
}

#[test]
fn root_module_is_a_valid_structural_identity() {
    let root = inventory(RustItemInventoryStatusV1::Complete, vec![item("root", &[])]);
    let result = resolve_rust_item_subject(
        &root,
        &RustItemSelectorV1 {
            module_path: Some(Vec::new()),
            ..selector()
        },
    );
    assert_eq!(result.class, RustItemResolutionClassV1::Exact);
}

#[test]
fn diagnostics_and_limitations_never_resolve_exactly() {
    let mut diagnostic_inventory = inventory(
        RustItemInventoryStatusV1::Complete,
        vec![item("diagnostic", &["diagnostic"])],
    );
    diagnostic_inventory
        .diagnostics
        .push("incomplete source".into());
    assert!(!diagnostic_inventory.validate());
    assert_eq!(
        resolve_rust_item_subject(&diagnostic_inventory, &selector()).class,
        RustItemResolutionClassV1::NotProven
    );

    let mut limited = item("limited", &["limited"]);
    limited.limitations.push("unresolved cfg".into());
    assert_eq!(
        resolve_rust_item_subject(
            &inventory(RustItemInventoryStatusV1::Complete, vec![limited]),
            &RustItemSelectorV1 {
                module_path: Some(vec!["limited".into()]),
                ..selector()
            },
        )
        .class,
        RustItemResolutionClassV1::NotProven
    );

    let mut generated_with_context = item("generated", &["generated"]);
    generated_with_context.generated_or_macro_owned = true;
    generated_with_context
        .limitations
        .push("macro expansion is not source-owned".into());
    assert_eq!(
        resolve_rust_item_subject(
            &inventory(
                RustItemInventoryStatusV1::Complete,
                vec![generated_with_context]
            ),
            &RustItemSelectorV1 {
                module_path: Some(vec!["generated".into()]),
                ..selector()
            },
        )
        .class,
        RustItemResolutionClassV1::GeneratedOrMacroOwned
    );
}

#[test]
fn count_preserving_substitution_changes_identity() {
    let first = item("first", &["left"]);
    let second = item("second", &["left"]);
    assert_ne!(first.subject_id, second.subject_id);
    assert_ne!(
        first
            .source
            .as_ref()
            .map(|source| &source.declaration_identity),
        second
            .source
            .as_ref()
            .map(|source| &source.declaration_identity)
    );
}

#[test]
fn malformed_selector_is_rejected_before_inventory_use() {
    let result = resolve_rust_item_subject(
        &inventory(
            RustItemInventoryStatusV1::Complete,
            vec![item("one", &["one"])],
        ),
        &RustItemSelectorV1::default(),
    );
    assert_eq!(result.class, RustItemResolutionClassV1::MalformedSelector);
}

#[test]
fn inventory_validation_enforces_snapshot_and_unique_subject_identity() {
    let subject = item("same", &["one"]);
    assert!(
        !inventory(
            RustItemInventoryStatusV1::Complete,
            vec![subject.clone(), subject],
        )
        .validate()
    );

    let mut wrong_snapshot = item("snapshot", &["snapshot"]);
    wrong_snapshot.snapshot_id = "tree:def".into();
    assert!(!inventory(RustItemInventoryStatusV1::Complete, vec![wrong_snapshot],).validate());

    let mut wrong_repository = item("repository", &["repository"]);
    wrong_repository.repository_id = "other".into();
    assert!(!inventory(RustItemInventoryStatusV1::Complete, vec![wrong_repository],).validate());
}

#[test]
fn inventory_validation_enforces_nested_lint_identity() {
    let mut invalid_schema = item("one", &["one"]);
    let mut declaration = lint("lint-one", "one");
    declaration.schema_version = "rust_item_subject.v2".into();
    invalid_schema.lint_declarations.push(declaration);
    assert!(!inventory(RustItemInventoryStatusV1::Complete, vec![invalid_schema],).validate());

    let mut misparented = item("two", &["two"]);
    misparented
        .lint_declarations
        .push(lint("lint-two", "other"));
    assert!(!inventory(RustItemInventoryStatusV1::Complete, vec![misparented],).validate());

    let mut empty_lint = item("three", &["three"]);
    let mut declaration = lint("lint-three", "three");
    declaration.lint_names.clear();
    empty_lint.lint_declarations.push(declaration);
    assert!(!inventory(RustItemInventoryStatusV1::Complete, vec![empty_lint],).validate());

    let mut first = item("first", &["first"]);
    first.lint_declarations.push(lint("shared", "first"));
    let mut second = item("second", &["second"]);
    second.lint_declarations.push(lint("shared", "second"));
    assert!(!inventory(RustItemInventoryStatusV1::Complete, vec![first, second],).validate());
}

#[test]
fn invalid_ranges_and_invalid_inventory_fail_closed() {
    let mut invalid_range = item("range", &["range"]);
    if let Some(source) = invalid_range.source.as_mut() {
        source.declaration_range.end_line = 0;
    }
    assert!(!inventory(RustItemInventoryStatusV1::Complete, vec![invalid_range],).validate());

    let mut stale = item("stale", &["stale"]);
    stale.snapshot_id = "tree:def".into();
    let result = resolve_rust_item_subject(
        &inventory(RustItemInventoryStatusV1::Complete, vec![stale]),
        &selector(),
    );
    assert_eq!(result.class, RustItemResolutionClassV1::NotProven);
    assert!(result.subjects.is_empty());
}
