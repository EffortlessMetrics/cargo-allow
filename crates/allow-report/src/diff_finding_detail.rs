use allow_core::StructuralIdentity;

pub(crate) fn structural_identity_summary(identity: &StructuralIdentity) -> String {
    let mut fields = Vec::new();
    push_required(&mut fields, "ast_kind", &identity.ast_kind);
    push_optional(&mut fields, "module", identity.module.as_deref());
    push_optional(&mut fields, "container", identity.container.as_deref());
    push_optional(&mut fields, "callee", identity.callee.as_deref());
    push_optional(&mut fields, "macro_name", identity.macro_name.as_deref());
    push_optional(&mut fields, "lint", identity.lint.as_deref());
    push_optional(&mut fields, "symbol", identity.symbol.as_deref());
    push_optional(
        &mut fields,
        "receiver_fingerprint",
        identity.receiver_fingerprint.as_deref(),
    );
    push_optional(
        &mut fields,
        "target_fingerprint",
        identity.target_fingerprint.as_deref(),
    );
    push_optional(
        &mut fields,
        "normalized_snippet_hash",
        identity.normalized_snippet_hash.as_deref(),
    );
    fields.join(",")
}

fn push_required(fields: &mut Vec<String>, name: &str, value: &str) {
    let trimmed = value.trim();
    if !trimmed.is_empty() {
        fields.push(format!("{name}={trimmed}"));
    }
}

fn push_optional(fields: &mut Vec<String>, name: &str, value: Option<&str>) {
    if let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) {
        fields.push(format!("{name}={value}"));
    }
}
