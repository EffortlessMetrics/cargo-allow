use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::import_roots::config::{
    ImportConfidence, ImportEdgeKind, ImportNodeRole, ImportProvenance, ImportRootEntry,
};
use crate::import_roots::discover::{ImportEdge, ImportNode};
use crate::import_roots::validate::{ImportDiagnostic, ImportDiagnosticKind};

use super::shared::repo_relative_path;

pub const XTASK_ECOSYSTEM: &str = "xtask";

const REGISTRY_FILE_NAMES: &[&str] = &["commands.toml", "command-registry.toml", "registry.toml"];

/// Returns true when `path` names the xtask import root (`xtask/`).
pub fn is_xtask_root(path: &str) -> bool {
    let normalized = path.trim().trim_start_matches("./").trim_end_matches('/');
    normalized == "xtask"
}

/// Discover xtask command registry TOML files under `xtask/` without Rust dispatch parsing.
pub fn discover_xtask_root(
    root: &Path,
    entry: &ImportRootEntry,
    directory: &Path,
    nodes: &mut Vec<ImportNode>,
    edges: &mut Vec<ImportEdge>,
    diagnostics: &mut Vec<ImportDiagnostic>,
) {
    let mut registry_files = Vec::new();
    collect_registry_files(directory, &mut registry_files);
    registry_files.sort();
    registry_files.dedup();

    for registry_path in registry_files {
        discover_registry_file(
            root,
            entry,
            &registry_path,
            entry.id.clone(),
            nodes,
            edges,
            diagnostics,
        );
    }
}

fn collect_registry_files(directory: &Path, out: &mut Vec<PathBuf>) {
    let Ok(read_dir) = fs::read_dir(directory) else {
        return;
    };
    for child in read_dir.flatten() {
        let child_path = child.path();
        if child_path.is_dir() {
            collect_registry_files(&child_path, out);
            continue;
        }
        if !child_path.is_file() {
            continue;
        }
        let Some(file_name) = child_path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if REGISTRY_FILE_NAMES.contains(&file_name) {
            out.push(child_path);
        }
    }
}

fn discover_registry_file(
    root: &Path,
    entry: &ImportRootEntry,
    registry_path: &Path,
    parent_id: String,
    nodes: &mut Vec<ImportNode>,
    edges: &mut Vec<ImportEdge>,
    diagnostics: &mut Vec<ImportDiagnostic>,
) {
    let relative = repo_relative_path(root, registry_path);
    let registry_node_id = xtask_node_id(entry, &relative);
    nodes.push(ImportNode {
        id: registry_node_id.clone(),
        path: relative.clone(),
        role: ImportNodeRole::Imported,
        ecosystem: entry.ecosystem.clone(),
        provenance: ImportProvenance::Discovered,
        confidence: ImportConfidence::High,
    });
    edges.push(ImportEdge {
        source_id: parent_id.clone(),
        target_id: registry_node_id.clone(),
        kind: ImportEdgeKind::Contains,
        provenance: ImportProvenance::Discovered,
    });

    let text = match fs::read_to_string(registry_path) {
        Ok(text) => text,
        Err(_) => {
            diagnostics.push(ImportDiagnostic {
                kind: ImportDiagnosticKind::BrokenEdge,
                message: format!("failed to read xtask command registry `{relative}`"),
                root_ids: vec![entry.id.clone(), registry_node_id],
            });
            return;
        }
    };

    match parse_command_registry(&text) {
        Ok(commands) => {
            for command in commands {
                let Some(command_key) = command_key(&command) else {
                    continue;
                };
                let command_node_id = format!("{registry_node_id}:{command_key}");
                nodes.push(ImportNode {
                    id: command_node_id.clone(),
                    path: relative.clone(),
                    role: ImportNodeRole::Generated,
                    ecosystem: entry.ecosystem.clone(),
                    provenance: ImportProvenance::Discovered,
                    confidence: ImportConfidence::Medium,
                });
                edges.push(ImportEdge {
                    source_id: registry_node_id.clone(),
                    target_id: command_node_id.clone(),
                    kind: ImportEdgeKind::Contains,
                    provenance: ImportProvenance::Discovered,
                });
                collect_command_reference_edges(&command_node_id, &command, edges);
            }
        }
        Err(message) => diagnostics.push(ImportDiagnostic {
            kind: ImportDiagnosticKind::BrokenEdge,
            message: format!("failed to parse xtask command registry `{relative}`: {message}"),
            root_ids: vec![entry.id.clone(), registry_node_id],
        }),
    }
}

