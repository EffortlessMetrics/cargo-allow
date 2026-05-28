use allow_core::{AllowConfig, CargoAllowResult};
use allow_policy::parse_policy;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::policy_change::PolicyChange;
use crate::policy_entry::{added_allow_change, entry_policy_changes, removed_allow_change};

pub fn policy_changes_from_git(
    root: impl AsRef<Path>,
    base: &str,
    policy_path: impl AsRef<Path>,
    head_cfg: &AllowConfig,
) -> CargoAllowResult<Vec<PolicyChange>> {
    let Some(base_cfg) = policy_config_at_revision(root, base, policy_path)? else {
        return Ok(Vec::new());
    };
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
    let mut changes = Vec::new();
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
    changes
}
