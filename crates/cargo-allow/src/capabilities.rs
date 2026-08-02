use allow_core::{CargoAllowError, CargoAllowResult};
use clap::{Parser, ValueEnum};
use serde::Serialize;
use std::path::PathBuf;

use crate::emit_text;

pub(crate) const SENSOR_CAPABILITY_SCHEMA: &str = "cargo-allow.sensor-capabilities.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum CapabilityFormat {
    Human,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum CapabilityClass {
    #[value(name = "supported-syntax")]
    SupportedSyntax,
    #[value(name = "supported-presence")]
    SupportedPresence,
    #[value(name = "compatibility-adapter")]
    CompatibilityAdapter,
    #[value(name = "policy-derived")]
    PolicyDerived,
    #[value(name = "not-included")]
    NotIncluded,
}

impl CapabilityClass {
    fn as_str(self) -> &'static str {
        match self {
            Self::SupportedSyntax => "supported_syntax",
            Self::SupportedPresence => "supported_presence",
            Self::CompatibilityAdapter => "compatibility_adapter",
            Self::PolicyDerived => "policy_derived",
            Self::NotIncluded => "not_included",
        }
    }
}

#[derive(Debug, Clone, Parser)]
pub(crate) struct CapabilitiesArgs {
    /// Output format.
    #[arg(long, value_enum, default_value_t = CapabilityFormat::Human)]
    pub(crate) format: CapabilityFormat,
    /// Restrict output to one analysis class.
    #[arg(long, value_enum)]
    pub(crate) class: Option<CapabilityClass>,
    /// Restrict output to one FindingKind code.
    #[arg(long)]
    pub(crate) kind: Option<String>,
    /// Restrict output to one exact finding family.
    #[arg(long)]
    pub(crate) family: Option<String>,
    /// Write capability output to a file instead of stdout.
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
struct CapabilityCatalog {
    schema: &'static str,
    generation: u32,
    claim_boundary: &'static str,
    capabilities: Vec<SensorCapability>,
}

#[derive(Debug, Clone, Copy, Serialize)]
struct SensorCapability {
    sensor_id: &'static str,
    generation: u32,
    owner: &'static str,
    kind: Option<&'static str>,
    family: Option<&'static str>,
    input: &'static str,
    selection: &'static str,
    analysis_class: &'static str,
    precision: &'static str,
    completeness: &'static str,
    platform: &'static str,
    profile: &'static str,
    limitations: &'static [&'static str],
    claims_supported: &'static [&'static str],
    claims_excluded: &'static [&'static str],
    fixtures: &'static [&'static str],
    docs: &'static [&'static str],
    support_tier: &'static str,
}

const SOURCE_LIMITATIONS: &[&str] = &[
    "syntax_visible_only",
    "no_macro_expansion",
    "no_type_or_flow_analysis",
    "readable_source_required",
];
const SOURCE_CLAIMS: &[&str] = &["recognized syntax construct exists in source text"];
const SOURCE_EXCLUDED: &[&str] = &[
    "compile_success",
    "type_semantics",
    "macro_expansion",
    "runtime_reachability",
    "test_adequacy",
];
const SOURCE_FIXTURES: &[&str] = &["crates/allow-rust/src/line_findings.rs"];
const SOURCE_DOCS: &[&str] = &["docs/claim-boundaries.md", "docs/identity.md"];
const FILE_LIMITATIONS: &[&str] = &[
    "path_presence_only",
    "no_file_content_safety_claim",
    "inventory_completeness_is_separate",
];
const FILE_CLAIMS: &[&str] = &["governed path exists and matches this family classifier"];
const FILE_EXCLUDED: &[&str] = &[
    "file_content_safety",
    "shell_behavior",
    "runtime_reachability",
    "type_or_flow_analysis",
];
const FILE_FIXTURES: &[&str] = &["crates/allow-files/src/families.rs"];
const FILE_DOCS: &[&str] = &["docs/claim-boundaries.md", "docs/getting-started.md"];
const POLICY_LIMITATIONS: &[&str] = &[
    "derived_from_policy_or_source_tree",
    "compatibility_shape_is_preserved",
    "no_referenced_behavior_execution",
];
const POLICY_CLAIMS: &[&str] =
    &["selected policy or compatibility fact was projected into a canonical finding"];
const POLICY_EXCLUDED: &[&str] = &[
    "process_execution",
    "network_access",
    "workflow_execution",
    "dependency_resolution",
];
const POLICY_FIXTURES: &[&str] = &["crates/allow-diff/src/tests/revision_companions.rs"];
const POLICY_DOCS: &[&str] = &["docs/claim-boundaries.md", "docs/design.md"];
const NOT_INCLUDED_LIMITATIONS: &[&str] = &["no_sensor_implemented"];
const NOT_INCLUDED_CLAIMS: &[&str] = &["not_included_is_not_a_clean_result"];
const NOT_INCLUDED_EXCLUDED: &[&str] = &["any_positive_detection_claim"];
const NOT_INCLUDED_FIXTURES: &[&str] = &["crates/cargo-allow/src/capabilities.rs"];
const NOT_INCLUDED_DOCS: &[&str] = &["docs/claim-boundaries.md"];

