//! Shared-crate product-neutrality guards (#3146/#3147/#2969).
//!
//! The three shared substrate crates (effortless-repo-snapshot,
//! effortless-rust-source-index, effortless-repo-edit) must stay
//! product-neutral: no product crate (allow-*, intent-*, proof-*,
//! cargo-allow/intent/proof) may appear in any dependency section, and
//! their doc headers must state the neutrality contract. The
//! `allow_core`-referencing helper doc-comments are deliberate
//! byte-compatibility contracts, not dependencies.

use std::path::PathBuf;

const NEUTRAL_CRATES: &[(&str, &str, &str)] = &[
    ("effortless-repo-snapshot", "repo-snapshot", "#3146"),
    ("effortless-rust-source-index", "rust-source-index", "#3147"),
    ("effortless-repo-edit", "repo-edit", "#2969"),
];

const PRODUCT_PREFIXES: &[&str] = &["allow-", "intent-", "proof-", "cargo-"];

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn shared_crates_have_no_product_dependencies() -> Result<(), String> {
    let root = workspace_root();
    for (crate_dir, _logical, issue) in NEUTRAL_CRATES {
        let manifest_text =
            std::fs::read_to_string(root.join("crates").join(crate_dir).join("Cargo.toml"))
                .map_err(|err| format!("read {crate_dir} manifest: {err}"))?;
        let manifest: toml::Table = toml::from_str(&manifest_text)
            .map_err(|err| format!("parse {crate_dir} manifest: {err}"))?;
        for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
            let Some(deps) = manifest.get(section).and_then(|v| v.as_table()) else {
                continue;
            };
            let offenders: Vec<&str> = deps
                .keys()
                .map(String::as_str)
                .filter(|name| {
                    PRODUCT_PREFIXES
                        .iter()
                        .any(|prefix| name.starts_with(prefix))
                })
                .collect();
            if !offenders.is_empty() {
                return Err(format!(
                    "{crate_dir} ({issue}) has product dependencies in {section}: {offenders:?}; \
                     shared substrate crates stay product-neutral"
                ));
            }
        }
    }
    Ok(())
}

#[test]
fn shared_crates_declare_their_neutrality_contract() -> Result<(), String> {
    let root = workspace_root();
    for (crate_dir, _logical, issue) in NEUTRAL_CRATES {
        let lib_text =
            std::fs::read_to_string(root.join("crates").join(crate_dir).join("src/lib.rs"))
                .map_err(|err| format!("read {crate_dir} lib.rs: {err}"))?;
        if !lib_text.contains("Product-neutrality contract") || !lib_text.contains(issue) {
            return Err(format!(
                "{crate_dir} lib.rs must declare its Product-neutrality contract citing {issue}"
            ));
        }
    }
    Ok(())
}

#[test]
fn shared_crate_public_surface_has_no_product_domain_types() -> Result<(), String> {
    let root = workspace_root();
    // Product-domain type names that must not leak into shared public APIs.
    let forbidden = [
        "Finding",
        "AllowEntry",
        "AllowConfig",
        "CheckMode",
        "IntentEnvelope",
        "ProofPlan",
    ];
    for (crate_dir, _logical, issue) in NEUTRAL_CRATES {
        let src_dir = root.join("crates").join(crate_dir).join("src");
        let mut sources = Vec::new();
        collect_rust_files(&src_dir, &mut sources);
        for path in sources {
            let text = std::fs::read_to_string(&path)
                .map_err(|err| format!("read {}: {err}", path.display()))?;
            for line in text.lines() {
                let trimmed = line.trim_start();
                for name in forbidden {
                    let pattern = format!("pub fn {name}");
                    let pattern2 = format!("pub struct {name}");
                    let pattern3 = format!("pub enum {name}");
                    if trimmed.starts_with(&pattern)
                        || trimmed.starts_with(&pattern2)
                        || trimmed.starts_with(&pattern3)
                    {
                        return Err(format!(
                            "{} declares public product-domain item `{name}` ({issue}); \
                             shared crates own neutral types only: {}",
                            path.display(),
                            trimmed.split('{').next().unwrap_or(trimmed)
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

fn collect_rust_files(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}
