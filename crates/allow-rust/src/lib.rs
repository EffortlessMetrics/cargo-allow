use allow_core::{
    CargoAllowError, CargoAllowResult, Finding, FindingKind, Span, StructuralIdentity,
    normalize_snippet, stable_hash_hex,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use tree_sitter::Node;

mod package;
mod syntax_kinds;
mod syntax_tree;
mod text;

use package::source_package_contexts;
use syntax_kinds::{
    LintAttributeKind, PanicMacroInvocation, PanicMacroKind, PanicMethodCall, PanicMethodKind,
    RustLineScope, RustSyntaxFacts, UnsafeSyntaxConstruct, UnsafeSyntaxKind,
};
use syntax_tree::{impl_container_name, node_text};
use text::{
    attribute_column, column, detect_attr, extract_first_lint, index_symbol, lint_policy_reference,
    receiver_before_method_column,
};

pub use package::{
    SourcePackageContext, apply_source_package_context, source_package_contexts_from_sources,
};
pub use syntax_tree::{RustSyntaxContainer, RustSyntaxTree, parse_rust_syntax};

pub fn scan_rust_files(
    root: impl AsRef<Path>,
    files: &[PathBuf],
) -> CargoAllowResult<Vec<Finding>> {
    let root = root.as_ref();
    let mut out = Vec::new();
    let packages = source_package_contexts(root, files)?;
    for rel in files {
        if rel.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let path = root.join(rel);
        let text = fs::read_to_string(&path)
            .map_err(|e| CargoAllowError::new(format!("failed to read {}: {e}", path.display())))?;
        let mut findings = scan_rust_source(rel, &text);
        apply_source_package_context(rel, &packages, &mut findings);
        out.extend(findings);
    }
    Ok(out)
}

pub fn scan_rust_source(path: impl AsRef<Path>, source: &str) -> Vec<Finding> {
    let path = path.as_ref().to_path_buf();
    let mut findings = Vec::new();
    let syntax = syntax_facts(source);
    let safety_comments = safety_comment_lines(source);

    for (line_idx, raw_line) in source.lines().enumerate() {
        let line_no = (line_idx + 1) as u32;
        let line = raw_line;
        let scope = syntax.scopes.get(&line_no).cloned().unwrap_or_default();

        scan_line(
            &path,
            line,
            line_no,
            &scope.container,
            &scope.module_path,
            SyntaxLineFacts {
                lint_attributes: syntax
                    .lint_attributes
                    .get(&line_no)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]),
                panic_macros: syntax
                    .panic_macros
                    .get(&line_no)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]),
                panic_methods: syntax
                    .panic_methods
                    .get(&line_no)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]),
                index_column: syntax.index_columns.get(&line_no).copied(),
                unsafe_constructs: syntax
                    .unsafe_constructs
                    .get(&line_no)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]),
                unsafe_attribute: syntax.unsafe_attribute_lines.contains(&line_no),
                safety_comment_nearby: has_nearby_safety_comment(&safety_comments, line_no),
            },
            &mut findings,
        );
    }
    findings
}

