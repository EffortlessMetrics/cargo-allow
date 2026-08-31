//! Feature-aware unused-dependency inventory contracts (#3909 PR A).
//!
//! The outcome is an advisory inventory: for one exact
//! package/manifest/source-input identity, every declared dependency row
//! lands in a closed disposition vocabulary, and the receipt carries the
//! analyzer identity, its limitations, and a claim boundary. Nothing here
//! removes dependencies, blocks CI, or executes project code; the pure core
//! reads only the manifest and source texts the caller supplies.
//!
//! ## Analyzer selection law (recorded decision, #3909 PR A)
//!
//! Candidate analyzers were qualified against the issue's evidence law
//! (workspace aliases, features, target dependencies, build/dev use,
//! generated source, package extraction) and the repository's
//! pinned/exact-evidence posture:
//!
//! - `cargo-udeps` requires a nightly rustc (`-Z` unloadable-runtime
//!   flags), which fails the workspace's pinned stable toolchain law.
//! - `cargo-machete` and `cargo-shear` are external binaries whose exact
//!   version cannot be pinned in-tree without adding supply-chain surface
//!   and configuration drift; their heuristics also cannot name their own
//!   limitations in the receipt vocabulary this module owns.
//!
//! The selected composition is bounded and homegrown: parse the caller's
//! manifest text with the `toml` crate, classify declared dependency rows
//! per class, scan the caller's declared source inputs for textual
//! references, and classify with honest limitations. Every edge this
//! composition cannot model is surfaced as an explicit
//! [`UnusedDependencyDispositionV1::Unsupported`] or
//! [`UnusedDependencyInstrumentPostureV1::InstrumentFailure`] row — never
//! silently clean. Absence-based findings stay advisory
//! ([`UnusedDependencyDispositionV1::ApparentlyUnused`]): source-reference
//! absence is never proof of non-use.
//!
//! ## Update law
//!
//! Schema and behavior changes to this module are bound by its contract
//! tests: `contract_tests` pins the fixture matrix (true-unused and
//! false-positive shapes, alias identity, feature activators, target rows,
//! malformed manifests, zero-inspection posture, exception validation, and
//! receipt round-trip). A change that moves any pinned disposition without
//! moving its fixture is a contract violation.
//!
//! ## Claim boundary
//!
//! This inventory is advisory evidence for reviewed dispositions. It is
//! never a removal authorization and never a CI gate; PR B owns
//! dispositioning the live inventory, and no enforcement exists until a
//! separately authorized PR introduces one.

use serde::{Deserialize, Serialize};

pub(crate) mod analysis;
pub(crate) mod exception;

#[cfg(test)]
mod contract_tests;

pub use exception::{UnusedDependencyExceptionV1, validate_exception};

/// Schema identity for the unused-dependency receipt family.
pub const UNUSED_DEPENDENCY_RECEIPT_V1_SCHEMA_ID: &str = "cargo-allow.unused-dependency-receipt.v1";

/// Current schema version of the unused-dependency receipt.
pub const UNUSED_DEPENDENCY_RECEIPT_V1_SCHEMA_VERSION: u32 = 1;

/// Exact identity of the selected bounded homegrown composition.
///
/// External analyzers (cargo-udeps nightly requirement; cargo-machete and
/// cargo-shear unpinned external binaries) fail the repository's
/// pinned/exact-evidence law; this composition names its own limitations
/// instead and marks unmodelable edges `Unsupported` rather than silently
/// clean.
pub const UNUSED_DEPENDENCY_ANALYZER_IDENTITY: &str = "cargo-allow.unused-dependency-inventory.v1: \
     bounded homegrown composition (toml manifest row parse + textual \
     source-reference scan over caller-supplied inputs); no external analyzer \
     binaries, no Cargo/rustc invocation, no proc-macro expansion; dependency \
     [lib] name remaps are modeled through caller-supplied identities, and \
     dependencies without a supplied identity are scanned under the folded \
     package name only";

