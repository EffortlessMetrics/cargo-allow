//! Rust derivation of one product's package closure, its declared
//! external direct dependency floors, and the live-tree identity
//! digests (#3903 PR D) — the same derivation
//! scripts/proof-direct-floors.sh performs in python. The retained
//! floor receipts are produced by the shell lane and graded here, so a
//! derivation drift in either implementation fails the live drift test
//! instead of silently diverging.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use sha2::{Digest, Sha256};

/// The proof products and the package roots whose dependency closure
/// each receipt certifies. Mirrors scripts/proof-direct-floors.sh's
/// `product_roots()` (asserted entry-by-entry by the registry mirror
/// test).
pub(crate) const PRODUCT_ROOTS: &[(&str, &[&str])] = &[
    ("cargo-allow", &["cargo-allow"]),
    (
        "shared",
        &[
            "effortless-repo-protocol",
            "effortless-repo-snapshot",
            "effortless-repo-edit",
            "effortless-rust-source-index",
        ],
    ),
    ("cargo-intent", &["cargo-intent"]),
    ("cargo-proof", &["cargo-proof"]),
];

/// The registered roots for one product, or `None` when the product is
/// unregistered (callers must fail closed rather than derive an empty
/// selection).
pub(crate) fn product_roots(product: &str) -> Option<&'static [&'static str]> {
    PRODUCT_ROOTS
        .iter()
        .find(|(name, _)| *name == product)
        .map(|(_, roots)| *roots)
}

/// Every registered product, in registry order.
pub(crate) fn products() -> impl Iterator<Item = &'static str> {
    PRODUCT_ROOTS.iter().map(|(name, _)| *name)
}

/// The default retained receipt path for one product, relative to the
/// repository root.
pub(crate) fn retained_receipt_path(product: &str) -> String {
    format!("docs/ci/receipts/direct-floor-proof-{product}-v1.json")
}

/// One product's derived proof selection: the closure's member
/// packages and the closure's declared external direct dependency
/// floors (package, requirement, floor), sorted by package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProductSelection {
    pub(crate) roots: Vec<String>,
    pub(crate) closure: Vec<String>,
    pub(crate) floors: Vec<(String, String, String)>,
}

/// The workspace-wide dependency graph the derivation walks.
#[derive(Debug)]
struct DependencyGraph {
    ws_deps: toml::Table,
    name_dir: BTreeMap<String, String>,
    dir_name: BTreeMap<String, String>,
}

fn parse_manifest(path: &Path) -> Result<toml::Table, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("manifest {} reads: {error}", path.display()))?;
    text.parse()
        .map_err(|error| format!("manifest {} parses: {error}", path.display()))
}

fn load_dependency_graph(root: &Path) -> Result<DependencyGraph, String> {
    let ws = parse_manifest(&root.join("Cargo.toml"))?;
    let workspace = ws
        .get("workspace")
        .and_then(toml::Value::as_table)
        .ok_or("the root manifest has no [workspace] table")?;
    let ws_deps = workspace
        .get("dependencies")
        .and_then(toml::Value::as_table)
        .ok_or("the root manifest has no [workspace.dependencies] table")?
        .clone();
    let members: Vec<String> = workspace
        .get("members")
        .and_then(toml::Value::as_array)
        .ok_or("the root manifest has no workspace members array")?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| "a workspace member entry is not a string".to_string())
        })
        .collect::<Result<_, _>>()?;

    let mut name_dir: BTreeMap<String, String> = BTreeMap::new();
    let mut dir_name: BTreeMap<String, String> = BTreeMap::new();
    for member in &members {
        let manifest = parse_manifest(&root.join(member).join("Cargo.toml"))?;
        let name = manifest
            .get("package")
            .and_then(|package| package.get("name"))
            .and_then(toml::Value::as_str)
            .ok_or_else(|| format!("member {member} has no package name"))?
            .to_string();
        name_dir.insert(name.clone(), member.clone());
        dir_name.insert(member.clone(), name);
    }
    Ok(DependencyGraph {
        ws_deps,
        name_dir,
        dir_name,
    })
}

