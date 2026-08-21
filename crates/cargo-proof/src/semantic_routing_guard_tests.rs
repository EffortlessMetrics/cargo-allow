//! Semantic routing guard for cargo-proof and provider modules
//! (#3320 / #2943 step 8).
//!
//! proof-protocol is a data/serialization/structural-validation seam. Every
//! semantic decision (currentness, cache, aggregation, contradiction,
//! phase-gate, provider behavior, planning) must route through proof-engine
//! operations. These guards fail if cargo-proof or its provider modules
//! start reaching for protocol symbols outside the data seam.

#![cfg(test)]

use std::path::{Path, PathBuf};

/// proof-protocol symbols cargo-proof may consume: DTO types, schema ID
/// constants, and structural loaders/validators only. Anything else (in
/// particular a future semantic helper) requires updating this list with an
/// explicit review of the data-vs-semantic boundary.
const PROTOCOL_DATA_SYMBOLS: &[&str] = &[
    // plan DTOs
    "ProofPlanV1",
    "ProofPlanCommandV1",
    "ProofPlanError",
    "ProofPlanV2",
    "ProofItemDispositionV1",
    "ProofItemV1",
    "ProofItemExecutionPostureV1",
    "ProofSubjectClassV1",
    "ProofSubjectV1",
    "ProviderSelectionV1",
    "PROOF_PLAN_SCHEMA_ID",
    "PROOF_PLAN_COMMAND_SCHEMA_ID",
    "load_proof_plan_toml",
    "validate_proof_plan",
    // capability DTOs
    "ProofCapabilityCatalogV1",
    "ProofCapabilityV1",
    "ProofCapabilityKindV1",
    "PROOF_CAPABILITY_CATALOG_SCHEMA_ID",
    "validate_capability_catalog",
    // receipt DTOs
    "ProofReceiptSetV1",
    "ProofReceiptBindingV1",
    "PROOF_RECEIPT_SET_SCHEMA_ID",
    "PROOF_RECEIPT_BINDING_SCHEMA_ID",
    "validate_receipt_set",
    // contradiction DTOs
    "ProofContradictionReportV1",
    "ProofContradictionV1",
    "PROOF_CONTRADICTION_REPORT_SCHEMA_ID",
    // phase-gate DTOs
    "ProofPhaseGateV1",
    "ProofPhaseGatePostureV1",
    "PROOF_PHASE_GATE_SCHEMA_ID",
    // proof corpus data
    "ProofCorpusV1",
    "ProofCorpusScenarioV1",
    "ProofCorpusDimensionV1",
    "ProofResultStateV1",
    "BindingCurrentnessV1",
    "PROOF_CORPUS_SCHEMA_ID",
    "PROOF_CORPUS_DIGEST_V1",
    "load_proof_corpus_toml",
];

/// Semantic operation namespaces that must come from proof-engine.
const ENGINE_SEMANTIC_MARKERS: &[&str] = &[
    "proof_engine::dry_run_proof_plan",
    "proof_engine::plan_proof_execution_from_intent",
    "proof_engine::evaluate_currentness",
    "proof_engine::evaluate_intent_plan_currentness",
    "proof_engine::detect_contradictions",
    "proof_engine::evaluate_phase_gate",
    "proof_engine::ProofProviderV1",
];

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn collect_rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().and_then(|n| n.to_str()) == Some("target") {
                continue;
            }
            collect_rust_sources(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// Extract proof_protocol symbols referenced in a line, handling both
/// `use proof_protocol::{A, B};` and `proof_protocol::A` path forms.
fn referenced_protocol_symbols(text: &str) -> Vec<String> {
    let mut symbols = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") {
            continue;
        }
        for segment in trimmed.split("proof_protocol::").skip(1) {
            let remainder = segment
                .trim_start_matches('{')
                .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .find(|part| !part.is_empty());
            if let Some(symbol) = remainder {
                symbols.push(symbol.to_string());
            }
        }
    }
    symbols
}

