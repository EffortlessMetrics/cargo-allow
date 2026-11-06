use crate::{
    ExpectedReceiptContractV1, STATE_MACHINE_EVIDENCE_BINDING_SCHEMA_ID,
    StateMachineEvidenceBindingV1, StateMachineEvidenceKindV1,
};
use effortless_repo_protocol::stable_digest_hex;
use serde_json::Value;

fn binding() -> StateMachineEvidenceBindingV1 {
    StateMachineEvidenceBindingV1 {
        schema_id: STATE_MACHINE_EVIDENCE_BINDING_SCHEMA_ID.to_string(),
        machine_id: "fixture.repair-attempt".to_string(),
        property_id: "finish-requires-current-snapshot".to_string(),
        evidence_kind: StateMachineEvidenceKindV1::ImplementationConformance,
        model_digest: stable_digest_hex(b"synthetic model fixture"),
        property_digest: stable_digest_hex(b"synthetic property fixture"),
        assumptions_digest: stable_digest_hex(b"synthetic assumptions fixture"),
        scope_digest: stable_digest_hex(b"synthetic scope: two states, one step"),
        toolchain_digest: stable_digest_hex(b"synthetic toolchain; nothing executed"),
        implementation_digest: Some(stable_digest_hex(b"synthetic implementation fixture")),
        adapter_digest: Some(stable_digest_hex(b"synthetic adapter fixture")),
    }
}

#[test]
fn state_machine_documented_fixture_and_schema_follow_the_serde_contract() -> Result<(), String> {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/proof-protocol/state-machine-evidence-binding-v1.json"
    ))
    .map_err(|error| error.to_string())?;
    let schema: Value = serde_json::from_str(include_str!(
        "../../../docs/schemas/proof-state-machine-evidence-binding-v1.schema.json"
    ))
    .map_err(|error| error.to_string())?;
    if fixture != serde_json::to_value(binding()).map_err(|error| error.to_string())? {
        return Err("documented synthetic fixture differs from the typed example".to_string());
    }
    let parsed: StateMachineEvidenceBindingV1 =
        serde_json::from_value(fixture.clone()).map_err(|error| error.to_string())?;
    parsed.validate()?;
    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .ok_or_else(|| "binding schema properties missing".to_string())?;
    let fields = fixture
        .as_object()
        .ok_or_else(|| "fixture object missing".to_string())?;
    if properties.keys().ne(fields.keys())
        || schema.get("additionalProperties").and_then(Value::as_bool) != Some(false)
    {
        return Err("schema field set does not match the closed serde object".to_string());
    }
    for field in schema
        .get("required")
        .and_then(Value::as_array)
        .ok_or_else(|| "binding schema required fields missing".to_string())?
    {
        let name = field
            .as_str()
            .ok_or_else(|| "schema field name is not a string".to_string())?;
        let mut missing = fixture.clone();
        missing
            .as_object_mut()
            .ok_or_else(|| "fixture object missing".to_string())?
            .remove(name);
        if serde_json::from_value::<StateMachineEvidenceBindingV1>(missing).is_ok() {
            return Err(format!("schema-required field {name} is not required by serde"));
        }
    }
    Ok(())
}

#[test]
fn state_machine_evidence_kinds_roundtrip_without_collapsing_claims() -> Result<(), String> {
    let mut encodings = std::collections::BTreeSet::new();
    for kind in [
        StateMachineEvidenceKindV1::ModelCheck,
        StateMachineEvidenceKindV1::ModelProof,
        StateMachineEvidenceKindV1::ImplementationConformance,
        StateMachineEvidenceKindV1::ImplementationProof,
        StateMachineEvidenceKindV1::ScheduleExploration,
    ] {
        let mut candidate = binding();
        candidate.evidence_kind = kind;
        if !kind.binds_implementation() {
            candidate.implementation_digest = None;
            candidate.adapter_digest = None;
        }
        candidate.validate()?;
        let encoded = serde_json::to_string(&candidate).map_err(|error| error.to_string())?;
        let decoded: StateMachineEvidenceBindingV1 =
            serde_json::from_str(&encoded).map_err(|error| error.to_string())?;
        if decoded != candidate || !encodings.insert(encoded) {
            return Err("distinct evidence kinds did not retain their identity".to_string());
        }
    }
    Ok(())
}

#[test]
fn state_machine_required_identity_fields_cannot_disappear() -> Result<(), String> {
    let original = serde_json::to_value(binding()).map_err(|error| error.to_string())?;
    for field in [
        "schema_id",
        "machine_id",
        "property_id",
        "evidence_kind",
        "model_digest",
        "property_digest",
        "assumptions_digest",
        "scope_digest",
        "toolchain_digest",
    ] {
        let mut candidate = original.clone();
        candidate
            .as_object_mut()
            .ok_or_else(|| "fixture is not an object".to_string())?
            .remove(field);
        if serde_json::from_value::<StateMachineEvidenceBindingV1>(candidate).is_ok() {
            return Err(format!("missing {field} was defaulted into a valid binding"));
        }
    }
    Ok(())
}

