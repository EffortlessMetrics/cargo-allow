// Re-based onto intent-model compat + intent-engine closure validation
// (#3562): the allow-policy architecture validators are deleted; the
// dependency-law assertions run through the intent-side surfaces at dev
// scope. The #3356 manifest-matrix tests below are standalone and
// unchanged.
use std::collections::BTreeSet;
use std::path::PathBuf;

#[test]
fn architecture_manifest_and_law_doc_are_current() -> Result<(), String> {
    let root = repo_root();
    let manifest_text = std::fs::read_to_string(root.join("policy/product-crates.toml"))
        .map_err(|err| format!("read architecture manifest: {err}"))?;
    let manifest: toml::Table = toml::from_str(&manifest_text)
        .map_err(|err| format!("parse architecture manifest: {err}"))?;
    if manifest.get("manifest_id").and_then(|v| v.as_str()) != Some("CARGO-ALLOW-ARCH-0001") {
        return Err("manifest_id drift".into());
    }
    if manifest
        .get("controlling_issue")
        .and_then(|v| v.as_integer())
        != Some(2580)
    {
        return Err("controlling issue drift".into());
    }
    let planned = manifest
        .get("planned_crate")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    if planned != 0 {
        return Err(format!("planned crates should be zero, got {planned}"));
    }

    // Every workspace member is covered by a product's owned crates or the
    // shared list (the ownership denominator).
    let identities = intent_model::parse_crate_identities_v1(
        &std::fs::read_to_string(root.join("policy/product-crates-v2.toml"))
            .map_err(|err| format!("read identity authority: {err}"))?,
    )?;
    let workspace: toml::Table = toml::from_str(
        &std::fs::read_to_string(root.join("Cargo.toml"))
            .map_err(|err| format!("read workspace manifest: {err}"))?,
    )
    .map_err(|err| format!("parse workspace manifest: {err}"))?;
    let members: BTreeSet<String> = workspace
        .get("workspace")
        .and_then(|w| w.get("members"))
        .and_then(|m| m.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .ok_or("workspace manifest missing members")?;
    for member in &members {
        if !identities
            .iter()
            .any(|identity| identity.workspace_path == *member)
        {
            return Err(format!(
                "workspace member {member} has no identity coverage"
            ));
        }
    }

    let law = root.join("docs/architecture/product-crate-law.md");
    let law_text = std::fs::read_to_string(&law)
        .map_err(|err| format!("product crate law readable: {err}"))?;
    if !law_text.contains("cargo-allow") {
        return Err("human projection missing cargo-allow ownership".to_string());
    }
    Ok(())
}

#[test]
fn dependency_law_closure_validation_is_clean_on_the_workspace() -> Result<(), String> {
    // The intent-engine closure validation (#3329) is the dependency-law
    // engine: build the observed graph from the workspace manifests (via
    // the cargo-intent governance reader at dev scope) and validate the
    // live law against it. No forbidden edge, no missing required edge.
    let root = repo_root();
    let (forbidden, required) = intent_model::parse_dependency_law_v1(
        &std::fs::read_to_string(root.join("policy/product-crates.toml"))
            .map_err(|err| format!("read dependency law: {err}"))?,
    )?;
    if forbidden.is_empty() {
        return Err("dependency law must record forbidden edges".into());
    }

    let identities = intent_model::parse_crate_identities_v1(
        &std::fs::read_to_string(root.join("policy/product-crates-v2.toml"))
            .map_err(|err| format!("read identity authority: {err}"))?,
    )?;
    // Map observed package-name edges to logical ids via the identities.
    let mut adjacency: BTreeSet<(String, String)> = BTreeSet::new();
    for identity in &identities {
        let manifest_path = root.join(&identity.workspace_path).join("Cargo.toml");
        let manifest_text = std::fs::read_to_string(&manifest_path)
            .map_err(|err| format!("read {}: {err}", manifest_path.display()))?;
        let manifest: toml::Table = toml::from_str(&manifest_text)
            .map_err(|err| format!("parse {}: {err}", manifest_path.display()))?;
        for section in ["dependencies", "dev-dependencies"] {
            let Some(deps) = manifest.get(section).and_then(|v| v.as_table()) else {
                continue;
            };
            for dep_name in deps.keys() {
                adjacency.insert((identity.cargo_package_name.clone(), dep_name.clone()));
            }
        }
    }

    let package_to_logical: std::collections::BTreeMap<&str, &str> = identities
        .iter()
        .map(|identity| {
            (
                identity.cargo_package_name.as_str(),
                identity.logical_id.as_str(),
            )
        })
        .collect();
    let logical_edges: BTreeSet<(String, String)> = adjacency
        .iter()
        .filter_map(|(from, to)| {
            let from_logical = package_to_logical.get(from.as_str())?;
            let to_logical = package_to_logical.get(to.as_str())?;
            Some((from_logical.to_string(), to_logical.to_string()))
        })
        .collect();

    for edge in &forbidden {
        if logical_edges.contains(&(edge.from_logical_id.clone(), edge.to_logical_id.clone())) {
            return Err(format!(
                "workspace violates the forbidden edge {} -> {}",
                edge.from_logical_id, edge.to_logical_id
            ));
        }
    }
    for edge in &required {
        if !logical_edges.contains(&(edge.from_logical_id.clone(), edge.to_logical_id.clone())) {
            return Err(format!(
                "workspace misses the required edge {} -> {}",
                edge.from_logical_id, edge.to_logical_id
            ));
        }
    }
    Ok(())
}

#[test]
fn intent_protocol_is_recorded_as_sole_obligation_input_for_proof() -> Result<(), String> {
    let root = repo_root();
    let (forbidden, required) = intent_model::parse_dependency_law_v1(
        &std::fs::read_to_string(root.join("policy/product-crates.toml"))
            .map_err(|err| format!("read dependency law: {err}"))?,
    )?;

    let converged = required.iter().any(|rule| {
        rule.from_logical_id == "proof-engine" && rule.to_logical_id == "intent-protocol"
    });
    if !converged {
        return Err(
            "dependency law must record proof-engine -> intent-protocol as the converged              obligation-input path (#2936/#3317)"
                .into(),
        );
    }

    let forbidden_intent_edges: Vec<_> = forbidden
        .iter()
        .filter(|rule| {
            rule.from_logical_id == "proof-engine" && rule.to_logical_id.starts_with("intent-")
        })
        .filter(|rule| rule.to_logical_id != "intent-protocol")
        .collect();
    if forbidden_intent_edges.is_empty() {
        return Err(
            "proof-engine must stay forbidden from intent-engine/intent-model internals (#3317)"
                .into(),
        );
    }
    Ok(())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn workspace_member_paths() -> Result<Vec<String>, String> {
    let root = repo_root();
    let text = std::fs::read_to_string(root.join("Cargo.toml"))
        .map_err(|err| format!("read workspace manifest: {err}"))?;
    let manifest: toml::Table =
        toml::from_str(&text).map_err(|err| format!("parse workspace manifest: {err}"))?;
    manifest
        .get("workspace")
        .and_then(|w| w.get("members"))
        .and_then(|m| m.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .ok_or_else(|| "workspace manifest missing members".to_string())
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
    let members = workspace_member_paths()?;
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
    // #3356 re-based (#3562): a seeded forbidden logical edge (cargo-allow
    // linking intent-engine) is flagged by the intent-engine closure
    // validation against the live dependency law.
    let root = repo_root();
    let identities = intent_model::parse_crate_identities_v1(
        &std::fs::read_to_string(root.join("policy/product-crates-v2.toml"))
            .map_err(|err| format!("read identity authority: {err}"))?,
    )?;
    let (forbidden, required) = intent_model::parse_dependency_law_v1(
        &std::fs::read_to_string(root.join("policy/product-crates.toml"))
            .map_err(|err| format!("read dependency law: {err}"))?,
    )?;

    // Seed: start from every identity observed (package names cover the
    // closure), then add one forbidden cargo-allow -> intent-engine edge.
    let mut packages: Vec<String> = identities
        .iter()
        .map(|identity| identity.cargo_package_name.clone())
        .collect();
    packages.push("intent-compiler".to_string());
    let mut edges: Vec<intent_engine::ObservedDependencyEdgeV2> = identities
        .iter()
        .filter(|identity| identity.logical_id == "proof-engine")
        .map(|identity| intent_engine::ObservedDependencyEdgeV2 {
            from_package: identity.cargo_package_name.clone(),
            to_package: "intent-protocol".to_string(),
            class: intent_engine::ObservedDependencyClassV2::Normal,
        })
        .collect();
    // allow-policy -> intent-compiler resolves to the live forbidden edge
    // allow-policy -> intent-engine in the dependency law.
    edges.push(intent_engine::ObservedDependencyEdgeV2 {
        from_package: "allow-policy".to_string(),
        to_package: "intent-compiler".to_string(),
        class: intent_engine::ObservedDependencyClassV2::Normal,
    });
    let graph = intent_engine::ObservedMetadataGraphV2 { packages, edges };

    let input = intent_engine::ClosureValidationInputV2 {
        observed: &graph,
        identities: &identities,
        forbidden_edges: &forbidden,
        required_edges: &required,
    };
    let report = intent_engine::validate_observed_closure(&input);
    let flagged = report.findings.iter().any(|finding| {
        finding.kind == intent_engine::ClosureFindingKindV2::ForbiddenDependency
            && finding.message.contains("allow-policy")
            && finding.message.contains("intent-engine")
    });
    if !flagged {
        return Err(format!(
            "seeded allow-policy -> intent-engine normal edge was not flagged: {:?}",
            report.findings
        ));
    }
    Ok(())
}
