use std::fs;
use std::path::Path;

use allow_core::read_text_file_capped;

use crate::import_roots::config::{
    ImportConfidence, ImportEdgeKind, ImportNodeRole, ImportProvenance, ImportRootEntry,
};
use crate::import_roots::discover::{ImportEdge, ImportNode};
use crate::import_roots::validate::{ImportDiagnostic, ImportDiagnosticKind};

use super::shared::{collect_reference_edges, repo_relative_path};

pub const SPEC_KIT_ECOSYSTEM: &str = "spec-kit";

const SPEC_KIT_FEATURE_FILES: &[&str] = &["spec.md", "plan.md", "tasks.md"];

/// Returns true when `path` names the Spec Kit import root (`.specify/`).
pub fn is_spec_kit_root(path: &str) -> bool {
    let normalized = path.trim().trim_start_matches("./").trim_end_matches('/');
    normalized == ".specify"
}

/// Discover Spec Kit artifacts under `.specify/` (constitution, spec, plan, tasks, templates).
pub fn discover_spec_kit_root(
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
            if !is_spec_kit_artifact(&relative, file_name) {
                continue;
            }
            let node_id = spec_kit_node_id(entry, &relative);
            let role = spec_kit_node_role(&relative, file_name);
            let confidence = spec_kit_node_confidence(&relative, file_name);
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
                Ok(text) => collect_reference_edges(&node_id, &text, edges),
                Err(err) => diagnostics.push(ImportDiagnostic {
                    kind: ImportDiagnosticKind::BrokenEdge,
                    message: format!(
                        "failed to read discovered Spec Kit import node `{relative}`: {err}"
                    ),
                    root_ids: vec![entry.id.clone(), node_id],
                }),
            }
        }
    }
}

fn is_spec_kit_artifact(relative_path: &str, file_name: &str) -> bool {
    if file_name == "constitution.md" {
        return true;
    }
    if SPEC_KIT_FEATURE_FILES.contains(&file_name) && !relative_path.contains("/templates/") {
        return true;
    }
    relative_path.contains("/templates/") && file_name.ends_with(".md")
}

fn spec_kit_node_id(entry: &ImportRootEntry, relative_path: &str) -> String {
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

fn spec_kit_node_role(relative_path: &str, file_name: &str) -> ImportNodeRole {
    if relative_path.contains("/templates/") {
        return ImportNodeRole::Generated;
    }
    match file_name {
        "constitution.md" | "spec.md" | "plan.md" => ImportNodeRole::Imported,
        "tasks.md" => ImportNodeRole::Generated,
        _ => ImportNodeRole::Generated,
    }
}

fn spec_kit_node_confidence(relative_path: &str, file_name: &str) -> ImportConfidence {
    if relative_path.contains("/templates/") {
        return ImportConfidence::Medium;
    }
    match file_name {
        "constitution.md" | "spec.md" => ImportConfidence::High,
        "plan.md" | "tasks.md" => ImportConfidence::Medium,
        _ => ImportConfidence::Low,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_spec_kit_root_matches_layout() {
        assert!(is_spec_kit_root(".specify"));
        assert!(is_spec_kit_root("./.specify/"));
        assert!(!is_spec_kit_root(".kiro"));
    }

    #[test]
    fn is_spec_kit_artifact_classifies_known_files() {
        assert!(is_spec_kit_artifact(
            ".specify/memory/constitution.md",
            "constitution.md"
        ));
        assert!(is_spec_kit_artifact(
            ".specify/specs/001-auth/spec.md",
            "spec.md"
        ));
        assert!(is_spec_kit_artifact(
            ".specify/templates/spec-template.md",
            "spec-template.md"
        ));
        assert!(!is_spec_kit_artifact(
            ".specify/scripts/bash/common.sh",
            "common.sh"
        ));
    }
}
