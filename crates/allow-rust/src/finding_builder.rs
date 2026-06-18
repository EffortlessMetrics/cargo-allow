use allow_core::{
    Finding, FindingKind, Span, StructuralIdentity, normalize_snippet, stable_hash_hex,
};
use std::path::Path;

pub(crate) struct FindingSite<'a> {
    pub(crate) path: &'a Path,
    pub(crate) line: &'a str,
    pub(crate) line_no: u32,
    pub(crate) column: u32,
    pub(crate) container: &'a Option<String>,
    pub(crate) module_stack: &'a [String],
}

pub(crate) fn push_finding<F>(
    site: FindingSite<'_>,
    kind: FindingKind,
    family: &str,
    ast_kind: &str,
    enrich: F,
    findings: &mut Vec<Finding>,
) where
    F: FnOnce(&mut StructuralIdentity),
{
    let mut identity = StructuralIdentity::new("rust", ast_kind);
    identity.container = site
        .container
        .as_ref()
        .map(|container| qualify_container_for_module(container, site.module_stack));
    if !site.module_stack.is_empty() {
        identity.module = Some(site.module_stack.join("::"));
    }
    identity.normalized_snippet_hash = Some(stable_hash_hex(&normalize_snippet(site.line)));
    identity.line_hint = Some(site.line_no);
    identity.column_hint = Some(site.column);
    enrich(&mut identity);
    findings.push(Finding {
        kind,
        family: Some(family.to_string()),
        path: site.path.to_path_buf(),
        span: Some(Span {
            line: site.line_no,
            column: site.column,
        }),
        identity,
        message: format!("{kind} {family} syntax found"),
        ledger: None,
    });
}

fn qualify_container_for_module(container: &str, module_stack: &[String]) -> String {
    if module_stack.is_empty() || container.contains("::") {
        return container.to_string();
    }
    format!("{}::{}", module_stack.join("::"), container)
}
