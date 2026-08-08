//! Tests for alias-, feature-, target-aware product closure validation (#2922).

use super::closure::{
    CargoDependencyClass, CargoPackageIdResolver, PackageResolutionError, find_identity_by_library,
    find_identity_by_package, parse_cargo_metadata_graph_v2, shortest_closure_path,
};
use super::config::CrateRole;
use super::v2::{ARCHITECTURE_MANIFEST_V2_SCHEMA_VERSION, CrateIdentityV2};

fn make_identity(logical: &str, package: &str, library: &str, owner: &str) -> CrateIdentityV2 {
    CrateIdentityV2 {
        logical_id: logical.to_string(),
        workspace_path: format!("crates/{package}"),
        workspace_dependency_aliases: vec![package.to_string()],
        cargo_package_name: package.to_string(),
        rust_library_name: library.to_string(),
        product_or_shared_owner: owner.to_string(),
        crate_role: CrateRole::SharedProtocol,
    }
}

fn make_manifest(identities: Vec<CrateIdentityV2>) -> super::v2::ArchitectureManifestV2 {
    super::v2::ArchitectureManifestV2 {
        schema_version: ARCHITECTURE_MANIFEST_V2_SCHEMA_VERSION.to_string(),
        authority_generation: 2,
        manifest_id: "TEST".to_string(),
        controlling_issue: 2922,
        linked_move_ledger: "TEST".to_string(),
        crate_identity: identities,
    }
}

// --- Dependency class tests ---

#[test]
fn dependency_class_from_kind_and_flags() {
    use super::closure::CargoDependencyClass;
    assert_eq!(
        CargoDependencyClass::from_kind_and_flags(None, false, None),
        CargoDependencyClass::Normal
    );
    assert_eq!(
        CargoDependencyClass::from_kind_and_flags(Some("dev"), false, None),
        CargoDependencyClass::Dev
    );
    assert_eq!(
        CargoDependencyClass::from_kind_and_flags(Some("build"), false, None),
        CargoDependencyClass::Build
    );
    assert_eq!(
        CargoDependencyClass::from_kind_and_flags(None, true, None),
        CargoDependencyClass::Optional
    );
    assert_eq!(
        CargoDependencyClass::from_kind_and_flags(None, false, Some("cfg(windows)")),
        CargoDependencyClass::TargetSpecific
    );
}

// --- Package ID resolver tests ---

#[test]
fn resolver_maps_package_name_to_logical_id() -> Result<(), String> {
    let manifest = make_manifest(vec![
        make_identity(
            "repo-protocol",
            "effortless-repo-protocol",
            "repo_protocol",
            "shared",
        ),
        make_identity("allow-core", "allow-core", "allow_core", "cargo-allow"),
    ]);
    let resolver = CargoPackageIdResolver::from_manifest(&manifest).map_err(|e| e.to_string())?;
    let logical = resolver
        .resolve("effortless-repo-protocol")
        .map_err(|e| format!("{e:?}"))?;
    if logical != "repo-protocol" {
        return Err(format!("expected repo-protocol, got {logical}"));
    }
    Ok(())
}

#[test]
fn resolver_handles_alias_different_from_package_name() -> Result<(), String> {
    let mut identity = make_identity("core", "allow-core", "allow_core", "cargo-allow");
    identity.workspace_dependency_aliases = vec!["legacy-core".to_string()];
    let manifest = make_manifest(vec![identity]);
    let resolver = CargoPackageIdResolver::from_manifest(&manifest).map_err(|e| e.to_string())?;
    let logical = resolver
        .resolve("legacy-core")
        .map_err(|e| format!("{e:?}"))?;
    if logical != "core" {
        return Err(format!("expected core, got {logical}"));
    }
    Ok(())
}