/// Claim boundary carried by every receipt this module renders.
pub const UNUSED_DEPENDENCY_CLAIM_BOUNDARY: &str = "advisory feature-aware inventory only: an \
     ApparentlyUnused disposition is a candidate finding for reviewed \
     disposition, never a removal authorization, and absence of textual \
     source references never proves non-use";

/// Manifest table context a dependency row was declared in.
///
/// The class is lossless with [`UnusedDependencyManifestRowV1::target`] and
/// [`UnusedDependencyManifestRowV1::optional`]: `[target.<spec>.X]` rows
/// keep their target spec, build/dev rows keep their class under a target,
/// and an optional `[dependencies]` row becomes `OptionalNormal` while
/// carrying `optional = true`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnusedDependencyDependencyClassV1 {
    /// `[dependencies]` without `optional = true`.
    Normal,
    /// `[dependencies]` with `optional = true`; selected only through
    /// features, so default-feature analysis can never declare it unused.
    OptionalNormal,
    /// `[build-dependencies]` (at root or under a target).
    Build,
    /// `[dev-dependencies]` (at root or under a target).
    Dev,
    /// `[target.<spec>.dependencies]`; the spec is kept on the row.
    TargetSpecific,
}

impl UnusedDependencyDependencyClassV1 {
    /// Stable machine-facing label derived from the variant name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::OptionalNormal => "optional_normal",
            Self::Build => "build",
            Self::Dev => "dev",
            Self::TargetSpecific => "target_specific",
        }
    }
}

/// Closed disposition vocabulary for one declared dependency row (#3909).
///
/// Declaration order is the issue's vocabulary order. The analyzer emits
/// `Used`, `ApparentlyUnused`, `ConditionallyUsed`, `BuildOrGeneratedUse`,
/// `DevFixtureUse`, `Unsupported`, and (via the receipt posture)
/// `InstrumentFailure`; `TransitionalUse` and `ExplicitException` are
/// downstream disposition joins reserved for later PRs and are never
/// fabricated from source evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnusedDependencyDispositionV1 {
    /// Exact reference evidence was found in the scanned inputs.
    Used,
    /// Advisory candidate finding: no reference was found in the scanned
    /// inputs. This is never a removal authorization — build scripts, proc
    /// macros, generated code, doctests, and package assets are outside
    /// this composition, so the row requires reviewed disposition (remove,
    /// retain with evidence, transition, or unsupported) before any change.
    ApparentlyUnused,
    /// Referenced only through `dep:` feature activators or `cfg(`
    /// attribute lines, so use exists but is configuration-dependent.
    /// Default-feature analysis can never declare an optional dependency
    /// unused globally.
    ConditionallyUsed,
    /// A build-dependency whose consumption is the build-script context:
    /// referenced by a build-script input, or present while the request
    /// declares a build script whose generated use is outside the scanned
    /// inputs.
    BuildOrGeneratedUse,
    /// A dev-dependency referenced only by fixture inputs under `tests/`,
    /// `examples/`, or `benches/`; retained as fixture evidence, not
    /// production use.
    DevFixtureUse,
    /// Reserved for downstream disposition: the row is used only through a
    /// temporary extraction shim and the disposition must name a
    /// controlling removal/expiry authority reference. The analyzer never
    /// assigns this from source evidence alone.
    TransitionalUse,
    /// Reserved for downstream disposition: a reviewed exception row in the
    /// durable ledger retains the dependency. The analyzer never assigns
    /// this from source evidence alone.
    ExplicitException,
    /// The analyzer cannot model the edge, so the row must never render
    /// clean: the composition names what it could not attribute in
    /// `limitations` (required non-empty for this disposition).
    Unsupported,
    /// The instrument failed for this row (for example a malformed
    /// manifest); the row is a failure description and never a finding
    /// about the dependency.
    InstrumentFailure,
}

impl UnusedDependencyDispositionV1 {
    /// Stable machine-facing label derived from the variant name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Used => "used",
            Self::ApparentlyUnused => "apparently_unused",
            Self::ConditionallyUsed => "conditionally_used",
            Self::BuildOrGeneratedUse => "build_or_generated_use",
            Self::DevFixtureUse => "dev_fixture_use",
            Self::TransitionalUse => "transitional_use",
            Self::ExplicitException => "explicit_exception",
            Self::Unsupported => "unsupported",
            Self::InstrumentFailure => "instrument_failure",
        }
    }
}

