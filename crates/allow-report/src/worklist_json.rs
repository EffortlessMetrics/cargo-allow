use crate::contracts::WORKLIST_ARTIFACT;
use crate::json::{bool_json, json_string_array, option_json, push_json_fixed_artifact_preamble};
use crate::worklist_summary::{
    worklist_difficulty_count, worklist_kind_counts, worklist_risk_count,
};
use crate::{InventoryContext, WorklistFilters, WorklistItem};
use allow_core::json_escape;

pub fn render_worklist_json(
    items: &[WorklistItem<'_>],
    filters: WorklistFilters<'_>,
    inventory: InventoryContext<'_>,
) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    push_json_fixed_artifact_preamble(&mut out, WORKLIST_ARTIFACT, inventory);
    out.push_str("  \"filters\": ");
    out.push_str(&render_worklist_filters_json(filters, "  "));
    out.push_str(",\n");
    out.push_str("  \"summary\": {\n");
    out.push_str(&format!("    \"work_items\": {},\n", items.len()));
    out.push_str(&format!(
        "    \"high\": {},\n",
        worklist_risk_count(items, "high")
    ));
    out.push_str(&format!(
        "    \"medium\": {},\n",
        worklist_risk_count(items, "medium")
    ));
    out.push_str(&format!(
        "    \"low\": {},\n",
        worklist_risk_count(items, "low")
    ));
    out.push_str(&format!(
        "    \"small_difficulty\": {},\n",
        worklist_difficulty_count(items, "small")
    ));
    out.push_str(&format!(
        "    \"medium_difficulty\": {}",
        worklist_difficulty_count(items, "medium")
    ));
    let kind_counts = worklist_kind_counts(items);
    if kind_counts.is_empty() {
        out.push('\n');
    } else {
        out.push_str(",\n");
        out.push_str("    \"item_kinds\": {\n");
        for (index, (kind, count)) in kind_counts.iter().enumerate() {
            if index > 0 {
                out.push_str(",\n");
            }
            out.push_str(&format!("      \"{}\": {}", json_escape(kind), count));
        }
        out.push_str("\n    }\n");
    }
    out.push_str("  },\n");
    out.push_str("  \"work_items\": [\n");
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str(&render_work_item_json(item));
    }
    out.push_str("\n  ]\n");
    out.push_str("}\n");
    out
}

fn render_work_item_json(item: &WorklistItem<'_>) -> String {
    let mut fields = vec![
        format!("      \"id\": \"{}\"", json_escape(item.id)),
        format!("      \"kind\": \"{}\"", json_escape(item.kind)),
    ];
    push_optional_json_string(&mut fields, "exception_kind", item.exception_kind);
    push_optional_json_string(&mut fields, "family", item.family);
    push_optional_json_string(&mut fields, "owner", item.owner);
    push_optional_json_string(&mut fields, "classification", item.classification);
    push_optional_json_string(&mut fields, "reason", item.reason);
    push_optional_json_string(&mut fields, "created", item.created);
    push_optional_json_string(&mut fields, "review_after", item.review_after);
    push_optional_json_string(&mut fields, "expires", item.expires);
    fields.push(format!(
        "      \"evidence_count\": {}",
        item.evidence_count
            .map(|count| count.to_string())
            .unwrap_or_else(|| "null".to_string())
    ));
    if let Some(selector_precision) = item.selector_precision {
        fields.push(format!(
            "      \"selector_precision\": {selector_precision}"
        ));
    }
    fields.extend([
        format!("      \"risk\": \"{}\"", json_escape(item.risk)),
        format!("      \"difficulty\": \"{}\"", json_escape(item.difficulty)),
        format!("      \"status\": \"{}\"", json_escape(item.status)),
        format!("      \"allow_id\": {}", option_json(item.allow_id)),
        format!(
            "      \"candidate_ids\": {}",
            json_string_array(item.candidate_ids)
        ),
        format!(
            "      \"finding_index\": {}",
            item.finding_index
                .map(|index| index.to_string())
                .unwrap_or_else(|| "null".to_string())
        ),
        format!("      \"path\": {}", option_json(item.path)),
    ]);
    if let Some(line) = item.line {
        fields.push(format!("      \"line\": {line}"));
    }
    if let Some(column) = item.column {
        fields.push(format!("      \"column\": {column}"));
    }
    if let Some(reference) = item.evidence_reference.as_ref() {
        fields.push(format!(
            "      \"evidence_reference\": {}",
            render_worklist_evidence_reference_json(reference)
        ));
    }
    push_optional_json_string(&mut fields, "source_package", item.source_package);
    fields.extend([
        format!("      \"message\": \"{}\"", json_escape(item.message)),
        format!(
            "      \"suggested_actions\": {}",
            json_string_array(item.suggested_actions)
        ),
        format!(
            "      \"proof_commands\": {}",
            json_string_array(item.proof_commands)
        ),
        format!("      \"ledger_id\": {}", option_json(item.ledger_id)),
        format!("      \"ledger_path\": {}", option_json(item.ledger_path)),
        format!("      \"lane\": {}", option_json(item.lane)),
        format!("      \"mode\": {}", option_json(item.mode)),
        format!("      \"role\": {}", option_json(item.role)),
    ]);
    format!("    {{\n{}\n    }}", fields.join(",\n"))
}

