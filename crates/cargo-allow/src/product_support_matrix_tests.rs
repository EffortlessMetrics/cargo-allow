//! Per-product support matrix validator (#3359 / #2559).
//!
//! Loads `policy/product-support-matrix.toml` and proves every product family
//! has a checked support tier, that postures match the V2 topology source of
//! truth, and that no product doc or release note implies another product is
//! installed/supported/stable without explicit opt-in.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read_policy(name: &str) -> Result<String, String> {
    let root = workspace_root();
    let path = root.join("policy").join(name);
    std::fs::read_to_string(&path).map_err(|e| format!("read policy/{name}: {e}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum SupportTier {
    Supported,
    ExperimentalOptIn,
    InternalStabilizing,
    InternalExperimental,
    Legacy,
}

impl SupportTier {
    fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "supported" => Ok(SupportTier::Supported),
            "experimental-opt-in" => Ok(SupportTier::ExperimentalOptIn),
            "internal-stabilizing" => Ok(SupportTier::InternalStabilizing),
            "internal-experimental" => Ok(SupportTier::InternalExperimental),
            "legacy" => Ok(SupportTier::Legacy),
            other => Err(format!("unknown support_tier `{other}`")),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SupportMatrixToml {
    schema_id: String,
    schema_version: u32,
    controlling_issue: u32,
    generated_by: String,
    #[serde(rename = "product")]
    products: Vec<ProductEntryToml>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProductEntryToml {
    product_id: String,
    posture: String,
    support_tier: String,
    installed_by_default: bool,
    stability_claim: String,
    claim_boundary: String,
}

#[derive(Debug, Clone)]
struct ProductEntry {
    product_id: String,
    posture: String,
    support_tier: SupportTier,
    installed_by_default: bool,
    stability_claim: String,
    claim_boundary: String,
}

fn load_matrix() -> Result<Vec<ProductEntry>, String> {
    let text = read_policy("product-support-matrix.toml")?;
    let raw: SupportMatrixToml =
        toml::from_str(&text).map_err(|e| format!("parse product-support-matrix.toml: {e}"))?;
    if raw.schema_id != "cargo-allow.product-support-matrix.v1" {
        return Err(format!(
            "unexpected schema_id `{}`; expected cargo-allow.product-support-matrix.v1",
            raw.schema_id
        ));
    }
    if raw.schema_version != 1 {
        return Err(format!(
            "unexpected schema_version {}; expected 1",
            raw.schema_version
        ));
    }
    if raw.controlling_issue != 2559 {
        return Err(format!(
            "unexpected controlling_issue {}; expected 2559",
            raw.controlling_issue
        ));
    }
    if !raw.generated_by.contains("#3359") {
        return Err(format!(
            "generated_by must reference #3359; found `{}`",
            raw.generated_by
        ));
    }
    raw.products
        .into_iter()
        .map(|p| {
            Ok(ProductEntry {
                support_tier: SupportTier::parse(&p.support_tier)?,
                product_id: p.product_id,
                posture: p.posture,
                installed_by_default: p.installed_by_default,
                stability_claim: p.stability_claim,
                claim_boundary: p.claim_boundary,
            })
        })
        .collect()
}

/// Parse `posture = "..."` values from the V2 topology, grouped by the
/// product family each package belongs to.
fn topology_postures_by_product() -> Result<BTreeMap<String, String>, String> {
    let text = read_policy("product-package-topology-v2.toml")?;
    let mut package_postures: BTreeMap<String, String> = BTreeMap::new();
    let mut current_package: Option<String> = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("[[package]]") {
            current_package = None;
        } else if let Some(rest) = trimmed.strip_prefix("logical_id = ") {
            current_package = Some(rest.trim_matches('"').to_string());
        } else if let Some(rest) = trimmed.strip_prefix("posture = ") {
            let posture = rest.trim_matches('"').to_string();
            if let Some(pkg) = &current_package {
                package_postures.insert(pkg.clone(), posture);
            }
        }
    }
    // The topology is per-package; we do not need to aggregate to product
    // here because the support matrix declares product-level postures. We
    // return the map so tests can assert each product's declared posture
    // actually appears in the topology.
    Ok(package_postures)
}

/// Detect whether a documentation string implies another product is
/// installed/supported/stable without explicit opt-in framing.
///
/// Returns the first offending phrase found, or None if the text is clean.
fn implies_other_product_stability(text: &str, owning_product: &str) -> Option<String> {
    let lowered = text.to_lowercase();
    // Forbidden implication patterns: one product claiming another is
    // stable/supported/installed without opt-in language.
    let other_products: &[(&str, &str)] = if owning_product == "cargo-allow" {
        &[
            ("cargo-intent", "cargo-intent"),
            ("cargo-proof", "cargo-proof"),
        ]
    } else if owning_product == "cargo-intent" {
        &[
            ("cargo-proof", "cargo-proof"),
            ("cargo-allow", "cargo-allow application"),
        ]
    } else if owning_product == "cargo-proof" {
        &[
            ("cargo-intent", "cargo-intent"),
            ("cargo-allow", "cargo-allow application"),
        ]
    } else {
        &[]
    };
    let stability_words = ["stable", "supported", "installed", "production-ready"];
    let opt_in_phrases = [
        "opt-in",
        "optional",
        "experimental",
        "not installed by default",
    ];

    for (product_label, _) in other_products {
        for word in &stability_words {
            let phrase = format!("{product_label} {word}");
            if let Some(idx) = lowered.find(&phrase) {
                // Check whether opt-in framing appears within 120 chars of
                // the implication. If it does, the claim is bounded.
                let window_start = idx.saturating_sub(120);
                let window_end = (idx + phrase.len() + 120).min(lowered.len());
                let window = lowered.get(window_start..window_end).unwrap_or(&lowered);
                let bounded = opt_in_phrases.iter().any(|p| window.contains(p));
                if !bounded {
                    return Some(phrase);
                }
            }
        }
    }
    None
}

#[test]
fn manifest_loads_and_covers_all_products() -> Result<(), String> {
    let products = load_matrix()?;
    if products.is_empty() {
        return Err("product-support-matrix.toml has no products".into());
    }
    let required = [
        "cargo-allow",
        "cargo-intent",
        "cargo-proof",
        "shared-protocols",
        "legacy-migration",
    ];
    let ids: std::collections::HashSet<&str> =
        products.iter().map(|p| p.product_id.as_str()).collect();
    for req in &required {
        if !ids.contains(req) {
            return Err(format!(
                "product-support-matrix.toml is missing required product `{req}`; found: {ids:?}"
            ));
        }
    }
    // Every entry must have non-empty required fields.
    for p in &products {
        for (label, value) in [
            ("product_id", p.product_id.as_str()),
            ("posture", p.posture.as_str()),
            ("stability_claim", p.stability_claim.as_str()),
            ("claim_boundary", p.claim_boundary.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(format!(
                    "product `{}` has empty required field `{}`",
                    p.product_id, label
                ));
            }
        }
        // support_tier must be consistent with posture: experimental postures
        // map to experimental-opt-in, supported to supported, etc.
        let expected_tier = match p.posture.as_str() {
            "CargoAllowSupported" => SupportTier::Supported,
            "CargoIntentExperimental" | "CargoProofExperimental" => SupportTier::ExperimentalOptIn,
            "SharedProtocolInternalOrStabilizing" | "SharedImplementationInternalOrStabilizing" => {
                SupportTier::InternalStabilizing
            }
            "SharedImplementationInternalOrExperimental" => SupportTier::InternalExperimental,
            "LegacyMigration" => SupportTier::Legacy,
            other => {
                return Err(format!(
                    "product `{}` has unknown posture `{other}`",
                    p.product_id
                ));
            }
        };
        if p.support_tier != expected_tier {
            return Err(format!(
                "product `{}` has support_tier {:?} but posture `{}` implies {:?}",
                p.product_id, p.support_tier, p.posture, expected_tier
            ));
        }
    }
    Ok(())
}

#[test]
fn postures_match_v2_topology() -> Result<(), String> {
    let products = load_matrix()?;
    let topology = topology_postures_by_product()?;
    let topology_postures: std::collections::HashSet<&str> =
        topology.values().map(|s| s.as_str()).collect();
    for p in &products {
        if !topology_postures.contains(p.posture.as_str()) {
            return Err(format!(
                "product `{}` declares posture `{}` which does not appear in policy/product-package-topology-v2.toml; postures must mirror the V2 topology source of truth",
                p.product_id, p.posture
            ));
        }
    }
    Ok(())
}

#[test]
fn only_cargo_allow_is_installed_by_default() -> Result<(), String> {
    let products = load_matrix()?;
    let defaults: Vec<&str> = products
        .iter()
        .filter(|p| p.installed_by_default)
        .map(|p| p.product_id.as_str())
        .collect();
    if defaults.len() != 2 {
        return Err(format!(
            "expected exactly two installed_by_default products (cargo-allow + shared-protocols); found {defaults:?}"
        ));
    }
    if !defaults.contains(&"cargo-allow") {
        return Err("cargo-allow must be installed_by_default = true".into());
    }
    // cargo-intent and cargo-proof must NOT be installed by default.
    for p in &products {
        if matches!(p.product_id.as_str(), "cargo-intent" | "cargo-proof") && p.installed_by_default
        {
            return Err(format!(
                "product `{}` must have installed_by_default = false; installing cargo-allow must not imply it is present",
                p.product_id
            ));
        }
    }
    Ok(())
}

#[test]
fn product_docs_do_not_imply_other_products_stability() -> Result<(), String> {
    // Scan every product's documentation surfaces for cross-product
    // stability implications without opt-in framing (#3447: all three
    // products now carry the full seven-doc surface).
    let root = workspace_root();
    let products = [
        (
            "cargo-allow",
            vec![
                "docs/getting-started.md",
                "docs/release/0.2.0.md",
                "docs/status/SUPPORT_TIERS.md",
            ],
        ),
        ("cargo-intent", vec![]),
        ("cargo-proof", vec![]),
    ];
    const DOC_KINDS: &[&str] = &[
        "getting-started",
        "command-reference",
        "schemas",
        "limitations",
        "compatibility",
        "support-and-security",
        "release-notes",
    ];
    for (product, extra_docs) in &products {
        let mut docs_to_check: Vec<String> = extra_docs.iter().map(|doc| doc.to_string()).collect();
        for kind in DOC_KINDS {
            docs_to_check.push(format!("docs/products/{product}/{kind}.md"));
        }
        for doc_rel in &docs_to_check {
            let path = root.join(doc_rel);
            if !path.is_file() {
                continue;
            }
            let text =
                std::fs::read_to_string(&path).map_err(|e| format!("read {doc_rel}: {e}"))?;
            if let Some(phrase) = implies_other_product_stability(&text, product) {
                return Err(format!(
                    "{doc_rel} implies `{phrase}` without opt-in framing; {product} docs must not imply another product is installed/supported/stable"
                ));
            }
        }
    }
    Ok(())
}

#[test]
fn product_docs_are_independent_and_indexed() -> Result<(), String> {
    let root = workspace_root();
    let docs = [
        "getting-started",
        "command-reference",
        "schemas",
        "limitations",
        "compatibility",
        "support-and-security",
        "release-notes",
    ];
    let products = ["cargo-allow", "cargo-intent", "cargo-proof"];
    for product in products {
        product_docs_check(&root, product, &docs)?;
    }
    product_index_check(&root)?;
    Ok(())
}

fn product_docs_check(root: &std::path::Path, product: &str, docs: &[&str]) -> Result<(), String> {
    let index = std::fs::read_to_string(root.join("docs/README.md"))
        .map_err(|error| format!("read docs/README.md: {error}"))?;
    for doc in docs {
        let relative = format!("docs/products/{product}/{doc}.md");
        let path = root.join(&relative);
        if !path.is_file() {
            return Err(format!("missing {product} product doc {relative}"));
        }
        let content =
            std::fs::read_to_string(&path).map_err(|error| format!("read {relative}: {error}"))?;
        if !content.contains(product) {
            return Err(format!("{product} product doc lacks identity: {relative}"));
        }
        let link = format!("products/{product}/{doc}.md");
        if !index.contains(&link) {
            return Err(format!("docs/README.md does not link {link}"));
        }
    }
    if product != "cargo-allow" {
        return Ok(());
    }
    let command_reference =
        std::fs::read_to_string(root.join("docs/products/cargo-allow/command-reference.md"))
            .map_err(|error| format!("read command reference: {error}"))?;
    for marker in [
        "core command reference",
        "not an exhaustive command inventory",
        "published-command-registry.toml",
        "propose --summary-format json --summary-output <path>",
        "add --summary-format json --summary-output <path>",
    ] {
        if !command_reference.contains(marker) {
            return Err(format!(
                "command reference lacks completeness marker {marker}"
            ));
        }
    }
    let schemas = std::fs::read_to_string(root.join("docs/products/cargo-allow/schemas.md"))
        .map_err(|error| format!("read schema catalog: {error}"))?;
    for marker in [
        "core schema and artifact catalog",
        "not an exhaustive artifact inventory",
        "schemas/README.md",
    ] {
        if !schemas.contains(marker) {
            return Err(format!("schema catalog lacks completeness marker {marker}"));
        }
    }
    Ok(())
}

fn product_index_check(root: &std::path::Path) -> Result<(), String> {
    let index = std::fs::read_to_string(root.join("docs/README.md"))
        .map_err(|error| format!("read docs/README.md: {error}"))?;
    for product in ["cargo-allow", "cargo-intent", "cargo-proof"] {
        let link = format!("products/{product}/getting-started.md");
        if !index.contains(&link) {
            return Err(format!(
                "docs/README.md does not index {product} getting-started"
            ));
        }
    }
    Ok(())
}

#[test]
fn no_product_claims_another_products_stability() -> Result<(), String> {
    // Seeded fixture: a cargo-allow release note claiming cargo-intent is
    // stable without opt-in framing must be flagged.
    let seeded_unbounded =
        "cargo-allow 0.2.0 ships with cargo-intent stable and ready for production use.";
    if implies_other_product_stability(seeded_unbounded, "cargo-allow").is_none() {
        return Err(
            "seeded unbounded stability claim (`cargo-intent stable`) was not detected".into(),
        );
    }

    // Seeded fixture: the same claim WITH opt-in framing must pass.
    let seeded_bounded = "cargo-allow 0.2.0 delegates to cargo-intent (experimental opt-in, not installed by default) when explicitly configured.";
    if implies_other_product_stability(seeded_bounded, "cargo-allow").is_some() {
        return Err(
            "seeded bounded stability claim (with opt-in framing) was incorrectly flagged".into(),
        );
    }
    Ok(())
}
