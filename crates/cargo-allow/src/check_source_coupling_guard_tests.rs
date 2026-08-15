use super::super::governance_projection::CrateIdentityProjection;
use super::super::governance_projection::GovernanceProjection;
use super::super::governance_projection::crate_identity_for_path;
use super::{
    source_coupling_diagnostics_at, source_coupling_diagnostics_for_check,
    source_coupling_diagnostics_for_sources, source_coupling_fails_check,
};
use allow_match::CheckMode;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture_manifest() -> GovernanceProjection {
    GovernanceProjection {
        crate_identities: vec![
            CrateIdentityProjection {
                logical_id: "product-a".to_string(),
                workspace_path: "crates/product-a".to_string(),
                cargo_package_name: "product-a".to_string(),
                workspace_dependency_aliases: vec!["product-a".to_string()],
                rust_library_name: "product_a".to_string(),
                product_or_shared_owner: "product-a".to_string(),
            },
            CrateIdentityProjection {
                logical_id: "product-b".to_string(),
                workspace_path: "crates/product-b".to_string(),
                cargo_package_name: "product-b".to_string(),
                workspace_dependency_aliases: vec![
                    "product-b".to_string(),
                    "product-b-alias".to_string(),
                ],
                rust_library_name: "product_b".to_string(),
                product_or_shared_owner: "product-b".to_string(),
            },
            CrateIdentityProjection {
                logical_id: "shared-protocol".to_string(),
                workspace_path: "crates/shared-protocol".to_string(),
                cargo_package_name: "shared-protocol".to_string(),
                workspace_dependency_aliases: vec!["shared-protocol".to_string()],
                rust_library_name: "shared_protocol".to_string(),
                product_or_shared_owner: "shared".to_string(),
            },
        ],
        forbidden_product_dependencies: BTreeMap::new(),
    }
}

#[test]
fn rejects_only_known_cross_product_imports() -> Result<(), String> {
    let manifest = fixture_manifest();
    let sources = vec![(
        PathBuf::from("crates/product-a/src/lib.rs"),
        "use product_b::private_api;\nuse product_b::another_api;\nuse product_a::own_api;\nuse shared_protocol::Wire;\nuse external::Thing;\nuse crate::local;\n".to_string(),
    )];
    let forbidden = BTreeMap::from([(
        "product-a".to_string(),
        BTreeSet::from(["product-b".to_string()]),
    )]);
    let tracked = BTreeSet::from([PathBuf::from("crates/product-a/src/lib.rs")]);
    let diagnostics =
        source_coupling_diagnostics_for_sources(&manifest, &forbidden, &tracked, &sources)
            .map_err(|error| format!("scan coupling fixture: {error}"))?;
    if diagnostics.len() != 2 {
        return Err(format!(
            "expected two cross-product diagnostics, got {diagnostics:?}"
        ));
    }
    let diagnostic = diagnostics
        .first()
        .ok_or_else(|| "missing cross-product diagnostic".to_string())?;
    if diagnostic.target_crate != "product-b"
        || diagnostic.source_owner != "product-a"
        || diagnostic.target_owner != "product-b"
        || diagnostic.line != 1
    {
        return Err(format!("unexpected diagnostic: {diagnostic:?}"));
    }
    Ok(())
}

