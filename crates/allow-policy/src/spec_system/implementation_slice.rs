use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;

use super::{RequirementId, RequirementLifecycle};

pub const IMPLEMENTATION_SLICE_SCHEMA_VERSION: &str = "1.0";

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ImplementationSliceId(pub String);

impl ImplementationSliceId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImplementationSliceClass {
    SpecOrPolicyChange,
    BehaviorChange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImplementationDispositionState {
    Outstanding,
    Implemented,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImplementationDisposition {
    pub state: ImplementationDispositionState,
    #[serde(default)]
    pub seams: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceDispositionState {
    Outstanding,
    Current,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceDisposition {
    pub state: EvidenceDispositionState,
    #[serde(default)]
    pub receipt: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportClaimDispositionState {
    Unchanged,
    Promoted,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SupportClaimDisposition {
    pub state: SupportClaimDispositionState,
    #[serde(default)]
    pub claim: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequirementDelta {
    pub requirement_id: RequirementId,
    pub requirement_generation: u32,
    pub runtime: bool,
    #[serde(default)]
    pub from: Option<RequirementLifecycle>,
    pub to: RequirementLifecycle,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImplementationSliceV1 {
    pub schema_version: String,
    pub id: ImplementationSliceId,
    pub generation: u32,
    pub source_issue: String,
    pub design_reference: String,
    pub change_class: ImplementationSliceClass,
    pub basis: String,
    pub requirement_delta: Vec<RequirementDelta>,
    pub implementation: ImplementationDisposition,
    pub evidence: EvidenceDisposition,
    pub support_claim: SupportClaimDisposition,
    #[serde(default)]
    pub owned_seams: Vec<String>,
    #[serde(default)]
    pub shared_seams: Vec<String>,
    #[serde(default)]
    pub forbidden_seams: Vec<String>,
    #[serde(default)]
    pub non_goals: Vec<String>,
    #[serde(default)]
    pub return_conditions: Vec<String>,
    pub claim_boundary: String,
}

pub fn parse_implementation_slice(input: &str) -> CargoAllowResult<ImplementationSliceV1> {
    parse_implementation_slice_at(None, input)
}

pub fn parse_implementation_slice_at(
    path: Option<&Path>,
    input: &str,
) -> CargoAllowResult<ImplementationSliceV1> {
    let slice = toml::from_str::<ImplementationSliceV1>(input).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidPolicy,
            format!("failed to parse implementation slice TOML: {error}"),
        )
        .with_toml_span(path, input, error.span())
    })?;
    validate_slice_structure(path, &slice)?;
    Ok(slice)
}

fn validate_slice_structure(
    path: Option<&Path>,
    slice: &ImplementationSliceV1,
) -> CargoAllowResult<()> {
    if slice.schema_version != IMPLEMENTATION_SLICE_SCHEMA_VERSION {
        return Err(invalid_slice(
            path,
            format!(
                "implementation slice schema_version must be {IMPLEMENTATION_SLICE_SCHEMA_VERSION}, found {}",
                slice.schema_version
            ),
        ));
    }
    if slice.id.as_str().trim().is_empty() {
        return Err(invalid_slice(
            path,
            "implementation slice id must not be empty",
        ));
    }
    if slice.generation == 0 {
        return Err(invalid_slice(
            path,
            "implementation slice generation must be greater than zero",
        ));
    }
    for (field, value) in [
        ("source_issue", slice.source_issue.as_str()),
        ("design_reference", slice.design_reference.as_str()),
        ("basis", slice.basis.as_str()),
        ("claim_boundary", slice.claim_boundary.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(invalid_slice(
                path,
                format!("implementation slice {field} must not be empty"),
            ));
        }
    }
    if slice.requirement_delta.is_empty() {
        return Err(invalid_slice(
            path,
            "implementation slice must name at least one [[requirement_delta]]",
        ));
    }

    let mut seen = BTreeSet::new();
    for delta in &slice.requirement_delta {
        if delta.requirement_id.as_str().trim().is_empty() {
            return Err(invalid_slice(
                path,
                "requirement delta id must not be empty",
            ));
        }
        if delta.requirement_generation == 0 {
            return Err(invalid_slice(
                path,
                format!(
                    "requirement {} generation must be greater than zero",
                    delta.requirement_id.as_str()
                ),
            ));
        }
        if !seen.insert(delta.requirement_id.clone()) {
            return Err(invalid_slice(
                path,
                format!(
                    "duplicate requirement delta {}",
                    delta.requirement_id.as_str()
                ),
            ));
        }
    }

    Ok(())
}

fn invalid_slice(path: Option<&Path>, message: impl Into<String>) -> CargoAllowError {
    let message = match path {
        Some(path) => format!("{}: {}", path.display(), message.into()),
        None => message.into(),
    };
    CargoAllowError::with_kind(CargoAllowErrorKind::InvalidPolicy, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SLICE: &str = r#"
schema_version = "1.0"
id = "cargo-allow.slice.self-hosted-runtime-promotion.v1"
generation = 1
source_issue = "issue:2206"
design_reference = "design:self-hosted-runtime-promotion"
change_class = "spec_or_policy_change"
basis = "git:example-head"
claim_boundary = "Defines the requirement without claiming runtime completion."
non_goals = ["runtime implementation"]
return_conditions = ["implementation and evidence are available"]
owned_seams = ["seam:spec-system:runtime-promotion-validator"]
forbidden_seams = ["support:runtime-stable"]

[[requirement_delta]]
requirement_id = "CARGO-ALLOW-SPEC-0009#spec-only-runtime-promotion"
requirement_generation = 1
runtime = true
to = "accepted"

[implementation]
state = "outstanding"

[evidence]
state = "outstanding"

[support_claim]
state = "unchanged"
"#;

    #[test]
    fn implementation_slice_roundtrip() -> Result<(), String> {
        let slice = parse_implementation_slice_at(Some(Path::new("slice.toml")), SLICE)
            .map_err(|error| error.to_string())?;
        let encoded = toml::to_string(&slice).map_err(|error| error.to_string())?;
        let decoded = parse_implementation_slice(&encoded).map_err(|error| error.to_string())?;

        assert_eq!(slice, decoded);
        assert_eq!(
            slice.id.as_str(),
            "cargo-allow.slice.self-hosted-runtime-promotion.v1"
        );
        assert_eq!(slice.requirement_delta.len(), 1);
        Ok(())
    }

    #[test]
    fn implementation_slice_rejects_unknown_generation() {
        let result = parse_implementation_slice(
            &SLICE.replace("schema_version = \"1.0\"", "schema_version = \"2.0\""),
        );
        assert!(result.is_err());
    }

    #[test]
    fn implementation_slice_rejects_mutable_execution_state() {
        let result = parse_implementation_slice(&format!("{SLICE}\nbranch = \"main\"\n"));
        assert!(result.is_err());
    }
}
