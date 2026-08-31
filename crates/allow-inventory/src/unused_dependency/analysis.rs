//! Bounded homegrown composition: manifest row parse, textual reference
//! scan, and honest classification (#3909 PR A).
//!
//! Everything is a pure function of the caller-supplied request: no
//! filesystem access, no process invocation, no network, no Cargo metadata.
//! Determinism comes from sorted rows, sorted evidence, and token-based
//! detection that never depends on map iteration order. Malformed manifests
//! and unmodelable edges surface as `InstrumentFailure` posture or
//! `Unsupported` dispositions — never silently clean rows.
//!
//! Classification ordering is law (see `classify_row`): build rows before
//! dev rows before normal/optional/target rows; direct textual references
//! win over `cfg(`-gated references, which win over `dep:` feature
//! activators, which win over absence-based advisory findings.

use super::{
    UNUSED_DEPENDENCY_ANALYZER_IDENTITY, UNUSED_DEPENDENCY_CLAIM_BOUNDARY,
    UnusedDependencyDependencyClassV1, UnusedDependencyDispositionV1, UnusedDependencyFindingV1,
    UnusedDependencyInstrumentPostureV1, UnusedDependencyManifestRowV1, UnusedDependencyReceiptV1,
    UnusedDependencyRequestV1, declared_absence_limitation, declared_unscanned_kinds,
};
use std::collections::BTreeSet;

/// Manifest-relative path used for evidence rooted in the manifest itself
/// (feature activators), shaped like the source-input evidence entries.
const MANIFEST_EVIDENCE_PATH: &str = "Cargo.toml";

/// Kind of one scanned source input, derived from its package-relative path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputKind {
    /// Under `src/`: production source of the package.
    Src,
    /// Under `tests/`, `examples/`, or `benches/`: fixture surface.
    Fixture,
    /// `build.rs` or under `build/`: build-script surface.
    BuildScript,
    /// Any other declared input (scanned, none of the above).
    Other,
}

/// One source input normalized for scanning: `/`-separated path, LF-only
/// text split into lines.
struct ScannedInput {
    path: String,
    kind: InputKind,
    lines: Vec<String>,
}

/// Everything the classifier needs beyond one manifest row.
struct AnalysisContext {
    inputs: Vec<ScannedInput>,
    manifest_lines: Vec<String>,
    /// Manifest keys activated through `dep:` entries in the `[features]`
    /// table (registry law: the key spelling, aliases included).
    activated_keys: BTreeSet<String>,
}

/// Inventory one request into a receipt. The current composition always
/// completes with a posture; the `Result` reserves the channel for
/// caller-level composition errors.
pub(crate) fn inventory(
    request: &UnusedDependencyRequestV1,
) -> Result<UnusedDependencyReceiptV1, String> {
    let manifest_text = normalize_text(&request.manifest_text);
    let root: toml::Value = match toml::from_str(&manifest_text) {
        Ok(root) => root,
        Err(error) => {
            return Ok(instrument_failure_receipt(
                request,
                format!("manifest_parse_error:{error}"),
            ));
        }
    };
    let mut rows = manifest_rows(&root);
    rows.sort();

    let context = AnalysisContext {
        inputs: scanned_inputs(request),
        manifest_lines: manifest_text.lines().map(str::to_string).collect(),
        activated_keys: feature_activator_keys(&root),
    };

    let findings: Vec<UnusedDependencyFindingV1> = rows
        .iter()
        .map(|row| classify_row(request, row, &context))
        .collect();

    Ok(UnusedDependencyReceiptV1 {
        schema_id: UnusedDependencyReceiptV1::CURRENT_SCHEMA_ID.to_string(),
        schema_version: UnusedDependencyReceiptV1::CURRENT_SCHEMA_VERSION,
        package_name: request.package_name.clone(),
        configuration_id: request.configuration_id.clone(),
        packages_inspected: 1,
        findings,
        instrument_posture: UnusedDependencyInstrumentPostureV1::Complete,
        analyzer_identity: UNUSED_DEPENDENCY_ANALYZER_IDENTITY.to_string(),
        claim_boundary: UNUSED_DEPENDENCY_CLAIM_BOUNDARY.to_string(),
    })
}

