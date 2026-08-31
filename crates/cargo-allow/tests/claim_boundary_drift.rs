//! Package claim-boundary drift contract (#3908 Slice C).
//!
//! Machine-checks a bounded table of documented package dependency /
//! non-dependency claims against the parsed workspace manifests. Edge
//! categories stay distinct: normal, dev, build, optional, public-protocol,
//! application, private-implementation, and installed-process edges are never
//! flattened. Prose-only architectural nuance stays linked to the canonical
//! dependency law (`docs/adr/CARGO-ALLOW-ADR-0002-three-product-ownership.md`,
//! `policy/product-crates.toml`); this contract does not duplicate the full
//! graph.
//!
//! Checked claim table (doc surface → manifest relationship):
//!
//! | Documented claim (surface) | Checked relationship |
//! | --- | --- |
//! | proof-protocol README/lib.rs: does not depend on intent, engine, or application crates | proof-protocol normal deps avoid allow, intent, and proof families |
//! | cargo-proof README/lib.rs: `intent-protocol` is the accepted public obligation-transport seam; no cargo-intent application dependency | cargo-proof normal deps include `intent-protocol` and avoid intent application crates, the cargo-allow family, and the proof family |
//! | cargo-proof normal dependency set is closed | every cargo-proof normal dep resolves into the declared set (catches seeded reverse edges) |
//! | cargo-allow: production deps contain no intent/proof crates; intent parity edges are dev-scope | normal/build deps avoid intent and proof families; dev deps carry `intent-model`, `intent-protocol`, `intent-engine`, `effortless-rust-source-index` |
//! | cargo-intent: proof execution remains outside | no proof-family dependency in any scope |
//! | intent-edit README: `intent-engine` must not depend on `intent-edit` | intent-engine normal deps exclude `intent-edit` |
//! | effortless-* substrate product-neutrality contracts (lib.rs) | substrate normal deps avoid all product families |
//!
//! Installed-process integration (cargo-allow delegating to an installed
//! `cargo-intent` binary) carries no build-time edge and is asserted only as
//! the absence of an application dependency, never as a flattened denial of
//! the `intent-protocol` public protocol.

use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use toml::Value;

fn repo_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let crates_dir = manifest_dir
        .parent()
        .ok_or_else(|| io::Error::other("no crates dir parent"))?;
    let root = crates_dir
        .parent()
        .ok_or_else(|| io::Error::other("no repo root"))?;
    Ok(root.to_path_buf())
}

fn read_repo_text(root: &Path, rel: &str) -> Result<String, Box<dyn Error>> {
    let path = root.join(rel);
    let text = fs::read_to_string(&path)
        .map_err(|error| io::Error::other(format!("read {rel}: {error}")))?;
    Ok(text)
}

/// Whitespace-normalized single-line form so claim fragments can be matched
/// without depending on rustdoc line wrapping. Rustdoc `//!`/`///` markers are
/// stripped so wrapped doc-comment fragments match README fragments.
fn normalized(text: &str) -> String {
    let stripped = text.replace("//!", " ").replace("///", " ");
    stripped.split_whitespace().collect::<Vec<&str>>().join(" ")
}

fn normalized_repo_text(root: &Path, rel: &str) -> Result<String, Box<dyn Error>> {
    Ok(normalized(&read_repo_text(root, rel)?))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EdgeScope {
    Normal,
    Dev,
    Build,
}

#[derive(Clone, Debug)]
struct ResolvedEdge {
    /// Physical workspace package name (crate directory for path edges) or
    /// registry crate name (honoring `package =` renames).
    package: String,
    scope: EdgeScope,
    optional: bool,
}

fn scope_for_section(key: &str) -> Option<EdgeScope> {
    match key {
        "dependencies" => Some(EdgeScope::Normal),
        "dev-dependencies" => Some(EdgeScope::Dev),
        "build-dependencies" => Some(EdgeScope::Build),
        _ => None,
    }
}

/// `(dep key, crate directory)` for every path-carrying entry in the root
/// `[workspace.dependencies]` table.
fn workspace_dependency_paths(root: &Path) -> Result<Vec<(String, String)>, Box<dyn Error>> {
    let text = read_repo_text(root, "Cargo.toml")?;
    let manifest: Value =
        toml::from_str(&text).map_err(|error| io::Error::other(format!("parse root: {error}")))?;
    let mut resolved = Vec::new();
    let Some(deps) = manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("dependencies"))
        .and_then(Value::as_table)
    else {
        return Ok(resolved);
    };
    for (key, value) in deps {
        let Some(path) = value.get("path").and_then(Value::as_str) else {
            continue;
        };
        let dir = Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| io::Error::other(format!("workspace dep {key} path has no segment")))?;
        resolved.push((key.clone(), dir.to_string()));
    }
    Ok(resolved)
}

