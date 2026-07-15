use allow_core::{normalize_path, stable_hash_hex};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use super::{
    ImplementationSliceClass, ImplementationSliceId, ImplementationSliceV1, RequirementDelta,
    RequirementGraph, RequirementId, RequirementLifecycle,
};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GraphSnapshotId(pub String);

impl GraphSnapshotId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

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

    pub fn with_symbol(mut self, symbol: impl Into<String>) -> Self {
        self.symbol = Some(symbol.into());
        self
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ImplementationSeamId(pub String);

impl ImplementationSeamId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EvidenceClaimId(pub String);

impl EvidenceClaimId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RustTestSubjectId(pub String);

impl RustTestSubjectId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProofCommandId(pub String);

impl ProofCommandId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidencePurpose {
    PositiveAcceptance,
    ForbiddenRuntimePromotion,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestSubjectRole {
    ExactEvidence,
    RelatedWeak,
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
pub struct EvidenceClaimRegistration {
    pub id: EvidenceClaimId,
    pub requirement_id: RequirementId,
    pub seam_id: ImplementationSeamId,
    pub purpose: EvidencePurpose,
    pub precondition: String,
    pub operation: String,
    pub expected_observable: String,
    pub discriminator: String,
    pub claim_boundary: String,
    pub source: SourceLocation,
    pub subject_ids: Vec<RustTestSubjectId>,
    pub proof_command_id: ProofCommandId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RustTestSubjectRegistration {
    pub id: RustTestSubjectId,
    pub role: TestSubjectRole,
    pub package: String,
    pub target: String,
    pub module_path: String,
    pub test_name: String,
    pub source: SourceLocation,
    pub source_identity: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofCommandRegistration {
    pub id: ProofCommandId,
    pub program: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub subject_ids: Vec<RustTestSubjectId>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequirementNode {
    pub id: RequirementId,
    pub generation: u32,
    pub lifecycle: RequirementLifecycle,
    pub source: SourceLocation,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImplementationSliceNode {
    pub id: ImplementationSliceId,
    pub generation: u32,
    pub change_class: ImplementationSliceClass,
    pub basis: String,
    pub requirement_delta: Vec<RequirementDelta>,
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
pub struct EvidenceClaimNode {
    pub id: EvidenceClaimId,
    pub requirement_id: RequirementId,
    pub seam_id: ImplementationSeamId,
    pub purpose: EvidencePurpose,
    pub precondition: String,
    pub operation: String,
    pub expected_observable: String,
    pub discriminator: String,
    pub claim_boundary: String,
    pub source: SourceLocation,
    pub subject_ids: Vec<RustTestSubjectId>,
    pub proof_command_id: ProofCommandId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RustTestSubjectNode {
    pub id: RustTestSubjectId,
    pub role: TestSubjectRole,
    pub package: String,
    pub target: String,
    pub module_path: String,
    pub test_name: String,
    pub source: SourceLocation,
    pub source_identity: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofCommandNode {
    pub id: ProofCommandId,
    pub program: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub subject_ids: Vec<RustTestSubjectId>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphDiagnosticCode {
    DuplicateId,
    UnknownRequirement,
    UnknownSeam,
    UnknownSubject,
    UnknownProofCommand,
    EmptyEvidenceSubjects,
    WeakSubjectMappedAsExactEvidence,
    SliceRequirementGenerationMismatch,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphDiagnostic {
    pub code: GraphDiagnosticCode,
    pub subject: String,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompiledSpecGraph {
    pub snapshot_id: GraphSnapshotId,
    pub requirements: BTreeMap<RequirementId, RequirementNode>,
    pub slices: BTreeMap<ImplementationSliceId, ImplementationSliceNode>,
    pub seams: BTreeMap<ImplementationSeamId, ImplementationSeamNode>,
    pub evidence_claims: BTreeMap<EvidenceClaimId, EvidenceClaimNode>,
    pub test_subjects: BTreeMap<RustTestSubjectId, RustTestSubjectNode>,
    pub proof_commands: BTreeMap<ProofCommandId, ProofCommandNode>,
    pub diagnostics: Vec<GraphDiagnostic>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphCompileInput {
    pub requirement_graphs: Vec<RequirementGraph>,
    pub implementation_slices: Vec<ImplementationSliceV1>,
    pub seams: Vec<ImplementationSeamRegistration>,
    pub evidence_claims: Vec<EvidenceClaimRegistration>,
    pub test_subjects: Vec<RustTestSubjectRegistration>,
    pub proof_commands: Vec<ProofCommandRegistration>,
}

impl CompiledSpecGraph {
    pub fn requirement_deltas_for_slice(
        &self,
        slice_id: &ImplementationSliceId,
    ) -> Option<&[RequirementDelta]> {
        self.slices
            .get(slice_id)
            .map(|slice| slice.requirement_delta.as_slice())
    }

    pub fn seam_for_requirement(
        &self,
        requirement_id: &RequirementId,
    ) -> Option<&ImplementationSeamNode> {
        self.evidence_claims
            .values()
            .find(|claim| &claim.requirement_id == requirement_id)
            .and_then(|claim| self.seams.get(&claim.seam_id))
    }

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
    ) -> Vec<&RustTestSubjectNode> {
        self.evidence_claims
            .get(evidence_id)
            .into_iter()
            .flat_map(|claim| claim.subject_ids.iter())
            .filter_map(|subject_id| self.test_subjects.get(subject_id))
            .collect()
    }

    pub fn evidence_for_subject(
        &self,
        subject_id: &RustTestSubjectId,
    ) -> Vec<&EvidenceClaimNode> {
        self.evidence_claims
            .values()
            .filter(|claim| claim.subject_ids.contains(subject_id))
            .collect()
    }

    pub fn diagnostics_for_slice(
        &self,
        slice_id: &ImplementationSliceId,
    ) -> Vec<&GraphDiagnostic> {
        let Some(slice) = self.slices.get(slice_id) else {
            return Vec::new();
        };
        let requirement_ids = slice
            .requirement_delta
            .iter()
            .map(|delta| delta.requirement_id.as_str())
            .collect::<BTreeSet<_>>();
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
    let mut requirements = BTreeMap::new();
    for graph in input.requirement_graphs {
        for requirement in graph.requirements {
            let node = RequirementNode {
                id: requirement.id.clone(),
                generation: requirement.generation,
                lifecycle: requirement.lifecycle,
                source: SourceLocation {
                    path: graph.source.path.clone().unwrap_or_default(),
                    line: Some(graph.source.start_line),
                    symbol: Some(requirement.local_id),
                },
            };
            insert_unique(
                &mut requirements,
                requirement.id,
                node,
                &mut diagnostics,
            );
        }
    }

    let mut slices = BTreeMap::new();
    for slice in input.implementation_slices {
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
        let node = ImplementationSliceNode {
            id: slice.id.clone(),
            generation: slice.generation,
            change_class: slice.change_class,
            basis: slice.basis,
            requirement_delta: slice.requirement_delta,
        };
        insert_unique(&mut slices, slice.id, node, &mut diagnostics);
    }

    let mut seams = BTreeMap::new();
    for seam in input.seams {
        let node = ImplementationSeamNode {
            id: seam.id.clone(),
            owner: seam.owner,
            operation: seam.operation,
            source: seam.source,
        };
        insert_unique(&mut seams, seam.id, node, &mut diagnostics);
    }

    let mut test_subjects = BTreeMap::new();
    for subject in input.test_subjects {
        let node = RustTestSubjectNode {
            id: subject.id.clone(),
            role: subject.role,
            package: subject.package,
            target: subject.target,
            module_path: subject.module_path,
            test_name: subject.test_name,
            source: subject.source,
            source_identity: subject.source_identity,
        };
        insert_unique(&mut test_subjects, subject.id, node, &mut diagnostics);
    }

    let mut proof_commands = BTreeMap::new();
    for command in input.proof_commands {
        let node = ProofCommandNode {
            id: command.id.clone(),
            program: command.program,
            args: command.args,
            cwd: normalize_path(command.cwd),
            subject_ids: command.subject_ids,
        };
        insert_unique(&mut proof_commands, command.id, node, &mut diagnostics);
    }

    let mut evidence_claims = BTreeMap::new();
    for claim in input.evidence_claims {
        validate_evidence_claim(
            &claim,
            &requirements,
            &seams,
            &test_subjects,
            &proof_commands,
            &mut diagnostics,
        );
        let node = EvidenceClaimNode {
            id: claim.id.clone(),
            requirement_id: claim.requirement_id,
            seam_id: claim.seam_id,
            purpose: claim.purpose,
            precondition: claim.precondition,
            operation: claim.operation,
            expected_observable: claim.expected_observable,
            discriminator: claim.discriminator,
            claim_boundary: claim.claim_boundary,
            source: claim.source,
            subject_ids: claim.subject_ids,
            proof_command_id: claim.proof_command_id,
        };
        insert_unique(&mut evidence_claims, claim.id, node, &mut diagnostics);
    }

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
        &test_subjects,
        &proof_commands,
        &diagnostics,
    );

    CompiledSpecGraph {
        snapshot_id,
        requirements,
        slices,
        seams,
        evidence_claims,
        test_subjects,
        proof_commands,
        diagnostics,
    }
}

fn insert_unique<K, V>(
    target: &mut BTreeMap<K, V>,
    key: K,
    value: V,
    diagnostics: &mut Vec<GraphDiagnostic>,
) where
    K: Ord + Clone + ToString,
{
    let subject = key.to_string();
    if target.insert(key, value).is_some() {
        diagnostics.push(GraphDiagnostic::new(
            GraphDiagnosticCode::DuplicateId,
            subject.clone(),
            format!("duplicate graph id {subject}"),
        ));
    }
}

fn validate_evidence_claim(
    claim: &EvidenceClaimRegistration,
    requirements: &BTreeMap<RequirementId, RequirementNode>,
    seams: &BTreeMap<ImplementationSeamId, ImplementationSeamNode>,
    subjects: &BTreeMap<RustTestSubjectId, RustTestSubjectNode>,
    commands: &BTreeMap<ProofCommandId, ProofCommandNode>,
    diagnostics: &mut Vec<GraphDiagnostic>,
) {
    if !requirements.contains_key(&claim.requirement_id) {
        diagnostics.push(GraphDiagnostic::new(
            GraphDiagnosticCode::UnknownRequirement,
            claim.id.as_str(),
            format!(
                "evidence {} references unknown requirement {}",
                claim.id.as_str(),
                claim.requirement_id.as_str()
            ),
        ));
    }
    if !seams.contains_key(&claim.seam_id) {
        diagnostics.push(GraphDiagnostic::new(
            GraphDiagnosticCode::UnknownSeam,
            claim.id.as_str(),
            format!(
                "evidence {} references unknown seam {}",
                claim.id.as_str(),
                claim.seam_id.as_str()
            ),
        ));
    }
    if claim.subject_ids.is_empty() {
        diagnostics.push(GraphDiagnostic::new(
            GraphDiagnosticCode::EmptyEvidenceSubjects,
            claim.id.as_str(),
            format!("evidence {} selects no test subjects", claim.id.as_str()),
        ));
    }
    for subject_id in &claim.subject_ids {
        match subjects.get(subject_id) {
            None => diagnostics.push(GraphDiagnostic::new(
                GraphDiagnosticCode::UnknownSubject,
                claim.id.as_str(),
                format!(
                    "evidence {} references unknown subject {}",
                    claim.id.as_str(),
                    subject_id.as_str()
                ),
            )),
            Some(subject) if subject.role == TestSubjectRole::RelatedWeak => {
                diagnostics.push(GraphDiagnostic::new(
                    GraphDiagnosticCode::WeakSubjectMappedAsExactEvidence,
                    claim.id.as_str(),
                    format!(
                        "evidence {} maps related weak subject {} as exact evidence",
                        claim.id.as_str(),
                        subject_id.as_str()
                    ),
                ));
            }
            Some(_) => {}
        }
    }
    if !commands.contains_key(&claim.proof_command_id) {
        diagnostics.push(GraphDiagnostic::new(
            GraphDiagnosticCode::UnknownProofCommand,
            claim.id.as_str(),
            format!(
                "evidence {} references unknown proof command {}",
                claim.id.as_str(),
                claim.proof_command_id.as_str()
            ),
        ));
    }
}

fn graph_snapshot_id(
    requirements: &BTreeMap<RequirementId, RequirementNode>,
    slices: &BTreeMap<ImplementationSliceId, ImplementationSliceNode>,
    seams: &BTreeMap<ImplementationSeamId, ImplementationSeamNode>,
    evidence_claims: &BTreeMap<EvidenceClaimId, EvidenceClaimNode>,
    test_subjects: &BTreeMap<RustTestSubjectId, RustTestSubjectNode>,
    proof_commands: &BTreeMap<ProofCommandId, ProofCommandNode>,
    diagnostics: &[GraphDiagnostic],
) -> GraphSnapshotId {
    let mut identity = String::new();
    for requirement in requirements.values() {
        identity.push_str(&format!(
            "requirement|{}|{}|{:?}\n",
            requirement.id.as_str(),
            requirement.generation,
            requirement.lifecycle
        ));
    }
    for slice in slices.values() {
        identity.push_str(&format!(
            "slice|{}|{}|{:?}|{}\n",
            slice.id.as_str(),
            slice.generation,
            slice.change_class,
            slice.basis
        ));
    }
    for seam in seams.values() {
        identity.push_str(&format!(
            "seam|{}|{}|{}\n",
            seam.id.as_str(),
            seam.owner,
            seam.operation
        ));
    }
    for claim in evidence_claims.values() {
        identity.push_str(&format!(
            "evidence|{}|{}|{}|{:?}|{}\n",
            claim.id.as_str(),
            claim.requirement_id.as_str(),
            claim.seam_id.as_str(),
            claim.purpose,
            claim.discriminator
        ));
        for subject_id in &claim.subject_ids {
            identity.push_str(&format!("selected_by|{}\n", subject_id.as_str()));
        }
    }
    for subject in test_subjects.values() {
        identity.push_str(&format!(
            "subject|{}|{:?}|{}|{}|{}|{}\n",
            subject.id.as_str(),
            subject.role,
            subject.package,
            subject.target,
            subject.module_path,
            subject.test_name
        ));
    }
    for command in proof_commands.values() {
        identity.push_str(&format!(
            "command|{}|{}|{}|{}\n",
            command.id.as_str(),
            command.program,
            command.args.join("\u{1f}"),
            command.cwd
        ));
    }
    for diagnostic in diagnostics {
        identity.push_str(&format!(
            "diagnostic|{:?}|{}|{}\n",
            diagnostic.code, diagnostic.subject, diagnostic.message
        ));
    }
    GraphSnapshotId(stable_hash_hex(&identity))
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

impl ToString for ImplementationSliceId {
    fn to_string(&self) -> String {
        self.as_str().to_string()
    }
}

impl ToString for RequirementId {
    fn to_string(&self) -> String {
        self.as_str().to_string()
    }
}

impl ToString for ImplementationSeamId {
    fn to_string(&self) -> String {
        self.as_str().to_string()
    }
}

impl ToString for EvidenceClaimId {
    fn to_string(&self) -> String {
        self.as_str().to_string()
    }
}

impl ToString for RustTestSubjectId {
    fn to_string(&self) -> String {
        self.as_str().to_string()
    }
}

impl ToString for ProofCommandId {
    fn to_string(&self) -> String {
        self.as_str().to_string()
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
lifecycle = "accepted"
statement = "A spec-only slice cannot promote runtime state without closure."
claim_class = "runtime_behavior"
```
"#;

    const SLICE: &str = r#"
schema_version = "1.0"
id = "cargo-allow.slice.self-hosted-runtime-promotion.v1"
generation = 1
source_issue = "issue:2206"
design_reference = "CARGO-ALLOW-SPEC-0009#spec-only-runtime-promotion"
change_class = "spec_or_policy_change"
basis = "git:example-head"
claim_boundary = "No runtime implementation or proof claim."

[[requirement_delta]]
requirement_id = "CARGO-ALLOW-SPEC-0009#spec-only-runtime-promotion"
requirement_generation = 1
runtime = true
from = "accepted"
to = "accepted"

[implementation]
state = "outstanding"

[evidence]
state = "outstanding"

[support_claim]
state = "unchanged"
"#;

    fn requirement_id() -> RequirementId {
        RequirementId("CARGO-ALLOW-SPEC-0009#spec-only-runtime-promotion".to_string())
    }

    fn seam_id() -> ImplementationSeamId {
        ImplementationSeamId("seam:allow-policy:runtime-promotion".to_string())
    }

    fn positive_subject_id() -> RustTestSubjectId {
        RustTestSubjectId("rust-test:allow-policy:spec-or-policy-accepted".to_string())
    }

    fn negative_subject_id() -> RustTestSubjectId {
        RustTestSubjectId("rust-test:allow-policy:forbidden-runtime-promotion".to_string())
    }

    fn weak_subject_id() -> RustTestSubjectId {
        RustTestSubjectId("rust-test:allow-policy:broad-invalid-transition".to_string())
    }

    fn compile_input() -> Result<GraphCompileInput, String> {
        let requirements = parse_requirement_blocks(SPEC).map_err(|error| error.to_string())?;
        let slice = parse_implementation_slice(SLICE).map_err(|error| error.to_string())?;
        let source = SourceLocation::new("crates/allow-policy/src/spec_system/runtime_promotion.rs");
        let seam = ImplementationSeamRegistration {
            id: seam_id(),
            owner: "allow-policy::spec_system".to_string(),
            operation: "validate_runtime_promotion".to_string(),
            source: source.clone().with_symbol("validate_runtime_promotion"),
        };
        let positive = RustTestSubjectRegistration {
            id: positive_subject_id(),
            role: TestSubjectRole::ExactEvidence,
            package: "allow-policy".to_string(),
            target: "lib".to_string(),
            module_path: "spec_system::runtime_promotion::tests".to_string(),
            test_name: "spec_or_policy_slice_keeps_runtime_requirement_accepted".to_string(),
            source: source.clone().with_symbol(
                "spec_or_policy_slice_keeps_runtime_requirement_accepted",
            ),
            source_identity: "source:positive-v1".to_string(),
        };
        let negative = RustTestSubjectRegistration {
            id: negative_subject_id(),
            role: TestSubjectRole::ExactEvidence,
            package: "allow-policy".to_string(),
            target: "lib".to_string(),
            module_path: "spec_system::runtime_promotion::tests".to_string(),
            test_name: "spec_or_policy_slice_rejects_unproved_runtime_promotion".to_string(),
            source: source.clone().with_symbol(
                "spec_or_policy_slice_rejects_unproved_runtime_promotion",
            ),
            source_identity: "source:negative-v1".to_string(),
        };
        let weak = RustTestSubjectRegistration {
            id: weak_subject_id(),
            role: TestSubjectRole::RelatedWeak,
            package: "allow-policy".to_string(),
            target: "lib".to_string(),
            module_path: "spec_system::runtime_promotion::tests".to_string(),
            test_name: "spec_or_policy_slice_rejects_invalid_transition_broadly".to_string(),
            source: source.with_symbol("spec_or_policy_slice_rejects_invalid_transition_broadly"),
            source_identity: "source:weak-v1".to_string(),
        };
        let proof_command_id = ProofCommandId("command:allow-policy:self-hosted".to_string());
        let positive_evidence_id = EvidenceClaimId("evidence:positive-acceptance".to_string());
        let negative_evidence_id = EvidenceClaimId("evidence:forbidden-promotion".to_string());

        Ok(GraphCompileInput {
            requirement_graphs: vec![requirements],
            implementation_slices: vec![slice],
            seams: vec![seam],
            evidence_claims: vec![
                EvidenceClaimRegistration {
                    id: positive_evidence_id,
                    requirement_id: requirement_id(),
                    seam_id: seam_id(),
                    purpose: EvidencePurpose::PositiveAcceptance,
                    precondition: "spec or policy slice leaves runtime work outstanding".to_string(),
                    operation: "validate runtime promotion".to_string(),
                    expected_observable: "accepted transition without runtime promotion".to_string(),
                    discriminator: "requirement remains accepted and support unchanged".to_string(),
                    claim_boundary: "structural validator behavior only".to_string(),
                    source: SourceLocation::new("graph:self-hosted").with_symbol("positive"),
                    subject_ids: vec![positive_subject_id()],
                    proof_command_id: proof_command_id.clone(),
                },
                EvidenceClaimRegistration {
                    id: negative_evidence_id,
                    requirement_id: requirement_id(),
                    seam_id: seam_id(),
                    purpose: EvidencePurpose::ForbiddenRuntimePromotion,
                    precondition: "spec or policy slice attempts runtime implementation".to_string(),
                    operation: "validate runtime promotion".to_string(),
                    expected_observable: "exact RuntimeImplementationWithoutDisposition finding"
                        .to_string(),
                    discriminator: "typed finding and unchanged requirement lifecycle".to_string(),
                    claim_boundary: "does not prove runtime execution".to_string(),
                    source: SourceLocation::new("graph:self-hosted").with_symbol("negative"),
                    subject_ids: vec![negative_subject_id()],
                    proof_command_id: proof_command_id.clone(),
                },
            ],
            test_subjects: vec![positive, negative, weak],
            proof_commands: vec![ProofCommandRegistration {
                id: proof_command_id,
                program: "cargo".to_string(),
                args: vec![
                    "test".to_string(),
                    "-p".to_string(),
                    "allow-policy".to_string(),
                    "spec_or_policy_slice_".to_string(),
                ],
                cwd: ".".to_string(),
                subject_ids: vec![positive_subject_id(), negative_subject_id()],
            }],
        })
    }

    #[test]
    fn compiles_self_hosted_requirement_to_exact_tests() -> Result<(), String> {
        let graph = compile_spec_graph(compile_input()?);
        let evidence = graph.evidence_for_requirement(&requirement_id());

        assert!(graph.diagnostics.is_empty());
        assert_eq!(evidence.len(), 2);
        assert_eq!(
            graph
                .seam_for_requirement(&requirement_id())
                .map(|seam| seam.id.clone()),
            Some(seam_id())
        );
        assert_eq!(
            graph.subjects_for_evidence(&EvidenceClaimId(
                "evidence:forbidden-promotion".to_string()
            )),
            vec![graph
                .test_subjects
                .get(&negative_subject_id())
                .ok_or_else(|| "expected exact negative subject".to_string())?]
        );
        assert!(
            graph
                .evidence_for_subject(&weak_subject_id())
                .is_empty()
        );
        assert!(graph.snapshot_id.as_str().starts_with("fnv1a64:"));
        Ok(())
    }

    #[test]
    fn weak_subject_cannot_satisfy_exact_negative_evidence() -> Result<(), String> {
        let mut input = compile_input()?;
        let negative = input
            .evidence_claims
            .iter_mut()
            .find(|claim| claim.purpose == EvidencePurpose::ForbiddenRuntimePromotion)
            .ok_or_else(|| "expected negative evidence claim".to_string())?;
        negative.subject_ids = vec![weak_subject_id()];

        let graph = compile_spec_graph(input);

        assert!(graph.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == GraphDiagnosticCode::WeakSubjectMappedAsExactEvidence
        }));
        Ok(())
    }

    #[test]
    fn missing_exact_subject_is_visible_before_execution() -> Result<(), String> {
        let mut input = compile_input()?;
        input
            .test_subjects
            .retain(|subject| subject.id != negative_subject_id());

        let graph = compile_spec_graph(input);

        assert!(graph.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == GraphDiagnosticCode::UnknownSubject
                && diagnostic.message.contains(negative_subject_id().as_str())
        }));
        Ok(())
    }

    #[test]
    fn graph_identity_is_independent_of_input_order() -> Result<(), String> {
        let first = compile_spec_graph(compile_input()?);
        let mut reordered = compile_input()?;
        reordered.evidence_claims.reverse();
        reordered.test_subjects.reverse();
        reordered.proof_commands.reverse();
        let second = compile_spec_graph(reordered);

        assert_eq!(first.snapshot_id, second.snapshot_id);
        Ok(())
    }
}
