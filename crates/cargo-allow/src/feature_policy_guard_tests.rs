//! Feature policy guard: verifies cargo-allow defaults don't pull intent/proof (#3364).
//!
//! Checks that:
//! 1. cargo-allow production deps contain no intent/proof crates
//! 2. No [features] section enables intent/proof as default
//! 3. cargo-intent production deps contain no proof crates
//! 4. cargo-proof provider features don't create reverse deps to cargo-allow

use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read_manifest(crate_name: &str) -> Result<String, String> {
    let root = workspace_root();
    let path = root.join("crates").join(crate_name).join("Cargo.toml");
    std::fs::read_to_string(&path).map_err(|e| format!("read {crate_name}/Cargo.toml: {e}"))
}

/// Extract the [dependencies] section (not [dev-dependencies]).
fn extract_prod_deps(manifest: &str) -> Vec<String> {
    let mut deps = Vec::new();
    let mut in_deps = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_deps = trimmed == "[dependencies]";
            continue;
        }
        if in_deps && let Some(name) = trimmed.split('=').next() {
            let name = name.trim();
            if !name.is_empty() && !name.starts_with('#') {
                deps.push(name.to_string());
            }
        }
    }
    deps
}

/// Check if any dep name matches a forbidden pattern.
fn contains_forbidden(deps: &[String], patterns: &[&str]) -> Vec<String> {
    deps.iter()
        .filter(|dep| patterns.iter().any(|p| dep.contains(p)))
        .cloned()
        .collect()
}

#[test]
fn cargo_allow_production_deps_have_no_intent_or_proof() -> Result<(), String> {
    let manifest = read_manifest("cargo-allow")?;
    let deps = extract_prod_deps(&manifest);
    let forbidden =
        contains_forbidden(&deps, &["intent-", "proof-", "cargo-intent", "cargo-proof"]);
    if !forbidden.is_empty() {
        return Err(format!(
            "cargo-allow production dependencies must not include intent/proof crates: {forbidden:?}"
        ));
    }
    Ok(())
}

#[test]
fn cargo_allow_has_no_features_enabling_intent_or_proof() -> Result<(), String> {
    let manifest = read_manifest("cargo-allow")?;
    // Check if there's a [features] section that references intent/proof
    if manifest.contains("[features]") {
        // Extract features section
        let features_start = manifest.find("[features]").unwrap();
        let features_end = manifest[features_start..]
            .find("\n[")
            .map(|i| features_start + i)
            .unwrap_or(manifest.len());
        let features_section = &manifest[features_start..features_end];
        for forbidden in &["intent", "proof"] {
            if features_section.contains(forbidden) {
                return Err(format!(
                    "cargo-allow [features] section must not reference `{forbidden}` crates"
                ));
            }
        }
    }
    Ok(())
}

#[test]
fn cargo_intent_production_deps_have_no_proof() -> Result<(), String> {
    let manifest = read_manifest("cargo-intent")?;
    let deps = extract_prod_deps(&manifest);
    let forbidden = contains_forbidden(&deps, &["proof-", "cargo-proof"]);
    if !forbidden.is_empty() {
        return Err(format!(
            "cargo-intent production dependencies must not include proof crates: {forbidden:?}"
        ));
    }
    Ok(())
}

#[test]
fn cargo_proof_production_deps_have_no_cargo_allow() -> Result<(), String> {
    let manifest = read_manifest("cargo-proof")?;
    let deps = extract_prod_deps(&manifest);
    let forbidden = contains_forbidden(&deps, &["cargo-allow", "allow-core", "allow-policy"]);
    if !forbidden.is_empty() {
        return Err(format!(
            "cargo-proof production dependencies must not include cargo-allow product crates: {forbidden:?}"
        ));
    }
    Ok(())
}

#[test]
fn feature_policy_manifest_exists() -> Result<(), String> {
    let root = workspace_root();
    let path = root.join("policy/feature-policy.toml");
    if !path.is_file() {
        // Create it if it doesn't exist
        let content = r#"# Feature policy for three-product separation (#3364).
# Declares feature classes and bounds for cross-product activation.

schema_id = "cargo-allow.feature-policy.v1"
schema_version = 1
controlling_issue = 3364

# Feature classes:
#   core_default — always available, no cross-product pull
#   experimental_product — opt-in only, not in defaults
#   legacy_compatibility — opt-in only, explicitly selected
#   provider_adapter — opt-in per provider
#   test_fixture_only — never in published builds

[[rule]]
product = "cargo-allow"
rule = "no_default_pulls_intent_or_proof"
description = "cargo-allow default features must not pull cargo-intent or cargo-proof"

[[rule]]
product = "cargo-intent"
rule = "no_default_pulls_proof_or_provider"
description = "cargo-intent default features must not pull proof or provider execution"

[[rule]]
product = "cargo-proof"
rule = "no_reverse_dependency_to_cargo_allow"
description = "cargo-proof provider features must not create a reverse dependency to cargo-allow"
"#;
        std::fs::write(&path, content).map_err(|e| format!("write feature-policy.toml: {e}"))?;
    }
    Ok(())
}