/// Malformed manifest: the receipt keeps its package identity but every row
/// is restricted to one failure description. Nothing renders clean.
pub(super) fn instrument_failure_receipt(
    request: &UnusedDependencyRequestV1,
    failure: String,
) -> UnusedDependencyReceiptV1 {
    let finding = UnusedDependencyFindingV1 {
        package_name: request.package_name.clone(),
        manifest_row: UnusedDependencyManifestRowV1 {
            // The manifest did not parse: this row is a failure marker, not
            // a manifest row, so it carries no dependency identity.
            dependency_name: String::new(),
            alias: None,
            class: UnusedDependencyDependencyClassV1::Normal,
            optional: false,
            target: None,
            features_selected: Vec::new(),
        },
        configuration_id: request.configuration_id.clone(),
        disposition: UnusedDependencyDispositionV1::InstrumentFailure,
        evidence: vec![failure],
        limitations: vec![
            "the manifest failed to parse, so no dependency row was \
             classified and no row renders clean"
                .to_string(),
        ],
    };
    UnusedDependencyReceiptV1 {
        schema_id: UnusedDependencyReceiptV1::CURRENT_SCHEMA_ID.to_string(),
        schema_version: UnusedDependencyReceiptV1::CURRENT_SCHEMA_VERSION,
        package_name: request.package_name.clone(),
        configuration_id: request.configuration_id.clone(),
        packages_inspected: 1,
        findings: vec![finding],
        instrument_posture: UnusedDependencyInstrumentPostureV1::InstrumentFailure,
        analyzer_identity: UNUSED_DEPENDENCY_ANALYZER_IDENTITY.to_string(),
        claim_boundary: UNUSED_DEPENDENCY_CLAIM_BOUNDARY.to_string(),
    }
}