fn push_optional_json_string(fields: &mut Vec<String>, key: &str, value: Option<&str>) {
    if let Some(value) = value {
        fields.push(format!("      \"{key}\": \"{}\"", json_escape(value)));
    }
}

fn render_worklist_evidence_reference_json(reference: &crate::EvidenceReference<'_>) -> String {
    format!(
        "{{\n        \"raw\": \"{}\",\n        \"prefix\": {},\n        \"target\": {},\n        \"status\": \"{}\",\n        \"category\": \"{}\",\n        \"message\": \"{}\"\n      }}",
        json_escape(reference.raw),
        option_json(reference.prefix),
        option_json(reference.target),
        json_escape(reference.status),
        json_escape(reference.category),
        json_escape(reference.message)
    )
}

fn render_worklist_filters_json(filters: WorklistFilters<'_>, indent: &str) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "{indent}  \"kind\": {},\n",
        option_json(filters.kind)
    ));
    out.push_str(&format!(
        "{indent}  \"family\": {},\n",
        option_json(filters.family)
    ));
    out.push_str(&format!(
        "{indent}  \"item_kind\": {},\n",
        option_json(filters.item_kind)
    ));
    out.push_str(&format!(
        "{indent}  \"status\": {},\n",
        option_json(filters.status)
    ));
    out.push_str(&format!(
        "{indent}  \"allow_id\": {},\n",
        option_json(filters.allow_id)
    ));
    out.push_str(&format!(
        "{indent}  \"path\": {},\n",
        option_json(filters.path)
    ));
    out.push_str(&format!(
        "{indent}  \"source_package\": {},\n",
        option_json(filters.source_package)
    ));
    out.push_str(&format!(
        "{indent}  \"owner\": {},\n",
        option_json(filters.owner)
    ));
    out.push_str(&format!(
        "{indent}  \"classification\": {},\n",
        option_json(filters.classification)
    ));
    out.push_str(&format!(
        "{indent}  \"baseline_debt\": {},\n",
        bool_json(filters.baseline_debt)
    ));
    out.push_str(&format!(
        "{indent}  \"broad_scope\": {},\n",
        bool_json(filters.broad_scope)
    ));
    out.push_str(&format!(
        "{indent}  \"risk\": {},\n",
        option_json(filters.risk)
    ));
    out.push_str(&format!(
        "{indent}  \"difficulty\": {},\n",
        option_json(filters.difficulty)
    ));
    out.push_str(&format!(
        "{indent}  \"missing_evidence\": {},\n",
        bool_json(filters.missing_evidence)
    ));
    out.push_str(&format!(
        "{indent}  \"broken_evidence\": {},\n",
        bool_json(filters.broken_evidence)
    ));
    out.push_str(&format!(
        "{indent}  \"weak_evidence\": {}\n",
        bool_json(filters.weak_evidence)
    ));
    out.push_str(&format!("{indent}}}"));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn work_item<'a>(
        suggested_actions: &'a [String],
        proof_commands: &'a [String],
    ) -> WorklistItem<'a> {
        WorklistItem {
            id: "work-0001",
            kind: "missing_evidence",
            exception_kind: Some("unsafe"),
            family: Some("unsafe_block"),
            owner: Some("runtime"),
            classification: Some("reviewed"),
            reason: Some("fixture reason"),
            created: Some("2026-06-01"),
            review_after: Some("2026-07-01"),
            expires: Some("2026-08-01"),
            evidence_count: Some(2),
            selector_precision: Some(9),
            risk: "high",
            difficulty: "small",
            status: "evidence_missing",
            allow_id: Some("allow-0001"),
            candidate_ids: &[],
            finding_index: Some(3),
            path: Some("src/lib.rs"),
            line: None,
            column: None,
            evidence_reference: Some(crate::EvidenceReference {
                raw: "test:unsafe_fixture",
                prefix: Some("test"),
                target: Some("unsafe_fixture"),
                status: "present",
                category: "typed",
                message: "evidence exists",
            }),
            source_package: Some("allow-report"),
            message: "missing evidence",
            suggested_actions,
            proof_commands,
            ledger_id: None,
            ledger_path: None,
            lane: None,
            mode: None,
            role: None,
        }
    }

    #[test]
    fn work_item_fixture_sets_expected_optional_fields() {
        let suggested_actions = vec!["add typed evidence".to_string()];
        let proof_commands = vec!["cargo-allow explain allow-0001".to_string()];
        let item = work_item(&suggested_actions, &proof_commands);

        assert_eq!(item.classification, Some("reviewed"));
        assert_eq!(item.reason, Some("fixture reason"));
        assert_eq!(item.evidence_count, Some(2));
        assert_eq!(item.selector_precision, Some(9));
        assert_eq!(item.finding_index, Some(3));
        let reference = item.evidence_reference.as_ref();
        assert_eq!(
            reference.map(|reference| reference.prefix),
            Some(Some("test"))
        );
        assert_eq!(
            reference.map(|reference| reference.target),
            Some(Some("unsafe_fixture"))
        );
        assert_eq!(reference.map(|reference| reference.category), Some("typed"));
    }

    #[test]
    fn render_work_item_json_includes_all_optional_fields_and_arrays() {
        let suggested_actions = vec![
            "add typed evidence".to_string(),
            "narrow the selector".to_string(),
        ];
        let proof_commands = vec![
            "cargo-allow explain allow-0001".to_string(),
            "cargo-allow check --mode no-new".to_string(),
        ];
        let json = render_work_item_json(&work_item(&suggested_actions, &proof_commands));

        for expected in [
            "\"id\": \"work-0001\"",
            "\"kind\": \"missing_evidence\"",
            "\"exception_kind\": \"unsafe\"",
            "\"family\": \"unsafe_block\"",
            "\"owner\": \"runtime\"",
            "\"classification\": \"reviewed\"",
            "\"reason\": \"fixture reason\"",
            "\"created\": \"2026-06-01\"",
            "\"review_after\": \"2026-07-01\"",
            "\"expires\": \"2026-08-01\"",
            "\"evidence_count\": 2",
            "\"selector_precision\": 9",
            "\"risk\": \"high\"",
            "\"difficulty\": \"small\"",
            "\"status\": \"evidence_missing\"",
            "\"allow_id\": \"allow-0001\"",
            "\"finding_index\": 3",
            "\"path\": \"src/lib.rs\"",
            "\"evidence_reference\": {",
            "\"raw\": \"test:unsafe_fixture\"",
            "\"prefix\": \"test\"",
            "\"target\": \"unsafe_fixture\"",
            "\"status\": \"present\"",
            "\"category\": \"typed\"",
            "\"message\": \"evidence exists\"",
            "\"source_package\": \"allow-report\"",
            "\"message\": \"missing evidence\"",
            "\"suggested_actions\": [\"add typed evidence\", \"narrow the selector\"]",
            "\"proof_commands\": [\"cargo-allow explain allow-0001\", \"cargo-allow check --mode no-new\"]",
        ] {
            assert!(json.contains(expected), "{expected}");
        }
    }

    #[test]
    fn render_work_item_json_omits_absent_policy_metadata_and_keeps_relationship_nulls() {
        let suggested_actions = Vec::new();
        let proof_commands = Vec::new();
        let item = WorklistItem {
            exception_kind: None,
            family: None,
            owner: None,
            classification: None,
            reason: None,
            created: None,
            review_after: None,
            expires: None,
            evidence_count: None,
            selector_precision: None,
            allow_id: None,
            candidate_ids: &[],
            finding_index: None,
            path: None,
            line: None,
            column: None,
            evidence_reference: None,
            source_package: None,
            suggested_actions: &suggested_actions,
            proof_commands: &proof_commands,
            ..work_item(&suggested_actions, &proof_commands)
        };

        let json = render_work_item_json(&item);

        for expected in [
            "\"evidence_count\": null",
            "\"allow_id\": null",
            "\"finding_index\": null",
            "\"path\": null",
            "\"suggested_actions\": []",
            "\"proof_commands\": []",
        ] {
            assert!(json.contains(expected), "{expected}");
        }
        for omitted in [
            "exception_kind",
            "family",
            "owner",
            "classification",
            "reason",
            "created",
            "review_after",
            "expires",
            "source_package",
        ] {
            assert!(!json.contains(&format!("\"{omitted}\":")), "{omitted}");
        }
        assert!(!json.contains("\"selector_precision\""));
        assert!(!json.contains("\"evidence_reference\""));
    }

    #[test]
    fn render_worklist_filters_json_records_every_filter_field() {
        let json = render_worklist_filters_json(
            WorklistFilters {
                kind: Some("unsafe"),
                family: Some("unsafe_block"),
                item_kind: Some("missing_evidence"),
                status: Some("evidence_missing"),
                allow_id: Some("allow-0001"),
                path: Some("src/lib.rs"),
                source_package: Some("allow-report"),
                owner: Some("runtime"),
                classification: Some("reviewed"),
                baseline_debt: true,
                broad_scope: true,
                risk: Some("high"),
                difficulty: Some("small"),
                missing_evidence: true,
                broken_evidence: true,
                weak_evidence: true,
            },
            "    ",
        );

        for expected in [
            "\"kind\": \"unsafe\"",
            "\"family\": \"unsafe_block\"",
            "\"item_kind\": \"missing_evidence\"",
            "\"status\": \"evidence_missing\"",
            "\"allow_id\": \"allow-0001\"",
            "\"path\": \"src/lib.rs\"",
            "\"source_package\": \"allow-report\"",
            "\"owner\": \"runtime\"",
            "\"classification\": \"reviewed\"",
            "\"baseline_debt\": true",
            "\"broad_scope\": true",
            "\"risk\": \"high\"",
            "\"difficulty\": \"small\"",
            "\"missing_evidence\": true",
            "\"broken_evidence\": true",
            "\"weak_evidence\": true",
        ] {
            assert!(json.contains(expected), "{expected}");
        }
    }
}
