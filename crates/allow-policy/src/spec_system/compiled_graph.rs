use allow_core::{normalize_path, stable_hash_hex};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use super::{
    EvidenceDispositionState, ImplementationClaimStatus, ImplementationSliceClass,
    ImplementationSliceId, ImplementationSliceV1, RequirementClaimClass, RequirementGraph,
    RequirementId, RequirementStatus, SupportClaimDispositionState,
};

macro_rules! graph_id {
    ($name:ident) => {
        #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

graph_id!(GraphSnapshotId);
graph_id!(ImplementationSeamId);
graph_id!(EvidenceClaimId);
graph_id!(EvidenceSubjectId);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceLocation {
    pub path: String,
    #[serde(default)]
    pub line: Option<u32>,
    #[serde(default)]
    pub symbol: Option<String>,
}

impl SourceLocation {
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: normalize_path(path.into()),
            line: None,
            symbol: None,
        }
    }

    pub fn with_line(mut self, line: u32) -> Self {
        self.line = Some(line);
        self
    }

    pub fn with_symbol(mut self, symbol: impl Into<String>) -> Self {
        self.symbol = Some(symbol.into());
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidencePurpose {
    PositiveAcceptance,
    ForbiddenRuntimePromotion,
}

impl EvidencePurpose {
    fn as_str(self) -> &'static str {
        match self {
            Self::PositiveAcceptance => "positive_acceptance",
            Self::ForbiddenRuntimePromotion => "forbidden_runtime_promotion",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceSubjectRole {
    ExactEvidence,
    RelatedWeak,
}

impl EvidenceSubjectRole {
    fn as_str(self) -> &'static str {
        match self {
            Self::ExactEvidence => "exact_evidence",
            Self::RelatedWeak => "related_weak",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImplementationSeamRegistration {
    pub id: ImplementationSeamId,
    pub owner: String,
    pub operation: String,
    pub source: SourceLocation,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceSubjectRegistration {
    pub id: EvidenceSubjectId,
    pub role: EvidenceSubjectRole,
    pub package: String,
    pub target: String,
    pub module_path: String,
    pub test_name: String,
    pub source: SourceLocation,
    pub source_identity: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceClaimRegistration {
    pub id: EvidenceClaimId,
    pub requirement_id: RequirementId,
    pub slice_id: ImplementationSliceId,
    pub seam_id: ImplementationSeamId,
    pub purpose: EvidencePurpose,
    pub precondition: String,
    pub operation: String,
    pub expected_observable: String,
    pub discriminator: String,
    pub claim_boundary: String,
    pub source: SourceLocation,
    pub subject_ids: Vec<EvidenceSubjectId>,
    #[serde(default)]
    pub related_subject_ids: Vec<EvidenceSubjectId>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequirementNode {
    pub id: RequirementId,
    pub generation: u32,
    pub status: RequirementStatus,
    pub claim_class: RequirementClaimClass,
    pub source: SourceLocation,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImplementationSliceNode {
    pub id: ImplementationSliceId,
    pub generation: u32,
    pub change_class: ImplementationSliceClass,
    pub implementation_claim_status: ImplementationClaimStatus,
    pub evidence_state: EvidenceDispositionState,
    pub support_claim_state: SupportClaimDispositionState,
    pub requirement_ids: Vec<RequirementId>,
    pub owned_seams: BTreeSet<String>,
    pub shared_seams: BTreeSet<String>,
    pub forbidden_seams: BTreeSet<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImplementationSeamNode {
    pub id: ImplementationSeamId,
    pub owner: String,
    pub operation: String,
    pub source: SourceLocation,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceSubjectNode {
    pub id: EvidenceSubjectId,
    pub role: EvidenceSubjectRole,
    pub package: String,
    pub target: String,
    pub module_path: String,
    pub test_name: String,
    pub source: SourceLocation,
    pub source_identity: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceClaimNode {
    pub id: EvidenceClaimId,
    pub requirement_id: RequirementId,
    pub slice_id: ImplementationSliceId,
    pub seam_id: ImplementationSeamId,
    pub purpose: EvidencePurpose,
    pub precondition: String,
    pub operation: String,
    pub expected_observable: String,
    pub discriminator: String,
    pub claim_boundary: String,
    pub source: SourceLocation,
    pub subject_ids: Vec<EvidenceSubjectId>,
    pub related_subject_ids: Vec<EvidenceSubjectId>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphDiagnosticCode {
    DuplicateId,
    UnknownRequirement,
    UnknownSlice,
    UnknownSeam,
    UnknownSubject,
    EmptyEvidenceSubjects,
    ExactSubjectMarkedWeak,
    RelatedSubjectMarkedExact,
    SliceRequirementGenerationMismatch,
    SeamNotDeclaredBySlice,
    ForbiddenSeam,
}

impl GraphDiagnosticCode {
    fn as_str(self) -> &'static str {
        match self {
            Self::DuplicateId => "duplicate_id",
            Self::UnknownRequirement => "unknown_requirement",
            Self::UnknownSlice => "unknown_slice",
            Self::UnknownSeam => "unknown_seam",
            Self::UnknownSubject => "unknown_subject",
            Self::EmptyEvidenceSubjects => "empty_evidence_subjects",
            Self::ExactSubjectMarkedWeak => "exact_subject_marked_weak",
            Self::RelatedSubjectMarkedExact => "related_subject_marked_exact",
            Self::SliceRequirementGenerationMismatch => "slice_requirement_generation_mismatch",
            Self::SeamNotDeclaredBySlice => "seam_not_declared_by_slice",
            Self::ForbiddenSeam => "forbidden_seam",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphDiagnostic {
    pub code: GraphDiagnosticCode,
    pub subject: String,
    pub message: String,
}

impl GraphDiagnostic {
    fn new(
        code: GraphDiagnosticCode,
        subject: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            subject: subject.into(),
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphCompileInput {
    pub requirement_graphs: Vec<RequirementGraph>,
    pub implementation_slices: Vec<ImplementationSliceV1>,
    pub seams: Vec<ImplementationSeamRegistration>,
    pub evidence_claims: Vec<EvidenceClaimRegistration>,
    pub subjects: Vec<EvidenceSubjectRegistration>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompiledSpecGraph {
    pub snapshot_id: GraphSnapshotId,
    pub requirements: BTreeMap<RequirementId, RequirementNode>,
    pub slices: BTreeMap<ImplementationSliceId, ImplementationSliceNode>,
    pub seams: BTreeMap<ImplementationSeamId, ImplementationSeamNode>,
    pub evidence_claims: BTreeMap<EvidenceClaimId, EvidenceClaimNode>,
    pub subjects: BTreeMap<EvidenceSubjectId, EvidenceSubjectNode>,
    pub diagnostics: Vec<GraphDiagnostic>,
}

impl CompiledSpecGraph {
    pub fn evidence_for_requirement(
        &self,
        requirement_id: &RequirementId,
    ) -> Vec<&EvidenceClaimNode> {
        self.evidence_claims
            .values()
            .filter(|claim| &claim.requirement_id == requirement_id)
            .collect()
    }

    pub fn subjects_for_evidence(
        &self,
        evidence_id: &EvidenceClaimId,
    ) -> Vec<&EvidenceSubjectNode> {
        self.evidence_claims
            .get(evidence_id)
            .into_iter()
            .flat_map(|claim| claim.subject_ids.iter())
            .filter_map(|subject_id| self.subjects.get(subject_id))
            .collect()
    }

    pub fn related_subjects_for_evidence(
        &self,
        evidence_id: &EvidenceClaimId,
    ) -> Vec<&EvidenceSubjectNode> {
        self.evidence_claims
            .get(evidence_id)
            .into_iter()
            .flat_map(|claim| claim.related_subject_ids.iter())
            .filter_map(|subject_id| self.subjects.get(subject_id))
            .collect()
    }

    pub fn diagnostics_for_slice(&self, slice_id: &ImplementationSliceId) -> Vec<&GraphDiagnostic> {
        let requirement_ids = self
            .slices
            .get(slice_id)
            .map(|slice| {
                slice
                    .requirement_ids
                    .iter()
                    .map(RequirementId::as_str)
                    .collect::<BTreeSet<_>>()
            })
            .unwrap_or_default();
        self.diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.subject == slice_id.as_str()
                    || requirement_ids.contains(diagnostic.subject.as_str())
            })
            .collect()
    }
}

pub fn compile_spec_graph(input: GraphCompileInput) -> CompiledSpecGraph {
    let mut diagnostics = Vec::new();
    let requirements = compile_requirements(input.requirement_graphs, &mut diagnostics);
    let slices = compile_slices(input.implementation_slices, &requirements, &mut diagnostics);
    let seams = compile_seams(input.seams, &mut diagnostics);
    let subjects = compile_subjects(input.subjects, &mut diagnostics);
    let evidence_claims = compile_evidence_claims(
        input.evidence_claims,
        &requirements,
        &slices,
        &seams,
        &subjects,
        &mut diagnostics,
    );
    diagnostics.sort_by(|left, right| {
        left.code
            .cmp(&right.code)
            .then_with(|| left.subject.cmp(&right.subject))
            .then_with(|| left.message.cmp(&right.message))
    });

    let snapshot_id = graph_snapshot_id(
        &requirements,
        &slices,
        &seams,
        &evidence_claims,
        &subjects,
        &diagnostics,
    );
    CompiledSpecGraph {
        snapshot_id,
        requirements,
        slices,
        seams,
        evidence_claims,
        subjects,
        diagnostics,
    }
}

fn compile_requirements(
    graphs: Vec<RequirementGraph>,
    diagnostics: &mut Vec<GraphDiagnostic>,
) -> BTreeMap<RequirementId, RequirementNode> {
    let mut requirements = BTreeMap::new();
    for graph in graphs {
        for requirement in graph.requirements {
            let id = requirement.id.clone();
            let node = RequirementNode {
                id: id.clone(),
                generation: requirement.generation,
                status: requirement.status,
                claim_class: requirement.claim_class,
                source: SourceLocation {
                    path: graph.source.path.clone().unwrap_or_default(),
                    line: Some(graph.source.start_line),
                    symbol: Some(requirement.local_id),
                },
            };
            insert_unique(
                &mut requirements,
                id.clone(),
                node,
                id.as_str(),
                diagnostics,
            );
        }
    }
    requirements
}

fn compile_slices(
    slices_input: Vec<ImplementationSliceV1>,
    requirements: &BTreeMap<RequirementId, RequirementNode>,
    diagnostics: &mut Vec<GraphDiagnostic>,
) -> BTreeMap<ImplementationSliceId, ImplementationSliceNode> {
    let mut slices = BTreeMap::new();
    for slice in slices_input {
        for delta in &slice.requirement_delta {
            match requirements.get(&delta.requirement_id) {
                None => diagnostics.push(GraphDiagnostic::new(
                    GraphDiagnosticCode::UnknownRequirement,
                    delta.requirement_id.as_str(),
                    format!(
                        "slice {} references unknown requirement {}",
                        slice.id.as_str(),
                        delta.requirement_id.as_str()
                    ),
                )),
                Some(requirement) if requirement.generation != delta.requirement_generation => {
                    diagnostics.push(GraphDiagnostic::new(
                        GraphDiagnosticCode::SliceRequirementGenerationMismatch,
                        delta.requirement_id.as_str(),
                        format!(
                            "slice {} uses requirement generation {}, current generation is {}",
                            slice.id.as_str(),
                            delta.requirement_generation,
                            requirement.generation
                        ),
                    ));
                }
                Some(_) => {}
            }
        }
        let id = slice.id.clone();
        let node = ImplementationSliceNode {
            id: id.clone(),
            generation: slice.generation,
            change_class: slice.change_class,
            implementation_claim_status: slice.implementation_claim.status,
            evidence_state: slice.evidence.state,
            support_claim_state: slice.support_claim.state,
            requirement_ids: slice
                .requirement_delta
                .into_iter()
                .map(|delta| delta.requirement_id)
                .collect(),
            owned_seams: slice.owned_seams.into_iter().collect(),
            shared_seams: slice.shared_seams.into_iter().collect(),
            forbidden_seams: slice.forbidden_seams.into_iter().collect(),
        };
        insert_unique(&mut slices, id.clone(), node, id.as_str(), diagnostics);
    }
    slices
}

fn compile_seams(
    registrations: Vec<ImplementationSeamRegistration>,
    diagnostics: &mut Vec<GraphDiagnostic>,
) -> BTreeMap<ImplementationSeamId, ImplementationSeamNode> {
    let mut seams = BTreeMap::new();
    for seam in registrations {
        let id = seam.id.clone();
        let node = ImplementationSeamNode {
            id: id.clone(),
            owner: seam.owner,
            operation: seam.operation,
            source: seam.source,
        };
        insert_unique(&mut seams, id.clone(), node, id.as_str(), diagnostics);
    }
    seams
}

fn compile_subjects(
    registrations: Vec<EvidenceSubjectRegistration>,
    diagnostics: &mut Vec<GraphDiagnostic>,
) -> BTreeMap<EvidenceSubjectId, EvidenceSubjectNode> {
    let mut subjects = BTreeMap::new();
    for subject in registrations {
        let id = subject.id.clone();
        let node = EvidenceSubjectNode {
            id: id.clone(),
            role: subject.role,
            package: subject.package,
            target: subject.target,
            module_path: subject.module_path,
            test_name: subject.test_name,
            source: subject.source,
            source_identity: subject.source_identity,
        };
        insert_unique(&mut subjects, id.clone(), node, id.as_str(), diagnostics);
    }
    subjects
}

fn compile_evidence_claims(
    registrations: Vec<EvidenceClaimRegistration>,
    requirements: &BTreeMap<RequirementId, RequirementNode>,
    slices: &BTreeMap<ImplementationSliceId, ImplementationSliceNode>,
    seams: &BTreeMap<ImplementationSeamId, ImplementationSeamNode>,
    subjects: &BTreeMap<EvidenceSubjectId, EvidenceSubjectNode>,
    diagnostics: &mut Vec<GraphDiagnostic>,
) -> BTreeMap<EvidenceClaimId, EvidenceClaimNode> {
    let mut claims = BTreeMap::new();
    for claim in registrations {
        validate_claim(&claim, requirements, slices, seams, subjects, diagnostics);
        let id = claim.id.clone();
        let node = EvidenceClaimNode {
            id: id.clone(),
            requirement_id: claim.requirement_id,
            slice_id: claim.slice_id,
            seam_id: claim.seam_id,
            purpose: claim.purpose,
            precondition: claim.precondition,
            operation: claim.operation,
            expected_observable: claim.expected_observable,
            discriminator: claim.discriminator,
            claim_boundary: claim.claim_boundary,
            source: claim.source,
            subject_ids: claim.subject_ids,
            related_subject_ids: claim.related_subject_ids,
        };
        insert_unique(&mut claims, id.clone(), node, id.as_str(), diagnostics);
    }
    claims
}

fn validate_claim(
    claim: &EvidenceClaimRegistration,
    requirements: &BTreeMap<RequirementId, RequirementNode>,
    slices: &BTreeMap<ImplementationSliceId, ImplementationSliceNode>,
    seams: &BTreeMap<ImplementationSeamId, ImplementationSeamNode>,
    subjects: &BTreeMap<EvidenceSubjectId, EvidenceSubjectNode>,
    diagnostics: &mut Vec<GraphDiagnostic>,
) {
    if !requirements.contains_key(&claim.requirement_id) {
        diagnostics.push(GraphDiagnostic::new(
            GraphDiagnosticCode::UnknownRequirement,
            claim.requirement_id.as_str(),
            format!(
                "evidence {} references an unknown requirement",
                claim.id.as_str()
            ),
        ));
    }
    let slice = slices.get(&claim.slice_id);
    if slice.is_none() {
        diagnostics.push(GraphDiagnostic::new(
            GraphDiagnosticCode::UnknownSlice,
            claim.slice_id.as_str(),
            format!("evidence {} references an unknown slice", claim.id.as_str()),
        ));
    }
    if !seams.contains_key(&claim.seam_id) {
        diagnostics.push(GraphDiagnostic::new(
            GraphDiagnosticCode::UnknownSeam,
            claim.seam_id.as_str(),
            format!("evidence {} references an unknown seam", claim.id.as_str()),
        ));
    }
    if claim.subject_ids.is_empty() {
        diagnostics.push(GraphDiagnostic::new(
            GraphDiagnosticCode::EmptyEvidenceSubjects,
            claim.id.as_str(),
            "an evidence claim must name at least one exact subject",
        ));
    }
    if let Some(slice) = slice {
        let seam = claim.seam_id.as_str();
        if slice.forbidden_seams.contains(seam) {
            diagnostics.push(GraphDiagnostic::new(
                GraphDiagnosticCode::ForbiddenSeam,
                claim.slice_id.as_str(),
                format!("evidence {} uses forbidden seam {seam}", claim.id.as_str()),
            ));
        } else if !slice.owned_seams.contains(seam) && !slice.shared_seams.contains(seam) {
            diagnostics.push(GraphDiagnostic::new(
                GraphDiagnosticCode::SeamNotDeclaredBySlice,
                claim.slice_id.as_str(),
                format!("evidence {} uses undeclared seam {seam}", claim.id.as_str()),
            ));
        }
    }
    for subject_id in &claim.subject_ids {
        match subjects.get(subject_id) {
            None => diagnostics.push(GraphDiagnostic::new(
                GraphDiagnosticCode::UnknownSubject,
                subject_id.as_str(),
                format!(
                    "evidence {} references an unknown exact subject",
                    claim.id.as_str()
                ),
            )),
            Some(subject) if subject.role != EvidenceSubjectRole::ExactEvidence => {
                diagnostics.push(GraphDiagnostic::new(
                    GraphDiagnosticCode::ExactSubjectMarkedWeak,
                    subject_id.as_str(),
                    format!(
                        "evidence {} maps a weak subject as exact evidence",
                        claim.id.as_str()
                    ),
                ));
            }
            Some(_) => {}
        }
    }
    for subject_id in &claim.related_subject_ids {
        match subjects.get(subject_id) {
            None => diagnostics.push(GraphDiagnostic::new(
                GraphDiagnosticCode::UnknownSubject,
                subject_id.as_str(),
                format!(
                    "evidence {} references an unknown related subject",
                    claim.id.as_str()
                ),
            )),
            Some(subject) if subject.role != EvidenceSubjectRole::RelatedWeak => {
                diagnostics.push(GraphDiagnostic::new(
                    GraphDiagnosticCode::RelatedSubjectMarkedExact,
                    subject_id.as_str(),
                    format!(
                        "evidence {} maps an exact subject as a weak neighbor",
                        claim.id.as_str()
                    ),
                ));
            }
            Some(_) => {}
        }
    }
}

fn insert_unique<K: Ord, V>(
    map: &mut BTreeMap<K, V>,
    key: K,
    value: V,
    subject: &str,
    diagnostics: &mut Vec<GraphDiagnostic>,
) {
    if map.insert(key, value).is_some() {
        diagnostics.push(GraphDiagnostic::new(
            GraphDiagnosticCode::DuplicateId,
            subject,
            format!("duplicate graph id {subject}"),
        ));
    }
}

fn graph_snapshot_id(
    requirements: &BTreeMap<RequirementId, RequirementNode>,
    slices: &BTreeMap<ImplementationSliceId, ImplementationSliceNode>,
    seams: &BTreeMap<ImplementationSeamId, ImplementationSeamNode>,
    claims: &BTreeMap<EvidenceClaimId, EvidenceClaimNode>,
    subjects: &BTreeMap<EvidenceSubjectId, EvidenceSubjectNode>,
    diagnostics: &[GraphDiagnostic],
) -> GraphSnapshotId {
    let mut canonical = String::new();
    for node in requirements.values() {
        let _ = writeln!(
            canonical,
            "requirement|{}|{}|{}|{}|{}",
            node.id.as_str(),
            node.generation,
            requirement_status_name(node.status),
            requirement_claim_class_name(node.claim_class),
            node.source.path
        );
    }
    for node in slices.values() {
        let _ = writeln!(
            canonical,
            "slice|{}|{}|{}|{}|{}|{}",
            node.id.as_str(),
            node.generation,
            slice_class_name(node.change_class),
            implementation_status_name(node.implementation_claim_status),
            evidence_state_name(node.evidence_state),
            support_state_name(node.support_claim_state)
        );
        for requirement_id in &node.requirement_ids {
            let _ = writeln!(
                canonical,
                "slice_requirement|{}|{}",
                node.id.as_str(),
                requirement_id.as_str()
            );
        }
    }
    for node in seams.values() {
        let _ = writeln!(
            canonical,
            "seam|{}|{}|{}|{}",
            node.id.as_str(),
            node.owner,
            node.operation,
            node.source.path
        );
    }
    for node in subjects.values() {
        let _ = writeln!(
            canonical,
            "subject|{}|{}|{}|{}|{}|{}|{}",
            node.id.as_str(),
            node.role.as_str(),
            node.package,
            node.target,
            node.module_path,
            node.test_name,
            node.source_identity
        );
    }
    for node in claims.values() {
        let _ = writeln!(
            canonical,
            "evidence|{}|{}|{}|{}|{}",
            node.id.as_str(),
            node.requirement_id.as_str(),
            node.slice_id.as_str(),
            node.seam_id.as_str(),
            node.purpose.as_str()
        );
        for subject_id in &node.subject_ids {
            let _ = writeln!(
                canonical,
                "evidence_subject|{}|{}",
                node.id.as_str(),
                subject_id.as_str()
            );
        }
        for subject_id in &node.related_subject_ids {
            let _ = writeln!(
                canonical,
                "related_subject|{}|{}",
                node.id.as_str(),
                subject_id.as_str()
            );
        }
    }
    for diagnostic in diagnostics {
        let _ = writeln!(
            canonical,
            "diagnostic|{}|{}|{}",
            diagnostic.code.as_str(),
            diagnostic.subject,
            diagnostic.message
        );
    }
    GraphSnapshotId(stable_hash_hex(&canonical))
}

fn requirement_status_name(status: RequirementStatus) -> &'static str {
    match status {
        RequirementStatus::Draft => "draft",
        RequirementStatus::Accepted => "accepted",
        RequirementStatus::Deferred => "deferred",
        RequirementStatus::Superseded => "superseded",
        RequirementStatus::Rejected => "rejected",
        RequirementStatus::RemovedWithReplacement => "removed_with_replacement",
    }
}

fn requirement_claim_class_name(class: RequirementClaimClass) -> &'static str {
    match class {
        RequirementClaimClass::RuntimeBehavior => "runtime_behavior",
    }
}

fn slice_class_name(class: ImplementationSliceClass) -> &'static str {
    match class {
        ImplementationSliceClass::SpecOrPolicyChange => "spec_or_policy_change",
        ImplementationSliceClass::BehaviorChange => "behavior_change",
    }
}

fn implementation_status_name(status: ImplementationClaimStatus) -> &'static str {
    match status {
        ImplementationClaimStatus::Outstanding => "outstanding",
        ImplementationClaimStatus::Partial => "partial",
        ImplementationClaimStatus::Implemented => "implemented",
        ImplementationClaimStatus::Unsupported => "unsupported",
        ImplementationClaimStatus::NotApplicable => "not_applicable",
        ImplementationClaimStatus::Removed => "removed",
    }
}

fn evidence_state_name(state: EvidenceDispositionState) -> &'static str {
    match state {
        EvidenceDispositionState::Outstanding => "outstanding",
        EvidenceDispositionState::Current => "current",
    }
}

fn support_state_name(state: SupportClaimDispositionState) -> &'static str {
    match state {
        SupportClaimDispositionState::Unchanged => "unchanged",
        SupportClaimDispositionState::Promoted => "promoted",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec_system::{parse_implementation_slice, parse_requirement_blocks};

    const SPEC: &str = r#"---
id: CARGO-ALLOW-SPEC-0009
---

```toml cargo-allow-requirements
schema_version = "1.0"

[[requirement]]
id = "spec-only-runtime-promotion"
generation = 1
status = "accepted"
statement = "A spec-only change cannot publish runtime completion."
claim_class = "runtime_behavior"
```
"#;

    const SLICE: &str = r#"
schema_version = "2.0"
id = "cargo-allow.slice.self-hosted-runtime-promotion.v1"
generation = 1
source_issue = "issue:2206"
design_reference = "CARGO-ALLOW-SPEC-0009#spec-only-runtime-promotion"
change_class = "spec_or_policy_change"
claim_boundary = "No runtime completion claim."
owned_seams = ["seam:runtime-promotion"]

[[requirement_delta]]
requirement_id = "CARGO-ALLOW-SPEC-0009#spec-only-runtime-promotion"
requirement_generation = 1

[implementation_claim]
status = "outstanding"

[evidence]
state = "outstanding"

[support_claim]
state = "unchanged"
"#;

    fn input() -> Result<GraphCompileInput, String> {
        let requirement_graph =
            parse_requirement_blocks(SPEC).map_err(|error| error.to_string())?;
        let slice = parse_implementation_slice(SLICE).map_err(|error| error.to_string())?;
        let exact = EvidenceSubjectId("subject:exact-negative".into());
        let weak = EvidenceSubjectId("subject:weak-neighbor".into());
        Ok(GraphCompileInput {
            requirement_graphs: vec![requirement_graph],
            implementation_slices: vec![slice],
            seams: vec![ImplementationSeamRegistration {
                id: ImplementationSeamId("seam:runtime-promotion".into()),
                owner: "allow-policy".into(),
                operation: "validate runtime promotion".into(),
                source: SourceLocation::new(
                    "crates/allow-policy/src/spec_system/runtime_promotion.rs",
                )
                .with_symbol("validate_runtime_promotion"),
            }],
            subjects: vec![
                EvidenceSubjectRegistration {
                    id: exact.clone(),
                    role: EvidenceSubjectRole::ExactEvidence,
                    package: "allow-policy".into(),
                    target: "lib".into(),
                    module_path: "spec_system::runtime_promotion::tests".into(),
                    test_name: "spec_or_policy_slice_rejects_unproved_runtime_promotion".into(),
                    source: SourceLocation::new(
                        "crates/allow-policy/src/spec_system/runtime_promotion.rs",
                    ),
                    source_identity: "fnv1a64:exact".into(),
                },
                EvidenceSubjectRegistration {
                    id: weak.clone(),
                    role: EvidenceSubjectRole::RelatedWeak,
                    package: "allow-policy".into(),
                    target: "lib".into(),
                    module_path: "spec_system::runtime_promotion::tests".into(),
                    test_name: "spec_or_policy_slice_rejects_invalid_transition_broadly".into(),
                    source: SourceLocation::new(
                        "crates/allow-policy/src/spec_system/runtime_promotion.rs",
                    ),
                    source_identity: "fnv1a64:weak".into(),
                },
            ],
            evidence_claims: vec![EvidenceClaimRegistration {
                id: EvidenceClaimId("evidence:forbidden-runtime-promotion".into()),
                requirement_id: RequirementId(
                    "CARGO-ALLOW-SPEC-0009#spec-only-runtime-promotion".into(),
                ),
                slice_id: ImplementationSliceId(
                    "cargo-allow.slice.self-hosted-runtime-promotion.v1".into(),
                ),
                seam_id: ImplementationSeamId("seam:runtime-promotion".into()),
                purpose: EvidencePurpose::ForbiddenRuntimePromotion,
                precondition: "spec-only runtime requirement".into(),
                operation: "claim implemented".into(),
                expected_observable: "typed rejection".into(),
                discriminator: "SpecOnlyRuntimeImplementationClaim".into(),
                claim_boundary: "Does not prove execution.".into(),
                source: SourceLocation::new(
                    "docs/specs/CARGO-ALLOW-SPEC-0009-design-to-proof-walking-skeleton.md",
                ),
                subject_ids: vec![exact],
                related_subject_ids: vec![weak],
            }],
        })
    }

    #[test]
    fn compiles_minimal_claim_graph_deterministically() -> Result<(), String> {
        let first = compile_spec_graph(input()?);
        let second = compile_spec_graph(input()?);
        assert!(first.diagnostics.is_empty());
        assert_eq!(first.snapshot_id, second.snapshot_id);
        assert_eq!(first.requirements.len(), 1);
        assert_eq!(first.slices.len(), 1);
        assert_eq!(first.seams.len(), 1);
        assert_eq!(first.evidence_claims.len(), 1);
        assert_eq!(first.subjects.len(), 2);
        let evidence_id = EvidenceClaimId("evidence:forbidden-runtime-promotion".into());
        assert_eq!(first.subjects_for_evidence(&evidence_id).len(), 1);
        assert_eq!(first.related_subjects_for_evidence(&evidence_id).len(), 1);
        Ok(())
    }

    #[test]
    fn reports_weak_subject_mapped_as_exact() -> Result<(), String> {
        let mut input = input()?;
        let claim = input
            .evidence_claims
            .first_mut()
            .ok_or_else(|| "expected evidence claim".to_string())?;
        claim.subject_ids = claim.related_subject_ids.clone();
        let graph = compile_spec_graph(input);
        assert!(
            graph.diagnostics.iter().any(|diagnostic| {
                diagnostic.code == GraphDiagnosticCode::ExactSubjectMarkedWeak
            })
        );
        Ok(())
    }

    #[test]
    fn reports_undeclared_and_forbidden_seams() -> Result<(), String> {
        let mut undeclared = input()?;
        let slice = undeclared
            .implementation_slices
            .first_mut()
            .ok_or_else(|| "expected implementation slice".to_string())?;
        slice.owned_seams.clear();
        let graph = compile_spec_graph(undeclared);
        assert!(
            graph.diagnostics.iter().any(|diagnostic| {
                diagnostic.code == GraphDiagnosticCode::SeamNotDeclaredBySlice
            })
        );

        let mut forbidden = input()?;
        let slice = forbidden
            .implementation_slices
            .first_mut()
            .ok_or_else(|| "expected implementation slice".to_string())?;
        slice.forbidden_seams.push("seam:runtime-promotion".into());
        let graph = compile_spec_graph(forbidden);
        assert!(
            graph
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.code == GraphDiagnosticCode::ForbiddenSeam })
        );
        Ok(())
    }
}