fn parse_command_registry(text: &str) -> Result<Vec<BTreeMap<String, String>>, String> {
    let value =
        toml::from_str::<toml::Value>(text).map_err(|err| format!("invalid TOML: {err}"))?;
    let Some(commands) = value.get("commands").and_then(toml::Value::as_array) else {
        return Ok(Vec::new());
    };
    let mut parsed = Vec::new();
    for command in commands {
        let Some(table) = command.as_table() else {
            continue;
        };
        let mut fields = BTreeMap::new();
        for (key, value) in table {
            if let Some(string_value) = value_as_string(value) {
                if !string_value.is_empty() {
                    fields.insert(key.clone(), string_value);
                }
            }
        }
        if !fields.is_empty() {
            parsed.push(fields);
        }
    }
    Ok(parsed)
}

fn value_as_string(value: &toml::Value) -> Option<String> {
    match value {
        toml::Value::String(text) => Some(text.clone()),
        toml::Value::Integer(number) => Some(number.to_string()),
        toml::Value::Boolean(flag) => Some(flag.to_string()),
        _ => None,
    }
}

fn command_key(command: &BTreeMap<String, String>) -> Option<String> {
    for key in ["name", "id", "command"] {
        if let Some(value) = command.get(key) {
            if !value.contains(' ') {
                return Some(value.clone());
            }
        }
    }
    None
}

fn collect_command_reference_edges(
    source_id: &str,
    command: &BTreeMap<String, String>,
    edges: &mut Vec<ImportEdge>,
) {
    if let Some(id) = command.get("id") {
        if !id.is_empty() && !id.contains(' ') {
            edges.push(ImportEdge {
                source_id: source_id.to_string(),
                target_id: id.clone(),
                kind: ImportEdgeKind::References,
                provenance: ImportProvenance::GeneratedMarker,
            });
        }
    }
    for (key, value) in command {
        if key.starts_with("linked_") && !value.is_empty() {
            edges.push(ImportEdge {
                source_id: source_id.to_string(),
                target_id: value.clone(),
                kind: ImportEdgeKind::References,
                provenance: ImportProvenance::Discovered,
            });
        }
    }
}

fn xtask_node_id(entry: &ImportRootEntry, relative_path: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_xtask_root_matches_layout() {
        assert!(is_xtask_root("xtask"));
        assert!(is_xtask_root("./xtask/"));
        assert!(!is_xtask_root(".kiro"));
    }

    #[test]
    fn parse_command_registry_reads_command_tables() {
        let text = r#"
            [[commands]]
            id = "FIXTURE-XTASK-CMD-001"
            name = "check-file-policy"
            linked_spec = "CARGO-ALLOW-SPEC-0002"
        "#;
        let commands = parse_command_registry(text).expect("registry");
        assert_eq!(commands.len(), 1);
        assert_eq!(
            commands[0].get("name"),
            Some(&"check-file-policy".to_string())
        );
        assert_eq!(
            commands[0].get("linked_spec"),
            Some(&"CARGO-ALLOW-SPEC-0002".to_string())
        );
    }

    #[test]
    fn command_key_prefers_name_over_id() {
        let mut command = BTreeMap::new();
        command.insert("id".to_string(), "FIXTURE-XTASK-CMD-001".to_string());
        command.insert("name".to_string(), "check-generated".to_string());
        assert_eq!(command_key(&command), Some("check-generated".to_string()));
    }
}
