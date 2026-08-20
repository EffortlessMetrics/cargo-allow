//! Read-only validation and projection of captured proof receipts (#3600).

use proof_engine::{ReceiptStatusReportV1, evaluate_captured_receipt_status_from_json};
use serde_json::Value;
use std::path::Path;

pub fn captured_receipt_status_from_paths(
    plan_path: &Path,
    manifest_path: &Path,
) -> Result<ReceiptStatusReportV1, String> {
    let plan_text = std::fs::read_to_string(plan_path)
        .map_err(|error| format!("read proof plan {}: {error}", plan_path.display()))?;
    let manifest_text = std::fs::read_to_string(manifest_path)
        .map_err(|error| format!("read receipt manifest {}: {error}", manifest_path.display()))?;
    let mut report = evaluate_captured_receipt_status_from_json(&plan_text, &manifest_text)
        .map_err(|error| {
            format!(
                "{} (plan {}; receipts {})",
                error,
                plan_path.display(),
                manifest_path.display()
            )
        })?;
    let registry = StaticProviderRegistryV1::selected()
        .map_err(|error| format!("provider registry: {}", error.as_str()))?;
    for row in &mut report.items {
        let Some(provider_id) = row.provider_id.as_deref() else {
            continue;
        };
        if !registry.provider_available(provider_id) {
            row.status = proof_engine::ProofItemReceiptStatusV1::ProviderUnavailable;
            row.reason = format!("provider {provider_id} is unavailable in this registry build");
            continue;
        }
        if let Some(capability_id) = row.capability_id.as_deref() {
            let capability_selected = registry.projections().into_iter().any(|projection| {
                projection.provider_id == provider_id
                    && projection
                        .capabilities
                        .capabilities
                        .iter()
                        .any(|capability| capability.capability_id == capability_id)
            });
            if !capability_selected {
                row.status = proof_engine::ProofItemReceiptStatusV1::ProviderUnavailable;
                row.reason = format!(
                    "capability {capability_id} for {provider_id} is unavailable in this registry build"
                );
            }
        }
        if row.status != proof_engine::ProofItemReceiptStatusV1::ProviderUnavailable
            && let Err(reason) = validate_native_payload(
                provider_id,
                &manifest_value_for_item(&manifest_text, &row.proof_item_id)?,
            )
        {
            row.status = proof_engine::ProofItemReceiptStatusV1::ReceiptMalformed;
            row.reason = reason;
        }
    }
    Ok(report)
}

fn manifest_value_for_item(manifest_text: &str, proof_item_id: &str) -> Result<Value, String> {
    let manifest: Value = serde_json::from_str(manifest_text)
        .map_err(|error| format!("parse receipt manifest: {error}"))?;
    let row = manifest
        .get("rows")
        .and_then(Value::as_array)
        .and_then(|rows| {
            rows.iter()
                .find(|row| row.get("proof_item_id").and_then(Value::as_str) == Some(proof_item_id))
        })
        .ok_or_else(|| format!("receipt row {proof_item_id} is missing from manifest"))?;
    Ok(row.clone())
}

fn validate_native_payload(provider_id: &str, row: &Value) -> Result<(), String> {
    let receipt = row
        .get("receipt")
        .ok_or_else(|| "receipt row has no envelope".to_string())?;
    let _schema = receipt
        .get("provider_payload_schema")
        .and_then(Value::as_str)
        .ok_or_else(|| "receipt payload schema is missing".to_string())?;
    let _payload = receipt
        .get("provider_payload")
        .cloned()
        .ok_or_else(|| "receipt provider payload is missing".to_string())?;
    match provider_id {
        "proof.hawk.v1" => {
            #[cfg(feature = "provider-hawk")]
            {
                if _schema != crate::providers::hawk::HAWK_ANALYSIS_RECEIPT_SCHEMA_ID {
                    return Err(format!("unsupported Hawk payload schema {_schema}"));
                }
                let parsed: crate::providers::hawk::HawkAnalysisReceiptV1 =
                    serde_json::from_value(_payload)
                        .map_err(|error| format!("malformed Hawk receipt: {error}"))?;
                crate::providers::hawk::validate_hawk_analysis_receipt(&parsed)
                    .map_err(|error| format!("invalid Hawk receipt: {}", error.as_str()))
            }
            #[cfg(not(feature = "provider-hawk"))]
            {
                Err("Hawk provider is unavailable in this registry build".to_string())
            }
        }
        "proof.ripr.v1" => {
            #[cfg(feature = "provider-ripr")]
            {
                if _schema != crate::providers::ripr::RIPR_GRIP_RECEIPT_SCHEMA_ID {
                    return Err(format!("unsupported RIPR payload schema {_schema}"));
                }
                let parsed: crate::providers::ripr::RiprGripReceiptV1 =
                    serde_json::from_value(_payload)
                        .map_err(|error| format!("malformed RIPR receipt: {error}"))?;
                crate::providers::ripr::validate_ripr_grip_receipt(&parsed)
                    .map_err(|error| format!("invalid RIPR receipt: {}", error.as_str()))
            }
            #[cfg(not(feature = "provider-ripr"))]
            {
                Err("RIPR provider is unavailable in this registry build".to_string())
            }
        }
        "proof.cargo-allow.v1" => {
            #[cfg(feature = "provider-cargo-allow")]
            {
                if _schema != crate::providers::cargo_allow::CARGO_ALLOW_PROVIDER_CONTRACT_SCHEMA_ID
                {
                    return Err(format!("unsupported cargo-allow payload schema {_schema}"));
                }
                let parsed: crate::providers::cargo_allow::CargoAllowProviderContractV1 =
                    serde_json::from_value(_payload)
                        .map_err(|error| format!("malformed cargo-allow receipt: {error}"))?;
                crate::providers::cargo_allow::validate_provider_contract(&parsed)
                    .map_err(|error| format!("invalid cargo-allow receipt: {}", error.as_str()))
            }
            #[cfg(not(feature = "provider-cargo-allow"))]
            {
                Err("cargo-allow provider is unavailable in this registry build".to_string())
            }
        }
        _ => Err(format!("unsupported provider {provider_id}")),
    }
}