/// Normalize intake text: CRLF and lone CR become LF so textual scanning
/// never depends on the checkout's line-ending smudging.
fn normalize_text(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// Normalize one declared relative path to `/` separators.
fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn input_kind(relative_path: &str) -> InputKind {
    let path = normalize_path(relative_path);
    let fixture_roots = ["tests", "examples", "benches"];
    if fixture_roots.iter().any(|root| {
        path == *root
            || path
                .strip_prefix(root)
                .is_some_and(|rest| rest.starts_with('/'))
    }) {
        return InputKind::Fixture;
    }
    if path == "build.rs" || path.strip_prefix("build/").is_some() {
        return InputKind::BuildScript;
    }
    if path == "src" || path.strip_prefix("src/").is_some() {
        return InputKind::Src;
    }
    InputKind::Other
}

fn scanned_inputs(request: &UnusedDependencyRequestV1) -> Vec<ScannedInput> {
    request
        .source_inputs
        .iter()
        .map(|input| {
            let normalized = normalize_text(&input.text);
            ScannedInput {
                kind: input_kind(&input.relative_path),
                path: normalize_path(&input.relative_path),
                lines: normalized.lines().map(str::to_string).collect(),
            }
        })
        .collect()
}

/// Extract every declared dependency row from the parsed manifest, keeping
/// table class, optional flag, target spec, alias identity, and selected
/// features. Shorthand strings, inline tables, and long-hand tables are all
/// accepted; rows come out sorted for determinism.
fn manifest_rows(root: &toml::Value) -> Vec<UnusedDependencyManifestRowV1> {
    let mut rows = Vec::new();
    for (table_key, class) in class_tables() {
        if let Some(table) = root.get(table_key).and_then(toml::Value::as_table) {
            for (name, value) in table {
                rows.push(manifest_row(name, value, class, None));
            }
        }
    }
    let Some(targets) = root.get("target").and_then(toml::Value::as_table) else {
        return rows;
    };
    for (spec, spec_table) in targets {
        for (table_key, class) in class_tables() {
            let Some(table) = spec_table.get(table_key).and_then(toml::Value::as_table) else {
                continue;
            };
            for (name, value) in table {
                // Under a target only the plain-dependencies table becomes
                // TargetSpecific; dev/build keep their class so the
                // (class, target, optional) triple stays lossless.
                let class = match class {
                    UnusedDependencyDependencyClassV1::Normal => {
                        UnusedDependencyDependencyClassV1::TargetSpecific
                    }
                    other => other,
                };
                rows.push(manifest_row(name, value, class, Some(spec.clone())));
            }
        }
    }
    rows
}

fn class_tables() -> [(&'static str, UnusedDependencyDependencyClassV1); 3] {
    [
        ("dependencies", UnusedDependencyDependencyClassV1::Normal),
        ("dev-dependencies", UnusedDependencyDependencyClassV1::Dev),
        (
            "build-dependencies",
            UnusedDependencyDependencyClassV1::Build,
        ),
    ]
}

fn manifest_row(
    key: &str,
    value: &toml::Value,
    class: UnusedDependencyDependencyClassV1,
    target: Option<String>,
) -> UnusedDependencyManifestRowV1 {
    let mut optional = false;
    let mut features_selected = Vec::new();
    let mut package_rename = None;
    if let Some(table) = value.as_table() {
        optional = table
            .get("optional")
            .and_then(toml::Value::as_bool)
            .unwrap_or(false);
        package_rename = table.get("package").and_then(toml::Value::as_str);
        if let Some(selected) = table.get("features").and_then(toml::Value::as_array) {
            for feature in selected {
                if let Some(name) = feature.as_str() {
                    features_selected.push(name.to_string());
                }
            }
        }
    }
    let dependency_name = package_rename.unwrap_or(key).to_string();
    let alias = package_rename.map(|_| key.to_string());
    let class = match class {
        UnusedDependencyDependencyClassV1::Normal if optional => {
            UnusedDependencyDependencyClassV1::OptionalNormal
        }
        other => other,
    };
    features_selected.sort();
    features_selected.dedup();
    UnusedDependencyManifestRowV1 {
        dependency_name,
        alias,
        class,
        optional,
        target,
        features_selected,
    }
}

/// Registry law for `dep:` activators: the feature table names the manifest
/// key (the alias when renamed), so the set collects key spellings.
fn feature_activator_keys(root: &toml::Value) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    let Some(features) = root.get("features").and_then(toml::Value::as_table) else {
        return keys;
    };
    for selections in features.values() {
        let Some(list) = selections.as_array() else {
            continue;
        };
        for selection in list {
            let Some(text) = selection.as_str() else {
                continue;
            };
            if let Some(key) = text.strip_prefix("dep:") {
                keys.insert(key.to_string());
            }
        }
    }
    keys
}

/// The Rust identifier a dependency row is referenced by: the alias's
/// identifier form when renamed, otherwise the registry name with `-`
/// folded to `_`.
fn reference_ident(row: &UnusedDependencyManifestRowV1) -> String {
    row.alias
        .as_deref()
        .unwrap_or(row.dependency_name.as_str())
        .replace('-', "_")
}

/// The manifest key a `dep:` activator names for this row: the alias when
/// renamed, otherwise the registry name as declared.
fn activator_key(row: &UnusedDependencyManifestRowV1) -> &str {
    row.alias.as_deref().unwrap_or(row.dependency_name.as_str())
}

/// Token-based reference detection for one line. Two tokenizations:
/// identifier tokens (splitting on every non-identifier character) for
/// `use <ident>` and `extern crate <ident>`, and path tokens (keeping `:`)
/// for `<ident>::` path prefixes. Token equality supplies the word
/// boundaries, so `my_crate::` never matches `crate::` and
/// `serde_json_alias` never matches `serde_json`.
fn line_references_ident(line: &str, ident: &str) -> bool {
    let identifier_tokens: Vec<&str> = line
        .split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
        .collect();
    let use_statement = identifier_tokens
        .windows(2)
        .any(|window| matches!(window, [keyword, name] if *keyword == "use" && *name == ident));
    let extern_crate = identifier_tokens.windows(3).any(|window| {
        matches!(window, [extern_keyword, crate_keyword, name]
            if *extern_keyword == "extern" && *crate_keyword == "crate" && *name == ident)
    });
    if use_statement || extern_crate {
        return true;
    }
    let path_tokens: Vec<&str> = line
        .split(|character: char| {
            !(character.is_ascii_alphanumeric() || character == '_' || character == ':')
        })
        .collect();
    path_tokens.iter().any(|token| {
        token
            .strip_prefix(ident)
            .is_some_and(|rest| rest.starts_with("::"))
    })
}

/// Feature-activator evidence lines for one manifest key: every manifest
/// line carrying the exact `dep:<key>` token. The parsed `[features]` table
/// gates activation, so comment mentions never fabricate evidence.
fn activator_evidence(context: &AnalysisContext, key: &str) -> Vec<String> {
    let needle = format!("dep:{key}");
    context
        .manifest_lines
        .iter()
        .enumerate()
        .filter(|(_, line)| {
            line.split(|character: char| {
                !(character.is_ascii_alphanumeric() || character == '_' || character == ':')
            })
            .any(|token| token == needle)
        })
        .map(|(index, _)| format!("{MANIFEST_EVIDENCE_PATH}:{}: {needle}", index + 1))
        .collect()
}

/// Evidence naming the scanned input set for zero-reference findings, so
/// the receipt shows exactly what absence was measured over.
fn scanned_set_evidence(context: &AnalysisContext) -> Vec<String> {
    if context.inputs.is_empty() {
        return vec!["no_source_inputs_supplied".to_string()];
    }
    context
        .inputs
        .iter()
        .map(|input| format!("{}: scanned_no_reference_found", input.path))
        .collect()
}

fn absence_limitations(row: &UnusedDependencyManifestRowV1) -> Vec<String> {
    let mut limitations: Vec<String> = declared_unscanned_kinds()
        .into_iter()
        .map(str::to_string)
        .collect();
    limitations.push(declared_absence_limitation().to_string());
    if row.target.is_some() {
        limitations.push(
            "target-specific rows compile only under their declared target, \
             so absence from the scanned inputs may reflect the host \
             configuration rather than non-use"
                .to_string(),
        );
    }
    limitations
}

/// Classify one row. The ordering is law: build rows, then dev rows, then
/// normal/optional/target rows; direct evidence beats `cfg(`-gated
/// evidence, which beats `dep:` activators, which beats absence.
fn classify_row(
    request: &UnusedDependencyRequestV1,
    row: &UnusedDependencyManifestRowV1,
    context: &AnalysisContext,
) -> UnusedDependencyFindingV1 {
    let mut idents = vec![reference_ident(row)];
    let lib_identity = request
        .dependency_lib_identities
        .iter()
        .find(|identity| identity.package_name == row.dependency_name)
        .map(|identity| identity.lib_name.replace('-', "_"));
    for lib_ident in &lib_identity {
        if !idents.contains(lib_ident) {
            idents.push(lib_ident.clone());
        }
    }
    let mut direct_refs = Vec::new();
    let mut gated_refs = Vec::new();
    let mut build_input_refs = Vec::new();
    let mut reference_seen = false;
    let mut non_fixture_reference_seen = false;
    for input in &context.inputs {
        for (index, line) in input.lines.iter().enumerate() {
            let Some(matched) = idents
                .iter()
                .find(|ident| line_references_ident(line, ident))
            else {
                continue;
            };
            reference_seen = true;
            if input.kind != InputKind::Fixture {
                non_fixture_reference_seen = true;
            }
            let evidence = format!("{}:{}: {}", input.path, index + 1, matched);
            if input.kind == InputKind::BuildScript {
                build_input_refs.push(evidence.clone());
            }
            if line.contains("cfg(") {
                gated_refs.push(evidence);
            } else {
                direct_refs.push(evidence);
            }
        }
    }
    let activators = if matches!(
        row.class,
        UnusedDependencyDependencyClassV1::Normal
            | UnusedDependencyDependencyClassV1::OptionalNormal
            | UnusedDependencyDependencyClassV1::TargetSpecific
    ) && context.activated_keys.contains(activator_key(row))
    {
        activator_evidence(context, activator_key(row))
    } else {
        Vec::new()
    };

    let mut evidence;
    let mut limitations = Vec::new();
    let disposition = match row.class {
        UnusedDependencyDependencyClassV1::Build => {
            if !build_input_refs.is_empty() {
                evidence = build_input_refs;
                UnusedDependencyDispositionV1::BuildOrGeneratedUse
            } else if reference_seen {
                evidence = direct_refs;
                evidence.extend(gated_refs);
                UnusedDependencyDispositionV1::Used
            } else if request.build_script_present {
                // A declared build script exists but is not among the
                // scanned inputs: its generated use is outside the
                // composition, so the row can never render unused.
                evidence = scanned_set_evidence(context);
                limitations.push(
                    "the package declares a build script that is not among \
                     the scanned inputs; build-script consumption is outside \
                     this composition"
                        .to_string(),
                );
                UnusedDependencyDispositionV1::BuildOrGeneratedUse
            } else {
                evidence = scanned_set_evidence(context);
                limitations = absence_limitations(row);
                UnusedDependencyDispositionV1::ApparentlyUnused
            }
        }
        UnusedDependencyDependencyClassV1::Dev => {
            if !reference_seen {
                evidence = scanned_set_evidence(context);
                limitations = absence_limitations(row);
                UnusedDependencyDispositionV1::ApparentlyUnused
            } else if non_fixture_reference_seen {
                // Referenced outside tests/, examples/, and benches/: at
                // least production-adjacent use is attested.
                evidence = direct_refs;
                evidence.extend(gated_refs);
                UnusedDependencyDispositionV1::Used
            } else {
                evidence = direct_refs;
                evidence.extend(gated_refs);
                UnusedDependencyDispositionV1::DevFixtureUse
            }
        }
        UnusedDependencyDependencyClassV1::Normal
        | UnusedDependencyDependencyClassV1::TargetSpecific => {
            if !direct_refs.is_empty() || !gated_refs.is_empty() {
                evidence = direct_refs;
                evidence.extend(gated_refs);
                UnusedDependencyDispositionV1::Used
            } else if !activators.is_empty() {
                // A non-optional row activated only through `dep:` is a
                // manifest shape Cargo rejects and this composition cannot
                // attribute: it must never render clean or used.
                evidence = activators;
                limitations.push(
                    "dep: feature activators only select optional \
                     dependencies; a non-optional row activated only through \
                     dep: is a manifest shape this composition cannot \
                     attribute"
                        .to_string(),
                );
                limitations.extend(declared_unscanned_kinds().into_iter().map(str::to_string));
                UnusedDependencyDispositionV1::Unsupported
            } else {
                evidence = scanned_set_evidence(context);
                limitations = absence_limitations(row);
                UnusedDependencyDispositionV1::ApparentlyUnused
            }
        }
        UnusedDependencyDependencyClassV1::OptionalNormal => {
            if !direct_refs.is_empty() {
                evidence = direct_refs;
                UnusedDependencyDispositionV1::Used
            } else if !gated_refs.is_empty() || !activators.is_empty() {
                // Optional rows referenced only through cfg-gated code or
                // dep: activators are configuration-dependent: default-
                // feature analysis can never declare them unused globally.
                evidence = gated_refs;
                evidence.extend(activators);
                UnusedDependencyDispositionV1::ConditionallyUsed
            } else {
                evidence = scanned_set_evidence(context);
                limitations = absence_limitations(row);
                UnusedDependencyDispositionV1::ApparentlyUnused
            }
        }
    };

    evidence.sort();
    evidence.dedup();
    UnusedDependencyFindingV1 {
        package_name: request.package_name.clone(),
        manifest_row: row.clone(),
        configuration_id: request.configuration_id.clone(),
        disposition,
        evidence,
        limitations,
    }
}