/// Resolve a member-relative dependency path against its member
/// directory (`crates/<dir>/../x` style paths normalize the same way
/// the shell derivation does).
fn normalize_member_path(base: &str, relative: &str) -> String {
    let joined = format!("{base}/{relative}");
    let mut parts: Vec<&str> = Vec::new();
    for component in joined.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

/// Derive one product's package closure and declared floors from the
/// checked-in manifests. Fails closed on an unregistered product, a
/// root outside the workspace, git dependencies, or a path dependency
/// that leaves the workspace.
pub(crate) fn derive_selection(root: &Path, product: &str) -> Result<ProductSelection, String> {
    let registered = product_roots(product)
        .ok_or_else(|| format!("product {product} is not in the registry"))?;
    let roots: Vec<String> = registered.iter().map(|name| (*name).to_string()).collect();
    let graph = load_dependency_graph(root)?;

    let mut closure: Vec<String> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut floors: Vec<(String, String, String)> = Vec::new();
    let mut requirements: BTreeMap<String, String> = BTreeMap::new();
    let mut stack: Vec<String> = roots.iter().rev().cloned().collect();
    while let Some(name) = stack.pop() {
        if !seen.insert(name.clone()) {
            continue;
        }
        let member = graph
            .name_dir
            .get(&name)
            .ok_or_else(|| format!("product root {name} is not a workspace member"))?;
        closure.push(name.clone());
        let manifest = parse_manifest(&root.join(member).join("Cargo.toml"))?;
        for table in ["dependencies", "build-dependencies"] {
            let Some(deps) = manifest.get(table).and_then(toml::Value::as_table) else {
                continue;
            };
            for (dep, spec) in deps {
                let inherited = spec
                    .get("workspace")
                    .and_then(toml::Value::as_bool)
                    .unwrap_or(false);
                let resolved: &toml::Value = if inherited {
                    graph.ws_deps.get(dep).ok_or_else(|| {
                        format!("{name} inherits {dep}, but the workspace does not declare it")
                    })?
                } else {
                    spec
                };
                if let Some(table) = resolved.as_table() {
                    if table.contains_key("git") {
                        return Err(format!(
                            "{name} declares a git dependency {dep}; out of proof scope"
                        ));
                    }
                    if table.contains_key("optional") {
                        // Optional dependencies are not part of the
                        // default feature set the proof classes
                        // compile; they stay out of the closure and
                        // the certified floor inventory.
                        continue;
                    }
                    if let Some(path) = table.get("path").and_then(toml::Value::as_str) {
                        let member_dir = graph
                            .name_dir
                            .get(&name)
                            .ok_or_else(|| format!("{name} is not a workspace member"))?;
                        let target = if inherited {
                            path.to_string()
                        } else {
                            normalize_member_path(member_dir, path)
                        };
                        let target_name = graph.dir_name.get(&target).ok_or_else(|| {
                            format!("{name} path dependency {dep} leaves the workspace ({target})")
                        })?;
                        stack.push(target_name.clone());
                        continue;
                    }
                }
                let requirement = match resolved.as_str() {
                    Some(text) => text.to_string(),
                    None => resolved
                        .get("version")
                        .and_then(toml::Value::as_str)
                        .ok_or_else(|| format!("dependency {dep} declares no version requirement"))?
                        .to_string(),
                };
                if let Some(recorded) = requirements.get(dep.as_str()) {
                    if *recorded != requirement {
                        return Err(format!(
                            "{dep} is declared with conflicting requirements \
                             ({recorded} vs {requirement})"
                        ));
                    }
                    continue;
                }
                requirements.insert(dep.clone(), requirement.clone());
                let mut parts: Vec<String> = requirement.split('.').map(str::to_string).collect();
                while parts.len() < 3 {
                    parts.push("0".to_string());
                }
                parts.truncate(3);
                floors.push((dep.clone(), requirement, parts.join(".")));
            }
        }
    }
    floors.sort();
    closure.sort();
    Ok(ProductSelection {
        roots,
        closure,
        floors,
    })
}

/// The manifest-set identity digest over the exact checked-in
/// manifests: the root manifest plus every `crates/<member>/Cargo.toml`,
/// each framed as `path\0content\0` — byte-for-byte the stream the
/// shell proof hashes.
pub(crate) fn manifest_set_digest(root: &Path) -> Result<String, String> {
    let mut hasher = Sha256::new();
    let root_manifest = std::fs::read(root.join("Cargo.toml"))
        .map_err(|error| format!("root manifest reads: {error}"))?;
    hasher.update(b"Cargo.toml");
    hasher.update([0u8]);
    hasher.update(cr_stripped(&root_manifest));
    hasher.update([0u8]);

    let crates_dir = root.join("crates");
    let mut manifest_paths: Vec<String> = Vec::new();
    let entries =
        std::fs::read_dir(&crates_dir).map_err(|error| format!("crates dir reads: {error}"))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("crates dir entry reads: {error}"))?;
        let member_manifest = entry.path().join("Cargo.toml");
        if member_manifest.is_file() {
            let relative = format!(
                "crates/{}/Cargo.toml",
                entry.file_name().to_string_lossy().replace('\\', "/")
            );
            manifest_paths.push(relative);
        }
    }
    manifest_paths.sort();
    for relative in &manifest_paths {
        let bytes = std::fs::read(root.join(relative))
            .map_err(|error| format!("manifest {relative} reads: {error}"))?;
        hasher.update(relative.as_bytes());
        hasher.update([0u8]);
        hasher.update(cr_stripped(&bytes));
        hasher.update([0u8]);
    }
    Ok(hex_sha256(hasher.finalize()))
}