#[test]
fn integration_tests_reject_forbidden_path_dev_dependencies() -> Result<(), String> {
    let manifest = fixture_manifest();
    let forbidden = BTreeMap::from([(
        "product-a".to_string(),
        BTreeSet::from(["product-b".to_string()]),
    )]);
    let manifests = vec![(
        PathBuf::from("crates/product-a/Cargo.toml"),
        "[package]\nname = \"product-a\"\n\n[dev-dependencies]\nproduct-b = { path = \"../product-b\" }\n"
            .to_string(),
    )];
    let tracked = BTreeSet::from([
        PathBuf::from("crates/product-a/Cargo.toml"),
        PathBuf::from("crates/product-a/tests/cross_product.rs"),
    ]);
    if super::normalize_relative_path(Path::new("crates/product-a"), "../product-b")
        != Some(PathBuf::from("crates/product-b"))
    {
        return Err("fixture path normalization failed".to_string());
    }
    if super::projection_identity_for_path(&manifest, "crates/product-a").is_none()
        || super::projection_identity_for_path(&manifest, "crates/product-b").is_none()
    {
        return Err("fixture identity projection failed".to_string());
    }
    let manifest_text = manifests
        .first()
        .map(|(_, text)| text)
        .ok_or_else(|| "fixture manifest missing".to_string())?;
    let parsed = toml::from_str::<toml::Table>(manifest_text).map_err(|e| e.to_string())?;
    let path = parsed
        .get("dev-dependencies")
        .and_then(toml::Value::as_table)
        .and_then(|table| table.get("product-b"))
        .and_then(toml::Value::as_table)
        .and_then(|table| table.get("path"))
        .and_then(toml::Value::as_str)
        .ok_or_else(|| "fixture TOML path dependency failed".to_string())?;
    let target_path = super::normalize_relative_path(Path::new("crates/product-a"), path)
        .ok_or_else(|| "fixture target path failed".to_string())?;
    let target =
        super::projection_identity_for_path(&manifest, &super::normalize_path(&target_path))
            .ok_or_else(|| format!("fixture target identity failed: {target_path:?}"))?;
    if !forbidden
        .get("product-a")
        .is_some_and(|targets| targets.contains(&target.product_or_shared_owner))
    {
        return Err(format!("fixture forbidden edge failed: {target:?}"));
    }
    let integration_tests = vec![(
        PathBuf::from("crates/product-a/tests/cross_product.rs"),
        "use product_b::private_api;\n".to_string(),
    )];
    let diagnostics = super::integration_test_dependency_diagnostics(
        &manifest,
        &forbidden,
        &tracked,
        &manifests,
        &integration_tests,
        &toml::map::Map::new(),
    )
    .map_err(|error| error.to_string())?;
    let Some(diagnostic) = diagnostics.first() else {
        return Err("missing integration-test path dependency diagnostic".to_string());
    };
    if diagnostics.len() != 1
        || diagnostic.kind != super::SourceCouplingDiagnosticKind::IntegrationTestDependency
        || diagnostic.source_owner != "product-a"
        || diagnostic.target_crate != "product-b"
        || diagnostic.target_owner != "product-b"
        || diagnostic.line != 5
    {
        return Err(format!(
            "unexpected integration-test dependency diagnostics: {diagnostics:?}"
        ));
    }
    Ok(())
}

#[test]
fn integration_test_dependency_guard_allows_same_product_shared_and_non_test_dependencies()
-> Result<(), String> {
    let manifest = fixture_manifest();
    let forbidden = BTreeMap::from([(
        "product-a".to_string(),
        BTreeSet::from(["product-b".to_string()]),
    )]);
    let manifests = vec![
        (
            PathBuf::from("crates/product-a/Cargo.toml"),
            "[dev-dependencies]\nproduct-a = { path = \".\" }\nshared-protocol = { path = \"../shared-protocol\" }\n"
                .to_string(),
        ),
        (
            PathBuf::from("crates/product-b/Cargo.toml"),
            "[dev-dependencies]\nproduct-a = { path = \"../product-a\" }\n".to_string(),
        ),
    ];
    let tracked = BTreeSet::from([
        PathBuf::from("crates/product-a/Cargo.toml"),
        PathBuf::from("crates/product-a/tests/owned.rs"),
        PathBuf::from("crates/product-b/Cargo.toml"),
    ]);
    let diagnostics = super::integration_test_dependency_diagnostics(
        &manifest,
        &forbidden,
        &tracked,
        &manifests,
        &[(
            PathBuf::from("crates/product-a/tests/owned.rs"),
            "use product_a::own_api;\nuse shared_protocol::Wire;\n".to_string(),
        )],
        &toml::map::Map::new(),
    )
    .map_err(|error| error.to_string())?;
    if !diagnostics.is_empty() {
        return Err(format!(
            "allowed or non-integration dependencies unexpectedly failed: {diagnostics:?}"
        ));
    }
    Ok(())
}

