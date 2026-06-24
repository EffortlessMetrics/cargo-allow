use allow_core::{AllowConfig, CargoAllowResult};
use allow_policy::parse_policy;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::policy_change::PolicyChange;
use crate::policy_entry::{added_allow_change, entry_policy_changes, removed_allow_change};
use crate::policy_header::policy_header_changes;
use crate::policy_requirements::requirement_policy_changes;
use crate::policy_workspace::workspace_policy_changes;

pub fn policy_changes_from_git(
    root: impl AsRef<Path>,
    base: &str,
    policy_path: impl AsRef<Path>,
    head_cfg: &AllowConfig,
) -> CargoAllowResult<Vec<PolicyChange>> {
    let base_cfg =
        policy_config_at_revision(root, base, policy_path)?.unwrap_or_else(AllowConfig::empty);
    Ok(policy_changes(&base_cfg, head_cfg))
}

pub fn policy_config_at_revision(
    root: impl AsRef<Path>,
    revision: &str,
    policy_path: impl AsRef<Path>,
) -> CargoAllowResult<Option<AllowConfig>> {
    let Some(text) = crate::read_file_at_revision(root, revision, policy_path)? else {
        return Ok(None);
    };
    parse_policy(&text).map(Some)
}

pub fn policy_changes(base: &AllowConfig, head: &AllowConfig) -> Vec<PolicyChange> {
    let base_by_id = base
        .allow
        .iter()
        .map(|entry| (entry.id.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let head_ids = head
        .allow
        .iter()
        .map(|entry| entry.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut changes = policy_header_changes(base, head);
    changes.extend(requirement_policy_changes(
        &base.requirements,
        &head.requirements,
    ));
    changes.extend(workspace_policy_changes(&base.workspace, &head.workspace));
    for head_entry in &head.allow {
        let Some(base_entry) = base_by_id.get(head_entry.id.as_str()).copied() else {
            changes.push(added_allow_change(head_entry));
            continue;
        };
        changes.extend(entry_policy_changes(base_entry, head_entry));
    }
    for base_entry in &base.allow {
        if !head_ids.contains(base_entry.id.as_str()) {
            changes.push(removed_allow_change(base_entry));
        }
    }
    // Sort by (allow_id, kind) so the output is deterministic regardless of
    // input entry ordering — critical for snapshot tests and CI diff caching
    // (#1933).
    changes.sort_by(|a, b| {
        a.allow_id
            .cmp(&b.allow_id)
            .then_with(|| format!("{:?}", a.kind).cmp(&format!("{:?}", b.kind)))
    });
    changes
}
