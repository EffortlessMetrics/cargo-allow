use std::collections::BTreeMap;
use std::path::Path;

use crate::import_roots::config::{ImportEdgeKind, ImportProvenance};
use crate::import_roots::discover::ImportEdge;

pub(crate) fn repo_relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

pub(crate) fn parse_front_matter(text: &str) -> Option<BTreeMap<String, String>> {
    let mut lines = text.lines();
    if lines.next()? != "---" {
        return None;
    }
    let mut fields = BTreeMap::new();
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

pub(crate) fn collect_reference_edges(source_id: &str, text: &str, edges: &mut Vec<ImportEdge>) {
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