fn scan_line(
    path: &Path,
    line: &str,
    line_no: u32,
    container: &Option<String>,
    module_stack: &[String],
    syntax: SyntaxLineFacts<'_>,
    findings: &mut Vec<Finding>,
) {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with("//") {
        return;
    }

    for attr_kind in syntax.lint_attributes {
        let Some(attr_text) = detect_attr(trimmed, attr_kind.name()) else {
            continue;
        };
        let lint = extract_first_lint(attr_text);
        let policy_id = lint_policy_reference(trimmed);
        push_finding(
            FindingSite {
                path,
                line,
                line_no,
                column: attribute_column(line),
                container,
                module_stack,
            },
            FindingKind::LintException,
            match attr_kind {
                LintAttributeKind::Allow => "allow_attribute",
                LintAttributeKind::Expect => "expect_attribute",
            },
            "attribute",
            |id| {
                id.lint = lint;
                id.symbol = Some(trimmed.to_string());
                id.target_fingerprint = policy_id.map(|policy_id| format!("policy:{policy_id}"));
            },
            findings,
        );
    }

    for unsafe_construct in syntax.unsafe_constructs {
        push_finding(
            FindingSite {
                path,
                line,
                line_no,
                column: unsafe_construct.column,
                container,
                module_stack,
            },
            FindingKind::Unsafe,
            unsafe_construct.kind.family(),
            unsafe_construct.kind.ast_kind(),
            |id| {
                if syntax.safety_comment_nearby {
                    id.target_fingerprint = Some("safety-comment:present".to_string());
                }
            },
            findings,
        );
    }
    if syntax.unsafe_attribute {
        push_finding(
            FindingSite {
                path,
                line,
                line_no,
                column: column(line, "unsafe"),
                container,
                module_stack,
            },
            FindingKind::Unsafe,
            "unsafe_attr",
            "unsafe_attr",
            |id| {
                if syntax.safety_comment_nearby {
                    id.target_fingerprint = Some("safety-comment:present".to_string());
                }
            },
            findings,
        );
    }

    for method_call in syntax.panic_methods {
        let receiver = receiver_before_method_column(line, method_call.column);
        push_finding(
            FindingSite {
                path,
                line,
                line_no,
                column: method_call.column,
                container,
                module_stack,
            },
            FindingKind::Panic,
            method_call.kind.family(),
            "method_call",
            |id| {
                id.callee = Some(method_call.kind.family().to_string());
                id.receiver_fingerprint = Some(receiver);
            },
            findings,
        );
    }

    for macro_invocation in syntax.panic_macros {
        push_finding(
            FindingSite {
                path,
                line,
                line_no,
                column: macro_invocation.column,
                container,
                module_stack,
            },
            FindingKind::Panic,
            macro_invocation.kind.family(),
            "macro_call",
            |id| {
                id.macro_name = Some(macro_invocation.kind.macro_name().to_string());
            },
            findings,
        );
    }

    if let Some(index_column) = syntax.index_column {
        let family = if line.contains("&") && line.contains("[") {
            "string_slice"
        } else {
            "indexing"
        };
        push_finding(
            FindingSite {
                path,
                line,
                line_no,
                column: index_column,
                container,
                module_stack,
            },
            FindingKind::Panic,
            family,
            "index_expr",
            |id| {
                id.symbol = Some(index_symbol(line));
                id.target_fingerprint = line.split('[').next().map(|s| {
                    normalize_snippet(s)
                        .chars()
                        .rev()
                        .take(40)
                        .collect::<String>()
                        .chars()
                        .rev()
                        .collect()
                });
            },
            findings,
        );
    }
}

fn syntax_facts(source: &str) -> RustSyntaxFacts {
    let Ok(tree) = parse_rust_syntax(source) else {
        return RustSyntaxFacts::default();
    };
    let mut facts = RustSyntaxFacts::default();
    collect_syntax_facts(tree.tree.root_node(), source, &mut facts);
    let mut module_path = Vec::new();
    let mut impl_path = Vec::new();
    collect_line_scopes(
        tree.tree.root_node(),
        source,
        &mut module_path,
        &mut impl_path,
        &mut facts.scopes,
    );
    facts
}

fn collect_syntax_facts(node: Node<'_>, source: &str, facts: &mut RustSyntaxFacts) {
    if node.kind() == "index_expression" {
        let start = node.start_position();
        let bracket_offset = node_text(source, node)
            .and_then(|text| text.find('['))
            .unwrap_or(0);
        let line = start.row as u32 + 1;
        let column = start.column as u32 + bracket_offset as u32 + 1;
        facts
            .index_columns
            .entry(line)
            .and_modify(|existing| *existing = (*existing).min(column))
            .or_insert(column);
    }
    if let Some((line, construct)) = unsafe_syntax_construct(node) {
        record_unsafe_construct(facts, line, construct);
    }
    if let Some((line, invocation)) = panic_macro_invocation(node, source) {
        facts.panic_macros.entry(line).or_default().push(invocation);
    }
    if let Some((line, method_call)) = panic_method_call(node, source) {
        facts
            .panic_methods
            .entry(line)
            .or_default()
            .push(method_call);
    }
    if matches!(node.kind(), "attribute_item" | "inner_attribute_item") {
        if let Some(text) = node_text(source, node) {
            let line = node.start_position().row as u32 + 1;
            if let Some(kind) = lint_attribute_kind(text) {
                facts.lint_attributes.entry(line).or_default().push(kind);
            }
            if unsafe_attribute_text(text) {
                facts.unsafe_attribute_lines.insert(line);
            }
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_syntax_facts(child, source, facts);
    }
}

fn collect_line_scopes(
    node: Node<'_>,
    source: &str,
    module_path: &mut Vec<String>,
    impl_path: &mut Vec<String>,
    scopes: &mut BTreeMap<u32, RustLineScope>,
) {
    if node.kind() == "mod_item" {
        if let Some(name) = node
            .child_by_field_name("name")
            .and_then(|name| node_text(source, name))
        {
            module_path.push(name.to_string());
            record_module_scope(node, module_path, scopes);
            visit_child_scopes(node, source, module_path, impl_path, scopes);
            module_path.pop();
            return;
        }
    }

    if node.kind() == "impl_item" {
        if let Some(name) = impl_container_name(node, source) {
            impl_path.push(name);
            visit_child_scopes(node, source, module_path, impl_path, scopes);
            impl_path.pop();
            return;
        }
    }

    if node.kind() == "function_item" {
        if let Some(name) = node
            .child_by_field_name("name")
            .and_then(|name| node_text(source, name))
        {
            let container = if let Some(impl_name) = impl_path.last() {
                format!("{impl_name}::{name}")
            } else {
                name.to_string()
            };
            record_container_scope(node, &container, module_path, scopes);
        }
    }

    visit_child_scopes(node, source, module_path, impl_path, scopes);
}

fn visit_child_scopes(
    node: Node<'_>,
    source: &str,
    module_path: &mut Vec<String>,
    impl_path: &mut Vec<String>,
    scopes: &mut BTreeMap<u32, RustLineScope>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_line_scopes(child, source, module_path, impl_path, scopes);
    }
}

