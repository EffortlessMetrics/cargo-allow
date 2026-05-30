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
    if let Some(v) = selector.line_hint {
        out.push_str(&format!("line_hint = {}\n", v));
    }
    render_optional_string_field(out, "glob", selector.glob.as_deref());
}
