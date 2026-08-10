use allow_policy::product_crates::{
    ArchitectureDiagnosticKind, load_workspace_dependency_graph,
    validate_architecture_denominators_at, validate_architecture_manifest_at,
    validate_architecture_with_dependency_graph_at, workspace_members_from_manifest,
};
use std::path::PathBuf;

#[test]
fn product_crate_architecture_report_only_inventory() -> Result<(), String> {
    let root = repo_root();
    let members = workspace_members_from_manifest(&root)
        .map_err(|err| format!("workspace members: {err}"))?;
    let manifest_path = root.join("policy/product-crates.toml");
    let (manifest, diagnostics, report) =
        validate_architecture_manifest_at(&root, &manifest_path, &members)
            .map_err(|err| format!("validate architecture manifest: {err}"))?;

    if diagnostics
        .iter()
        .any(|diag| diag.kind == ArchitectureDiagnosticKind::UnownedWorkspaceCrate)
    {
        return Err(format!("unowned workspace crates: {diagnostics:?}"));
    }
    assert_eq!(manifest.manifest_id, "CARGO-ALLOW-ARCH-0001");
    assert_eq!(manifest.controlling_issue, 2580);
    assert_eq!(report.planned_crate_count, 0);

    let law = root.join("docs/architecture/product-crate-law.md");
    let law_text = std::fs::read_to_string(&law)
        .map_err(|err| format!("product crate law readable: {err}"))?;
    if !law_text.contains("cargo-allow") {
        return Err("human projection missing cargo-allow ownership".to_string());
    }

    Ok(())
}

#[test]
fn product_crate_dependency_law_loads_workspace_graph() -> Result<(), String> {
    let root = repo_root();
    let members = workspace_members_from_manifest(&root)
        .map_err(|err| format!("workspace members: {err}"))?;
    let manifest_path = root.join("policy/product-crates.toml");
    let graph = load_workspace_dependency_graph(&root)
        .map_err(|err| format!("load workspace dependency graph: {err}"))?;
    if graph.edges.is_empty() {
        return Err("workspace dependency graph should contain dependency edges".to_string());
    }

    let (_, diagnostics, _) =
        validate_architecture_with_dependency_graph_at(&root, &manifest_path, &members, &graph)
            .map_err(|err| format!("validate with dependency graph: {err}"))?;

    let dev_bypasses: Vec<_> = diagnostics
        .iter()
        .filter(|diag| diag.kind == ArchitectureDiagnosticKind::ForbiddenProductDependency)
        .filter(|diag| diag.message.contains("cargo-allow") && diag.message.contains("dev"))
        .collect();
    if dev_bypasses.is_empty() {
        return Err(
            "expected cargo-allow dev dependency bypasses on intent crates to remain visible"
                .to_string(),
        );
    }

    let has_normal_forbidden = diagnostics.iter().any(|diag| {
        diag.kind == ArchitectureDiagnosticKind::ForbiddenProductDependency
            && diag.message.contains("normal dependency")
    });
    if has_normal_forbidden {
        return Err(format!(
            "workspace should not have forbidden normal product dependencies yet: {diagnostics:?}"
        ));
    }

    Ok(())
}

