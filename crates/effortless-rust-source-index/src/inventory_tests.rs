use crate::{
    RustTestInventory, RustTestInventoryOptions, RustTestInventoryStatus, RustTestResolution,
    RustTestSubject, RustTestTargetIdentity, RustTestTargetKind,
    inventory_rust_test_subjects_from_sources, resolve_rust_test_selector,
};
use std::collections::BTreeSet;
use std::path::PathBuf;

fn manifest() -> Vec<(PathBuf, String)> {
    vec![(
        PathBuf::from("crates/demo/Cargo.toml"),
        "[package]\nname = \"demo-package\"\nversion = \"0.1.0\"\n".to_string(),
    )]
}

fn only_subject(inventory: &RustTestInventory) -> Result<&RustTestSubject, String> {
    if inventory.subjects.len() != 1 {
        return Err(format!(
            "expected one subject, found {}",
            inventory.subjects.len()
        ));
    }
    inventory
        .subjects
        .first()
        .ok_or_else(|| "expected one exact source-declared test subject".to_string())
}

#[test]
fn inventories_nested_inline_unit_test() -> Result<(), String> {
    let inventory = inventory_rust_test_subjects_from_sources(
        manifest(),
        vec![(
            PathBuf::from("crates/demo/src/lib.rs"),
            "#[cfg(test)]\nmod policy_tests { #[test] fn rejects_boundary() { assert_eq!(2 + 2, 4); } }"
                .to_string(),
        )],
        &RustTestInventoryOptions::default(),
    );
    let subject = only_subject(&inventory)?;
    assert_eq!(inventory.status, RustTestInventoryStatus::Complete);
    assert_eq!(subject.selector.package, "demo-package");
    assert_eq!(subject.selector.target.kind, RustTestTargetKind::Library);
    assert_eq!(subject.selector.target.name, "demo_package");
    assert_eq!(subject.selector.module_path, vec!["policy_tests"]);
    assert_eq!(subject.selector.function, "rejects_boundary");
    assert!(!subject.cfg_or_feature_unknown);
    Ok(())
}

#[test]
fn same_named_integration_tests_have_distinct_targets() -> Result<(), String> {
    let inventory = inventory_rust_test_subjects_from_sources(
        manifest(),
        vec![
            (
                PathBuf::from("crates/demo/tests/alpha.rs"),
                "#[test]\nfn roundtrip() {}\n".to_string(),
            ),
            (
                PathBuf::from("crates/demo/tests/beta.rs"),
                "#[test]\nfn roundtrip() {}\n".to_string(),
            ),
        ],
        &RustTestInventoryOptions::default(),
    );
    let mut subjects = inventory.subjects.iter();
    let first = subjects
        .next()
        .ok_or_else(|| "expected first integration subject".to_string())?;
    let second = subjects
        .next()
        .ok_or_else(|| "expected second integration subject".to_string())?;
    assert!(subjects.next().is_none());
    assert_ne!(first.selector.target.name, second.selector.target.name);
    Ok(())
}

#[test]
fn explicit_and_auto_binary_targets_with_same_identity_are_deduplicated() -> Result<(), String> {
    let inventory = inventory_rust_test_subjects_from_sources(
        vec![(
            PathBuf::from("crates/demo/Cargo.toml"),
            "[package]\nname = \"demo-package\"\nversion = \"0.1.0\"\n\n[[bin]]\nname = \"demo_package\"\npath = \"src/bin/demo_package.rs\"\n".to_string(),
        )],
        vec![(
            PathBuf::from("crates/demo/src/bin/demo_package.rs"),
            "#[test]\nfn bin_test() {}".into(),
        )],
        &RustTestInventoryOptions::default(),
    );
    let subject = only_subject(&inventory)?;
    assert_eq!(inventory.status, RustTestInventoryStatus::Complete);
    assert_eq!(
        subject.selector.target,
        RustTestTargetIdentity {
            kind: RustTestTargetKind::Binary,
            name: "demo_package".to_string(),
        }
    );
    Ok(())
}

#[test]
fn changed_body_changes_identity_without_changing_selector() -> Result<(), String> {
    let path = PathBuf::from("crates/demo/src/lib.rs");
    let first = inventory_rust_test_subjects_from_sources(
        manifest(),
        vec![(
            path.clone(),
            "#[test]\nfn exact() { assert_eq!(1, 1); }".into(),
        )],
        &RustTestInventoryOptions::default(),
    );
    let second = inventory_rust_test_subjects_from_sources(
        manifest(),
        vec![(path, "#[test]\nfn exact() { assert_eq!(1, 2); }".into())],
        &RustTestInventoryOptions::default(),
    );
    let first_subject = only_subject(&first)?;
    let second_subject = only_subject(&second)?;
    assert_eq!(first_subject.selector, second_subject.selector);
    assert_ne!(first_subject.body_identity, second_subject.body_identity);
    Ok(())
}