fn record_module_scope(
    node: Node<'_>,
    module_path: &[String],
    scopes: &mut BTreeMap<u32, RustLineScope>,
) {
    let start_line = node.start_position().row as u32 + 1;
    let end_line = node.end_position().row as u32 + 1;
    let span_len = end_line.saturating_sub(start_line) + 1;
    for line in start_line..=end_line {
        let candidate = RustLineScope {
            container: None,
            module_path: module_path.to_vec(),
            span_len,
        };
        merge_scope(scopes, line, candidate);
    }
}

fn record_container_scope(
    node: Node<'_>,
    name: &str,
    module_path: &[String],
    scopes: &mut BTreeMap<u32, RustLineScope>,
) {
    let start_line = node.start_position().row as u32 + 1;
    let end_line = node.end_position().row as u32 + 1;
    let span_len = end_line.saturating_sub(start_line) + 1;
    for line in start_line..=end_line {
        let candidate = RustLineScope {
            container: Some(name.to_string()),
            module_path: module_path.to_vec(),
            span_len,
        };
        merge_scope(scopes, line, candidate);
    }
}

fn merge_scope(scopes: &mut BTreeMap<u32, RustLineScope>, line: u32, candidate: RustLineScope) {
    scopes
        .entry(line)
        .and_modify(|existing| {
            let candidate_has_container = candidate.container.is_some();
            let existing_has_container = existing.container.is_some();
            if (candidate_has_container && !existing_has_container)
                || (candidate_has_container == existing_has_container
                    && candidate.span_len < existing.span_len)
            {
                *existing = candidate.clone();
            }
        })
        .or_insert(candidate);
}

fn panic_macro_invocation(node: Node<'_>, source: &str) -> Option<(u32, PanicMacroInvocation)> {
    if node.kind() != "macro_invocation" {
        return None;
    }
    let macro_node = node.child_by_field_name("macro")?;
    let macro_text = node_text(source, macro_node)?;
    let base_name = macro_text.rsplit("::").next().unwrap_or(macro_text);
    let kind = PanicMacroKind::from_name(base_name)?;
    let start = macro_node.start_position();
    let base_offset = macro_text.len().saturating_sub(base_name.len()) as u32;
    Some((
        start.row as u32 + 1,
        PanicMacroInvocation {
            kind,
            column: start.column as u32 + base_offset + 1,
        },
    ))
}

fn panic_method_call(node: Node<'_>, source: &str) -> Option<(u32, PanicMethodCall)> {
    if node.kind() != "call_expression" {
        return None;
    }
    let function = node.child_by_field_name("function")?;
    if function.kind() != "field_expression" {
        return None;
    }
    let field = function.child_by_field_name("field")?;
    let method_name = node_text(source, field)?;
    let kind = PanicMethodKind::from_name(method_name)?;
    let start = field.start_position();
    Some((
        start.row as u32 + 1,
        PanicMethodCall {
            kind,
            column: start.column as u32 + 1,
        },
    ))
}

