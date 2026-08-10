use allow_core::{CargoAllowError, CargoAllowResult};
use std::collections::BTreeMap;

/// The semantic role of a production-reachable authority candidate.
/// Only `SemanticEvaluator` candidates compete for product authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorityKind {
    SemanticEvaluator,
    CompatibilityProjection,
    HistoricalReader,
    TestFixtureOnly,
    GeneratedView,
}

impl AuthorityKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SemanticEvaluator => "SemanticEvaluator",
            Self::CompatibilityProjection => "CompatibilityProjection",
            Self::HistoricalReader => "HistoricalReader",
            Self::TestFixtureOnly => "TestFixtureOnly",
            Self::GeneratedView => "GeneratedView",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorityNode {
    pub id: String,
    pub ledger_entry_id: String,
    pub kind: AuthorityKind,
    pub production_reachable: bool,
    pub bound: Option<String>,
}

/// The six dispositions that can close an old path for a DeleteAfterParity
/// move. `OldPathStillReachable` is intentionally not cutover evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OldPathDisposition {
    Deleted,
    CompileUnreachable,
    FeatureUnreachableInSupportedCandidate,
    CompatibilityProjectionOnly,
    HistoricalReaderOnly,
    ExplicitlyDeferredWithinBound,
}

impl OldPathDisposition {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Deleted => "Deleted",
            Self::CompileUnreachable => "CompileUnreachable",
            Self::FeatureUnreachableInSupportedCandidate => {
                "FeatureUnreachableInSupportedCandidate"
            }
            Self::CompatibilityProjectionOnly => "CompatibilityProjectionOnly",
            Self::HistoricalReaderOnly => "HistoricalReaderOnly",
            Self::ExplicitlyDeferredWithinBound => "ExplicitlyDeferredWithinBound",
        }
    }

    pub fn parse(value: &str) -> CargoAllowResult<Self> {
        match value.trim() {
            "Deleted" => Ok(Self::Deleted),
            "CompileUnreachable" => Ok(Self::CompileUnreachable),
            "FeatureUnreachableInSupportedCandidate" => {
                Ok(Self::FeatureUnreachableInSupportedCandidate)
            }
            "CompatibilityProjectionOnly" => Ok(Self::CompatibilityProjectionOnly),
            "HistoricalReaderOnly" => Ok(Self::HistoricalReaderOnly),
            "ExplicitlyDeferredWithinBound" => Ok(Self::ExplicitlyDeferredWithinBound),
            other => Err(CargoAllowError::new(format!(
                "unsupported old-path reachability disposition `{other}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OldPathCase {
    pub ledger_entry_id: String,
    pub disposition: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReachabilityDiagnosticKind {
    DuplicateSemanticAuthority,
    UnboundedOldSemanticEvaluator,
    InvalidDisposition,
    MissingDeleteAfterParityEvidence,
    ContradictoryReachabilityDisposition,
}

impl ReachabilityDiagnosticKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DuplicateSemanticAuthority => "duplicate_semantic_authority",
            Self::UnboundedOldSemanticEvaluator => "unbounded_old_semantic_evaluator",
            Self::InvalidDisposition => "invalid_disposition",
            Self::MissingDeleteAfterParityEvidence => "missing_delete_after_parity_evidence",
            Self::ContradictoryReachabilityDisposition => "contradictory_reachability_disposition",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReachabilityDiagnostic {
    pub kind: ReachabilityDiagnosticKind,
    pub message: String,
    pub ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReachabilityReport {
    pub authority_node_count: usize,
    pub production_semantic_authority_count: usize,
    pub delete_after_parity_count: usize,
    pub disposition_counts: BTreeMap<String, usize>,
}

impl ReachabilityReport {
    pub fn is_clean(&self, diagnostics: &[ReachabilityDiagnostic]) -> bool {
        diagnostics.is_empty()
    }
}

/// Validate that production-reachable semantic authorities have a single
/// owner. Other authority classes do not count as duplicate evaluators.
pub fn validate_duplicate_authority(
    nodes: &[AuthorityNode],
) -> (Vec<ReachabilityDiagnostic>, usize) {
    let semantic_nodes: Vec<&AuthorityNode> = nodes
        .iter()
        .filter(|node| node.production_reachable && node.kind == AuthorityKind::SemanticEvaluator)
        .collect();
    if semantic_nodes.len() <= 1 {
        return (Vec::new(), semantic_nodes.len());
    }

    let ids: Vec<String> = semantic_nodes.iter().map(|node| node.id.clone()).collect();
    (
        vec![ReachabilityDiagnostic {
            kind: ReachabilityDiagnosticKind::DuplicateSemanticAuthority,
            message: format!(
                "{} production-reachable semantic evaluator authorities: {}",
                ids.len(),
                ids.join(", ")
            ),
            ids,
        }],
        semantic_nodes.len(),
    )
}

/// Validate old-path evidence for DeleteAfterParity cases.
/// A source node remaining in the tree is not itself a failure.
pub fn validate_old_path_reachability(
    cases: &[OldPathCase],
    nodes: &[AuthorityNode],
) -> (Vec<ReachabilityDiagnostic>, BTreeMap<String, usize>) {
    let mut diagnostics = Vec::new();
    let mut disposition_counts = BTreeMap::new();

    for case in cases {
        let disposition = match OldPathDisposition::parse(&case.disposition) {
            Ok(disposition) => disposition,
            Err(error) => {
                diagnostics.push(ReachabilityDiagnostic {
                    kind: ReachabilityDiagnosticKind::InvalidDisposition,
                    message: error.to_string(),
                    ids: vec![case.ledger_entry_id.clone()],
                });
                continue;
            }
        };
        *disposition_counts
            .entry(disposition.as_str().to_string())
            .or_insert(0) += 1;

        let matching: Vec<&AuthorityNode> = nodes
            .iter()
            .filter(|node| node.ledger_entry_id == case.ledger_entry_id)
            .collect();
        if matching.is_empty() {
            diagnostics.push(ReachabilityDiagnostic {
                kind: ReachabilityDiagnosticKind::MissingDeleteAfterParityEvidence,
                message: format!(
                    "DeleteAfterParity entry `{}` has no reachability evidence",
                    case.ledger_entry_id
                ),
                ids: vec![case.ledger_entry_id.clone()],
            });
            continue;
        }

        for node in matching {
            if !node.production_reachable || node.kind != AuthorityKind::SemanticEvaluator {
                continue;
            }
            if node.bound.as_deref().is_none_or(str::is_empty) {
                diagnostics.push(ReachabilityDiagnostic {
                    kind: ReachabilityDiagnosticKind::UnboundedOldSemanticEvaluator,
                    message: format!(
                        "old semantic evaluator `{}` remains production-reachable without a bound",
                        node.id
                    ),
                    ids: vec![case.ledger_entry_id.clone(), node.id.clone()],
                });
            } else if disposition != OldPathDisposition::ExplicitlyDeferredWithinBound {
                diagnostics.push(ReachabilityDiagnostic {
                    kind: ReachabilityDiagnosticKind::ContradictoryReachabilityDisposition,
                    message: format!(
                        "reachable old semantic evaluator `{}` requires ExplicitlyDeferredWithinBound",
                        node.id
                    ),
                    ids: vec![case.ledger_entry_id.clone(), node.id.clone()],
                });
            }
        }
    }

    (diagnostics, disposition_counts)
}

/// Run both checker cores and return a report suitable for a later stage
/// cutover receipt producer.
pub fn validate_cutover_reachability(
    cases: &[OldPathCase],
    nodes: &[AuthorityNode],
) -> (ReachabilityReport, Vec<ReachabilityDiagnostic>) {
    let (mut diagnostics, semantic_count) = validate_duplicate_authority(nodes);
    let (old_path_diagnostics, disposition_counts) = validate_old_path_reachability(cases, nodes);
    diagnostics.extend(old_path_diagnostics);
    (
        ReachabilityReport {
            authority_node_count: nodes.len(),
            production_semantic_authority_count: semantic_count,
            delete_after_parity_count: cases.len(),
            disposition_counts,
        },
        diagnostics,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(
        id: &str,
        ledger_entry_id: &str,
        kind: AuthorityKind,
        production_reachable: bool,
        bound: Option<&str>,
    ) -> AuthorityNode {
        AuthorityNode {
            id: id.to_string(),
            ledger_entry_id: ledger_entry_id.to_string(),
            kind,
            production_reachable,
            bound: bound.map(str::to_string),
        }
    }

    fn case(id: &str, disposition: &str) -> OldPathCase {
        OldPathCase {
            ledger_entry_id: id.to_string(),
            disposition: disposition.to_string(),
        }
    }

    #[test]
    fn seeded_second_semantic_authority_fails() -> Result<(), String> {
        let (diagnostics, count) = validate_duplicate_authority(&[
            node(
                "old",
                "move-a",
                AuthorityKind::SemanticEvaluator,
                true,
                None,
            ),
            node(
                "new",
                "move-b",
                AuthorityKind::SemanticEvaluator,
                true,
                None,
            ),
        ]);
        if count != 2 || diagnostics.len() != 1 {
            return Err(format!("unexpected duplicate report: {diagnostics:?}"));
        }
        let Some(first_diagnostic) = diagnostics.first() else {
            return Err("duplicate diagnostic was missing".to_string());
        };
        if first_diagnostic.kind != ReachabilityDiagnosticKind::DuplicateSemanticAuthority {
            return Err("wrong duplicate diagnostic".to_string());
        }
        Ok(())
    }

    #[test]
    fn non_evaluator_classes_do_not_count_as_duplicate_authority() -> Result<(), String> {
        let nodes = [
            node(
                "compat",
                "move-a",
                AuthorityKind::CompatibilityProjection,
                true,
                None,
            ),
            node(
                "history",
                "move-a",
                AuthorityKind::HistoricalReader,
                true,
                None,
            ),
            node(
                "fixture",
                "move-a",
                AuthorityKind::TestFixtureOnly,
                true,
                None,
            ),
            node(
                "generated",
                "move-a",
                AuthorityKind::GeneratedView,
                true,
                None,
            ),
            node(
                "semantic",
                "move-a",
                AuthorityKind::SemanticEvaluator,
                true,
                None,
            ),
        ];
        let (diagnostics, count) = validate_duplicate_authority(&nodes);
        if count != 1 || !diagnostics.is_empty() {
            return Err(format!(
                "non-evaluator classes were misclassified: {diagnostics:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn reachable_old_semantic_evaluator_without_bound_fails() -> Result<(), String> {
        let (diagnostics, _) = validate_old_path_reachability(
            &[case("move-a", "ExplicitlyDeferredWithinBound")],
            &[node(
                "old-evaluator",
                "move-a",
                AuthorityKind::SemanticEvaluator,
                true,
                None,
            )],
        );
        if !diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == ReachabilityDiagnosticKind::UnboundedOldSemanticEvaluator
        }) {
            return Err(format!(
                "missing unbounded evaluator diagnostic: {diagnostics:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn old_tree_node_without_production_reachability_is_not_a_defect() -> Result<(), String> {
        let (diagnostics, _) = validate_old_path_reachability(
            &[case("move-a", "Deleted")],
            &[node(
                "old-module",
                "move-a",
                AuthorityKind::SemanticEvaluator,
                false,
                None,
            )],
        );
        if !diagnostics.is_empty() {
            return Err(format!(
                "unreachable old node was rejected: {diagnostics:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn bounded_deferred_evaluator_and_projection_are_clean() -> Result<(), String> {
        let (report, diagnostics) = validate_cutover_reachability(
            &[
                case("move-a", "ExplicitlyDeferredWithinBound"),
                case("move-b", "CompatibilityProjectionOnly"),
            ],
            &[
                node(
                    "old-evaluator",
                    "move-a",
                    AuthorityKind::SemanticEvaluator,
                    true,
                    Some("parity-harness"),
                ),
                node(
                    "compatibility",
                    "move-b",
                    AuthorityKind::CompatibilityProjection,
                    true,
                    None,
                ),
            ],
        );
        if !diagnostics.is_empty() || !report.is_clean(&diagnostics) {
            return Err(format!(
                "bounded reachability was rejected: {diagnostics:?}"
            ));
        }
        if report.delete_after_parity_count != 2 {
            return Err("wrong DeleteAfterParity count".to_string());
        }
        Ok(())
    }
}