#[test]
fn cfg_limited_test_resolves_as_unknown() -> Result<(), String> {
    let inventory = inventory_rust_test_subjects_from_sources(
        manifest(),
        vec![(
            PathBuf::from("crates/demo/src/lib.rs"),
            "#[cfg(feature = \"special\")]\n#[test]\nfn gated() {}".into(),
        )],
        &RustTestInventoryOptions::default(),
    );
    let selector = only_subject(&inventory)?.selector.clone();
    assert!(matches!(
        resolve_rust_test_selector(&inventory, &selector),
        RustTestResolution::CfgOrFeatureUnknown(_)
    ));
    Ok(())
}

#[test]
fn configured_parameterized_test_stays_non_exact() -> Result<(), String> {
    let mut options = RustTestInventoryOptions::default();
    options.additional_test_attributes.insert("rstest".into());
    let inventory = inventory_rust_test_subjects_from_sources(
        manifest(),
        vec![(
            PathBuf::from("crates/demo/src/lib.rs"),
            "#[rstest]\nfn cases(#[case] value: u32) { assert!(value > 0); }".into(),
        )],
        &options,
    );
    let selector = only_subject(&inventory)?.selector.clone();
    assert!(matches!(
        resolve_rust_test_selector(&inventory, &selector),
        RustTestResolution::GeneratedOrParameterized(_)
    ));
    Ok(())
}

#[test]
fn ignored_test_is_not_an_executable_subject() -> Result<(), String> {
    let inventory = inventory_rust_test_subjects_from_sources(
        manifest(),
        vec![(
            PathBuf::from("crates/demo/src/lib.rs"),
            "#[test]\n#[ignore]\nfn deferred() {}".into(),
        )],
        &RustTestInventoryOptions::default(),
    );
    let selector = only_subject(&inventory)?.selector.clone();
    assert!(matches!(
        resolve_rust_test_selector(&inventory, &selector),
        RustTestResolution::Ignored(_)
    ));
    Ok(())
}

#[test]
fn partial_inventory_never_resolves_exactly() -> Result<(), String> {
    let inventory = inventory_rust_test_subjects_from_sources(
        manifest(),
        vec![
            (
                PathBuf::from("crates/demo/src/lib.rs"),
                "#[test]\nfn exact() {}".into(),
            ),
            (
                PathBuf::from("crates/demo/src/broken.rs"),
                "fn broken( {".into(),
            ),
        ],
        &RustTestInventoryOptions::default(),
    );
    let selector = inventory
        .subjects
        .first()
        .ok_or_else(|| "expected discovered subject".to_string())?
        .selector
        .clone();
    assert_eq!(inventory.status, RustTestInventoryStatus::Partial);
    assert_eq!(
        resolve_rust_test_selector(&inventory, &selector),
        RustTestResolution::PartialInventory
    );
    Ok(())
}

