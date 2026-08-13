//! Authored seam/evidence mapping DTOs (#2584-B).

use serde::{Deserialize, Serialize};

use super::implementation_slice::ImplementationSliceId;
use super::requirement::RequirementId;

pub const AUTHORED_MAPPING_SCHEMA_VERSION: &str = "1.0";

// Shared ID types previously in compiled_graph.rs (#3304).
// Moved here because authored_mapping is the only consumer.

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
            path: allow_core::normalize_path(path.into()),
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

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EvidenceSubjectId(pub String);

impl EvidenceSubjectId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceSubjectRole {
    ExactEvidence,
    RelatedWeak,
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
pub struct AuthoredSeamSource {
    pub schema_version: String,
    #[serde(default)]
    pub seam: Vec<AuthoredSeam>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredSeam {
    pub id: ImplementationSeamId,
    pub generation: u32,
    pub owner: String,
    pub operation: String,
    pub source: SourceLocation,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredEvidenceSource {
    pub schema_version: String,
    #[serde(default)]
    pub evidence: Vec<AuthoredEvidenceClaim>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredEvidenceClaim {
    pub id: EvidenceClaimId,
    pub requirement_id: RequirementId,
    pub requirement_generation: u32,
    pub slice_id: ImplementationSliceId,
    pub slice_generation: u32,
    pub seam_id: ImplementationSeamId,
    pub purpose: EvidencePurpose,
    pub precondition: String,
    pub operation: String,
    pub expected_observable: String,
    pub discriminator: String,
    pub claim_boundary: String,
    pub source: SourceLocation,
    pub subject: Vec<AuthoredSubjectSelector>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredSubjectSelector {
    pub id: String,
    pub role: AuthoredSubjectRole,
    pub package: String,
    pub target: String,
    pub module_path: String,
    pub test_name: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthoredSubjectRole {
    ExactEvidence,
    RelatedWeak,
}

use super::implementation_slice::ImplementationSliceV1;
use super::requirement::RequirementGraph;

use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult, normalize_path};
use std::collections::BTreeSet;
use std::path::Path;

pub fn parse_authored_seams(input: &str) -> CargoAllowResult<AuthoredSeamSource> {
    parse_authored_seams_at(None, input)
}

pub fn parse_authored_seams_at(
    path: Option<&Path>,
    input: &str,
) -> CargoAllowResult<AuthoredSeamSource> {
    let mut source = parse_toml::<AuthoredSeamSource>(path, input, "seam source")?;
    validate_schema(path, &source.schema_version, "seam source")?;
    if source.seam.is_empty() {
        return Err(invalid_mapping(
            path,
            "seam source must declare at least one seam",
        ));
    }
    let mut ids = BTreeSet::new();
    for seam in &mut source.seam {
        validate_seam(path, seam)?;
        if !ids.insert(seam.id.clone()) {
            return Err(invalid_mapping(
                path,
                format!("duplicate authored seam id {}", seam.id.as_str()),
            ));
        }
    }
    Ok(source)
}

pub fn parse_authored_evidence(input: &str) -> CargoAllowResult<AuthoredEvidenceSource> {
    parse_authored_evidence_at(None, input)
}

pub fn parse_authored_evidence_at(
    path: Option<&Path>,
    input: &str,
) -> CargoAllowResult<AuthoredEvidenceSource> {
    let mut source = parse_toml::<AuthoredEvidenceSource>(path, input, "evidence source")?;
    validate_schema(path, &source.schema_version, "evidence source")?;
    if source.evidence.is_empty() {
        return Err(invalid_mapping(
            path,
            "evidence source must declare at least one evidence claim",
        ));
    }
    let mut claim_ids = BTreeSet::new();
    let mut subject_ids = BTreeSet::new();
    for claim in &mut source.evidence {
        validate_claim(path, claim)?;
        if !claim_ids.insert(claim.id.clone()) {
            return Err(invalid_mapping(
                path,
                format!("duplicate authored evidence id {}", claim.id.as_str()),
            ));
        }
        for subject in &claim.subject {
            if !subject_ids.insert(subject.id.as_str()) {
                return Err(invalid_mapping(
                    path,
                    format!("duplicate authored subject id {}", subject.id),
                ));
            }
        }
    }
    Ok(source)
}

pub fn validate_authored_mapping(
    requirements: &RequirementGraph,
    slice: &ImplementationSliceV1,
    seams: &AuthoredSeamSource,
    evidence: &AuthoredEvidenceSource,
) -> CargoAllowResult<()> {
    let requirement_generations = requirements
        .requirements
        .iter()
        .map(|requirement| (requirement.id.clone(), requirement.generation))
        .collect::<std::collections::BTreeMap<_, _>>();
    let slice_requirements = slice
        .requirement_delta
        .iter()
        .map(|delta| (delta.requirement_id.clone(), delta.requirement_generation))
        .collect::<std::collections::BTreeMap<_, _>>();
    let seam_ids = seams
        .seam
        .iter()
        .map(|seam| seam.id.clone())
        .collect::<BTreeSet<_>>();

    for seam in &seams.seam {
        let seam_name = seam.id.as_str();
        if slice.forbidden_seams.iter().any(|item| item == seam_name) {
            return Err(CargoAllowError::new(format!(
                "authored seam {seam_name} is forbidden by slice {}",
                slice.id.as_str()
            )));
        }
        if !slice
            .owned_seams
            .iter()
            .chain(&slice.shared_seams)
            .any(|item| item == seam_name)
        {
            return Err(CargoAllowError::new(format!(
                "authored seam {seam_name} is not declared by slice {}",
                slice.id.as_str()
            )));
        }
    }

    for claim in &evidence.evidence {
        if !seam_ids.contains(&claim.seam_id) {
            return Err(CargoAllowError::new(format!(
                "authored evidence {} references unknown seam {}",
                claim.id.as_str(),
                claim.seam_id.as_str()
            )));
        }
        if claim.slice_id != slice.id || claim.slice_generation != slice.generation {
            return Err(CargoAllowError::new(format!(
                "authored evidence {} does not match slice {} generation {}",
                claim.id.as_str(),
                slice.id.as_str(),
                slice.generation
            )));
        }
        match requirement_generations.get(&claim.requirement_id) {
            Some(generation) if *generation == claim.requirement_generation => {}
            Some(generation) => {
                return Err(CargoAllowError::new(format!(
                    "authored evidence {} uses requirement generation {}, current generation is {}",
                    claim.id.as_str(),
                    claim.requirement_generation,
                    generation
                )));
            }
            None => {
                return Err(CargoAllowError::new(format!(
                    "authored evidence {} references unknown requirement {}",
                    claim.id.as_str(),
                    claim.requirement_id.as_str()
                )));
            }
        }
        match slice_requirements.get(&claim.requirement_id) {
            Some(generation) if *generation == claim.requirement_generation => {}
            Some(generation) => {
                return Err(CargoAllowError::new(format!(
                    "authored evidence {} uses slice requirement generation {}, slice declares {}",
                    claim.id.as_str(),
                    claim.requirement_generation,
                    generation
                )));
            }
            None => {
                return Err(CargoAllowError::new(format!(
                    "authored evidence {} requirement {} is not declared by slice {}",
                    claim.id.as_str(),
                    claim.requirement_id.as_str(),
                    slice.id.as_str()
                )));
            }
        }
    }
    Ok(())
}

fn parse_toml<T: for<'de> Deserialize<'de>>(
    path: Option<&Path>,
    input: &str,
    kind: &str,
) -> CargoAllowResult<T> {
    toml::from_str(input).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidPolicy,
            format!("failed to parse authored {kind} TOML: {error}"),
        )
        .with_toml_span(path, input, error.span())
    })
}

fn validate_schema(path: Option<&Path>, version: &str, kind: &str) -> CargoAllowResult<()> {
    if version != AUTHORED_MAPPING_SCHEMA_VERSION {
        return Err(invalid_mapping(
            path,
            format!(
                "authored {kind} schema_version must be {AUTHORED_MAPPING_SCHEMA_VERSION}, found {version}"
            ),
        ));
    }
    Ok(())
}

fn validate_seam(path: Option<&Path>, seam: &mut AuthoredSeam) -> CargoAllowResult<()> {
    ensure_non_empty(path, "authored seam id", seam.id.as_str())?;
    if seam.generation == 0 {
        return Err(invalid_mapping(
            path,
            "authored seam generation must be greater than zero",
        ));
    }
    ensure_non_empty(path, "authored seam owner", &seam.owner)?;
    ensure_non_empty(path, "authored seam operation", &seam.operation)?;
    validate_source_location(path, &mut seam.source, "authored seam source")
}

fn validate_claim(path: Option<&Path>, claim: &mut AuthoredEvidenceClaim) -> CargoAllowResult<()> {
    ensure_non_empty(path, "authored evidence id", claim.id.as_str())?;
    if claim.requirement_generation == 0 || claim.slice_generation == 0 {
        return Err(invalid_mapping(
            path,
            format!(
                "authored evidence {} generations must be greater than zero",
                claim.id.as_str()
            ),
        ));
    }
    for (label, value) in [
        ("precondition", &claim.precondition),
        ("operation", &claim.operation),
        ("expected_observable", &claim.expected_observable),
        ("discriminator", &claim.discriminator),
        ("claim_boundary", &claim.claim_boundary),
    ] {
        ensure_non_empty(
            path,
            &format!("authored evidence {} {label}", claim.id.as_str()),
            value,
        )?;
    }
    validate_source_location(path, &mut claim.source, "authored evidence source")?;
    if claim.subject.is_empty() {
        return Err(invalid_mapping(
            path,
            format!(
                "authored evidence {} must name at least one subject",
                claim.id.as_str()
            ),
        ));
    }
    let mut ids = BTreeSet::new();
    let mut has_exact = false;
    for subject in &claim.subject {
        for (label, value) in [
            ("id", &subject.id),
            ("package", &subject.package),
            ("target", &subject.target),
            ("module_path", &subject.module_path),
            ("test_name", &subject.test_name),
        ] {
            ensure_non_empty(
                path,
                &format!("authored evidence {} subject {label}", claim.id.as_str()),
                value,
            )?;
        }
        validate_target_selector(path, &subject.target, claim.id.as_str())?;
        if !ids.insert(subject.id.as_str()) {
            return Err(invalid_mapping(
                path,
                format!(
                    "duplicate subject {} in authored evidence {}",
                    subject.id,
                    claim.id.as_str()
                ),
            ));
        }
        has_exact |= matches!(subject.role, AuthoredSubjectRole::ExactEvidence);
    }
    if !has_exact {
        return Err(invalid_mapping(
            path,
            format!(
                "authored evidence {} must name an exact subject",
                claim.id.as_str()
            ),
        ));
    }
    Ok(())
}

fn validate_target_selector(
    path: Option<&Path>,
    target: &str,
    evidence_id: &str,
) -> CargoAllowResult<()> {
    let Some((kind, name)) = target.split_once(':') else {
        return Err(invalid_mapping(
            path,
            format!("authored evidence {evidence_id} target must use kind:name syntax"),
        ));
    };
    if !matches!(kind, "lib" | "bin" | "integration_test") || name.trim().is_empty() {
        return Err(invalid_mapping(
            path,
            format!("authored evidence {evidence_id} target is not supported: {target}"),
        ));
    }
    Ok(())
}

fn validate_source_location(
    path: Option<&Path>,
    location: &mut SourceLocation,
    label: &str,
) -> CargoAllowResult<()> {
    ensure_non_empty(path, &format!("{label} path"), &location.path)?;
    let normalized = normalize_path(location.path.clone());
    let absolute_windows = normalized.as_bytes().get(1) == Some(&b':');
    if normalized.starts_with('/')
        || Path::new(&normalized).is_absolute()
        || absolute_windows
        || normalized == ".."
        || normalized.starts_with("../")
    {
        return Err(invalid_mapping(
            path,
            format!("{label} path must be repository-relative: {normalized}"),
        ));
    }
    location.path = normalized;
    if location.line == Some(0) {
        return Err(invalid_mapping(
            path,
            format!("{label} line must be greater than zero"),
        ));
    }
    if let Some(symbol) = &location.symbol {
        ensure_non_empty(path, &format!("{label} symbol"), symbol)?;
    }
    Ok(())
}

fn ensure_non_empty(path: Option<&Path>, label: &str, value: &str) -> CargoAllowResult<()> {
    if value.trim().is_empty() {
        return Err(invalid_mapping(path, format!("{label} must not be empty")));
    }
    if value.trim() != value {
        return Err(invalid_mapping(
            path,
            format!("{label} must not have leading or trailing whitespace"),
        ));
    }
    Ok(())
}

fn invalid_mapping(path: Option<&Path>, message: impl Into<String>) -> CargoAllowError {
    let message = match path {
        Some(path) => format!("{}: {}", path.display(), message.into()),
        None => message.into(),
    };
    CargoAllowError::with_kind(CargoAllowErrorKind::InvalidPolicy, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEAM: &str = r#"
schema_version = "1.0"

[[seam]]
id = "seam:runtime-promotion"
generation = 1
owner = "allow-policy"
operation = "validate runtime promotion"

[seam.source]
path = "crates/allow-policy/src/spec_system/runtime_promotion.rs"
symbol = "validate_runtime_promotion"
"#;

    const EVIDENCE: &str = r#"
schema_version = "1.0"

[[evidence]]
id = "evidence:runtime-promotion"
requirement_id = "spec#runtime-promotion"
requirement_generation = 1
slice_id = "slice:runtime-promotion"
slice_generation = 1
seam_id = "seam:runtime-promotion"
purpose = "forbidden_runtime_promotion"
precondition = "spec-only slice"
operation = "claim implemented"
expected_observable = "typed rejection"
discriminator = "SpecOnlyRuntimeImplementationClaim"
claim_boundary = "Does not prove execution."

[evidence.source]
path = "crates/allow-policy/src/spec_system/runtime_promotion.rs"
symbol = "spec_or_policy_slice_rejects_unproved_runtime_promotion"

[[evidence.subject]]
id = "subject:exact"
role = "exact_evidence"
package = "allow-policy"
target = "lib:allow_policy"
module_path = "spec_system::runtime_promotion::tests"
test_name = "spec_or_policy_slice_rejects_unproved_runtime_promotion"

[[evidence.subject]]
id = "subject:weak"
role = "related_weak"
package = "allow-policy"
target = "lib:allow_policy"
module_path = "spec_system::runtime_promotion::tests"
test_name = "behavior_change_rejects_implemented_claim_without_evidence_closure"
"#;

    #[test]
    fn parses_authored_seam_and_evidence_sources() -> Result<(), String> {
        let seams = parse_authored_seams_at(Some(Path::new("seams.toml")), SEAM)
            .map_err(|error| error.to_string())?;
        let evidence = parse_authored_evidence_at(Some(Path::new("evidence.toml")), EVIDENCE)
            .map_err(|error| error.to_string())?;
        assert_eq!(seams.seam.len(), 1);
        assert_eq!(evidence.evidence.len(), 1);
        assert_eq!(evidence.evidence[0].subject.len(), 2);
        assert_eq!(
            evidence.evidence[0].source.path,
            "crates/allow-policy/src/spec_system/runtime_promotion.rs"
        );
        Ok(())
    }

    #[test]
    fn rejects_mutable_or_root_specific_mapping_fields() {
        let mutable = format!("{SEAM}\nbranch = \"main\"\n");
        assert!(parse_authored_seams(&mutable).is_err());
        let absolute = SEAM.replace(
            "crates/allow-policy/src/spec_system/runtime_promotion.rs",
            "/tmp/runtime_promotion.rs",
        );
        assert!(parse_authored_seams(&absolute).is_err());
    }

    #[test]
    fn rejects_duplicate_subjects_and_weak_only_evidence() {
        let duplicate = EVIDENCE.replace("id = \"subject:weak\"", "id = \"subject:exact\"");
        assert!(parse_authored_evidence(&duplicate).is_err());
        let weak_only = EVIDENCE
            .replace("role = \"exact_evidence\"", "role = \"related_weak\"")
            .replace("[[evidence.subject]]\nid = \"subject:weak\"", "");
        assert!(parse_authored_evidence(&weak_only).is_err());
    }
}
