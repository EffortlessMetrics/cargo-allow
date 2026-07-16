use allow_core::{
    CargoAllowError, CargoAllowResult, normalize_path, read_text_file_capped, stable_hash_hex,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use tree_sitter::Node;

use crate::syntax_tree::{node_text, parse_rust_syntax};
use crate::text::source_column;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RustTestTargetKind {
    Library,
    Binary,
    IntegrationTest,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RustTestTargetIdentity {
    pub kind: RustTestTargetKind,
    pub name: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RustTestSelector {
    pub package: String,
    pub target: RustTestTargetIdentity,
    pub module_path: Vec<String>,
    pub function: String,
}

impl RustTestSelector {
    pub fn display_name(&self) -> String {
        let mut parts = self.module_path.clone();
        parts.push(self.function.clone());
        format!(
            "{}:{}:{}::{}",
            self.package,
            target_kind_name(self.target.kind),
            self.target.name,
            parts.join("::")
        )
    }

    fn validate(&self) -> bool {
        !self.package.trim().is_empty()
            && !self.target.name.trim().is_empty()
            && !self.function.trim().is_empty()
            && self
                .module_path
                .iter()
                .all(|segment| !segment.trim().is_empty())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustTestSourceRange {
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustTestSubject {
    pub selector: RustTestSelector,
    pub source_path: String,
    pub source_range: RustTestSourceRange,
    pub body_identity: String,
    pub attributes: Vec<String>,
    pub generated_or_parameterized: bool,
    pub cfg_or_feature_unknown: bool,
    pub limitations: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RustTestInventoryStatus {
    Complete,
    Partial,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RustTestInventoryDiagnosticKind {
    ManifestReadFailed,
    ManifestMalformed,
    SourceReadFailed,
    SourceParseFailed,
    TargetUnresolved,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustTestInventoryDiagnostic {
    pub kind: RustTestInventoryDiagnosticKind,
    pub path: Option<String>,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustTestInventory {
    pub subjects: Vec<RustTestSubject>,
    pub status: RustTestInventoryStatus,
    pub diagnostics: Vec<RustTestInventoryDiagnostic>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RustTestResolution {
    ResolvedExact(RustTestSubject),
    Ambiguous(Vec<RustTestSelector>),
    NotFound,
    GeneratedOrParameterized(RustTestSubject),
    CfgOrFeatureUnknown(RustTestSubject),
    PartialInventory,
    MalformedSelector,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustTestInventoryOptions {
    pub additional_test_attributes: BTreeSet<String>,
}

impl Default for RustTestInventoryOptions {
    fn default() -> Self {
        Self {
            additional_test_attributes: BTreeSet::new(),
        }
    }
}

pub fn inventory_rust_test_subjects(
    root: impl AsRef<Path>,
    files: &[PathBuf],
    options: &RustTestInventoryOptions,
) -> CargoAllowResult<RustTestInventory> {
    let root = root.as_ref();
    let mut manifests = Vec::new();
    let mut sources = Vec::new();
    let mut diagnostics = Vec::new();

    for rel in files {
        let full = root.join(rel);
        if rel.file_name().and_then(|name| name.to_str()) == Some("Cargo.toml") {
            match read_text_file_capped(&full) {
                Ok(text) => manifests.push((rel.clone(), text)),
                Err(error) => diagnostics.push(RustTestInventoryDiagnostic {
                    kind: RustTestInventoryDiagnosticKind::ManifestReadFailed,
                    path: Some(normalize_path(rel)),
                    message: error.to_string(),
                }),
            }
        } else if rel.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            match read_text_file_capped(&full) {
                Ok(text) => sources.push((rel.clone(), text)),
                Err(error) => diagnostics.push(RustTestInventoryDiagnostic {
                    kind: RustTestInventoryDiagnosticKind::SourceReadFailed,
                    path: Some(normalize_path(rel)),
                    message: error.to_string(),
                }),
            }
        }
    }

    let mut inventory = inventory_rust_test_subjects_from_sources(manifests, sources, options);
    inventory.diagnostics.extend(diagnostics);
    inventory.diagnostics.sort_by(diagnostic_order);
    inventory.diagnostics.dedup();
    if !inventory.diagnostics.is_empty() {
        inventory.status = RustTestInventoryStatus::Partial;
    }
    Ok(inventory)
}

pub fn inventory_rust_test_subjects_from_sources(
    manifests: impl IntoIterator<Item = (PathBuf, String)>,
    sources: impl IntoIterator<Item = (PathBuf, String)>,
    options: &RustTestInventoryOptions,
) -> RustTestInventory {
    let mut diagnostics = Vec::new();
    let mut packages = manifests
        .into_iter()
        .filter_map(|(path, text)| match PackageManifest::parse(&path, &text) {
            Ok(package) => Some(package),
            Err(error) => {
                diagnostics.push(RustTestInventoryDiagnostic {
                    kind: RustTestInventoryDiagnosticKind::ManifestMalformed,
                    path: Some(normalize_path(&path)),
                    message: error.to_string(),
                });
                None
            }
        })
        .collect::<Vec<_>>();
    packages.sort_by_key(|package| std::cmp::Reverse(package.root.components().count()));

    let mut subjects = Vec::new();
    for (path, source) in sources {
        let Some(package) = packages.iter().find(|package| package.owns(&path)) else {
            diagnostics.push(RustTestInventoryDiagnostic {
                kind: RustTestInventoryDiagnosticKind::TargetUnresolved,
                path: Some(normalize_path(&path)),
                message: "Rust source is not owned by a parsed package manifest".to_string(),
            });
            continue;
        };
        let Some(target) = package.target_for(&path) else {
            diagnostics.push(RustTestInventoryDiagnostic {
                kind: RustTestInventoryDiagnosticKind::TargetUnresolved,
                path: Some(normalize_path(&path)),
                message:
                    "Rust source target cannot be resolved from source-only Cargo manifest rules"
                        .to_string(),
            });
            continue;
        };

        let syntax = match parse_rust_syntax(&source) {
            Ok(syntax) => syntax,
            Err(error) => {
                diagnostics.push(RustTestInventoryDiagnostic {
                    kind: RustTestInventoryDiagnosticKind::SourceParseFailed,
                    path: Some(normalize_path(&path)),
                    message: error.to_string(),
                });
                continue;
            }
        };
        if syntax.has_error() {
            diagnostics.push(RustTestInventoryDiagnostic {
                kind: RustTestInventoryDiagnosticKind::SourceParseFailed,
                path: Some(normalize_path(&path)),
                message: "Rust syntax tree contains parse errors; discovered subjects are partial"
                    .to_string(),
            });
        }

        let mut file_prefix = file_module_prefix(&path, &target.root_path);
        let mut inline_modules = Vec::new();
        collect_test_subjects(
            syntax.tree.root_node(),
            &source,
            &path,
            package,
            &target,
            &mut file_prefix,
            &mut inline_modules,
            false,
            options,
            &mut subjects,
        );
    }

    subjects.sort_by(|left, right| {
        left.selector
            .cmp(&right.selector)
            .then_with(|| left.source_path.cmp(&right.source_path))
            .then_with(|| {
                left.source_range
                    .start_line
                    .cmp(&right.source_range.start_line)
            })
    });
    diagnostics.sort_by(diagnostic_order);
    diagnostics.dedup();
    RustTestInventory {
        status: if diagnostics.is_empty() {
            RustTestInventoryStatus::Complete
        } else {
            RustTestInventoryStatus::Partial
        },
        subjects,
        diagnostics,
    }
}

pub fn resolve_rust_test_selector(
    inventory: &RustTestInventory,
    selector: &RustTestSelector,
) -> RustTestResolution {
    if !selector.validate() {
        return RustTestResolution::MalformedSelector;
    }
    let matches = inventory
        .subjects
        .iter()
        .filter(|subject| &subject.selector == selector)
        .cloned()
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] if inventory.status == RustTestInventoryStatus::Partial => {
            RustTestResolution::PartialInventory
        }
        [] => RustTestResolution::NotFound,
        [subject] if subject.generated_or_parameterized => {
            RustTestResolution::GeneratedOrParameterized(subject.clone())
        }
        [subject] if subject.cfg_or_feature_unknown => {
            RustTestResolution::CfgOrFeatureUnknown(subject.clone())
        }
        [subject] => RustTestResolution::ResolvedExact(subject.clone()),
        _ => RustTestResolution::Ambiguous(
            matches
                .into_iter()
                .map(|subject| subject.selector)
                .collect(),
        ),
    }
}

#[derive(Clone, Debug)]
struct TargetContext {
    identity: RustTestTargetIdentity,
    root_path: PathBuf,
    source_membership_limited: bool,
}

#[derive(Clone, Debug)]
struct DeclaredTarget {
    identity: RustTestTargetIdentity,
    path: PathBuf,
}

#[derive(Clone, Debug)]
struct PackageManifest {
    root: PathBuf,
    name: String,
    library: DeclaredTarget,
    binaries: Vec<DeclaredTarget>,
    integration_tests: Vec<DeclaredTarget>,
}

impl PackageManifest {
    fn parse(path: &Path, text: &str) -> Result<Self, CargoAllowError> {
        let table = toml::from_str::<toml::Table>(text)
            .map_err(|error| CargoAllowError::new(format!("invalid Cargo manifest: {error}")))?;
        let package = table
            .get("package")
            .and_then(toml::Value::as_table)
            .ok_or_else(|| CargoAllowError::new("Cargo manifest has no [package] table"))?;
        let name = package
            .get("name")
            .and_then(toml::Value::as_str)
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .ok_or_else(|| CargoAllowError::new("Cargo package name is missing"))?
            .to_string();
        let root = path.parent().unwrap_or_else(|| Path::new("")).to_path_buf();
        let default_target_name = name.replace('-', "_");

        let library_table = table.get("lib").and_then(toml::Value::as_table);
        let library = DeclaredTarget {
            identity: RustTestTargetIdentity {
                kind: RustTestTargetKind::Library,
                name: library_table
                    .and_then(|lib| lib.get("name"))
                    .and_then(toml::Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| default_target_name.clone()),
            },
            path: root.join(
                library_table
                    .and_then(|lib| lib.get("path"))
                    .and_then(toml::Value::as_str)
                    .unwrap_or("src/lib.rs"),
            ),
        };
        let binaries = declared_targets(
            &table,
            "bin",
            RustTestTargetKind::Binary,
            &root,
            &default_target_name,
            "src/main.rs",
        );
        let integration_tests = declared_targets(
            &table,
            "test",
            RustTestTargetKind::IntegrationTest,
            &root,
            "",
            "",
        );

        Ok(Self {
            root,
            name,
            library,
            binaries,
            integration_tests,
        })
    }

    fn owns(&self, path: &Path) -> bool {
        self.root.as_os_str().is_empty() || path.starts_with(&self.root)
    }

    fn target_for(&self, path: &Path) -> Option<TargetContext> {
        if let Some(target) = exact_or_descendant_target(&self.integration_tests, path) {
            return Some(target);
        }
        if let Some(target) = exact_or_descendant_target(&self.binaries, path) {
            return Some(target);
        }

        let relative = path.strip_prefix(&self.root).ok()?;
        let components = path_components(relative);
        if matches!(components.as_slice(), [first, file] if first == "tests" && file.ends_with(".rs"))
        {
            let name = Path::new(&components[1]).file_stem()?.to_str()?.to_string();
            return Some(TargetContext {
                identity: RustTestTargetIdentity {
                    kind: RustTestTargetKind::IntegrationTest,
                    name,
                },
                root_path: path.to_path_buf(),
                source_membership_limited: false,
            });
        }
        if relative == Path::new("src/main.rs") {
            return Some(TargetContext {
                identity: RustTestTargetIdentity {
                    kind: RustTestTargetKind::Binary,
                    name: self.name.replace('-', "_"),
                },
                root_path: path.to_path_buf(),
                source_membership_limited: false,
            });
        }
        if components
            .first()
            .is_some_and(|component| component == "src")
            && components
                .get(1)
                .is_some_and(|component| component == "bin")
        {
            return default_bin_target(path, &self.root, &components);
        }
        if relative.starts_with("src") {
            return Some(TargetContext {
                identity: self.library.identity.clone(),
                root_path: self.library.path.clone(),
                source_membership_limited: path != self.library.path,
            });
        }
        None
    }
}

fn declared_targets(
    table: &toml::Table,
    key: &str,
    kind: RustTestTargetKind,
    root: &Path,
    default_name: &str,
    default_path: &str,
) -> Vec<DeclaredTarget> {
    let mut targets = Vec::new();
    if key == "bin" && table.get(key).is_none() {
        targets.push(DeclaredTarget {
            identity: RustTestTargetIdentity {
                kind,
                name: default_name.to_string(),
            },
            path: root.join(default_path),
        });
        return targets;
    }
    let Some(values) = table.get(key).and_then(toml::Value::as_array) else {
        return targets;
    };
    for value in values {
        let Some(target) = value.as_table() else {
            continue;
        };
        let Some(name) = target
            .get("name")
            .and_then(toml::Value::as_str)
            .map(str::trim)
            .filter(|name| !name.is_empty())
        else {
            continue;
        };
        let path = target
            .get("path")
            .and_then(toml::Value::as_str)
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                if kind == RustTestTargetKind::Binary {
                    PathBuf::from(format!("src/bin/{name}.rs"))
                } else {
                    PathBuf::from(format!("tests/{name}.rs"))
                }
            });
        targets.push(DeclaredTarget {
            identity: RustTestTargetIdentity {
                kind,
                name: name.to_string(),
            },
            path: root.join(path),
        });
    }
    targets.sort_by(|left, right| left.path.cmp(&right.path));
    targets
}

fn exact_or_descendant_target(targets: &[DeclaredTarget], path: &Path) -> Option<TargetContext> {
    targets.iter().find_map(|target| {
        if path == target.path {
            return Some(TargetContext {
                identity: target.identity.clone(),
                root_path: target.path.clone(),
                source_membership_limited: false,
            });
        }
        let parent = target.path.parent()?;
        path.starts_with(parent).then(|| TargetContext {
            identity: target.identity.clone(),
            root_path: target.path.clone(),
            source_membership_limited: true,
        })
    })
}

fn default_bin_target(
    path: &Path,
    package_root: &Path,
    components: &[String],
) -> Option<TargetContext> {
    match components {
        [src, bin, file] if src == "src" && bin == "bin" && file.ends_with(".rs") => {
            let name = Path::new(file).file_stem()?.to_str()?.to_string();
            Some(TargetContext {
                identity: RustTestTargetIdentity {
                    kind: RustTestTargetKind::Binary,
                    name,
                },
                root_path: path.to_path_buf(),
                source_membership_limited: false,
            })
        }
        [src, bin, name, rest @ ..] if src == "src" && bin == "bin" && !rest.is_empty() => {
            Some(TargetContext {
                identity: RustTestTargetIdentity {
                    kind: RustTestTargetKind::Binary,
                    name: name.clone(),
                },
                root_path: package_root
                    .join("src")
                    .join("bin")
                    .join(name)
                    .join("main.rs"),
                source_membership_limited: path
                    != package_root
                        .join("src")
                        .join("bin")
                        .join(name)
                        .join("main.rs"),
            })
        }
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_test_subjects(
    node: Node<'_>,
    source: &str,
    path: &Path,
    package: &PackageManifest,
    target: &TargetContext,
    file_prefix: &mut Vec<String>,
    inline_modules: &mut Vec<String>,
    inherited_cfg_unknown: bool,
    options: &RustTestInventoryOptions,
    subjects: &mut Vec<RustTestSubject>,
) {
    let attributes = node_attributes(node, source);
    let local_cfg_unknown = attributes.iter().any(|attribute| cfg_is_unknown(attribute));

    if node.kind() == "mod_item" {
        if let Some(name) = node
            .child_by_field_name("name")
            .and_then(|name| node_text(source, name))
        {
            inline_modules.push(name.to_string());
            visit_children(
                node,
                source,
                path,
                package,
                target,
                file_prefix,
                inline_modules,
                inherited_cfg_unknown || local_cfg_unknown,
                options,
                subjects,
            );
            inline_modules.pop();
            return;
        }
    }

    if node.kind() == "function_item" {
        let attribute_names = attributes
            .iter()
            .filter_map(|attribute| attribute_name(attribute))
            .collect::<Vec<_>>();
        let is_test = attribute_names.iter().any(|name| name == "test")
            || attribute_names
                .iter()
                .any(|name| options.additional_test_attributes.contains(name));
        if is_test {
            if let Some(name) = node
                .child_by_field_name("name")
                .and_then(|name| node_text(source, name))
            {
                let start_byte = attribute_start_byte(node, source).unwrap_or(node.start_byte());
                let start_position =
                    attribute_start_position(node, source).unwrap_or_else(|| node.start_position());
                let end_position = node.end_position();
                let body = source
                    .get(start_byte..node.end_byte())
                    .unwrap_or_default()
                    .replace("\r\n", "\n");
                let mut module_path = file_prefix.clone();
                module_path.extend(inline_modules.iter().cloned());
                let generated_or_parameterized = attribute_names.iter().any(|name| {
                    name == "rstest"
                        || name == "test_case"
                        || name.ends_with("::rstest")
                        || name.ends_with("::test_case")
                });
                let cfg_or_feature_unknown = inherited_cfg_unknown || local_cfg_unknown;
                let mut limitations = Vec::new();
                if target.source_membership_limited {
                    limitations.push(
                        "source-only analysis cannot prove the module is included by the target"
                            .to_string(),
                    );
                }
                if generated_or_parameterized {
                    limitations.push(
                        "parameterized/generated test cases do not have exact executable identities"
                            .to_string(),
                    );
                }
                if cfg_or_feature_unknown {
                    limitations.push(
                        "cfg or feature expansion is not resolved by source-only inventory"
                            .to_string(),
                    );
                }
                limitations.sort();
                limitations.dedup();
                subjects.push(RustTestSubject {
                    selector: RustTestSelector {
                        package: package.name.clone(),
                        target: target.identity.clone(),
                        module_path,
                        function: name.to_string(),
                    },
                    source_path: normalize_path(path),
                    source_range: RustTestSourceRange {
                        start_line: start_position.row as u32 + 1,
                        start_column: source_column(
                            source,
                            start_position.row,
                            start_position.column,
                        ),
                        end_line: end_position.row as u32 + 1,
                        end_column: source_column(source, end_position.row, end_position.column),
                    },
                    body_identity: stable_hash_hex(&body),
                    attributes,
                    generated_or_parameterized,
                    cfg_or_feature_unknown,
                    limitations,
                });
            }
        }
    }

    visit_children(
        node,
        source,
        path,
        package,
        target,
        file_prefix,
        inline_modules,
        inherited_cfg_unknown || local_cfg_unknown,
        options,
        subjects,
    );
}

#[allow(clippy::too_many_arguments)]
fn visit_children(
    node: Node<'_>,
    source: &str,
    path: &Path,
    package: &PackageManifest,
    target: &TargetContext,
    file_prefix: &mut Vec<String>,
    inline_modules: &mut Vec<String>,
    inherited_cfg_unknown: bool,
    options: &RustTestInventoryOptions,
    subjects: &mut Vec<RustTestSubject>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_test_subjects(
            child,
            source,
            path,
            package,
            target,
            file_prefix,
            inline_modules,
            inherited_cfg_unknown,
            options,
            subjects,
        );
    }
}

fn node_attributes(node: Node<'_>, source: &str) -> Vec<String> {
    let mut nodes = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "attribute_item" {
            nodes.push(child);
        }
    }
    if nodes.is_empty() {
        let mut previous = node.prev_named_sibling();
        while let Some(attribute) = previous {
            if attribute.kind() != "attribute_item" {
                break;
            }
            nodes.push(attribute);
            previous = attribute.prev_named_sibling();
        }
        nodes.reverse();
    }
    nodes
        .into_iter()
        .filter_map(|attribute| node_text(source, attribute))
        .map(|attribute| attribute.trim().to_string())
        .collect()
}

fn attribute_start_byte(node: Node<'_>, source: &str) -> Option<usize> {
    attribute_nodes(node, source)
        .into_iter()
        .map(|attribute| attribute.start_byte())
        .min()
}

fn attribute_start_position(node: Node<'_>, source: &str) -> Option<tree_sitter::Point> {
    attribute_nodes(node, source)
        .into_iter()
        .map(|attribute| attribute.start_position())
        .min_by_key(|position| (position.row, position.column))
}

fn attribute_nodes<'tree>(node: Node<'tree>, source: &str) -> Vec<Node<'tree>> {
    let mut nodes = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "attribute_item" && node_text(source, child).is_some() {
            nodes.push(child);
        }
    }
    if nodes.is_empty() {
        let mut previous = node.prev_named_sibling();
        while let Some(attribute) = previous {
            if attribute.kind() != "attribute_item" || node_text(source, attribute).is_none() {
                break;
            }
            nodes.push(attribute);
            previous = attribute.prev_named_sibling();
        }
        nodes.reverse();
    }
    nodes
}

fn attribute_name(attribute: &str) -> Option<String> {
    let trimmed = attribute.trim().strip_prefix("#[")?.strip_suffix(']')?;
    let name = trimmed.split_once('(').map_or(trimmed, |(name, _)| name);
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_string())
}

fn cfg_is_unknown(attribute: &str) -> bool {
    let compact = attribute.split_whitespace().collect::<String>();
    (compact.starts_with("#[cfg(") || compact.starts_with("#[cfg_attr("))
        && compact != "#[cfg(test)]"
}

fn file_module_prefix(path: &Path, target_root: &Path) -> Vec<String> {
    if path == target_root {
        return Vec::new();
    }
    let Some(root) = target_root.parent() else {
        return Vec::new();
    };
    let Ok(relative) = path.strip_prefix(root) else {
        return Vec::new();
    };
    let mut components = relative
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str().map(str::to_string),
            _ => None,
        })
        .collect::<Vec<_>>();
    let Some(file) = components.pop() else {
        return Vec::new();
    };
    if let Some(stem) = Path::new(&file).file_stem().and_then(|stem| stem.to_str()) {
        if !matches!(stem, "lib" | "main" | "mod") {
            components.push(stem.to_string());
        }
    }
    components
}

fn path_components(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str().map(str::to_string),
            _ => None,
        })
        .collect()
}

