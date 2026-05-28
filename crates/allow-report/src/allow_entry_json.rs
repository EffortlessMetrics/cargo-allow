use crate::json::{json_string_array, option_json, option_u32_json};
use allow_core::{AllowEntry, LastSeen, Lifecycle, Selector, json_escape, normalize_path};

pub fn render_allow_entry_json(entry: &AllowEntry, indent: &str) -> String {
    let path = entry.path.as_ref().map(normalize_path);
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "{indent}  \"id\": \"{}\",\n",
        json_escape(&entry.id)
    ));
    out.push_str(&format!("{indent}  \"kind\": \"{}\",\n", entry.kind));
    out.push_str(&format!(
        "{indent}  \"family\": {},\n",
        option_json(entry.family.as_deref())
    ));
    out.push_str(&format!(
        "{indent}  \"scope\": \"{}\",\n",
        json_escape(&entry.path_or_glob())
    ));
    out.push_str(&format!(
        "{indent}  \"path\": {},\n",
        option_json(path.as_deref())
    ));
    out.push_str(&format!(
        "{indent}  \"glob\": {},\n",
        option_json(entry.glob.as_deref())
    ));
    out.push_str(&format!(
        "{indent}  \"owner\": \"{}\",\n",
        json_escape(&entry.owner)
    ));
    out.push_str(&format!(
        "{indent}  \"classification\": \"{}\",\n",
        json_escape(&entry.classification)
    ));
    out.push_str(&format!(
        "{indent}  \"reason\": \"{}\",\n",
        json_escape(&entry.reason)
    ));
    out.push_str(&format!(
        "{indent}  \"evidence\": {},\n",
        json_string_array(&entry.evidence)
    ));
    out.push_str(&format!(
        "{indent}  \"links\": {},\n",
        json_string_array(&entry.links)
    ));
    out.push_str(&format!(
        "{indent}  \"occurrence_limit\": {},\n",
        option_u32_json(entry.occurrence_limit)
    ));
    out.push_str(&format!(
        "{indent}  \"lifecycle\": {},\n",
        lifecycle_json(&entry.lifecycle, indent)
    ));
    out.push_str(&format!(
        "{indent}  \"selector\": {},\n",
        render_selector_json(&entry.selector, indent)
    ));
    out.push_str(&format!(
        "{indent}  \"last_seen\": {}\n",
        render_last_seen_json(entry.last_seen.as_ref(), indent)
    ));
    out.push_str(&format!("{indent}}}"));
    out
}

fn lifecycle_json(lifecycle: &Lifecycle, indent: &str) -> String {
    format!(
        "{{\n{indent}    \"created\": {},\n{indent}    \"review_after\": {},\n{indent}    \"expires\": {}\n{indent}  }}",
        option_json(lifecycle.created.as_deref()),
        option_json(lifecycle.review_after.as_deref()),
        option_json(lifecycle.expires.as_deref())
    )
}

pub fn render_selector_json(selector: &Selector, indent: &str) -> String {
    format!(
        "{{\n{indent}    \"ast_kind\": {},\n{indent}    \"container\": {},\n{indent}    \"callee\": {},\n{indent}    \"macro_name\": {},\n{indent}    \"lint\": {},\n{indent}    \"symbol\": {},\n{indent}    \"receiver_fingerprint\": {},\n{indent}    \"target_fingerprint\": {},\n{indent}    \"normalized_snippet_hash\": {},\n{indent}    \"line_hint\": {},\n{indent}    \"glob\": {}\n{indent}  }}",
        option_json(selector.ast_kind.as_deref()),
        option_json(selector.container.as_deref()),
        option_json(selector.callee.as_deref()),
        option_json(selector.macro_name.as_deref()),
        option_json(selector.lint.as_deref()),
        option_json(selector.symbol.as_deref()),
        option_json(selector.receiver_fingerprint.as_deref()),
        option_json(selector.target_fingerprint.as_deref()),
        option_json(selector.normalized_snippet_hash.as_deref()),
        option_u32_json(selector.line_hint),
        option_json(selector.glob.as_deref())
    )
}

pub fn render_last_seen_json(last_seen: Option<&LastSeen>, indent: &str) -> String {
    last_seen
        .map(|last_seen| {
            format!(
                "{{\n{indent}    \"line\": {},\n{indent}    \"column\": {}\n{indent}  }}",
                last_seen.line, last_seen.column
            )
        })
        .unwrap_or_else(|| "null".to_string())
}
