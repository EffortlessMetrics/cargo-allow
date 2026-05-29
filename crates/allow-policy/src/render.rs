use allow_core::{AllowConfig, Requirements, WorkspaceConfig};

use crate::render_entry::render_allow_entry;
use crate::render_toml::{escape_toml, render_array};

pub fn render_policy(cfg: &AllowConfig) -> String {
    let mut out = String::new();
    render_policy_header(&mut out, cfg);
    render_workspace(&mut out, &cfg.workspace);
    render_requirements(&mut out, &cfg.requirements);
    for entry in &cfg.allow {
        render_allow_entry(&mut out, entry);
    }
    out
}

fn render_policy_header(out: &mut String, cfg: &AllowConfig) {
    out.push_str(&format!(
        "schema_version = \"{}\"\npolicy = \"{}\"\n",
        cfg.schema_version, cfg.policy
    ));
    if let Some(owner) = &cfg.owner {
        out.push_str(&format!("owner = \"{}\"\n", escape_toml(owner)));
    }
    if let Some(status) = &cfg.status {
        out.push_str(&format!("status = \"{}\"\n", escape_toml(status)));
    }
    out.push('\n');
}

fn render_workspace(out: &mut String, workspace: &WorkspaceConfig) {
    out.push_str("[workspace]\n");
    out.push_str(&format!(
        "root = \"{}\"\ninventory = \"{}\"\ndefault_mode = \"{}\"\n",
        escape_toml(&workspace.root),
        escape_toml(&workspace.inventory),
        escape_toml(&workspace.default_mode)
    ));
    out.push_str(&format!(
        "ignored = [{}]\n",
        render_array(&workspace.ignored)
    ));
    out.push_str(&format!(
        "generated = [{}]\n\n",
        render_array(&workspace.generated)
    ));
}

fn render_requirements(out: &mut String, requirements: &Requirements) {
    out.push_str("[requirements]\n");
    out.push_str(&format!("owner_required = {}\nreason_required = {}\nclassification_required = {}\nevidence_required = {}\nexpires_or_review_after_required = {}\nallow_bare_allow_attributes = {}\nlint_policy_id_required = {}\nstale_entries_fail = {}\n\n", requirements.owner_required, requirements.reason_required, requirements.classification_required, requirements.evidence_required, requirements.expires_or_review_after_required, requirements.allow_bare_allow_attributes, requirements.lint_policy_id_required, requirements.stale_entries_fail));
    out.push_str("[requirements.unsafe]\n");
    out.push_str(&format!(
        "evidence_required = {}\nsafety_comment_required = {}\n",
        requirements.unsafe_evidence_required, requirements.unsafe_safety_comment_required
    ));
}
