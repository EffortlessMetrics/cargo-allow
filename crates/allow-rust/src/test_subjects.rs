use allow_core::{
    CargoAllowError, CargoAllowResult, normalize_path, read_text_file_capped, stable_hash_hex,
};
use std::path::{Component, Path, PathBuf};
use tree_sitter::Node;

use crate::syntax_tree::{node_text, parse_rust_syntax};
use crate::text::source_column;

#[path = "snapshot_package/test_subjects.rs"]
mod subject_types;
pub use subject_types::*;

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
        let normalized_rel = normalize_repo_path(rel.clone());
        let full = root.join(rel);
        if normalized_rel.file_name().and_then(|name| name.to_str()) == Some("Cargo.toml") {
            match read_text_file_capped(&full) {
                Ok(text) => manifests.push((normalized_rel, text)),
                Err(error) => diagnostics.push(RustTestInventoryDiagnostic {
                    kind: RustTestInventoryDiagnosticKind::ManifestReadFailed,
                    path: Some(normalize_path(rel)),
                    message: error.to_string(),
                }),
            }
        } else if normalized_rel
            .extension()
            .and_then(|extension| extension.to_str())
            == Some("rs")
        {
            match read_text_file_capped(&full) {
                Ok(text) => sources.push((normalized_rel, text)),
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
    finalize_inventory(&mut inventory);
    Ok(inventory)
}

pub fn inventory_rust_test_subjects_from_sources(
    manifests: impl IntoIterator<Item = (PathBuf, String)>,
    sources: impl IntoIterator<Item = (PathBuf, String)>,
    options: &RustTestInventoryOptions,
) -> RustTestInventory {
    let sources = sources
        .into_iter()
        .map(|(path, source)| (normalize_repo_path(path), strip_bom(source)))
        .collect::<Vec<_>>();
    let source_paths = sources
        .iter()
        .map(|(path, _)| path.clone())
        .collect::<Vec<_>>();
    let mut diagnostics = Vec::new();
    let mut packages = Vec::new();

    for (path, text) in manifests {
        let path = normalize_repo_path(path);
        match PackageManifest::parse(&path, &text, &source_paths) {
            Ok(Some(package)) => packages.push(package),
            Ok(None) => {}
            Err(error) => diagnostics.push(RustTestInventoryDiagnostic {
                kind: RustTestInventoryDiagnosticKind::ManifestMalformed,
                path: Some(normalize_path(&path)),
                message: error.to_string(),
            }),
        }
    }
    packages.sort_by_key(|package| std::cmp::Reverse(package.root.components().count()));

    let mut subjects = Vec::new();
    for (path, source) in sources {
        let Some(package) = packages.iter().find(|package| package.owns(&path)) else {
            diagnostics.push(target_diagnostic(
                &path,
                "Rust source is not owned by a parsed package manifest",
            ));
            continue;
        };
        let Some(target) = package.target_for(&path) else {
            diagnostics.push(target_diagnostic(
                &path,
                "Rust source target cannot be resolved conservatively from source-only Cargo rules",
            ));
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

        let file_prefix = file_module_prefix(&path, &target);
        let mut traversal = TestTraversal {
            source: &source,
            path: &path,
            package,
            target: &target,
            file_prefix: &file_prefix,
            inline_modules: Vec::new(),
            options,
            subjects: &mut subjects,
        };
        traversal.collect(syntax.tree.root_node(), false);
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
    let mut inventory = RustTestInventory {
        subjects,
        status: RustTestInventoryStatus::Complete,
        diagnostics,
    };
    finalize_inventory(&mut inventory);
    inventory
}

pub fn resolve_rust_test_selector(
    inventory: &RustTestInventory,
    selector: &RustTestSelector,
) -> RustTestResolution {
    if !selector.validate() {
        return RustTestResolution::MalformedSelector;
    }
    if inventory.status == RustTestInventoryStatus::Partial {
        return RustTestResolution::PartialInventory;
    }

    let matches = inventory
        .subjects
        .iter()
        .filter(|subject| &subject.selector == selector)
        .cloned()
        .collect::<Vec<_>>();
    let mut subjects = matches.into_iter();
    let Some(subject) = subjects.next() else {
        return RustTestResolution::NotFound;
    };
    if subjects.next().is_some() {
        let mut selectors = inventory
            .subjects
            .iter()
            .filter(|candidate| &candidate.selector == selector)
            .map(|candidate| candidate.selector.clone())
            .collect::<Vec<_>>();
        selectors.sort();
        return RustTestResolution::Ambiguous(selectors);
    }
    if subject.ignored {
        RustTestResolution::Ignored(subject)
    } else if subject.generated_or_parameterized {
        RustTestResolution::GeneratedOrParameterized(subject)
    } else if subject.cfg_or_feature_unknown {
        RustTestResolution::CfgOrFeatureUnknown(subject)
    } else {
        RustTestResolution::ResolvedExact(subject)
    }
}

fn finalize_inventory(inventory: &mut RustTestInventory) {
    inventory.diagnostics.sort_by(diagnostic_order);
    inventory.diagnostics.dedup();
    inventory.status = if inventory.diagnostics.is_empty() {
        RustTestInventoryStatus::Complete
    } else {
        RustTestInventoryStatus::Partial
    };
}

fn target_diagnostic(path: &Path, message: &str) -> RustTestInventoryDiagnostic {
    RustTestInventoryDiagnostic {
        kind: RustTestInventoryDiagnosticKind::TargetUnresolved,
        path: Some(normalize_path(path)),
        message: message.to_string(),
    }
}

#[derive(Clone, Debug)]
struct TargetContext {
    identity: RustTestTargetIdentity,
    root_path: PathBuf,
    module_base: PathBuf,
    source_membership_limited: bool,
}

#[derive(Clone, Debug)]
struct DeclaredTarget {
    identity: RustTestTargetIdentity,
    path: PathBuf,
    module_base: PathBuf,
    descendant_root: Option<PathBuf>,
    top_level_source_root: Option<PathBuf>,
}

#[derive(Clone, Debug)]
struct PackageManifest {
    root: PathBuf,
    name: String,
    targets: Vec<DeclaredTarget>,
}

impl PackageManifest {
    fn parse(
        path: &Path,
        text: &str,
        source_paths: &[PathBuf],
    ) -> Result<Option<Self>, CargoAllowError> {
        let table = toml::from_str::<toml::Table>(text)
            .map_err(|error| CargoAllowError::new(format!("invalid Cargo manifest: {error}")))?;
        let Some(package) = table.get("package").and_then(toml::Value::as_table) else {
            if table
                .get("workspace")
                .and_then(toml::Value::as_table)
                .is_some()
            {
                return Ok(None);
            }
            return Err(CargoAllowError::new(
                "Cargo manifest has neither [package] nor [workspace]",
            ));
        };
        let name = package
            .get("name")
            .and_then(toml::Value::as_str)
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .ok_or_else(|| CargoAllowError::new("Cargo package name is missing"))?
            .to_string();
        let root = path.parent().unwrap_or_else(|| Path::new("")).to_path_buf();
        let default_target_name = name.replace('-', "_");
        let autobins = package_bool(package, "autobins", true);
        let autotests = package_bool(package, "autotests", true);
        let mut targets = Vec::new();

        if let Some(library) = library_target(&table, &root, &default_target_name, source_paths) {
            targets.push(library);
        }
        targets.extend(explicit_targets(
            &table,
            "bin",
            RustTestTargetKind::Binary,
            &root,
        ));
        targets.extend(explicit_targets(
            &table,
            "test",
            RustTestTargetKind::IntegrationTest,
            &root,
        ));
        if autobins {
            targets.extend(default_binary_targets(
                &root,
                &default_target_name,
                source_paths,
            ));
        }
        if autotests {
            targets.extend(default_integration_targets(&root, source_paths));
        }

        targets.sort_by(|left, right| {
            left.identity
                .cmp(&right.identity)
                .then_with(|| left.path.cmp(&right.path))
        });
        targets.dedup_by(|left, right| left.identity == right.identity);
        assign_descendant_roots(&root, &mut targets);
        Ok(Some(Self {
            root,
            name,
            targets,
        }))
    }

    fn owns(&self, path: &Path) -> bool {
        self.root.as_os_str().is_empty() || path.starts_with(&self.root)
    }

    fn target_for(&self, path: &Path) -> Option<TargetContext> {
        if let Some(target) = self.targets.iter().find(|target| path == target.path) {
            return Some(target.context(false));
        }
        if let Some(target) = self.targets.iter().find(|target| {
            target
                .descendant_root
                .as_ref()
                .is_some_and(|root| path.starts_with(root))
        }) {
            return Some(target.context(true));
        }

        let src_root = self.root.join("src");
        if !path.starts_with(&src_root) || path.starts_with(src_root.join("bin")) {
            return None;
        }
        let candidates = self
            .targets
            .iter()
            .filter(|target| {
                target
                    .top_level_source_root
                    .as_ref()
                    .is_some_and(|root| path.starts_with(root))
            })
            .collect::<Vec<_>>();
        match candidates.as_slice() {
            [target] => Some(target.context(true)),
            _ => None,
        }
    }
}

impl DeclaredTarget {
    fn context(&self, source_membership_limited: bool) -> TargetContext {
        TargetContext {
            identity: self.identity.clone(),
            root_path: self.path.clone(),
            module_base: self.module_base.clone(),
            source_membership_limited,
        }
    }
}

fn package_bool(package: &toml::Table, key: &str, default: bool) -> bool {
    package
        .get(key)
        .and_then(toml::Value::as_bool)
        .unwrap_or(default)
}

fn library_target(
    table: &toml::Table,
    root: &Path,
    default_name: &str,
    source_paths: &[PathBuf],
) -> Option<DeclaredTarget> {
    let library_table = table.get("lib").and_then(toml::Value::as_table);
    let default_path = root.join("src/lib.rs");
    if library_table.is_none() && !source_paths.iter().any(|path| path == &default_path) {
        return None;
    }
    let name = library_table
        .and_then(|library| library.get("name"))
        .and_then(toml::Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or(default_name)
        .to_string();
    let path = library_table
        .and_then(|library| library.get("path"))
        .and_then(toml::Value::as_str)
        .map(PathBuf::from)
        .map(|path| root.join(path))
        .unwrap_or(default_path);
    Some(target_from_path(
        RustTestTargetKind::Library,
        name,
        path,
        root,
        true,
    ))
}

fn explicit_targets(
    table: &toml::Table,
    key: &str,
    kind: RustTestTargetKind,
    root: &Path,
) -> Vec<DeclaredTarget> {
    let mut targets = Vec::new();
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
            .unwrap_or_else(|| default_declared_target_path(kind, name));
        targets.push(target_from_path(
            kind,
            name.to_string(),
            root.join(path),
            root,
            false,
        ));
    }
    targets
}

fn default_declared_target_path(kind: RustTestTargetKind, name: &str) -> PathBuf {
    match kind {
        RustTestTargetKind::Binary => PathBuf::from(format!("src/bin/{name}.rs")),
        RustTestTargetKind::IntegrationTest => PathBuf::from(format!("tests/{name}.rs")),
        RustTestTargetKind::Library => PathBuf::from("src/lib.rs"),
    }
}

fn default_binary_targets(
    root: &Path,
    default_name: &str,
    source_paths: &[PathBuf],
) -> Vec<DeclaredTarget> {
    let mut targets = Vec::new();
    let main = root.join("src/main.rs");
    if source_paths.iter().any(|path| path == &main) {
        targets.push(target_from_path(
            RustTestTargetKind::Binary,
            default_name.to_string(),
            main,
            root,
            true,
        ));
    }
    let bin_root = root.join("src/bin");
    for source in source_paths {
        let Ok(relative) = source.strip_prefix(&bin_root) else {
            continue;
        };
        let components = path_components(relative);
        match components.as_slice() {
            [file] if file.ends_with(".rs") => {
                if let Some(name) = file_stem(file) {
                    targets.push(target_from_path(
                        RustTestTargetKind::Binary,
                        name,
                        source.clone(),
                        root,
                        false,
                    ));
                }
            }
            [name, main] if main == "main.rs" => targets.push(target_from_path(
                RustTestTargetKind::Binary,
                name.clone(),
                source.clone(),
                root,
                false,
            )),
            _ => {}
        }
    }
    targets
}

fn default_integration_targets(root: &Path, source_paths: &[PathBuf]) -> Vec<DeclaredTarget> {
    let tests_root = root.join("tests");
    let mut targets = Vec::new();
    for source in source_paths {
        let Ok(relative) = source.strip_prefix(&tests_root) else {
            continue;
        };
        let components = path_components(relative);
        if let [file] = components.as_slice()
            && file.ends_with(".rs")
            && let Some(name) = file_stem(file)
        {
            targets.push(target_from_path(
                RustTestTargetKind::IntegrationTest,
                name,
                source.clone(),
                root,
                false,
            ));
        }
    }
    targets
}

fn target_from_path(
    kind: RustTestTargetKind,
    name: String,
    path: PathBuf,
    package_root: &Path,
    top_level: bool,
) -> DeclaredTarget {
    let parent = path.parent().unwrap_or(package_root).to_path_buf();
    DeclaredTarget {
        identity: RustTestTargetIdentity { kind, name },
        path,
        module_base: parent,
        descendant_root: None,
        top_level_source_root: top_level.then(|| package_root.join("src")),
    }
}

fn assign_descendant_roots(package_root: &Path, targets: &mut [DeclaredTarget]) {
    let shared = [
        package_root.to_path_buf(),
        package_root.join("src"),
        package_root.join("src/bin"),
        package_root.join("tests"),
    ];
    let paths = targets
        .iter()
        .map(|target| target.path.clone())
        .collect::<Vec<_>>();
    for target in targets {
        if target.top_level_source_root.is_some() {
            continue;
        }
        let Some(parent) = target.path.parent() else {
            continue;
        };
        let stem = target
            .path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default();
        let candidate = if target.path.file_name().and_then(|name| name.to_str()) == Some("main.rs")
            || target.path.file_name().and_then(|name| name.to_str()) == Some("mod.rs")
        {
            parent.to_path_buf()
        } else {
            parent.join(stem)
        };
        if shared.iter().any(|root| root == &candidate) {
            continue;
        }
        let claimed_by_other = paths.iter().any(|path| {
            path != &target.path && (path == &candidate || path.starts_with(&candidate))
        });
        if !claimed_by_other {
            target.module_base = candidate.clone();
            target.descendant_root = Some(candidate);
        }
    }
}

struct TestTraversal<'a> {
    source: &'a str,
    path: &'a Path,
    package: &'a PackageManifest,
    target: &'a TargetContext,
    file_prefix: &'a [String],
    inline_modules: Vec<String>,
    options: &'a RustTestInventoryOptions,
    subjects: &'a mut Vec<RustTestSubject>,
}

impl TestTraversal<'_> {
    fn collect(&mut self, node: Node<'_>, inherited_cfg_unknown: bool) {
        let attributes = node_attributes(node, self.source);
        let local_cfg_unknown = attributes.iter().any(|attribute| cfg_is_unknown(attribute));

        if node.kind() == "mod_item"
            && let Some(name) = node
                .child_by_field_name("name")
                .and_then(|name| node_text(self.source, name))
        {
            self.inline_modules.push(name.to_string());
            self.visit_children(node, inherited_cfg_unknown || local_cfg_unknown);
            self.inline_modules.pop();
            return;
        }

        if node.kind() == "function_item" {
            self.collect_function(node, inherited_cfg_unknown, local_cfg_unknown, attributes);
        }
        self.visit_children(node, inherited_cfg_unknown || local_cfg_unknown);
    }

    fn collect_function(
        &mut self,
        node: Node<'_>,
        inherited_cfg_unknown: bool,
        local_cfg_unknown: bool,
        attributes: Vec<String>,
    ) {
        let attribute_names = attributes
            .iter()
            .filter_map(|attribute| attribute_name(attribute))
            .collect::<Vec<_>>();
        let additional_attribute = attribute_names
            .iter()
            .find(|name| self.options.additional_test_attributes.contains(*name));
        let is_test =
            attribute_names.iter().any(|name| name == "test") || additional_attribute.is_some();
        if !is_test {
            return;
        }
        let Some(name) = node
            .child_by_field_name("name")
            .and_then(|name| node_text(self.source, name))
        else {
            return;
        };

        let start_byte = attribute_start_byte(node, self.source).unwrap_or(node.start_byte());
        let start_position =
            attribute_start_position(node, self.source).unwrap_or_else(|| node.start_position());
        let end_position = node.end_position();
        let body = self
            .source
            .get(start_byte..node.end_byte())
            .unwrap_or_default()
            .replace("\r\n", "\n");
        let mut module_path = self.file_prefix.to_vec();
        module_path.extend(self.inline_modules.iter().cloned());
        let generated_or_parameterized = additional_attribute.is_some()
            || attribute_names.iter().any(|name| {
                name == "rstest"
                    || name == "test_case"
                    || name.ends_with("::rstest")
                    || name.ends_with("::test_case")
            });
        let cfg_or_feature_unknown = inherited_cfg_unknown || local_cfg_unknown;
        let ignored = attribute_names.iter().any(|name| name == "ignore");
        let mut limitations = Vec::new();
        if self.target.source_membership_limited {
            limitations.push(
                "source-only analysis cannot prove the module is included by the target"
                    .to_string(),
            );
        }
        if generated_or_parameterized {
            limitations.push(
                "custom or parameterized test attributes do not have exact executable identities"
                    .to_string(),
            );
        }
        if cfg_or_feature_unknown {
            limitations.push(
                "cfg or feature expansion is not resolved by source-only inventory".to_string(),
            );
        }
        if ignored {
            limitations
                .push("ignored tests are not executable proof subjects by default".to_string());
        }
        limitations.sort();
        limitations.dedup();
        self.subjects.push(RustTestSubject {
            selector: RustTestSelector {
                package: self.package.name.clone(),
                target: self.target.identity.clone(),
                module_path,
                function: name.to_string(),
            },
            source_path: normalize_path(self.path),
            source_range: RustTestSourceRange {
                start_line: start_position.row as u32 + 1,
                start_column: source_column(self.source, start_position.row, start_position.column),
                end_line: end_position.row as u32 + 1,
                end_column: source_column(self.source, end_position.row, end_position.column),
            },
            body_identity: stable_hash_hex(&body),
            attributes,
            generated_or_parameterized,
            cfg_or_feature_unknown,
            ignored,
            limitations,
        });
    }

    fn visit_children(&mut self, node: Node<'_>, inherited_cfg_unknown: bool) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.collect(child, inherited_cfg_unknown);
        }
    }
}

