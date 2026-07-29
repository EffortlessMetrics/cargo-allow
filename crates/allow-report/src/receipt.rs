use crate::advisory_class::AdvisoryClass;
use crate::artifacts::federation::FederationReportContext;
use crate::contracts::{CLAIM_BOUNDARY, RECEIPT_ARTIFACT, SCANNER_LIMITATIONS};
use crate::evidence_repair::evidence_repair_queues_from_context;
use crate::source_inventory::render_source_inventory_value;
use crate::{
    ARTIFACT_STATUS_ERROR, RECEIPT_COMMAND_CHECK, RECEIPT_COMMANDS, ReportContext,
    STATUS_COUNT_ORDER, Summary,
};
use allow_core::{Finding, MatchOutcome};
use serde_json::{Map, Value};

pub fn render_receipt(command: &str, outcomes: &[MatchOutcome], failed: bool) -> String {
    render_receipt_with_context(command, outcomes, failed, ReportContext::default())
}

pub fn render_receipt_with_context(
    command: &str,
    outcomes: &[MatchOutcome],
    failed: bool,
    context: ReportContext<'_>,
) -> String {
    render_receipt_json(command, None, outcomes, failed, context)
}

pub fn render_receipt_with_context_and_inventory(
    command: &str,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    failed: bool,
    context: ReportContext<'_>,
) -> String {
    render_receipt_json(command, Some(findings), outcomes, failed, context)
}

pub fn render_error_receipt(diagnostic: &str, context: ReportContext<'_>) -> String {
    render_receipt_value(ReceiptRenderInput {
        command: RECEIPT_COMMAND_CHECK,
        findings: None,
        outcomes: &[],
        summary: &Summary::default(),
        status: ARTIFACT_STATUS_ERROR,
        failed: true,
        diagnostic: Some(diagnostic),
        context,
    })
}

fn render_receipt_json(
    command: &str,
    findings: Option<&[Finding]>,
    outcomes: &[MatchOutcome],
    failed: bool,
    context: ReportContext<'_>,
) -> String {
    assert!(
        RECEIPT_COMMANDS.contains(&command),
        "receipt artifacts support only registered receipt commands"
    );
    let summary = Summary::from_outcomes(outcomes);
    render_receipt_value(ReceiptRenderInput {
        command,
        findings,
        outcomes,
        summary: &summary,
        status: if failed {
            crate::ARTIFACT_STATUS_FAILED
        } else {
            crate::ARTIFACT_STATUS_PASSED
        },
        failed,
        diagnostic: None,
        context,
    })
}

struct ReceiptRenderInput<'a> {
    command: &'a str,
    findings: Option<&'a [Finding]>,
    outcomes: &'a [MatchOutcome],
    summary: &'a Summary,
    status: &'a str,
    failed: bool,
    diagnostic: Option<&'a str>,
    context: ReportContext<'a>,
}

fn render_receipt_value(input: ReceiptRenderInput<'_>) -> String {
    let ReceiptRenderInput {
        command,
        findings,
        outcomes,
        summary,
        status,
        failed,
        diagnostic,
        context,
    } = input;
    let mut artifact = Map::new();
    artifact.insert(
        "schema_version".to_string(),
        Value::from(RECEIPT_ARTIFACT.schema_version),
    );
    artifact.insert(
        "schema_id".to_string(),
        Value::String(RECEIPT_ARTIFACT.schema_id.to_string()),
    );
    artifact.insert("tool".to_string(), Value::String("cargo-allow".to_string()));
    artifact.insert("command".to_string(), Value::String(command.to_string()));
    artifact.insert("status".to_string(), Value::String(status.to_string()));
    artifact.insert("failed".to_string(), Value::Bool(failed));

    insert_run_metadata(&mut artifact, context);
    artifact.insert(
        "claim_boundary".to_string(),
        string_array_value(CLAIM_BOUNDARY),
    );
    artifact.insert(
        "scanner_limitations".to_string(),
        string_array_value(SCANNER_LIMITATIONS),
    );
    artifact.insert("inventory".to_string(), inventory_value(context));

    if let Some(diagnostic) = diagnostic {
        artifact.insert(
            "diagnostic".to_string(),
            Value::String(diagnostic.to_string()),
        );
    }

    artifact.insert("counts".to_string(), counts_value(summary, context));
    artifact.insert("advisory".to_string(), advisory_value(summary, context));

    // #1858: always emit evidence_repair_queues (even when empty) for
    // consistent empty-handling across artifacts.
    let queues = evidence_repair_queues_from_context(summary, context);
    artifact.insert(
        "evidence_repair_queues".to_string(),
        Value::Array(queues.into_iter().map(queue_value).collect()),
    );

    if let Some(source_inventory) =
        findings.and_then(|findings| render_source_inventory_value(findings, outcomes))
    {
        artifact.insert("source_inventory".to_string(), source_inventory);
    }

    serialize_artifact(Value::Object(artifact))
}

