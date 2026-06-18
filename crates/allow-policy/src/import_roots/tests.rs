use std::io;

use super::config::default_import_roots_config;
use super::*;

#[test]
fn parse_import_roots_config_reads_entries() {
    let config = parse_import_roots_config(
        r#"
            owned = ".allow/imports"

            [[entries]]
            id = "kiro"
            path = ".kiro"
            ecosystem = "kiro"
            role = "imported"
        "#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("parse import roots: {err}")));
    assert_eq!(config.owned.as_deref(), Some(".allow/imports"));
    assert_eq!(config.entries.len(), 1);
    assert_eq!(config.entries[0].id, "kiro");
    assert_eq!(config.entries[0].role, ImportNodeRole::Imported);
}

#[test]
fn validate_import_roots_config_rejects_duplicate_ids() {
    let config = ImportRootsConfig {
        owned: None,
        entries: vec![
            ImportRootEntry {
                id: "dup".to_string(),
                path: ".kiro".to_string(),
                ecosystem: "kiro".to_string(),
                role: ImportNodeRole::Imported,
            },
            ImportRootEntry {
                id: "dup".to_string(),
                path: ".specify".to_string(),
                ecosystem: "spec-kit".to_string(),
                role: ImportNodeRole::Imported,
            },
        ],
    };
    let validated = validate_import_roots_config(config);
    assert!(!validated.valid);
    assert!(
        validated
            .diagnostics
            .iter()
            .any(|diag| diag.kind == ImportDiagnosticKind::DuplicateRootId)
    );
}

#[test]
fn discover_import_graph_reports_missing_roots() -> io::Result<()> {
    let root = fixture_root("missing-root")?;
    let validated = validate_import_roots_config(ImportRootsConfig {
        owned: None,
        entries: vec![ImportRootEntry {
            id: "kiro".to_string(),
            path: ".kiro".to_string(),
            ecosystem: "kiro".to_string(),
            role: ImportNodeRole::Imported,
        }],
    });
    let graph = discover_import_graph(&root, &validated);
    assert_eq!(graph.nodes.len(), 1);
    assert!(
        graph
            .diagnostics
            .iter()
            .any(|diag| diag.kind == ImportDiagnosticKind::MissingRoot)
    );
    Ok(())
}

#[test]
fn discover_import_graph_normalizes_owned_import_markdown() -> io::Result<()> {
    let root = fixture_root("owned-imports")?;
    std::fs::create_dir_all(root.join(".allow/imports"))?;
    std::fs::write(
        root.join(".allow/imports/README.md"),
        "---\nid: IMPORT-README\n---\n\nlinked_spec = \"CARGO-ALLOW-SPEC-0004\"\n",
    )?;
    let validated = validate_import_roots_config(default_import_roots_config());
    let graph = discover_import_graph(&root, &validated);
    assert!(
        graph
            .nodes
            .iter()
            .any(|node| node.path == ".allow/imports/README.md"),
        "expected discovered README node: {:?}",
        graph.nodes
    );
    assert!(
        graph
            .edges
            .iter()
            .any(|edge| edge.kind == ImportEdgeKind::Contains)
    );
    Ok(())
}

fn fixture_root(label: &str) -> io::Result<std::path::PathBuf> {
    let unique = format!(
        "cargo-allow-import-roots-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let path = std::env::temp_dir().join(unique);
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path)?;
    Ok(path)
}
