use allow_core::{AllowConfig, Requirements, WorkspaceConfig};

use crate::render_toml::{escape_toml, render_array, render_bool_field};

pub(crate) fn render_policy_header(out: &mut String, cfg: &AllowConfig) {
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

pub(crate) fn render_lanes(
    out: &mut String,
    lanes: &std::collections::BTreeMap<String, allow_core::LaneConfig>,
) {
    if lanes.is_empty() {
        return;
    }
    for (name, lane) in lanes {
        out.push_str(&format!(
            "[lanes.{name}]\nmode = \"{}\"\n\n",
            lane.mode.as_str()
        ));
    }
}

pub(crate) fn render_workspace(out: &mut String, workspace: &WorkspaceConfig) {
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
        "generated = [{}]\n",
        render_array(&workspace.generated)
    ));
    for rule in &workspace.file_families {
        out.push_str(&format!(
            "\n[[workspace.file_family]]\n\
id = \"{}\"\n\
family = \"{}\"\n\
glob = \"{}\"\n\
reason = \"{}\"\n",
            escape_toml(&rule.id),
            escape_toml(&rule.family),
            escape_toml(&rule.glob),
            escape_toml(&rule.reason),
        ));
    }
    out.push('\n');
}

pub(crate) fn render_requirements(out: &mut String, requirements: &Requirements) {
    out.push_str("[requirements]\n");
    render_bool_field(out, "owner_required", requirements.owner_required);
    render_bool_field(out, "reason_required", requirements.reason_required);
    render_bool_field(
        out,
        "classification_required",
        requirements.classification_required,
    );
    render_bool_field(out, "evidence_required", requirements.evidence_required);
    render_bool_field(
        out,
        "expires_or_review_after_required",
        requirements.expires_or_review_after_required,
    );
    render_bool_field(
        out,
        "allow_bare_allow_attributes",
        requirements.allow_bare_allow_attributes,
    );
    render_bool_field(
        out,
        "lint_policy_id_required",
        requirements.lint_policy_id_required,
    );
    render_bool_field(out, "stale_entries_fail", requirements.stale_entries_fail);
    out.push('\n');
    out.push_str("[requirements.unsafe]\n");
    render_bool_field(
        out,
        "evidence_required",
        requirements.unsafe_evidence_required,
    );
    render_bool_field(
        out,
        "verified_evidence_required",
        requirements.unsafe_verified_evidence_required,
    );
    render_bool_field(
        out,
        "safety_comment_required",
        requirements.unsafe_safety_comment_required,
    );
}
