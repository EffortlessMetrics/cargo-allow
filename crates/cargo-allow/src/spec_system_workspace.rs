use crate::spec_system_workspace_composition::SELF_HOSTED_RUNTIME_PROMOTION;
use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use allow_inventory::Inventory;
use allow_policy::spec_system::{
    AuthoredSubjectRole, AuthoredSubjectSelector, CompiledSpecGraph, EvidenceClaimRegistration,
    EvidenceSubjectId, EvidenceSubjectRegistration, EvidenceSubjectRole, GraphCompileInput,
    ImplementationSeamRegistration, ImplementationSliceV1, PrecommitChangeDeclaration,
    PrecommitEvaluationInput, PrecommitInventoryPosture, PrecommitMovement, PrecommitMovementKind,
    PrecommitObjectiveEvaluation, PrecommitSubjectResolution, PrecommitSubjectResolutionStatus,
    SourceLocation, compile_spec_graph, evaluate_precommit_objectives, parse_authored_evidence_at,
    parse_authored_seams_at, parse_implementation_slice_at, parse_requirement_blocks_at,
    validate_authored_mapping,
};
use allow_rust::{
    RustTestInventory, RustTestInventoryStatus, RustTestResolution, RustTestSelector,
    RustTestSubject, RustTestTargetIdentity, RustTestTargetKind,
    inventory_rust_test_subjects_from_sources, resolve_rust_test_selector,
};
use effortless_repo_snapshot::{
    RepositorySourceView, ResolvedRevisionIdentity, SnapshotError, SourceInventory,
    SourceInventoryCompleteness, SourceInventorySource, resolve_revision_identity,
    staged_repository_snapshot,
};
use std::collections::BTreeSet;
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

pub use crate::spec_system_graph_movement::{SpecGraphMovement, SpecGraphMovementKind};
pub use crate::spec_system_workspace_composition::self_hosted_graph_sources_present;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelfHostedGraphDiagnostic {
    pub code: &'static str,
    pub subject: String,
    pub message: String,
}

#[derive(Debug)]
pub struct SelfHostedGraphCompilation {
    pub graph: CompiledSpecGraph,
    pub slice: ImplementationSliceV1,
    pub slice_source: SourceLocation,
    pub file_inventory: Inventory,
    pub inventory: RustTestInventory,
    pub diagnostics: Vec<SelfHostedGraphDiagnostic>,
    pub source_identity: Option<String>,
}

fn legacy_inventory(value: &SourceInventory) -> Inventory {
    Inventory {
        files: value.files.clone(),
        source: match value.source {
            SourceInventorySource::GitTracked => allow_inventory::InventorySource::GitTracked,
            SourceInventorySource::GitIndexStagedCandidate => {
                allow_inventory::InventorySource::GitIndexStagedCandidate
            }
            SourceInventorySource::FilesystemFallback => {
                allow_inventory::InventorySource::FilesystemFallback
            }
            SourceInventorySource::FilesystemIncludeUntracked => {
                allow_inventory::InventorySource::FilesystemIncludeUntracked
            }
        },
        completeness: match value.completeness {
            SourceInventoryCompleteness::Complete => {
                allow_inventory::InventoryCompleteness::Complete
            }
            SourceInventoryCompleteness::Scoped => allow_inventory::InventoryCompleteness::Scoped,
            SourceInventoryCompleteness::Fallback => {
                allow_inventory::InventoryCompleteness::Fallback
            }
            SourceInventoryCompleteness::Partial => allow_inventory::InventoryCompleteness::Partial,
        },
        empty_git_tracked: value.empty_git_tracked,
        deleted_tracked: value.deleted_tracked.clone(),
        git_error: value.git_error.clone(),
        skipped_paths: value.skipped_paths.clone(),
        submodule_paths: value.submodule_paths.clone(),
    }
}

fn snapshot_error(error: SnapshotError) -> CargoAllowError {
    crate::command_support::snapshot_error(error)
}