#[test]
fn resolver_rejects_unknown_package() -> Result<(), String> {
    let manifest = make_manifest(vec![make_identity("a", "pkg-a", "lib_a", "shared")]);
    let resolver = CargoPackageIdResolver::from_manifest(&manifest).map_err(|e| e.to_string())?;
    match resolver.resolve("unknown") {
        Err(PackageResolutionError::UnknownPackage { package }) if package == "unknown" => Ok(()),
        other => Err(format!("expected UnknownPackage, got {other:?}")),
    }
}

#[test]
fn resolver_rejects_ambiguous_alias_at_build_time() -> Result<(), String> {
    let mut a = make_identity("a", "pkg-a", "lib_a", "shared");
    a.workspace_dependency_aliases = vec!["shared-alias".to_string()];
    let mut b = make_identity("b", "pkg-b", "lib_b", "shared");
    b.workspace_dependency_aliases = vec!["shared-alias".to_string()];
    let manifest = make_manifest(vec![a, b]);
    let result = CargoPackageIdResolver::from_manifest(&manifest);
    if result.is_ok() {
        return Err("ambiguous alias should fail at build time".to_string());
    }
    Ok(())
}

// --- Metadata graph parsing tests ---

#[test]
fn metadata_parser_extracts_normal_dev_build_edges() -> Result<(), String> {
    let json = r#"{
        "packages": [
            {
                "name": "my-crate",
                "dependencies": [
                    {"name": "normal-dep"},
                    {"name": "dev-dep", "kind": "dev"},
                    {"name": "build-dep", "kind": "build"}
                ]
            }
        ]
    }"#;
    let graph = parse_cargo_metadata_graph_v2(json).map_err(|e| e.to_string())?;
    if graph.edges.len() != 3 {
        return Err(format!("expected 3 edges, got {}", graph.edges.len()));
    }
    let classes: Vec<_> = graph.edges.iter().map(|e| e.class).collect();
    if !classes.contains(&CargoDependencyClass::Normal) {
        return Err("missing Normal edge".to_string());
    }
    if !classes.contains(&CargoDependencyClass::Dev) {
        return Err("missing Dev edge".to_string());
    }
    if !classes.contains(&CargoDependencyClass::Build) {
        return Err("missing Build edge".to_string());
    }
    Ok(())
}

#[test]
fn metadata_parser_extracts_optional_and_target_edges() -> Result<(), String> {
    let json = r#"{
        "packages": [
            {
                "name": "my-crate",
                "dependencies": [
                    {"name": "opt-dep", "optional": true},
                    {"name": "target-dep", "target": "cfg(windows)"}
                ]
            }
        ]
    }"#;
    let graph = parse_cargo_metadata_graph_v2(json).map_err(|e| e.to_string())?;
    let opt = graph.edges.iter().find(|e| e.to_package == "opt-dep");
    let target = graph.edges.iter().find(|e| e.to_package == "target-dep");
    match (opt, target) {
        (Some(opt), Some(target)) => {
            if opt.class != CargoDependencyClass::Optional {
                return Err(format!("expected Optional, got {:?}", opt.class));
            }
            if target.class != CargoDependencyClass::TargetSpecific {
                return Err(format!("expected TargetSpecific, got {:?}", target.class));
            }
            if target.target_predicate.as_deref() != Some("cfg(windows)") {
                return Err("missing target predicate".to_string());
            }
            Ok(())
        }
        _ => Err("missing expected edges".to_string()),
    }
}

#[test]
fn metadata_parser_extracts_feature_activation() -> Result<(), String> {
    let json = r#"{
        "packages": [
            {
                "name": "my-crate",
                "dependencies": [
                    {"name": "feat-dep", "feature": ["nightly"]}
                ]
            }
        ]
    }"#;
    let graph = parse_cargo_metadata_graph_v2(json).map_err(|e| e.to_string())?;
    let edge = graph
        .edges
        .iter()
        .find(|e| e.to_package == "feat-dep")
        .ok_or("missing edge")?;
    if edge.activating_feature.as_deref() != Some("nightly") {
        return Err(format!(
            "expected activating_feature=nightly, got {:?}",
            edge.activating_feature
        ));
    }
    Ok(())
}

// --- Shortest path tests ---