const fn source(
    sensor_id: &'static str,
    kind: &'static str,
    family: &'static str,
) -> SensorCapability {
    SensorCapability {
        sensor_id,
        generation: 1,
        owner: "allow-rust",
        kind: Some(kind),
        family: Some(family),
        input: "Rust source text",
        selection: "tracked .rs files selected by source inventory",
        analysis_class: "supported_syntax",
        precision: "source line/column plus structural identity",
        completeness: "complete for recognized syntax on readable parseable files; parse limitations remain observable",
        platform: "source-text scanner; no target build required",
        profile: "default",
        limitations: SOURCE_LIMITATIONS,
        claims_supported: SOURCE_CLAIMS,
        claims_excluded: SOURCE_EXCLUDED,
        fixtures: SOURCE_FIXTURES,
        docs: SOURCE_DOCS,
        support_tier: "supported",
    }
}

const fn file(
    sensor_id: &'static str,
    kind: &'static str,
    family: &'static str,
) -> SensorCapability {
    SensorCapability {
        sensor_id,
        generation: 1,
        owner: "allow-files",
        kind: Some(kind),
        family: Some(family),
        input: "tracked source-tree path",
        selection: "tracked inventory or documented filesystem fallback",
        analysis_class: "supported_presence",
        precision: "relative path and classified family",
        completeness: "depends on the inventory completeness reported by the scan",
        platform: "source-tree inventory; platform-neutral path classification",
        profile: "default",
        limitations: FILE_LIMITATIONS,
        claims_supported: FILE_CLAIMS,
        claims_excluded: FILE_EXCLUDED,
        fixtures: FILE_FIXTURES,
        docs: FILE_DOCS,
        support_tier: "supported",
    }
}

const fn policy(
    sensor_id: &'static str,
    family: &'static str,
    class: &'static str,
) -> SensorCapability {
    SensorCapability {
        sensor_id,
        generation: 1,
        owner: "allow-policy-legacy",
        kind: Some("policy_exception"),
        family: Some(family),
        input: "canonical policy plus selected source-tree/configuration facts",
        selection: "enabled by a matching policy family or compatibility adapter",
        analysis_class: class,
        precision: "policy family plus source/configuration identity where available",
        completeness: "complete only for the selected adapter input; unsupported source shapes remain outside the adapter",
        platform: "source-tree/configuration adapter; no referenced behavior execution",
        profile: "default",
        limitations: POLICY_LIMITATIONS,
        claims_supported: POLICY_CLAIMS,
        claims_excluded: POLICY_EXCLUDED,
        fixtures: POLICY_FIXTURES,
        docs: POLICY_DOCS,
        support_tier: "supported",
    }
}

const fn not_included(sensor_id: &'static str, input: &'static str) -> SensorCapability {
    SensorCapability {
        sensor_id,
        generation: 1,
        owner: "cargo-allow",
        kind: None,
        family: None,
        input,
        selection: "not selected; no implementation in this generation",
        analysis_class: "not_included",
        precision: "none",
        completeness: "not applicable",
        platform: "none",
        profile: "none",
        limitations: NOT_INCLUDED_LIMITATIONS,
        claims_supported: NOT_INCLUDED_CLAIMS,
        claims_excluded: NOT_INCLUDED_EXCLUDED,
        fixtures: NOT_INCLUDED_FIXTURES,
        docs: NOT_INCLUDED_DOCS,
        support_tier: "excluded",
    }
}