/// Evaluate a paired exact parent/staged graph through the pure policy seam.
///
/// Git identity and source reads have already completed at this boundary. This
/// adapter only translates the paired compiler's normalized movement and
/// inventory facts into the policy crate's agent-neutral DTO.
pub fn evaluate_paired_precommit_objectives(
    paired: &PairedSelfHostedGraphCompilation,
    declaration: &PrecommitChangeDeclaration,
    legacy_baseline: bool,
) -> PrecommitObjectiveEvaluation {
    let movements = paired
        .movements
        .iter()
        .map(|movement| PrecommitMovement {
            kind: match movement.kind {
                SpecGraphMovementKind::RequirementAdded => PrecommitMovementKind::RequirementAdded,
                SpecGraphMovementKind::RequirementRemoved => {
                    PrecommitMovementKind::RequirementRemoved
                }
                SpecGraphMovementKind::RequirementChanged => {
                    PrecommitMovementKind::RequirementChanged
                }
                SpecGraphMovementKind::ImplementationSliceAdded => {
                    PrecommitMovementKind::ImplementationSliceAdded
                }
                SpecGraphMovementKind::ImplementationSliceRemoved => {
                    PrecommitMovementKind::ImplementationSliceRemoved
                }
                SpecGraphMovementKind::ImplementationSliceChanged => {
                    PrecommitMovementKind::ImplementationSliceChanged
                }
                SpecGraphMovementKind::SeamMappingAdded => PrecommitMovementKind::SeamMappingAdded,
                SpecGraphMovementKind::SeamMappingRemoved => {
                    PrecommitMovementKind::SeamMappingRemoved
                }
                SpecGraphMovementKind::SeamMappingChanged => {
                    PrecommitMovementKind::SeamMappingChanged
                }
                SpecGraphMovementKind::EvidencePurposeAdded => {
                    PrecommitMovementKind::EvidencePurposeAdded
                }
                SpecGraphMovementKind::EvidencePurposeRemoved => {
                    PrecommitMovementKind::EvidencePurposeRemoved
                }
                SpecGraphMovementKind::EvidencePurposeChanged => {
                    PrecommitMovementKind::EvidencePurposeChanged
                }
                SpecGraphMovementKind::EvidenceClaimChanged => {
                    PrecommitMovementKind::EvidenceClaimChanged
                }
                SpecGraphMovementKind::SubjectSelectorAdded => {
                    PrecommitMovementKind::SubjectSelectorAdded
                }
                SpecGraphMovementKind::SubjectSelectorRemoved => {
                    PrecommitMovementKind::SubjectSelectorRemoved
                }
                SpecGraphMovementKind::SubjectSelectorChanged => {
                    PrecommitMovementKind::SubjectSelectorChanged
                }
                SpecGraphMovementKind::SubjectBodyIdentityChanged => {
                    PrecommitMovementKind::SubjectBodyIdentityChanged
                }
                SpecGraphMovementKind::ProfileOrDialectChanged => {
                    PrecommitMovementKind::ProfileOrDialectChanged
                }
                SpecGraphMovementKind::UnknownOrUncomparable => {
                    PrecommitMovementKind::UnknownOrUncomparable
                }
            },
            id: movement.id.clone(),
        })
        .collect::<Vec<_>>();
    let mut declaration = declaration.clone();
    if declaration.changed_subject_ids.is_empty() {
        declaration.changed_subject_ids = paired
            .movements
            .iter()
            .filter(|movement| {
                matches!(
                    movement.kind,
                    SpecGraphMovementKind::SubjectSelectorAdded
                        | SpecGraphMovementKind::SubjectSelectorRemoved
                        | SpecGraphMovementKind::SubjectSelectorChanged
                        | SpecGraphMovementKind::SubjectBodyIdentityChanged
                )
            })
            .map(|movement| EvidenceSubjectId(movement.id.clone()))
            .collect();
    }
    let inventory = if paired.candidate.inventory.status == RustTestInventoryStatus::Partial {
        PrecommitInventoryPosture::Partial
    } else {
        match paired.candidate.file_inventory.completeness {
            allow_inventory::InventoryCompleteness::Complete
            | allow_inventory::InventoryCompleteness::Scoped => PrecommitInventoryPosture::Complete,
            allow_inventory::InventoryCompleteness::Partial => PrecommitInventoryPosture::Partial,
            allow_inventory::InventoryCompleteness::Fallback => {
                PrecommitInventoryPosture::Unsupported
            }
        }
    };
    let subject_resolutions = paired
        .candidate
        .diagnostics
        .iter()
        .filter_map(|diagnostic| {
            let status = match diagnostic.code {
                "spec_graph_selector_ambiguous" => PrecommitSubjectResolutionStatus::Ambiguous,
                "spec_graph_selector_not_found" => PrecommitSubjectResolutionStatus::Missing,
                "spec_graph_rust_inventory_partial" => PrecommitSubjectResolutionStatus::Partial,
                "spec_graph_subject_non_executable"
                | "spec_graph_subject_generated_or_parameterized"
                | "spec_graph_selector_malformed"
                | "spec_graph_selector_cfg_or_feature_unknown" => {
                    PrecommitSubjectResolutionStatus::Unsupported
                }
                _ => return None,
            };
            Some(PrecommitSubjectResolution {
                id: EvidenceSubjectId(diagnostic.subject.clone()),
                status,
            })
        })
        .collect::<Vec<_>>();
    evaluate_precommit_objectives(PrecommitEvaluationInput {
        candidate: &paired.candidate.graph,
        slices: std::slice::from_ref(&paired.candidate.slice),
        movements: &movements,
        declaration: &declaration,
        subject_resolutions: &subject_resolutions,
        inventory,
        legacy_baseline,
    })
}

#[derive(Debug)]
pub struct PairedSelfHostedGraphCompilation {
    pub parent: SelfHostedGraphCompilation,
    pub candidate: SelfHostedGraphCompilation,
    pub parent_identity: ResolvedRevisionIdentity,
    pub candidate_identity_before: String,
    pub candidate_identity_after: String,
    pub movements: Vec<SpecGraphMovement>,
}

pub fn compile_self_hosted_graph(
    root: impl AsRef<Path>,
) -> CargoAllowResult<SelfHostedGraphCompilation> {
    let view = RepositorySourceView::filesystem(root).map_err(snapshot_error)?;
    compile_self_hosted_graph_from_view(&view)
}