/// Strip CR bytes so the hashed identity is line-ending independent: a
/// CRLF checkout (Windows autocrlf) hashes the same as an LF checkout,
/// mirroring the shell recipe's `tr -d '\r'`.
fn cr_stripped(bytes: &[u8]) -> Vec<u8> {
    bytes
        .iter()
        .copied()
        .filter(|&byte| byte != b'\r')
        .collect()
}

/// The lock identity digest over the `Cargo.lock` bytes, CR-stripped
/// for line-ending independence.
pub(crate) fn lock_digest(root: &Path) -> Result<String, String> {
    let bytes = std::fs::read(root.join("Cargo.lock"))
        .map_err(|error| format!("Cargo.lock reads: {error}"))?;
    let mut hasher = Sha256::new();
    hasher.update(cr_stripped(&bytes));
    Ok(hex_sha256(hasher.finalize()))
}

/// The workspace's claimed MSRV (`workspace.package.rust-version`).
pub(crate) fn workspace_msrv(root: &Path) -> Result<String, String> {
    let ws = parse_manifest(&root.join("Cargo.toml"))?;
    ws.get("workspace")
        .and_then(|workspace| workspace.get("package"))
        .and_then(|package| package.get("rust-version"))
        .and_then(toml::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "the root manifest has no workspace.package.rust-version".to_string())
}

fn hex_sha256(digest: impl AsRef<[u8]>) -> String {
    digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_covers_the_four_proof_products() {
        for product in products() {
            let roots = product_roots(product).expect("registered product has roots");
            assert!(!roots.is_empty(), "{product} registers at least one root");
        }
        assert!(product_roots("not-a-product").is_none());
    }

    #[test]
    fn every_member_manifest_enters_the_digest_framing() {
        let root = std::path::PathBuf::from(
            std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set"),
        )
        .join("../..")
        .canonicalize()
        .expect("workspace root resolves");
        let digest = manifest_set_digest(&root).expect("digest derives");
        assert_eq!(digest.len(), 64, "bare sha256 hex, no prefix");
        let msrv = workspace_msrv(&root).expect("msrv derives");
        assert_eq!(msrv, "1.95");
    }
}