#[test]
fn integration_test_dependency_guard_rejects_target_specific_path_dependencies()
-> Result<(), String> {
    let manifest = fixture_manifest();
    let forbidden = BTreeMap::from([(
        "product-a".to_string(),
        BTreeSet::from(["product-b".to_string()]),
    )]);
    let manifests = vec![(
        PathBuf::from("crates/product-a/Cargo.toml"),
        "[target.'cfg(windows)'.dev-dependencies]\nproduct-b-alias = { workspace = true }\n"
            .to_string(),
    )];
    let tracked = BTreeSet::from([
        PathBuf::from("crates/product-a/Cargo.toml"),
        PathBuf::from("crates/product-a/tests/cross_product.rs"),
    ]);
    let integration_tests = vec![(
        PathBuf::from("crates/product-a/tests/cross_product.rs"),
        "use product_b::private_api;\n".to_string(),
    )];
    let workspace = toml::from_str::<toml::Table>(
        "[workspace.dependencies]\nproduct-b-alias = { path = \"crates/product-b\" }\n",
    )
    .map_err(|error| error.to_string())?;
    let workspace_dependencies = workspace
        .get("workspace")
        .and_then(toml::Value::as_table)
        .and_then(|table| table.get("dependencies"))
        .and_then(toml::Value::as_table)
        .ok_or_else(|| "missing workspace dependency fixture".to_string())?;
    let diagnostics = super::integration_test_dependency_diagnostics(
        &manifest,
        &forbidden,
        &tracked,
        &manifests,
        &integration_tests,
        workspace_dependencies,
    )
    .map_err(|error| error.to_string())?;
    let Some(diagnostic) = diagnostics.first() else {
        return Err(format!(
            "missing target-specific diagnostic: {diagnostics:?}"
        ));
    };
    if diagnostics.len() != 1 || diagnostic.target_crate != "product-b" || diagnostic.line != 2 {
        return Err(format!(
            "unexpected target-specific diagnostics: {diagnostics:?}"
        ));
    }
    Ok(())
}

#[test]
fn integration_test_dependency_import_matching_handles_multiline_use_and_comments()
-> Result<(), String> {
    let aliases = BTreeSet::from(["product_b".to_string()]);
    if !super::rust_source_uses_dependency(
        "use\n    product_b::{private_api,\n    another_api};\n",
        &aliases,
    ) {
        return Err("multiline use was not recognized".to_string());
    }
    if super::rust_source_uses_dependency(
        "// use product_b::private_api;\nlet text = \"product_b::private_api\";\n",
        &aliases,
    ) {
        return Err("comment or string falsely matched dependency".to_string());
    }
    if !super::rust_source_uses_dependency("extern crate product_b;\n", &aliases) {
        return Err("extern crate use was not recognized".to_string());
    }
    if !super::rust_source_uses_dependency(
        "extern crate unrelated;\nfn call() { product_b::private_api(); }\n",
        &aliases,
    ) {
        return Err("qualified dependency path was not recognized".to_string());
    }
    if !super::rust_source_uses_dependency(
        "/* product_b::fake /* nested product_b::fake */ still fake */\nlet c = ':';\nproduct_b::real_api();\n",
        &aliases,
    ) {
        return Err("qualified path after nested comment was not recognized".to_string());
    }
    Ok(())
}