const CAPABILITIES: &[SensorCapability] = &[
    source("rust.panic.unwrap", "panic", "unwrap"),
    source("rust.panic.expect", "panic", "expect"),
    source("rust.panic.panic_macro", "panic", "panic_macro"),
    source("rust.panic.todo", "panic", "todo"),
    source("rust.panic.unimplemented", "panic", "unimplemented"),
    source("rust.panic.unreachable", "panic", "unreachable"),
    source("rust.panic.indexing", "panic", "indexing"),
    source("rust.panic.string_slice", "panic", "string_slice"),
    source("rust.unsafe.unsafe_fn", "unsafe", "unsafe_fn"),
    source("rust.unsafe.unsafe_impl", "unsafe", "unsafe_impl"),
    source("rust.unsafe.unsafe_trait", "unsafe", "unsafe_trait"),
    source(
        "rust.unsafe.unsafe_extern_block",
        "unsafe",
        "unsafe_extern_block",
    ),
    source("rust.unsafe.unsafe_block", "unsafe", "unsafe_block"),
    source("rust.unsafe.unsafe_const", "unsafe", "unsafe_const"),
    source("rust.unsafe.unsafe_static", "unsafe", "unsafe_static"),
    source("rust.unsafe.unsafe_attr", "unsafe", "unsafe_attr"),
    source(
        "rust.lint.allow_attribute",
        "lint_exception",
        "allow_attribute",
    ),
    source(
        "rust.lint.expect_attribute",
        "lint_exception",
        "expect_attribute",
    ),
    source(
        "rust.lint.deny_attribute",
        "lint_exception",
        "deny_attribute",
    ),
    source(
        "rust.lint.forbid_attribute",
        "lint_exception",
        "forbid_attribute",
    ),
    source(
        "rust.lint.warn_attribute",
        "lint_exception",
        "warn_attribute",
    ),
    file(
        "file.non_rust.ci_declarative",
        "non_rust_file",
        "ci_declarative",
    ),
    file(
        "file.non_rust.editor_extension",
        "non_rust_file",
        "editor_extension",
    ),
    file(
        "file.non_rust.package_metadata",
        "non_rust_file",
        "package_metadata",
    ),
    file(
        "file.non_rust.test_fixture",
        "non_rust_file",
        "test_fixture",
    ),
    file(
        "file.non_rust.release_script",
        "non_rust_file",
        "release_script",
    ),
    file(
        "file.non_rust.documentation",
        "non_rust_file",
        "documentation",
    ),
    file(
        "file.non_rust.shell_script",
        "non_rust_file",
        "shell_script",
    ),
    file("file.non_rust.python_tool", "non_rust_file", "python_tool"),
    file(
        "file.non_rust.javascript_tool",
        "non_rust_file",
        "javascript_tool",
    ),
    file(
        "file.non_rust.configuration",
        "non_rust_file",
        "configuration",
    ),
    file(
        "file.non_rust.unknown_non_rust",
        "non_rust_file",
        "unknown_non_rust",
    ),
    file(
        "file.non_rust.ambiguous_file_family",
        "non_rust_file",
        "ambiguous_file_family",
    ),
    file(
        "file.generated.generated_code",
        "generated_code",
        "generated_code",
    ),
    policy(
        "policy.workflow.file",
        "github_workflow",
        "compatibility_adapter",
    ),
    policy(
        "policy.workflow.action",
        "workflow_external_action",
        "compatibility_adapter",
    ),
    policy(
        "policy.dependency",
        "dependency_surface",
        "compatibility_adapter",
    ),
    policy("policy.process", "process_spawn", "policy_derived"),
    policy("policy.network", "network_destination", "policy_derived"),
    policy("policy.executable", "executable_file", "policy_derived"),
    not_included(
        "excluded.workflow.rich_semantics",
        "workflow permissions, triggers, and expressions",
    ),
    not_included(
        "excluded.shell.behavior",
        "shell command behavior, taint, and runtime safety",
    ),
    not_included(
        "excluded.rust.macro_expansion",
        "Rust macro expansion and token-tree semantics",
    ),
    not_included(
        "excluded.rust.type_flow",
        "Rust type, control-flow, data-flow, and MIR semantics",
    ),
    not_included(
        "excluded.runtime.test_adequacy",
        "runtime behavior and test adequacy",
    ),
];

pub(crate) fn cmd_capabilities(args: &CapabilitiesArgs) -> CargoAllowResult<()> {
    validate_catalog()?;
    let capabilities = CAPABILITIES
        .iter()
        .copied()
        .filter(|capability| {
            args.class
                .is_none_or(|class| capability.analysis_class == class.as_str())
        })
        .filter(|capability| {
            args.kind
                .as_deref()
                .is_none_or(|kind| capability.kind == Some(kind))
        })
        .filter(|capability| {
            args.family
                .as_deref()
                .is_none_or(|family| capability.family == Some(family))
        })
        .collect::<Vec<_>>();
    let catalog = CapabilityCatalog {
        schema: SENSOR_CAPABILITY_SCHEMA,
        generation: 1,
        claim_boundary: "Source-tree observations only; no compilation, type, macro, MIR, runtime, or test-adequacy claim.",
        capabilities,
    };
    let rendered = match args.format {
        CapabilityFormat::Human => render_human(&catalog),
        CapabilityFormat::Json => serde_json::to_string_pretty(&catalog).map_err(|error| {
            CargoAllowError::new(format!("failed to render capability JSON: {error}"))
        })?,
    };
    emit_text(args.output.as_deref(), &format!("{rendered}\n"))
}

