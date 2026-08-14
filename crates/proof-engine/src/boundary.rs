//! Boundary surface and upstream topology markers (#2589-A).

use proof_protocol::PROOF_PLAN_SCHEMA_ID;

pub struct BoundarySurface;

impl BoundarySurface {
    pub const MODULE_ID: &'static str = "proof-engine::boundary";
}

pub const ALLOWED_UPSTREAM_CRATES: &[&str] = &[
    "effortless-repo-protocol",
    "effortless-rust-source-index",
    "intent-protocol",
    "proof-protocol",
];

pub const FORBIDDEN_DEPENDENCY_EDGES: &[&str] = &[
    "proof-engine -> intent-model",
    "proof-engine -> intent-engine",
    "cargo-allow product -> proof-engine",
];

/// Converged obligation-input dependency path (#2936 / #3317).
///
/// intent-protocol is the sole obligation input authority for proof-engine.
/// The proof-owned duplicate obligation model was deleted in #3314; this
/// edge records the final dependency path so the duplicate authority cannot
/// silently return.
pub const REQUIRED_DEPENDENCY_EDGES: &[&str] = &["proof-engine -> intent-protocol"];

pub fn upstream_surface_markers() -> [&'static str; 1] {
    [PROOF_PLAN_SCHEMA_ID]
}

#[cfg(test)]
mod tests {
    use super::ALLOWED_UPSTREAM_CRATES;
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    #[test]
    fn allowed_upstream_crates_match_manifest_internal_dependencies() -> Result<(), String> {
        let manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let manifest_text = std::fs::read_to_string(&manifest_path)
            .map_err(|err| format!("read {}: {err}", manifest_path.display()))?;
        let manifest: toml::Table = toml::from_str(&manifest_text)
            .map_err(|err| format!("parse {}: {err}", manifest_path.display()))?;
        let dependencies = manifest
            .get("dependencies")
            .and_then(toml::Value::as_table)
            .ok_or_else(|| "proof-orchestrator manifest is missing [dependencies]".to_string())?;
        let actual = dependencies
            .keys()
            .filter(|name| {
                name.starts_with("effortless-")
                    || name.starts_with("proof-")
                    || name.starts_with("intent-")
            })
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let expected = ALLOWED_UPSTREAM_CRATES
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if actual != expected {
            return Err(format!(
                "boundary upstream receipt differs from manifest: expected {expected:?}, actual {actual:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn required_obligation_input_edge_is_declared_in_manifest() -> Result<(), String> {
        let manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let manifest_text = std::fs::read_to_string(&manifest_path)
            .map_err(|err| format!("read {}: {err}", manifest_path.display()))?;
        let manifest: toml::Table = toml::from_str(&manifest_text)
            .map_err(|err| format!("parse {}: {err}", manifest_path.display()))?;
        let dependencies = manifest
            .get("dependencies")
            .and_then(toml::Value::as_table)
            .ok_or_else(|| "proof-orchestrator manifest is missing [dependencies]".to_string())?;
        for edge in super::REQUIRED_DEPENDENCY_EDGES {
            let Some((_from, to)) = edge.split_once(" -> ") else {
                return Err(format!("invalid required edge {edge}"));
            };
            if !dependencies.contains_key(to) {
                return Err(format!(
                    "required obligation-input edge {edge} is not declared in the manifest"
                ));
            }
        }
        Ok(())
    }
}
