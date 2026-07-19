use crate::spec_system_source::RepositorySourceView;
use allow_core::{CargoAllowError, CargoAllowResult};
use allow_inventory::Inventory;
use allow_policy::spec_system::{
    AuthoredSubjectRole, AuthoredSubjectSelector, CompiledSpecGraph, EvidenceClaimRegistration,
    EvidenceSubjectId, EvidenceSubjectRegistration, EvidenceSubjectRole, GraphCompileInput,
    ImplementationSeamRegistration, SourceLocation, compile_spec_graph, parse_authored_evidence_at,
    parse_authored_seams_at, parse_implementation_slice_at, parse_requirement_blocks_at,
    validate_authored_mapping,
};
use allow_rust::{
    RustTestInventory, RustTestInventoryStatus, RustTestResolution, RustTestSelector,
    RustTestSubject, RustTestTargetIdentity, RustTestTargetKind,
    inventory_rust_test_subjects_from_sources, resolve_rust_test_selector,
};
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

const SPEC_PATH: &str = "docs/specs/CARGO-ALLOW-SPEC-0009-design-to-proof-walking-skeleton.md";
const SLICE_PATH: &str = ".allow/spec-system/slices/self-hosted-runtime-promotion-v1.toml";
const SEAMS_PATH: &str = ".allow/spec-system/seams/runtime-promotion-validator-v1.toml";
const EVIDENCE_PATH: &str = ".allow/spec-system/evidence/runtime-promotion-v1.toml";

pub fn self_hosted_graph_sources_present(root: impl AsRef<Path>) -> bool {
    let root = root.as_ref();
    [SPEC_PATH, SLICE_PATH, SEAMS_PATH, EVIDENCE_PATH]
        .iter()
        .all(|path| root.join(path).is_file())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelfHostedGraphDiagnostic {
    pub code: &'static str,
    pub subject: String,
    pub message: String,
}

#[derive(Debug)]
pub struct SelfHostedGraphCompilation {
    pub graph: CompiledSpecGraph,
    pub slice_source: SourceLocation,
    pub file_inventory: Inventory,
    pub inventory: RustTestInventory,
    pub diagnostics: Vec<SelfHostedGraphDiagnostic>,
    pub source_identity: Option<String>,
}

pub fn compile_self_hosted_graph(
    root: impl AsRef<Path>,
) -> CargoAllowResult<SelfHostedGraphCompilation> {
    let view = RepositorySourceView::filesystem(root)?;
    compile_self_hosted_graph_from_view(&view)
}

pub fn compile_self_hosted_graph_staged(
    root: impl AsRef<Path>,
) -> CargoAllowResult<SelfHostedGraphCompilation> {
    let view = RepositorySourceView::staged(root)?;
    compile_self_hosted_graph_from_view(&view)
}

fn compile_self_hosted_graph_from_view(
    view: &RepositorySourceView,
) -> CargoAllowResult<SelfHostedGraphCompilation> {
    let requirement_text = view.read_text(Path::new(SPEC_PATH))?;
    let slice_text = view.read_text(Path::new(SLICE_PATH))?;
    let seams_text = view.read_text(Path::new(SEAMS_PATH))?;
    let evidence_text = view.read_text(Path::new(EVIDENCE_PATH))?;
    let requirements = parse_requirement_blocks_at(Some(Path::new(SPEC_PATH)), &requirement_text)?;
    let slice = parse_implementation_slice_at(Some(Path::new(SLICE_PATH)), &slice_text)?;
    let seams = parse_authored_seams_at(Some(Path::new(SEAMS_PATH)), &seams_text)?;
    let evidence = parse_authored_evidence_at(Some(Path::new(EVIDENCE_PATH)), &evidence_text)?;
    validate_authored_mapping(&requirements, &slice, &seams, &evidence)?;

    let (manifests, sources) = view.rust_inputs()?;
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
        implementation_slices: vec![slice],
        seams: seam_registrations,
        evidence_claims: claims,
        subjects,
    });
    Ok(SelfHostedGraphCompilation {
        graph,
        slice_source: SourceLocation::new(SLICE_PATH),
        file_inventory: view.inventory().clone(),
        inventory: rust_inventory,
        diagnostics,
        source_identity: view.source_identity().map(str::to_string),
    })
}

fn authored_selector(subject: &AuthoredSubjectSelector) -> CargoAllowResult<RustTestSelector> {
    let (kind, name) = subject.target.split_once(':').ok_or_else(|| {
        CargoAllowError::new(format!(
            "authored subject {} has malformed target {}",
            subject.id, subject.target
        ))
    })?;
    let target_kind = match kind {
        "lib" => RustTestTargetKind::Library,
        "bin" => RustTestTargetKind::Binary,
        "integration_test" => RustTestTargetKind::IntegrationTest,
        _ => {
            return Err(CargoAllowError::new(format!(
                "authored subject {} has unsupported target kind {}",
                subject.id, kind
            )));
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
            root.join(SEAMS_PATH),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
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
                SPEC_PATH,
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../docs/specs/CARGO-ALLOW-SPEC-0009-design-to-proof-walking-skeleton.md"
                )),
            ),
            (
                SLICE_PATH,
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../.allow/spec-system/slices/self-hosted-runtime-promotion-v1.toml"
                )),
            ),
            (
                SEAMS_PATH,
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../.allow/spec-system/seams/runtime-promotion-validator-v1.toml"
                )),
            ),
            (
                EVIDENCE_PATH,
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../.allow/spec-system/evidence/runtime-promotion-v1.toml"
                )),
            ),
            (
                "crates/allow-policy/Cargo.toml",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../allow-policy/Cargo.toml"
                )),
            ),
            (
                "crates/allow-policy/src/lib.rs",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../allow-policy/src/lib.rs"
                )),
            ),
            (
                "crates/allow-policy/src/spec_system/runtime_promotion.rs",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
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