pub fn render_captured_receipt_status(
    report: &ReceiptStatusReportV1,
    format: crate::OutputFormat,
) -> Result<String, String> {
    match format {
        crate::OutputFormat::Json => serde_json::to_string_pretty(report)
            .map(|json| format!("{json}\n"))
            .map_err(|error| error.to_string()),
        crate::OutputFormat::Human => {
            let mut output = format!(
                "proof status {} ({} items)\n",
                report.plan_id,
                report.items.len()
            );
            for item in &report.items {
                output.push_str(&format!(
                    "{}: {:?} - {}\n",
                    item.proof_item_id, item.status, item.reason
                ));
            }
            output.push_str(&format!("claim boundary: {}\n", report.claim_boundary));
            Ok(output)
        }
    }
}

pub fn render_captured_receipt_validation(
    report: &ReceiptStatusReportV1,
    format: crate::OutputFormat,
) -> Result<String, String> {
    let structurally_valid = report.items.iter().all(|item| {
        !matches!(
            item.status,
            proof_engine::ProofItemReceiptStatusV1::ReceiptMissing
                | proof_engine::ProofItemReceiptStatusV1::ReceiptMalformed
                | proof_engine::ProofItemReceiptStatusV1::ReceiptStale
                | proof_engine::ProofItemReceiptStatusV1::ReceiptForDifferentItem
                | proof_engine::ProofItemReceiptStatusV1::ProviderUnavailable
        )
    });
    let satisfies_plan = structurally_valid
        && report.items.iter().all(|item| {
            item.status == proof_engine::ProofItemReceiptStatusV1::SatisfiedByCurrentReceipt
        });
    match format {
        crate::OutputFormat::Json => serde_json::to_string_pretty(&serde_json::json!({
            "schema_id": "proof.receipt-validation.v1",
            "plan_id": report.plan_id,
            "valid": satisfies_plan,
            "structurally_valid": structurally_valid,
            "satisfies_plan": satisfies_plan,
            "items": report.items,
            "claim_boundary": "Receipt files and provider bindings were validated read-only; no provider executed and no source was mutated."
        }))
        .map(|json| format!("{json}\n"))
        .map_err(|error| error.to_string()),
        crate::OutputFormat::Human => Ok(format!(
            "receipts {} for {} ({} items)\nclaim boundary: receipt files and provider bindings were validated read-only; no provider executed and no source was mutated.\n",
            if satisfies_plan { "satisfy the plan" } else if structurally_valid { "are structurally valid but do not satisfy the plan" } else { "are invalid" },
            report.plan_id,
            report.items.len()
        )),
    }
}

pub use crate::StaticProviderRegistryV1;

#[cfg(test)]
mod tests {
    use super::*;
    use proof_engine::{
        ProofItemReceiptStatusRowV1, ProofItemReceiptStatusV1, RECEIPT_STATUS_REPORT_SCHEMA_ID,
    };

    fn report() -> ReceiptStatusReportV1 {
        ReceiptStatusReportV1 {
            schema_id: RECEIPT_STATUS_REPORT_SCHEMA_ID.to_string(),
            plan_id: "plan-test".to_string(),
            items: vec![ProofItemReceiptStatusRowV1 {
                proof_item_id: "item-test".to_string(),
                status: ProofItemReceiptStatusV1::CurrentFindings,
                provider_id: Some("provider-test".to_string()),
                capability_id: Some("capability-test".to_string()),
                reason: "captured finding".to_string(),
            }],
            claim_boundary: "read-only".to_string(),
        }
    }

    #[test]
    fn human_and_json_projections_share_the_same_report() -> Result<(), String> {
        let report = report();
        let human = render_captured_receipt_status(&report, crate::OutputFormat::Human)?;
        let json = render_captured_receipt_status(&report, crate::OutputFormat::Json)?;
        if !human.contains("CurrentFindings") || !json.contains("current_findings") {
            return Err("status projections lost the typed result".to_string());
        }
        Ok(())
    }

    #[test]
    fn validation_projection_is_read_only_and_explicit() -> Result<(), String> {
        let output = render_captured_receipt_validation(&report(), crate::OutputFormat::Human)?;
        if !output.contains("no provider executed") || !output.contains("no source was mutated") {
            return Err("validation claim boundary was not explicit".to_string());
        }
        Ok(())
    }

    #[test]
    fn unknown_provider_payload_is_rejected_without_execution() -> Result<(), String> {
        let row = serde_json::json!({
            "receipt": {
                "provider_payload_schema": "unknown.provider.v1",
                "provider_payload": {}
            }
        });
        match validate_native_payload("unknown.provider.v1", &row) {
            Err(message) if message.contains("unsupported provider") => Ok(()),
            Err(message) => Err(format!("unexpected native validation error: {message}")),
            Ok(()) => Err("unknown provider payload was accepted".to_string()),
        }
    }
}
