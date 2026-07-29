use crate::json::{json_string_array, option_json, option_u32_json};
use allow_core::{AllowEntry, LastSeen, Lifecycle, Selector, json_escape, normalize_path};

pub fn render_allow_entry_json(entry: &AllowEntry, indent: &str) -> String {
    let path = entry.path.as_ref().map(normalize_path);
    let field_indent = format!("{indent}  ");
    let mut fields = vec![
        format!("{field_indent}\"id\": \"{}\"", json_escape(&entry.id)),
        format!("{field_indent}\"kind\": \"{}\"", entry.kind),
    ];
    push_optional_string_field(
        &mut fields,
        &field_indent,
        "family",
        entry.family.as_deref(),
    );
    fields.extend([
        format!(
            "{field_indent}\"scope\": \"{}\"",
            json_escape(&entry.path_or_glob())
        ),
        format!("{field_indent}\"path\": {}", option_json(path.as_deref())),
        format!(
            "{field_indent}\"glob\": {}",
            option_json(entry.glob.as_deref())
        ),
        format!("{field_indent}\"owner\": \"{}\"", json_escape(&entry.owner)),
        format!(
            "{field_indent}\"classification\": \"{}\"",
            json_escape(&entry.classification)
        ),
        format!(
            "{field_indent}\"reason\": \"{}\"",
            json_escape(&entry.reason)
        ),
        format!(
            "{field_indent}\"evidence\": {}",
            json_string_array(&entry.evidence)
        ),
        format!(
            "{field_indent}\"links\": {}",
            json_string_array(&entry.links)
        ),
        format!(
            "{field_indent}\"occurrence_limit\": {}",
            option_u32_json(entry.occurrence_limit)
        ),
        format!(
            "{field_indent}\"lifecycle\": {}",
            lifecycle_json(&entry.lifecycle, indent)
        ),
        format!(
            "{field_indent}\"selector\": {}",
            render_selector_json(&entry.selector, indent)
        ),
        format!(
            "{field_indent}\"last_seen\": {}",
            render_last_seen_json(entry.last_seen.as_ref(), indent)
        ),
    ]);
    format!("{{\n{}\n{indent}}}", fields.join(",\n"))
}

fn lifecycle_json(lifecycle: &Lifecycle, indent: &str) -> String {
    let field_indent = format!("{indent}    ");
    let mut fields = Vec::new();
    push_optional_string_field(
        &mut fields,
        &field_indent,
        "created",
        lifecycle.created.as_deref(),
    );
    push_optional_string_field(
        &mut fields,
        &field_indent,
        "review_after",
        lifecycle.review_after.as_deref(),
    );
    push_optional_string_field(
        &mut fields,
        &field_indent,
        "expires",
        lifecycle.expires.as_deref(),
    );
    if fields.is_empty() {
        return "{}".to_string();
    }
    format!("{{\n{}\n{indent}  }}", fields.join(",\n"))
}

pub(crate) fn push_optional_string_field(
    fields: &mut Vec<String>,
    field_indent: &str,
    name: &str,
    value: Option<&str>,
) {
    if let Some(value) = value {
        fields.push(format!(
            "{field_indent}\"{name}\": \"{}\"",
            json_escape(value)
        ));
    }
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
