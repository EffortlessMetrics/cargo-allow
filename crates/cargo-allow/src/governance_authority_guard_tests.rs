//! Governance-authority allow-list guard for allow-policy (#3330 / #2942 step 8).
//!
//! allow-policy currently owns governance authority (product/crate/package/move/
//! shim/parity/topology/spec-system). #2942 requires that new V2 governance
//! canonical types/validators NOT be re-introduced into allow-policy without an
//! explicit reviewed exception. This guard enumerates allow-policy's `pub mod`
//! governance modules and compares against an allow-list seeded from the current
//! authority. The check starts green and only fails on NEW additions.

use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read_allow_policy_lib() -> Result<String, String> {
    let root = workspace_root();
    let path = root.join("crates/allow-policy/src/lib.rs");
    std::fs::read_to_string(&path).map_err(|e| format!("read allow-policy/src/lib.rs: {e}"))
}

/// Extract `pub mod <name>;` declarations from the allow-policy lib.rs.
/// These are the publicly-exported module roots — the surface that governance
/// authority leaks through.
fn public_modules(source: &str) -> Vec<String> {
    let mut modules = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim_start();
        // Match `pub mod <name>;` (possibly with attributes on prior lines,
        // but we only care about the pub mod line itself).
        if let Some(rest) = trimmed
            .strip_prefix("pub mod ")
            .or_else(|| trimmed.strip_prefix("pub(crate) mod "))
        {
            let name = rest.trim_end_matches(';').trim();
            // Skip inline module bodies (pub mod foo { ... }) — we only care
            // about file-backed module declarations (pub mod foo;).
            if !name.contains('{') && !name.is_empty() {
                modules.push(name.to_string());
            }
        }
    }
    modules
}

/// The governance-authority allow-list: the set of `pub mod` modules in
/// allow-policy that currently own governance authority. New governance
/// modules must NOT be added here without explicit review — they belong in
/// intent-model (the V2 governance owner per #2942).
///
/// If a module is intentionally being moved OUT of allow-policy (deletion),
/// remove it from this list. If a module is being added that is NOT governance
/// authority (e.g. a pure rendering helper), add it to NON_GOVERNANCE_MODULES.
const GOVERNANCE_AUTHORITY_MODULES: &[&str] = &[
    "extraction_parity",
    "extraction_shims",
    "product_crates",
    "product_move",
    "product_packages",
    "spec_system",
];

/// Modules that are `pub mod` but do NOT carry governance authority. These are
/// allowed to exist without being governance canonical types/validators.
const NON_GOVERNANCE_MODULES: &[&str] = &["federation", "import_roots"];

#[test]
fn governance_authority_modules_match_allow_list() -> Result<(), String> {
    let source = read_allow_policy_lib()?;
    let pub_mods = public_modules(&source);

    // Every module in the allow-list must still be present (detects
    // accidental deletion of authority — which is fine but should be
    // intentional).
    for expected in GOVERNANCE_AUTHORITY_MODULES {
        if !pub_mods.iter().any(|m| m == expected) {
            // Deletion is allowed (authority moving to intent-model is the
            // goal of #2942), so we only warn, not fail. But we record it.
            eprintln!(
                "note: governance module `{expected}` is no longer pub-mod-exported from allow-policy (may have moved to intent-model)"
            );
        }
    }

    // Every pub mod that is NOT in the allow-list and NOT in the
    // non-governance list is a POTENTIAL new governance authority leak.
    let allowed: std::collections::HashSet<&str> = GOVERNANCE_AUTHORITY_MODULES
        .iter()
        .chain(NON_GOVERNANCE_MODULES.iter())
        .copied()
        .collect();
    let unexpected: Vec<&str> = pub_mods
        .iter()
        .map(|s| s.as_str())
        .filter(|m| !allowed.contains(*m))
        .collect();
    if !unexpected.is_empty() {
        return Err(format!(
            "allow-policy has new pub mod governance modules not in the allow-list: {unexpected:?}\n\
             New V2 governance canonical types/validators must live in intent-model (#2942), \
             not allow-policy. If this module is NOT governance authority, add it to \
             NON_GOVERNANCE_MODULES in governance_authority_guard_tests.rs."
        ));
    }
    Ok(())
}

#[test]
fn guard_detects_new_governance_module() -> Result<(), String> {
    // Seeded fixture: a synthetic lib.rs with a new governance module must
    // be flagged.
    let seeded = r#"
pub mod extraction_parity;
pub mod extraction_shims;
pub mod product_crates;
pub mod product_move;
pub mod product_packages;
pub mod spec_system;
pub mod federation;
pub mod import_roots;
pub mod new_v2_governance_authority;
"#;
    let pub_mods = public_modules(seeded);
    let allowed: std::collections::HashSet<&str> = GOVERNANCE_AUTHORITY_MODULES
        .iter()
        .chain(NON_GOVERNANCE_MODULES.iter())
        .copied()
        .collect();
    let unexpected: Vec<&str> = pub_mods
        .iter()
        .map(|s| s.as_str())
        .filter(|m| !allowed.contains(*m))
        .collect();
    if unexpected.is_empty() {
        return Err(
            "seeded new governance module `new_v2_governance_authority` was not detected".into(),
        );
    }
    if !unexpected.contains(&"new_v2_governance_authority") {
        return Err(format!(
            "seeded module was detected but not named correctly: {unexpected:?}"
        ));
    }
    Ok(())
}

#[test]
fn guard_accepts_non_governance_module() -> Result<(), String> {
    // Seeded fixture: federation and import_roots are pub mod but NOT
    // governance authority — they must pass the guard.
    let seeded = r#"
pub mod extraction_parity;
pub mod federation;
pub mod import_roots;
"#;
    let pub_mods = public_modules(seeded);
    let allowed: std::collections::HashSet<&str> = GOVERNANCE_AUTHORITY_MODULES
        .iter()
        .chain(NON_GOVERNANCE_MODULES.iter())
        .copied()
        .collect();
    let unexpected: Vec<&str> = pub_mods
        .iter()
        .map(|s| s.as_str())
        .filter(|m| !allowed.contains(*m))
        .collect();
    // extraction_parity IS governance, so it's in the allow-list.
    // federation and import_roots are non-governance.
    // No unexpected modules should remain.
    if !unexpected.is_empty() {
        return Err(format!(
            "non-governance modules were incorrectly flagged: {unexpected:?}"
        ));
    }
    Ok(())
}

#[test]
fn spec_system_remains_doc_hidden() -> Result<(), String> {
    // spec_system is governance authority that is currently tolerated but
    // must remain #[doc(hidden)] so it is not part of the advertised public
    // API surface. If it loses #[doc(hidden)], that widens the governance
    // surface.
    let source = read_allow_policy_lib()?;
    let lines: Vec<&str> = source.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("pub mod spec_system") {
            // Check the preceding line for #[doc(hidden)].
            if i == 0 {
                return Err("pub mod spec_system has no preceding attribute".into());
            }
            let Some(prev) = lines.get(i.wrapping_sub(1)).map(|s| s.trim_start()) else {
                return Err("pub mod spec_system preceding line was unreadable".into());
            };
            if !prev.contains("#[doc(hidden)]") {
                return Err(format!(
                    "pub mod spec_system must remain #[doc(hidden)]; found preceding line: `{prev}`"
                ));
            }
            return Ok(());
        }
    }
    // If spec_system is gone entirely, that's fine (authority moved out).
    eprintln!("note: spec_system is no longer exported from allow-policy");
    Ok(())
}
