//! Read-only validation and projection of captured proof receipts (#3600).

use proof_engine::{ReceiptStatusReportV1, evaluate_captured_receipt_status_from_json};
use serde_json::Value;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiptCommandError {
    ReadPlan(String),
    ReadManifest(String),
    MalformedPlan(String),
    MalformedManifest(String),
    InvalidReceipt(String),
    ProviderRegistry(String),
}

impl ReceiptCommandError {
    pub fn message(&self) -> &str {
        match self {
            Self::ReadPlan(message)
            | Self::ReadManifest(message)
            | Self::MalformedPlan(message)
            | Self::MalformedManifest(message)
            | Self::InvalidReceipt(message)
            | Self::ProviderRegistry(message) => message,
        }
    }

    pub const fn family(&self) -> crate::ProcessExitFamilyV1 {
        match self {
            Self::ReadPlan(_) | Self::ReadManifest(_) => crate::ProcessExitFamilyV1::Usage,
            Self::MalformedPlan(_)
            | Self::MalformedManifest(_)
            | Self::InvalidReceipt(_)
            | Self::ProviderRegistry(_) => crate::ProcessExitFamilyV1::InstrumentFailure,
        }
    }
}

pub fn captured_receipt_status_from_paths(
    plan_path: &Path,
    manifest_path: &Path,
) -> Result<ReceiptStatusReportV1, ReceiptCommandError> {
    let plan_text = std::fs::read_to_string(plan_path).map_err(|error| {
        ReceiptCommandError::ReadPlan(format!("read proof plan {}: {error}", plan_path.display()))
    })?;
    let manifest_text = std::fs::read_to_string(manifest_path).map_err(|error| {
        ReceiptCommandError::ReadManifest(format!(
            "read receipt manifest {}: {error}",
            manifest_path.display()
        ))
    })?;
    let mut report = evaluate_captured_receipt_status_from_json(&plan_text, &manifest_text)
        .map_err(|error| {
            let message = format!(
                "{} (plan {}; receipts {})",
                error,
                plan_path.display(),
                manifest_path.display()
            );
            if error.starts_with("parse proof plan") {
                ReceiptCommandError::MalformedPlan(message)
            } else if error.starts_with("parse receipt manifest") {
                ReceiptCommandError::MalformedManifest(message)
            } else {
                ReceiptCommandError::InvalidReceipt(message)
            }
        })?;
    let plan: Value = serde_json::from_str(&plan_text)
        .map_err(|error| ReceiptCommandError::MalformedPlan(error.to_string()))?;
    let manifest: Value = serde_json::from_str(&manifest_text)
        .map_err(|error| ReceiptCommandError::MalformedManifest(error.to_string()))?;
    let registry = StaticProviderRegistryV1::selected().map_err(|error| {
        ReceiptCommandError::ProviderRegistry(format!("provider registry: {}", error.as_str()))
    })?;
    for row in &mut report.items {
        let Some(provider_id) = row.provider_id.as_deref() else {
            continue;
        };
        if !registry.provider_available(provider_id) {
            let context = format!("provider {provider_id} is unavailable in this registry build");
            apply_provider_context(&mut row.status, &mut row.reason, context);
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
                let context = format!(
                    "capability {capability_id} for {provider_id} is unavailable in this registry build"
                );
                apply_provider_context(&mut row.status, &mut row.reason, context);
            }
        }
        let Some(manifest_row) = manifest_value_for_item_value(&manifest, &row.proof_item_id)
        else {
            continue;
        };
        if row.status != proof_engine::ProofItemReceiptStatusV1::ProviderUnavailable
            && let Err(reason) = validate_native_payload(provider_id, manifest_row)
        {
            apply_native_context(&mut row.status, &mut row.reason, reason);
        } else if row.status != proof_engine::ProofItemReceiptStatusV1::ProviderUnavailable {
            let plan_item = plan
                .get("items")
                .and_then(Value::as_array)
                .and_then(|items| {
                    items.iter().find(|item| {
                        item.get("proof_item_id").and_then(Value::as_str)
                            == Some(row.proof_item_id.as_str())
                    })
                })
                .ok_or_else(|| {
                    ReceiptCommandError::InvalidReceipt(format!(
                        "receipt row {} has no proof-plan item",
                        row.proof_item_id
                    ))
                })?;
            if let Err(reason) = validate_native_currentness(provider_id, plan_item, manifest_row) {
                apply_native_currentness_context(&mut row.status, &mut row.reason, reason);
            }
        }
    }
    Ok(report)
}