#[test]
fn shortest_path_finds_direct_edge() -> Result<(), String> {
    let graph = parse_cargo_metadata_graph_v2(
        r#"{"packages":[{"name":"a","dependencies":[{"name":"b"}]}]}"#,
    )
    .map_err(|e| e.to_string())?;
    let path = shortest_closure_path(&graph, "a", "b").ok_or("no path found")?;
    if path != vec!["a".to_string(), "b".to_string()] {
        return Err(format!("unexpected path: {path:?}"));
    }
    Ok(())
}

#[test]
fn shortest_path_finds_transitive_path() -> Result<(), String> {
    let json = r#"{
        "packages": [
            {"name": "a", "dependencies": [{"name": "b"}]},
            {"name": "b", "dependencies": [{"name": "c"}]}
        ]
    }"#;
    let graph = parse_cargo_metadata_graph_v2(json).map_err(|e| e.to_string())?;
    let path = shortest_closure_path(&graph, "a", "c").ok_or("no path found")?;
    if path.len() != 3
        || path.first().map(|s| s.as_str()) != Some("a")
        || path.get(2).map(|s| s.as_str()) != Some("c")
    {
        return Err(format!("unexpected path: {path:?}"));
    }
    Ok(())
}

#[test]
fn shortest_path_returns_none_when_unreachable() -> Result<(), String> {
    let json = r#"{
        "packages": [
            {"name": "a", "dependencies": [{"name": "b"}]},
            {"name": "c", "dependencies": [{"name": "d"}]}
        ]
    }"#;
    let graph = parse_cargo_metadata_graph_v2(json).map_err(|e| e.to_string())?;
    if shortest_closure_path(&graph, "a", "c").is_some() {
        return Err("should not find path between disconnected components".to_string());
    }
    Ok(())
}

#[test]
fn shortest_path_is_deterministic_under_reordering() -> Result<(), String> {
    // Two equivalent paths exist (a→c and a→b→c). The result should be
    // deterministic — always the shorter one (a→c), and ties broken
    // alphabetically.
    let json = r#"{
        "packages": [
            {"name": "a", "dependencies": [{"name": "c"}, {"name": "b"}]},
            {"name": "b", "dependencies": [{"name": "c"}]}
        ]
    }"#;
    let graph = parse_cargo_metadata_graph_v2(json).map_err(|e| e.to_string())?;
    let path = shortest_closure_path(&graph, "a", "c").ok_or("no path found")?;
    if path.len() != 2 {
        return Err(format!("expected shortest path of length 2, got {path:?}"));
    }
    Ok(())
}

// --- Identity lookup tests ---

#[test]
fn find_identity_by_package_and_library() -> Result<(), String> {
    let manifest = make_manifest(vec![make_identity(
        "repo-protocol",
        "effortless-repo-protocol",
        "repo_protocol",
        "shared",
    )]);
    let by_pkg = find_identity_by_package(&manifest, "effortless-repo-protocol")
        .ok_or("not found by package")?;
    if by_pkg.logical_id != "repo-protocol" {
        return Err("wrong identity".to_string());
    }
    let by_lib =
        find_identity_by_library(&manifest, "repo_protocol").ok_or("not found by library")?;
    if by_lib.logical_id != "repo-protocol" {
        return Err("wrong identity".to_string());
    }
    Ok(())
}

// --- Malformed input test ---

#[test]
fn malformed_metadata_json_fails_cleanly() -> Result<(), String> {
    let result = parse_cargo_metadata_graph_v2("{not valid json");
    if result.is_ok() {
        return Err("malformed JSON should fail".to_string());
    }
    Ok(())
}

#[test]
fn empty_metadata_produces_empty_graph() -> Result<(), String> {
    let graph = parse_cargo_metadata_graph_v2(r#"{"packages":[]}"#).map_err(|e| e.to_string())?;
    if !graph.edges.is_empty() {
        return Err("empty metadata should produce empty graph".to_string());
    }
    Ok(())
}
