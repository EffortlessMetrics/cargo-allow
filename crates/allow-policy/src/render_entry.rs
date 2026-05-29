use allow_core::{AllowEntry, LastSeen, Selector};

use crate::render_toml::{escape_toml, render_array};

pub(crate) fn render_allow_entry(out: &mut String, entry: &AllowEntry) {
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