fn resolve_edge(
    key: &str,
    entry: &Value,
    scope: EdgeScope,
    workspace_paths: &[(String, String)],
) -> Result<ResolvedEdge, Box<dyn Error>> {
    let optional = entry
        .get("optional")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let package = if entry.get("workspace").and_then(Value::as_bool) == Some(true) {
        match workspace_paths.iter().find(|(dep_key, _)| dep_key == key) {
            Some((_, dir)) => dir.clone(),
            // External workspace requirement with no path edge: the dep key
            // is the registry crate name.
            None => key.to_string(),
        }
    } else if let Some(renamed) = entry.get("package").and_then(Value::as_str) {
        renamed.to_string()
    } else if let Some(path) = entry.get("path").and_then(Value::as_str) {
        let dir = Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| io::Error::other(format!("dep {key} path has no segment")))?;
        dir.to_string()
    } else {
        key.to_string()
    };
    Ok(ResolvedEdge {
        package,
        scope,
        optional,
    })
}

fn section_edges(
    section_label: &str,
    value: &Value,
    scope: EdgeScope,
    workspace_paths: &[(String, String)],
    manifest_label: &str,
) -> Result<Vec<ResolvedEdge>, Box<dyn Error>> {
    let mut edges = Vec::new();
    let Some(entries) = value.as_table() else {
        return Ok(edges);
    };
    for (key, entry) in entries {
        let edge = resolve_edge(key, entry, scope, workspace_paths).map_err(|error| {
            io::Error::other(format!("{manifest_label}: {section_label}/{key}: {error}"))
        })?;
        edges.push(edge);
    }
    Ok(edges)
}

fn edges_from_manifest_text(
    text: &str,
    label: &str,
    workspace_paths: &[(String, String)],
) -> Result<Vec<ResolvedEdge>, Box<dyn Error>> {
    let manifest: Value = toml::from_str(text)
        .map_err(|error| io::Error::other(format!("parse {label}: {error}")))?;
    let mut edges = Vec::new();
    let Some(table) = manifest.as_table() else {
        return Ok(edges);
    };
    for (key, value) in table {
        if let Some(scope) = scope_for_section(key) {
            edges.extend(section_edges(key, value, scope, workspace_paths, label)?);
        } else if key == "target" {
            let Some(targets) = value.as_table() else {
                continue;
            };
            for (target_name, target_value) in targets {
                let Some(target_table) = target_value.as_table() else {
                    continue;
                };
                for (section_key, section_value) in target_table {
                    if let Some(scope) = scope_for_section(section_key) {
                        edges.extend(section_edges(
                            &format!("{section_key} (target {target_name})"),
                            section_value,
                            scope,
                            workspace_paths,
                            label,
                        )?);
                    }
                }
            }
        }
    }
    Ok(edges)
}

/// Resolve the dependency edges of one workspace crate under `crates/`.
fn manifest_edges(root: &Path, crate_dir: &str) -> Result<Vec<ResolvedEdge>, Box<dyn Error>> {
    let rel = format!("crates/{crate_dir}/Cargo.toml");
    let text = read_repo_text(root, &rel)?;
    let workspace_paths = workspace_dependency_paths(root)?;
    edges_from_manifest_text(&text, &rel, &workspace_paths)
}

fn packages_in_scope(edges: &[ResolvedEdge], scope: EdgeScope) -> Vec<String> {
    edges
        .iter()
        .filter(|edge| edge.scope == scope)
        .map(|edge| edge.package.clone())
        .collect()
}

const INTENT_APPLICATION_PACKAGES: [&str; 4] = [
    "intent-model",
    "intent-engine",
    "intent-edit",
    "cargo-intent",
];

fn is_intent_application(package: &str) -> bool {
    INTENT_APPLICATION_PACKAGES.contains(&package)
}

