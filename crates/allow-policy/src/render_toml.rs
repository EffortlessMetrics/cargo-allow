pub(crate) fn escape_toml(input: &str) -> String {
    let mut out = String::new();
    for ch in input.chars() {
        match ch {
            '\u{08}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{0C}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            ch if ch.is_control() => out.push_str(&format!("\\u{:04X}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out
}

pub(crate) fn render_array(values: &[String]) -> String {
    values
        .iter()
        .map(|v| format!("\"{}\"", escape_toml(v)))
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn render_string_field(out: &mut String, name: &str, value: &str) {
    out.push_str(name);
    out.push_str(" = \"");
    out.push_str(&escape_toml(value));
    out.push_str("\"\n");
}

pub(crate) fn render_optional_string_field(out: &mut String, name: &str, value: Option<&str>) {
    if let Some(value) = value {
        render_string_field(out, name, value);
    }
}

pub(crate) fn render_bool_field(out: &mut String, name: &str, value: bool) {
    out.push_str(name);
    out.push_str(" = ");
    out.push_str(if value { "true" } else { "false" });
    out.push('\n');
}
