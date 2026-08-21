//! Read-only explain and reconcile projections for captured receipt status.

use crate::{ProviderAvailabilityV1, ProviderDispositionV1, StaticProviderRegistryV1};
use proof_engine::{
    ProofItemReceiptStatusRowV1, ProofItemReceiptStatusV1, RECEIPT_STATUS_REPORT_SCHEMA_ID,
    ReceiptStatusReportV1,
};
use proof_protocol::{ProofItemDispositionV1, ProofPlanV2, ProofSubjectV1};
use serde::Serialize;
use std::collections::BTreeMap;

pub const RECEIPT_EXPLAIN_SCHEMA_ID: &str = "proof.receipt-explain.v1";
pub const RECEIPT_RECONCILE_SCHEMA_ID: &str = "proof.receipt-reconcile.v1";
const CLAIM_BOUNDARY: &str = "Captured receipt evidence was projected read-only; no provider executed, no source mutated, and no phase gate was opened.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiptProjectionError {
    MissingSelector(String),
    AmbiguousSelector(String),
    InvalidBinding(String),
}

impl ReceiptProjectionError {
    pub fn message(&self) -> &str {
        match self {
            Self::MissingSelector(message)
            | Self::AmbiguousSelector(message)
            | Self::InvalidBinding(message) => message,
        }
    }
}

impl std::fmt::Display for ReceiptProjectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message())
    }
}