fn apply_provider_context(
    status: &mut proof_engine::ProofItemReceiptStatusV1,
    reason: &mut String,
    context: String,
) {
    if *status == proof_engine::ProofItemReceiptStatusV1::SatisfiedByCurrentReceipt {
        *status = proof_engine::ProofItemReceiptStatusV1::ProviderUnavailable;
        *reason = context;
    } else {
        *reason = format!("{reason}; {context}");
    }
}

fn apply_native_context(
    status: &mut proof_engine::ProofItemReceiptStatusV1,
    reason: &mut String,
    context: String,
) {
    match status {
        proof_engine::ProofItemReceiptStatusV1::ReceiptStale
        | proof_engine::ProofItemReceiptStatusV1::ReceiptForDifferentItem
        | proof_engine::ProofItemReceiptStatusV1::ReceiptMalformed => {
            *reason = format!("{reason}; {context}");
        }
        _ => {
            *status = proof_engine::ProofItemReceiptStatusV1::ReceiptMalformed;
            *reason = context;
        }
    }
}

fn apply_native_currentness_context(
    status: &mut proof_engine::ProofItemReceiptStatusV1,
    reason: &mut String,
    context: String,
) {
    match status {
        proof_engine::ProofItemReceiptStatusV1::SatisfiedByCurrentReceipt
        | proof_engine::ProofItemReceiptStatusV1::CurrentFindings
        | proof_engine::ProofItemReceiptStatusV1::CurrentFailed
        | proof_engine::ProofItemReceiptStatusV1::CurrentPartial
        | proof_engine::ProofItemReceiptStatusV1::CurrentUnsupported
        | proof_engine::ProofItemReceiptStatusV1::CurrentNotProven
        | proof_engine::ProofItemReceiptStatusV1::CurrentInstrumentFailure => {
            *status = proof_engine::ProofItemReceiptStatusV1::CurrentNotProven;
            *reason = context;
        }
        _ => *reason = format!("{reason}; {context}"),
    }
}

fn validate_native_currentness(
    provider_id: &str,
    _plan_item: &Value,
    _manifest_row: &Value,
) -> Result<(), String> {
    match provider_id {
        "proof.cargo-allow.v1" => Err(
            "cargo-allow native currentness is not proven: static provider contract has no captured identity binding"
                .to_string(),
        ),
        "proof.ripr.v1" => Err(
            "RIPR native currentness is not proven: authoritative requirement evidence purpose/seam is not present in the receipt manifest"
                .to_string(),
        ),
        "proof.hawk.v1" => Err(
            "Hawk native currentness is not proven: expected frontend and driver identities are not declared by the receipt manifest"
                .to_string(),
        ),
        _ => Err(format!("native currentness is unsupported for provider {provider_id}")),
    }
}

pub fn receipt_validation_satisfies_plan(report: &ReceiptStatusReportV1) -> bool {
    report.items.iter().all(|item| {
        item.status == proof_engine::ProofItemReceiptStatusV1::SatisfiedByCurrentReceipt
    })
}

