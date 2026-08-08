//! Hawk finding-to-adapter result mapping (#2555 Stage A).

use serde::{Deserialize, Serialize};

use super::analysis_receipt::HawkFindingV1;

pub const HAWK_FINDING_RESULT_SCHEMA_ID: &str = "proof.hawk-finding-result.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HawkResultClassV1 {
    DeadPublicFinding,
    UnnecessarilyPublic,
    NonProductionOnlyLive,
    ProductionLiveFromConfiguredClosure,
    RestrictedVisibilityReducible,
    CrateVisibilityReducible,
    NoFindingObserved,
    NotProven,
    ExcludedByPolicy,
    UnsupportedTarget,
    CompletenessUnestablished,
    UnknownReachability,
    InstrumentFailure,
    MalformedOrStale,
}

impl HawkResultClassV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DeadPublicFinding => "dead_public_finding",
            Self::UnnecessarilyPublic => "unnecessarily_public",
            Self::NonProductionOnlyLive => "non_production_only_live",
            Self::ProductionLiveFromConfiguredClosure => "production_live_from_configured_closure",
            Self::RestrictedVisibilityReducible => "restricted_visibility_reducible",
            Self::CrateVisibilityReducible => "crate_visibility_reducible",
            Self::NoFindingObserved => "no_finding_observed",
            Self::NotProven => "not_proven",
            Self::ExcludedByPolicy => "excluded_by_policy",
            Self::UnsupportedTarget => "unsupported_target",
            Self::CompletenessUnestablished => "completeness_unestablished",
            Self::UnknownReachability => "unknown_reachability",
            Self::InstrumentFailure => "instrument_failure",
            Self::MalformedOrStale => "malformed_or_stale",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HawkFindingResultV1 {
    pub schema_id: String,
    pub declaration_identity: String,
    pub hawk_code: String,
    pub primary_class: HawkResultClassV1,
    pub secondary_class: Option<HawkResultClassV1>,
}

pub fn map_hawk_finding(finding: &HawkFindingV1) -> HawkFindingResultV1 {
    let (primary, secondary) = match finding.hawk_code.as_str() {
        "hawk::dead_public" => (HawkResultClassV1::DeadPublicFinding, None),
        "hawk::unnecessary_public" => match finding.test_only {
            Some(true) => (
                HawkResultClassV1::NonProductionOnlyLive,
                Some(HawkResultClassV1::UnnecessarilyPublic),
            ),
            Some(false) => (
                HawkResultClassV1::ProductionLiveFromConfiguredClosure,
                Some(HawkResultClassV1::UnnecessarilyPublic),
            ),
            None => (HawkResultClassV1::NotProven, None),
        },
        "hawk::unnecessary_restricted_visibility" => {
            (HawkResultClassV1::RestrictedVisibilityReducible, None)
        }
        "hawk::unnecessary_crate_visibility" => (HawkResultClassV1::CrateVisibilityReducible, None),
        _ => (HawkResultClassV1::NotProven, None),
    };
    HawkFindingResultV1 {
        schema_id: HAWK_FINDING_RESULT_SCHEMA_ID.to_string(),
        declaration_identity: finding.declaration_identity.clone(),
        hawk_code: finding.hawk_code.clone(),
        primary_class: primary,
        secondary_class: secondary,
    }
}

pub fn map_missing_finding(declaration_identity: &str) -> HawkFindingResultV1 {
    HawkFindingResultV1 {
        schema_id: HAWK_FINDING_RESULT_SCHEMA_ID.to_string(),
        declaration_identity: declaration_identity.to_string(),
        hawk_code: String::new(),
        primary_class: HawkResultClassV1::NoFindingObserved,
        secondary_class: Some(HawkResultClassV1::NotProven),
    }
}
