use super::{
    DEFAULT_OWNED_IMPORTS_ROOT, SpecSystemFederationSummary, SpecSystemFinding,
    SpecSystemImportDiagnostic, SpecSystemImportEdge, SpecSystemImportGraphSummary,
    SpecSystemImportNode, SpecSystemLedgerContributor, SpecSystemWorkItem,
    apply_work_item_ledger_provenance, spec_system_proof_commands,
};
use allow_policy::federation::{
    FederationLoadOutcome, evaluate_spec_system_ledger, load_federation_config,
};
use allow_policy::import_roots::{
    ImportDiagnosticKind, ImportGraph, discover_import_graph, resolve_spec_system_import_roots,
    validate_import_roots_config,
};
use allow_policy::spec_system::ImportRootsConfig;
use std::path::Path;

pub(super) fn discover_spec_system_import_graph(
    root: &Path,
    config: Option<&ImportRootsConfig>,
) -> ImportGraph {
    let import_config = resolve_spec_system_import_roots(config);
    let validated_import_roots = validate_import_roots_config(import_config);
    discover_import_graph(root, &validated_import_roots)
}

pub(super) fn spec_system_federation_summary(
    root: &Path,
    work_items: &mut [SpecSystemWorkItem],
) -> Option<SpecSystemFederationSummary> {
    evaluate_spec_system_ledger(root).map(|evaluation| {
        if let Some(provenance) = &evaluation.active_provenance {
            apply_work_item_ledger_provenance(work_items, provenance);
        }
        SpecSystemFederationSummary {
            federation_version: evaluation.federation_version.to_string(),
            precedence_applied: evaluation.precedence_applied.as_str().to_string(),
            ledger_contributors: evaluation
                .ledger_contributors
                .into_iter()
                .map(|contributor| SpecSystemLedgerContributor {
                    id: contributor.id,
                    path: contributor.path,
                    role: contributor.role.as_str().to_string(),
                    dialect: contributor.dialect,
                    mode: contributor.mode.as_str().to_string(),
                    priority: contributor.priority,
                    lanes: contributor.lanes,
                })
                .collect(),
        }
    })
}

pub(super) fn federation_config_findings(root: &Path) -> Vec<SpecSystemFinding> {
    let Ok(loaded) = load_federation_config(root) else {
        return Vec::new();
    };
    let FederationLoadOutcome::Parsed(validated) = loaded.outcome else {
        return Vec::new();
    };
    validated
        .diagnostics
        .into_iter()
        .filter(|diagnostic| {
            !matches!(
                diagnostic.kind,
                allow_policy::federation::FederationDiagnosticKind::DialectSkipped
            )
        })
        .map(|diagnostic| {
            SpecSystemFinding::new_typed(
                "federation_config",
                format!("{}: {}", diagnostic.kind.as_str(), diagnostic.message),
                federation_diagnostic_kind(diagnostic.kind),
            )
        })
        .collect()
}

fn federation_diagnostic_kind(
    kind: allow_policy::federation::FederationDiagnosticKind,
) -> &'static str {
    match kind {
        allow_policy::federation::FederationDiagnosticKind::DuplicateId => "duplicate_id",
        allow_policy::federation::FederationDiagnosticKind::DuplicatePath
        | allow_policy::federation::FederationDiagnosticKind::DuplicateCanonicalLane
        | allow_policy::federation::FederationDiagnosticKind::MirrorMissingTarget
        | allow_policy::federation::FederationDiagnosticKind::UnknownMirrorTarget
        | allow_policy::federation::FederationDiagnosticKind::UnknownDrainMirrorLedger
        | allow_policy::federation::FederationDiagnosticKind::DrainWindowMissingField
        | allow_policy::federation::FederationDiagnosticKind::DrainWindowInvalidDate
        | allow_policy::federation::FederationDiagnosticKind::DrainWindowNotMirror
        | allow_policy::federation::FederationDiagnosticKind::PriorityTie => {
            "federation_config_invalid"
        }
        allow_policy::federation::FederationDiagnosticKind::DialectConflict => "dialect_conflict",
        allow_policy::federation::FederationDiagnosticKind::DialectSkipped => "dialect_skipped",
    }
}

pub(super) fn import_graph_findings(graph: &ImportGraph) -> Vec<SpecSystemFinding> {
    graph
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.kind != ImportDiagnosticKind::MissingRoot)
        .map(|diagnostic| {
            SpecSystemFinding::new(
                "import_graph",
                format!("{}: {}", diagnostic.kind.as_str(), diagnostic.message),
            )
        })
        .collect()
}

