//! Provider-neutral analysis receipt wrapping for process delegation (#2601-B).

use crate::{CHANGE_STATUS_SCHEMA_ID, ChangeStatusReportV1};
use effortless_repo_protocol::{
    ANALYSIS_RECEIPT_SCHEMA_ID, AnalysisReceiptEnvelopeV1, ClaimBoundaryV1, CompletenessV1,
    CurrentnessV1, REPOSITORY_SNAPSHOT_SCHEMA_ID, RepositorySnapshotV1, ResultClassV1,
};
use intent_protocol::RepositorySnapshotV1 as IntentRepositorySnapshotV1;

pub const PROVIDER_ID: &str = "cargo-intent";

pub fn wrap_change_status_report(
    report: &ChangeStatusReportV1,
    snapshot: &IntentRepositorySnapshotV1,
) -> Result<AnalysisReceiptEnvelopeV1, String> {
    let repo_snapshot = snapshot_to_repo_protocol(snapshot)?;
    let result_class = parse_result_class(&report.result_class)?;
    let completeness = match report.inventory_completeness.as_str() {
        "complete" => CompletenessV1::Complete,
        "partial" => CompletenessV1::Partial,
        _ => CompletenessV1::Unknown,
    };
    let payload =
        serde_json::to_value(report).map_err(|err| format!("serialize provider payload: {err}"))?;
    Ok(AnalysisReceiptEnvelopeV1 {
        schema_id: ANALYSIS_RECEIPT_SCHEMA_ID.to_string(),
        provider: PROVIDER_ID.to_string(),
        snapshot: repo_snapshot,
        result_class,
        completeness,
        currentness: CurrentnessV1::Current,
        provider_payload_schema: CHANGE_STATUS_SCHEMA_ID.to_string(),
        provider_payload: payload,
        claim_boundary: ClaimBoundaryV1::new(report.claim_boundary.clone()),
    })
}

fn snapshot_to_repo_protocol(
    snapshot: &IntentRepositorySnapshotV1,
) -> Result<RepositorySnapshotV1, String> {
    let value = serde_json::to_value(snapshot).map_err(|err| format!("encode snapshot: {err}"))?;
    let decoded: RepositorySnapshotV1 =
        serde_json::from_value(value).map_err(|err| format!("decode repo snapshot: {err}"))?;
    if decoded.schema_id != REPOSITORY_SNAPSHOT_SCHEMA_ID {
        return Err(format!(
            "unexpected snapshot schema_id {} (expected {REPOSITORY_SNAPSHOT_SCHEMA_ID})",
            decoded.schema_id
        ));
    }
    Ok(decoded)
}

fn parse_result_class(raw: &str) -> Result<ResultClassV1, String> {
    match raw {
        "completed" => Ok(ResultClassV1::Completed),
        "findings" => Ok(ResultClassV1::Findings),
        "not_proven" => Ok(ResultClassV1::NotProven),
        "partial_data" => Ok(ResultClassV1::PartialData),
        "stale_input" => Ok(ResultClassV1::StaleInput),
        "unsupported" => Ok(ResultClassV1::Unsupported),
        "malformed_input" => Ok(ResultClassV1::MalformedInput),
        "instrument_failure" => Ok(ResultClassV1::InstrumentFailure),
        "cancelled" => Ok(ResultClassV1::Cancelled),
        "conflict" => Ok(ResultClassV1::Conflict),
        other => Err(format!("unsupported result_class {other}")),
    }
}
