//! Source-anchor resolution against Hawk declaration identities (#2555).

use serde::{Deserialize, Serialize};

use super::analysis_receipt::{HawkAnalysisReceiptV1, validate_hawk_analysis_receipt};

pub const HAWK_SOURCE_ANCHOR_RESOLUTION_SCHEMA_ID: &str = "proof.hawk-source-anchor-resolution.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceAnchorResolutionClassV1 {
    Exact,
    Ambiguous,
    Missing,
    UnsupportedGeneratedDeclaration,
    OutsideAnalyzedProduct,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HawkSourceAnchorResolutionV1 {
    pub schema_id: String,
    pub requested_anchor: String,
    pub resolved_declaration_identity: Option<String>,
    pub resolution: SourceAnchorResolutionClassV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceAnchorResolutionError {
    Receipt(super::analysis_receipt::HawkAnalysisReceiptError),
}

impl SourceAnchorResolutionError {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Receipt(_) => "receipt_invalid",
        }
    }
}

pub struct SourceAnchorRequest<'a> {
    pub receipt: &'a HawkAnalysisReceiptV1,
    pub requested_anchor: &'a str,
    pub expected_product_name: &'a str,
}

pub fn resolve_source_anchor(
    request: &SourceAnchorRequest<'_>,
) -> Result<HawkSourceAnchorResolutionV1, SourceAnchorResolutionError> {
    validate_hawk_analysis_receipt(request.receipt)
        .map_err(SourceAnchorResolutionError::Receipt)?;
    if request.receipt.product_name != request.expected_product_name {
        return Ok(HawkSourceAnchorResolutionV1 {
            schema_id: HAWK_SOURCE_ANCHOR_RESOLUTION_SCHEMA_ID.to_string(),
            requested_anchor: request.requested_anchor.to_string(),
            resolved_declaration_identity: None,
            resolution: SourceAnchorResolutionClassV1::OutsideAnalyzedProduct,
        });
    }
    if request.requested_anchor.contains("generated::") {
        return Ok(HawkSourceAnchorResolutionV1 {
            schema_id: HAWK_SOURCE_ANCHOR_RESOLUTION_SCHEMA_ID.to_string(),
            requested_anchor: request.requested_anchor.to_string(),
            resolved_declaration_identity: None,
            resolution: SourceAnchorResolutionClassV1::UnsupportedGeneratedDeclaration,
        });
    }
    let matches: Vec<&str> = request
        .receipt
        .findings
        .iter()
        .filter(|finding| finding.declaration_identity == request.requested_anchor)
        .map(|finding| finding.declaration_identity.as_str())
        .collect();
    let resolution = match matches.len() {
        0 => SourceAnchorResolutionClassV1::Missing,
        1 => SourceAnchorResolutionClassV1::Exact,
        _ => SourceAnchorResolutionClassV1::Ambiguous,
    };
    Ok(HawkSourceAnchorResolutionV1 {
        schema_id: HAWK_SOURCE_ANCHOR_RESOLUTION_SCHEMA_ID.to_string(),
        requested_anchor: request.requested_anchor.to_string(),
        resolved_declaration_identity: matches.first().map(|value| (*value).to_string()),
        resolution,
    })
}