pub(super) fn work_items_from_import_graph(graph: &ImportGraph) -> Vec<SpecSystemWorkItem> {
    graph
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.kind != ImportDiagnosticKind::MissingRoot)
        .map(|diagnostic| {
            let kind = match diagnostic.kind {
                ImportDiagnosticKind::MissingRoot => "missing_import_root",
                ImportDiagnosticKind::BrokenEdge => "broken_import",
                ImportDiagnosticKind::DuplicateRootId
                | ImportDiagnosticKind::DuplicateRootPath
                | ImportDiagnosticKind::UnknownRole
                | ImportDiagnosticKind::InvalidRootPath => "broken_import",
            };
            let path = diagnostic
                .root_ids
                .first()
                .cloned()
                .or_else(|| Some(DEFAULT_OWNED_IMPORTS_ROOT.to_string()));
            SpecSystemWorkItem {
                kind,
                artifact_id: None,
                path,
                owner: Some("repo-infra".to_string()),
                status: Some(diagnostic.kind.as_str().to_string()),
                message: diagnostic.message.clone(),
                suggested_actions: import_graph_suggested_actions(diagnostic.kind),
                proof_commands: spec_system_proof_commands(),
                ledger_id: None,
                ledger_path: None,
                lane: Some("import".to_string()),
                mode: Some("advisory".to_string()),
                role: Some("imported".to_string()),
            }
        })
        .collect()
}

fn import_graph_suggested_actions(kind: ImportDiagnosticKind) -> Vec<String> {
    match kind {
        ImportDiagnosticKind::MissingRoot => vec![
            "create the configured import root directory".to_string(),
            "or remove the unused import root entry from the spec-system profile config"
                .to_string(),
        ],
        ImportDiagnosticKind::BrokenEdge => vec![
            "fix the broken import reference in the foreign file".to_string(),
            "or promote the import node into the owned artifact ledger".to_string(),
        ],
        ImportDiagnosticKind::DuplicateRootId | ImportDiagnosticKind::DuplicateRootPath => vec![
            "deduplicate import root ids and paths in the spec-system profile config".to_string(),
        ],
        ImportDiagnosticKind::UnknownRole => {
            vec!["use an import root role of owned, imported, legacy, or generated".to_string()]
        }
        ImportDiagnosticKind::InvalidRootPath => vec![
            "use a source-tree-relative path for the import root (no .., absolute, or drive paths)"
                .to_string(),
        ],
    }
}

