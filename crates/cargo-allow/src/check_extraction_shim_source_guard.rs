//! Source-scanning shim findings (#3376b / #2607 PR2).
//!
//! The registry-level findings (#3574) cross-reference the shim registry
//! against the move ledger. This module adds the four source-level
//! findings for ACTIVE shims whose old identity names a workspace crate:
//!
//! - `extraction_shim_unregistered`: the old-identity source path is
//!   missing, or a facade-style shim no longer forwards to the new owner
//! - `extraction_shim_semantic_logic`: a facade-style shim file declares
//!   public functions (identity-only forwarding may not carry logic)
//! - `extraction_shim_reverse_dependency`: the host crate's manifest
//!   declares a dependency the governance law forbids
//! - `extraction_shim_hidden_feature`: the host crate's manifest defines a
//!   feature edge naming the old module (optional resurrection)
//!
//! Shims whose old identity names an external crate (e.g. ripr) have no
//! local source to scan and are out of scope here.

use std::collections::BTreeSet;
use std::path::Path;

use allow_core::{CargoAllowError, CargoAllowResult};

/// The parsed shim rows the source scan consumes (kept structural so the
/// guard depends only on the registry text, not the allow-policy types).
pub(crate) struct ShimSourceSubject {
    pub shim_id: String,
    pub old_identity: String,
    pub new_identity: String,
    pub host_crate: String,
    pub module_path: String,
    /// Family shims (`old::path*`) cover whole module families that
    /// delegate rather than re-export; facade checks do not apply.
    pub is_family: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ShimSourceFindingKind {
    Unregistered,
    SemanticLogic,
    ReverseDependency,
    HiddenFeature,
}

impl ShimSourceFindingKind {
    #[cfg(test)]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Unregistered => "extraction_shim_unregistered",
            Self::SemanticLogic => "extraction_shim_semantic_logic",
            Self::ReverseDependency => "extraction_shim_reverse_dependency",
            Self::HiddenFeature => "extraction_shim_hidden_feature",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ShimSourceFinding {
    pub kind: ShimSourceFindingKind,
    pub shim_id: String,
    pub message: String,
}

/// Parse the active shim rows from the registry text into scan subjects.
///
/// Old identities have the form `<crate>::<module>` (with an optional
/// trailing `*` for module families). Host crate and module path are the
/// two components; identities without a `::` separator or naming a
/// non-workspace crate yield no subject (nothing to scan locally).
pub(crate) fn shim_source_subjects(
    registry_text: &str,
    workspace_crates: &BTreeSet<String>,
) -> CargoAllowResult<Vec<ShimSourceSubject>> {
    #[derive(serde::Deserialize)]
    struct RegistryToml {
        #[serde(default)]
        shim: Vec<ShimToml>,
    }
    #[derive(serde::Deserialize)]
    struct ShimToml {
        id: String,
        old_identity: String,
        new_identity: String,
        status: String,
    }
    let registry: RegistryToml = toml::from_str(registry_text)
        .map_err(|err| CargoAllowError::new(format!("parse shim registry: {err}")))?;
    let mut subjects = Vec::new();
    for shim in registry.shim {
        if shim.status != "active" {
            continue;
        }
        let is_family = shim.old_identity.trim_end().ends_with('*');
        let trimmed = shim.old_identity.trim_end_matches('*');
        let Some((crate_name, module_path)) = trimmed.split_once("::") else {
            continue;
        };
        if !workspace_crates.contains(crate_name) {
            continue;
        }
        subjects.push(ShimSourceSubject {
            shim_id: shim.id,
            old_identity: trimmed.to_string(),
            new_identity: shim.new_identity,
            host_crate: crate_name.to_string(),
            module_path: module_path.to_string(),
            is_family,
        });
    }
    Ok(subjects)
}

/// Scan one subject's facade source for the four findings.
///
/// `crate_source_dir` is the host crate's `src/` directory; `manifest_text`
/// is the host crate's Cargo.toml. `forbidden_targets` maps this host crate
/// to the crate names its product law forbids.
pub(crate) fn scan_shim_source(
    subject: &ShimSourceSubject,
    crate_source_dir: &Path,
    manifest_text: &str,
    forbidden_targets: &BTreeSet<String>,
) -> CargoAllowResult<Vec<ShimSourceFinding>> {
    let mut findings = Vec::new();
    let facade_path = crate_source_dir.join(format!("{}.rs", subject.module_path));

    if !facade_path.is_file() {
        // Module-style shim (a directory) or a missing path: only a missing
        // path with no directory counterpart is unregistered.
        let module_dir = crate_source_dir.join(&subject.module_path);
        if !module_dir.is_dir() {
            findings.push(ShimSourceFinding {
                kind: ShimSourceFindingKind::Unregistered,
                shim_id: subject.shim_id.clone(),
                message: format!(
                    "shim `{}` old identity `{}` has no source path at {} (module dir absent too); intended owner is `{}`",
                    subject.shim_id,
                    subject.old_identity,
                    facade_path.display(),
                    subject.new_identity
                ),
            });
            return Ok(findings);
        }
        // Module-style shims own real modules; the semantic-logic and
        // forwarding checks apply to facade files only.
    } else if !subject.is_family {
        let text = std::fs::read_to_string(&facade_path).map_err(|err| {
            CargoAllowError::new(format!("read {}: {err}", facade_path.display()))
        })?;
        findings.extend(scan_facade_text(subject, &text));
    }

    findings.extend(scan_manifest(subject, manifest_text, forbidden_targets));
    Ok(findings)
}

/// Facade checks: identity-only forwarding to the new owner, no semantic
/// logic (public functions beyond the re-export).
fn scan_facade_text(subject: &ShimSourceSubject, text: &str) -> Vec<ShimSourceFinding> {
    let mut findings = Vec::new();
    // A facade forwards when it re-exports anything (pub use / pub(crate)
    // use). Repo facades re-export through snapshot module copies, so the
    // new owner's crate name need not appear literally; the presence of a
    // public re-export is the identity-forwarding contract.
    let forwards = text.lines().any(|line| {
        let trimmed = line.trim_start();
        trimmed.starts_with("pub use ") || trimmed.starts_with("pub(crate) use ")
    });
    if !forwards {
        findings.push(ShimSourceFinding {
            kind: ShimSourceFindingKind::Unregistered,
            shim_id: subject.shim_id.clone(),
            message: format!(
                "shim `{}` facade no longer re-exports; the compat surface is unregistered",
                subject.shim_id
            ),
        });
    }

    let declares_fn = text.lines().any(|line| {
        let trimmed = line.trim_start();
        trimmed.starts_with("pub fn ") || trimmed.starts_with("pub const fn ")
    });
    if declares_fn {
        findings.push(ShimSourceFinding {
            kind: ShimSourceFindingKind::SemanticLogic,
            shim_id: subject.shim_id.clone(),
            message: format!(
                "shim `{}` facade declares a public function; compatibility facades are identity-only forwarding",
                subject.shim_id
            ),
        });
    }
    findings
}

/// Manifest checks: forbidden dependencies and hidden feature edges naming
/// the old module.
fn scan_manifest(
    subject: &ShimSourceSubject,
    manifest_text: &str,
    forbidden_targets: &BTreeSet<String>,
) -> Vec<ShimSourceFinding> {
    let mut findings = Vec::new();
    let manifest: toml::Table = match toml::from_str(manifest_text) {
        Ok(table) => table,
        Err(_) => return findings,
    };
    // Dev-dependencies are the sanctioned compatibility surface (the
    // architecture tests assert cargo-allow's intent dev deps stay
    // visible); the shim reverse-dependency finding covers production
    // edges only.
    for section in ["dependencies", "build-dependencies"] {
        let Some(deps) = manifest.get(section).and_then(|value| value.as_table()) else {
            continue;
        };
        for offender in forbidden_targets.iter() {
            if deps.contains_key(offender) {
                findings.push(ShimSourceFinding {
                    kind: ShimSourceFindingKind::ReverseDependency,
                    shim_id: subject.shim_id.clone(),
                    message: format!(
                        "shim `{}` host crate `{}` declares a forbidden dependency `{offender}` in {section}",
                        subject.shim_id, subject.host_crate
                    ),
                });
            }
        }
    }
    if let Some(features) = manifest.get("features").and_then(|value| value.as_table()) {
        let module_ident = subject.module_path.replace('-', "_");
        for (feature, edges) in features {
            let Some(edges) = edges.as_array() else {
                continue;
            };
            let names_old = edges.iter().any(|edge| {
                edge.as_str()
                    .is_some_and(|text| text.replace('-', "_").contains(&module_ident))
            });
            if names_old {
                findings.push(ShimSourceFinding {
                    kind: ShimSourceFindingKind::HiddenFeature,
                    shim_id: subject.shim_id.clone(),
                    message: format!(
                        "shim `{}` host crate `{}` feature `{feature}` references the old module `{}`; optional resurrection of the old evaluator",
                        subject.shim_id, subject.host_crate, subject.module_path
                    ),
                });
            }
        }
    }
    findings
}

/// Full pass over the live tree: registry text + projection identities +
/// dependency law feed the scan. Returns all findings across subjects.
pub(crate) fn scan_shim_sources_at(root: &Path) -> CargoAllowResult<Vec<ShimSourceFinding>> {
    // Outside the source checkout (install journeys, extracted candidates)
    // there is no policy tree to scan; nothing to enforce. The registry
    // gate in allow-policy behaves the same when the registry is absent.
    let Ok(registry_text) = std::fs::read_to_string(root.join("policy/extraction-shims.toml"))
    else {
        return Ok(Vec::new());
    };
    let projection = crate::check::governance_projection::load_governance_projection_at(root)?;
    let workspace_crates: BTreeSet<String> = projection
        .crate_identities
        .iter()
        .map(|identity| identity.logical_id.clone())
        .collect();
    let path_for: std::collections::BTreeMap<&str, &str> = projection
        .crate_identities
        .iter()
        .map(|identity| {
            (
                identity.logical_id.as_str(),
                identity.workspace_path.as_str(),
            )
        })
        .collect();

    // Forbidden targets per host crate, from the dependency law (the
    // projection already carries the forbid map).
    let forbid = crate::check::governance_projection::forbidden_product_targets(&projection);

    let subjects = shim_source_subjects(&registry_text, &workspace_crates)?;
    let mut findings = Vec::new();
    for subject in &subjects {
        let Some(workspace_path) = path_for.get(subject.host_crate.as_str()) else {
            continue;
        };
        let crate_source_dir = root.join(workspace_path).join("src");
        let manifest_path = root.join(workspace_path).join("Cargo.toml");
        let Ok(manifest_text) = std::fs::read_to_string(&manifest_path) else {
            continue;
        };
        let empty = BTreeSet::new();
        let forbidden = forbid.get(subject.host_crate.as_str()).unwrap_or(&empty);
        findings.extend(scan_shim_source(
            subject,
            &crate_source_dir,
            &manifest_text,
            forbidden,
        )?);
    }
    Ok(findings)
}

/// Check-pipeline gate: active shim source findings fail no-new/strict.
pub(crate) fn shim_sources_fail_check(
    root: &Path,
    mode: allow_match::CheckMode,
) -> CargoAllowResult<bool> {
    if mode != allow_match::CheckMode::NoNew && mode != allow_match::CheckMode::Strict {
        return Ok(false);
    }
    Ok(!scan_shim_sources_at(root)?.is_empty())
}

#[cfg(test)]
#[path = "check_extraction_shim_source_guard_tests.rs"]
mod shim_source_tests;