#[test]
fn integration_test_dependency_guard_handles_root_package_tests() -> Result<(), String> {
    let mut manifest = fixture_manifest();
    if let Some(identity) = manifest.crate_identities.first_mut() {
        identity.workspace_path.clear();
    }
    let forbidden = BTreeMap::from([(
        "product-a".to_string(),
        BTreeSet::from(["product-b".to_string()]),
    )]);
    let tracked = BTreeSet::from([
        PathBuf::from("Cargo.toml"),
        PathBuf::from("tests/root_product.rs"),
    ]);
    let manifests = vec![(
        PathBuf::from("Cargo.toml"),
        "[dev-dependencies]\nproduct-b = { path = \"crates/product-b\" }\n".to_string(),
    )];
    let integration_tests = vec![(
        PathBuf::from("tests/root_product.rs"),
        "use product_b::private_api;\n".to_string(),
    )];
    let diagnostics = super::integration_test_dependency_diagnostics(
        &manifest,
        &forbidden,
        &tracked,
        &manifests,
        &integration_tests,
        &toml::map::Map::new(),
    )
    .map_err(|error| error.to_string())?;
    if diagnostics.len() != 1 {
        return Err(format!(
            "root package test was not diagnosed: {diagnostics:?}"
        ));
    }
    Ok(())
}

#[test]
fn integration_test_dependency_guard_rejects_malformed_manifest() -> Result<(), String> {
    let manifest = fixture_manifest();
    let forbidden = BTreeMap::new();
    let tracked = BTreeSet::from([PathBuf::from("crates/product-a/tests/bad.rs")]);
    let manifests = vec![(
        PathBuf::from("crates/product-a/Cargo.toml"),
        "[dev-dependencies\n".to_string(),
    )];
    let error = super::integration_test_dependency_diagnostics(
        &manifest,
        &forbidden,
        &tracked,
        &manifests,
        &[],
        &toml::map::Map::new(),
    )
    .expect_err("malformed manifest must fail closed");
    if !error.to_string().contains("manifest parse failed") {
        return Err(format!("unexpected malformed manifest error: {error}"));
    }
    Ok(())
}

#[test]
fn integration_test_dependency_diagnostic_has_stable_cli_relation() -> Result<(), String> {
    let expected = [
        (super::SourceCouplingDiagnosticKind::Import, "imports"),
        (super::SourceCouplingDiagnosticKind::PathRead, "reads"),
        (
            super::SourceCouplingDiagnosticKind::IntegrationTestDependency,
            "uses integration-test dependency",
        ),
    ];
    for (kind, expected_relation) in expected {
        let relation = super::super::source_coupling_relation(kind);
        if relation != expected_relation {
            return Err(format!("unexpected diagnostic relation: {relation}"));
        }
    }
    Ok(())
}

#[test]
fn audit_mode_remains_advisory() -> Result<(), String> {
    if source_coupling_fails_check(repo_root().as_path(), CheckMode::Audit)
        .map_err(|error| format!("audit guard: {error}"))?
    {
        return Err("audit mode unexpectedly enforced source coupling".to_string());
    }
    Ok(())
}

#[test]
fn release_mode_enforces_source_coupling() -> Result<(), String> {
    for mode in [CheckMode::NoNew, CheckMode::Strict, CheckMode::Release] {
        if !super::source_coupling_mode_enforced(mode) {
            return Err(format!("{mode:?} unexpectedly bypassed source coupling"));
        }
    }
    if super::source_coupling_mode_enforced(CheckMode::Audit) {
        return Err("audit unexpectedly enforced source coupling".to_string());
    }
    Ok(())
}

#[test]
fn strict_mode_checks_architecture_repositories() -> Result<(), String> {
    let root = repo_root();
    if !root.join("policy/product-crates-v2.toml").is_file()
        || !root.join("policy/product-crates.toml").is_file()
    {
        return Err("architecture repository fixture is missing policy manifests".to_string());
    }
    let diagnostics = source_coupling_diagnostics_for_check(&root, CheckMode::Strict)
        .map_err(|error| format!("strict guard: {error}"))?;
    if !diagnostics.is_empty() {
        return Err(format!(
            "unexpected tracked source coupling: {diagnostics:?}"
        ));
    }
    Ok(())
}