pub(super) fn import_graph_summary_from_graph(graph: &ImportGraph) -> SpecSystemImportGraphSummary {
    SpecSystemImportGraphSummary {
        node_count: graph.nodes.len(),
        edge_count: graph.edges.len(),
        diagnostic_count: graph.diagnostics.len(),
        nodes: graph
            .nodes
            .iter()
            .map(|node| SpecSystemImportNode {
                id: node.id.clone(),
                path: node.path.clone(),
                role: node.role.as_str().to_string(),
                ecosystem: node.ecosystem.clone(),
                provenance: node.provenance.as_str().to_string(),
                confidence: node.confidence.as_str().to_string(),
            })
            .collect(),
        edges: graph
            .edges
            .iter()
            .map(|edge| SpecSystemImportEdge {
                source_id: edge.source_id.clone(),
                target_id: edge.target_id.clone(),
                kind: edge.kind.as_str().to_string(),
                provenance: edge.provenance.as_str().to_string(),
            })
            .collect(),
        diagnostics: graph
            .diagnostics
            .iter()
            .map(|diagnostic| SpecSystemImportDiagnostic {
                kind: diagnostic.kind.as_str().to_string(),
                message: diagnostic.message.clone(),
                root_ids: diagnostic.root_ids.clone(),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_policy::import_roots::{
        ImportConfidence, ImportDiagnostic, ImportEdge, ImportEdgeKind, ImportNode, ImportNodeRole,
        ImportProvenance,
    };

    fn diagnostic(kind: ImportDiagnosticKind, root_ids: &[&str]) -> ImportDiagnostic {
        ImportDiagnostic {
            kind,
            message: format!("{} diagnostic", kind.as_str()),
            root_ids: root_ids.iter().map(|id| (*id).to_string()).collect(),
        }
    }

    fn check(condition: bool, message: &str) -> Result<(), String> {
        condition.then_some(()).ok_or_else(|| message.to_string())
    }

    #[test]
    fn import_graph_diagnostic_actions_cover_all_kinds() -> Result<(), String> {
        let kinds = [
            ImportDiagnosticKind::MissingRoot,
            ImportDiagnosticKind::BrokenEdge,
            ImportDiagnosticKind::DuplicateRootId,
            ImportDiagnosticKind::DuplicateRootPath,
            ImportDiagnosticKind::UnknownRole,
            ImportDiagnosticKind::InvalidRootPath,
        ];
        for kind in kinds {
            check(
                !import_graph_suggested_actions(kind).is_empty(),
                kind.as_str(),
            )?;
        }

        let graph = ImportGraph {
            nodes: Vec::new(),
            edges: Vec::new(),
            diagnostics: vec![
                diagnostic(ImportDiagnosticKind::MissingRoot, &["missing"]),
                diagnostic(ImportDiagnosticKind::BrokenEdge, &["broken"]),
                diagnostic(ImportDiagnosticKind::DuplicateRootId, &["duplicate"]),
                diagnostic(ImportDiagnosticKind::UnknownRole, &["role"]),
                diagnostic(ImportDiagnosticKind::InvalidRootPath, &["path"]),
            ],
        };
        let findings = import_graph_findings(&graph);
        check(findings.len() == 4, "missing roots stay advisory-only")?;
        let work_items = work_items_from_import_graph(&graph);
        check(work_items.len() == 4, "unexpected import work-item count")?;
        check(
            work_items
                .iter()
                .all(|item| item.lane.as_deref() == Some("import")),
            "import work items must retain their lane",
        )
    }

    #[test]
    fn import_graph_summary_projects_nodes_edges_and_diagnostics() -> Result<(), String> {
        let graph = ImportGraph {
            nodes: vec![ImportNode {
                id: "owned-specs".to_string(),
                path: "docs/specs".to_string(),
                role: ImportNodeRole::Owned,
                ecosystem: "generic".to_string(),
                provenance: ImportProvenance::Configured,
                confidence: ImportConfidence::High,
            }],
            edges: vec![ImportEdge {
                source_id: "owned-specs".to_string(),
                target_id: "foreign-plan".to_string(),
                kind: ImportEdgeKind::References,
                provenance: ImportProvenance::Configured,
            }],
            diagnostics: vec![diagnostic(
                ImportDiagnosticKind::BrokenEdge,
                &["foreign-plan"],
            )],
        };
        let summary = import_graph_summary_from_graph(&graph);
        check(summary.node_count == 1, "node count was not projected")?;
        check(summary.edge_count == 1, "edge count was not projected")?;
        check(
            summary.diagnostic_count == 1,
            "diagnostic count was not projected",
        )?;
        let node = summary
            .nodes
            .first()
            .ok_or_else(|| "projected node was missing".to_string())?;
        let edge = summary
            .edges
            .first()
            .ok_or_else(|| "projected edge was missing".to_string())?;
        let diagnostic = summary
            .diagnostics
            .first()
            .ok_or_else(|| "projected diagnostic was missing".to_string())?;
        check(node.role == "owned", "node role was not projected")?;
        check(edge.kind == "references", "edge kind was not projected")?;
        check(
            diagnostic.kind == "broken_edge",
            "diagnostic kind was not projected",
        )
    }

    #[test]
    fn missing_federation_config_does_not_create_findings() -> Result<(), String> {
        check(
            federation_config_findings(Path::new("target/absent-spec-system-root")).is_empty(),
            "missing federation config should remain non-fatal",
        )
    }

    #[test]
    fn federation_findings_use_diagnostic_kinds_not_rendered_messages() -> Result<(), String> {
        use allow_policy::federation::{FederationDiagnostic, FederationDiagnosticKind};

        let config = tempfile::tempdir().map_err(|err| format!("temp dir: {err}"))?;
        std::fs::create_dir_all(config.path().join(".allow"))
            .map_err(|err| format!("allow dir: {err}"))?;
        std::fs::write(
            config.path().join(".allow/config.toml"),
            "schema_version = \"1.0\"\n\n[[ledgers]]\nid = \"a\"\npath = \"policy/a.toml\"\ndialect = \"cargo-allow\"\nrole = \"canonical\"\nlanes = [\"source-exception\"]\npriority = 1\n\n[[ledgers]]\nid = \"b\"\npath = \"policy/b.toml\"\ndialect = \"cargo-allow\"\nrole = \"canonical\"\nlanes = [\"source-exception\"]\npriority = 1\n",
        )
        .map_err(|err| format!("config: {err}"))?;

        let findings = federation_config_findings(config.path());
        check(findings.len() == 2, "expected duplicate-lane and priority-tie findings")?;
        check(
            findings.iter().all(|finding| finding.blocking_eligible),
            "all blocking federation diagnostics must remain blocking",
        )?;
        check(
            findings
                .iter()
                .all(|finding| finding.blocking_reason.is_some()),
            "typed federation diagnostics must carry blocking reasons",
        )?;

        let synthetic = FederationDiagnostic {
            kind: FederationDiagnosticKind::PriorityTie,
            message: "renamed upstream wording".to_string(),
            ledger_ids: vec!["a".to_string(), "b".to_string()],
        };
        let finding = SpecSystemFinding::new_typed(
            "federation_config",
            synthetic.message,
            federation_diagnostic_kind(synthetic.kind),
        );
        check(
            finding.blocking_reason == Some("federation_config_invalid"),
            "priority ties must not depend on rendered message wording",
        )
    }
}