fn target_kind_name(kind: RustTestTargetKind) -> &'static str {
    match kind {
        RustTestTargetKind::Library => "lib",
        RustTestTargetKind::Binary => "bin",
        RustTestTargetKind::IntegrationTest => "test",
    }
}

fn diagnostic_order(
    left: &RustTestInventoryDiagnostic,
    right: &RustTestInventoryDiagnostic,
) -> std::cmp::Ordering {
    left.path
        .cmp(&right.path)
        .then_with(|| format!("{:?}", left.kind).cmp(&format!("{:?}", right.kind)))
        .then_with(|| left.message.cmp(&right.message))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> Vec<(PathBuf, String)> {
        vec![(
            PathBuf::from("crates/demo/Cargo.toml"),
            "[package]\nname = \"demo-package\"\nversion = \"0.1.0\"\n".to_string(),
        )]
    }

    #[test]
    fn inventories_nested_inline_unit_test() {
        let sources = vec![(
            PathBuf::from("crates/demo/src/lib.rs"),
            r#"
#[cfg(test)]
mod policy_tests {
    #[test]
    fn rejects_boundary() {
        assert_eq!(2 + 2, 4);
    }
}
"#
            .to_string(),
        )];
        let inventory = inventory_rust_test_subjects_from_sources(
            manifest(),
            sources,
            &RustTestInventoryOptions::default(),
        );
        assert_eq!(inventory.status, RustTestInventoryStatus::Complete);
        assert_eq!(inventory.subjects.len(), 1);
        let subject = &inventory.subjects[0];
        assert_eq!(subject.selector.package, "demo-package");
        assert_eq!(subject.selector.target.kind, RustTestTargetKind::Library);
        assert_eq!(subject.selector.target.name, "demo_package");
        assert_eq!(subject.selector.module_path, vec!["policy_tests"]);
        assert_eq!(subject.selector.function, "rejects_boundary");
        assert!(!subject.cfg_or_feature_unknown);
    }

    #[test]
    fn same_named_integration_tests_have_distinct_targets() {
        let sources = vec![
            (
                PathBuf::from("crates/demo/tests/alpha.rs"),
                "#[test]\nfn roundtrip() {}\n".to_string(),
            ),
            (
                PathBuf::from("crates/demo/tests/beta.rs"),
                "#[test]\nfn roundtrip() {}\n".to_string(),
            ),
        ];
        let inventory = inventory_rust_test_subjects_from_sources(
            manifest(),
            sources,
            &RustTestInventoryOptions::default(),
        );
        assert_eq!(inventory.subjects.len(), 2);
        assert_ne!(
            inventory.subjects[0].selector.target.name,
            inventory.subjects[1].selector.target.name
        );
    }

    #[test]
    fn changed_body_changes_identity_without_changing_selector() {
        let path = PathBuf::from("crates/demo/src/lib.rs");
        let first = inventory_rust_test_subjects_from_sources(
            manifest(),
            vec![(
                path.clone(),
                "#[test]\nfn exact() { assert_eq!(1, 1); }".into(),
            )],
            &RustTestInventoryOptions::default(),
        );
        let second = inventory_rust_test_subjects_from_sources(
            manifest(),
            vec![(path, "#[test]\nfn exact() { assert_eq!(1, 2); }".into())],
            &RustTestInventoryOptions::default(),
        );
        assert_eq!(first.subjects[0].selector, second.subjects[0].selector);
        assert_ne!(
            first.subjects[0].body_identity,
            second.subjects[0].body_identity
        );
    }

    #[test]
    fn cfg_limited_test_resolves_as_unknown() {
        let inventory = inventory_rust_test_subjects_from_sources(
            manifest(),
            vec![(
                PathBuf::from("crates/demo/src/lib.rs"),
                "#[cfg(feature = \"special\")]\n#[test]\nfn gated() {}".into(),
            )],
            &RustTestInventoryOptions::default(),
        );
        let selector = inventory.subjects[0].selector.clone();
        assert!(matches!(
            resolve_rust_test_selector(&inventory, &selector),
            RustTestResolution::CfgOrFeatureUnknown(_)
        ));
    }

    #[test]
    fn configured_parameterized_test_stays_non_exact() {
        let mut options = RustTestInventoryOptions::default();
        options.additional_test_attributes.insert("rstest".into());
        let inventory = inventory_rust_test_subjects_from_sources(
            manifest(),
            vec![(
                PathBuf::from("crates/demo/src/lib.rs"),
                "#[rstest]\nfn cases(#[case] value: u32) { assert!(value > 0); }".into(),
            )],
            &options,
        );
        let selector = inventory.subjects[0].selector.clone();
        assert!(matches!(
            resolve_rust_test_selector(&inventory, &selector),
            RustTestResolution::GeneratedOrParameterized(_)
        ));
    }

    #[test]
    fn missing_selector_is_not_guessed() {
        let inventory = inventory_rust_test_subjects_from_sources(
            manifest(),
            vec![(
                PathBuf::from("crates/demo/src/lib.rs"),
                "#[test]\nfn exact_name() {}".into(),
            )],
            &RustTestInventoryOptions::default(),
        );
        let mut selector = inventory.subjects[0].selector.clone();
        selector.function = "nearby_name".into();
        assert_eq!(
            resolve_rust_test_selector(&inventory, &selector),
            RustTestResolution::NotFound
        );
    }
}
