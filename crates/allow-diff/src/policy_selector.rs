use allow_core::Selector;

pub(crate) fn selector_identity_changed(base: &Selector, head: &Selector) -> bool {
    !selector_identity_changed_fields(base, head).is_empty()
}

pub(crate) fn selector_identity_changed_fields(
    base: &Selector,
    head: &Selector,
) -> Vec<&'static str> {
    [
        ("ast_kind", &base.ast_kind, &head.ast_kind),
        ("container", &base.container, &head.container),
        ("callee", &base.callee, &head.callee),
        ("macro_name", &base.macro_name, &head.macro_name),
        ("lint", &base.lint, &head.lint),
        ("symbol", &base.symbol, &head.symbol),
        (
            "receiver_fingerprint",
            &base.receiver_fingerprint,
            &head.receiver_fingerprint,
        ),
        (
            "target_fingerprint",
            &base.target_fingerprint,
            &head.target_fingerprint,
        ),
        (
            "normalized_snippet_hash",
            &base.normalized_snippet_hash,
            &head.normalized_snippet_hash,
        ),
    ]
    .into_iter()
    .filter_map(|(label, base, head)| text_field_changed(base, head).then_some(label))
    .collect()
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
