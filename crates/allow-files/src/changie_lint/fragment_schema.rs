//! Config-derived fragment JSON Schema projection (#3613 PR D).
//!
//! `fragment_json_schema` projects the already-compiled
//! `ChangieCompiledFragmentContractV1` into a JSON Schema Draft 2020-12
//! document plus schema-association metadata. The compiled contract is
//! the only semantic input — this module contains no second rule
//! evaluator. Where JSON Schema cannot express a constraint (numeric
//! bounds on string-valued integers, runtime/template semantics), the
//! schema annotates honestly with namespaced `x-changie-*` metadata and
//! the Rust sensor stays authoritative.

use crate::changie::ChangieContentIdentity;
use crate::changie_lint::compiled_contract::{
    ChangieCompiledFragmentContractV1, ChoiceScope, ChoiceType,
};
use crate::changie_lint::sensor::ChangieSensor;

/// Namespaced annotation prefix binding schema fields to sensor semantics.
pub const CHANGIE_ANNOTATION_PREFIX: &str = "x-changie";

/// The emitted schema document, deterministic under equal contracts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangieFragmentSchemaV1 {
    pub schema_text: String,
    pub schema_digest: ChangieContentIdentity,
    /// Changie.* sensor rule IDs the schema can express; rules that stay
    /// Rust-only are absent by design and remain in the sensor.
    pub expressible_rule_ids: Vec<&'static str>,
}

/// Versioned editor-association descriptor (#3613). No absolute
/// checkout paths; the config identity and digest make config changes
/// stale the association.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangieFragmentSchemaAssociationV1 {
    pub compatibility_generation: &'static str,
    pub config_path: String,
    pub config_content_identity: ChangieContentIdentity,
    pub schema_id: String,
    pub schema_digest: ChangieContentIdentity,
    pub fragment_path_patterns: Vec<String>,
    pub source_subject: &'static str,
    pub completeness: &'static str,
    pub limitations: Vec<String>,
}