fn node_attributes(node: Node<'_>, source: &str) -> Vec<String> {
    attribute_nodes(node, source)
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

fn file_module_prefix(path: &Path, target: &TargetContext) -> Vec<String> {
    if path == target.root_path {
        return Vec::new();
    }
    let Ok(relative) = path.strip_prefix(&target.module_base) else {
        return Vec::new();
    };
    let mut components = path_components(relative);
    let Some(file) = components.pop() else {
        return Vec::new();
    };
    if let Some(stem) = file_stem(&file)
        && !matches!(stem.as_str(), "lib" | "main" | "mod")
    {
        components.push(stem);
    }
    components
}

fn file_stem(file: &str) -> Option<String> {
    Path::new(file)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::to_string)
}

fn path_components(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str().map(str::to_string),
            _ => None,
        })
        .collect()
}

fn normalize_repo_path(path: PathBuf) -> PathBuf {
    PathBuf::from(normalize_path(&path))
}

fn strip_bom(source: String) -> String {
    source
        .strip_prefix('\u{feff}')
        .map(str::to_string)
        .unwrap_or(source)
}

fn diagnostic_order(
    left: &RustTestInventoryDiagnostic,
    right: &RustTestInventoryDiagnostic,
) -> std::cmp::Ordering {
    left.path
        .cmp(&right.path)
        .then_with(|| left.kind.cmp(&right.kind))
        .then_with(|| left.message.cmp(&right.message))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn manifest() -> Vec<(PathBuf, String)> {
        vec![(
            PathBuf::from("crates/demo/Cargo.toml"),
            "[package]\nname = \"demo-package\"\nversion = \"0.1.0\"\n".to_string(),
        )]
    }

    fn only_subject(inventory: &RustTestInventory) -> Result<&RustTestSubject, String> {
        if inventory.subjects.len() != 1 {
            return Err(format!(
                "expected one subject, found {}",
                inventory.subjects.len()
            ));
        }
        inventory
            .subjects
            .first()
            .ok_or_else(|| "expected one exact source-declared test subject".to_string())
    }

    #[test]
    fn inventories_nested_inline_unit_test() -> Result<(), String> {
        let inventory = inventory_rust_test_subjects_from_sources(
            manifest(),
            vec![(
                PathBuf::from("crates/demo/src/lib.rs"),
                "#[cfg(test)]\nmod policy_tests { #[test] fn rejects_boundary() { assert_eq!(2 + 2, 4); } }"
                    .to_string(),
            )],
            &RustTestInventoryOptions::default(),
        );
        let subject = only_subject(&inventory)?;
        assert_eq!(inventory.status, RustTestInventoryStatus::Complete);
        assert_eq!(subject.selector.package, "demo-package");
        assert_eq!(subject.selector.target.kind, RustTestTargetKind::Library);
        assert_eq!(subject.selector.target.name, "demo_package");
        assert_eq!(subject.selector.module_path, vec!["policy_tests"]);
        assert_eq!(subject.selector.function, "rejects_boundary");
        assert!(!subject.cfg_or_feature_unknown);
        Ok(())
    }

    #[test]
    fn same_named_integration_tests_have_distinct_targets() -> Result<(), String> {
        let inventory = inventory_rust_test_subjects_from_sources(
            manifest(),
            vec![
                (
                    PathBuf::from("crates/demo/tests/alpha.rs"),
                    "#[test]\nfn roundtrip() {}\n".to_string(),
                ),
                (
                    PathBuf::from("crates/demo/tests/beta.rs"),
                    "#[test]\nfn roundtrip() {}\n".to_string(),
                ),
            ],
            &RustTestInventoryOptions::default(),
        );
        let mut subjects = inventory.subjects.iter();
        let first = subjects
            .next()
            .ok_or_else(|| "expected first integration subject".to_string())?;
        let second = subjects
            .next()
            .ok_or_else(|| "expected second integration subject".to_string())?;
        assert!(subjects.next().is_none());
        assert_ne!(first.selector.target.name, second.selector.target.name);
        Ok(())
    }

    #[test]
    fn explicit_and_auto_binary_targets_with_same_identity_are_deduplicated() -> Result<(), String>
    {
        let package = PackageManifest::parse(
            Path::new("crates/demo/Cargo.toml"),
            "[package]\nname = \"demo-package\"\nversion = \"0.1.0\"\n\n[[bin]]\nname = \"demo_package\"\npath = \"src/bin/demo_package.rs\"\n",
            &[PathBuf::from("crates/demo/src/bin/demo_package.rs")],
        )
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "expected package manifest".to_string())?;

        assert_eq!(package.targets.len(), 1);
        assert_eq!(
            package.targets[0].identity,
            RustTestTargetIdentity {
                kind: RustTestTargetKind::Binary,
                name: "demo_package".to_string(),
            }
        );
        assert_eq!(
            package.targets[0].path,
            PathBuf::from("crates/demo/src/bin/demo_package.rs")
        );
        Ok(())
    }

    #[test]
    fn changed_body_changes_identity_without_changing_selector() -> Result<(), String> {
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
        let first_subject = only_subject(&first)?;
        let second_subject = only_subject(&second)?;
        assert_eq!(first_subject.selector, second_subject.selector);
        assert_ne!(first_subject.body_identity, second_subject.body_identity);
        Ok(())
    }

    #[test]
    fn cfg_limited_test_resolves_as_unknown() -> Result<(), String> {
        let inventory = inventory_rust_test_subjects_from_sources(
            manifest(),
            vec![(
                PathBuf::from("crates/demo/src/lib.rs"),
                "#[cfg(feature = \"special\")]\n#[test]\nfn gated() {}".into(),
            )],
            &RustTestInventoryOptions::default(),
        );
        let selector = only_subject(&inventory)?.selector.clone();
        assert!(matches!(
            resolve_rust_test_selector(&inventory, &selector),
            RustTestResolution::CfgOrFeatureUnknown(_)
        ));
        Ok(())
    }

    #[test]
    fn configured_parameterized_test_stays_non_exact() -> Result<(), String> {
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
        let selector = only_subject(&inventory)?.selector.clone();
        assert!(matches!(
            resolve_rust_test_selector(&inventory, &selector),
            RustTestResolution::GeneratedOrParameterized(_)
        ));
        Ok(())
    }

    #[test]
    fn ignored_test_is_not_an_executable_subject() -> Result<(), String> {
        let inventory = inventory_rust_test_subjects_from_sources(
            manifest(),
            vec![(
                PathBuf::from("crates/demo/src/lib.rs"),
                "#[test]\n#[ignore]\nfn deferred() {}".into(),
            )],
            &RustTestInventoryOptions::default(),
        );
        let selector = only_subject(&inventory)?.selector.clone();
        assert!(matches!(
            resolve_rust_test_selector(&inventory, &selector),
            RustTestResolution::Ignored(_)
        ));
        Ok(())
    }

    #[test]
    fn partial_inventory_never_resolves_exactly() -> Result<(), String> {
        let inventory = inventory_rust_test_subjects_from_sources(
            manifest(),
            vec![
                (
                    PathBuf::from("crates/demo/src/lib.rs"),
                    "#[test]\nfn exact() {}".into(),
                ),
                (
                    PathBuf::from("crates/demo/src/broken.rs"),
                    "fn broken( {".into(),
                ),
            ],
            &RustTestInventoryOptions::default(),
        );
        let selector = inventory
            .subjects
            .first()
            .ok_or_else(|| "expected discovered subject".to_string())?
            .selector
            .clone();
        assert_eq!(inventory.status, RustTestInventoryStatus::Partial);
        assert_eq!(
            resolve_rust_test_selector(&inventory, &selector),
            RustTestResolution::PartialInventory
        );
        Ok(())
    }

    #[test]
    fn workspace_only_manifest_is_ignored_without_partial_status() {
        let manifests = vec![
            (
                PathBuf::from("Cargo.toml"),
                "[workspace]\nmembers = [\"crates/demo\"]\n".to_string(),
            ),
            manifest().remove(0),
        ];
        let inventory = inventory_rust_test_subjects_from_sources(
            manifests,
            vec![(
                PathBuf::from("crates/demo/src/lib.rs"),
                "#[test]\nfn exact() {}".into(),
            )],
            &RustTestInventoryOptions::default(),
        );
        assert_eq!(inventory.status, RustTestInventoryStatus::Complete);
        assert_eq!(inventory.subjects.len(), 1);
    }

    #[test]
    fn bom_prefixed_source_is_parsed_normally() {
        let inventory = inventory_rust_test_subjects_from_sources(
            manifest(),
            vec![(
                PathBuf::from("crates/demo/src/lib.rs"),
                "\u{feff}#[test]\nfn exact() {}".into(),
            )],
            &RustTestInventoryOptions::default(),
        );
        assert_eq!(inventory.status, RustTestInventoryStatus::Complete);
        assert_eq!(inventory.subjects.len(), 1);
    }

    #[test]
    fn binary_only_helper_is_not_fabricated_as_library() -> Result<(), String> {
        let binary_manifest = vec![(
            PathBuf::from("crates/demo/Cargo.toml"),
            "[package]\nname = \"demo-package\"\nversion = \"0.1.0\"\n".to_string(),
        )];
        let inventory = inventory_rust_test_subjects_from_sources(
            binary_manifest,
            vec![
                (
                    PathBuf::from("crates/demo/src/main.rs"),
                    "mod helper; fn main() {}".into(),
                ),
                (
                    PathBuf::from("crates/demo/src/helper.rs"),
                    "#[test]\nfn helper_test() {}".into(),
                ),
            ],
            &RustTestInventoryOptions::default(),
        );
        let subject = only_subject(&inventory)?;
        assert_eq!(subject.selector.target.kind, RustTestTargetKind::Binary);
        assert!(
            subject
                .limitations
                .iter()
                .any(|limitation| { limitation.contains("cannot prove the module is included") })
        );
        Ok(())
    }

    #[test]
    fn shared_src_parent_is_not_claimed_when_lib_and_bin_both_exist() {
        let inventory = inventory_rust_test_subjects_from_sources(
            manifest(),
            vec![
                (
                    PathBuf::from("crates/demo/src/lib.rs"),
                    "pub mod helper;".into(),
                ),
                (
                    PathBuf::from("crates/demo/src/main.rs"),
                    "mod helper; fn main() {}".into(),
                ),
                (
                    PathBuf::from("crates/demo/src/helper.rs"),
                    "#[test]\nfn ambiguous_owner() {}".into(),
                ),
            ],
            &RustTestInventoryOptions::default(),
        );
        assert!(inventory.subjects.is_empty());
        assert_eq!(inventory.status, RustTestInventoryStatus::Partial);
    }

    #[test]
    fn integration_target_does_not_claim_tests_common_module() {
        let inventory = inventory_rust_test_subjects_from_sources(
            manifest(),
            vec![
                (
                    PathBuf::from("crates/demo/tests/alpha.rs"),
                    "mod common; #[test] fn alpha() {}".into(),
                ),
                (
                    PathBuf::from("crates/demo/tests/common/mod.rs"),
                    "#[test]\nfn helper_only() {}".into(),
                ),
            ],
            &RustTestInventoryOptions::default(),
        );
        assert_eq!(inventory.subjects.len(), 1);
        assert_eq!(inventory.status, RustTestInventoryStatus::Partial);
    }

    #[test]
    fn split_library_module_has_stable_module_path() -> Result<(), String> {
        let inventory = inventory_rust_test_subjects_from_sources(
            manifest(),
            vec![
                (
                    PathBuf::from("crates/demo/src/lib.rs"),
                    "mod policy;".into(),
                ),
                (
                    PathBuf::from("crates/demo/src/policy.rs"),
                    "#[test]\nfn rejects_boundary() {}".into(),
                ),
            ],
            &RustTestInventoryOptions::default(),
        );
        let subject = only_subject(&inventory)?;
        assert_eq!(subject.selector.module_path, vec!["policy"]);
        Ok(())
    }

    #[test]
    fn duplicate_function_names_in_modules_have_distinct_selectors() {
        let inventory = inventory_rust_test_subjects_from_sources(
            manifest(),
            vec![(
                PathBuf::from("crates/demo/src/lib.rs"),
                "mod alpha { #[test] fn same() {} } mod beta { #[test] fn same() {} }".into(),
            )],
            &RustTestInventoryOptions::default(),
        );
        let module_paths = inventory
            .subjects
            .iter()
            .map(|subject| subject.selector.module_path.clone())
            .collect::<BTreeSet<_>>();
        assert_eq!(module_paths.len(), 2);
    }

    #[test]
    fn explicit_integration_target_keeps_declared_identity() -> Result<(), String> {
        let manifests = vec![(
            PathBuf::from("crates/demo/Cargo.toml"),
            "[package]\nname = \"demo-package\"\nversion = \"0.1.0\"\nautotests = false\n\n[[test]]\nname = \"contract\"\npath = \"tests/custom_case.rs\"\n"
                .to_string(),
        )];
        let inventory = inventory_rust_test_subjects_from_sources(
            manifests,
            vec![(
                PathBuf::from("crates/demo/tests/custom_case.rs"),
                "#[test]\nfn exact() {}".into(),
            )],
            &RustTestInventoryOptions::default(),
        );
        let subject = only_subject(&inventory)?;
        assert_eq!(
            subject.selector.target.kind,
            RustTestTargetKind::IntegrationTest
        );
        assert_eq!(subject.selector.target.name, "contract");
        Ok(())
    }

    #[test]
    fn malformed_manifest_keeps_inventory_partial() {
        let inventory = inventory_rust_test_subjects_from_sources(
            vec![(PathBuf::from("crates/demo/Cargo.toml"), "[package".into())],
            vec![(
                PathBuf::from("crates/demo/src/lib.rs"),
                "#[test]\nfn exact() {}".into(),
            )],
            &RustTestInventoryOptions::default(),
        );
        assert_eq!(inventory.status, RustTestInventoryStatus::Partial);
        assert!(inventory.subjects.is_empty());
    }

    #[test]
    fn windows_style_repo_paths_are_normalized() -> Result<(), String> {
        let inventory = inventory_rust_test_subjects_from_sources(
            vec![(
                PathBuf::from("crates\\demo\\Cargo.toml"),
                "[package]\nname = \"demo-package\"\nversion = \"0.1.0\"\n".into(),
            )],
            vec![(
                PathBuf::from("crates\\demo\\src\\lib.rs"),
                "#[test]\nfn exact() {}".into(),
            )],
            &RustTestInventoryOptions::default(),
        );
        let subject = only_subject(&inventory)?;
        assert_eq!(subject.source_path, "crates/demo/src/lib.rs");
        Ok(())
    }

    #[test]
    fn missing_selector_is_not_guessed() -> Result<(), String> {
        let inventory = inventory_rust_test_subjects_from_sources(
            manifest(),
            vec![(
                PathBuf::from("crates/demo/src/lib.rs"),
                "#[test]\nfn exact_name() {}".into(),
            )],
            &RustTestInventoryOptions::default(),
        );
        let mut selector = only_subject(&inventory)?.selector.clone();
        selector.function = "nearby_name".into();
        assert_eq!(
            resolve_rust_test_selector(&inventory, &selector),
            RustTestResolution::NotFound
        );
        Ok(())
    }
}