fn unsafe_syntax_construct(node: Node<'_>) -> Option<(u32, UnsafeSyntaxConstruct)> {
    match node.kind() {
        "function_item" | "function_signature_item" => {
            unsafe_modifier_construct(node, "fn", UnsafeSyntaxKind::Fn)
        }
        "impl_item" => unsafe_modifier_construct(node, "impl", UnsafeSyntaxKind::Impl),
        "trait_item" => unsafe_modifier_construct(node, "trait", UnsafeSyntaxKind::Trait),
        "foreign_mod_item" => {
            unsafe_modifier_construct(node, "extern_modifier", UnsafeSyntaxKind::ExternBlock)
        }
        "unsafe_block" => {
            unsafe_child_point(node).map(|point| located_construct(point, UnsafeSyntaxKind::Block))
        }
        _ => None,
    }
}

fn unsafe_modifier_construct(
    node: Node<'_>,
    keyword_kind: &str,
    kind: UnsafeSyntaxKind,
) -> Option<(u32, UnsafeSyntaxConstruct)> {
    let mut unsafe_point = None;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == keyword_kind {
            return unsafe_point.map(|point| located_construct(point, kind));
        }
        if unsafe_point.is_none() {
            unsafe_point = unsafe_child_point(child);
        }
    }
    None
}

fn unsafe_child_point(node: Node<'_>) -> Option<tree_sitter::Point> {
    if node.kind() == "unsafe" {
        return Some(node.start_position());
    }
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find_map(|child| (child.kind() == "unsafe").then(|| child.start_position()))
}

fn located_construct(
    point: tree_sitter::Point,
    kind: UnsafeSyntaxKind,
) -> (u32, UnsafeSyntaxConstruct) {
    (
        point.row as u32 + 1,
        UnsafeSyntaxConstruct {
            kind,
            column: point.column as u32 + 1,
        },
    )
}

fn record_unsafe_construct(
    facts: &mut RustSyntaxFacts,
    line: u32,
    construct: UnsafeSyntaxConstruct,
) {
    let constructs = facts.unsafe_constructs.entry(line).or_default();
    if !constructs
        .iter()
        .any(|existing| existing.kind == construct.kind && existing.column == construct.column)
    {
        constructs.push(construct);
        constructs.sort_by_key(|construct| (construct.column, construct.kind.priority()));
    }
}

fn lint_attribute_kind(text: &str) -> Option<LintAttributeKind> {
    let trimmed = text.trim_start();
    if detect_attr(trimmed, "allow").is_some() {
        Some(LintAttributeKind::Allow)
    } else if detect_attr(trimmed, "expect").is_some() {
        Some(LintAttributeKind::Expect)
    } else {
        None
    }
}

fn unsafe_attribute_text(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with("#[unsafe(") || trimmed.starts_with("#![unsafe(")
}

fn safety_comment_lines(source: &str) -> BTreeSet<u32> {
    source
        .lines()
        .enumerate()
        .filter_map(|(line_idx, line)| is_safety_comment(line).then_some((line_idx + 1) as u32))
        .collect()
}

fn is_safety_comment(line: &str) -> bool {
    let trimmed = line.trim_start();
    if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*') {
        return trimmed.contains("SAFETY:");
    }
    line.split_once("//")
        .is_some_and(|(_, comment)| comment.contains("SAFETY:"))
}

fn has_nearby_safety_comment(safety_comments: &BTreeSet<u32>, line_no: u32) -> bool {
    let first = line_no.saturating_sub(3).max(1);
    (first..=line_no).any(|line| safety_comments.contains(&line))
}

struct SyntaxLineFacts<'a> {
    lint_attributes: &'a [LintAttributeKind],
    panic_macros: &'a [PanicMacroInvocation],
    panic_methods: &'a [PanicMethodCall],
    index_column: Option<u32>,
    unsafe_constructs: &'a [UnsafeSyntaxConstruct],
    unsafe_attribute: bool,
    safety_comment_nearby: bool,
}

struct FindingSite<'a> {
    path: &'a Path,
    line: &'a str,
    line_no: u32,
    column: u32,
    container: &'a Option<String>,
    module_stack: &'a [String],
}

fn push_finding<F>(
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
    identity.container = site.container.clone();
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
    });
}

#[cfg(test)]
mod tests;
