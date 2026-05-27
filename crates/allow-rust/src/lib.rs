use allow_core::{
    CargoAllowError, CargoAllowResult, Finding, FindingKind, Span, StructuralIdentity,
    normalize_path, normalize_snippet, stable_hash_hex,
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct RustLineScope {
    container: Option<String>,
    module_path: Vec<String>,
    span_len: u32,
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
        let mut impl_path = Vec::new();
        collect_containers(
            self.tree.root_node(),
            source,
            &mut module_path,
            &mut impl_path,
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
    impl_path: &mut Vec<String>,
    containers: &mut Vec<RustSyntaxContainer>,
) {
    if node.kind() == "mod_item" {
        if let Some(name) = node
            .child_by_field_name("name")
            .and_then(|name| node_text(source, name))
        {
            module_path.push(name.to_string());
            visit_child_containers(node, source, module_path, impl_path, containers);
            module_path.pop();
            return;
        }
    }

    if node.kind() == "impl_item" {
        if let Some(name) = impl_container_name(node, source) {
            impl_path.push(name);
            visit_child_containers(node, source, module_path, impl_path, containers);
            impl_path.pop();
            return;
        }
    }

    if node.kind() == "function_item" {
        if let Some(name) = node
            .child_by_field_name("name")
            .and_then(|name| node_text(source, name))
        {
            let (kind, name) = if let Some(impl_name) = impl_path.last() {
                ("method", format!("{impl_name}::{name}"))
            } else {
                ("function", name.to_string())
            };
            let start = node.start_position();
            let end = node.end_position();
            containers.push(RustSyntaxContainer {
                kind: kind.to_string(),
                name,
                module_path: module_path.clone(),
                start_line: start.row as u32 + 1,
                start_column: start.column as u32 + 1,
                end_line: end.row as u32 + 1,
                end_column: end.column as u32 + 1,
            });
        }
    }

    visit_child_containers(node, source, module_path, impl_path, containers);
}

fn visit_child_containers(
    node: Node<'_>,
    source: &str,
    module_path: &mut Vec<String>,
    impl_path: &mut Vec<String>,
    containers: &mut Vec<RustSyntaxContainer>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_containers(child, source, module_path, impl_path, containers);
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
    let packages = source_package_contexts(root, files)?;
    for rel in files {
        if rel.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let path = root.join(rel);
        let text = fs::read_to_string(&path)
            .map_err(|e| CargoAllowError::new(format!("failed to read {}: {e}", path.display())))?;
        let mut findings = scan_rust_source(rel, &text);
        if let Some(package) = source_package_for_path(rel, &packages) {
            for finding in &mut findings {
                finding.identity.crate_name = Some(package.name.clone());
            }
        }
        out.extend(findings);
    }
    Ok(out)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SourcePackageContext {
    root: String,
    name: String,
}

fn source_package_contexts(
    root: &Path,
    files: &[PathBuf],
) -> CargoAllowResult<Vec<SourcePackageContext>> {
    let mut packages = Vec::new();
    for rel in files {
        let normalized = normalize_path(rel);
        if normalized.rsplit('/').next() != Some("Cargo.toml") {
            continue;
        }
        let path = root.join(rel);
        let text = fs::read_to_string(&path)
            .map_err(|e| CargoAllowError::new(format!("failed to read {}: {e}", path.display())))?;
        if let Some(name) = source_package_name(&text) {
            let package_root = normalized
                .strip_suffix("Cargo.toml")
                .unwrap_or("")
                .trim_end_matches('/')
                .to_string();
            packages.push(SourcePackageContext {
                root: package_root,
                name,
            });
        }
    }
    packages.sort_by_key(|package| std::cmp::Reverse(package.root.len()));
    Ok(packages)
}

fn source_package_name(manifest: &str) -> Option<String> {
    toml::from_str::<toml::Table>(manifest)
        .ok()?
        .get("package")?
        .as_table()?
        .get("name")?
        .as_str()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
}

fn source_package_for_path<'a>(
    path: &Path,
    packages: &'a [SourcePackageContext],
) -> Option<&'a SourcePackageContext> {
    let normalized = normalize_path(path);
    packages.iter().find(|package| {
        package.root.is_empty()
            || normalized == package.root
            || normalized.starts_with(&format!("{}/", package.root))
    })
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

#[derive(Default)]
struct RustSyntaxFacts {
    index_columns: BTreeMap<u32, u32>,
    lint_attributes: BTreeMap<u32, Vec<LintAttributeKind>>,
    panic_macros: BTreeMap<u32, Vec<PanicMacroInvocation>>,
    panic_methods: BTreeMap<u32, Vec<PanicMethodCall>>,
    scopes: BTreeMap<u32, RustLineScope>,
    unsafe_constructs: BTreeMap<u32, Vec<UnsafeSyntaxConstruct>>,
    unsafe_attribute_lines: BTreeSet<u32>,
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

fn impl_container_name(node: Node<'_>, source: &str) -> Option<String> {
    let impl_type = node
        .child_by_field_name("type")
        .and_then(|type_node| node_text(source, type_node))
        .map(normalize_scope_text)?;
    if let Some(trait_name) = node
        .child_by_field_name("trait")
        .and_then(|trait_node| node_text(source, trait_node))
        .map(normalize_scope_text)
    {
        Some(format!("<{impl_type} as {trait_name}>"))
    } else {
        Some(impl_type)
    }
}

fn normalize_scope_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
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

fn lint_policy_reference(text: &str) -> Option<String> {
    let (_, after) = text.split_once("policy:")?;
    let id = after
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
        .collect::<String>();
    if id.is_empty() { None } else { Some(id) }
}

fn column(line: &str, needle: &str) -> u32 {
    line.find(needle).map(|idx| idx as u32 + 1).unwrap_or(1)
}

fn attribute_column(line: &str) -> u32 {
    line.find("#[")
        .or_else(|| line.find("#!["))
        .map_or(1, |idx| idx as u32 + 1)
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
    fn scan_rust_files_adds_source_package_context_from_manifest() {
        let root = temp_root("source-package");
        let crate_dir = root.join("crates").join("parser");
        fs::create_dir_all(crate_dir.join("src"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("crate dir: {err}")));
        fs::write(
            crate_dir.join("Cargo.toml"),
            "[package]\nname = \"parser\"\nversion = \"0.1.0\"\n",
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("manifest write: {err}")));
        fs::write(
            crate_dir.join("src").join("lib.rs"),
            "fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("rust write: {err}")));
        let files = vec![
            PathBuf::from("crates/parser/Cargo.toml"),
            PathBuf::from("crates/parser/src/lib.rs"),
        ];
        assert_eq!(
            source_package_name("[package]\nname = \"parser\"\n"),
            Some("parser".to_string())
        );
        let packages = source_package_contexts(&root, &files)
            .unwrap_or_else(|err| std::panic::panic_any(format!("package contexts: {err}")));
        assert_eq!(
            packages,
            vec![SourcePackageContext {
                root: "crates/parser".to_string(),
                name: "parser".to_string()
            }]
        );
        assert!(source_package_for_path(&files[1], &packages).is_some());

        let findings = scan_rust_files(&root, &files)
            .unwrap_or_else(|err| std::panic::panic_any(format!("scan rust files: {err}")));

        let unwrap = findings
            .iter()
            .find(|finding| finding.family.as_deref() == Some("unwrap"))
            .unwrap_or_else(|| std::panic::panic_any("expected unwrap finding"));
        assert_eq!(unwrap.identity.crate_name.as_deref(), Some("parser"));
        fs::remove_dir_all(root)
            .unwrap_or_else(|err| std::panic::panic_any(format!("cleanup: {err}")));
    }

    #[test]
    fn scan_rust_files_ignores_workspace_manifest_without_package_name() {
        let root = temp_root("workspace-manifest");
        fs::create_dir_all(root.join("src"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("src dir: {err}")));
        fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = []\n")
            .unwrap_or_else(|err| std::panic::panic_any(format!("manifest write: {err}")));
        fs::write(
            root.join("src").join("lib.rs"),
            "fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("rust write: {err}")));
        let files = vec![PathBuf::from("Cargo.toml"), PathBuf::from("src/lib.rs")];

        let findings = scan_rust_files(&root, &files)
            .unwrap_or_else(|err| std::panic::panic_any(format!("scan rust files: {err}")));

        let unwrap = findings
            .iter()
            .find(|finding| finding.family.as_deref() == Some("unwrap"))
            .unwrap_or_else(|| std::panic::panic_any("expected unwrap finding"));
        assert_eq!(unwrap.identity.crate_name, None);
        fs::remove_dir_all(root)
            .unwrap_or_else(|err| std::panic::panic_any(format!("cleanup: {err}")));
    }

    #[test]
    fn scan_rust_files_ignores_invalid_manifest_source_text() {
        let root = temp_root("invalid-manifest");
        fs::create_dir_all(root.join("src"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("src dir: {err}")));
        fs::write(root.join("Cargo.toml"), "[package\nname = \"broken\"\n")
            .unwrap_or_else(|err| std::panic::panic_any(format!("manifest write: {err}")));
        fs::write(
            root.join("src").join("lib.rs"),
            "fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("rust write: {err}")));
        let files = vec![PathBuf::from("Cargo.toml"), PathBuf::from("src/lib.rs")];

        let findings = scan_rust_files(&root, &files)
            .unwrap_or_else(|err| std::panic::panic_any(format!("scan rust files: {err}")));

        let unwrap = findings
            .iter()
            .find(|finding| finding.family.as_deref() == Some("unwrap"))
            .unwrap_or_else(|| std::panic::panic_any("expected unwrap finding"));
        assert_eq!(unwrap.identity.crate_name, None);
        fs::remove_dir_all(root)
            .unwrap_or_else(|err| std::panic::panic_any(format!("cleanup: {err}")));
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
    fn scan_uses_syntax_container_scope() {
        let src = r#"
        fn actual(value: Result<(), ()>) {
            let text = "fn fake() {";
            value.unwrap();
        }
        "#;
        let findings = scan_rust_source("src/lib.rs", src);
        let Some(finding) = findings
            .iter()
            .find(|f| f.family.as_deref() == Some("unwrap"))
        else {
            std::panic::panic_any("unwrap finding should exist");
        };

        assert_eq!(finding.identity.container.as_deref(), Some("actual"));
    }

    #[test]
    fn scan_uses_syntax_module_scope() {
        let src = r#"
        mod parser {
            fn parse(value: Result<(), ()>) {
                value.unwrap();
            }
        }

        fn load(value: Result<(), ()>) {
            value.expect("loaded");
        }
        "#;
        let findings = scan_rust_source("src/lib.rs", src);
        let Some(parser_finding) = findings
            .iter()
            .find(|f| f.family.as_deref() == Some("unwrap"))
        else {
            std::panic::panic_any("parser unwrap finding should exist");
        };
        let Some(root_finding) = findings
            .iter()
            .find(|f| f.family.as_deref() == Some("expect"))
        else {
            std::panic::panic_any("root expect finding should exist");
        };

        assert_eq!(parser_finding.identity.module.as_deref(), Some("parser"));
        assert_eq!(parser_finding.identity.container.as_deref(), Some("parse"));
        assert_eq!(root_finding.identity.module, None);
        assert_eq!(root_finding.identity.container.as_deref(), Some("load"));
    }

    #[test]
    fn scan_uses_syntax_impl_method_scope() {
        let src = r#"
        mod parser {
            struct Parser;

            impl Parser {
                fn parse(&self, value: Result<(), ()>) {
                    value.unwrap();
                }
            }
        }
        "#;
        let findings = scan_rust_source("src/lib.rs", src);
        let Some(finding) = findings
            .iter()
            .find(|f| f.family.as_deref() == Some("unwrap"))
        else {
            std::panic::panic_any("impl unwrap finding should exist");
        };

        assert_eq!(finding.identity.module.as_deref(), Some("parser"));
        assert_eq!(finding.identity.container.as_deref(), Some("Parser::parse"));
    }

    #[test]
    fn scan_uses_syntax_trait_impl_method_scope() {
        let src = r#"
        trait ParserApi {
            fn parse(&self, value: Result<(), ()>);
        }

        struct Parser;

        impl ParserApi for Parser {
            fn parse(&self, value: Result<(), ()>) {
                value.unwrap();
            }
        }
        "#;
        let findings = scan_rust_source("src/lib.rs", src);
        let Some(finding) = findings
            .iter()
            .find(|f| f.family.as_deref() == Some("unwrap"))
        else {
            std::panic::panic_any("trait impl unwrap finding should exist");
        };

        assert_eq!(
            finding.identity.container.as_deref(),
            Some("<Parser as ParserApi>::parse")
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
    fn detects_multiple_unsafe_constructs_on_one_line() {
        let src = r#"
        unsafe fn read(ptr: *const u8) -> u8 { unsafe { core::ptr::read(ptr) } }
        "#;
        let findings = scan_rust_source("src/lib.rs", src);

        assert!(findings.iter().any(|f| {
            f.kind == FindingKind::Unsafe && f.family.as_deref() == Some("unsafe_fn")
        }));
        assert!(findings.iter().any(|f| {
            f.kind == FindingKind::Unsafe && f.family.as_deref() == Some("unsafe_block")
        }));
    }

    #[test]
    fn detects_repeated_unsafe_blocks_on_one_line() {
        let src = r#"
        fn read(left: *const u8, right: *const u8) { unsafe { core::ptr::read(left); } unsafe { core::ptr::read(right); } }
        "#;
        let findings = scan_rust_source("src/lib.rs", src);
        let unsafe_blocks = findings
            .iter()
            .filter(|f| {
                f.kind == FindingKind::Unsafe && f.family.as_deref() == Some("unsafe_block")
            })
            .count();

        assert_eq!(unsafe_blocks, 2);
    }

    #[test]
    fn unsafe_findings_record_nearby_safety_comment_metadata() {
        let src = r#"
        fn read(ptr: *const u8) -> u8 {
            // SAFETY: caller provides a valid pointer.
            unsafe { core::ptr::read(ptr) }
        }
        "#;
        let findings = scan_rust_source("src/lib.rs", src);

        let unsafe_block = findings
            .iter()
            .find(|f| f.kind == FindingKind::Unsafe && f.family.as_deref() == Some("unsafe_block"))
            .unwrap_or_else(|| std::panic::panic_any("unsafe block should be found"));
        assert_eq!(
            unsafe_block.identity.target_fingerprint.as_deref(),
            Some("safety-comment:present")
        );
    }

    #[test]
    fn unsafe_findings_without_safety_comment_have_no_safety_metadata() {
        let src = r#"
        fn read(ptr: *const u8) -> u8 {
            unsafe { core::ptr::read(ptr) }
        }
        "#;
        let findings = scan_rust_source("src/lib.rs", src);

        let unsafe_block = findings
            .iter()
            .find(|f| f.kind == FindingKind::Unsafe && f.family.as_deref() == Some("unsafe_block"))
            .unwrap_or_else(|| std::panic::panic_any("unsafe block should be found"));
        assert_ne!(
            unsafe_block.identity.target_fingerprint.as_deref(),
            Some("safety-comment:present")
        );
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
    fn detects_outer_and_inner_lint_attributes_from_syntax() {
        let src = r#"
#![allow(dead_code)]

  #[expect(clippy::unwrap_used, reason = "policy:allow-lint")]
fn load() {}
        "#;
        let findings = scan_rust_source("src/lib.rs", src);

        let allow = findings
            .iter()
            .find(|f| {
                f.kind == FindingKind::LintException
                    && f.family.as_deref() == Some("allow_attribute")
            })
            .unwrap_or_else(|| std::panic::panic_any("inner allow attribute should be found"));
        assert_eq!(allow.identity.lint.as_deref(), Some("dead_code"));

        let expect = findings
            .iter()
            .find(|f| {
                f.kind == FindingKind::LintException
                    && f.family.as_deref() == Some("expect_attribute")
            })
            .unwrap_or_else(|| std::panic::panic_any("outer expect attribute should be found"));
        assert_eq!(expect.identity.lint.as_deref(), Some("clippy::unwrap_used"));
        assert!(
            expect
                .identity
                .symbol
                .as_deref()
                .is_some_and(|symbol| symbol.contains("policy:allow-lint"))
        );
        assert_eq!(
            expect.identity.target_fingerprint.as_deref(),
            Some("policy:allow-lint")
        );
        assert_eq!(expect.span.as_ref().map(|span| span.column), Some(3));
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
    fn syntax_containers_include_inherent_impl_methods() {
        let source = r#"
        mod parser {
            struct Parser;

            impl Parser {
                fn parse_span(&self) {}
            }
        }
        "#;
        let tree = parse_rust_syntax(source)
            .unwrap_or_else(|err| std::panic::panic_any(format!("parser should load: {err}")));
        let containers = tree.containers(source);

        let method = containers
            .iter()
            .find(|container| container.name == "Parser::parse_span")
            .unwrap_or_else(|| std::panic::panic_any("Parser::parse_span should exist"));
        assert_eq!(method.kind, "method");
        assert_eq!(method.module().as_deref(), Some("parser"));
        assert!(method.start_line > 0);
        assert!(method.end_line >= method.start_line);
    }

    #[test]
    fn syntax_containers_include_trait_impl_methods() {
        let source = r#"
        trait ParserApi {
            fn parse_span(&self);
        }

        struct Parser;

        impl ParserApi for Parser {
            fn parse_span(&self) {}
        }
        "#;
        let tree = parse_rust_syntax(source)
            .unwrap_or_else(|err| std::panic::panic_any(format!("parser should load: {err}")));
        let containers = tree.containers(source);

        let method = containers
            .iter()
            .find(|container| container.name == "<Parser as ParserApi>::parse_span")
            .unwrap_or_else(|| {
                std::panic::panic_any("<Parser as ParserApi>::parse_span should exist")
            });
        assert_eq!(method.kind, "method");
        assert_eq!(method.module(), None);
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

    fn temp_root(label: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_else(|err| std::panic::panic_any(format!("system clock: {err}")))
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "cargo-allow-rust-{label}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&root)
            .unwrap_or_else(|err| std::panic::panic_any(format!("temp root: {err}")));
        root
    }
}