fn insert_run_metadata(artifact: &mut Map<String, Value>, context: ReportContext<'_>) {
    insert_optional_string(artifact, "mode", context.mode);
    insert_optional_string(artifact, "enforcement", context.enforcement);
    insert_optional_string(artifact, "policy_config", context.policy_config);
    insert_optional_string(artifact, "tool_version", context.tool_version);

    if let Some(lane_posture) = context.lane_posture {
        let mut posture = Map::new();
        for (lane, mode) in lane_posture {
            posture.insert(lane.clone(), Value::String(mode.as_str().to_string()));
        }
        artifact.insert("lane_posture".to_string(), Value::Object(posture));
    }
    if let Some(federation) = context.federation {
        artifact.insert("federation".to_string(), federation_value(federation));
    }
    insert_optional_string(artifact, "git_sha", context.git_sha);
    insert_optional_string(artifact, "policy_digest", context.policy_digest);
    insert_optional_string(artifact, "started_at", context.started_at);
    insert_optional_string(artifact, "run_id", context.run_id);
}

fn insert_optional_string(artifact: &mut Map<String, Value>, name: &str, value: Option<&str>) {
    if let Some(value) = value {
        artifact.insert(name.to_string(), Value::String(value.to_string()));
    }
}

fn inventory_value(context: ReportContext<'_>) -> Value {
    let inventory = context.inventory;
    let mut value = Map::new();
    value.insert(
        "scope".to_string(),
        Value::String(inventory.scope.to_string()),
    );
    value.insert(
        "scanner".to_string(),
        Value::String(inventory.scanner.to_string()),
    );
    value.insert(
        "source".to_string(),
        Value::String(inventory.source.to_string()),
    );
    if let Some(root) = inventory.root {
        value.insert("root".to_string(), Value::String(root.to_string()));
    }
    if let Some(files_scanned) = inventory.files_scanned {
        value.insert("files_scanned".to_string(), Value::from(files_scanned));
    }
    if inventory.empty_git_tracked {
        value.insert("empty_git_tracked".to_string(), Value::Bool(true));
    }
    if let Some(completeness) = inventory.completeness {
        value.insert(
            "completeness".to_string(),
            Value::String(completeness.to_string()),
        );
    }
    Value::Object(value)
}

fn counts_value(summary: &Summary, context: ReportContext<'_>) -> Value {
    let mut counts = Map::new();
    for status in STATUS_COUNT_ORDER {
        counts.insert(
            status.as_str().to_string(),
            Value::from(summary.count(status)),
        );
    }

    let optional_fields = [
        (
            "policy_baseline_debt",
            context
                .baseline_debt_entries
                .filter(|count| *count > summary.count(allow_core::MatchStatus::BaselineDebt)),
        ),
        (
            "policy_missing_evidence",
            context
                .policy_missing_evidence_entries
                .filter(|count| *count > summary.count(allow_core::MatchStatus::EvidenceMissing)),
        ),
        (
            "broken_evidence_links",
            context.broken_evidence_links.filter(|count| *count > 0),
        ),
        (
            "weak_evidence_references",
            context.weak_evidence_references.filter(|count| *count > 0),
        ),
        (
            "blocking_divergence",
            context
                .blocking_divergence_entries
                .filter(|count| *count > 0),
        ),
    ];
    for (name, value) in optional_fields
        .into_iter()
        .filter_map(|(name, value)| value.map(|value| (name, value)))
    {
        counts.insert(name.to_string(), Value::from(value));
    }
    Value::Object(counts)
}

fn advisory_value(summary: &Summary, context: ReportContext<'_>) -> Value {
    let mut advisory = Map::new();
    for (class, value) in AdvisoryClass::receipt_fields(summary, context) {
        advisory.insert(class.field_name().to_string(), Value::from(value));
    }
    Value::Object(advisory)
}

fn queue_value(queue: crate::evidence_repair::EvidenceRepairQueue) -> Value {
    let mut value = Map::new();
    value.insert(
        "signal".to_string(),
        Value::String(queue.signal.to_string()),
    );
    value.insert("label".to_string(), Value::String(queue.label.to_string()));
    value.insert(
        "route_kind".to_string(),
        Value::String(queue.route_kind.to_string()),
    );
    if let Some(item_kind) = queue.item_kind {
        value.insert(
            "item_kind".to_string(),
            Value::String(item_kind.to_string()),
        );
    }
    if let Some(worklist_filter) = queue.worklist_filter {
        value.insert(
            "worklist_filter".to_string(),
            Value::String(worklist_filter.to_string()),
        );
    }
    value.insert("count".to_string(), Value::from(queue.count));
    value.insert(
        "command".to_string(),
        Value::String(queue.command.to_string()),
    );
    Value::Object(value)
}

