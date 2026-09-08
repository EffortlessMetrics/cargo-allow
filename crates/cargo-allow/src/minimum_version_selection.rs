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
    derive_selection_for_roots(root, roots)
}

/// The derivation over explicit package roots (the registry wrapper
/// above is the production entry point; the explicit form lets the
/// fixture tests exercise the same BFS on a synthetic workspace).
pub(crate) fn derive_selection_for_roots(
    root: &Path,
    roots: Vec<String>,
) -> Result<ProductSelection, String> {
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
                let member_optional = spec
                    .get("optional")
                    .and_then(toml::Value::as_bool)
                    .unwrap_or(false);
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
                    let resolved_optional = table
                        .get("optional")
                        .and_then(toml::Value::as_bool)
                        .unwrap_or(false);
                    if (member_optional || resolved_optional) && !default_enables(&manifest, dep) {
                        // Optional dependencies stay out of the closure
                        // and the certified floor inventory unless a
                        // default feature enables them; an explicit
                        // `optional = false` never excludes a
                        // dependency.
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
/// manifests: the root manifest plus the union of the `crates/*`
/// scan and every declared workspace member's manifest, each framed as
/// `path\0content\0` — byte-for-byte the stream the shell proof
/// hashes.
pub(crate) fn manifest_set_digest(root: &Path) -> Result<String, String> {
    let mut hasher = Sha256::new();
    let root_manifest = std::fs::read(root.join("Cargo.toml"))
        .map_err(|error| format!("root manifest reads: {error}"))?;
    hasher.update(b"Cargo.toml");
    hasher.update([0u8]);
    hasher.update(cr_stripped(&root_manifest));
    hasher.update([0u8]);

    let crates_dir = root.join("crates");
    let mut manifest_paths: BTreeSet<String> = BTreeSet::new();
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
            manifest_paths.insert(relative);
        }
    }
    // Bind declared workspace members too: a member manifest outside
    // or nested under crates/ must move the digest when it changes.
    // Today's members are all direct crates/ children, so the union
    // hashes the same set the shell recipe has always hashed.
    let ws = parse_manifest(&root.join("Cargo.toml"))?;
    let members = ws
        .get("workspace")
        .and_then(|workspace| workspace.get("members"))
        .and_then(toml::Value::as_array)
        .ok_or("the root manifest has no workspace members array")?;
    for member in members {
        let member = member
            .as_str()
            .ok_or("a workspace member entry is not a string")?;
        manifest_paths.insert(format!("{member}/Cargo.toml"));
    }
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

/// Whether a default feature of the member transitively enables the
/// dependency (`dep:x` edges or legacy same-name feature edges). The
/// certified set is the default-feature compile set: an optional
/// dependency a default feature enables is compiled — and certifiable
/// — by the proof classes.
fn default_enables(manifest: &toml::Table, dep: &str) -> bool {
    let features = manifest.get("features").and_then(toml::Value::as_table);
    let Some(features) = features else {
        return false;
    };
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut stack: Vec<String> = features
        .get("default")
        .and_then(toml::Value::as_array)
        .map(|default| {
            default
                .iter()
                .filter_map(|value| value.as_str())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    while let Some(feature) = stack.pop() {
        if !seen.insert(feature.clone()) {
            continue;
        }
        let edges = features
            .get(&feature)
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten();
        for edge in edges {
            let Some(edge) = edge.as_str() else {
                continue;
            };
            let name = edge.strip_prefix("dep:").unwrap_or(edge);
            if name == dep {
                return true;
            }
            if features.contains_key(name) {
                stack.push(name.to_string());
            }
        }
    }
    false
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

    static FIXTURE_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    /// A synthetic workspace with a member inside `crates/` (alpha) and
    /// one outside (`vendor/beta`), exercising the optional-dependency
    /// and digest-membership rules: `enabled` is member-optional but
    /// enabled by a default feature, `skipped` is optional with no
    /// default feature enabling it, and `serde` carries an explicit
    /// `optional = false` that must never exclude it.
    fn write_fixture_workspace(lf: bool) -> std::path::PathBuf {
        let id = FIXTURE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let root =
            std::env::temp_dir().join(format!("min-version-selection-{}-{id}", std::process::id()));
        let newline = if lf { "\n" } else { "\r\n" };
        let write = |relative: &str, body: &str| {
            let path = root.join(relative);
            std::fs::create_dir_all(path.parent().expect("fixture parent")).expect("dir writes");
            std::fs::write(path, body.replace('\n', newline)).expect("fixture writes");
        };
        write(
            "Cargo.toml",
            r#"[workspace]
resolver = "2"
members = ["crates/alpha", "vendor/beta"]

[workspace.dependencies]
serde = "1.0"
"#,
        );
        write(
            "crates/alpha/Cargo.toml",
            r#"[package]
name = "alpha"
version = "0.1.0"

[features]
default = ["parses"]
parses = ["dep:enabled"]

[dependencies]
serde = { workspace = true, optional = false }
enabled = { version = "0.3", optional = true }
skipped = { version = "0.4", optional = true }
"#,
        );
        write(
            "vendor/beta/Cargo.toml",
            r#"[package]
name = "beta"
version = "0.1.0"

[dependencies]
shared = "0.9"
"#,
        );
        root
    }

    #[test]
    fn optional_dependencies_follow_the_default_feature_set() {
        let root = write_fixture_workspace(true);
        let selection =
            derive_selection_for_roots(&root, vec!["alpha".to_string(), "beta".to_string()])
                .expect("the fixture selection derives");
        let floors: Vec<String> = selection
            .floors
            .iter()
            .map(|floor| floor.0.clone())
            .collect();
        // An explicit `optional = false` never excludes a dependency,
        // a default-feature-enabled optional dependency stays
        // certified, and an optional dependency no default feature
        // enables stays out.
        assert_eq!(
            floors,
            vec![
                "enabled".to_string(),
                "serde".to_string(),
                "shared".to_string()
            ],
            "skipped must stay out; enabled and serde stay in"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn digest_is_line_ending_independent_and_binds_members_outside_crates() {
        let lf_root = write_fixture_workspace(true);
        let crlf_root = write_fixture_workspace(false);
        let lf_digest = manifest_set_digest(&lf_root).expect("lf digest derives");
        let crlf_digest = manifest_set_digest(&crlf_root).expect("crlf digest derives");
        assert_eq!(lf_digest, crlf_digest, "CRLF and LF checkouts hash alike");

        // A member manifest outside crates/ moves the digest when it
        // changes: the union binds declared members, not just the
        // scanned directory.
        let beta = crlf_root.join("vendor/beta/Cargo.toml");
        let text = std::fs::read_to_string(&beta).expect("beta manifest reads");
        std::fs::write(&beta, text.replace("0.9", "0.10")).expect("beta manifest writes");
        let moved = manifest_set_digest(&crlf_root).expect("moved digest derives");
        assert_ne!(
            moved, crlf_digest,
            "a member manifest change moves the digest"
        );
        let _ = std::fs::remove_dir_all(&lf_root);
        let _ = std::fs::remove_dir_all(&crlf_root);
    }
}