/// Posture of one unused-dependency receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnusedDependencyInstrumentPostureV1 {
    /// The package's manifest parsed and every declared row is classified
    /// (advisory dispositions included).
    Complete,
    /// The instrument failed (malformed manifest) or zero packages were
    /// inspected; the receipt can never render clean.
    InstrumentFailure,
}

impl UnusedDependencyInstrumentPostureV1 {
    /// Stable machine-facing label derived from the variant name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::InstrumentFailure => "instrument_failure",
        }
    }
}

/// One source input the caller supplies for scanning. The pure core has no
/// filesystem access; text is normalized to LF on intake.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct UnusedDependencySourceInputV1 {
    /// Package-relative, `/`-separated path (for example `src/lib.rs` or
    /// `tests/it.rs`).
    pub relative_path: String,
    /// Exact source text; CRLF is normalized to LF on intake.
    pub text: String,
}

/// One dependency row parsed from the manifest text.
///
/// `dependency_name` is the registry package name; `alias` is the manifest
/// table key when the row is renamed via `package = "..."`, so alias and
/// package identity stay distinct (#3909 negative control 4).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct UnusedDependencyManifestRowV1 {
    /// Registry package name the row resolves to.
    pub dependency_name: String,
    /// Manifest table key when renamed via `package = "..."`.
    pub alias: Option<String>,
    pub class: UnusedDependencyDependencyClassV1,
    pub optional: bool,
    /// Target spec for `[target.<spec>...]` rows.
    pub target: Option<String>,
    /// Feature names the row selects, sorted and deduplicated.
    pub features_selected: Vec<String>,
}

/// A dependency package's Rust lib identity when it differs from the folded
/// package name: a manifest may rename the crate root with
/// `[lib] name = "..."`, and references then use the lib spelling, not the
/// folded package name. The caller supplies these identities where it can
/// observe them (workspace member manifests); dependencies without a
/// supplied identity are scanned under the folded package name only.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct UnusedDependencyLibIdentityV1 {
    pub package_name: String,
    pub lib_name: String,
}

/// Exact request identity for one package/configuration inspection.
///
/// The caller supplies the exact manifest text, package identity, selected
/// configuration ID (#3905), source inputs, and build-script presence. No
/// Cargo metadata, resolution, or filesystem access happens here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnusedDependencyRequestV1 {
    pub package_name: String,
    pub package_version: String,
    /// Selected feature-configuration identity (#3905) the inspection is
    /// bound to.
    pub configuration_id: String,
    /// Exact manifest text for the package.
    pub manifest_text: String,
    /// Declared source inputs to scan, in caller-supplied order.
    pub source_inputs: Vec<UnusedDependencySourceInputV1>,
    /// Whether the package declares a build script (its generated use is
    /// then outside the scanned inputs).
    pub build_script_present: bool,
    /// Lib identities for dependency packages whose crate root is renamed
    /// via `[lib] name`. Absent identities are scanned under the folded
    /// package name (declared limitation for unobserved registry deps).
    #[serde(default)]
    pub dependency_lib_identities: Vec<UnusedDependencyLibIdentityV1>,
}

/// One classified dependency row with exact evidence and limitations.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct UnusedDependencyFindingV1 {
    pub package_name: String,
    pub manifest_row: UnusedDependencyManifestRowV1,
    pub configuration_id: String,
    pub disposition: UnusedDependencyDispositionV1,
    /// Exact references found, each shaped `<relative_path>:<line>:
    /// <identifier>`. Zero-reference findings instead name the scanned
    /// input set; instrument-failure findings name the failure.
    pub evidence: Vec<String>,
    /// Analyzer limitation notes; required non-empty for `Unsupported` and
    /// `InstrumentFailure` dispositions.
    pub limitations: Vec<String>,
}

