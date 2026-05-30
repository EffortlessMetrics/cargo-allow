use allow_core::{AllowEntry, LastSeen, Selector};

use crate::render_toml::{escape_toml, render_array};

pub(crate) fn render_allow_entry(out: &mut String, entry: &AllowEntry) {
    out.push_str("\n[[allow]]\n");
    out.push_str(&format!(
        "id = \"{}\"\nkind = \"{}\"\n",
        escape_toml(&entry.id),
        entry.kind.as_str()
    ));
    render_optional_string_field(out, "family", entry.family.as_deref());
    if let Some(path) = &entry.path {
        render_string_field(out, "path", path.to_string_lossy().as_ref());
    }
    render_optional_string_field(out, "glob", entry.glob.as_deref());
    render_string_field(out, "owner", &entry.owner);
    render_string_field(out, "classification", &entry.classification);
    render_string_field(out, "reason", &entry.reason);
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
        render_string_field(out, "created", created);
    }
    if let Some(review_after) = &entry.lifecycle.review_after {
        render_string_field(out, "review_after", review_after);
    }
    if let Some(expires) = &entry.lifecycle.expires {
        render_string_field(out, "expires", expires);
    }
    render_selector(out, &entry.selector);
    if let Some(last_seen) = &entry.last_seen {
        render_last_seen(out, last_seen);
    }
}

fn render_selector(out: &mut String, selector: &Selector) {
    out.push_str("\n[allow.selector]\n");
    render_optional_string_field(out, "ast_kind", selector.ast_kind.as_deref());
    render_optional_string_field(out, "container", selector.container.as_deref());
    render_optional_string_field(out, "callee", selector.callee.as_deref());
    render_optional_string_field(out, "macro_name", selector.macro_name.as_deref());
    render_optional_string_field(out, "lint", selector.lint.as_deref());
    render_optional_string_field(out, "symbol", selector.symbol.as_deref());
    render_optional_string_field(
        out,
        "receiver_fingerprint",
        selector.receiver_fingerprint.as_deref(),
    );
    render_optional_string_field(
        out,
        "target_fingerprint",
        selector.target_fingerprint.as_deref(),
    );
    render_optional_string_field(
        out,
        "normalized_snippet_hash",
        selector.normalized_snippet_hash.as_deref(),
    );
    if let Some(v) = selector.line_hint {
        out.push_str(&format!("line_hint = {}\n", v));
    }
    render_optional_string_field(out, "glob", selector.glob.as_deref());
}

fn render_last_seen(out: &mut String, last_seen: &LastSeen) {
    out.push_str("\n[allow.last_seen]\n");
    out.push_str(&format!(
        "line = {}\ncolumn = {}\n",
        last_seen.line, last_seen.column
    ));
}

fn render_optional_string_field(out: &mut String, name: &str, value: Option<&str>) {
    if let Some(value) = value {
        render_string_field(out, name, value);
    }
}

fn render_string_field(out: &mut String, name: &str, value: &str) {
    out.push_str(name);
    out.push_str(" = \"");
    out.push_str(&escape_toml(value));
    out.push_str("\"\n");
}
