//! Bounded domain queries returning intent-protocol-shaped responses (#2586-D).
//!
//! These queries expose a fixed catalog of read-only domain facts. They do not
//! compile graphs, execute proof commands, or return raw compiled graph IR.

use crate::graph_comparison::canonical_graph_movement_kinds;
use crate::phase_obligations::PhaseObligationKindV1;
use crate::workspace::{
    AUTHORITY_COMPILE_PLAN_SCHEMA_ID, SELF_HOSTED_RUNTIME_PROMOTION_COMPOSITION_ID,
    WorkspaceCompositionV1, plan_authority_compile,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const BOUNDED_DOMAIN_QUERY_SCHEMA_ID: &str = "intent.bounded-domain-query.v1";
pub const BOUNDED_DOMAIN_QUERY_RESPONSE_SCHEMA_ID: &str = "intent.bounded-domain-query-response.v1";
pub const RESULT_CLASS_COMPLETED: &str = "completed";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoundedDomainQueryKindV1 {
    WorkspaceComposition,
    AuthorityCompilePlan,
    MovementKindsCatalog,
    PhaseObligationKindsCatalog,
}

impl BoundedDomainQueryKindV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WorkspaceComposition => "workspace_composition",
            Self::AuthorityCompilePlan => "authority_compile_plan",
            Self::MovementKindsCatalog => "movement_kinds_catalog",
            Self::PhaseObligationKindsCatalog => "phase_obligation_kinds_catalog",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundedDomainQueryRequestV1 {
    pub schema_id: String,
    pub kind: BoundedDomainQueryKindV1,
    pub identity: intent_protocol::IntentIdentityEnvelopeV1,
    pub selector: String,
}

impl BoundedDomainQueryRequestV1 {
    pub fn new(
        identity: intent_protocol::IntentIdentityEnvelopeV1,
        kind: BoundedDomainQueryKindV1,
        selector: impl Into<String>,
    ) -> Self {
        Self {
            schema_id: BOUNDED_DOMAIN_QUERY_SCHEMA_ID.to_string(),
            kind,
            identity,
            selector: selector.into(),
        }
    }

    pub fn composition_id(&self) -> &str {
        &self.identity.artifact_id
    }

    pub fn with_selector(mut self, selector: impl Into<String>) -> Self {
        self.selector = selector.into();
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundedDomainQueryResponseV1 {
    pub schema_id: String,
    pub query: BoundedDomainQueryRequestV1,
    pub result_class: String,
    pub payload_schema: String,
    pub payload: Value,
}

impl BoundedDomainQueryResponseV1 {
    pub fn new(
        query: BoundedDomainQueryRequestV1,
        result_class: impl Into<String>,
        payload_schema: impl Into<String>,
        payload: Value,
    ) -> Self {
        Self {
            schema_id: BOUNDED_DOMAIN_QUERY_RESPONSE_SCHEMA_ID.to_string(),
            query,
            result_class: result_class.into(),
            payload_schema: payload_schema.into(),
            payload,
        }
    }
}

/// Execute one bounded domain query and return an intent-protocol-shaped response.
pub fn execute_bounded_domain_query(
    request: &BoundedDomainQueryRequestV1,
) -> BoundedDomainQueryResponseV1 {
    match request.kind {
        BoundedDomainQueryKindV1::WorkspaceComposition => workspace_composition_response(request),
        BoundedDomainQueryKindV1::AuthorityCompilePlan => authority_compile_plan_response(request),
        BoundedDomainQueryKindV1::MovementKindsCatalog => movement_kinds_catalog_response(request),
        BoundedDomainQueryKindV1::PhaseObligationKindsCatalog => {
            phase_obligation_kinds_catalog_response(request)
        }
    }
}

/// Project a bounded domain response into `intent.query-response.v1` JSON.
pub fn to_intent_query_response_json(response: &BoundedDomainQueryResponseV1) -> Value {
    json!({
        "schema_id": intent_protocol::INTENT_QUERY_RESPONSE_SCHEMA_ID,
        "query": {
            "schema_id": intent_protocol::INTENT_QUERY_SCHEMA_ID,
            "kind": "domain_query",
            "selector": response.query.selector,
            "identity": response.query.identity,
        },
        "result_class": response.result_class,
        "payload_schema": response.payload_schema,
        "payload": response.payload,
    })
}

pub fn canonical_bounded_domain_query_kinds() -> &'static [BoundedDomainQueryKindV1] {
    const KINDS: &[BoundedDomainQueryKindV1] = &[
        BoundedDomainQueryKindV1::WorkspaceComposition,
        BoundedDomainQueryKindV1::AuthorityCompilePlan,
        BoundedDomainQueryKindV1::MovementKindsCatalog,
        BoundedDomainQueryKindV1::PhaseObligationKindsCatalog,
    ];
    KINDS
}

fn workspace_composition_response(
    request: &BoundedDomainQueryRequestV1,
) -> BoundedDomainQueryResponseV1 {
    let composition = resolve_composition(request);
    let payload = match serde_json::to_value(&composition) {
        Ok(value) => value,
        Err(err) => json!({ "error": format!("serialize workspace composition: {err}") }),
    };
    BoundedDomainQueryResponseV1::new(
        request.clone(),
        RESULT_CLASS_COMPLETED,
        "intent.workspace-composition.v1",
        payload,
    )
}

fn authority_compile_plan_response(
    request: &BoundedDomainQueryRequestV1,
) -> BoundedDomainQueryResponseV1 {
    let composition = resolve_composition(request);
    let plan = plan_authority_compile(&composition);
    let payload = match serde_json::to_value(&plan) {
        Ok(value) => value,
        Err(err) => json!({ "error": format!("serialize plan: {err}") }),
    };
    BoundedDomainQueryResponseV1::new(
        request.clone(),
        RESULT_CLASS_COMPLETED,
        AUTHORITY_COMPILE_PLAN_SCHEMA_ID,
        payload,
    )
}

fn movement_kinds_catalog_response(
    request: &BoundedDomainQueryRequestV1,
) -> BoundedDomainQueryResponseV1 {
    let kinds = canonical_graph_movement_kinds()
        .iter()
        .map(|kind| kind.as_str())
        .collect::<Vec<_>>();
    BoundedDomainQueryResponseV1::new(
        request.clone(),
        RESULT_CLASS_COMPLETED,
        "intent.graph-movement-kinds.v1",
        json!({ "movement_kinds": kinds }),
    )
}

fn phase_obligation_kinds_catalog_response(
    request: &BoundedDomainQueryRequestV1,
) -> BoundedDomainQueryResponseV1 {
    let kinds = [
        PhaseObligationKindV1::EvidenceReview,
        PhaseObligationKindV1::ImplementationClosure,
        PhaseObligationKindV1::SupportClaimReview,
        PhaseObligationKindV1::InventoryCompleteness,
        PhaseObligationKindV1::SubjectResolution,
        PhaseObligationKindV1::PolicyAlignment,
    ]
    .into_iter()
    .map(|kind| kind.as_str())
    .collect::<Vec<_>>();
    BoundedDomainQueryResponseV1::new(
        request.clone(),
        RESULT_CLASS_COMPLETED,
        "intent.phase-obligation-kinds.v1",
        json!({ "obligation_kinds": kinds }),
    )
}

fn resolve_composition(request: &BoundedDomainQueryRequestV1) -> WorkspaceCompositionV1 {
    match Some(request.composition_id()) {
        Some(id) if id != SELF_HOSTED_RUNTIME_PROMOTION_COMPOSITION_ID => WorkspaceCompositionV1 {
            composition_id: id.to_string(),
            requirement_path: String::new(),
            slice_path: String::new(),
            seams_path: String::new(),
            evidence_path: String::new(),
            subject_inventory: String::new(),
        },
        _ => WorkspaceCompositionV1::self_hosted_runtime_promotion(),
    }
}

pub fn load_bounded_domain_query_catalog_toml(text: &str) -> Result<Vec<String>, String> {
    let table: toml::Table =
        toml::from_str(text).map_err(|err| format!("parse bounded query catalog: {err}"))?;
    let Some(kinds) = table.get("query_kinds").and_then(|value| value.as_array()) else {
        return Err("bounded query catalog missing query_kinds".to_string());
    };
    kinds
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| "query_kinds entries must be strings".to_string())
        })
        .collect()
}