/// Deterministic advisory receipt for one package/configuration inspection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnusedDependencyReceiptV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub package_name: String,
    pub configuration_id: String,
    /// 1 for every receipt this module's analyzer produces (the package was
    /// inspected; `Complete` or `InstrumentFailure` is the posture), 0 only
    /// for zero-inspection receipts from [`empty_receipt`].
    pub packages_inspected: u32,
    pub findings: Vec<UnusedDependencyFindingV1>,
    pub instrument_posture: UnusedDependencyInstrumentPostureV1,
    /// Pinned composition identity; see
    /// [`UNUSED_DEPENDENCY_ANALYZER_IDENTITY`].
    pub analyzer_identity: String,
    /// Pinned advisory claim boundary; see
    /// [`UNUSED_DEPENDENCY_CLAIM_BOUNDARY`].
    pub claim_boundary: String,
}

impl UnusedDependencyReceiptV1 {
    pub const CURRENT_SCHEMA_ID: &'static str = UNUSED_DEPENDENCY_RECEIPT_V1_SCHEMA_ID;
    pub const CURRENT_SCHEMA_VERSION: u32 = UNUSED_DEPENDENCY_RECEIPT_V1_SCHEMA_VERSION;
}

/// The limitation strings the analyzer emits for input kinds outside the
/// composition. Callers and reviewers read these to know what an
/// `ApparentlyUnused` disposition does not cover.
pub fn declared_unscanned_kinds() -> Vec<&'static str> {
    vec![
        "build-script execution and build-script-generated code",
        "proc-macro expansion",
        "doctests",
        "generated code outside the supplied source inputs",
        "packaged assets and non-Rust fixture harnesses",
        "cfg-gated module bodies beyond their cfg( attribute line",
        "source references without use/path/extern-crate shapes",
    ]
}

/// Limitation bound to every zero-reference finding: absence of a textual
/// source reference is not proof of non-use.
pub fn declared_absence_limitation() -> &'static str {
    "source-reference absence is not proof of non-use: build scripts, proc \
     macros, generated code, doctests, and package assets are outside this \
     composition"
}

/// Zero-inspection receipt for a package the caller could not inspect at
/// all (#3909 negative control 8): analyzer success with zero inspected
/// packages is not clean, so the posture is forced to `InstrumentFailure`
/// and no findings are rendered.
pub fn empty_receipt(package_name: &str) -> UnusedDependencyReceiptV1 {
    UnusedDependencyReceiptV1 {
        schema_id: UnusedDependencyReceiptV1::CURRENT_SCHEMA_ID.to_string(),
        schema_version: UnusedDependencyReceiptV1::CURRENT_SCHEMA_VERSION,
        package_name: package_name.to_string(),
        configuration_id: String::new(),
        packages_inspected: 0,
        findings: Vec::new(),
        instrument_posture: UnusedDependencyInstrumentPostureV1::InstrumentFailure,
        analyzer_identity: UNUSED_DEPENDENCY_ANALYZER_IDENTITY.to_string(),
        claim_boundary: UNUSED_DEPENDENCY_CLAIM_BOUNDARY.to_string(),
    }
}

/// Render one receipt deterministically as pretty JSON.
pub fn render_unused_dependency_receipt_v1(
    receipt: &UnusedDependencyReceiptV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(receipt)
}

/// The evidence marker a finding carries when the caller supplied no source
/// inputs: the scan then cannot distinguish use from absence, so such rows
/// are incomplete-scan findings — review-visible, never enforcement-grade.
pub const INCOMPLETE_SCAN_EVIDENCE_MARKER: &str = "no_source_inputs_supplied";

