use allow_core::Selector;

use crate::render_toml::render_optional_string_field;

pub(crate) fn render_selector(out: &mut String, selector: &Selector) {
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
    // line_hint is intentionally NOT rendered: the parser always discards it
    // (toml_selector.rs sets line_hint: None for backward compat). Rendering
    // it would create a confusing lossy round-trip where the field appears in
    // the file but is silently dropped on next load (#2512).
    render_optional_string_field(out, "glob", selector.glob.as_deref());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_selector_writes_present_fields_but_not_line_hint() {
        let mut out = String::from("prefix\n");
        let selector = Selector {
            ast_kind: Some("method_call".to_string()),
            container: Some("load_policy".to_string()),
            callee: Some("unwrap".to_string()),
            macro_name: Some("panic".to_string()),
            lint: Some("clippy::unwrap_used".to_string()),
            symbol: Some("load_policy::unwrap".to_string()),
            receiver_fingerprint: Some("recv:result".to_string()),
            target_fingerprint: Some("target:unwrap".to_string()),
            normalized_snippet_hash: Some("abc123".to_string()),
            line_hint: Some(42), // should NOT be rendered
            glob: Some("src/lib.rs".to_string()),
        };

        render_selector(&mut out, &selector);

        assert_eq!(
            out,
            "prefix\n\n[allow.selector]\n\
ast_kind = \"method_call\"\n\
container = \"load_policy\"\n\
callee = \"unwrap\"\n\
macro_name = \"panic\"\n\
lint = \"clippy::unwrap_used\"\n\
symbol = \"load_policy::unwrap\"\n\
receiver_fingerprint = \"recv:result\"\n\
target_fingerprint = \"target:unwrap\"\n\
normalized_snippet_hash = \"abc123\"\n\
glob = \"src/lib.rs\"\n"
        );
    }

    #[test]
    fn render_selector_omits_absent_optional_fields() {
        let mut out = String::new();
        let selector = Selector {
            ast_kind: Some("macro_call".to_string()),
            macro_name: Some("panic".to_string()),
            ..Selector::default()
        };

        render_selector(&mut out, &selector);

        assert_eq!(
            out,
            "\n[allow.selector]\nast_kind = \"macro_call\"\nmacro_name = \"panic\"\n"
        );
    }
}