fn render_human(catalog: &CapabilityCatalog) -> String {
    let mut out = format!("cargo-allow sensor capabilities ({})\n\n", catalog.schema);
    out.push_str("Each row states the strongest source-tree claim this sensor supports.\n");
    out.push_str("It does not claim compilation, type, macro, MIR, runtime, or test adequacy.\n\n");
    for capability in &catalog.capabilities {
        let target = match (capability.kind, capability.family) {
            (Some(kind), Some(family)) => format!("{kind}/{family}"),
            _ => capability.input.to_string(),
        };
        out.push_str(&format!(
            "- {}: {} [{}]\n  owner={} input={} selection={} support={}\n",
            target,
            capability.sensor_id,
            capability.analysis_class,
            capability.owner,
            capability.input,
            capability.selection,
            capability.support_tier,
        ));
        out.push_str(&format!(
            "  supported={} excluded={} limitations={}\n",
            capability.claims_supported.join(", "),
            capability.claims_excluded.join(", "),
            capability.limitations.join(", "),
        ));
    }
    out
}

fn validate_catalog() -> CargoAllowResult<()> {
    let mut seen_ids = std::collections::BTreeSet::new();
    let mut seen_families = std::collections::BTreeSet::new();
    for capability in CAPABILITIES {
        if !seen_ids.insert(capability.sensor_id) {
            return Err(CargoAllowError::new(format!(
                "duplicate sensor capability id `{}`",
                capability.sensor_id
            )));
        }
        if capability.analysis_class == "not_included" {
            if capability.kind.is_some() || capability.family.is_some() {
                return Err(CargoAllowError::new(format!(
                    "not-included capability `{}` must not claim a finding family",
                    capability.sensor_id
                )));
            }
        } else if let (Some(kind), Some(family)) = (capability.kind, capability.family) {
            if !seen_families.insert((kind, family)) {
                return Err(CargoAllowError::new(format!(
                    "duplicate finding capability `{kind}/{family}`"
                )));
            }
        } else {
            return Err(CargoAllowError::new(format!(
                "finding capability `{}` is missing kind/family",
                capability.sensor_id
            )));
        }
        for field in [
            capability.owner,
            capability.input,
            capability.selection,
            capability.analysis_class,
            capability.precision,
            capability.completeness,
            capability.platform,
            capability.profile,
            capability.support_tier,
        ] {
            if field.trim().is_empty() {
                return Err(CargoAllowError::new(format!(
                    "capability `{}` has an empty required field",
                    capability.sensor_id
                )));
            }
        }
    }
    let mut expected = allow_rust::SOURCE_FINDING_FAMILIES.to_vec();
    expected.extend_from_slice(allow_files::FILE_FINDING_FAMILIES);
    expected.extend_from_slice(allow_policy_legacy::POLICY_FINDING_FAMILIES);
    expected.sort_unstable();
    let actual = seen_families.into_iter().collect::<Vec<_>>();
    if actual != expected {
        return Err(CargoAllowError::new(format!(
            "sensor capability family drift: catalog={actual:?}, scanner owners={expected:?}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensor_capability_catalog_is_unique_and_matches_scanner_owned_families() {
        validate_catalog().unwrap_or_else(|error| std::panic::panic_any(error.to_string()));
    }

    #[test]
    fn sensor_claim_boundaries_are_explicit_for_every_row() {
        assert!(CAPABILITIES.iter().all(|capability| {
            !capability.claims_supported.is_empty()
                && !capability.claims_excluded.is_empty()
                && !capability.limitations.is_empty()
                && !capability.fixtures.is_empty()
                && !capability.docs.is_empty()
        }));
        assert!(
            CAPABILITIES
                .iter()
                .any(|capability| capability.analysis_class == "not_included")
        );
    }

    #[test]
    fn packaged_sensor_capabilities_are_machine_parseable_and_filterable() {
        validate_catalog().unwrap_or_else(|error| std::panic::panic_any(error.to_string()));
        let json = serde_json::to_string(&CapabilityCatalog {
            schema: SENSOR_CAPABILITY_SCHEMA,
            generation: 1,
            claim_boundary: "source-only",
            capabilities: CAPABILITIES.to_vec(),
        })
        .unwrap_or_else(|error| std::panic::panic_any(error.to_string()));
        let value: serde_json::Value = serde_json::from_str(&json)
            .unwrap_or_else(|error| std::panic::panic_any(error.to_string()));
        assert_eq!(value["schema"], SENSOR_CAPABILITY_SCHEMA);
        assert!(
            value["capabilities"]
                .as_array()
                .is_some_and(|rows| rows.len() >= 40)
        );
        assert!(
            CAPABILITIES
                .iter()
                .filter(|capability| capability.analysis_class == "not_included")
                .all(|capability| capability.kind.is_none() && capability.family.is_none())
        );
    }
}