#[test]
fn consumer_trees_without_architecture_manifests_are_outside_scope() -> Result<(), String> {
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-source-coupling-consumer-{}",
        std::process::id()
    ));
    if root.exists() {
        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("clean consumer fixture: {error}"))?;
    }
    std::fs::create_dir_all(root.join("src"))
        .map_err(|error| format!("create consumer fixture: {error}"))?;
    std::fs::write(root.join("src/lib.rs"), "use external::Thing;\n")
        .map_err(|error| format!("write consumer fixture: {error}"))?;
    if root.join("policy/product-crates-v2.toml").exists()
        || root.join("policy/product-crates.toml").exists()
    {
        return Err("consumer fixture unexpectedly contains architecture manifests".to_string());
    }
    let diagnostics = source_coupling_diagnostics_at(&root)
        .map_err(|error| format!("consumer tree guard: {error}"))?;
    std::fs::remove_dir_all(&root).map_err(|error| format!("remove consumer fixture: {error}"))?;
    if !diagnostics.is_empty() {
        return Err(format!(
            "unexpected consumer-tree diagnostics: {diagnostics:?}"
        ));
    }
    Ok(())
}

#[test]
fn tracked_worktree_source_coupling_is_clean() -> Result<(), String> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let diagnostics = super::source_coupling_diagnostics_at(&root)
        .map_err(|error| format!("scan tracked source coupling: {error}"))?;
    if !diagnostics.is_empty() {
        return Err(format!("tracked worktree source coupling: {diagnostics:?}"));
    }
    Ok(())
}