fn manifest_value_for_item_value<'a>(
    manifest: &'a Value,
    proof_item_id: &str,
) -> Option<&'a Value> {
    manifest
        .get("rows")
        .and_then(Value::as_array)
        .and_then(|rows| {
            rows.iter()
                .find(|row| row.get("proof_item_id").and_then(Value::as_str) == Some(proof_item_id))
        })
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
                    "{}: {} - {}\n",
                    item.proof_item_id,
                    item.status.as_str(),
                    item.reason
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
    let satisfies_plan = structurally_valid && receipt_validation_satisfies_plan(report);
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
    use effortless_repo_protocol::{
        AnalysisReceiptEnvelopeV1, ClaimBoundaryV1, RepositorySnapshotV1, ResolvedRevisionV1,
        ResultClassV1,
    };
    use proof_engine::{
        ProofItemReceiptStatusRowV1, ProofItemReceiptStatusV1, RECEIPT_STATUS_REPORT_SCHEMA_ID,
    };
    use proof_protocol::{
        CapturedReceiptManifestRowV1, CapturedReceiptManifestV1, ExpectedReceiptContractV1,
        ProofItemDispositionV1, ProofItemExecutionPostureV1, ProofItemV1, ProofPlanV2,
        ProofSubjectClassV1, ProofSubjectV1, ProviderSelectionV1,
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

    fn snapshot() -> RepositorySnapshotV1 {
        RepositorySnapshotV1::new_committed_head(
            "snapshot-1",
            "sha",
            ResolvedRevisionV1 {
                requested: "HEAD".to_string(),
                commit: "commit".to_string(),
                tree: "tree".to_string(),
            },
        )
    }

    fn snapshot_identity() -> String {
        effortless_repo_protocol::stable_digest_json(&snapshot())
            .map_err(|error| error.message().to_string())
            .unwrap_or_else(|_| "invalid-snapshot".to_string())
    }

    fn path_inputs() -> (ProofPlanV2, CapturedReceiptManifestV1) {
        path_inputs_for_result(ResultClassV1::Findings)
    }

    fn path_inputs_for_result(
        result_class: ResultClassV1,
    ) -> (ProofPlanV2, CapturedReceiptManifestV1) {
        let snapshot_identity = snapshot_identity();
        let plan = ProofPlanV2::new(
            "plan-1",
            "intent-1",
            snapshot_identity.clone(),
            vec![ProofItemV1 {
                proof_item_id: "item-1".to_string(),
                intent_obligation_id: "obligation-1".to_string(),
                phase: "precommit".to_string(),
                blocking: true,
                evidence_purpose_ref: "purpose".to_string(),
                required_capability_class: "capability-1".to_string(),
                snapshot_identity: snapshot_identity.clone(),
                subject: ProofSubjectV1 {
                    subject_class: ProofSubjectClassV1::Commit,
                    revision: Some(snapshot_identity.clone()),
                    selector: None,
                    body_identity: None,
                    limitations: Vec::new(),
                },
                disposition: ProofItemDispositionV1::SelectedForExecution,
                selection: Some(ProviderSelectionV1 {
                    provider_id: "provider-1".to_string(),
                    capability_id: "capability-1".to_string(),
                    request_digest: "request-1".to_string(),
                }),
                current_receipt: None,
                expected_receipt: Some(ExpectedReceiptContractV1 {
                    receipt_schema: effortless_repo_protocol::ANALYSIS_RECEIPT_SCHEMA_ID
                        .to_string(),
                    receipt_generation: 1,
                    config_identity: "config:test".to_string(),
                    currentness_dimensions: vec![
                        "snapshot_identity".to_string(),
                        "subject".to_string(),
                        "provider_request".to_string(),
                        "config".to_string(),
                    ],
                }),
                execution_posture: ProofItemExecutionPostureV1::Execute,
                dependency_group: None,
                limitations: Vec::new(),
                claim_boundary: "test".to_string(),
            }],
        );
        let receipt = AnalysisReceiptEnvelopeV1::new(
            "provider-1",
            snapshot(),
            result_class,
            "provider.payload.v1",
            serde_json::json!({"payload": true}),
            ClaimBoundaryV1::new("captured test evidence"),
        );
        let manifest = CapturedReceiptManifestV1::new(
            "plan-1",
            vec![CapturedReceiptManifestRowV1 {
                proof_item_id: "item-1".to_string(),
                plan_id: "plan-1".to_string(),
                provider_id: "provider-1".to_string(),
                capability_id: "capability-1".to_string(),
                snapshot_identity: snapshot_identity.clone(),
                subject_identity: snapshot_identity,
                provider_request_identity: "request-1".to_string(),
                config_identity: "config:test".to_string(),
                receipt_generation: 1,
                receipt,
            }],
        );
        (plan, manifest)
    }

    #[test]
    fn human_and_json_projections_share_the_same_report() -> Result<(), String> {
        let report = report();
        let human = render_captured_receipt_status(&report, crate::OutputFormat::Human)?;
        let json = render_captured_receipt_status(&report, crate::OutputFormat::Json)?;
        if !human.contains("current_findings") || !json.contains("current_findings") {
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
    fn findings_do_not_satisfy_validation() -> Result<(), String> {
        if receipt_validation_satisfies_plan(&report()) {
            return Err("findings must not produce a successful validation exit".to_string());
        }
        Ok(())
    }

    #[test]
    fn non_satisfying_receipt_states_fail_validation() -> Result<(), String> {
        for status in [
            ProofItemReceiptStatusV1::ReceiptMissing,
            ProofItemReceiptStatusV1::ReceiptMalformed,
            ProofItemReceiptStatusV1::ReceiptStale,
            ProofItemReceiptStatusV1::ReceiptForDifferentItem,
            ProofItemReceiptStatusV1::CurrentPartial,
            ProofItemReceiptStatusV1::CurrentUnsupported,
            ProofItemReceiptStatusV1::CurrentNotProven,
            ProofItemReceiptStatusV1::CurrentInstrumentFailure,
            ProofItemReceiptStatusV1::Conflict,
        ] {
            let mut candidate = report();
            if let Some(item) = candidate.items.first_mut() {
                item.status = status;
            }
            if receipt_validation_satisfies_plan(&candidate) {
                return Err(format!("{status:?} incorrectly satisfied validation"));
            }
        }
        Ok(())
    }

    #[test]
    fn unavailable_context_does_not_hide_malformed_or_stale_status() -> Result<(), String> {
        for status in [
            ProofItemReceiptStatusV1::ReceiptMalformed,
            ProofItemReceiptStatusV1::ReceiptStale,
        ] {
            let mut reason = "specific receipt state".to_string();
            let mut current = status;
            apply_provider_context(
                &mut current,
                &mut reason,
                "provider unavailable".to_string(),
            );
            if current != status || !reason.contains("provider unavailable") {
                return Err("availability context overwrote specific receipt status".to_string());
            }
        }
        Ok(())
    }

    #[test]
    fn provider_and_native_context_transition_only_verified_success() -> Result<(), String> {
        let mut provider_status = ProofItemReceiptStatusV1::SatisfiedByCurrentReceipt;
        let mut provider_reason = "verified receipt".to_string();
        apply_provider_context(
            &mut provider_status,
            &mut provider_reason,
            "provider unavailable".to_string(),
        );
        if provider_status != ProofItemReceiptStatusV1::ProviderUnavailable
            || provider_reason != "provider unavailable"
        {
            return Err("provider context did not transition verified success".to_string());
        }

        let mut native_status = ProofItemReceiptStatusV1::SatisfiedByCurrentReceipt;
        let mut native_reason = "verified receipt".to_string();
        apply_native_context(
            &mut native_status,
            &mut native_reason,
            "malformed native payload".to_string(),
        );
        if native_status != ProofItemReceiptStatusV1::ReceiptMalformed
            || native_reason != "malformed native payload"
        {
            return Err("native context did not transition verified success".to_string());
        }
        Ok(())
    }

    #[test]
    fn native_payload_shape_errors_fail_closed() -> Result<(), String> {
        for row in [
            serde_json::json!({}),
            serde_json::json!({"receipt": {}}),
            serde_json::json!({"receipt": {"provider_payload_schema": "schema"}}),
        ] {
            if validate_native_payload("proof.cargo-allow.v1", &row).is_ok() {
                return Err("incomplete native payload was accepted".to_string());
            }
        }
        Ok(())
    }

    #[test]
    fn human_status_uses_stable_snake_case_token() -> Result<(), String> {
        let output = render_captured_receipt_status(&report(), crate::OutputFormat::Human)?;
        if !output.contains("current_findings") || output.contains("CurrentFindings") {
            return Err("human status token drifted from JSON token".to_string());
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

    #[test]
    fn known_provider_payloads_fail_closed_when_unavailable_or_schema_mismatched()
    -> Result<(), String> {
        for provider_id in ["proof.hawk.v1", "proof.ripr.v1", "proof.cargo-allow.v1"] {
            let row = serde_json::json!({
                "receipt": {
                    "provider_payload_schema": "unsupported.provider.schema",
                    "provider_payload": {}
                }
            });
            let error = validate_native_payload(provider_id, &row)
                .err()
                .ok_or_else(|| format!("{provider_id} payload was accepted"))?;
            if !error.contains("unavailable") && !error.contains("unsupported") {
                return Err(format!(
                    "unexpected {provider_id} validation error: {error}"
                ));
            }
        }
        for row in [serde_json::json!({}), serde_json::json!({"receipt": {}})] {
            if validate_native_payload("proof.hawk.v1", &row).is_ok() {
                return Err("incomplete native payload envelope was accepted".to_string());
            }
        }
        Ok(())
    }

    #[test]
    fn path_loader_covers_typed_io_and_unavailable_provider_projection() -> Result<(), String> {
        let root =
            std::env::temp_dir().join(format!("cargo-proof-receipt-status-{}", std::process::id()));
        std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        let plan_path = root.join("plan.json");
        let manifest_path = root.join("manifest.json");
        let (plan, manifest) = path_inputs();
        std::fs::write(
            &plan_path,
            serde_json::to_vec(&plan).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        std::fs::write(
            &manifest_path,
            serde_json::to_vec(&manifest).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;

        let report = captured_receipt_status_from_paths(&plan_path, &manifest_path)
            .map_err(|error| error.message().to_string())?;
        if !report
            .items
            .first()
            .map(|item| item.reason.contains("unavailable"))
            .unwrap_or(false)
        {
            return Err("unavailable provider context was not projected".to_string());
        }
        let human = render_captured_receipt_status(&report, crate::OutputFormat::Human)?;
        let json = render_captured_receipt_validation(&report, crate::OutputFormat::Json)?;
        if !human.contains("current_findings") || !json.contains("satisfies_plan") {
            return Err("path-loaded report projections were incomplete".to_string());
        }
        if receipt_validation_satisfies_plan(&report) {
            return Err("findings must remain non-satisfying after path loading".to_string());
        }

        for result_class in [
            ResultClassV1::Completed,
            ResultClassV1::NotProven,
            ResultClassV1::PartialData,
            ResultClassV1::StaleInput,
            ResultClassV1::Unsupported,
            ResultClassV1::MalformedInput,
            ResultClassV1::InstrumentFailure,
        ] {
            let (_, manifest) = path_inputs_for_result(result_class);
            std::fs::write(
                &manifest_path,
                serde_json::to_vec(&manifest).map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;
            let projected = captured_receipt_status_from_paths(&plan_path, &manifest_path)
                .map_err(|error| error.message().to_string())?;
            if projected.items.is_empty() {
                return Err("path-loaded status report lost its item".to_string());
            }
        }

        std::fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
        Ok(())
    }

    #[test]
    fn path_loader_classifies_missing_and_malformed_inputs() -> Result<(), String> {
        let root =
            std::env::temp_dir().join(format!("cargo-proof-receipt-errors-{}", std::process::id()));
        std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        let missing =
            captured_receipt_status_from_paths(&root.join("missing"), &root.join("also-missing"));
        if !matches!(missing, Err(ReceiptCommandError::ReadPlan(_))) {
            return Err("missing plan was not classified as usage input".to_string());
        }
        let plan_path = root.join("plan.json");
        let manifest_path = root.join("manifest.json");
        std::fs::write(&plan_path, b"{}").map_err(|error| error.to_string())?;
        std::fs::write(&manifest_path, b"{}").map_err(|error| error.to_string())?;
        let malformed = captured_receipt_status_from_paths(&plan_path, &manifest_path);
        if !matches!(malformed, Err(ReceiptCommandError::MalformedPlan(_))) {
            return Err("malformed plan was not typed as malformed".to_string());
        }
        std::fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
        Ok(())
    }

    #[test]
    fn serialized_missing_row_is_reported_and_blocks_validation() -> Result<(), String> {
        let root = std::env::temp_dir().join(format!(
            "cargo-proof-receipt-missing-row-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        let plan_path = root.join("plan.json");
        let manifest_path = root.join("manifest.json");
        let (plan, _) = path_inputs_for_result(ResultClassV1::Completed);
        let manifest = CapturedReceiptManifestV1::new("plan-1", Vec::new());
        std::fs::write(
            &plan_path,
            serde_json::to_vec(&plan).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        std::fs::write(
            &manifest_path,
            serde_json::to_vec(&manifest).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        let report = captured_receipt_status_from_paths(&plan_path, &manifest_path)
            .map_err(|error| error.message().to_string())?;
        if report.items.first().map(|item| item.status)
            != Some(ProofItemReceiptStatusV1::ReceiptMissing)
        {
            return Err("missing serialized row was not reported".to_string());
        }
        let validation = render_captured_receipt_validation(&report, crate::OutputFormat::Json)?;
        if !validation.contains("\"valid\": false") || !validation.contains("receipt_missing") {
            return Err("missing serialized row did not block validation".to_string());
        }
        std::fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
        Ok(())
    }

    #[cfg(feature = "provider-cargo-allow")]
    #[test]
    fn malformed_native_payload_from_paths_is_structurally_invalid() -> Result<(), String> {
        let root = std::env::temp_dir().join(format!(
            "cargo-proof-receipt-native-malformed-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        let plan_path = root.join("plan.json");
        let manifest_path = root.join("manifest.json");
        let (mut plan, mut manifest) = path_inputs_for_result(ResultClassV1::Completed);
        let item = plan
            .items
            .first_mut()
            .ok_or_else(|| "fixture item missing".to_string())?;
        let selection = item
            .selection
            .as_mut()
            .ok_or_else(|| "fixture selection missing".to_string())?;
        selection.provider_id =
            crate::providers::cargo_allow::CARGO_ALLOW_PROOF_PROVIDER_ID.to_string();
        selection.capability_id = "cargo-allow.check.no-new".to_string();
        let row = manifest
            .rows
            .first_mut()
            .ok_or_else(|| "fixture row missing".to_string())?;
        row.provider_id = selection.provider_id.clone();
        row.capability_id = selection.capability_id.clone();
        row.receipt.provider = row.provider_id.clone();
        row.receipt.provider_payload_schema =
            crate::providers::cargo_allow::CARGO_ALLOW_PROVIDER_CONTRACT_SCHEMA_ID.to_string();
        row.receipt.provider_payload = serde_json::json!({});
        std::fs::write(
            &plan_path,
            serde_json::to_vec(&plan).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        std::fs::write(
            &manifest_path,
            serde_json::to_vec(&manifest).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        let report = captured_receipt_status_from_paths(&plan_path, &manifest_path)
            .map_err(|error| error.message().to_string())?;
        if report.items.first().map(|item| item.status)
            != Some(ProofItemReceiptStatusV1::ReceiptMalformed)
        {
            return Err("malformed native payload remained structurally valid".to_string());
        }
        let validation = render_captured_receipt_validation(&report, crate::OutputFormat::Json)?;
        if !validation.contains("\"valid\": false") {
            return Err("malformed native payload was reported as valid".to_string());
        }
        std::fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
        Ok(())
    }

    #[cfg(feature = "provider-ripr")]
    #[test]
    fn ripr_native_currentness_fails_closed_without_typed_purpose() -> Result<(), String> {
        let plan_item = serde_json::json!({"intent_obligation_id": "requirement-1"});
        let manifest_row = serde_json::json!({
            "snapshot_identity": "sha256:v1:expected-snapshot",
            "subject_identity": "subject-1",
            "provider_request_identity": "seam-1",
            "receipt": {
                "provider_payload": {
                    "schema_id": crate::providers::ripr::RIPR_GRIP_RECEIPT_SCHEMA_ID,
                    "receipt_id": "receipt-1",
                    "ripr_provider_id": "ripr",
                    "ripr_schema_generation": "generation-1",
                    "analyzer_generation": "analyzer-1",
                    "config_fingerprint": "config-1",
                    "snapshot_digest": "sha256:v1:wrong-snapshot",
                    "subject_ref": "subject-1",
                    "seam_ref": "seam-1",
                    "requirement_id": "requirement-1",
                    "execution_mode": "captured_receipt",
                    "completeness": "complete",
                    "grip_disposition": "likely_discriminating",
                    "receipt_digest": "sha256:v1:receipt"
                }
            }
        });
        let error = validate_native_currentness("proof.ripr.v1", &plan_item, &manifest_row)
            .err()
            .ok_or_else(|| "mismatched RIPR snapshot was accepted".to_string())?;
        if !error.contains("not proven") {
            return Err(format!("unexpected RIPR mismatch: {error}"));
        }
        Ok(())
    }

    #[cfg(feature = "provider-cargo-allow")]
    #[test]
    fn valid_cargo_allow_contract_uses_native_validator() -> Result<(), String> {
        let row = serde_json::json!({
            "receipt": {
                "provider_payload_schema": crate::providers::cargo_allow::CARGO_ALLOW_PROVIDER_CONTRACT_SCHEMA_ID,
                "provider_payload": {
                    "schema_id": crate::providers::cargo_allow::CARGO_ALLOW_PROVIDER_CONTRACT_SCHEMA_ID,
                    "schema_version": 1,
                    "provider_id": crate::providers::cargo_allow::CARGO_ALLOW_PROOF_PROVIDER_ID,
                    "product_name": "cargo-allow",
                    "access_posture": "read_only",
                    "snapshot_bound": true,
                    "discovery_order": ["explicit_environment", "compatibility_config", "path_lookup"],
                    "forbidden_path_prefixes": ["target/", "crates/"],
                    "environment_variable": "CARGO_ALLOW_BIN",
                    "config_relative_path": ".allow/compatibility/proof-delegation.toml",
                    "required_capabilities": ["cargo-allow.check.no-new", "cargo-allow.capabilities.json"]
                }
            }
        });
        validate_native_payload("proof.cargo-allow.v1", &row)
            .map_err(|error| format!("valid cargo-allow contract rejected: {error}"))
    }

    #[cfg(feature = "provider-hawk")]
    #[test]
    fn valid_hawk_receipt_uses_native_validator() -> Result<(), String> {
        let row = serde_json::json!({
            "receipt": {
                "provider_payload_schema": crate::providers::hawk::HAWK_ANALYSIS_RECEIPT_SCHEMA_ID,
                "provider_payload": {
                    "schema_id": crate::providers::hawk::HAWK_ANALYSIS_RECEIPT_SCHEMA_ID,
                    "receipt_id": "receipt-1",
                    "hawk_frontend_digest": "frontend-1",
                    "hawk_driver_digest": "driver-1",
                    "rustc_release": "rustc-1",
                    "rustc_commit": "commit-1",
                    "host_triple": "x86_64-pc-windows-msvc",
                    "hawk_schema_generation": "generation-1",
                    "config_path": "config.toml",
                    "config_digest": "sha256:v1:config",
                    "manifest_digest": "manifest-1",
                    "lockfile_digest": "lockfile-1",
                    "feature_profile": "default",
                    "target_triple": "x86_64-pc-windows-msvc",
                    "snapshot_digest": "sha256:v1:snapshot",
                    "product_name": "cargo-allow",
                    "raw_payload_digest": "sha256:v1:payload",
                    "execution_mode": "captured_report",
                    "findings": []
                }
            }
        });
        validate_native_payload("proof.hawk.v1", &row)
            .map_err(|error| format!("valid Hawk receipt rejected: {error}"))
    }

    #[cfg(feature = "provider-ripr")]
    #[test]
    fn valid_ripr_receipt_uses_native_validator() -> Result<(), String> {
        let row = serde_json::json!({
            "receipt": {
                "provider_payload_schema": crate::providers::ripr::RIPR_GRIP_RECEIPT_SCHEMA_ID,
                "provider_payload": {
                    "schema_id": crate::providers::ripr::RIPR_GRIP_RECEIPT_SCHEMA_ID,
                    "receipt_id": "receipt-1",
                    "ripr_provider_id": "ripr",
                    "ripr_schema_generation": "generation-1",
                    "analyzer_generation": "analyzer-1",
                    "config_fingerprint": "config-1",
                    "snapshot_digest": "sha256:v1:snapshot",
                    "subject_ref": "subject-1",
                    "seam_ref": "seam-1",
                    "requirement_id": "requirement-1",
                    "execution_mode": "captured_receipt",
                    "completeness": "complete",
                    "grip_disposition": "likely_discriminating",
                    "receipt_digest": "sha256:v1:receipt"
                }
            }
        });
        validate_native_payload("proof.ripr.v1", &row)
            .map_err(|error| format!("valid RIPR receipt rejected: {error}"))
    }

    #[cfg(feature = "provider-hawk")]
    #[test]
    fn hawk_schema_mismatch_is_rejected_before_deserialization() -> Result<(), String> {
        let row = serde_json::json!({
            "receipt": {
                "provider_payload_schema": "wrong.hawk.schema",
                "provider_payload": {}
            }
        });
        let error = validate_native_payload("proof.hawk.v1", &row)
            .err()
            .ok_or_else(|| "wrong Hawk schema was accepted".to_string())?;
        if !error.contains("unsupported Hawk payload schema") {
            return Err(format!("unexpected Hawk schema error: {error}"));
        }
        Ok(())
    }

    #[cfg(feature = "provider-ripr")]
    #[test]
    fn ripr_schema_mismatch_is_rejected_before_deserialization() -> Result<(), String> {
        let row = serde_json::json!({
            "receipt": {
                "provider_payload_schema": "wrong.ripr.schema",
                "provider_payload": {}
            }
        });
        let error = validate_native_payload("proof.ripr.v1", &row)
            .err()
            .ok_or_else(|| "wrong RIPR schema was accepted".to_string())?;
        if !error.contains("unsupported RIPR payload schema") {
            return Err(format!("unexpected RIPR schema error: {error}"));
        }
        Ok(())
    }

    #[cfg(feature = "provider-hawk")]
    #[test]
    fn malformed_hawk_payload_is_rejected_by_native_validator() -> Result<(), String> {
        let row = serde_json::json!({
            "receipt": {
                "provider_payload_schema": crate::providers::hawk::HAWK_ANALYSIS_RECEIPT_SCHEMA_ID,
                "provider_payload": {}
            }
        });
        match validate_native_payload("proof.hawk.v1", &row) {
            Err(message) if message.contains("malformed Hawk receipt") => Ok(()),
            Err(message) => Err(format!("unexpected Hawk validation error: {message}")),
            Ok(()) => Err("malformed Hawk payload was accepted".to_string()),
        }
    }

    #[cfg(feature = "provider-ripr")]
    #[test]
    fn malformed_ripr_payload_is_rejected_by_native_validator() -> Result<(), String> {
        let row = serde_json::json!({
            "receipt": {
                "provider_payload_schema": crate::providers::ripr::RIPR_GRIP_RECEIPT_SCHEMA_ID,
                "provider_payload": {}
            }
        });
        match validate_native_payload("proof.ripr.v1", &row) {
            Err(message) if message.contains("malformed RIPR receipt") => Ok(()),
            Err(message) => Err(format!("unexpected RIPR validation error: {message}")),
            Ok(()) => Err("malformed RIPR payload was accepted".to_string()),
        }
    }
}
