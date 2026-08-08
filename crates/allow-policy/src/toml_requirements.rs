use allow_core::Requirements;
use serde::Deserialize;

use crate::toml_de::option_bool_or_string;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RequirementsToml {
    #[serde(default, deserialize_with = "option_bool_or_string")]
    owner_required: Option<bool>,
    #[serde(default, deserialize_with = "option_bool_or_string")]
    reason_required: Option<bool>,
    #[serde(default, deserialize_with = "option_bool_or_string")]
    classification_required: Option<bool>,
    #[serde(default, deserialize_with = "option_bool_or_string")]
    evidence_required: Option<bool>,
    #[serde(default, deserialize_with = "option_bool_or_string")]
    expires_or_review_after_required: Option<bool>,
    #[serde(default, deserialize_with = "option_bool_or_string")]
    allow_bare_allow_attributes: Option<bool>,
    #[serde(default, deserialize_with = "option_bool_or_string")]
    lint_policy_id_required: Option<bool>,
    #[serde(default, deserialize_with = "option_bool_or_string")]
    stale_entries_fail: Option<bool>,
    #[serde(default, rename = "unsafe")]
    unsafe_requirements: UnsafeRequirementsToml,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct UnsafeRequirementsToml {
    #[serde(default, deserialize_with = "option_bool_or_string")]
    evidence_required: Option<bool>,
    #[serde(default, deserialize_with = "option_bool_or_string")]
    safety_comment_required: Option<bool>,
}

impl RequirementsToml {
    pub(crate) fn into_requirements(self) -> Requirements {
        let default = Requirements::default();
        Requirements {
            owner_required: self.owner_required.unwrap_or(default.owner_required),
            reason_required: self.reason_required.unwrap_or(default.reason_required),
            classification_required: self
                .classification_required
                .unwrap_or(default.classification_required),
            evidence_required: self.evidence_required.unwrap_or(default.evidence_required),
            expires_or_review_after_required: self
                .expires_or_review_after_required
                .unwrap_or(default.expires_or_review_after_required),
            allow_bare_allow_attributes: self
                .allow_bare_allow_attributes
                .unwrap_or(default.allow_bare_allow_attributes),
            lint_policy_id_required: self
                .lint_policy_id_required
                .unwrap_or(default.lint_policy_id_required),
            stale_entries_fail: self
                .stale_entries_fail
                .unwrap_or(default.stale_entries_fail),
            unsafe_evidence_required: self
                .unsafe_requirements
                .evidence_required
                .unwrap_or(default.unsafe_evidence_required),
            unsafe_safety_comment_required: self
                .unsafe_requirements
                .safety_comment_required
                .unwrap_or(default.unsafe_safety_comment_required),
            unsafe_verified_evidence_required: default.unsafe_verified_evidence_required,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn into_requirements_preserves_defaults_for_omitted_fields() {
        let requirements = RequirementsToml::default().into_requirements();

        assert_eq!(requirements, Requirements::default());
    }

    #[test]
    fn into_requirements_maps_explicit_requirement_fields() {
        let requirements = RequirementsToml {
            owner_required: Some(false),
            reason_required: Some(false),
            classification_required: Some(false),
            evidence_required: Some(true),
            expires_or_review_after_required: Some(false),
            allow_bare_allow_attributes: Some(true),
            lint_policy_id_required: Some(true),
            stale_entries_fail: Some(true),
            unsafe_requirements: UnsafeRequirementsToml {
                evidence_required: Some(false),
                safety_comment_required: Some(true),
            },
        }
        .into_requirements();

        assert_eq!(
            requirements,
            Requirements {
                owner_required: false,
                reason_required: false,
                classification_required: false,
                evidence_required: true,
                expires_or_review_after_required: false,
                allow_bare_allow_attributes: true,
                lint_policy_id_required: true,
                stale_entries_fail: true,
                unsafe_evidence_required: false,
                unsafe_safety_comment_required: true,
                unsafe_verified_evidence_required: false,
            }
        );
    }
}
