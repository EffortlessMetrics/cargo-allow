use crate::artifact_contract_support::parse_json_artifact;
use crate::list;
use allow_report::{self, LIST_SCHEMA_ID};
use serde_json::Value;

#[test]
fn expected_inventory_scanner_call_presence_observer() {
    let json = list::sample_list_json_for_contract_test();
    let value = parse_json_artifact("list", &json, LIST_SCHEMA_ID, "list");
    let contract = allow_report::artifact_contract_for_schema_id(LIST_SCHEMA_ID)
        .unwrap_or_else(|| std::panic::panic_any("list schema should have a contract"));

    assert_eq!(
        value.pointer("/inventory/scanner").and_then(Value::as_str),
        Some(contract.inventory_scanner)
    );
}

#[test]
fn assert_json_array_eq_call_presence_observer() {
    let json = list::sample_list_json_for_contract_test();
    let value = parse_json_artifact("list", &json, LIST_SCHEMA_ID, "list");

    let claim_boundary = value
        .get("claim_boundary")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).collect::<Vec<_>>());
    assert_eq!(
        claim_boundary.as_deref(),
        Some(allow_report::claim_boundary_for_schema_id(LIST_SCHEMA_ID))
    );

    let scanner_limitations = value
        .get("scanner_limitations")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).collect::<Vec<_>>());
    assert_eq!(
        scanner_limitations.as_deref(),
        Some(allow_report::scanner_limitations_for_schema_id(
            LIST_SCHEMA_ID
        ))
    );
}
