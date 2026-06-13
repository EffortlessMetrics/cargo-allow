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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_toml_observes_each_escape_branch() {
        assert_eq!(escape_toml("plain"), "plain");
        assert_eq!(escape_toml("\u{08}"), "\\b");
        assert_eq!(escape_toml("\t"), "\\t");
        assert_eq!(escape_toml("\n"), "\\n");
        assert_eq!(escape_toml("\u{0C}"), "\\f");
        assert_eq!(escape_toml("\r"), "\\r");
        assert_eq!(escape_toml("\""), "\\\"");
        assert_eq!(escape_toml("\\"), "\\\\");
        assert_eq!(escape_toml("\u{01}"), "\\u0001");
        assert_eq!(
            escape_toml("a\u{08}\t\n\u{0C}\r\"\\\u{01}z"),
            "a\\b\\t\\n\\f\\r\\\"\\\\\\u0001z"
        );
    }

    #[test]
    fn render_array_quotes_and_escapes_each_value() {
        let values = vec![
            "plain".to_string(),
            "needs \"quote\"".to_string(),
            "line\nbreak".to_string(),
            "slash\\value".to_string(),
        ];

        let rendered = render_array(&values);

        assert_eq!(
            rendered,
            "\"plain\", \"needs \\\"quote\\\"\", \"line\\nbreak\", \"slash\\\\value\""
        );
    }

    #[test]
    fn render_string_and_optional_fields_append_expected_lines() {
        let mut out = String::new();

        render_string_field(&mut out, "reason", "line\n\"quoted\"");
        render_optional_string_field(&mut out, "owner", Some("repo\\team"));
        render_optional_string_field(&mut out, "empty", None);

        assert_eq!(
            out,
            "reason = \"line\\n\\\"quoted\\\"\"\nowner = \"repo\\\\team\"\n"
        );
    }

    #[test]
    fn render_bool_field_appends_true_and_false_lines() {
        let mut out = String::new();

        render_bool_field(&mut out, "owner_required", true);
        render_bool_field(&mut out, "evidence_required", false);

        assert_eq!(out, "owner_required = true\nevidence_required = false\n");
    }
}