fn is_proof_family(package: &str) -> bool {
    matches!(
        package,
        "proof-protocol" | "proof-engine" | "proof-orchestrator" | "cargo-proof"
    )
}

fn is_allow_family(package: &str) -> bool {
    package == "cargo-allow" || package.starts_with("allow-")
}

/// Detects the stale blanket denial ("does not depend on intent crates") on a
/// doc surface whose package manifest deliberately carries an accepted
/// intent-family public protocol edge. The true boundary is against
/// cargo-intent application/private implementation, not every intent-owned
/// public protocol (#3908).
fn stale_denial_findings(doc_text: &str, edges: &[ResolvedEdge]) -> Vec<String> {
    let denies_intent = doc_text.contains("depend on intent crates")
        || doc_text.contains("no dependency on intent crates");
    if !denies_intent {
        return Vec::new();
    }
    let accepted_protocol_edges: Vec<&str> = edges
        .iter()
        .filter(|edge| edge.scope == EdgeScope::Normal && edge.package.starts_with("intent-"))
        .map(|edge| edge.package.as_str())
        .collect();
    if accepted_protocol_edges.is_empty() {
        return Vec::new();
    }
    vec![format!(
        "doc denies intent dependencies while the manifest carries accepted public \
         protocol edge(s) {accepted_protocol_edges:?}; state the public-protocol seam and \
         bound the denial to cargo-intent application crates"
    )]
}

const CARGO_PROOF_DECLARED_NORMAL_DEPENDENCIES: [&str; 10] = [
    "clap",
    "effortless-repo-protocol",
    "effortless-rust-source-index",
    "intent-protocol",
    "proof-engine",
    "proof-protocol",
    "serde",
    "serde_json",
    "sha2",
    "toml",
];

#[test]
fn proof_protocol_data_seam_claim_matches_manifest() -> Result<(), Box<dyn Error>> {
    let root = repo_root()?;
    let edges = manifest_edges(&root, "proof-protocol")?;
    let unexpected: Vec<String> = edges
        .iter()
        .filter(|edge| edge.scope == EdgeScope::Normal)
        .map(|edge| edge.package.clone())
        .filter(|package| {
            is_allow_family(package)
                || package.starts_with("intent-")
                || package == "cargo-intent"
                || is_proof_family(package)
        })
        .collect();
    assert!(
        unexpected.is_empty(),
        "proof-protocol documents that it does not depend on intent, engine, or \
         application crates; unexpected normal dependencies: {unexpected:?}"
    );
    let readme = normalized_repo_text(&root, "crates/proof-protocol/README.md")?;
    assert!(
        readme.contains("depend on intent, engine, or application crates"),
        "the checked proof-protocol README denial sentence drifted"
    );
    Ok(())
}

#[test]
fn cargo_proof_claim_distinguishes_public_protocol_from_application() -> Result<(), Box<dyn Error>>
{
    let root = repo_root()?;
    let edges = manifest_edges(&root, "cargo-proof")?;
    let normal = packages_in_scope(&edges, EdgeScope::Normal);
    assert!(
        normal.contains(&"intent-protocol".to_string()),
        "cargo-proof documents the intent-protocol public obligation-transport seam; \
         it must remain a normal dependency (found {normal:?})"
    );
    let application: Vec<String> = normal
        .iter()
        .filter(|package| is_intent_application(package))
        .cloned()
        .collect();
    assert!(
        application.is_empty(),
        "cargo-proof must not couple to cargo-intent application crates: {application:?}"
    );
    let reverse: Vec<String> = normal
        .iter()
        .filter(|package| is_allow_family(package))
        .cloned()
        .collect();
    assert!(
        reverse.is_empty(),
        "cargo-proof must not depend on cargo-allow product crates \
         (policy/product-crates.toml): {reverse:?}"
    );
    for surface in [
        "crates/cargo-proof/README.md",
        "crates/cargo-proof/src/lib.rs",
    ] {
        let text = normalized_repo_text(&root, surface)?;
        assert!(
            stale_denial_findings(&text, &edges).is_empty(),
            "{surface} must not blanket-deny intent dependencies while \
             intent-protocol is an accepted public protocol edge"
        );
        assert!(
            text.contains("intent-protocol"),
            "{surface} must name the accepted intent-protocol public protocol seam"
        );
        assert!(
            text.contains("public obligation-transport seam"),
            "{surface} must keep the public-protocol distinction from the \
             cargo-intent application boundary"
        );
    }
    Ok(())
}