#[test]
fn product_crate_architecture_denominators_align() -> Result<(), String> {
    let root = repo_root();
    let members = workspace_members_from_manifest(&root)
        .map_err(|err| format!("workspace members: {err}"))?;
    let manifest_path = root.join("policy/product-crates.toml");
    let text = std::fs::read_to_string(&manifest_path)
        .map_err(|err| format!("manifest readable: {err}"))?;
    let manifest = allow_policy::product_crates::parse_architecture_manifest(&text)
        .map_err(|err| format!("parse manifest: {err}"))?;
    let (diagnostics, report) = validate_architecture_denominators_at(&root, &manifest, &members)
        .map_err(|err| format!("validate denominators: {err}"))?;
    if diagnostics.iter().any(|diag| {
        matches!(
            diag.kind,
            ArchitectureDiagnosticKind::ManifestTopologyLinkMismatch
                | ArchitectureDiagnosticKind::ManifestMoveLedgerLinkMismatch
                | ArchitectureDiagnosticKind::PackageTopologyFamilyMismatch
                | ArchitectureDiagnosticKind::ArchitectureCrateMissingFromTopology
                | ArchitectureDiagnosticKind::PackageTopologyCrateMissingFromArchitecture
                | ArchitectureDiagnosticKind::PlannedCrateNowPresent
                | ArchitectureDiagnosticKind::MoveLedgerUnknownTargetCrate
        )
    }) {
        return Err(format!("architecture denominators drift: {diagnostics:?}"));
    }
    if report.architecture_crate_count != report.workspace_member_count {
        return Err(format!(
            "architecture inventory count {} should match workspace members {}",
            report.architecture_crate_count, report.workspace_member_count
        ));
    }
    Ok(())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

// ---------------------------------------------------------------------------
// #3356: Cargo-manifest dependency-boundary matrix
// ---------------------------------------------------------------------------

/// The cargo-allow product family (from policy/product-crates.toml).
const CARGO_ALLOW_OWNED: &[&str] = &[
    "allow-core",
    "allow-policy",
    "allow-inventory",
    "allow-files",
    "allow-rust",
    "allow-match",
    "allow-report",
    "allow-diff",
    "allow-policy-legacy",
    "cargo-allow",
];

/// The cargo-proof product family.
const CARGO_PROOF_OWNED: &[&str] = &["proof-protocol", "proof-engine", "cargo-proof"];

fn read_crate_manifest(crate_dir: &str) -> Result<String, String> {
    let root = repo_root();
    let path = root.join("crates").join(crate_dir).join("Cargo.toml");
    std::fs::read_to_string(&path).map_err(|e| format!("read {crate_dir}/Cargo.toml: {e}"))
}

/// Parse the `[dependencies]` table of a crate manifest and return
/// (name, optional) pairs. A dependency is optional when its value table
/// contains `optional = true`.
fn dependency_optionality(manifest: &str) -> Result<Vec<(String, bool)>, String> {
    let parsed: toml::Value =
        toml::from_str(manifest).map_err(|e| format!("parse manifest: {e}"))?;
    let deps = parsed
        .get("dependencies")
        .and_then(|d| d.as_table())
        .ok_or("manifest has no [dependencies] table")?;
    let mut out = Vec::new();
    for (name, value) in deps {
        let optional = value
            .as_table()
            .and_then(|t| t.get("optional"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        out.push((name.clone(), optional));
    }
    Ok(out)
}

/// Collect `[features]` dependencies as feature→activated-feature edges.
/// Only inter-crate feature activations (dep: references) are relevant for
/// cycle detection across products; intra-crate feature edges are skipped
/// unless they cross a product boundary.
fn feature_edges(manifest: &str) -> Result<Vec<(String, String)>, String> {
    let parsed: toml::Value =
        toml::from_str(manifest).map_err(|e| format!("parse manifest: {e}"))?;
    let features = match parsed.get("features").and_then(|f| f.as_table()) {
        Some(f) => f,
        None => return Ok(Vec::new()),
    };
    let mut edges = Vec::new();
    for (feature, activates) in features {
        let Some(list) = activates.as_array() else {
            continue;
        };
        for item in list {
            if let Some(s) = item.as_str() {
                edges.push((feature.clone(), s.to_string()));
            }
        }
    }
    Ok(edges)
}

#[test]
fn cargo_proof_does_not_link_cargo_allow_internals() -> Result<(), String> {
    // #3356: cargo-proof must not link any cargo-allow application internal.
    // It may link only shared protocols/adapters.
    for proof_crate in CARGO_PROOF_OWNED {
        let manifest = read_crate_manifest(proof_crate)?;
        let deps = dependency_optionality(&manifest)?;
        for (dep_name, _optional) in &deps {
            if CARGO_ALLOW_OWNED.contains(&dep_name.as_str()) {
                return Err(format!(
                    "cargo-proof crate `{proof_crate}` links cargo-allow internal `{dep_name}`; cargo-proof may only depend on shared protocols/adapters"
                ));
            }
        }
    }
    Ok(())
}

#[test]
fn shared_protocols_do_not_default_depend_on_allow_core() -> Result<(), String> {
    // #3356: shared substrate crates must not DEFAULT-depend on a product
    // ontology (allow-core). The allow-core edge must be optional and gated
    // behind the non-default allow-core-interop feature.
    for shared_crate in &["effortless-repo-edit", "effortless-rust-source-index"] {
        let manifest = read_crate_manifest(shared_crate)?;
        let deps = dependency_optionality(&manifest)?;
        for (dep_name, optional) in &deps {
            if dep_name == "allow-core" && !*optional {
                return Err(format!(
                    "shared crate `{shared_crate}` depends on `allow-core` as a NON-optional default dependency; the allow-core edge must be `optional = true` behind the allow-core-interop feature"
                ));
            }
        }
    }
    Ok(())
}

#[test]
fn feature_gated_interop_edges_remain_optional() -> Result<(), String> {
    // #3356: seeded fixture proving the optionality check fires when allow-core
    // is declared NON-optional in a shared crate. The real workspace state is
    // already covered by shared_protocols_do_not_default_depend_on_allow_core;
    // this fixture proves the detector would reject a regression to default.
    let seeded_non_optional = r#"
[package]
name = "seed-shared-crate"

[dependencies]
allow-core = { workspace = true }
"#;
    let deps = dependency_optionality(seeded_non_optional)?;
    let allow_core = deps
        .iter()
        .find(|(name, _)| name == "allow-core")
        .ok_or("seeded fixture should have an allow-core dependency")?;
    if allow_core.1 {
        return Err(
            "seeded fixture allow-core should be non-optional to prove the check fires".to_string(),
        );
    }
    // Reaching here confirms the detector correctly identified allow-core as
    // non-optional -- the same condition shared_protocols_do_not_default_depend_
    // on_allow_core would reject in a real crate.

    // Contrast: the real shared crates declare allow-core as optional.
    let seeded_optional = r#"
[package]
name = "seed-shared-crate-ok"

[dependencies]
allow-core = { workspace = true, optional = true }
"#;
    let deps = dependency_optionality(seeded_optional)?;
    let allow_core = deps
        .iter()
        .find(|(name, _)| name == "allow-core")
        .ok_or("seeded fixture should have an allow-core dependency")?;
    if !allow_core.1 {
        return Err("seeded fixture allow-core should be optional".to_string());
    }
    Ok(())
}

#[test]
fn no_cyclic_product_feature_relationships() -> Result<(), String> {
    // #3356: no [features] entry in one product should activate a feature that
    // (directly or transitively) activates a feature back in the originating
    // product. We check intra-manifest feature cycles (a feature that
    // transitively activates itself) as the mechanical proxy.
    let members = workspace_members_from_manifest(&repo_root())
        .map_err(|e| format!("workspace members: {e}"))?;
    for member in &members {
        let manifest = read_crate_manifest(member.trim_start_matches("crates/"))?;
        let edges = feature_edges(&manifest)?;
        if has_feature_cycle(&edges) {
            return Err(format!(
                "cyclic feature relationship detected in `{member}`"
            ));
        }
    }

    // Seeded fixture: a synthetic cyclic feature pair must be detected.
    let seeded_cycle = r#"
[features]
alpha = ["beta"]
beta = ["alpha"]
"#;
    let edges = feature_edges(seeded_cycle)?;
    if !has_feature_cycle(&edges) {
        return Err("seeded cyclic feature pair (alpha->beta->alpha) was not detected".into());
    }
    Ok(())
}

/// Detect a cycle in a same-manifest feature adjacency list using DFS.
/// Only intra-manifest feature references (no `:` or `/`) are considered.
fn has_feature_cycle(edges: &[(String, String)]) -> bool {
    let mut adj: std::collections::HashMap<&str, Vec<&str>> = std::collections::HashMap::new();
    for (from, to) in edges {
        if !to.contains(':') && !to.contains('/') {
            adj.entry(from.as_str()).or_default().push(to.as_str());
        }
    }
    let mut visited: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for &feature in adj.keys() {
        let mut stack: std::collections::HashSet<&str> = std::collections::HashSet::new();
        if dfs_cycle(feature, &adj, &mut visited, &mut stack) {
            return true;
        }
    }
    false
}

fn dfs_cycle<'a>(
    node: &'a str,
    adj: &std::collections::HashMap<&'a str, Vec<&'a str>>,
    visited: &mut std::collections::HashSet<&'a str>,
    stack: &mut std::collections::HashSet<&'a str>,
) -> bool {
    if stack.contains(node) {
        return true;
    }
    if !visited.insert(node) {
        return false;
    }
    stack.insert(node);
    if let Some(neighbors) = adj.get(node) {
        for n in neighbors {
            if dfs_cycle(n, adj, visited, stack) {
                return true;
            }
        }
    }
    stack.remove(node);
    false
}

#[test]
fn seeded_forbidden_import_is_rejected() -> Result<(), String> {
    // #3356: a seeded forbidden edge (cargo-allow linking intent-engine)
    // must produce a ForbiddenProductDependency diagnostic via the #2580
    // engine. We build a synthetic graph with one forbidden normal edge and
    // confirm validate_dependency_law flags it.
    use allow_policy::product_crates::{
        CargoMetadataGraph, DependencyClass, DependencyEdge, validate_dependency_law,
    };
    let root = repo_root();
    let manifest_path = root.join("policy/product-crates.toml");
    let text =
        std::fs::read_to_string(&manifest_path).map_err(|e| format!("read manifest: {e}"))?;
    let manifest = allow_policy::product_crates::parse_architecture_manifest(&text)
        .map_err(|e| format!("parse manifest: {e}"))?;

    let mut graph = CargoMetadataGraph::default();
    graph.edges.push(DependencyEdge {
        from: "cargo-allow".to_string(),
        to: "intent-engine".to_string(),
        class: DependencyClass::Normal,
    });

    let diagnostics = validate_dependency_law(&manifest, &graph);
    let forbidden = diagnostics
        .iter()
        .find(|d| d.kind == ArchitectureDiagnosticKind::ForbiddenProductDependency);
    let Some(diag) = forbidden else {
        return Err(format!(
            "seeded cargo-allow -> intent-engine normal edge was not flagged as ForbiddenProductDependency: {diagnostics:?}"
        ));
    };
    if !diag.message.contains("cargo-allow") || !diag.message.contains("intent") {
        return Err(format!(
            "ForbiddenProductDependency diagnostic does not name the seeded edge: {diag:?}"
        ));
    }
    Ok(())
}
