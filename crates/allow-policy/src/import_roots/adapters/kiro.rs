use std::fs;
use std::path::Path;

use crate::import_roots::config::{
    ImportConfidence, ImportEdgeKind, ImportNodeRole, ImportProvenance, ImportRootEntry,
};
use crate::import_roots::discover::{ImportEdge, ImportNode};
use crate::import_roots::validate::{ImportDiagnostic, ImportDiagnosticKind};

use super::shared::{collect_reference_edges, repo_relative_path};

pub const KIRO_ECOSYSTEM: &str = "kiro";

const KIRO_ARTIFACT_FILES: &[&str] = &["requirements.md", "bugfix.md", "design.md", "tasks.md"];

/// Returns true when `path` names the Kiro import root (`.kiro/`).
pub fn is_kiro_root(path: &str) -> bool {
    let normalized = path.trim().trim_start_matches("./").trim_end_matches('/');
    normalized == ".kiro"
}

/// Discover Kiro spec artifacts under `.kiro/` (requirements|bugfix, design, tasks).
pub fn discover_kiro_root(
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
            if !KIRO_ARTIFACT_FILES.contains(&file_name) {
                continue;
            }
            let relative = repo_relative_path(root, &child_path);
            let node_id = kiro_node_id(entry, &relative);
            let role = kiro_node_role(file_name);
            let confidence = kiro_node_confidence(file_name);
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
            match fs::read_to_string(&child_path) {
                Ok(text) => collect_reference_edges(&node_id, &text, edges),
                Err(_) => diagnostics.push(ImportDiagnostic {
                    kind: ImportDiagnosticKind::BrokenEdge,
                    message: format!("failed to read discovered Kiro import node `{relative}`"),
                    root_ids: vec![entry.id.clone(), node_id],
                }),
            }
        }
    }
}

fn kiro_node_id(entry: &ImportRootEntry, relative_path: &str) -> String {
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

fn kiro_node_role(file_name: &str) -> ImportNodeRole {
    match file_name {
        "requirements.md" | "bugfix.md" | "design.md" => ImportNodeRole::Imported,
        "tasks.md" => ImportNodeRole::Generated,
        _ => ImportNodeRole::Generated,
    }
}

fn kiro_node_confidence(file_name: &str) -> ImportConfidence {
    match file_name {
        "requirements.md" | "bugfix.md" | "design.md" => ImportConfidence::High,
        "tasks.md" => ImportConfidence::Medium,
        _ => ImportConfidence::Low,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_kiro_root_matches_layout() {
        assert!(is_kiro_root(".kiro"));
        assert!(is_kiro_root("./.kiro/"));
        assert!(!is_kiro_root(".specify"));
    }
}