impl From<ReceiptProjectionError> for String {
    fn from(error: ReceiptProjectionError) -> Self {
        error.to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReceiptExplainItemV1 {
    pub proof_item_id: String,
    pub intent_obligation_id: String,
    pub phase: String,
    pub blocking: bool,
    pub disposition: ProofItemDispositionV1,
    pub provider_id: Option<String>,
    pub capability_id: Option<String>,
    pub provider_availability: Option<ProviderDispositionV1>,
    pub subject: ProofSubjectV1,
    pub snapshot_identity: String,
    pub expected_currentness_dimensions: Vec<String>,
    pub captured_status: ProofItemReceiptStatusV1,
    pub reason: String,
    pub limitations: Vec<String>,
    pub next_action: String,
    pub claim_boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReceiptExplainProjectionV1 {
    pub schema_id: String,
    pub plan_id: String,
    pub item: ReceiptExplainItemV1,
    pub claim_boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReceiptReconcileItemV1 {
    pub proof_item_id: String,
    pub blocking: bool,
    pub disposition: ProofItemDispositionV1,
    pub status: ProofItemReceiptStatusV1,
    pub provider_id: Option<String>,
    pub reason: String,
    pub next_action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReceiptReconcileProjectionV1 {
    pub schema_id: String,
    pub plan_id: String,
    pub items: Vec<ReceiptReconcileItemV1>,
    pub status_counts: BTreeMap<String, usize>,
    pub provider_availability: Vec<ProviderAvailabilityV1>,
    pub outstanding: Vec<String>,
    pub claim_boundary: String,
}

pub fn explain_receipt_item(
    plan: &ProofPlanV2,
    report: &ReceiptStatusReportV1,
    registry: &StaticProviderRegistryV1,
    selector: &str,
) -> Result<ReceiptExplainProjectionV1, ReceiptProjectionError> {
    validate_report_binding(plan, report)?;
    let item = plan_item_for_selector(plan, selector)?;
    let row = report_row(report, &item.proof_item_id)?;
    let provider_id = row.provider_id.clone();
    let provider_availability = provider_id
        .as_deref()
        .map(|provider| provider_disposition(registry, provider));
    Ok(ReceiptExplainProjectionV1 {
        schema_id: RECEIPT_EXPLAIN_SCHEMA_ID.to_string(),
        plan_id: plan.plan_id.clone(),
        item: explain_item(item, row, provider_availability),
        claim_boundary: CLAIM_BOUNDARY.to_string(),
    })
}

pub fn reconcile_receipts(
    plan: &ProofPlanV2,
    report: &ReceiptStatusReportV1,
    registry: &StaticProviderRegistryV1,
) -> Result<ReceiptReconcileProjectionV1, ReceiptProjectionError> {
    validate_report_binding(plan, report)?;
    let mut items = Vec::with_capacity(plan.items.len());
    let mut status_counts = BTreeMap::new();
    let mut outstanding = Vec::new();
    for plan_item in &plan.items {
        let row = report_row(report, &plan_item.proof_item_id)?;
        let status_name = row.status.as_str().to_string();
        *status_counts.entry(status_name).or_insert(0) += 1;
        let next_action = next_action(row.status);
        if next_action != "none" {
            outstanding.push(format!("{}: {next_action}", plan_item.proof_item_id));
        }
        items.push(ReceiptReconcileItemV1 {
            proof_item_id: plan_item.proof_item_id.clone(),
            blocking: plan_item.blocking,
            disposition: plan_item.disposition,
            status: row.status,
            provider_id: row.provider_id.clone(),
            reason: row.reason.clone(),
            next_action: next_action.to_string(),
        });
    }
    Ok(ReceiptReconcileProjectionV1 {
        schema_id: RECEIPT_RECONCILE_SCHEMA_ID.to_string(),
        plan_id: plan.plan_id.clone(),
        items,
        status_counts,
        provider_availability: registry.availability(),
        outstanding,
        claim_boundary: CLAIM_BOUNDARY.to_string(),
    })
}

pub fn render_receipt_explain(
    projection: &ReceiptExplainProjectionV1,
    format: crate::OutputFormat,
) -> Result<String, String> {
    match format {
        crate::OutputFormat::Json => serde_json::to_string_pretty(projection)
            .map(|json| format!("{json}\n"))
            .map_err(|error| error.to_string()),
        crate::OutputFormat::Human => Ok(format!(
            "proof item {}\nobligation: {}\nphase: {}\nblocking: {}\ndisposition: {}\nprovider: {}\ncapability: {}\nprovider availability: {}\nsubject: {:?}\nsnapshot: {}\nexpected currentness: {:?}\ncaptured status: {}\nreason: {}\nlimitations: {:?}\nnext action: {}\nclaim boundary: {}\n",
            projection.item.proof_item_id,
            projection.item.intent_obligation_id,
            projection.item.phase,
            projection.item.blocking,
            projection.item.disposition.as_str(),
            projection.item.provider_id.as_deref().unwrap_or("none"),
            projection.item.capability_id.as_deref().unwrap_or("none"),
            projection
                .item
                .provider_availability
                .map(|availability| format!("{availability:?}"))
                .unwrap_or_else(|| "none".to_string()),
            projection.item.subject,
            projection.item.snapshot_identity,
            projection.item.expected_currentness_dimensions,
            projection.item.captured_status.as_str(),
            projection.item.reason,
            projection.item.limitations,
            projection.item.next_action,
            projection.claim_boundary
        )),
    }
}

pub fn render_receipt_reconcile(
    projection: &ReceiptReconcileProjectionV1,
    format: crate::OutputFormat,
) -> Result<String, String> {
    match format {
        crate::OutputFormat::Json => serde_json::to_string_pretty(projection)
            .map(|json| format!("{json}\n"))
            .map_err(|error| error.to_string()),
        crate::OutputFormat::Human => {
            let mut output = format!(
                "proof reconcile {} ({} items)\nstatus counts: {:?}\nprovider availability: {:?}\noutstanding: {:?}\n",
                projection.plan_id,
                projection.items.len(),
                projection.status_counts,
                projection.provider_availability,
                projection.outstanding,
            );
            for item in &projection.items {
                output.push_str(&format!(
                    "{}: blocking={} disposition={} provider={} status={} reason={} next_action={}\n",
                    item.proof_item_id,
                    item.blocking,
                    item.disposition.as_str(),
                    item.provider_id.as_deref().unwrap_or("none"),
                    item.status.as_str(),
                    item.reason,
                    item.next_action
                ));
            }
            output.push_str(&format!("claim boundary: {}\n", projection.claim_boundary));
            Ok(output)
        }
    }
}

fn plan_item_for_selector<'a>(
    plan: &'a ProofPlanV2,
    selector: &str,
) -> Result<&'a proof_protocol::ProofItemV1, ReceiptProjectionError> {
    let matches = plan
        .items
        .iter()
        .filter(|item| item.proof_item_id == selector || item.intent_obligation_id == selector)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => Err(ReceiptProjectionError::MissingSelector(format!(
            "no proof item or obligation matches {selector}"
        ))),
        [item] => Ok(item),
        _ => Err(ReceiptProjectionError::AmbiguousSelector(format!(
            "selector {selector} matches multiple proof items"
        ))),
    }
}

fn report_row<'a>(
    report: &'a ReceiptStatusReportV1,
    proof_item_id: &str,
) -> Result<&'a ProofItemReceiptStatusRowV1, ReceiptProjectionError> {
    report
        .items
        .iter()
        .find(|row| row.proof_item_id == proof_item_id)
        .ok_or_else(|| {
            ReceiptProjectionError::InvalidBinding(format!(
                "receipt status report has no row for proof item {proof_item_id}"
            ))
        })
}