pub fn compile_self_hosted_graph_staged(
    root: impl AsRef<Path>,
) -> CargoAllowResult<SelfHostedGraphCompilation> {
    let view = RepositorySourceView::staged(root).map_err(snapshot_error)?;
    compile_self_hosted_graph_from_view(&view)
}

pub fn compile_paired_self_hosted_graph(
    root: impl AsRef<Path>,
) -> CargoAllowResult<PairedSelfHostedGraphCompilation> {
    let root = root.as_ref();
    let parent_view = RepositorySourceView::committed(root, "HEAD").map_err(snapshot_error)?;
    let candidate_view = RepositorySourceView::staged(root).map_err(snapshot_error)?;
    let parent_identity = parent_view.revision_identity().cloned().ok_or_else(|| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Artifact,
            "parent source view did not retain its revision identity",
        )
    })?;
    let candidate_identity_before = candidate_view
        .source_identity()
        .map(str::to_string)
        .ok_or_else(|| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::Artifact,
                "staged source view did not retain its candidate identity",
            )
        })?;

    let parent = compile_self_hosted_graph_from_view(&parent_view)?;
    let candidate = compile_self_hosted_graph_from_view(&candidate_view)?;
    let parent_after = resolve_revision_identity(root, "HEAD").map_err(snapshot_error)?;
    let candidate_identity_after = staged_repository_snapshot(root)
        .map_err(snapshot_error)?
        .identity
        .semantic_hash;
    if parent_after != parent_identity {
        return Err(stale_source_error(
            "HEAD changed while compiling the paired parent and staged candidate",
        ));
    }
    if candidate_identity_after != candidate_identity_before {
        return Err(stale_source_error(
            "Git index changed while compiling the staged candidate",
        ));
    }
    let movements = compare_graphs(&parent.graph, &candidate.graph);
    Ok(PairedSelfHostedGraphCompilation {
        parent,
        candidate,
        parent_identity,
        candidate_identity_before,
        candidate_identity_after,
        movements,
    })
}

fn stale_source_error(message: &str) -> CargoAllowError {
    CargoAllowError::with_kind(CargoAllowErrorKind::Inventory, message)
}

