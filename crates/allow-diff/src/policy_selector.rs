use allow_core::Selector;

pub(crate) fn selector_identity_changed(base: &Selector, head: &Selector) -> bool {
    text_field_changed(&base.ast_kind, &head.ast_kind)
        || text_field_changed(&base.container, &head.container)
        || text_field_changed(&base.callee, &head.callee)
        || text_field_changed(&base.macro_name, &head.macro_name)
        || text_field_changed(&base.lint, &head.lint)
        || text_field_changed(&base.symbol, &head.symbol)
        || text_field_changed(&base.receiver_fingerprint, &head.receiver_fingerprint)
        || text_field_changed(&base.target_fingerprint, &head.target_fingerprint)
        || text_field_changed(&base.normalized_snippet_hash, &head.normalized_snippet_hash)
}

fn text_field_changed(base: &Option<String>, head: &Option<String>) -> bool {
    normalized_text(base) != normalized_text(head)
}

fn normalized_text(value: &Option<String>) -> Option<&str> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}