fn validate_report_binding(
    plan: &ProofPlanV2,
    report: &ReceiptStatusReportV1,
) -> Result<(), ReceiptProjectionError> {
    plan.validate()
        .map_err(ReceiptProjectionError::InvalidBinding)?;
    if report.schema_id != RECEIPT_STATUS_REPORT_SCHEMA_ID {
        return Err(ReceiptProjectionError::InvalidBinding(format!(
            "unexpected receipt status schema {}",
            report.schema_id
        )));
    }
    if report.plan_id != plan.plan_id {
        return Err(ReceiptProjectionError::InvalidBinding(format!(
            "receipt status report belongs to plan {}, expected {}",
            report.plan_id, plan.plan_id
        )));
    }
    if report.items.len() != plan.items.len() {
        return Err(ReceiptProjectionError::InvalidBinding(
            "receipt status report must contain exactly one row per proof item".to_string(),
        ));
    }
    let mut seen = std::collections::BTreeSet::new();
    for row in &report.items {
        if !seen.insert(row.proof_item_id.as_str()) {
            return Err(ReceiptProjectionError::InvalidBinding(format!(
                "duplicate receipt status row {}",
                row.proof_item_id
            )));
        }
        let item = plan
            .items
            .iter()
            .find(|item| item.proof_item_id == row.proof_item_id)
            .ok_or_else(|| {
                ReceiptProjectionError::InvalidBinding(format!(
                    "receipt status row {} does not belong to the proof plan",
                    row.proof_item_id
                ))
            })?;
        let Some(selection) = item.selection.as_ref() else {
            continue;
        };
        if row.provider_id.as_deref() != Some(selection.provider_id.as_str())
            || row.capability_id.as_deref() != Some(selection.capability_id.as_str())
        {
            return Err(ReceiptProjectionError::InvalidBinding(format!(
                "receipt status row {} provider identity does not match the plan",
                row.proof_item_id
            )));
        }
    }
    Ok(())
}

fn explain_item(
    item: &proof_protocol::ProofItemV1,
    row: &ProofItemReceiptStatusRowV1,
    provider_availability: Option<ProviderDispositionV1>,
) -> ReceiptExplainItemV1 {
    ReceiptExplainItemV1 {
        proof_item_id: item.proof_item_id.clone(),
        intent_obligation_id: item.intent_obligation_id.clone(),
        phase: item.phase.clone(),
        blocking: item.blocking,
        disposition: item.disposition,
        provider_id: row.provider_id.clone(),
        capability_id: row.capability_id.clone(),
        provider_availability,
        subject: item.subject.clone(),
        snapshot_identity: item.snapshot_identity.clone(),
        expected_currentness_dimensions: item
            .expected_receipt
            .as_ref()
            .map(|expected| expected.currentness_dimensions.clone())
            .unwrap_or_default(),
        captured_status: row.status,
        reason: row.reason.clone(),
        limitations: item.limitations.clone(),
        next_action: next_action(row.status).to_string(),
        claim_boundary: item.claim_boundary.clone(),
    }
}

fn provider_disposition(
    registry: &StaticProviderRegistryV1,
    provider_id: &str,
) -> ProviderDispositionV1 {
    if registry.provider_available(provider_id) {
        ProviderDispositionV1::Selected
    } else {
        ProviderDispositionV1::ProviderUnavailable
    }
}