#[test]
fn path_read_fixtures_allow_owned_and_shared_paths_and_reject_forbidden_or_unresolved_paths()
-> Result<(), String> {
    let manifest = fixture_manifest();
    let forbidden = BTreeMap::from([(
        "product-a".to_string(),
        BTreeSet::from(["product-b".to_string()]),
    )]);
    let allowed = vec![
        (
            PathBuf::from("crates/product-a/src/lib.rs"),
            include_str!("../../../tests/fixtures/source-coupling/path-reads-allowed.rs")
                .to_string(),
        ),
        (
            PathBuf::from("crates/product-a/src/local/owned.rs"),
            String::new(),
        ),
    ];
    let allowed_tracked = BTreeSet::from([
        PathBuf::from("crates/product-a/src/lib.rs"),
        PathBuf::from("crates/product-a/src/local/owned.rs"),
        PathBuf::from("crates/shared-protocol/src/public.rs"),
    ]);
    let allowed_diagnostics =
        source_coupling_diagnostics_for_sources(&manifest, &forbidden, &allowed_tracked, &allowed)
            .map_err(|error| format!("scan allowed path-read fixture: {error}"))?;
    if !allowed_diagnostics.is_empty() {
        return Err(format!(
            "owned/shared path reads unexpectedly failed: {allowed_diagnostics:?}"
        ));
    }

    let untracked_sources = vec![(
        PathBuf::from("crates/product-a/src/lib.rs"),
        "::std::include_str!(\"local/untracked.txt\");\n".to_string(),
    )];
    let untracked_diagnostics = source_coupling_diagnostics_for_sources(
        &manifest,
        &forbidden,
        &BTreeSet::from([PathBuf::from("crates/product-a/src/lib.rs")]),
        &untracked_sources,
    )
    .map_err(|error| format!("scan untracked same-product path read: {error}"))?;
    if untracked_diagnostics.len() != 1
        || untracked_diagnostics
            .first()
            .is_some_and(|diagnostic| diagnostic.target_crate != "<untracked-path>")
    {
        return Err(format!(
            "untracked same-product path read did not fail closed: {untracked_diagnostics:?}"
        ));
    }

    let nested_include = vec![(
        PathBuf::from("crates/product-a/src/lib.rs"),
        "::std::include!(\"nested/include\");\n::std::include!(\"tests.rs\");\n".to_string(),
    )];
    let nested_tracked = BTreeSet::from([
        PathBuf::from("crates/product-a/src/lib.rs"),
        PathBuf::from("crates/product-a/src/nested/include"),
        PathBuf::from("crates/product-a/src/tests.rs"),
    ]);
    let nested_diagnostics = source_coupling_diagnostics_for_sources(
        &manifest,
        &forbidden,
        &nested_tracked,
        &nested_include,
    )
    .map_err(|error| format!("scan unscanned nested include: {error}"))?;
    if nested_diagnostics.len() != 2
        || nested_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.target_crate != "<unscanned-include-path>")
    {
        return Err(format!(
            "unscanned nested include did not fail closed: {nested_diagnostics:?}"
        ));
    }
    for invocation in [
        "std::include!(\"nested/include\")",
        "std::include /* mention include ! /* nested */ still */ ! (\"nested/include\")",
        "include // note !\n ! (\"tests.rs\")",
        "r#include!(\"tests.rs\")",
    ] {
        if !super::is_include_macro(invocation) {
            return Err(format!("include identity missed trivia: {invocation}"));
        }
    }

    let forbidden_sources = vec![
        (
            PathBuf::from("crates/product-a/src/lib.rs"),
            include_str!("../../../tests/fixtures/source-coupling/path-reads-forbidden.rs")
                .to_string(),
        ),
        (
            PathBuf::from("crates/product-b/src/private.rs"),
            String::new(),
        ),
    ];
    let forbidden_tracked = BTreeSet::from([
        PathBuf::from("crates/product-a/src/lib.rs"),
        PathBuf::from("crates/product-b/src/private.rs"),
    ]);
    let forbidden_diagnostics = source_coupling_diagnostics_for_sources(
        &manifest,
        &forbidden,
        &forbidden_tracked,
        &forbidden_sources,
    )
    .map_err(|error| format!("scan forbidden path-read fixture: {error}"))?;
    let Some(forbidden_diagnostic) = forbidden_diagnostics.first() else {
        return Err("missing forbidden path-read diagnostic".to_string());
    };
    if forbidden_diagnostics.len() != 1
        || forbidden_diagnostic.kind != super::SourceCouplingDiagnosticKind::PathRead
        || forbidden_diagnostic.target_crate != "product-b"
        || forbidden_diagnostic.line != 1
    {
        return Err(format!(
            "unexpected forbidden path-read diagnostics: {forbidden_diagnostics:?}"
        ));
    }

    let unresolved_sources = vec![(
        PathBuf::from("crates/product-a/src/lib.rs"),
        include_str!("../../../tests/fixtures/source-coupling/path-reads-unresolved.rs")
            .to_string(),
    )];
    let unresolved_tracked = BTreeSet::from([PathBuf::from("crates/product-a/src/lib.rs")]);
    let unresolved_diagnostics = source_coupling_diagnostics_for_sources(
        &manifest,
        &forbidden,
        &unresolved_tracked,
        &unresolved_sources,
    )
    .map_err(|error| format!("scan unresolved path-read fixture: {error}"))?;
    let unresolved_targets: Vec<_> = unresolved_diagnostics
        .iter()
        .map(|diagnostic| diagnostic.target_crate.as_str())
        .collect();
    if unresolved_targets != ["<unresolved-path>", "<escaping-path>"] {
        return Err(format!(
            "unexpected unresolved path-read diagnostics: {unresolved_diagnostics:?}"
        ));
    }
    Ok(())
}

