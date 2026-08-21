//! The embeddable Changie sensor facade (#3621 PR C1).
//!
//! One bounded, experimental public entry point for external Rust
//! consumers (editors, language servers, repository tooling): supply
//! caller-owned source documents and entry facts, receive deterministic
//! parse/compile/lint results. The facade performs no acquisition of
//! its own — no Git, filesystem walk, network, or process — and its
//! signatures carry no cargo-allow application types.
//!
//! API evolution (experimental 0.x): breaking changes to this facade
//! may land within the `changie` feature's experimental window; the
//! compatibility generation (\`CHANGIE_COMPATIBILITY_GENERATION\`) and
//! the schema generations below are the stability seams. Consumers
//! should pin the feature and re-qualify on generation bumps.

use crate::changie::{
    CHANGIE_COMPATIBILITY_GENERATION, ChangieConfigDocument, ChangieFragmentDocument,
    ChangieSourceDocument,
};

use super::compiled_contract::{
    ChangieCompiledFragmentContractV1, ContractCompileBlocker, canonical_contract_text,
};
use super::{
    ChangieCompleteness, ChangieLintCandidate, ChangieLintReport, ChangieResultClass,
    compile_contract, lint,
};

/// Diagnostic schema generation for facade consumers: the shape of
/// `ChangieLintReport` diagnostics as projected by this facade.
pub const CHANGIE_DIAGNOSTIC_SCHEMA_GENERATION: u32 = 1;

/// Effective-rule schema generation: the shape of the compiled
/// contract's canonical serialization.
pub const CHANGIE_EFFECTIVE_RULE_SCHEMA_GENERATION: u32 = 1;

/// The experimental embeddable sensor facade. Constructing it is
/// free; every method is pure over caller-supplied inputs.
#[derive(Debug, Clone, Copy, Default)]
pub struct ChangieSensor;

impl ChangieSensor {
    /// The compatibility generation this sensor models.
    pub const fn generation(self) -> &'static str {
        CHANGIE_COMPATIBILITY_GENERATION
    }

    /// Diagnostic schema generation (see the module docs).
    pub const fn diagnostic_schema_generation(self) -> u32 {
        CHANGIE_DIAGNOSTIC_SCHEMA_GENERATION
    }

    /// Effective-rule schema generation (see the module docs).
    pub const fn effective_rule_schema_generation(self) -> u32 {
        CHANGIE_EFFECTIVE_RULE_SCHEMA_GENERATION
    }

    /// Parse a caller-supplied configuration document.
    pub fn parse_config(&self, source: ChangieSourceDocument) -> ChangieConfigDocument {
        crate::changie::parse_config(source)
    }

    /// Parse a caller-supplied fragment document.
    pub fn parse_fragment(&self, source: ChangieSourceDocument) -> ChangieFragmentDocument {
        crate::changie::parse_fragment(source)
    }

    /// Compile the effective fragment contract from a parsed
    /// configuration. Ambiguous or malformed configurations fail
    /// closed rather than producing a falsely complete contract.
    pub fn compile_contract(
        &self,
        config: &ChangieConfigDocument,
    ) -> Result<ChangieCompiledFragmentContractV1, ContractCompileBlocker> {
        compile_contract(config)
    }

    /// Deterministic canonical serialization of a compiled contract
    /// (the effective-rule schema surface for downstream projection).
    pub fn contract_text(&self, contract: &ChangieCompiledFragmentContractV1) -> String {
        canonical_contract_text(contract)
    }

    /// Lint caller-supplied candidate entries against a parsed
    /// configuration: configuration consistency, discovery
    /// classification, and persisted-fragment semantics in one
    /// deterministic report.
    pub fn lint(&self, candidate: ChangieLintCandidate) -> ChangieLintReport {
        lint(candidate)
    }
}

/// Deterministic report serialization for facade consumers: a compact,
/// stable projection of a lint report with the generation, schema
/// generations, completeness, not-proven dimensions, rule identities,
/// provenance classes, and source-located field paths. Equal reports
/// serialize equally; this is the wire shape editors and language
/// servers can cache against.
impl ChangieSensor {
    /// Deterministic report serialization (see the free-fn docs).
    pub fn serialize(self, report: &ChangieLintReport) -> String {
        serialize_report(self, report)
    }
}

pub fn serialize_report(sensor: ChangieSensor, report: &ChangieLintReport) -> String {
    let mut out = String::new();
    out.push_str("changie.lint-report.v1\n");
    out.push_str(&format!("generation={}\n", sensor.generation()));
    out.push_str(&format!(
        "diagnostic_schema_generation={}\n",
        sensor.diagnostic_schema_generation()
    ));
    out.push_str(&format!(
        "effective_rule_schema_generation={}\n",
        sensor.effective_rule_schema_generation()
    ));
    match report.completeness {
        ChangieCompleteness::Complete => out.push_str("completeness=complete\n"),
        ChangieCompleteness::Partial => out.push_str("completeness=partial\n"),
        ChangieCompleteness::NotProven => out.push_str("completeness=not_proven\n"),
    }
    for dimension in &report.not_proven_dimensions {
        out.push_str(&format!("not_proven={dimension}\n"));
    }
    for path in &report.discovered {
        out.push_str(&format!("discovered={path}\n"));
    }
    for path in &report.not_discovered {
        out.push_str(&format!("not_discovered={path}\n"));
    }
    let mut ordered: Vec<&super::ChangieDiagnostic> = report.diagnostics.iter().collect();
    ordered.sort_by_key(|diagnostic| {
        (
            diagnostic.repo_path.clone(),
            diagnostic.rule.as_str().to_string(),
            diagnostic.message.clone(),
        )
    });
    for diagnostic in ordered {
        out.push_str(&format!(
            "diagnostic rule={} class={} provenance={} path={}\n",
            diagnostic.rule.as_str(),
            result_class_str(diagnostic.result_class),
            diagnostic.provenance(),
            diagnostic.repo_path,
        ));
        if let Some(field_path) = &diagnostic.field_path {
            out.push_str(&format!("  field={field_path}\n"));
        }
        if let Some(expected_actual) = &diagnostic.expected_actual {
            out.push_str(&format!(
                "  expected={} actual={}\n",
                expected_actual.expected, expected_actual.actual
            ));
        }
    }
    out
}

fn result_class_str(class: ChangieResultClass) -> &'static str {
    class.as_str()
}

/// Re-exported so facade consumers need no other import path: the full
/// public model surface in one place (source documents, parses,
/// contract, lint types).
pub use super::compiled_contract::ChangieCompiledFragmentContractV1 as Contract;
pub use crate::changie::{
    ChangieConfigDocument as ConfigDocument, ChangieFragmentDocument as FragmentDocument,
    ChangieSourceDocument as SourceDocument,
};

#[cfg(test)]
#[path = "sensor_tests.rs"]
mod sensor_tests;
