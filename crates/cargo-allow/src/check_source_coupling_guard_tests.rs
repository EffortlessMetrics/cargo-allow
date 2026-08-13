use super::{
    crate_identity_for_path, source_coupling_diagnostics_at, source_coupling_diagnostics_for_check,
    source_coupling_diagnostics_for_sources, source_coupling_fails_check,
};
use allow_match::CheckMode;
use allow_policy::product_crates::parse_architecture_manifest_v2;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture_manifest() -> Result<allow_policy::product_crates::ArchitectureManifestV2, String> {
    parse_architecture_manifest_v2(
        r#"
schema_version = "2.0"
authority_generation = 2
manifest_id = "fixture"
controlling_issue = 3443
linked_move_ledger = "fixture-ledger"

[[crate_identity]]
logical_id = "product-a"
workspace_path = "crates/product-a"
workspace_dependency_aliases = ["product-a"]
cargo_package_name = "product-a"
rust_library_name = "product_a"
product_or_shared_owner = "product-a"
crate_role = "CargoAllowCore"

[[crate_identity]]
logical_id = "product-b"
workspace_path = "crates/product-b"
workspace_dependency_aliases = ["product-b"]
cargo_package_name = "product-b"
rust_library_name = "product_b"
product_or_shared_owner = "product-b"
crate_role = "CargoAllowCore"

[[crate_identity]]
logical_id = "shared-protocol"
workspace_path = "crates/shared-protocol"
workspace_dependency_aliases = ["shared-protocol"]
cargo_package_name = "shared-protocol"
rust_library_name = "shared_protocol"
product_or_shared_owner = "shared"
crate_role = "SharedProtocol"
"#,
    )
    .map_err(|error| format!("fixture manifest: {error}"))
}

#[test]
fn rejects_only_known_cross_product_imports() -> Result<(), String> {
    let manifest = fixture_manifest()?;
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
    let manifest = fixture_manifest()?;
    let forbidden = BTreeMap::from([(
        "product-a".to_string(),
        BTreeSet::from(["product-b".to_string()]),
    )]);
    let allowed = vec![(
        PathBuf::from("crates/product-a/src/lib.rs"),
        include_str!("../../../tests/fixtures/source-coupling/path-reads-allowed.rs").to_string(),
    )];
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

    let nested_include = vec![(
        PathBuf::from("crates/product-a/src/lib.rs"),
        "include ! (\"nested.inc\");\ninclude!(\"tests.rs\");\n".to_string(),
    )];
    let nested_tracked = BTreeSet::from([
        PathBuf::from("crates/product-a/src/lib.rs"),
        PathBuf::from("crates/product-a/src/nested.inc"),
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

    let forbidden_sources = vec![(
        PathBuf::from("crates/product-a/src/lib.rs"),
        include_str!("../../../tests/fixtures/source-coupling/path-reads-forbidden.rs").to_string(),
    )];
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
        ManifestDirectory,
        "/shared/public.rs",
        "crates/product-a",
    ) != Resolved(PathBuf::from("crates/product-a/shared/public.rs"))
    {
        return Err("manifest-relative path did not resolve from the crate root".to_string());
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
    let manifest = fixture_manifest()?;
    let identity = crate_identity_for_path(&manifest, Path::new("crates/product-a/src/local.rs"))
        .ok_or_else(|| "missing containing product identity".to_string())?;
    if identity.logical_id != "product-a" {
        return Err(format!("unexpected containing identity: {identity:?}"));
    }
    if crate_identity_for_path(&manifest, Path::new("crates/product")).is_some()
        || crate_identity_for_path(&manifest, Path::new("outside.rs")).is_some()
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
    let manifest = fixture_manifest()?;
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
        "include_str!(\"linked.rs\");\n".to_string(),
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
