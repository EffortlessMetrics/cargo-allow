use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use allow_core::read_text_file_capped;

use crate::import_roots::config::{
    ImportConfidence, ImportEdgeKind, ImportNodeRole, ImportProvenance, ImportRootEntry,
};
use crate::import_roots::discover::{ImportEdge, ImportNode};
use crate::import_roots::validate::{ImportDiagnostic, ImportDiagnosticKind};

pub const GENERIC_SPEC_ECOSYSTEM: &str = "generic-spec";

/// Returns true when `path` names a generic spec root (`.spec/`, `.rails/`, or `.<repo>-spec/`).
pub fn is_generic_spec_root(path: &str) -> bool {
    let normalized = path.trim().trim_start_matches("./").trim_end_matches('/');
    normalized == ".spec"
        || normalized == ".rails"
        || (normalized.starts_with('.') && normalized.ends_with("-spec"))
}

/// Scan the repository root for `.<name>-spec/` directories not already configured.
pub fn discover_auto_repo_spec_roots(
    root: &Path,
    configured_paths: &BTreeSet<String>,
) -> Vec<ImportRootEntry> {
    let Ok(read_dir) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    for child in read_dir.flatten() {
        let child_path = child.path();
        if !child_path.is_dir() {
            continue;
        }
        let Some(name) = child_path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.starts_with('.') || !name.ends_with("-spec") {
            continue;
        }
        let rel = repo_relative_path(root, &child_path);
        if configured_paths.contains(&rel) {
            continue;
        }
        entries.push(ImportRootEntry {
            id: format!("auto-{name}"),
            path: rel,
            ecosystem: GENERIC_SPEC_ECOSYSTEM.to_string(),
            role: ImportNodeRole::Imported,
        });
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    entries
}

/// Discover markdown artifacts under a generic spec root recursively.
pub fn discover_generic_spec_root(
    root: &Path,
    entry: &ImportRootEntry,
    directory: &Path,
    nodes: &mut Vec<ImportNode>,
    edges: &mut Vec<ImportEdge>,
    diagnostics: &mut Vec<ImportDiagnostic>,
) {
    let mut stack = vec![(directory.to_path_buf(), entry.id.clone())];
    while let Some((current_dir, parent_id)) = stack.pop() {
        let Ok(read_dir) = fs::read_dir(&current_dir) else {
            continue;
        };
        for child in read_dir.flatten() {
            let child_path = child.path();
            if child_path.is_dir() {
                stack.push((child_path, parent_id.clone()));
                continue;
            }
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
            let node_id = generic_node_id(entry, &relative);
            let role = generic_node_role(entry, file_name, &relative);
            let confidence = generic_node_confidence(&relative, file_name);
            nodes.push(ImportNode {
                id: node_id.clone(),
                path: relative.clone(),
                role,
                ecosystem: entry.ecosystem.clone(),
                provenance: ImportProvenance::Discovered,
                confidence,
            });
            edges.push(ImportEdge {
                source_id: parent_id.clone(),
                target_id: node_id.clone(),
                kind: ImportEdgeKind::Contains,
                provenance: ImportProvenance::Discovered,
            });
            match read_text_file_capped(&child_path) {
                Ok(text) => collect_generic_reference_edges(&node_id, &text, edges),
                Err(err) => diagnostics.push(ImportDiagnostic {
                    kind: ImportDiagnosticKind::BrokenEdge,
                    message: format!("failed to read discovered import node `{relative}`: {err}"),
                    root_ids: vec![entry.id.clone(), node_id],
                }),
            }
        }
    }
}

fn generic_node_id(entry: &ImportRootEntry, relative_path: &str) -> String {
    let suffix = relative_path
        .strip_prefix(&entry.path)
        .unwrap_or(relative_path)
        .trim_start_matches('/')
        .replace('/', ":");
    if suffix.is_empty() {
        entry.id.clone()
    } else {
        format!("{}:{suffix}", entry.id)
    }
}

fn generic_node_role(
    entry: &ImportRootEntry,
    file_name: &str,
    relative_path: &str,
) -> ImportNodeRole {
    if file_name == "README.md" && relative_path == format!("{}/README.md", entry.path) {
        return entry.role;
    }
    if relative_path.contains("/specs/") || relative_path.ends_with("/spec.md") {
        return ImportNodeRole::Imported;
    }
    if relative_path.contains("/plans/") || relative_path.ends_with("/plan.md") {
        return ImportNodeRole::Imported;
    }
    ImportNodeRole::Generated
}

fn generic_node_confidence(relative_path: &str, file_name: &str) -> ImportConfidence {
    if file_name == "README.md" {
        return ImportConfidence::High;
    }
    if relative_path.contains("/specs/")
        || relative_path.contains("/plans/")
        || relative_path.ends_with("/spec.md")
        || relative_path.ends_with("/plan.md")
    {
        return ImportConfidence::Medium;
    }
    ImportConfidence::Low
}

fn collect_generic_reference_edges(source_id: &str, text: &str, edges: &mut Vec<ImportEdge>) {
    if let Some(front_matter) = parse_front_matter(text) {
        if let Some(id) = front_matter.get("id") {
            if !id.is_empty() && !id.contains(' ') {
                edges.push(ImportEdge {
                    source_id: source_id.to_string(),
                    target_id: id.clone(),
                    kind: ImportEdgeKind::References,
                    provenance: ImportProvenance::GeneratedMarker,
                });
            }
        }
        for (key, value) in front_matter {
            if key.starts_with("linked_") && !value.is_empty() {
                edges.push(ImportEdge {
                    source_id: source_id.to_string(),
                    target_id: value,
                    kind: ImportEdgeKind::References,
                    provenance: ImportProvenance::Discovered,
                });
            }
        }
        return;
    }
    collect_body_reference_edges(source_id, text, edges);
}

fn collect_body_reference_edges(source_id: &str, text: &str, edges: &mut Vec<ImportEdge>) {
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
    }
}

fn parse_front_matter(text: &str) -> Option<std::collections::BTreeMap<String, String>> {
    let mut lines = text.lines();
    if lines.next()? != "---" {
        return None;
    }
    let mut fields = std::collections::BTreeMap::new();
    for line in lines.by_ref() {
        if line.trim() == "---" {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        let key = key.trim().to_string();
        let value = value
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .to_string();
        if !key.is_empty() {
            fields.insert(key, value);
        }
    }
    Some(fields)
}

fn repo_relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_generic_spec_root_matches_known_layouts() {
        assert!(is_generic_spec_root(".spec"));
        assert!(is_generic_spec_root(".rails"));
        assert!(is_generic_spec_root(".cargo-allow-spec"));
        assert!(!is_generic_spec_root(".kiro"));
        assert!(!is_generic_spec_root(".specify"));
    }

    #[test]
    fn parse_front_matter_extracts_id_and_links() {
        let text = "---\nid: EXAMPLE-SPEC\nlinked_plan: PLAN-001\n---\n\nbody\n";
        let fields = parse_front_matter(text).expect("front matter");
        assert_eq!(fields.get("id"), Some(&"EXAMPLE-SPEC".to_string()));
        assert_eq!(fields.get("linked_plan"), Some(&"PLAN-001".to_string()));
    }
}