#[test]
fn path_resolution_rejects_ambiguous_and_escaping_inputs() -> Result<(), String> {
    use super::PathReadResolution::{Escapes, Resolved, Unresolved};
    use allow_rust::RustSourceCouplingPathBase::{ManifestDirectory, SourceFile};

    if super::resolve_relative_source_path_from_crate_root(
        Path::new("crates/product-a/src/lib.rs"),
        SourceFile,
        "local.rs",
        "crates/product-a",
    ) != Resolved(PathBuf::from("crates/product-a/src/local.rs"))
    {
        return Err("source-relative path did not resolve from the source directory".to_string());
    }
    if super::resolve_relative_source_path_from_crate_root(
        Path::new("crates/product-a/src/lib.rs"),
        SourceFile,
        " spaced.rs ",
        "crates/product-a",
    ) != Resolved(PathBuf::from("crates/product-a/src/ spaced.rs "))
    {
        return Err("source-relative literal path whitespace was not preserved".to_string());
    }
    if super::resolve_relative_source_path_from_crate_root(
        Path::new("crates/product-a/src/lib.rs"),
        ManifestDirectory,
        "/shared/public.rs",
        "crates/product-a",
    ) != Resolved(PathBuf::from("crates/product-a/shared/public.rs"))
    {
        return Err("manifest-relative path did not resolve from the crate root".to_string());
    }
    #[cfg(unix)]
    if super::resolve_relative_source_path_from_crate_root(
        Path::new("crates/product-a/src/lib.rs"),
        SourceFile,
        "safe\\owned.rs",
        "crates/product-a",
    ) != Resolved(PathBuf::from("crates/product-a/src/safe\\owned.rs"))
    {
        return Err("unix path resolution treated a literal backslash as a separator".to_string());
    }
    #[cfg(windows)]
    if super::resolve_relative_source_path_from_crate_root(
        Path::new("crates/product-a/src/lib.rs"),
        SourceFile,
        "safe\\owned.rs",
        "crates/product-a",
    ) != Resolved(PathBuf::from("crates/product-a/src/safe/owned.rs"))
    {
        return Err("windows path resolution did not treat backslash as a separator".to_string());
    }
    for path in [
        "",
        "../../../../outside.rs",
        "/absolute.rs",
        "C:\\absolute.rs",
    ] {
        let resolution = super::resolve_relative_source_path_from_crate_root(
            Path::new("crates/product-a/src/lib.rs"),
            SourceFile,
            path,
            "crates/product-a",
        );
        if !matches!(resolution, Unresolved | Escapes) {
            return Err(format!(
                "ambiguous or escaping path resolved unexpectedly: {path:?}"
            ));
        }
    }
    if super::resolve_relative_source_path_from_crate_root(
        Path::new("README.md"),
        ManifestDirectory,
        "shared.rs",
        "",
    ) != Unresolved
    {
        return Err("manifest-relative path without a source root was not unresolved".to_string());
    }
    Ok(())
}

