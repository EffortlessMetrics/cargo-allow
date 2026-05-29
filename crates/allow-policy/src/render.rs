use allow_core::{AllowConfig, AllowEntry, LastSeen, Requirements, Selector, WorkspaceConfig};

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

fn render_allow_entry(out: &mut String, entry: &AllowEntry) {
    out.push_str("\n[[allow]]\n");
    out.push_str(&format!(
        "id = \"{}\"\nkind = \"{}\"\n",
        escape_toml(&entry.id),
        entry.kind.as_str()
    ));
    if let Some(family) = &entry.family {
        out.push_str(&format!("family = \"{}\"\n", escape_toml(family)));
    }
    if let Some(path) = &entry.path {
        out.push_str(&format!(
            "path = \"{}\"\n",
            escape_toml(&path.to_string_lossy())
        ));
    }
    if let Some(glob) = &entry.glob {
        out.push_str(&format!("glob = \"{}\"\n", escape_toml(glob)));
    }
    out.push_str(&format!(
        "owner = \"{}\"\nclassification = \"{}\"\nreason = \"{}\"\n",
        escape_toml(&entry.owner),
        escape_toml(&entry.classification),
        escape_toml(&entry.reason)
    ));
    if !entry.evidence.is_empty() {
        out.push_str(&format!("evidence = [{}]\n", render_array(&entry.evidence)));
    }
    if !entry.links.is_empty() {
        out.push_str(&format!("links = [{}]\n", render_array(&entry.links)));
    }
    if let Some(limit) = entry.occurrence_limit {
        out.push_str(&format!("occurrence_limit = {limit}\n"));
    }
    if let Some(created) = &entry.lifecycle.created {
        out.push_str(&format!("created = \"{}\"\n", escape_toml(created)));
    }
    if let Some(review_after) = &entry.lifecycle.review_after {
        out.push_str(&format!(
            "review_after = \"{}\"\n",
            escape_toml(review_after)
        ));
    }
    if let Some(expires) = &entry.lifecycle.expires {
        out.push_str(&format!("expires = \"{}\"\n", escape_toml(expires)));
    }
    render_selector(out, &entry.selector);
    if let Some(last_seen) = &entry.last_seen {
        render_last_seen(out, last_seen);
    }
}

fn render_selector(out: &mut String, selector: &Selector) {
    out.push_str("\n[allow.selector]\n");
    if let Some(v) = &selector.ast_kind {
        out.push_str(&format!("ast_kind = \"{}\"\n", escape_toml(v)));
    }
    if let Some(v) = &selector.container {
        out.push_str(&format!("container = \"{}\"\n", escape_toml(v)));
    }
    if let Some(v) = &selector.callee {
        out.push_str(&format!("callee = \"{}\"\n", escape_toml(v)));
    }
    if let Some(v) = &selector.macro_name {
        out.push_str(&format!("macro_name = \"{}\"\n", escape_toml(v)));
    }
    if let Some(v) = &selector.lint {
        out.push_str(&format!("lint = \"{}\"\n", escape_toml(v)));
    }
    if let Some(v) = &selector.symbol {
        out.push_str(&format!("symbol = \"{}\"\n", escape_toml(v)));
    }
    if let Some(v) = &selector.receiver_fingerprint {
        out.push_str(&format!("receiver_fingerprint = \"{}\"\n", escape_toml(v)));
    }
    if let Some(v) = &selector.target_fingerprint {
        out.push_str(&format!("target_fingerprint = \"{}\"\n", escape_toml(v)));
    }
    if let Some(v) = &selector.normalized_snippet_hash {
        out.push_str(&format!(
            "normalized_snippet_hash = \"{}\"\n",
            escape_toml(v)
        ));
    }
    if let Some(v) = selector.line_hint {
        out.push_str(&format!("line_hint = {}\n", v));
    }
    if let Some(v) = &selector.glob {
        out.push_str(&format!("glob = \"{}\"\n", escape_toml(v)));
    }
}

fn render_last_seen(out: &mut String, last_seen: &LastSeen) {
    out.push_str("\n[allow.last_seen]\n");
    out.push_str(&format!(
        "line = {}\ncolumn = {}\n",
        last_seen.line, last_seen.column
    ));
}

fn escape_toml(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

fn render_array(values: &[String]) -> String {
    values
        .iter()
        .map(|v| format!("\"{}\"", escape_toml(v)))
        .collect::<Vec<_>>()
        .join(", ")
}