/// Project the compiled contract into the fragment schema. Kind-specific
/// branches use oneOf in compiled kind order with stable generated IDs.
pub fn fragment_json_schema(
    compiled: &ChangieCompiledFragmentContractV1,
) -> ChangieFragmentSchemaV1 {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str("  \"$schema\": \"https://json-schema.org/draft/2020-12/schema\",\n");
    out.push_str(&format!(
        "  \"$id\": \"cargo-allow.changie-fragment.v1;contract={}\",\n",
        compiled.digest
    ));
    out.push_str(&format!(
        "  \"title\": \"Changie fragment authoring contract (contract {})\",\n",
        compiled.digest
    ));
    out.push_str(&format!(
        "  \"{CHANGIE_ANNOTATION_PREFIX}-compatibility-generation\": \"{}\",\n",
        compiled.generation
    ));
    out.push_str(&format!(
        "  \"{CHANGIE_ANNOTATION_PREFIX}-contract-digest\": \"{}\",\n",
        compiled.digest
    ));
    // Honest completeness binding: the schema expresses the static
    // authoring contract only (limitation 7 negative control).
    out.push_str(&format!(
        "  \"{CHANGIE_ANNOTATION_PREFIX}-claim-boundary\": \"static authoring contract only; the Rust sensor remains authoritative; no render, batch, merge, or template proof\",\n"
    ));
    for limitation in &compiled.opaque_limitations {
        out.push_str(&format!(
            "  \"{CHANGIE_ANNOTATION_PREFIX}-limitation\": \"{limitation}\",\n"
        ));
    }

    out.push_str("  \"type\": \"object\",\n");
    out.push_str("  \"additionalProperties\": false,\n");
    out.push_str("  \"properties\": {\n");
    out.push_str("    \"kind\": {\n");
    out.push_str("      \"type\": \"string\",\n");
    if !compiled.kinds.is_empty() {
        out.push_str(&format!(
            "      \"enum\": [{}],\n",
            compiled
                .kinds
                .iter()
                .map(|kind| format!("\"{}\"", kind.label))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    out.push_str(&format!(
        "      \"{CHANGIE_ANNOTATION_PREFIX}-rule\": \"changie.fragment.kind_unknown\"\n"
    ));
    out.push_str("    },\n");

    if !compiled.projects.is_empty() {
        out.push_str("    \"project\": {\n");
        out.push_str("      \"type\": \"string\",\n");
        // Canonical persisted identity is the project key, never the
        // display label (negative control 4).
        out.push_str(&format!(
            "      \"enum\": [{}],\n",
            compiled
                .projects
                .iter()
                .map(|project| format!("\"{}\"", project.key))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        out.push_str(&format!(
            "      \"{CHANGIE_ANNOTATION_PREFIX}-identity\": \"canonical-key\",\n"
        ));
        out.push_str(&format!(
            "      \"{CHANGIE_ANNOTATION_PREFIX}-rule\": \"changie.fragment.project_not_canonical\"\n"
        ));
        out.push_str("    },\n");
    }

    if !compiled.components.is_empty() {
        out.push_str("    \"component\": {\n");
        out.push_str("      \"type\": \"string\",\n");
        out.push_str(&format!(
            "      \"enum\": [{}],\n",
            compiled
                .components
                .iter()
                .map(|component| format!("\"{component}\""))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        out.push_str(&format!(
            "      \"{CHANGIE_ANNOTATION_PREFIX}-rule\": \"changie.fragment.component_unknown\"\n"
        ));
        out.push_str("    },\n");
    }

    // Body: min/max length in UTF-8 bytes, matching the compiled
    // contract's pinned semantics.
    out.push_str("    \"body\": {\n      \"type\": \"string\",\n");
    if let Some(min) = compiled.body.min_length {
        out.push_str(&format!("      \"minLength\": {min},\n"));
    }
    if let Some(max) = compiled.body.max_length {
        out.push_str(&format!("      \"maxLength\": {max},\n"));
    }
    out.push_str(&format!(
        "      \"{CHANGIE_ANNOTATION_PREFIX}-length-semantics\": \"{}\",\n",
        crate::changie_lint::fragment_rules::BODY_LENGTH_SEMANTICS
    ));
    out.push_str(&format!(
        "      \"{CHANGIE_ANNOTATION_PREFIX}-rule\": \"changie.fragment.body_too_short\"\n"
    ));
    out.push_str("    },\n");

    if compiled.time_format.is_some() {
        out.push_str("    \"time\": {\n");
        out.push_str("      \"type\": \"string\",\n");
        out.push_str("      \"format\": \"date-time\",\n");
        out.push_str(&format!(
            "      \"{CHANGIE_ANNOTATION_PREFIX}-rule\": \"changie.fragment.time_invalid\"\n"
        ));
        out.push_str("    },\n");
    }

    // Custom mapping: global + kind-scoped choices are merged into the
    // custom properties; requiredness per kind is handled in the kind
    // branches below via allOf overrides.
    let mut custom_properties: Vec<String> = Vec::new();
    let mut global_custom_required: Vec<String> = Vec::new();
    for choice in &compiled.choices {
        let mut property = format!("      \"{}\": {{\n", choice.key);
        match choice.choice_type {
            ChoiceType::String | ChoiceType::Block => {
                property.push_str("        \"type\": \"string\",\n");
                if let Some(min) = choice.min_length {
                    property.push_str(&format!("        \"minLength\": {min},\n"));
                }
                if let Some(max) = choice.max_length {
                    property.push_str(&format!("        \"maxLength\": {max},\n"));
                }
            }
            ChoiceType::Int => {
                // Persisted int values are strings. Express lexical shape;
                // do NOT claim numeric min/max enforcement JSON Schema
                // cannot deliver on strings (negative control 5).
                property.push_str("        \"type\": \"string\",\n");
                property.push_str("        \"pattern\": \"^-?[0-9]+$\",\n");
                property.push_str(&format!(
                    "        \"{CHANGIE_ANNOTATION_PREFIX}-type\": \"int\",\n"
                ));
                if let Some(min) = choice.min_int {
                    property.push_str(&format!(
                        "        \"{CHANGIE_ANNOTATION_PREFIX}-min-int\": {min},\n"
                    ));
                }
                if let Some(max) = choice.max_int {
                    property.push_str(&format!(
                        "        \"{CHANGIE_ANNOTATION_PREFIX}-max-int\": {max},\n"
                    ));
                }
                property.push_str(&format!(
                    "        \"{CHANGIE_ANNOTATION_PREFIX}-note\": \"numeric bounds are annotated, not JSON-Schema-enforced; the Rust sensor validates them\",\n"
                ));
            }
            ChoiceType::Enum => {
                property.push_str("        \"type\": \"string\",\n");
                if !choice.enum_options.is_empty() {
                    property.push_str(&format!(
                        "        \"enum\": [{}],\n",
                        choice
                            .enum_options
                            .iter()
                            .map(|option| format!("\"{option}\""))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
            }
        }
        property.push_str("      }");
        custom_properties.push(property);
        if !choice.optional && matches!(choice.scope, ChoiceScope::Global) {
            global_custom_required.push(choice.key.clone());
        }
    }
    if !custom_properties.is_empty() {
        out.push_str("    \"custom\": {\n      \"type\": \"object\",\n");
        // Post-generated keys are not user authoring input; the sensor
        // reports them as unconfigured rather than forbidding them here
        // (negative control 6). additionalProperties stays false for the
        // active configured vocabulary; generated keys are annotated.
        out.push_str("      \"additionalProperties\": false,\n");
        out.push_str("      \"properties\": {\n");
        out.push_str(&custom_properties.join(",\n"));
        out.push_str("\n      },\n");
        if !global_custom_required.is_empty() {
            out.push_str(&format!(
                "      \"required\": [{}],\n",
                global_custom_required
                    .iter()
                    .map(|key| format!("\"{key}\""))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        out.push_str(&format!(
            "      \"{CHANGIE_ANNOTATION_PREFIX}-rule\": \"changie.fragment.custom_missing\"\n"
        ));
        out.push_str("    }\n");
    } else {
        out.push_str("    \"custom\": {\n      \"type\": \"object\",\n");
        out.push_str("      \"additionalProperties\": false\n");
        out.push_str("    }\n");
    }
    out.push_str("  }");

    // Base required fields.
    let mut required: Vec<&str> = Vec::new();
    if !compiled.kinds.is_empty() {
        required.push("kind");
    }
    if !compiled.projects.is_empty() {
        required.push("project");
    }
    if !compiled.components.is_empty() {
        required.push("component");
    }
    required.push("body");
    if compiled.time_format.is_some() {
        required.push("time");
    }
    if !required.is_empty() {
        out.push_str(",\n  \"required\": [");
        out.push_str(
            &required
                .iter()
                .map(|field| format!("\"{field}\""))
                .collect::<Vec<_>>()
                .join(", "),
        );
        out.push(']');
    }

    // Kind-specific branches: skipBody, skipGlobalChoices,
    // additionalChoices, in compiled kind order with stable IDs.
    if !compiled.kinds.is_empty() {
        out.push_str(",\n  \"allOf\": [\n");
        let branches: Vec<String> = compiled
            .kinds
            .iter()
            .map(|kind| kind_branch(compiled, &kind.label.clone()))
            .collect();
        out.push_str(&branches.join(",\n"));
        out.push_str("\n  ]");
    }

    out.push_str("\n}\n");

    let schema_digest = ChangieContentIdentity::of(out.as_bytes());
    ChangieFragmentSchemaV1 {
        schema_text: out,
        schema_digest,
        expressible_rule_ids: expressible_rules(compiled),
    }
}

fn kind_branch(compiled: &ChangieCompiledFragmentContractV1, label: &str) -> String {
    let kind = compiled
        .kinds
        .iter()
        .find(|kind| kind.label == label)
        .unwrap_or_else(|| std::panic::panic_any("compiled kind missing"));
    let mut branch = String::new();
    branch.push_str("    {\n");
    branch.push_str(&format!(
        "      \"if\": {{\"properties\": {{\"kind\": {{\"const\": \"{label}\"}}}}, \"required\": [\"kind\"]}},\n"
    ));
    branch.push_str("      \"then\": {\n");
    branch.push_str(&format!(
        "        \"{CHANGIE_ANNOTATION_PREFIX}-kind-branch\": \"{label}\",\n"
    ));
    if kind.skip_body {
        // body not required for this kind
        branch.push_str(&format!(
            "        \"{CHANGIE_ANNOTATION_PREFIX}-body-required\": false,\n"
        ));
    }
    // Active custom requiredness for this kind: global unless skipped,
    // plus this kind's additional choices.
    let mut required_custom: Vec<String> = Vec::new();
    for choice in &compiled.choices {
        let active = match &choice.scope {
            ChoiceScope::Global => !kind.skip_global_choices,
            ChoiceScope::KindSpecific { kind_label } => kind_label == label,
        };
        if active && !choice.optional {
            required_custom.push(choice.key.clone());
        }
    }
    if !required_custom.is_empty() {
        branch.push_str(&format!(
            "        \"properties\": {{\"custom\": {{\"required\": [{}]}}}}",
            required_custom
                .iter()
                .map(|key| format!("\"{key}\""))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    } else {
        branch.push_str("        \"properties\": {}");
    }
    branch.push_str("\n      }\n    }");
    branch
}

fn expressible_rules(compiled: &ChangieCompiledFragmentContractV1) -> Vec<&'static str> {
    let mut rules = vec![
        "changie.fragment.kind_unknown",
        "changie.fragment.body_too_short",
        "changie.fragment.body_too_long",
        "changie.fragment.custom_missing",
        "changie.fragment.custom_unknown_value",
        "changie.fragment.custom_out_of_range",
    ];
    if !compiled.projects.is_empty() {
        rules.push("changie.fragment.project_not_canonical");
        rules.push("changie.fragment.project_unknown");
    }
    if !compiled.components.is_empty() {
        rules.push("changie.fragment.component_unknown");
    }
    if compiled.time_format.is_some() {
        rules.push("changie.fragment.time_invalid");
    }
    rules
}

/// Association metadata for schema-aware editors. The fragment patterns
/// derive from the config's own discovery semantics: direct-child
/// `.yaml` under `<changesDir>/<unreleasedDir>`.
pub fn fragment_schema_association(
    compiled: &ChangieCompiledFragmentContractV1,
    schema: &ChangieFragmentSchemaV1,
    config_path: &str,
    root: &str,
) -> ChangieFragmentSchemaAssociationV1 {
    let pattern = format!("{root}/*.yaml");
    ChangieFragmentSchemaAssociationV1 {
        compatibility_generation: compiled.generation,
        config_path: config_path.to_string(),
        config_content_identity: compiled.config_identity,
        schema_id: format!(
            "cargo-allow.changie-fragment.v1;contract={}",
            compiled.digest
        ),
        schema_digest: schema.schema_digest,
        fragment_path_patterns: vec![pattern],
        source_subject: "saved-worktree",
        completeness: if compiled.opaque_limitations.is_empty() {
            "complete"
        } else {
            "not-proven"
        },
        limitations: compiled
            .opaque_limitations
            .iter()
            .map(|limitation| limitation.to_string())
            .collect(),
    }
}

/// The sensor's facade accessor for the schema projection.
impl ChangieSensor {
    /// Project the compiled contract into the fragment JSON Schema.
    pub fn fragment_schema(
        &self,
        compiled: &ChangieCompiledFragmentContractV1,
    ) -> ChangieFragmentSchemaV1 {
        fragment_json_schema(compiled)
    }
}

#[cfg(test)]
#[path = "fragment_schema_tests.rs"]
mod fragment_schema_tests;