#[test]
fn cargo_proof_normal_dependency_set_stays_closed() -> Result<(), Box<dyn Error>> {
    let root = repo_root()?;
    let edges = manifest_edges(&root, "cargo-proof")?;
    let outside: Vec<String> = edges
        .iter()
        .filter(|edge| edge.scope == EdgeScope::Normal)
        .map(|edge| edge.package.clone())
        .filter(|package| !CARGO_PROOF_DECLARED_NORMAL_DEPENDENCIES.contains(&package.as_str()))
        .collect();
    assert!(
        outside.is_empty(),
        "new cargo-proof normal dependency needs a claim-boundary review and an \
         ADR-0002 move/shim record before the docs can stay accurate: {outside:?}"
    );
    Ok(())
}

#[test]
fn cargo_allow_intent_edges_stay_dev_scope() -> Result<(), Box<dyn Error>> {
    let root = repo_root()?;
    let edges = manifest_edges(&root, "cargo-allow")?;
    let production: Vec<String> = edges
        .iter()
        .filter(|edge| matches!(edge.scope, EdgeScope::Normal | EdgeScope::Build))
        .map(|edge| edge.package.clone())
        .filter(|package| {
            package.starts_with("intent-") || package == "cargo-intent" || is_proof_family(package)
        })
        .collect();
    assert!(
        production.is_empty(),
        "cargo-allow production dependencies must stay intent/proof-free \
         (feature policy #3364); intent parity runs at dev scope only: {production:?}"
    );
    let dev = packages_in_scope(&edges, EdgeScope::Dev);
    for required in [
        "intent-model",
        "intent-protocol",
        "intent-engine",
        "effortless-rust-source-index",
    ] {
        assert!(
            dev.contains(&required.to_string()),
            "documented dev-scope parity edge {required} is missing from \
             cargo-allow dev-dependencies (found {dev:?})"
        );
    }
    Ok(())
}

#[test]
fn cargo_intent_declares_no_proof_family_dependency() -> Result<(), Box<dyn Error>> {
    let root = repo_root()?;
    let edges = manifest_edges(&root, "cargo-intent")?;
    let proof: Vec<String> = edges
        .iter()
        .map(|edge| edge.package.clone())
        .filter(|package| is_proof_family(package))
        .collect();
    assert!(
        proof.is_empty(),
        "cargo-intent documents that proof execution remains in intent-engine \
         and cargo-proof; unexpected proof-family dependencies: {proof:?}"
    );
    Ok(())
}

#[test]
fn intent_engine_does_not_depend_on_intent_edit() -> Result<(), Box<dyn Error>> {
    let root = repo_root()?;
    let edges = manifest_edges(&root, "intent-engine")?;
    let hits: Vec<String> = edges
        .iter()
        .map(|edge| edge.package.clone())
        .filter(|package| package == "intent-edit")
        .collect();
    assert!(
        hits.is_empty(),
        "intent-edit documents the enforced topology that intent-engine must not \
         depend on intent-edit: {hits:?}"
    );
    let readme = normalized_repo_text(&root, "crates/intent-edit/README.md")?;
    assert!(
        readme.contains("`intent-engine` must not depend on `intent-edit`"),
        "the checked intent-edit README topology sentence drifted"
    );
    Ok(())
}

#[test]
fn shared_substrate_stays_product_neutral() -> Result<(), Box<dyn Error>> {
    let root = repo_root()?;
    for crate_dir in [
        "effortless-repo-protocol",
        "effortless-repo-snapshot",
        "effortless-repo-edit",
        "effortless-rust-source-index",
    ] {
        let edges = manifest_edges(&root, crate_dir)?;
        let product: Vec<String> = edges
            .iter()
            .filter(|edge| edge.scope == EdgeScope::Normal)
            .map(|edge| edge.package.clone())
            .filter(|package| {
                is_allow_family(package)
                    || package.starts_with("intent-")
                    || package == "cargo-intent"
                    || is_proof_family(package)
            })
            .collect();
        assert!(
            product.is_empty(),
            "{crate_dir} documents substrate product-neutrality; unexpected normal \
             dependencies: {product:?}"
        );
    }
    Ok(())
}

