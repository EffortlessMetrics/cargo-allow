use allow_core::AllowEntry;

use crate::render_last_seen::render_last_seen;
use crate::render_selector::render_selector;
use crate::render_toml::{
    escape_toml, render_array, render_optional_string_field, render_string_field,
};

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