fn next_action(status: ProofItemReceiptStatusV1) -> &'static str {
    match status {
        ProofItemReceiptStatusV1::SatisfiedByCurrentReceipt => "none",
        ProofItemReceiptStatusV1::ReceiptMissing => "capture the required receipt",
        ProofItemReceiptStatusV1::ReceiptStale => "recapture against the current snapshot",
        ProofItemReceiptStatusV1::ReceiptMalformed
        | ProofItemReceiptStatusV1::ReceiptForDifferentItem => "repair the receipt binding",
        ProofItemReceiptStatusV1::ProviderUnavailable => "enable the selected provider feature",
        ProofItemReceiptStatusV1::ManualOrNativeOutstanding => {
            "complete the manual or native evidence step"
        }
        ProofItemReceiptStatusV1::NotApplicable => "record the applicability decision",
        ProofItemReceiptStatusV1::Conflict => "resolve the conflicting receipt evidence",
        ProofItemReceiptStatusV1::CurrentFindings
        | ProofItemReceiptStatusV1::CurrentFailed
        | ProofItemReceiptStatusV1::CurrentPartial
        | ProofItemReceiptStatusV1::CurrentUnsupported
        | ProofItemReceiptStatusV1::CurrentNotProven
        | ProofItemReceiptStatusV1::CurrentInstrumentFailure => {
            "review the captured provider result"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> Result<ProofPlanV2, String> {
        serde_json::from_value(serde_json::json!({
            "schema_id": "proof.plan.v2",
            "schema_version": 1,
            "plan_id": "plan-1",
            "intent_plan_digest": "intent-1",
            "snapshot_identity": "snapshot-1",
            "items": [{
                "proof_item_id": "item-1",
                "intent_obligation_id": "obligation-1",
                "phase": "precommit",
                "blocking": true,
                "evidence_purpose_ref": "purpose-1",
                "required_capability_class": "capability-1",
                "snapshot_identity": "snapshot-1",
                "subject": {
                    "subject_class": "commit",
                    "revision": "snapshot-1",
                    "selector": null,
                    "body_identity": null,
                    "limitations": []
                },
                "disposition": "manual_or_native_outstanding",
                "selection": null,
                "current_receipt": null,
                "expected_receipt": null,
                "execution_posture": "manual_native",
                "dependency_group": null,
                "limitations": ["captured-only"],
                "claim_boundary": "read-only"
            }]
        }))
        .map_err(|error| error.to_string())
    }

    fn report(status: ProofItemReceiptStatusV1) -> ReceiptStatusReportV1 {
        ReceiptStatusReportV1 {
            schema_id: proof_engine::RECEIPT_STATUS_REPORT_SCHEMA_ID.to_string(),
            plan_id: "plan-1".to_string(),
            items: vec![ProofItemReceiptStatusRowV1 {
                proof_item_id: "item-1".to_string(),
                status,
                provider_id: Some("proof.cargo-allow.v1".to_string()),
                capability_id: Some("capability-1".to_string()),
                reason: "captured status".to_string(),
            }],
            claim_boundary: "read-only".to_string(),
        }
    }

    #[test]
    fn explain_is_typed_and_selector_is_deterministic() -> Result<(), String> {
        let registry = StaticProviderRegistryV1::selected().map_err(|error| error.as_str())?;
        let projection = explain_receipt_item(
            &plan()?,
            &report(ProofItemReceiptStatusV1::ReceiptMissing),
            &registry,
            "obligation-1",
        )?;
        if projection.item.proof_item_id != "item-1"
            || projection.item.captured_status != ProofItemReceiptStatusV1::ReceiptMissing
            || projection.item.next_action != "capture the required receipt"
        {
            return Err("explain projection lost typed item state".to_string());
        }
        let json = render_receipt_explain(&projection, crate::OutputFormat::Json)?;
        let human = render_receipt_explain(&projection, crate::OutputFormat::Human)?;
        if !json.contains("proof.receipt-explain.v1")
            || !human.contains("receipt_missing")
            || !human.contains("disposition:")
            || !human.contains("snapshot:")
            || !human.contains("expected currentness:")
            || !human.contains("limitations:")
        {
            return Err("explain projections diverged from the typed state".to_string());
        }
        Ok(())
    }

    #[test]
    fn reconcile_preserves_plan_order_and_reports_outstanding_work() -> Result<(), String> {
        let registry = StaticProviderRegistryV1::selected().map_err(|error| error.as_str())?;
        let projection = reconcile_receipts(
            &plan()?,
            &report(ProofItemReceiptStatusV1::CurrentNotProven),
            &registry,
        )?;
        if projection
            .items
            .first()
            .map(|item| item.proof_item_id.as_str())
            != Some("item-1")
            || projection.outstanding.len() != 1
            || projection.status_counts.get("current_not_proven") != Some(&1)
        {
            return Err("reconcile projection lost blocking item state".to_string());
        }
        let human = render_receipt_reconcile(&projection, crate::OutputFormat::Human)?;
        let json = render_receipt_reconcile(&projection, crate::OutputFormat::Json)?;
        if !human.contains("current_not_proven")
            || !human.contains("disposition=")
            || !human.contains("status counts:")
            || !json.contains("proof.receipt-reconcile.v1")
        {
            return Err("reconcile projections diverged from the typed state".to_string());
        }
        Ok(())
    }

    #[test]
    fn explain_rejects_unknown_and_ambiguous_selectors() -> Result<(), String> {
        let registry = StaticProviderRegistryV1::selected().map_err(|error| error.as_str())?;
        if explain_receipt_item(
            &plan()?,
            &report(ProofItemReceiptStatusV1::ReceiptMissing),
            &registry,
            "unknown",
        )
        .is_ok()
        {
            return Err("unknown explain selector was accepted".to_string());
        }
        let mut duplicate = plan()?;
        let mut second = duplicate
            .items
            .first()
            .cloned()
            .ok_or_else(|| "fixture item missing".to_string())?;
        second.proof_item_id = "item-2".to_string();
        duplicate.items.push(second);
        let mut duplicate_report = report(ProofItemReceiptStatusV1::ReceiptMissing);
        duplicate_report.items.push(ProofItemReceiptStatusRowV1 {
            proof_item_id: "item-2".to_string(),
            status: ProofItemReceiptStatusV1::ReceiptMissing,
            provider_id: Some("proof.cargo-allow.v1".to_string()),
            capability_id: Some("capability-1".to_string()),
            reason: "captured status".to_string(),
        });
        if explain_receipt_item(&duplicate, &duplicate_report, &registry, "obligation-1").is_ok() {
            return Err("ambiguous explain selector was accepted".to_string());
        }
        Ok(())
    }

    #[test]
    fn every_status_has_a_deterministic_remediation() -> Result<(), String> {
        let statuses = [
            ProofItemReceiptStatusV1::SatisfiedByCurrentReceipt,
            ProofItemReceiptStatusV1::CurrentFindings,
            ProofItemReceiptStatusV1::CurrentFailed,
            ProofItemReceiptStatusV1::CurrentPartial,
            ProofItemReceiptStatusV1::CurrentUnsupported,
            ProofItemReceiptStatusV1::CurrentNotProven,
            ProofItemReceiptStatusV1::CurrentInstrumentFailure,
            ProofItemReceiptStatusV1::ReceiptMissing,
            ProofItemReceiptStatusV1::ReceiptMalformed,
            ProofItemReceiptStatusV1::ReceiptStale,
            ProofItemReceiptStatusV1::ReceiptForDifferentItem,
            ProofItemReceiptStatusV1::ProviderUnavailable,
            ProofItemReceiptStatusV1::ManualOrNativeOutstanding,
            ProofItemReceiptStatusV1::NotApplicable,
            ProofItemReceiptStatusV1::Conflict,
        ];
        for status in statuses {
            if next_action(status).is_empty() {
                return Err(format!("status {status:?} has no remediation"));
            }
        }
        Ok(())
    }

    #[test]
    fn binding_rejects_duplicate_or_omitted_rows() -> Result<(), String> {
        let registry = StaticProviderRegistryV1::selected().map_err(|error| error.as_str())?;
        let plan = plan()?;
        let mut duplicate = report(ProofItemReceiptStatusV1::ReceiptMissing);
        duplicate.items.push(
            duplicate
                .items
                .first()
                .cloned()
                .ok_or_else(|| "fixture row missing".to_string())?,
        );
        if !matches!(
            reconcile_receipts(&plan, &duplicate, &registry),
            Err(ReceiptProjectionError::InvalidBinding(_))
        ) {
            return Err("duplicate report rows were accepted".to_string());
        }
        let mut omitted = plan.clone();
        omitted.items.push({
            let mut item = omitted
                .items
                .first()
                .cloned()
                .ok_or_else(|| "fixture item missing".to_string())?;
            item.proof_item_id = "item-2".to_string();
            item
        });
        if !matches!(
            reconcile_receipts(
                &omitted,
                &report(ProofItemReceiptStatusV1::ReceiptMissing),
                &registry
            ),
            Err(ReceiptProjectionError::InvalidBinding(_))
        ) {
            return Err("omitted report row was accepted".to_string());
        }
        Ok(())
    }
}