fn federation_value(federation: FederationReportContext<'_>) -> Value {
    let mut value = Map::new();
    value.insert(
        "federation_version".to_string(),
        optional_string_value(federation.federation_version),
    );
    value.insert(
        "precedence_applied".to_string(),
        optional_string_value(federation.precedence_applied),
    );

    let contributors = federation
        .ledger_contributors
        .unwrap_or(&[])
        .iter()
        .map(|contributor| {
            let mut item = Map::new();
            item.insert("id".to_string(), Value::String(contributor.id.to_string()));
            item.insert(
                "path".to_string(),
                Value::String(contributor.path.to_string()),
            );
            item.insert(
                "role".to_string(),
                Value::String(contributor.role.to_string()),
            );
            item.insert(
                "dialect".to_string(),
                Value::String(contributor.dialect.to_string()),
            );
            item.insert(
                "mode".to_string(),
                Value::String(contributor.mode.to_string()),
            );
            item.insert("priority".to_string(), Value::from(contributor.priority));
            item.insert("lanes".to_string(), string_array_value(contributor.lanes));
            Value::Object(item)
        })
        .collect();
    value.insert(
        "ledger_contributors".to_string(),
        Value::Array(contributors),
    );

    if let Some(summary) = federation.divergence_summary {
        let mut divergence = Map::new();
        let counts = summary
            .counts_by_kind
            .unwrap_or(&[])
            .iter()
            .map(|count| {
                let mut item = Map::new();
                item.insert("kind".to_string(), Value::String(count.kind.to_string()));
                item.insert("count".to_string(), Value::from(count.count));
                Value::Object(item)
            })
            .collect();
        divergence.insert("counts_by_kind".to_string(), Value::Array(counts));

        let records = summary
            .records
            .unwrap_or(&[])
            .iter()
            .map(|record| {
                let mut item = Map::new();
                item.insert("kind".to_string(), Value::String(record.kind.to_string()));
                item.insert(
                    "message".to_string(),
                    Value::String(record.message.to_string()),
                );
                item.insert(
                    "canonical_ledger_id".to_string(),
                    Value::String(record.canonical_ledger_id.to_string()),
                );
                item.insert(
                    "mirror_ledger_id".to_string(),
                    Value::String(record.mirror_ledger_id.to_string()),
                );
                item.insert(
                    "canonical_path".to_string(),
                    Value::String(record.canonical_path.to_string()),
                );
                item.insert(
                    "mirror_path".to_string(),
                    Value::String(record.mirror_path.to_string()),
                );
                item.insert(
                    "sample_entry_ids".to_string(),
                    string_array_value(record.sample_entry_ids),
                );
                item.insert(
                    "canonical_fingerprint".to_string(),
                    optional_string_value(record.canonical_fingerprint),
                );
                item.insert(
                    "mirror_fingerprint".to_string(),
                    optional_string_value(record.mirror_fingerprint),
                );
                item.insert(
                    "recommended_action".to_string(),
                    Value::String(record.recommended_action.to_string()),
                );
                Value::Object(item)
            })
            .collect();
        divergence.insert("records".to_string(), Value::Array(records));
        value.insert("divergence_summary".to_string(), Value::Object(divergence));
    }
    Value::Object(value)
}

fn optional_string_value(value: Option<&str>) -> Value {
    value
        .map(|value| Value::String(value.to_string()))
        .unwrap_or(Value::Null)
}

fn string_array_value<T: AsRef<str>>(values: &[T]) -> Value {
    Value::Array(
        values
            .iter()
            .map(|value| Value::String(value.as_ref().to_string()))
            .collect(),
    )
}

fn serialize_artifact(value: Value) -> String {
    match serde_json::to_string_pretty(&value) {
        Ok(json) => format!("{json}\n"),
        Err(error) => {
            let mut fallback = Map::new();
            fallback.insert(
                "schema_version".to_string(),
                Value::from(RECEIPT_ARTIFACT.schema_version),
            );
            fallback.insert(
                "schema_id".to_string(),
                Value::String(RECEIPT_ARTIFACT.schema_id.to_string()),
            );
            fallback.insert("tool".to_string(), Value::String("cargo-allow".to_string()));
            fallback.insert(
                "command".to_string(),
                Value::String(RECEIPT_COMMAND_CHECK.to_string()),
            );
            fallback.insert(
                "status".to_string(),
                Value::String(ARTIFACT_STATUS_ERROR.to_string()),
            );
            fallback.insert("failed".to_string(), Value::Bool(true));
            fallback.insert(
                "diagnostic".to_string(),
                Value::String(format!("receipt serialization failed: {error}")),
            );
            format!("{}\n", Value::Object(fallback))
        }
    }
}
