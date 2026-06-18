use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use super::adapters::{
    discover_auto_repo_spec_roots, discover_generic_spec_root, is_generic_spec_root,
    GENERIC_SPEC_ECOSYSTEM,
};
use super::config::{
    ImportConfidence, ImportEdgeKind, ImportNodeRole, ImportProvenance, ImportRootEntry,
    ImportRootsConfig,
};
use super::validate::{ImportDiagnostic, ImportDiagnosticKind, ValidatedImportRootsConfig};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportNode {
    pub id: String,
    pub path: String,
    pub role: ImportNodeRole,
    pub ecosystem: String,
    pub provenance: ImportProvenance,
    pub confidence: ImportConfidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportEdge {
    pub source_id: String,
    pub target_id: String,
    pub kind: ImportEdgeKind,
    pub provenance: ImportProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportGraph {
    pub nodes: Vec<ImportNode>,
    pub edges: Vec<ImportEdge>,
    pub diagnostics: Vec<ImportDiagnostic>,
}

pub fn discover_import_graph(root: &Path, validated: &ValidatedImportRootsConfig) -> ImportGraph {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut diagnostics = validated.diagnostics.clone();

    let configured_paths = validated
        .config
        .entries
        .iter()
        .map(|entry| entry.path.clone())
        .collect::<BTreeSet<_>>();
    let auto_roots = discover_auto_repo_spec_roots(root, &configured_paths);
    let entries = validated
        .config
        .entries
        .iter()
        .chain(auto_roots.iter())
        .collect::<Vec<_>>();

    for entry in entries {
        let absolute = root.join(&entry.path);
        let exists = absolute.exists();
        nodes.push(root_node(entry, auto_roots.iter().any(|auto| auto.id == entry.id)));
        if !exists {
            diagnostics.push(ImportDiagnostic {
                kind: ImportDiagnosticKind::MissingRoot,
                message: format!(
                    "import root `{}` ({}) is not present under the source tree",
                    entry.id, entry.path
                ),
                root_ids: vec![entry.id.clone()],
            });
            continue;
        }
        if absolute.is_dir() {
            if is_generic_spec_root(&entry.path) || entry.ecosystem == GENERIC_SPEC_ECOSYSTEM {
                discover_generic_spec_root(
                    root,
                    entry,
                    &absolute,
                    &mut nodes,
                    &mut edges,
                    &mut diagnostics,
                );
            } else {
                discover_directory_children(
                    root,
                    entry,
                    &absolute,
                    &mut nodes,
                    &mut edges,
                    &mut diagnostics,
                );
            }
        }
    }

    validate_edge_targets(&nodes, &edges, &mut diagnostics);
    ImportGraph {
        nodes,
        edges,
        diagnostics,
    }
}

fn root_node(entry: &ImportRootEntry, auto_discovered: bool) -> ImportNode {
    ImportNode {
        id: entry.id.clone(),
        path: entry.path.clone(),
        role: entry.role,
        ecosystem: entry.ecosystem.clone(),
        provenance: if auto_discovered {
            ImportProvenance::Discovered
        } else {
            ImportProvenance::Configured
        },
        confidence: if auto_discovered {
            ImportConfidence::Medium
        } else {
            ImportConfidence::High
        },
    }
}

fn discover_directory_children(
    root: &Path,
    entry: &ImportRootEntry,
    directory: &Path,
    nodes: &mut Vec<ImportNode>,
    edges: &mut Vec<ImportEdge>,
    diagnostics: &mut Vec<ImportDiagnostic>,
) {
    let Ok(read_dir) = fs::read_dir(directory) else {
        return;
    };
    for child in read_dir.flatten() {
        let child_path = child.path();
        if !child_path.is_file() {
            continue;
        }
        let Some(file_name) = child_path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !file_name.ends_with(".md") {
            continue;
        }
        let relative = repo_relative_path(root, &child_path);
        let child_id = format!("{}:{}", entry.id, file_name);
        let role = if file_name == "README.md" {
            entry.role
        } else {
            ImportNodeRole::Generated
        };
        nodes.push(ImportNode {
            id: child_id.clone(),
            path: relative.clone(),
            role,
            ecosystem: entry.ecosystem.clone(),
            provenance: ImportProvenance::Discovered,
            confidence: ImportConfidence::Medium,
        });
        edges.push(ImportEdge {
            source_id: entry.id.clone(),
            target_id: child_id.clone(),
            kind: ImportEdgeKind::Contains,
            provenance: ImportProvenance::Discovered,
        });
        if let Ok(text) = fs::read_to_string(&child_path) {
            collect_reference_edges(&child_id, &text, edges);
        } else {
            diagnostics.push(ImportDiagnostic {
                kind: ImportDiagnosticKind::BrokenEdge,
                message: format!("failed to read discovered import node `{relative}`"),
                root_ids: vec![entry.id.clone(), child_id],
            });
        }
    }
}

fn collect_reference_edges(source_id: &str, text: &str, edges: &mut Vec<ImportEdge>) {
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(target) = trimmed
            .strip_prefix("linked_")
            .and_then(|rest| rest.split_once('='))
            .map(|(_, value)| value.trim().trim_matches('"').trim_matches('\''))
        {
            if !target.is_empty() {
                edges.push(ImportEdge {
                    source_id: source_id.to_string(),
                    target_id: target.to_string(),
                    kind: ImportEdgeKind::References,
                    provenance: ImportProvenance::Discovered,
                });
            }
        }
        if let Some(target) = trimmed
            .strip_prefix("id:")
            .or_else(|| trimmed.strip_prefix("id ="))
            .map(str::trim)
            .map(|value| value.trim_matches('"').trim_matches('\''))
        {
            if !target.is_empty() && !target.contains(' ') {
                edges.push(ImportEdge {
                    source_id: source_id.to_string(),
                    target_id: target.to_string(),
                    kind: ImportEdgeKind::References,
                    provenance: ImportProvenance::GeneratedMarker,
                });
            }
        }
    }
}

fn validate_edge_targets(
    nodes: &[ImportNode],
    edges: &[ImportEdge],
    diagnostics: &mut Vec<ImportDiagnostic>,
) {
    let known = nodes
        .iter()
        .map(|node| node.id.as_str())
        .chain(nodes.iter().map(|node| node.path.as_str()))
        .collect::<std::collections::BTreeSet<_>>();
    for edge in edges.iter() {
        if edge.kind == ImportEdgeKind::Contains {
            continue;
        }
        if !known.contains(edge.target_id.as_str()) {
            diagnostics.push(ImportDiagnostic {
                kind: ImportDiagnosticKind::BrokenEdge,
                message: format!(
                    "import edge from `{}` references unknown target `{}`",
                    edge.source_id, edge.target_id
                ),
                root_ids: vec![edge.source_id.clone()],
            });
        }
    }
}

fn repo_relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

pub fn resolve_import_roots_config(config: Option<&ImportRootsConfig>) -> ImportRootsConfig {
    config
        .cloned()
        .filter(|cfg| !cfg.entries.is_empty() || cfg.owned.is_some())
        .unwrap_or_else(super::config::default_import_roots_config)
}