/// Validation pass over a receipt, the prerequisite for any downstream
/// enforcement to trust it (#3909 PR D). Laws:
///
/// - identity: schema id/version must match the module constants, and the
///   package name must be non-empty;
/// - posture agreement: a `Complete` receipt may not carry
///   `InstrumentFailure` rows, and an `InstrumentFailure` receipt may not
///   carry any row that looks clean (only `InstrumentFailure` dispositions);
/// - limitations: every `Unsupported` finding carries non-empty limitations
///   (an unexplained unsupported row can never be review-visible).
///
/// Absence of these violations is exactly what lets PR D's selective guard
/// fail only on findings this composition can actually stand behind.
pub fn validate_receipt(receipt: &UnusedDependencyReceiptV1) -> Result<(), String> {
    if receipt.schema_id != UnusedDependencyReceiptV1::CURRENT_SCHEMA_ID {
        return Err(format!(
            "receipt schema id {} is not the module's {}",
            receipt.schema_id,
            UnusedDependencyReceiptV1::CURRENT_SCHEMA_ID
        ));
    }
    if receipt.schema_version != UnusedDependencyReceiptV1::CURRENT_SCHEMA_VERSION {
        return Err(format!(
            "receipt schema version {} is not the module's {}",
            receipt.schema_version,
            UnusedDependencyReceiptV1::CURRENT_SCHEMA_VERSION
        ));
    }
    if receipt.package_name.trim().is_empty() {
        return Err("receipt package name must be non-empty".to_string());
    }
    for finding in &receipt.findings {
        if finding.disposition == UnusedDependencyDispositionV1::InstrumentFailure
            && receipt.instrument_posture != UnusedDependencyInstrumentPostureV1::InstrumentFailure
        {
            return Err(format!(
                "receipt for {} is posture {:?} but carries an InstrumentFailure row; \
                 posture and rows must agree",
                receipt.package_name, receipt.instrument_posture
            ));
        }
        if receipt.instrument_posture == UnusedDependencyInstrumentPostureV1::InstrumentFailure
            && finding.disposition != UnusedDependencyDispositionV1::InstrumentFailure
        {
            return Err(format!(
                "InstrumentFailure receipt for {} carries a {:?} row; failed \
                 inspections must not render classified rows",
                receipt.package_name, finding.disposition
            ));
        }
        if finding.disposition == UnusedDependencyDispositionV1::Unsupported
            && finding.limitations.is_empty()
        {
            return Err(format!(
                "Unsupported finding for {} {} carries no limitations; an \
                 unexplained unsupported row can never be review-visible",
                finding.package_name, finding.manifest_row.dependency_name
            ));
        }
    }
    Ok(())
}

/// Whether a receipt's scan saw everything the composition needs to stand
/// behind absence-based findings: posture must be `Complete` and no finding
/// may carry the incomplete-scan evidence marker. Incomplete receipts are
/// review-visible in the family inventory but are excluded from the
/// selective no-new guard, because their absence-based rows would be noise.
pub fn receipt_scan_is_complete(receipt: &UnusedDependencyReceiptV1) -> bool {
    if receipt.instrument_posture != UnusedDependencyInstrumentPostureV1::Complete {
        return false;
    }
    !receipt.findings.iter().any(|finding| {
        finding
            .evidence
            .iter()
            .any(|entry| entry == INCOMPLETE_SCAN_EVIDENCE_MARKER)
    })
}

/// Inventory one package request into an advisory receipt.
///
/// Malformed manifests produce an `InstrumentFailure` receipt whose rows are
/// restricted to a failure description; they never render clean. See the
/// module documentation for the selection and update law.
pub fn inventory_unused_dependencies(
    request: &UnusedDependencyRequestV1,
) -> Result<UnusedDependencyReceiptV1, String> {
    analysis::inventory(request)
}

/// Inventory a batch of package requests; callers compose the per-package
/// receipts. One request never influences another package's evidence
/// (#3909: one product's use does not retain another package's dependency).
/// A caller-level composition failure for one package renders that package
/// as an `InstrumentFailure` receipt — packages are never silently dropped
/// from the batch.
pub fn inventory_packages(
    requests: &[UnusedDependencyRequestV1],
) -> Vec<UnusedDependencyReceiptV1> {
    requests
        .iter()
        .map(|request| {
            analysis::inventory(request)
                .unwrap_or_else(|failure| analysis::instrument_failure_receipt(request, failure))
        })
        .collect()
}