#[test]
fn state_machine_digests_are_strict_and_field_specific() -> Result<(), String> {
    let original = serde_json::to_value(binding()).map_err(|error| error.to_string())?;
    for field in [
        "model_digest",
        "property_digest",
        "assumptions_digest",
        "scope_digest",
        "toolchain_digest",
        "implementation_digest",
        "adapter_digest",
    ] {
        for invalid in [
            String::new(),
            "current".to_string(),
            "sha256:v1:short".to_string(),
            format!("sha256:v1:{}", "A".repeat(64)),
            format!("sha256:v1:{} ", "a".repeat(64)),
        ] {
            let mut candidate = original.clone();
            *candidate
                .get_mut(field)
                .ok_or_else(|| format!("fixture {field} is missing"))? = Value::String(invalid);
            let parsed: StateMachineEvidenceBindingV1 =
                serde_json::from_value(candidate).map_err(|error| error.to_string())?;
            match parsed.validate() {
                Err(reason) if reason.contains(field) => {}
                Err(reason) => return Err(format!("wrong failure for {field}: {reason}")),
                Ok(()) => return Err(format!("invalid {field} was accepted")),
            }
        }
    }
    Ok(())
}

#[test]
fn state_machine_model_evidence_does_not_claim_an_implementation() -> Result<(), String> {
    for kind in [
        StateMachineEvidenceKindV1::ModelCheck,
        StateMachineEvidenceKindV1::ModelProof,
    ] {
        let mut candidate = binding();
        candidate.evidence_kind = kind;
        if candidate.validate().is_ok() {
            return Err("model-only evidence accepted a production binding".to_string());
        }
        candidate.implementation_digest = None;
        if candidate.validate().is_ok() {
            return Err("model-only evidence accepted an orphan adapter".to_string());
        }
        candidate.adapter_digest = None;
        candidate.validate()?;
    }
    Ok(())
}

#[test]
fn state_machine_implementation_evidence_requires_source_and_adapter() -> Result<(), String> {
    for kind in [
        StateMachineEvidenceKindV1::ImplementationConformance,
        StateMachineEvidenceKindV1::ImplementationProof,
        StateMachineEvidenceKindV1::ScheduleExploration,
    ] {
        for missing in ["implementation", "adapter", "both"] {
            let mut candidate = binding();
            candidate.evidence_kind = kind;
            if missing != "adapter" {
                candidate.implementation_digest = None;
            }
            if missing != "implementation" {
                candidate.adapter_digest = None;
            }
            if candidate.validate().is_ok() {
                return Err(format!("{kind:?} accepted missing {missing} identity"));
            }
        }
    }
    Ok(())
}

#[test]
fn state_machine_unknown_claims_and_blank_identifiers_fail_closed() -> Result<(), String> {
    let mut candidate = binding();
    candidate.schema_id = "proof.state-machine-evidence-binding.v2".to_string();
    if candidate.validate().is_ok() {
        return Err("unknown schema was accepted".to_string());
    }
    for field in ["machine_id", "property_id"] {
        for invalid in ["", " ", "contains a space", "../not a stable id"] {
            let mut value = serde_json::to_value(binding()).map_err(|error| error.to_string())?;
            *value
                .get_mut(field)
                .ok_or_else(|| format!("fixture {field} missing"))? =
                Value::String(invalid.to_string());
            let parsed: StateMachineEvidenceBindingV1 =
                serde_json::from_value(value).map_err(|error| error.to_string())?;
            if parsed.validate().is_ok() {
                return Err(format!("invalid {field} was accepted"));
            }
        }
    }
    let mut value = serde_json::to_value(binding()).map_err(|error| error.to_string())?;
    value
        .as_object_mut()
        .ok_or_else(|| "fixture object missing".to_string())?
        .insert("verified".to_string(), Value::Bool(true));
    if serde_json::from_value::<StateMachineEvidenceBindingV1>(value).is_ok() {
        return Err("uncontracted verified flag was accepted".to_string());
    }
    Ok(())
}

#[test]
fn state_machine_extension_is_absent_from_legacy_receipt_contract_json() -> Result<(), String> {
    let legacy = serde_json::json!({
        "receipt_schema": "repo.analysis-receipt.v1",
        "receipt_generation": 1,
        "config_identity": "config:test",
        "currentness_dimensions": ["snapshot", "subject", "provider_request", "config"]
    });
    let decoded: ExpectedReceiptContractV1 =
        serde_json::from_value(legacy.clone()).map_err(|error| error.to_string())?;
    if decoded.state_machine.is_some()
        || serde_json::to_value(decoded).map_err(|error| error.to_string())? != legacy
    {
        return Err("legacy contract serialization changed".to_string());
    }
    Ok(())
}