fn compare_graphs(
    parent: &CompiledSpecGraph,
    candidate: &CompiledSpecGraph,
) -> Vec<SpecGraphMovement> {
    let mut movements = Vec::new();

    for id in parent
        .requirements
        .keys()
        .chain(candidate.requirements.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
    {
        match (
            parent.requirements.get(&id),
            candidate.requirements.get(&id),
        ) {
            (None, Some(_)) => movements.push(movement(
                SpecGraphMovementKind::RequirementAdded,
                id.as_str(),
            )),
            (Some(_), None) => movements.push(movement(
                SpecGraphMovementKind::RequirementRemoved,
                id.as_str(),
            )),
            (Some(before), Some(after)) if before != after => movements.push(movement(
                SpecGraphMovementKind::RequirementChanged,
                id.as_str(),
            )),
            _ => {}
        }
    }
    for id in parent
        .slices
        .keys()
        .chain(candidate.slices.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
    {
        match (parent.slices.get(&id), candidate.slices.get(&id)) {
            (None, Some(_)) => movements.push(movement(
                SpecGraphMovementKind::ImplementationSliceAdded,
                id.as_str(),
            )),
            (Some(_), None) => movements.push(movement(
                SpecGraphMovementKind::ImplementationSliceRemoved,
                id.as_str(),
            )),
            (Some(before), Some(after)) if before != after => movements.push(movement(
                SpecGraphMovementKind::ImplementationSliceChanged,
                id.as_str(),
            )),
            _ => {}
        }
    }
    for id in parent
        .seams
        .keys()
        .chain(candidate.seams.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
    {
        match (parent.seams.get(&id), candidate.seams.get(&id)) {
            (None, Some(_)) => movements.push(movement(
                SpecGraphMovementKind::SeamMappingAdded,
                id.as_str(),
            )),
            (Some(_), None) => movements.push(movement(
                SpecGraphMovementKind::SeamMappingRemoved,
                id.as_str(),
            )),
            (Some(before), Some(after)) if before != after => movements.push(movement(
                SpecGraphMovementKind::SeamMappingChanged,
                id.as_str(),
            )),
            _ => {}
        }
    }
    for id in parent
        .evidence_claims
        .keys()
        .chain(candidate.evidence_claims.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
    {
        match (
            parent.evidence_claims.get(&id),
            candidate.evidence_claims.get(&id),
        ) {
            (None, Some(_)) => movements.push(movement(
                SpecGraphMovementKind::EvidencePurposeAdded,
                id.as_str(),
            )),
            (Some(_), None) => movements.push(movement(
                SpecGraphMovementKind::EvidencePurposeRemoved,
                id.as_str(),
            )),
            (Some(before), Some(after)) if before != after => {
                let kind = if before.purpose != after.purpose {
                    SpecGraphMovementKind::EvidencePurposeChanged
                } else {
                    SpecGraphMovementKind::EvidenceClaimChanged
                };
                movements.push(movement(kind, id.as_str()));
            }
            _ => {}
        }
    }
    for id in parent
        .subjects
        .keys()
        .chain(candidate.subjects.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
    {
        match (parent.subjects.get(&id), candidate.subjects.get(&id)) {
            (None, Some(_)) => movements.push(movement(
                SpecGraphMovementKind::SubjectSelectorAdded,
                id.as_str(),
            )),
            (Some(_), None) => movements.push(movement(
                SpecGraphMovementKind::SubjectSelectorRemoved,
                id.as_str(),
            )),
            (Some(before), Some(after)) => {
                let selector_changed = before.package != after.package
                    || before.target != after.target
                    || before.module_path != after.module_path
                    || before.test_name != after.test_name;
                if selector_changed {
                    movements.push(movement(
                        SpecGraphMovementKind::SubjectSelectorChanged,
                        id.as_str(),
                    ));
                } else if before.source_identity != after.source_identity {
                    movements.push(movement(
                        SpecGraphMovementKind::SubjectBodyIdentityChanged,
                        id.as_str(),
                    ));
                } else if before != after {
                    movements.push(movement(
                        SpecGraphMovementKind::UnknownOrUncomparable,
                        id.as_str(),
                    ));
                }
            }
            _ => {}
        }
    }
    if parent.diagnostics != candidate.diagnostics {
        movements.push(movement(
            SpecGraphMovementKind::UnknownOrUncomparable,
            "graph-diagnostics",
        ));
    }
    movements.sort_by(|left, right| {
        left.kind
            .as_str()
            .cmp(right.kind.as_str())
            .then_with(|| left.id.cmp(&right.id))
    });
    movements
}

fn movement(kind: SpecGraphMovementKind, id: &str) -> SpecGraphMovement {
    SpecGraphMovement {
        kind,
        id: id.to_string(),
    }
}

fn compile_self_hosted_graph_from_view(
    view: &RepositorySourceView,
) -> CargoAllowResult<SelfHostedGraphCompilation> {
    let composition = &SELF_HOSTED_RUNTIME_PROMOTION;
    let requirement_text = view
        .read_text(Path::new(composition.requirement_path))
        .map_err(snapshot_error)?;
    let slice_text = view
        .read_text(Path::new(composition.slice_path))
        .map_err(snapshot_error)?;
    let seams_text = view
        .read_text(Path::new(composition.seams_path))
        .map_err(snapshot_error)?;
    let evidence_text = view
        .read_text(Path::new(composition.evidence_path))
        .map_err(snapshot_error)?;
    let requirements = parse_requirement_blocks_at(
        Some(Path::new(composition.requirement_path)),
        &requirement_text,
    )?;
    let slice =
        parse_implementation_slice_at(Some(Path::new(composition.slice_path)), &slice_text)?;
    let seams = parse_authored_seams_at(Some(Path::new(composition.seams_path)), &seams_text)?;
    let evidence =
        parse_authored_evidence_at(Some(Path::new(composition.evidence_path)), &evidence_text)?;
    validate_authored_mapping(&requirements, &slice, &seams, &evidence)?;

    let (manifests, sources) = view.rust_inputs().map_err(snapshot_error)?;
    let rust_inventory =
        inventory_rust_test_subjects_from_sources(manifests, sources, &Default::default());
    let mut diagnostics = view
        .limitations()
        .iter()
        .map(|limitation| SelfHostedGraphDiagnostic {
            code: "spec_graph_source_view_partial",
            subject: "source-view".to_string(),
            message: limitation.clone(),
        })
        .collect::<Vec<_>>();
    if rust_inventory.status == RustTestInventoryStatus::Partial {
        diagnostics.push(SelfHostedGraphDiagnostic {
            code: "spec_graph_rust_inventory_partial",
            subject: "rust-inventory".to_string(),
            message: "Rust subject inventory is partial; exact subject edges are not complete"
                .to_string(),
        });
    }

    let mut subjects = Vec::new();
    let mut claims = Vec::new();
    for claim in &evidence.evidence {
        let mut subject_ids = Vec::new();
        let mut related_subject_ids = Vec::new();
        for authored_subject in &claim.subject {
            let selector = authored_selector(authored_subject)?;
            let resolution = resolve_rust_test_selector(&rust_inventory, &selector);
            let subject_id = EvidenceSubjectId(authored_subject.id.clone());
            let registration = subject_registration(
                &subject_id,
                authored_subject.role,
                &selector,
                &claim.source,
                resolution,
                &mut diagnostics,
            );
            subjects.push(registration);
            if authored_subject.role == AuthoredSubjectRole::ExactEvidence {
                subject_ids.push(subject_id);
            } else {
                related_subject_ids.push(subject_id);
            }
        }
        claims.push(EvidenceClaimRegistration {
            id: claim.id.clone(),
            requirement_id: claim.requirement_id.clone(),
            slice_id: claim.slice_id.clone(),
            seam_id: claim.seam_id.clone(),
            purpose: claim.purpose,
            precondition: claim.precondition.clone(),
            operation: claim.operation.clone(),
            expected_observable: claim.expected_observable.clone(),
            discriminator: claim.discriminator.clone(),
            claim_boundary: claim.claim_boundary.clone(),
            source: claim.source.clone(),
            subject_ids,
            related_subject_ids,
        });
    }

    let seam_registrations = seams
        .seam
        .iter()
        .map(|seam| ImplementationSeamRegistration {
            id: seam.id.clone(),
            owner: seam.owner.clone(),
            operation: seam.operation.clone(),
            source: seam.source.clone(),
        })
        .collect();
    let graph = compile_spec_graph(GraphCompileInput {
        requirement_graphs: vec![requirements],
        implementation_slices: vec![slice.clone()],
        seams: seam_registrations,
        evidence_claims: claims,
        subjects,
    });
    Ok(SelfHostedGraphCompilation {
        graph,
        slice,
        slice_source: SourceLocation::new(composition.slice_path),
        file_inventory: legacy_inventory(view.inventory()),
        inventory: rust_inventory,
        diagnostics,
        source_identity: view.source_identity().map(str::to_string),
    })
}

fn authored_selector(subject: &AuthoredSubjectSelector) -> CargoAllowResult<RustTestSelector> {
    let (kind, name) = subject.target.split_once(':').ok_or_else(|| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Artifact,
            format!(
                "authored subject {} has malformed target {}",
                subject.id, subject.target
            ),
        )
    })?;
    let target_kind = match kind {
        "lib" => RustTestTargetKind::Library,
        "bin" => RustTestTargetKind::Binary,
        "integration_test" => RustTestTargetKind::IntegrationTest,
        _ => {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::Artifact,
                format!(
                    "authored subject {} has unsupported target kind {}",
                    subject.id, kind
                ),
            ));
        }
    };
    Ok(RustTestSelector {
        package: subject.package.clone(),
        target: RustTestTargetIdentity {
            kind: target_kind,
            name: name.to_string(),
        },
        module_path: subject
            .module_path
            .split("::")
            .map(str::to_string)
            .collect(),
        function: subject.test_name.clone(),
    })
}

fn subject_registration(
    subject_id: &EvidenceSubjectId,
    role: AuthoredSubjectRole,
    selector: &RustTestSelector,
    authored_source: &SourceLocation,
    resolution: RustTestResolution,
    diagnostics: &mut Vec<SelfHostedGraphDiagnostic>,
) -> EvidenceSubjectRegistration {
    let role = match role {
        AuthoredSubjectRole::ExactEvidence => EvidenceSubjectRole::ExactEvidence,
        AuthoredSubjectRole::RelatedWeak => EvidenceSubjectRole::RelatedWeak,
    };
    let (source, source_identity) = match resolution {
        RustTestResolution::ResolvedExact(subject) => {
            (rust_subject_source(&subject), subject.body_identity)
        }
        resolution => {
            diagnostics.push(resolution_diagnostic(subject_id, selector, &resolution));
            (
                authored_source.clone(),
                format!("authored-selector:{}", subject_id.as_str()),
            )
        }
    };
    EvidenceSubjectRegistration {
        id: subject_id.clone(),
        role,
        package: selector.package.clone(),
        target: selector.target.name.clone(),
        module_path: selector.module_path.join("::"),
        test_name: selector.function.clone(),
        source,
        source_identity,
    }
}

fn rust_subject_source(subject: &RustTestSubject) -> SourceLocation {
    SourceLocation {
        path: subject.source_path.clone(),
        line: Some(subject.source_range.start_line),
        symbol: Some(subject.selector.display_name()),
    }
}

fn resolution_diagnostic(
    subject_id: &EvidenceSubjectId,
    selector: &RustTestSelector,
    resolution: &RustTestResolution,
) -> SelfHostedGraphDiagnostic {
    let (code, message) = match resolution {
        RustTestResolution::Ambiguous(_) => (
            "spec_graph_selector_ambiguous",
            "authored selector resolves to multiple Rust subjects",
        ),
        RustTestResolution::NotFound => (
            "spec_graph_selector_not_found",
            "authored selector does not resolve to a discovered Rust subject",
        ),
        RustTestResolution::Ignored(_) => (
            "spec_graph_subject_non_executable",
            "authored selector resolves to an ignored Rust test",
        ),
        RustTestResolution::GeneratedOrParameterized(_) => (
            "spec_graph_subject_generated_or_parameterized",
            "authored selector resolves to a generated or parameterized Rust test",
        ),
        RustTestResolution::CfgOrFeatureUnknown(_) => (
            "spec_graph_subject_cfg_or_feature_unknown",
            "authored selector has unknown cfg or feature conditions",
        ),
        RustTestResolution::PartialInventory => (
            "spec_graph_rust_inventory_partial",
            "authored selector could not be resolved from a partial Rust inventory",
        ),
        RustTestResolution::MalformedSelector => (
            "spec_graph_selector_malformed",
            "authored selector is malformed",
        ),
        RustTestResolution::ResolvedExact(_) => (
            "spec_graph_internal",
            "resolved Rust subject was passed to diagnostic projection",
        ),
    };
    SelfHostedGraphDiagnostic {
        code,
        subject: subject_id.as_str().to_string(),
        message: format!("{}: {message}", selector.display_name()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec_system_workspace_composition::SELF_HOSTED_RUNTIME_PROMOTION;
    use effortless_repo_snapshot::SnapshotErrorKind;
    use std::fs;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    }

    #[test]
    fn compiles_checked_in_self_hosted_graph_from_real_sources() -> Result<(), String> {
        let result =
            compile_self_hosted_graph(workspace_root()).map_err(|error| error.to_string())?;
        if result.file_inventory.files.is_empty() {
            return Err(format!(
                "expected a non-empty source inventory, got {:?}",
                result.file_inventory
            ));
        }
        let partial_diagnostics = result
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "spec_graph_rust_inventory_partial")
            .count();
        if partial_diagnostics != 4 {
            return Err(format!(
                "expected inventory and three subject partial diagnostics, got {:?}; inventory: {:?}",
                result.diagnostics, result.inventory.diagnostics
            ));
        }
        if !result.graph.diagnostics.is_empty() {
            return Err(format!(
                "unexpected graph diagnostics: {:?}",
                result.graph.diagnostics
            ));
        }
        require_relative_paths(
            result
                .graph
                .requirements
                .values()
                .map(|node| &node.source.path),
        )?;
        require_relative_paths(result.graph.seams.values().map(|node| &node.source.path))?;
        require_relative_paths(
            result
                .graph
                .evidence_claims
                .values()
                .map(|node| &node.source.path),
        )?;
        require_relative_paths(result.graph.subjects.values().map(|node| &node.source.path))?;
        require_len(result.graph.requirements.len(), 1, "requirements")?;
        require_len(result.graph.slices.len(), 1, "slices")?;
        require_len(result.graph.seams.len(), 1, "seams")?;
        require_len(result.graph.evidence_claims.len(), 2, "evidence claims")?;
        require_len(result.graph.subjects.len(), 3, "subjects")?;
        Ok(())
    }

    #[test]
    fn snapshot_error_projection_preserves_error_kind() {
        let cases = [
            (SnapshotErrorKind::Internal, CargoAllowErrorKind::Internal),
            (
                SnapshotErrorKind::InvalidConfig,
                CargoAllowErrorKind::InvalidConfig,
            ),
            (SnapshotErrorKind::Inventory, CargoAllowErrorKind::Inventory),
            (SnapshotErrorKind::Artifact, CargoAllowErrorKind::Artifact),
            (SnapshotErrorKind::Unknown, CargoAllowErrorKind::Unknown),
            (SnapshotErrorKind::Scan, CargoAllowErrorKind::Scan),
        ];
        for (source, expected) in cases {
            assert_eq!(
                snapshot_error(SnapshotError::with_kind(source, "error")).kind(),
                expected
            );
        }
    }

    fn require_len(actual: usize, expected: usize, label: &str) -> Result<(), String> {
        if actual == expected {
            Ok(())
        } else {
            Err(format!(
                "expected {label} length {expected}, got {}",
                actual
            ))
        }
    }

    fn require_relative_paths<'a>(paths: impl Iterator<Item = &'a String>) -> Result<(), String> {
        for path in paths {
            if path.contains(':') || path.starts_with('/') || path.starts_with('\\') {
                return Err(format!(
                    "source location must be repository-relative: {path}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn missing_exact_subject_is_reported_before_graph_consumers() -> Result<(), String> {
        let inventory = RustTestInventory {
            subjects: Vec::new(),
            status: RustTestInventoryStatus::Complete,
            diagnostics: Vec::new(),
        };
        let selector = RustTestSelector {
            package: "allow-policy".to_string(),
            target: RustTestTargetIdentity {
                kind: RustTestTargetKind::Library,
                name: "allow_policy".to_string(),
            },
            module_path: vec!["spec_system".to_string()],
            function: "removed_test".to_string(),
        };
        match resolve_rust_test_selector(&inventory, &selector) {
            RustTestResolution::NotFound => Ok(()),
            resolution => Err(format!("expected not found resolution, got {resolution:?}")),
        }
    }

    #[test]
    fn staged_graph_compilation_uses_one_candidate_for_spec_and_subject_inputs()
    -> Result<(), String> {
        let root = staged_fixture_repository()?;
        fs::write(
            root.join(SELF_HOSTED_RUNTIME_PROMOTION.seams_path),
            include_str!(concat!(
                ::std::env!("CARGO_MANIFEST_DIR"),
                "/../../.allow/spec-system/seams/runtime-promotion-validator-v1.toml"
            ))
            .replace("owner = \"allow-policy\"", "owner = \"worktree-only\""),
        )
        .map_err(|error| error.to_string())?;

        let staged = compile_self_hosted_graph_staged(&root).map_err(|error| error.to_string())?;
        let current = compile_self_hosted_graph(&root).map_err(|error| error.to_string())?;
        assert_eq!(
            staged.file_inventory.source,
            allow_inventory::InventorySource::GitIndexStagedCandidate
        );
        assert_eq!(
            current.file_inventory.source,
            allow_inventory::InventorySource::GitTracked
        );
        assert!(staged.source_identity.is_some());
        assert_eq!(
            staged
                .graph
                .seams
                .values()
                .next()
                .ok_or_else(|| "staged graph has no seam".to_string())?
                .owner,
            "allow-policy"
        );
        assert_eq!(
            current
                .graph
                .seams
                .values()
                .next()
                .ok_or_else(|| "current graph has no seam".to_string())?
                .owner,
            "worktree-only"
        );
        assert_ne!(staged.graph.snapshot_id, current.graph.snapshot_id);
        fs::remove_dir_all(root).map_err(|error| error.to_string())
    }

    #[test]
    fn paired_graph_compilation_excludes_dirty_worktree_from_parent_and_candidate()
    -> Result<(), String> {
        let root = staged_fixture_repository()?;
        run_git(&root, &["commit", "-qm", "parent"])?;
        let seam_source = include_str!(concat!(
            ::std::env!("CARGO_MANIFEST_DIR"),
            "/../../.allow/spec-system/seams/runtime-promotion-validator-v1.toml"
        ));
        fs::write(
            root.join(SELF_HOSTED_RUNTIME_PROMOTION.seams_path),
            seam_source.replace("owner = \"allow-policy\"", "owner = \"candidate-owner\""),
        )
        .map_err(|error| error.to_string())?;
        run_git(
            &root,
            &["add", "--", SELF_HOSTED_RUNTIME_PROMOTION.seams_path],
        )?;
        fs::write(
            root.join(SELF_HOSTED_RUNTIME_PROMOTION.seams_path),
            seam_source.replace("owner = \"allow-policy\"", "owner = \"worktree-only\""),
        )
        .map_err(|error| error.to_string())?;

        let paired = compile_paired_self_hosted_graph(&root).map_err(|error| error.to_string())?;
        assert_eq!(paired.parent_identity.commit.len(), 40);
        assert_eq!(
            paired.candidate_identity_before,
            paired.candidate_identity_after
        );
        assert_eq!(
            paired
                .parent
                .graph
                .seams
                .values()
                .next()
                .ok_or_else(|| "parent graph has no seam".to_string())?
                .owner,
            "allow-policy"
        );
        assert_eq!(
            paired
                .candidate
                .graph
                .seams
                .values()
                .next()
                .ok_or_else(|| "candidate graph has no seam".to_string())?
                .owner,
            "candidate-owner"
        );
        assert!(
            paired
                .movements
                .iter()
                .any(|movement| { movement.kind == SpecGraphMovementKind::SeamMappingChanged })
        );
        fs::remove_dir_all(root).map_err(|error| error.to_string())
    }

    #[test]
    fn spec_precommit_change_classes() -> Result<(), String> {
        let root = staged_fixture_repository()?;
        run_git(&root, &["commit", "-qm", "parent"])?;
        let paired = compile_paired_self_hosted_graph(&root).map_err(|error| error.to_string())?;
        for class in [
            allow_policy::spec_system::PrecommitChangeClass::BehaviorChange,
            allow_policy::spec_system::PrecommitChangeClass::BugFix,
            allow_policy::spec_system::PrecommitChangeClass::SpecOrPolicyChange,
            allow_policy::spec_system::PrecommitChangeClass::DocsOnly,
            allow_policy::spec_system::PrecommitChangeClass::Mechanical,
        ] {
            let declaration = allow_policy::spec_system::PrecommitChangeDeclaration {
                class: Some(class),
                ..Default::default()
            };
            let result = evaluate_paired_precommit_objectives(&paired, &declaration, false);
            assert_eq!(result.change_class, class);
        }
        fs::remove_dir_all(root).map_err(|error| error.to_string())
    }

    #[test]
    fn spec_precommit_brownfield_no_new() -> Result<(), String> {
        let root = staged_fixture_repository()?;
        run_git(&root, &["commit", "-qm", "parent"])?;
        let paired = compile_paired_self_hosted_graph(&root).map_err(|error| error.to_string())?;
        let declaration = allow_policy::spec_system::PrecommitChangeDeclaration {
            class: Some(allow_policy::spec_system::PrecommitChangeClass::BehaviorChange),
            ..Default::default()
        };
        let result = allow_policy::spec_system::evaluate_precommit_objectives(
            allow_policy::spec_system::PrecommitEvaluationInput {
                candidate: &paired.candidate.graph,
                slices: &[],
                movements: &[],
                declaration: &declaration,
                subject_resolutions: &[],
                inventory: allow_policy::spec_system::PrecommitInventoryPosture::Complete,
                legacy_baseline: true,
            },
        );
        let finding = result
            .findings
            .iter()
            .find(|finding| {
                finding.code
                    == allow_policy::spec_system::PrecommitFindingCode::BehaviorSliceMissing
            })
            .ok_or_else(|| "legacy baseline hid a new behavior-slice defect".to_string())?;
        assert_eq!(
            finding.posture,
            allow_policy::spec_system::PrecommitFindingPosture::Blocking
        );
        fs::remove_dir_all(root).map_err(|error| error.to_string())
    }

    #[test]
    fn spec_precommit_proportionate_changes() -> Result<(), String> {
        let root = staged_fixture_repository()?;
        run_git(&root, &["commit", "-qm", "parent"])?;
        let paired = compile_paired_self_hosted_graph(&root).map_err(|error| error.to_string())?;
        for class in [
            allow_policy::spec_system::PrecommitChangeClass::DocsOnly,
            allow_policy::spec_system::PrecommitChangeClass::Mechanical,
            allow_policy::spec_system::PrecommitChangeClass::ToolingOrCiChange,
        ] {
            let declaration = allow_policy::spec_system::PrecommitChangeDeclaration {
                class: Some(class),
                ..Default::default()
            };
            let result = evaluate_paired_precommit_objectives(&paired, &declaration, false);
            assert!(
                result.findings.is_empty(),
                "unexpected findings for {class:?}: {:?}",
                result.findings
            );
        }
        fs::remove_dir_all(root).map_err(|error| error.to_string())
    }

    #[test]
    fn spec_precommit_partial_stage_corpus() -> Result<(), String> {
        let root = staged_fixture_repository()?;
        let _cleanup = FixtureCleanup(root.clone());
        run_git(&root, &["commit", "-qm", "parent"])?;
        let paired = compile_paired_self_hosted_graph(&root).map_err(|error| error.to_string())?;
        let declaration = allow_policy::spec_system::PrecommitChangeDeclaration {
            class: Some(allow_policy::spec_system::PrecommitChangeClass::BehaviorChange),
            ..Default::default()
        };
        let result = allow_policy::spec_system::evaluate_precommit_objectives(
            allow_policy::spec_system::PrecommitEvaluationInput {
                candidate: &paired.candidate.graph,
                slices: &[],
                movements: &[],
                declaration: &declaration,
                subject_resolutions: &[],
                inventory: allow_policy::spec_system::PrecommitInventoryPosture::Complete,
                legacy_baseline: false,
            },
        );
        if !result.findings.iter().any(|finding| {
            finding.code == allow_policy::spec_system::PrecommitFindingCode::BehaviorSliceMissing
        }) {
            return Err("partial staged candidate without a slice passed".to_string());
        }
        Ok(())
    }

    struct FixtureCleanup(PathBuf);

    impl Drop for FixtureCleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn staged_fixture_repository() -> Result<PathBuf, String> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "cargo-allow-staged-graph-{}-{nonce}",
            std::process::id()
        ));
        let files = [
            (
                SELF_HOSTED_RUNTIME_PROMOTION.requirement_path,
                include_str!(concat!(
                    ::std::env!("CARGO_MANIFEST_DIR"),
                    "/../../docs/specs/CARGO-ALLOW-SPEC-0009-design-to-proof-walking-skeleton.md"
                )),
            ),
            (
                SELF_HOSTED_RUNTIME_PROMOTION.slice_path,
                include_str!(concat!(
                    ::std::env!("CARGO_MANIFEST_DIR"),
                    "/../../.allow/spec-system/slices/self-hosted-runtime-promotion-v1.toml"
                )),
            ),
            (
                SELF_HOSTED_RUNTIME_PROMOTION.seams_path,
                include_str!(concat!(
                    ::std::env!("CARGO_MANIFEST_DIR"),
                    "/../../.allow/spec-system/seams/runtime-promotion-validator-v1.toml"
                )),
            ),
            (
                SELF_HOSTED_RUNTIME_PROMOTION.evidence_path,
                include_str!(concat!(
                    ::std::env!("CARGO_MANIFEST_DIR"),
                    "/../../.allow/spec-system/evidence/runtime-promotion-v1.toml"
                )),
            ),
            (
                "crates/allow-policy/Cargo.toml",
                include_str!(concat!(
                    ::std::env!("CARGO_MANIFEST_DIR"),
                    "/../allow-policy/Cargo.toml"
                )),
            ),
            (
                "crates/allow-policy/src/lib.rs",
                include_str!(concat!(
                    ::std::env!("CARGO_MANIFEST_DIR"),
                    "/../allow-policy/src/lib.rs"
                )),
            ),
            (
                "crates/allow-policy/src/spec_system/runtime_promotion.rs",
                include_str!(concat!(
                    ::std::env!("CARGO_MANIFEST_DIR"),
                    "/../allow-policy/src/spec_system/runtime_promotion.rs"
                )),
            ),
        ];
        fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        run_git(&root, &["init", "-q"])?;
        run_git(&root, &["config", "user.name", "Cargo Allow"])?;
        run_git(
            &root,
            &["config", "user.email", "cargo-allow@example.invalid"],
        )?;
        for (path, contents) in files {
            let full = root.join(path);
            if let Some(parent) = full.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            fs::write(full, contents).map_err(|error| error.to_string())?;
        }
        run_git(&root, &["add", "--all"])?;
        Ok(root)
    }

    fn run_git(root: &Path, args: &[&str]) -> Result<String, String> {
        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .map_err(|error| error.to_string())?;
        if output.status.success() {
            String::from_utf8(output.stdout).map_err(|error| error.to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).into_owned())
        }
    }
}