#[test]
fn crate_identity_uses_the_deepest_containing_workspace_path() -> Result<(), String> {
    let manifest = fixture_manifest();
    let identity = crate_identity_for_path(&manifest, "crates/product-a/src/local.rs")
        .ok_or_else(|| "missing containing product identity".to_string())?;
    if identity.logical_id != "product-a" {
        return Err(format!("unexpected containing identity: {identity:?}"));
    }
    if crate_identity_for_path(&manifest, "crates/product").is_some()
        || crate_identity_for_path(&manifest, "outside.rs").is_some()
    {
        return Err("non-containing path was assigned a crate identity".to_string());
    }
    Ok(())
}
#[test]
fn manifest_directory_paths_use_crate_root_for_build_and_example_sources() -> Result<(), String> {
    use super::resolve_relative_source_path_from_crate_root;
    use allow_rust::RustSourceCouplingPathBase::ManifestDirectory;
    for source in [
        Path::new("crates/product-a/build.rs"),
        Path::new("crates/product-a/examples/demo.rs"),
    ] {
        let path = resolve_relative_source_path_from_crate_root(
            source,
            ManifestDirectory,
            "shared/public.rs",
            "crates/product-a",
        );
        if path
            != super::PathReadResolution::Resolved(PathBuf::from(
                "crates/product-a/shared/public.rs",
            ))
        {
            return Err(format!(
                "manifest path did not use crate root for {source:?}: {path:?}"
            ));
        }
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn symlinked_tracked_targets_resolve_inside_and_reject_escape() -> Result<(), String> {
    use std::os::unix::fs::symlink;
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-coupling-symlink-{}",
        std::process::id()
    ));
    if root.exists() {
        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("clean symlink fixture: {error}"))?;
    }
    std::fs::create_dir_all(root.join("crates/product-a/src"))
        .map_err(|error| format!("create symlink source fixture: {error}"))?;
    std::fs::create_dir_all(root.join("crates/product-b/src"))
        .map_err(|error| format!("create symlink fixture: {error}"))?;
    std::fs::write(root.join("crates/product-b/src/private.rs"), "secret\n")
        .map_err(|error| format!("write symlink target: {error}"))?;
    symlink(
        "product-b/src/private.rs",
        root.join("crates/product-a-link.rs"),
    )
    .map_err(|error| format!("create inside symlink: {error}"))?;
    if super::resolve_tracked_target(&root, Path::new("crates/product-a-link.rs"))
        .map_err(|error| format!("resolve inside symlink: {error}"))?
        != super::TrackedTargetResolution::Inside(PathBuf::from("crates/product-b/src/private.rs"))
    {
        return Err("inside symlink target was not canonicalized".to_string());
    }
    symlink(
        "../../product-b/src/private.rs",
        root.join("crates/product-a/src/linked.rs"),
    )
    .map_err(|error| format!("create cross-product symlink: {error}"))?;
    let manifest = fixture_manifest();
    let forbidden = BTreeMap::from([(
        "product-a".to_string(),
        BTreeSet::from(["product-b".to_string()]),
    )]);
    let tracked = BTreeSet::from([
        PathBuf::from("crates/product-a/src/lib.rs"),
        PathBuf::from("crates/product-a/src/linked.rs"),
        PathBuf::from("crates/product-b/src/private.rs"),
    ]);
    let sources = vec![(
        PathBuf::from("crates/product-a/src/lib.rs"),
        "::std::include_str!(\"linked.rs\");\n".to_string(),
    )];
    let diagnostics = super::source_coupling_diagnostics_for_sources_at_root(
        &manifest,
        &forbidden,
        &tracked,
        &sources,
        Some(&root),
    )
    .map_err(|error| format!("scan cross-product symlink: {error}"))?;
    let Some(diagnostic) = diagnostics.first() else {
        return Err("cross-product symlink produced no diagnostic".to_string());
    };
    if diagnostics.len() != 1
        || diagnostic.target_crate != "product-b"
        || diagnostic.target_owner != "product-b"
    {
        return Err(format!(
            "cross-product symlink was not rejected: {diagnostics:?}"
        ));
    }
    let outside = root
        .parent()
        .ok_or_else(|| "missing temp parent".to_string())?;
    let outside_file = outside.join(format!("cargo-allow-outside-{}.rs", std::process::id()));
    std::fs::write(&outside_file, "outside\n")
        .map_err(|error| format!("write outside target: {error}"))?;
    symlink(&outside_file, root.join("escape.rs"))
        .map_err(|error| format!("create escape symlink: {error}"))?;
    if super::resolve_tracked_target(&root, Path::new("escape.rs"))
        .map_err(|error| format!("resolve escape symlink: {error}"))?
        != super::TrackedTargetResolution::Outside
    {
        return Err("outside symlink target was accepted".to_string());
    }
    std::fs::remove_file(&outside_file).ok();
    std::fs::remove_dir_all(&root).map_err(|error| format!("remove symlink fixture: {error}"))?;
    Ok(())
}