/// Negative control for a stale machine-checkable claim (#3908 control 6): a
/// doc that denies every intent dependency despite an accepted
/// `intent-protocol` edge must be flagged, and the live cargo-proof surfaces
/// must stay clean.
#[test]
fn seeded_stale_denial_against_public_protocol_edge_is_flagged() -> Result<(), Box<dyn Error>> {
    let stale_doc = "## Claim boundary\n\ncargo-proof does not depend on intent crates.\n";
    let seeded_manifest = r#"
[dependencies]
intent-protocol = { path = "../intent-protocol", version = "0.1.0" }
"#;
    let edges = edges_from_manifest_text(seeded_manifest, "seeded-stale", &[])?;
    let findings = stale_denial_findings(stale_doc, &edges);
    assert_eq!(
        findings.len(),
        1,
        "the stale denial must be flagged exactly once: {findings:?}"
    );
    for finding in &findings {
        assert!(
            finding.contains("public protocol edge"),
            "the flag must name the accepted public protocol edge: {finding}"
        );
    }

    let clean_doc = "## Claim boundary\n\nThe `intent-protocol` dependency is the accepted \
                     public protocol seam; no cargo-intent application crate is a dependency.\n";
    assert!(
        stale_denial_findings(clean_doc, &edges).is_empty(),
        "a bounded protocol-vs-application statement must stay clean"
    );
    Ok(())
}

/// Negative control for a forbidden reverse dependency: a seeded
/// cargo-proof-shaped manifest reaching into the cargo-allow family must fail
/// both the closed-set check and the family check.
#[test]
fn seeded_forbidden_reverse_dependency_is_flagged() -> Result<(), Box<dyn Error>> {
    let seeded = r#"
[dependencies]
allow-core = { path = "../allow-core", version = "0.2.0-rc.1" }
proof-protocol = { path = "../proof-protocol", version = "0.1.0" }
"#;
    let edges = edges_from_manifest_text(seeded, "seeded-reverse", &[])?;
    let outside: Vec<String> = edges
        .iter()
        .filter(|edge| edge.scope == EdgeScope::Normal)
        .map(|edge| edge.package.clone())
        .filter(|package| !CARGO_PROOF_DECLARED_NORMAL_DEPENDENCIES.contains(&package.as_str()))
        .collect();
    assert_eq!(
        outside,
        vec!["allow-core".to_string()],
        "the forbidden reverse dependency must be flagged by the closed set"
    );
    assert!(
        is_allow_family("allow-core"),
        "the seeded edge must classify as a cargo-allow family edge"
    );
    Ok(())
}

/// Negative control for category flattening (#3908 control 7): dev-scope and
/// optional edges must not be reported as production coupling, and the
/// resolver must keep rename, path, and registry identities distinct.
#[test]
fn edge_scope_and_identity_resolution_do_not_flatten_categories() -> Result<(), Box<dyn Error>> {
    let seeded = r#"
[dependencies]
serde_json = { version = "1" }
proof-orchestrator = { path = "../proof-engine", package = "proof-orchestrator" }

[dev-dependencies]
intent-protocol = { path = "../intent-protocol", optional = true }
"#;
    let edges = edges_from_manifest_text(seeded, "seeded-scopes", &[])?;
    let normal = packages_in_scope(&edges, EdgeScope::Normal);
    let dev = packages_in_scope(&edges, EdgeScope::Dev);
    // An explicit `package =` rename is the crate identity and wins over the
    // path segment: crates/proof-engine publishes as `proof-orchestrator`.
    assert_eq!(
        normal,
        vec!["proof-orchestrator".to_string(), "serde_json".to_string()]
    );
    assert_eq!(dev, vec!["intent-protocol".to_string()]);
    let intent_dev_optional = edges
        .iter()
        .find(|edge| edge.package == "intent-protocol")
        .ok_or_else(|| io::Error::other("seeded intent-protocol edge missing"))?;
    assert!(
        intent_dev_optional.scope == EdgeScope::Dev && intent_dev_optional.optional,
        "the dev-scope optional edge must not flatten into a normal dependency"
    );
    assert!(
        !is_intent_application("proof-engine") && !is_proof_family("intent-protocol"),
        "family classifiers must not cross the intent/proof boundary"
    );
    Ok(())
}
