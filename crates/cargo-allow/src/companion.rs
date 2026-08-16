use allow_core::{AllowConfig, CargoAllowResult, Finding, FindingKind};
use std::path::Path;

pub(crate) fn canonical_companion_findings(
    root: &Path,
    cfg: &AllowConfig,
    inventory_files: &[std::path::PathBuf],
) -> CargoAllowResult<Vec<Finding>> {
    let mut findings = Vec::new();
    if has_allow_family(cfg, FindingKind::GeneratedCode, "generated_code") {
        findings.extend(allow_files::generated_findings_from_gitattributes(root)?);
    }
    if has_allow_family(cfg, FindingKind::PolicyException, "executable_file") {
        findings.extend(allow_files::executable_findings_from_git(root)?);
    }
    if has_policy_family(cfg, &["github_workflow", "workflow_external_action"]) {
        findings.extend(allow_files::workflow_findings_from_files(root)?);
    }
    if has_allow_family(cfg, FindingKind::PolicyException, "dependency_surface") {
        findings.extend(allow_files::dependency_surface_findings_from_paths(
            inventory_files,
            cfg,
        ));
    }
    if has_allow_family(cfg, FindingKind::PolicyException, "process_spawn") {
        findings.extend(allow_files::process_findings_from_config(cfg));
    }
    if has_allow_family(cfg, FindingKind::PolicyException, "network_destination") {
        findings.extend(allow_files::network_findings_from_config(cfg));
    }
    Ok(findings)
}

/// Companion findings that are derived from policy and the exact candidate
/// path set, rather than from worktree-only metadata or file contents.
///
/// The staged source path deliberately rejects the remaining worktree-derived
/// families in `world.rs`; keeping this adapter small prevents an exact staged
/// report from accidentally consuming ambient `.gitattributes`, executable
/// bits, or workflow bytes.
pub(crate) fn staged_companion_findings(
    cfg: &AllowConfig,
    inventory_files: &[std::path::PathBuf],
) -> CargoAllowResult<Vec<Finding>> {
    let mut findings = Vec::new();
    if has_staged_family(cfg, "dependency_surface") {
        findings.extend(allow_files::dependency_surface_findings_from_paths(
            inventory_files,
            cfg,
        ));
    }
    if has_staged_family(cfg, "process_spawn") {
        findings.extend(allow_files::process_findings_from_config(cfg));
    }
    if has_staged_family(cfg, "network_destination") {
        findings.extend(allow_files::network_findings_from_config(cfg));
    }
    Ok(findings)
}

pub(crate) const STAGED_SUPPORTED_COMPANION_FAMILIES: &[&str] =
    &["dependency_surface", "process_spawn", "network_destination"];

pub(crate) fn staged_companion_family_supported(family: &str) -> bool {
    STAGED_SUPPORTED_COMPANION_FAMILIES.contains(&family)
}

fn has_staged_family(cfg: &AllowConfig, family: &str) -> bool {
    staged_companion_family_supported(family)
        && has_allow_family(cfg, FindingKind::PolicyException, family)
}

fn has_policy_family(cfg: &AllowConfig, families: &[&str]) -> bool {
    cfg.allow.iter().any(|entry| {
        entry.kind == FindingKind::PolicyException
            && entry
                .family
                .as_deref()
                .is_some_and(|family| families.contains(&family))
    })
}

fn has_allow_family(cfg: &AllowConfig, kind: FindingKind, family: &str) -> bool {
    cfg.allow
        .iter()
        .any(|entry| entry.kind == kind && entry.family.as_deref() == Some(family))
}

pub(crate) fn extend_unique_findings(findings: &mut Vec<Finding>, additional: Vec<Finding>) {
    if findings.is_empty() {
        findings.extend(additional);
        return;
    }
    // Build a HashSet of identity keys for the accumulated set to avoid
    // O(n×m) per-comparison scans (#2677). First finding with a given
    // identity wins (same rule as the previous linear search).
    use std::collections::HashSet;
    let mut seen: HashSet<String> = findings
        .iter()
        .map(allow_core::finding_identity_key)
        .collect();
    for finding in additional {
        let key = allow_core::finding_identity_key(&finding);
        if seen.insert(key) {
            findings.push(finding);
        }
    }
}

#[cfg(test)]
#[path = "companion_helpers_tests.rs"]
mod tests;