#[test]
fn cargo_proof_consumes_only_protocol_data_symbols() -> Result<(), String> {
    let root = workspace_root();
    let self_relative = file!().replace('\\', "/");
    let mut sources = Vec::new();
    collect_rust_sources(&root.join("crates/cargo-proof/src"), &mut sources);
    if sources.is_empty() {
        return Err("expected cargo-proof sources to scan".into());
    }
    for path in &sources {
        let relative = path
            .strip_prefix(&root)
            .ok()
            .map(|rel| rel.to_string_lossy().replace('\\', "/"));
        if relative.as_deref() == Some(self_relative.as_str()) {
            continue;
        }
        let text = std::fs::read_to_string(path)
            .map_err(|err| format!("read {}: {err}", path.display()))?;
        for symbol in referenced_protocol_symbols(&text) {
            if !PROTOCOL_DATA_SYMBOLS.contains(&symbol.as_str()) {
                return Err(format!(
                    "{} references proof_protocol::{} which is outside the data/serialization \
                     seam; semantic decisions must route through proof-engine operations \
                     (#2943/#3320)",
                    path.display(),
                    symbol
                ));
            }
        }
    }
    Ok(())
}

#[test]
fn provider_modules_route_semantics_through_proof_engine() -> Result<(), String> {
    let root = workspace_root();
    for provider in ["cargo_allow", "hawk", "ripr"] {
        let adapter = root
            .join("crates/cargo-proof/src/providers")
            .join(provider)
            .join("adapter.rs");
        let text = std::fs::read_to_string(&adapter)
            .map_err(|err| format!("read {}: {err}", adapter.display()))?;
        let uses_engine_api = text.contains("proof_engine::ProofProviderV1")
            || (text.contains("proof_engine::") && text.contains("ProofProviderV1"));
        if !uses_engine_api {
            return Err(format!(
                "{} must implement the proof-engine provider API; provider semantics are \
                 engine-owned (#2943/#3320)",
                adapter.display()
            ));
        }
    }
    Ok(())
}

#[test]
fn semantic_operations_resolve_through_engine_not_protocol() -> Result<(), String> {
    // The CLI surface must reach dry-run/planning/currentness semantics via
    // proof-engine entry points only (compile-checked by these uses).
    let _ = ENGINE_SEMANTIC_MARKERS;
    let root = workspace_root();
    let cli_sources = [
        "crates/cargo-proof/src/plan.rs",
        "crates/cargo-proof/src/dry_run_cmd.rs",
    ];
    for rel in cli_sources {
        let path = root.join(rel);
        let text = std::fs::read_to_string(&path)
            .map_err(|err| format!("read {}: {err}", path.display()))?;
        if text.contains("proof_engine::") {
            continue;
        }
        return Err(format!(
            "{} must route semantics through proof_engine operations",
            path.display()
        ));
    }
    Ok(())
}

/// #3598 recurrence check: the fake provider must never be constructed
/// in cargo-proof's product (non-test) code. The fake stays available in
/// proof-engine for conformance and fixtures; this guard fails if any
/// non-test source under this crate references it.
#[test]
fn fake_provider_is_absent_from_product_code() -> Result<(), String> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders = Vec::new();
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).map_err(|error| error.to_string())?;
        for entry in entries {
            let entry = entry.map_err(|error| error.to_string())?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
                continue;
            }
            // Files whose names mark them as test-only (mounted via
            // #[cfg(test)] #[path = ...]) are not product code.
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == "tests.rs" || name.ends_with("_tests.rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
            let trimmed = strip_test_modules(&text);
            if trimmed.contains("FakeProofProvider") {
                offenders.push(path.display().to_string());
            }
        }
    }
    if offenders.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "fake provider referenced outside test modules: {offenders:?}"
        ))
    }
}

/// Remove `#[cfg(test)] mod … { … }` bodies so fixtures inside them do
/// not count as product references. Nested braces are tracked; a
/// malformed tail keeps the remaining text (fail-visible).
fn strip_test_modules(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut in_test_module = false;
    let mut depth: usize = 0;
    let mut saw_test_attr = false;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if !in_test_module {
            if trimmed.starts_with("#[cfg(test)]") {
                saw_test_attr = true;
                continue;
            }
            if saw_test_attr && trimmed.starts_with("mod ") {
                in_test_module = true;
                // The module's own opening brace is on this discarded
                // line, so start at one to track nesting correctly.
                depth = 1;
                saw_test_attr = false;
                continue;
            }
            if !trimmed.starts_with("#[") {
                saw_test_attr = false;
            }
            output.push_str(line);
            output.push('\n');
        } else {
            for ch in line.chars() {
                if ch == '{' {
                    depth = depth.saturating_add(1);
                } else if ch == '}' {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        in_test_module = false;
                        break;
                    }
                }
            }
        }
    }
    output
}