#[test]
fn workspace_only_manifest_is_ignored_without_partial_status() {
    let manifests = vec![
        (
            PathBuf::from("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/demo\"]\n".to_string(),
        ),
        manifest().remove(0),
    ];
    let inventory = inventory_rust_test_subjects_from_sources(
        manifests,
        vec![(
            PathBuf::from("crates/demo/src/lib.rs"),
            "#[test]\nfn exact() {}".into(),
        )],
        &RustTestInventoryOptions::default(),
    );
    assert_eq!(inventory.status, RustTestInventoryStatus::Complete);
    assert_eq!(inventory.subjects.len(), 1);
}

#[test]
fn bom_prefixed_source_is_parsed_normally() {
    let inventory = inventory_rust_test_subjects_from_sources(
        manifest(),
        vec![(
            PathBuf::from("crates/demo/src/lib.rs"),
            "\u{feff}#[test]\nfn exact() {}".into(),
        )],
        &RustTestInventoryOptions::default(),
    );
    assert_eq!(inventory.status, RustTestInventoryStatus::Complete);
    assert_eq!(inventory.subjects.len(), 1);
}

#[test]
fn binary_only_helper_is_not_fabricated_as_library() -> Result<(), String> {
    let binary_manifest = vec![(
        PathBuf::from("crates/demo/Cargo.toml"),
        "[package]\nname = \"demo-package\"\nversion = \"0.1.0\"\n".to_string(),
    )];
    let inventory = inventory_rust_test_subjects_from_sources(
        binary_manifest,
        vec![
            (
                PathBuf::from("crates/demo/src/main.rs"),
                "mod helper; fn main() {}".into(),
            ),
            (
                PathBuf::from("crates/demo/src/helper.rs"),
                "#[test]\nfn helper_test() {}".into(),
            ),
        ],
        &RustTestInventoryOptions::default(),
    );
    let subject = only_subject(&inventory)?;
    assert_eq!(subject.selector.target.kind, RustTestTargetKind::Binary);
    assert!(
        subject
            .limitations
            .iter()
            .any(|limitation| { limitation.contains("cannot prove the module is included") })
    );
    Ok(())
}

#[test]
fn shared_src_parent_is_not_claimed_when_lib_and_bin_both_exist() {
    let inventory = inventory_rust_test_subjects_from_sources(
        manifest(),
        vec![
            (
                PathBuf::from("crates/demo/src/lib.rs"),
                "pub mod helper;".into(),
            ),
            (
                PathBuf::from("crates/demo/src/main.rs"),
                "mod helper; fn main() {}".into(),
            ),
            (
                PathBuf::from("crates/demo/src/helper.rs"),
                "#[test]\nfn ambiguous_owner() {}".into(),
            ),
        ],
        &RustTestInventoryOptions::default(),
    );
    assert!(inventory.subjects.is_empty());
    assert_eq!(inventory.status, RustTestInventoryStatus::Partial);
}

#[test]
fn integration_target_does_not_claim_tests_common_module() {
    let inventory = inventory_rust_test_subjects_from_sources(
        manifest(),
        vec![
            (
                PathBuf::from("crates/demo/tests/alpha.rs"),
                "mod common; #[test] fn alpha() {}".into(),
            ),
            (
                PathBuf::from("crates/demo/tests/common/mod.rs"),
                "#[test]\nfn helper_only() {}".into(),
            ),
        ],
        &RustTestInventoryOptions::default(),
    );
    assert_eq!(inventory.subjects.len(), 1);
    assert_eq!(inventory.status, RustTestInventoryStatus::Partial);
}

#[test]
fn split_library_module_has_stable_module_path() -> Result<(), String> {
    let inventory = inventory_rust_test_subjects_from_sources(
        manifest(),
        vec![
            (
                PathBuf::from("crates/demo/src/lib.rs"),
                "mod policy;".into(),
            ),
            (
                PathBuf::from("crates/demo/src/policy.rs"),
                "#[test]\nfn rejects_boundary() {}".into(),
            ),
        ],
        &RustTestInventoryOptions::default(),
    );
    let subject = only_subject(&inventory)?;
    assert_eq!(subject.selector.module_path, vec!["policy"]);
    Ok(())
}

#[test]
fn duplicate_function_names_in_modules_have_distinct_selectors() {
    let inventory = inventory_rust_test_subjects_from_sources(
        manifest(),
        vec![(
            PathBuf::from("crates/demo/src/lib.rs"),
            "mod alpha { #[test] fn same() {} } mod beta { #[test] fn same() {} }".into(),
        )],
        &RustTestInventoryOptions::default(),
    );
    let module_paths = inventory
        .subjects
        .iter()
        .map(|subject| subject.selector.module_path.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(module_paths.len(), 2);
}

#[test]
fn explicit_integration_target_keeps_declared_identity() -> Result<(), String> {
    let manifests = vec![(
        PathBuf::from("crates/demo/Cargo.toml"),
        "[package]\nname = \"demo-package\"\nversion = \"0.1.0\"\nautotests = false\n\n[[test]]\nname = \"contract\"\npath = \"tests/custom_case.rs\"\n"
            .to_string(),
    )];
    let inventory = inventory_rust_test_subjects_from_sources(
        manifests,
        vec![(
            PathBuf::from("crates/demo/tests/custom_case.rs"),
            "#[test]\nfn exact() {}".into(),
        )],
        &RustTestInventoryOptions::default(),
    );
    let subject = only_subject(&inventory)?;
    assert_eq!(
        subject.selector.target.kind,
        RustTestTargetKind::IntegrationTest
    );
    assert_eq!(subject.selector.target.name, "contract");
    Ok(())
}

#[test]
fn malformed_manifest_keeps_inventory_partial() {
    let inventory = inventory_rust_test_subjects_from_sources(
        vec![(PathBuf::from("crates/demo/Cargo.toml"), "[package".into())],
        vec![(
            PathBuf::from("crates/demo/src/lib.rs"),
            "#[test]\nfn exact() {}".into(),
        )],
        &RustTestInventoryOptions::default(),
    );
    assert_eq!(inventory.status, RustTestInventoryStatus::Partial);
    assert!(inventory.subjects.is_empty());
}

#[test]
fn windows_style_repo_paths_are_normalized() -> Result<(), String> {
    let inventory = inventory_rust_test_subjects_from_sources(
        vec![(
            PathBuf::from("crates\\demo\\Cargo.toml"),
            "[package]\nname = \"demo-package\"\nversion = \"0.1.0\"\n".into(),
        )],
        vec![(
            PathBuf::from("crates\\demo\\src\\lib.rs"),
            "#[test]\nfn exact() {}".into(),
        )],
        &RustTestInventoryOptions::default(),
    );
    let subject = only_subject(&inventory)?;
    assert_eq!(subject.source_path, "crates/demo/src/lib.rs");
    Ok(())
}

#[test]
fn missing_selector_is_not_guessed() -> Result<(), String> {
    let inventory = inventory_rust_test_subjects_from_sources(
        manifest(),
        vec![(
            PathBuf::from("crates/demo/src/lib.rs"),
            "#[test]\nfn exact_name() {}".into(),
        )],
        &RustTestInventoryOptions::default(),
    );
    let mut selector = only_subject(&inventory)?.selector.clone();
    selector.function = "nearby_name".into();
    assert_eq!(
        resolve_rust_test_selector(&inventory, &selector),
        RustTestResolution::NotFound
    );
    Ok(())
}
