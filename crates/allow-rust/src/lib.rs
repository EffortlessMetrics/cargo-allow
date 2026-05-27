use allow_core::{
    CargoAllowError, CargoAllowResult, Finding, FindingKind, Span, StructuralIdentity,
    normalize_snippet, stable_hash_hex,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use tree_sitter::{Node, Parser, Tree};

pub struct RustSyntaxTree {
    tree: Tree,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustSyntaxContainer {
    pub kind: String,
    pub name: String,
    pub module_path: Vec<String>,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LintAttributeKind {
    Allow,
    Expect,
}

impl LintAttributeKind {
    fn name(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Expect => "expect",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnsafeSyntaxKind {
    Fn,
    Impl,
    Trait,
    ExternBlock,
    Block,
}

impl UnsafeSyntaxKind {
    fn family(self) -> &'static str {
        match self {
            Self::Fn => "unsafe_fn",
            Self::Impl => "unsafe_impl",
            Self::Trait => "unsafe_trait",
            Self::ExternBlock => "unsafe_extern_block",
            Self::Block => "unsafe_block",
        }
    }

    fn ast_kind(self) -> &'static str {
        self.family()
    }

    fn priority(self) -> u8 {
        match self {
            Self::Fn => 0,
            Self::Impl => 1,
            Self::Trait => 2,
            Self::ExternBlock => 3,
            Self::Block => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct UnsafeSyntaxConstruct {
    kind: UnsafeSyntaxKind,
    column: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PanicMacroKind {
    Panic,
    Todo,
    Unimplemented,
    Unreachable,
}

impl PanicMacroKind {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "panic" => Some(Self::Panic),
            "todo" => Some(Self::Todo),
            "unimplemented" => Some(Self::Unimplemented),
            "unreachable" => Some(Self::Unreachable),
            _ => None,
        }
    }

    fn macro_name(self) -> &'static str {
        match self {
            Self::Panic => "panic",
            Self::Todo => "todo",
            Self::Unimplemented => "unimplemented",
            Self::Unreachable => "unreachable",
        }
    }

    fn family(self) -> &'static str {
        match self {
            Self::Panic => "panic_macro",
            Self::Todo => "todo",
            Self::Unimplemented => "unimplemented",
            Self::Unreachable => "unreachable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PanicMacroInvocation {
    kind: PanicMacroKind,
    column: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PanicMethodKind {
    Unwrap,
    Expect,
}

impl PanicMethodKind {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "unwrap" => Some(Self::Unwrap),
            "expect" => Some(Self::Expect),
            _ => None,
        }
    }

    fn family(self) -> &'static str {
        match self {
            Self::Unwrap => "unwrap",
            Self::Expect => "expect",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PanicMethodCall {
    kind: PanicMethodKind,
    column: u32,
}

impl RustSyntaxContainer {
    pub fn module(&self) -> Option<String> {
        if self.module_path.is_empty() {
            None
        } else {
            Some(self.module_path.join("::"))
        }
    }
}

impl RustSyntaxTree {
    pub fn root_kind(&self) -> &'static str {
        self.tree.root_node().kind()
    }

    pub fn has_error(&self) -> bool {
        self.tree.root_node().has_error()
    }

    pub fn named_node_count(&self) -> usize {
        named_node_count(self.tree.root_node())
    }

    pub fn containers(&self, source: &str) -> Vec<RustSyntaxContainer> {
        let mut containers = Vec::new();
        let mut module_path = Vec::new();
        collect_containers(
            self.tree.root_node(),
            source,
            &mut module_path,
            &mut containers,
        );
        containers
    }
}

pub fn parse_rust_syntax(source: &str) -> CargoAllowResult<RustSyntaxTree> {
    let mut parser = Parser::new();
    let language = tree_sitter_rust::LANGUAGE.into();
    parser
        .set_language(&language)
        .map_err(|e| CargoAllowError::new(format!("failed to load Rust parser: {e}")))?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| CargoAllowError::new("failed to parse Rust source"))?;
    Ok(RustSyntaxTree { tree })
}

fn named_node_count(node: Node<'_>) -> usize {
    let mut cursor = node.walk();
    let children = node
        .children(&mut cursor)
        .map(named_node_count)
        .sum::<usize>();
    if node.is_named() {
        children + 1
    } else {
        children
    }
}

fn collect_containers(
    node: Node<'_>,
    source: &str,
    module_path: &mut Vec<String>,
    containers: &mut Vec<RustSyntaxContainer>,
) {
    if node.kind() == "mod_item" {
        if let Some(name) = node
            .child_by_field_name("name")
            .and_then(|name| node_text(source, name))
        {
            module_path.push(name.to_string());
            visit_child_containers(node, source, module_path, containers);
            module_path.pop();
            return;
        }
    }

    if node.kind() == "function_item" {
        if let Some(name) = node
            .child_by_field_name("name")
            .and_then(|name| node_text(source, name))
        {
            let start = node.start_position();
            let end = node.end_position();
            containers.push(RustSyntaxContainer {
                kind: "function".to_string(),
                name: name.to_string(),
                module_path: module_path.clone(),
                start_line: start.row as u32 + 1,
                start_column: start.column as u32 + 1,
                end_line: end.row as u32 + 1,
                end_column: end.column as u32 + 1,
            });
        }
    }

    visit_child_containers(node, source, module_path, containers);
}

fn visit_child_containers(
    node: Node<'_>,
    source: &str,
    module_path: &mut Vec<String>,
    containers: &mut Vec<RustSyntaxContainer>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_containers(child, source, module_path, containers);
    }
}

fn node_text<'a>(source: &'a str, node: Node<'a>) -> Option<&'a str> {
    node.utf8_text(source.as_bytes()).ok()
}

pub fn scan_rust_files(
    root: impl AsRef<Path>,
    files: &[PathBuf],
) -> CargoAllowResult<Vec<Finding>> {
    let root = root.as_ref();
    let mut out = Vec::new();
    for rel in files {
        if rel.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let path = root.join(rel);
        let text = fs::read_to_string(&path)
            .map_err(|e| CargoAllowError::new(format!("failed to read {}: {e}", path.display())))?;
        out.extend(scan_rust_source(rel, &text));
    }
    Ok(out)
}

pub fn scan_rust_source(path: impl AsRef<Path>, source: &str) -> Vec<Finding> {
    let path = path.as_ref().to_path_buf();
    let mut findings = Vec::new();
    let mut container: Option<String> = None;
    let mut container_depth: Option<i32> = None;
    let mut brace_depth: i32 = 0;
    let mut module_stack: Vec<String> = Vec::new();
    let syntax = syntax_facts(source);

    for (line_idx, raw_line) in source.lines().enumerate() {
        let line_no = (line_idx + 1) as u32;
        let line = raw_line;
        let trimmed = line.trim();
        if let Some(name) = parse_mod_name(trimmed) {
            module_stack.push(name);
        }
        if let Some(name) = parse_fn_name(trimmed) {
            container = Some(name);
            container_depth = Some(brace_depth + count_char(line, '{') - count_char(line, '}'));
        }

        scan_line(
            &path,
            line,
            line_no,
            &container,
            &module_stack,
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
                unsafe_construct: syntax.unsafe_constructs.get(&line_no).copied(),
                unsafe_attribute: syntax.unsafe_attribute_lines.contains(&line_no),
            },
            &mut findings,
        );

        brace_depth += count_char(line, '{') - count_char(line, '}');
        if let Some(depth) = container_depth {
            if brace_depth < depth {
                container = None;
                container_depth = None;
            }
        }
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
        push_finding(
            FindingSite {
                path,
                line,
                line_no,
                column: 1,
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
            },
            findings,
        );
    }

    if let Some(unsafe_construct) = syntax.unsafe_construct {
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
            |_| {},
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
            |_| {},
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

#[derive(Default)]
struct RustSyntaxFacts {
    index_columns: BTreeMap<u32, u32>,
    lint_attributes: BTreeMap<u32, Vec<LintAttributeKind>>,
    panic_macros: BTreeMap<u32, Vec<PanicMacroInvocation>>,
    panic_methods: BTreeMap<u32, Vec<PanicMethodCall>>,
    unsafe_constructs: BTreeMap<u32, UnsafeSyntaxConstruct>,
    unsafe_attribute_lines: BTreeSet<u32>,
}

fn syntax_facts(source: &str) -> RustSyntaxFacts {
    let Ok(tree) = parse_rust_syntax(source) else {
        return RustSyntaxFacts::default();
    };
    let mut facts = RustSyntaxFacts::default();
    collect_syntax_facts(tree.tree.root_node(), source, &mut facts);
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
    facts
        .unsafe_constructs
        .entry(line)
        .and_modify(|existing| {
            if construct.kind.priority() < existing.kind.priority()
                || (construct.kind.priority() == existing.kind.priority()
                    && construct.column < existing.column)
            {
                *existing = construct;
            }
        })
        .or_insert(construct);
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

struct SyntaxLineFacts<'a> {
    lint_attributes: &'a [LintAttributeKind],
    panic_macros: &'a [PanicMacroInvocation],
    panic_methods: &'a [PanicMethodCall],
    index_column: Option<u32>,
    unsafe_construct: Option<UnsafeSyntaxConstruct>,
    unsafe_attribute: bool,
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

fn detect_attr<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let outer = format!("#[{name}(");
    let inner = format!("#![{name}(");
    if line.starts_with(&outer) {
        Some(&line[outer.len()..])
    } else if line.starts_with(&inner) {
        Some(&line[inner.len()..])
    } else {
        None
    }
}

fn extract_first_lint(text: &str) -> Option<String> {
    let until = text.split([',', ')']).next()?.trim();
    if until.is_empty() {
        None
    } else {
        Some(until.to_string())
    }
}

fn parse_fn_name(line: &str) -> Option<String> {
    let mut text = line;
    for prefix in [
        "pub(crate) ",
        "pub(super) ",
        "pub ",
        "async ",
        "const ",
        "unsafe ",
        "extern \"C\" ",
    ] {
        if let Some(rest) = text.strip_prefix(prefix) {
            text = rest;
        }
    }
    let idx = text.find("fn ")?;
    let rest = &text[idx + 3..];
    let name = rest
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .next()?;
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn parse_mod_name(line: &str) -> Option<String> {
    let text = line
        .strip_prefix("mod ")
        .or_else(|| line.strip_prefix("pub mod "))?;
    if !line.contains('{') {
        return None;
    }
    let name = text
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .next()?;
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn count_char(line: &str, ch: char) -> i32 {
    line.chars().filter(|c| *c == ch).count() as i32
}

fn column(line: &str, needle: &str) -> u32 {
    line.find(needle).map(|idx| idx as u32 + 1).unwrap_or(1)
}

fn receiver_before_method_column(line: &str, method_column: u32) -> String {
    let Some(dot_pos) = method_column.checked_sub(2).map(|pos| pos as usize) else {
        return String::new();
    };
    if dot_pos <= line.len() {
        receiver_before(line, dot_pos)
    } else {
        String::new()
    }
}

fn receiver_before(line: &str, pos: usize) -> String {
    let prefix = &line[..pos];
    let trimmed = normalize_snippet(prefix);
    trimmed
        .chars()
        .rev()
        .take(80)
        .collect::<String>()
        .chars()
        .rev()
        .collect()
}

fn index_symbol(line: &str) -> String {
    let norm = normalize_snippet(line);
    norm.chars().take(100).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_panic_family() {
        let src = r#"
        fn load() {
            let x = std::fs::read_to_string("x").unwrap();
            let y = items[0];
            panic!("bad");
        }
        "#;
        let findings = scan_rust_source("src/lib.rs", src);
        assert!(
            findings
                .iter()
                .any(|f| f.family.as_deref() == Some("unwrap"))
        );
        assert!(
            findings
                .iter()
                .any(|f| f.family.as_deref() == Some("indexing"))
        );
        assert!(
            findings
                .iter()
                .any(|f| f.family.as_deref() == Some("panic_macro"))
        );
    }

    #[test]
    fn detects_panic_methods_from_syntax() {
        let src = r#"
        fn load() {
            let x = std::fs::read_to_string("x").unwrap();
            let y = std::fs::read_to_string("y").expect("read y");
        }
        "#;
        let findings = scan_rust_source("src/lib.rs", src);

        for family in ["unwrap", "expect"] {
            assert!(
                findings.iter().any(|f| f.kind == FindingKind::Panic
                    && f.family.as_deref() == Some(family)
                    && f.identity.ast_kind == "method_call"),
                "missing {family}"
            );
        }
    }

    #[test]
    fn syntax_panic_methods_ignore_text_in_strings_and_comments() {
        let src = r#"
        fn load() {
            // value.unwrap();
            let text = "value.expect(\"string\")";
        }
        "#;
        let findings = scan_rust_source("src/lib.rs", src);

        assert!(
            !findings
                .iter()
                .any(|f| f.kind == FindingKind::Panic && f.identity.ast_kind == "method_call")
        );
    }

    #[test]
    fn syntax_panic_methods_do_not_parse_macro_token_trees() {
        let src = r#"
        fn load(value: Result<(), ()>) {
            assert!(value.unwrap() == ());
        }
        "#;
        let findings = scan_rust_source("src/lib.rs", src);

        assert!(
            !findings
                .iter()
                .any(|f| f.kind == FindingKind::Panic && f.identity.ast_kind == "method_call")
        );
    }

    #[test]
    fn detects_panic_macros_from_syntax() {
        let src = r#"
        fn load() {
            panic!("bad");
            todo!("later");
            unimplemented!("later");
            unreachable!("bad state");
            std::panic!("scoped");
        }
        "#;
        let findings = scan_rust_source("src/lib.rs", src);

        for family in ["panic_macro", "todo", "unimplemented", "unreachable"] {
            assert!(
                findings
                    .iter()
                    .any(|f| f.kind == FindingKind::Panic && f.family.as_deref() == Some(family)),
                "missing {family}"
            );
        }
        assert_eq!(
            findings
                .iter()
                .filter(
                    |f| f.kind == FindingKind::Panic && f.family.as_deref() == Some("panic_macro")
                )
                .count(),
            2
        );
    }

    #[test]
    fn syntax_panic_macros_ignore_text_in_strings_and_comments() {
        let src = r##"
        fn load() {
            // panic!("comment");
            let text = "todo!(\"string\") unimplemented!(\"string\") unreachable!(\"string\")";
        }
        "##;
        let findings = scan_rust_source("src/lib.rs", src);

        assert!(
            !findings
                .iter()
                .any(|f| f.kind == FindingKind::Panic && f.identity.ast_kind == "macro_call")
        );
    }

    #[test]
    fn detects_unsafe_and_attrs() {
        let src = r#"
        #[allow(clippy::unwrap_used)]
        unsafe fn read() {
            unsafe { core::ptr::read(0 as *const u8); }
        }
        "#;
        let findings = scan_rust_source("src/lib.rs", src);
        assert!(
            findings
                .iter()
                .any(|f| f.kind == FindingKind::Unsafe && f.family.as_deref() == Some("unsafe_fn"))
        );
        assert!(
            findings
                .iter()
                .any(|f| f.kind == FindingKind::Unsafe
                    && f.family.as_deref() == Some("unsafe_block"))
        );
        assert!(
            findings
                .iter()
                .any(|f| f.kind == FindingKind::LintException)
        );
    }

    #[test]
    fn detects_unsafe_item_kinds_from_syntax() {
        let src = r#"
        struct Handle;
        unsafe impl Send for Handle {}
        unsafe trait Marker {}
        unsafe extern "C" {
            fn read_handle();
        }
        "#;
        let findings = scan_rust_source("src/lib.rs", src);

        for family in ["unsafe_impl", "unsafe_trait", "unsafe_extern_block"] {
            assert!(
                findings
                    .iter()
                    .any(|f| f.kind == FindingKind::Unsafe && f.family.as_deref() == Some(family)),
                "missing {family}"
            );
        }
    }

    #[test]
    fn detects_unsafe_function_signatures_from_syntax() {
        let src = r#"
        trait Reader {
            unsafe fn read();
        }
        extern "C" {
            pub unsafe fn read_handle();
        }
        "#;
        let findings = scan_rust_source("src/lib.rs", src);

        let unsafe_fn_count = findings
            .iter()
            .filter(|f| f.kind == FindingKind::Unsafe && f.family.as_deref() == Some("unsafe_fn"))
            .count();
        assert_eq!(unsafe_fn_count, 2);
    }

    #[test]
    fn syntax_unsafe_constructs_ignore_text_in_strings() {
        let src = r##"
        /// unsafe fn documented_only();
        fn load() {
            // unsafe { core::ptr::read(ptr) }
            let unsafe_fn = "unsafe fn read() {}";
            let unsafe_block = "unsafe { core::ptr::read(ptr) }";
            let unsafe_impl = "unsafe impl Send for Handle {}";
        }
        "##;
        let findings = scan_rust_source("src/lib.rs", src);

        assert!(!findings.iter().any(|f| f.kind == FindingKind::Unsafe));
    }

    #[test]
    fn detects_unsafe_attribute_from_syntax() {
        let src = r#"
        #[unsafe(no_mangle)]
        fn exported() {}
        "#;
        let findings = scan_rust_source("src/lib.rs", src);

        assert!(findings.iter().any(|f| {
            f.kind == FindingKind::Unsafe && f.family.as_deref() == Some("unsafe_attr")
        }));
        assert!(!findings.iter().any(|f| {
            f.kind == FindingKind::Unsafe && f.family.as_deref() == Some("unsafe_fn")
        }));
    }

    #[test]
    fn syntax_unsafe_attributes_ignore_attribute_text_in_strings() {
        let src = r##"
        fn load() {
            let text = "#[unsafe(no_mangle)]";
        }
        "##;
        let findings = scan_rust_source("src/lib.rs", src);

        assert!(!findings.iter().any(|f| {
            f.kind == FindingKind::Unsafe && f.family.as_deref() == Some("unsafe_attr")
        }));
    }

    #[test]
    fn syntax_lint_attributes_ignore_attribute_text_in_strings() {
        let src = r##"
        fn load() {
            let text = "#[allow(dead_code)]";
        }
        "##;
        let findings = scan_rust_source("src/lib.rs", src);

        assert!(
            !findings
                .iter()
                .any(|f| f.kind == FindingKind::LintException)
        );
    }

    #[test]
    fn syntax_indexing_ignores_common_bracket_false_positives() {
        let src = [
            "#[allow(dead_code)]",
            "fn load(xs: &[u8]) {",
            "    let literal = [1, 2, 3];",
            "    let nested_type: Vec<[u8; 4]> = Vec::new();",
            "    let macro_vec = vec![1, 2, 3];",
            "    let macro_custom = custom![1, 2, 3];",
            "    let string_literal = \"items[0]\";",
            "    use crate::{alpha, beta};",
            "    let actual = xs[0];",
            "    let call_index = xs.as_ref()[0];",
            "}",
        ]
        .join("\n");
        let findings = scan_rust_source("src/lib.rs", &src);
        let indexing = findings
            .iter()
            .filter(|f| f.family.as_deref() == Some("indexing"))
            .count();

        assert_eq!(indexing, 2);
    }

    #[test]
    fn syntax_indexing_detects_true_positive_shapes() {
        let lb = char::from(91);
        let rb = char::from(93);
        let src = format!(
            r#"
        fn load(xs: &Vec<u8>, matrix: &Vec<Vec<u8>>) {{
            let direct = xs{lb}0{rb};
            let nested = matrix{lb}0{rb}{lb}1{rb};
            let call = xs.as_ref(){lb}0{rb};
        }}
        "#
        );
        let findings = scan_rust_source("src/lib.rs", &src);
        let indexing = findings
            .iter()
            .filter(|f| f.family.as_deref() == Some("indexing"))
            .count();

        assert_eq!(indexing, 3);
    }

    #[test]
    fn index_symbol_truncates_on_character_boundaries() {
        let line = format!("let actual = values[{}];", "\u{00e9}".repeat(120));

        assert_eq!(index_symbol(&line).chars().count(), 100);
    }

    #[test]
    fn parser_foundation_parses_valid_rust() {
        let tree = parse_rust_syntax("fn load() { let value = 1; }")
            .unwrap_or_else(|err| std::panic::panic_any(format!("parser should load: {err}")));

        assert_eq!(tree.root_kind(), "source_file");
        assert!(!tree.has_error());
        assert!(tree.named_node_count() > 1);
    }

    #[test]
    fn parser_foundation_reports_invalid_rust_without_compilation() {
        let tree = parse_rust_syntax("fn broken( { let value = ;")
            .unwrap_or_else(|err| std::panic::panic_any(format!("parser should load: {err}")));

        assert_eq!(tree.root_kind(), "source_file");
        assert!(tree.has_error());
        assert!(tree.named_node_count() > 0);
    }

    #[test]
    fn syntax_containers_include_nested_module_functions() {
        let source = r#"
        mod parser {
            pub fn parse_span() {}
            mod inner {
                fn normalize_span() {}
            }
        }
        "#;
        let tree = parse_rust_syntax(source)
            .unwrap_or_else(|err| std::panic::panic_any(format!("parser should load: {err}")));
        let containers = tree.containers(source);

        let parse_span = containers
            .iter()
            .find(|container| container.name == "parse_span")
            .unwrap_or_else(|| std::panic::panic_any("parse_span container should exist"));
        assert_eq!(parse_span.kind, "function");
        assert_eq!(parse_span.module().as_deref(), Some("parser"));
        assert!(parse_span.start_line > 0);
        assert!(parse_span.end_line >= parse_span.start_line);

        let normalize_span = containers
            .iter()
            .find(|container| container.name == "normalize_span")
            .unwrap_or_else(|| std::panic::panic_any("normalize_span container should exist"));
        assert_eq!(normalize_span.module().as_deref(), Some("parser::inner"));
    }

    #[test]
    fn syntax_containers_recover_from_invalid_source() {
        let source = r#"
        fn parsed_before_error() {}
        fn broken( {
        "#;
        let tree = parse_rust_syntax(source)
            .unwrap_or_else(|err| std::panic::panic_any(format!("parser should load: {err}")));
        let containers = tree.containers(source);

        assert!(tree.has_error());
        assert!(
            containers
                .iter()
                .any(|container| container.name == "parsed_before_error")
        );
    }
}
