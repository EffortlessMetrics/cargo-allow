//! Typed supported-feature-configuration matrix and proof contracts
//! (#3905 PR A).
//!
//! One checked-in source owns the finite set of package feature
//! configurations the repository deliberately supports, the closed
//! proof-depth vocabulary, and the request/receipt contracts later CI lanes
//! consume. Workflows must not maintain independent feature lists: they
//! derive rows from [`supported_feature_configuration_matrix`].
//!
//! The matrix law encoded here: rows are explicit and finite (a fabricated
//! full-powerset row is rejected), configuration IDs are unique and bound to
//! inventoried manifest features, default/minimal/optional/package/installed
//! proof stay distinct, and a green shallower run can never render as a
//! deeper proof. The inventory documents the actual manifests (features,
//! optional dependencies, cfg-gated surface) as of #3905 PR A; exact Cargo
//! metadata reconciliation lands with PR B (#2922 evidence) and the PR D
//! drift guard. This module reads no manifests, spawns no Cargo, and
//! performs no dependency resolution itself.

use super::package_candidate_v2::PackageCandidateFamilyV2;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const SUPPORTED_FEATURE_CONFIGURATION_V1_SCHEMA_ID: &str =
    "cargo-allow.supported-feature-configuration.v1";
pub const SUPPORTED_FEATURE_CONFIGURATION_V1_SCHEMA_VERSION: u32 = 1;

pub const FEATURE_CONFIGURATION_PROOF_REQUEST_V1_SCHEMA_ID: &str =
    "cargo-allow.feature-configuration-proof-request.v1";
pub const FEATURE_CONFIGURATION_PROOF_REQUEST_V1_SCHEMA_VERSION: u32 = 1;

pub const FEATURE_CONFIGURATION_PROOF_RECEIPT_V1_SCHEMA_ID: &str =
    "cargo-allow.feature-configuration-proof-receipt.v1";
pub const FEATURE_CONFIGURATION_PROOF_RECEIPT_V1_SCHEMA_VERSION: u32 = 1;

/// Stable identity of the checked-in matrix revision.
pub const FEATURE_CONFIGURATION_MATRIX_ID: &str = "CARGO-ALLOW-FEATURE-CONFIG-MATRIX-V1-0001";

/// Workspace MSRV the rows are proven against.
pub const WORKSPACE_RUST_VERSION: &str = "1.95";

/// Product identity of a matrix row. Closed to the three packages that
/// declare features today; the issue forbids manufacturing features so
/// every package can appear in the matrix (#3905).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureConfigurationProductV1 {
    AllowRust,
    AllowFiles,
    CargoProof,
}

impl FeatureConfigurationProductV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AllowRust => "allow-rust",
            Self::AllowFiles => "allow-files",
            Self::CargoProof => "cargo-proof",
        }
    }
}

/// Whether the row builds with or without the manifest default features.
/// Default, minimal, and explicit-feature postures stay distinct rows even
/// when their effective feature sets coincide (#3905 matrix law).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoDefaultFeaturesPostureV1 {
    /// Built with the manifest defaults as declared.
    WithDefaultFeatures,
    /// Built with `--no-default-features`; only explicit features apply.
    NoDefaultFeatures,
}

/// Platform class a row is proven on. Closed vocabulary; `HostWorkspaceMsrv`
/// means the workspace MSRV on the supported host platform class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureConfigurationTargetClassV1 {
    HostWorkspaceMsrv,
}

/// Closed proof-depth vocabulary (#3905). Ordered shallow to deep; a green
/// shallower run can never render as a deeper proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureConfigurationProofDepthV1 {
    CompileOnly,
    UnitAndDocTests,
    AllTargets,
    PackageCandidate,
    InstalledJourney,
    InteropJourney,
}

impl FeatureConfigurationProofDepthV1 {
    /// Depth rank, shallow to deep. The ordering is the law: proof at rank
    /// N never implies proof at a deeper rank.
    pub const fn rank(self) -> u8 {
        match self {
            Self::CompileOnly => 1,
            Self::UnitAndDocTests => 2,
            Self::AllTargets => 3,
            Self::PackageCandidate => 4,
            Self::InstalledJourney => 5,
            Self::InteropJourney => 6,
        }
    }

    /// Whether a run executed at `executed_depth` may render as proof of
    /// this depth. Only equal-or-deeper execution proves a claim.
    pub fn is_proven_by(self, executed_depth: Self) -> bool {
        executed_depth.rank() >= self.rank()
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CompileOnly => "compile_only",
            Self::UnitAndDocTests => "unit_and_doc_tests",
            Self::AllTargets => "all_targets",
            Self::PackageCandidate => "package_candidate",
            Self::InstalledJourney => "installed_journey",
            Self::InteropJourney => "interop_journey",
        }
    }
}

/// Support tier of a configuration, fixed by package-family authority:
/// cargo-allow 0.2 product crates are supported; cargo-proof remains an
/// independently experimental sibling (#3905; ADR-0002).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureConfigurationSupportTierV1 {
    Supported,
    Experimental,
}

/// Blocking/advisory posture of a row. Only ratified required rows become
/// blocking (PR D); experimental rows never block cargo-allow (#3905
/// negative control 12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureConfigurationEnforcementPostureV1 {
    Blocking,
    Advisory,
}

/// One optional dependency recorded by the inventory, with the feature that
/// selects it. Exact closure evidence is producer-owned (#2922); the
/// inventory records the declared selection edge only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CrateOptionalDependencyV1 {
    pub dependency_name: String,
    pub selected_by_feature: String,
}

/// One feature-to-feature implication recorded by the inventory (Cargo
/// `feature = ["other", ...]` edges).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeatureImplicationV1 {
    pub feature: String,
    pub enables: Vec<String>,
}

/// Inventoried manifest facts for one crate, checked in as the grounding
/// for matrix rows (#3905 PR A: inventory actual features, optional
/// dependencies, cfg gates, capabilities, and package docs).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CrateFeatureInventoryV1 {
    pub crate_name: String,
    pub manifest_path: String,
    pub package_version: String,
    /// Explicitly declared features, excluding the implicit `default` key.
    pub declared_features: Vec<String>,
    /// Features selected by the manifest `default` key (empty when the
    /// manifest declares no default).
    pub default_features: Vec<String>,
    pub feature_implies: Vec<FeatureImplicationV1>,
    pub optional_dependencies: Vec<CrateOptionalDependencyV1>,
    /// Public surface reachable only under a cfg feature gate.
    pub cfg_gated_surface: Vec<String>,
    /// Capability summary from the package docs at inventory time.
    pub capability_summary: String,
}

impl CrateFeatureInventoryV1 {
    /// All selectable feature names including the implicit `default`.
    pub fn selectable_features(&self) -> BTreeSet<String> {
        let mut names: BTreeSet<String> = self.declared_features.iter().cloned().collect();
        names.insert("default".to_string());
        names
    }

    /// Features directly enabled by selecting `feature`, per the inventory.
    pub fn implies(&self, feature: &str) -> Vec<String> {
        self.feature_implies
            .iter()
            .filter(|implication| implication.feature == feature)
            .flat_map(|implication| implication.enables.iter().cloned())
            .collect()
    }
}

/// Actual manifest inventory as of #3905 PR A. Only these three crates
/// declare features in the workspace; this function is the single
/// inventory source for matrix validation.
pub fn crate_feature_inventory() -> Vec<CrateFeatureInventoryV1> {
    vec![
        CrateFeatureInventoryV1 {
            crate_name: "allow-rust".to_string(),
            manifest_path: "crates/allow-rust/Cargo.toml".to_string(),
            package_version: "0.2.0-rc.1".to_string(),
            declared_features: vec!["syntax".to_string()],
            default_features: vec!["syntax".to_string()],
            feature_implies: vec![FeatureImplicationV1 {
                feature: "syntax".to_string(),
                enables: Vec::new(),
            }],
            optional_dependencies: vec![
                CrateOptionalDependencyV1 {
                    dependency_name: "tree-sitter".to_string(),
                    selected_by_feature: "syntax".to_string(),
                },
                CrateOptionalDependencyV1 {
                    dependency_name: "tree-sitter-rust".to_string(),
                    selected_by_feature: "syntax".to_string(),
                },
            ],
            cfg_gated_surface: vec![
                "syntax_tree: parse_rust_syntax, RustSyntaxTree, RustSyntaxContainer".to_string(),
                "syntax_facts: parser-backed syntax facts".to_string(),
                "syntax_coupling: scan_rust_source_coupling family".to_string(),
                "root_bound_scan_cache: RootBoundScanCacheStore, ScanCacheTargetDispositionV1"
                    .to_string(),
                "test_subjects: parser-backed inventory_rust_test_subjects and \
                 resolve_rust_test_selector"
                    .to_string(),
            ],
            capability_summary: "source-syntax Rust scanner: unsafe, panic-family, \
                 indexing/slicing, lint-suppression, and exact source-declared test \
                 surfaces; the syntax feature embeds the tree-sitter Rust parser, while \
                 --no-default-features keeps the data model without tree-sitter (#2821)"
                .to_string(),
        },
        CrateFeatureInventoryV1 {
            crate_name: "allow-files".to_string(),
            manifest_path: "crates/allow-files/Cargo.toml".to_string(),
            package_version: "0.2.0-rc.1".to_string(),
            declared_features: vec!["changie".to_string()],
            // The manifest declares no default feature at all: the default
            // build equals the no-feature build (#3905 inventory note).
            default_features: Vec::new(),
            feature_implies: vec![FeatureImplicationV1 {
                feature: "changie".to_string(),
                enables: Vec::new(),
            }],
            optional_dependencies: vec![CrateOptionalDependencyV1 {
                dependency_name: "yaml-rust2".to_string(),
                selected_by_feature: "changie".to_string(),
            }],
            cfg_gated_surface: vec![
                "changie: Changie 1.25 static parse slice (#3588)".to_string(),
                "changie_lint: static authoring contract validation (#3589)".to_string(),
            ],
            capability_summary: "non-Rust, generated, and workflow file classification; the \
                 changie feature embeds the static Changie source sensor (parse plus lint) \
                 without Changie execution, Go, release mutation, or editor hosting (#3587)"
                .to_string(),
        },
        CrateFeatureInventoryV1 {
            crate_name: "cargo-proof".to_string(),
            manifest_path: "crates/cargo-proof/Cargo.toml".to_string(),
            package_version: "0.1.0".to_string(),
            declared_features: vec![
                "provider-cargo-allow".to_string(),
                "provider-hawk".to_string(),
                "provider-ripr".to_string(),
                "all-providers".to_string(),
            ],
            default_features: Vec::new(),
            feature_implies: vec![FeatureImplicationV1 {
                feature: "all-providers".to_string(),
                enables: vec![
                    "provider-cargo-allow".to_string(),
                    "provider-hawk".to_string(),
                    "provider-ripr".to_string(),
                ],
            }],
            // Provider features select no optional dependencies: they are
            // pure cfg gates over in-crate provider modules, so enabling a
            // feature proves code/package inclusion only.
            optional_dependencies: Vec::new(),
            cfg_gated_surface: vec![
                "providers::registry: feature-selected StaticProviderRegistryV1 construction"
                    .to_string(),
                "providers::{cargo_allow, rippr, hawk}: provider modules and fixtures".to_string(),
                "receipt_status: provider-aware status rendering".to_string(),
            ],
            capability_summary: "exact-snapshot evidence orchestration shell; provider features \
                 select built-in provider code and fixtures without proving the external \
                 provider is installed, reachable, or semantically current"
                .to_string(),
        },
    ]
}

/// One finite supported feature configuration row (#3905). Each field is a
/// binding: identity, selection, platform, depth, posture, capabilities,
/// exclusions, and claim boundary travel together and validate together.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SupportedFeatureConfigurationV1 {
    pub configuration_id: String,
    pub product: FeatureConfigurationProductV1,
    pub product_family: PackageCandidateFamilyV2,
    pub root_package_name: String,
    pub root_package_version: String,
    /// Default features the row keeps on. Must equal the manifest default
    /// set exactly when the posture keeps defaults.
    pub default_features_selected: Vec<String>,
    /// Features passed explicitly (`--features ...`).
    pub explicit_features: Vec<String>,
    pub no_default_features: NoDefaultFeaturesPostureV1,
    pub target_class: FeatureConfigurationTargetClassV1,
    pub rust_version: String,
    pub proof_depth: FeatureConfigurationProofDepthV1,
    pub support_tier: FeatureConfigurationSupportTierV1,
    pub enforcement: FeatureConfigurationEnforcementPostureV1,
    pub expected_capabilities: Vec<String>,
    pub expected_assets: Vec<String>,
    pub known_exclusions: Vec<String>,
    pub claim_boundary: String,
}

impl SupportedFeatureConfigurationV1 {
    /// Canonical `name:version` identity the packaged manifest must carry.
    pub fn manifest_identity(&self) -> String {
        format!("{}:{}", self.root_package_name, self.root_package_version)
    }
}

/// A feature combination Cargo can express that the repository deliberately
/// does not support. Recorded so "unsupported" is explicit rather than
/// implicit (#3905 matrix law).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeatureConfigurationNonSelectionV1 {
    pub package_name: String,
    pub selected_features: Vec<String>,
    pub reason: String,
}

/// The checked-in supported-configuration set: finite rows plus explicit
/// non-selections under one claim boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SupportedFeatureConfigurationMatrixV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub matrix_id: String,
    pub rows: Vec<SupportedFeatureConfigurationV1>,
    pub explicit_non_selections: Vec<FeatureConfigurationNonSelectionV1>,
    pub claim_boundary: String,
}

/// The checked-in matrix. One source owns configuration IDs and selected
/// features; workflows derive from this function and never maintain
/// independent lists (#3905 matrix law).
pub fn supported_feature_configuration_matrix() -> SupportedFeatureConfigurationMatrixV1 {
    SupportedFeatureConfigurationMatrixV1 {
        schema_id: SUPPORTED_FEATURE_CONFIGURATION_V1_SCHEMA_ID.to_string(),
        schema_version: SUPPORTED_FEATURE_CONFIGURATION_V1_SCHEMA_VERSION,
        matrix_id: FEATURE_CONFIGURATION_MATRIX_ID.to_string(),
        rows: supported_feature_configuration_rows(),
        explicit_non_selections: explicit_non_selections(),
        claim_boundary: "exact finite proof for the package feature configurations the \
             repository deliberately supports, at explicitly stated proof depths and product \
             postures; no full powerset, no provider availability, and no semantic correctness \
             beyond the selected fixtures and journeys"
            .to_string(),
    }
}

/// The ten finite configuration rows (#3905 initial matrix), reconciled with
/// the actual manifests in [`crate_feature_inventory`].
pub fn supported_feature_configuration_rows() -> Vec<SupportedFeatureConfigurationV1> {
    let default_exclusions = vec![
        "no macro-expanded, type-aware, MIR-level, or build-aware behavior".to_string(),
        "no Cargo metadata, rustc, Clippy, or build-script execution".to_string(),
    ];
    let minimal_exclusions = vec![
        "tree-sitter and tree-sitter-rust must be absent from the resolved normal closure"
            .to_string(),
        "parser-dependent public APIs must not compile or expose in this build".to_string(),
    ];
    vec![
        SupportedFeatureConfigurationV1 {
            configuration_id: "allow-rust.default".to_string(),
            product: FeatureConfigurationProductV1::AllowRust,
            product_family: PackageCandidateFamilyV2::CargoAllow02,
            root_package_name: "allow-rust".to_string(),
            root_package_version: "0.2.0-rc.1".to_string(),
            default_features_selected: vec!["syntax".to_string()],
            explicit_features: Vec::new(),
            no_default_features: NoDefaultFeaturesPostureV1::WithDefaultFeatures,
            target_class: FeatureConfigurationTargetClassV1::HostWorkspaceMsrv,
            rust_version: WORKSPACE_RUST_VERSION.to_string(),
            proof_depth: FeatureConfigurationProofDepthV1::AllTargets,
            support_tier: FeatureConfigurationSupportTierV1::Supported,
            enforcement: FeatureConfigurationEnforcementPostureV1::Blocking,
            expected_capabilities: vec![
                "syntax-visible finding families: unsafe, panic-family, indexing/slicing, \
                 lint suppression"
                    .to_string(),
                "exact source-declared test inventory".to_string(),
                "tree-sitter Rust parser embedded via the syntax feature".to_string(),
            ],
            expected_assets: vec!["crates/allow-rust/README.md".to_string()],
            known_exclusions: default_exclusions.clone(),
            claim_boundary: "current normal scanner surface at workspace MSRV; source-text \
                 scanning only"
                .to_string(),
        },
        SupportedFeatureConfigurationV1 {
            configuration_id: "allow-rust.minimal-model".to_string(),
            product: FeatureConfigurationProductV1::AllowRust,
            product_family: PackageCandidateFamilyV2::CargoAllow02,
            root_package_name: "allow-rust".to_string(),
            root_package_version: "0.2.0-rc.1".to_string(),
            default_features_selected: Vec::new(),
            explicit_features: Vec::new(),
            no_default_features: NoDefaultFeaturesPostureV1::NoDefaultFeatures,
            target_class: FeatureConfigurationTargetClassV1::HostWorkspaceMsrv,
            rust_version: WORKSPACE_RUST_VERSION.to_string(),
            proof_depth: FeatureConfigurationProofDepthV1::CompileOnly,
            support_tier: FeatureConfigurationSupportTierV1::Supported,
            enforcement: FeatureConfigurationEnforcementPostureV1::Advisory,
            expected_capabilities: vec![
                "data-model surface: finding models, scan results, scan-cache and \
                 test-subject model types"
                    .to_string(),
                "no tree-sitter dependency selected".to_string(),
            ],
            expected_assets: vec!["crates/allow-rust/README.md".to_string()],
            known_exclusions: minimal_exclusions,
            claim_boundary: "minimal data-model build proven at compile level only; the \
                 syntax capability is deliberately absent"
                .to_string(),
        },
        SupportedFeatureConfigurationV1 {
            configuration_id: "allow-rust.syntax-explicit".to_string(),
            product: FeatureConfigurationProductV1::AllowRust,
            product_family: PackageCandidateFamilyV2::CargoAllow02,
            root_package_name: "allow-rust".to_string(),
            root_package_version: "0.2.0-rc.1".to_string(),
            default_features_selected: Vec::new(),
            explicit_features: vec!["syntax".to_string()],
            no_default_features: NoDefaultFeaturesPostureV1::NoDefaultFeatures,
            target_class: FeatureConfigurationTargetClassV1::HostWorkspaceMsrv,
            rust_version: WORKSPACE_RUST_VERSION.to_string(),
            proof_depth: FeatureConfigurationProofDepthV1::AllTargets,
            support_tier: FeatureConfigurationSupportTierV1::Supported,
            enforcement: FeatureConfigurationEnforcementPostureV1::Advisory,
            expected_capabilities: vec![
                "selected syntax capability equivalent to default".to_string(),
                "tree-sitter Rust parser embedded without default features".to_string(),
            ],
            expected_assets: vec!["crates/allow-rust/README.md".to_string()],
            known_exclusions: vec![
                "no macro-expanded, type-aware, MIR-level, or build-aware behavior".to_string(),
            ],
            claim_boundary: "explicitly selected syntax without default features; the \
                 selected syntax capability matches the default row, not a deeper surface"
                .to_string(),
        },
        SupportedFeatureConfigurationV1 {
            configuration_id: "allow-files.default".to_string(),
            product: FeatureConfigurationProductV1::AllowFiles,
            product_family: PackageCandidateFamilyV2::CargoAllow02,
            root_package_name: "allow-files".to_string(),
            root_package_version: "0.2.0-rc.1".to_string(),
            // The manifest declares no default feature: the default row is
            // legitimately a zero-feature selection.
            default_features_selected: Vec::new(),
            explicit_features: Vec::new(),
            no_default_features: NoDefaultFeaturesPostureV1::WithDefaultFeatures,
            target_class: FeatureConfigurationTargetClassV1::HostWorkspaceMsrv,
            rust_version: WORKSPACE_RUST_VERSION.to_string(),
            proof_depth: FeatureConfigurationProofDepthV1::AllTargets,
            support_tier: FeatureConfigurationSupportTierV1::Supported,
            enforcement: FeatureConfigurationEnforcementPostureV1::Blocking,
            expected_capabilities: vec![
                "non-Rust, generated, and workflow classification".to_string(),
                "FileFamilyRule classification with explicit ambiguity findings".to_string(),
                "workflow and dependency-surface projections without execution".to_string(),
            ],
            expected_assets: vec!["crates/allow-files/README.md".to_string()],
            known_exclusions: default_exclusions.clone(),
            claim_boundary: "core non-Rust/generated/workflow classification; no yaml-rust2 \
                 dependency and no Changie sensor surface in this build"
                .to_string(),
        },
        SupportedFeatureConfigurationV1 {
            configuration_id: "allow-files.changie".to_string(),
            product: FeatureConfigurationProductV1::AllowFiles,
            product_family: PackageCandidateFamilyV2::CargoAllow02,
            root_package_name: "allow-files".to_string(),
            root_package_version: "0.2.0-rc.1".to_string(),
            default_features_selected: Vec::new(),
            explicit_features: vec!["changie".to_string()],
            no_default_features: NoDefaultFeaturesPostureV1::WithDefaultFeatures,
            target_class: FeatureConfigurationTargetClassV1::HostWorkspaceMsrv,
            rust_version: WORKSPACE_RUST_VERSION.to_string(),
            proof_depth: FeatureConfigurationProofDepthV1::AllTargets,
            support_tier: FeatureConfigurationSupportTierV1::Supported,
            enforcement: FeatureConfigurationEnforcementPostureV1::Advisory,
            expected_capabilities: vec![
                "Changie 1.25 static parse slice with source ranges and duplicate-key order"
                    .to_string(),
                "static authoring contract lint (changie_lint)".to_string(),
                "yaml-rust2 parser embedded".to_string(),
            ],
            expected_assets: vec![
                "crates/allow-files/README.md".to_string(),
                "CHANGIE_COMPATIBILITY_GENERATION = \"1.25\"".to_string(),
            ],
            known_exclusions: vec![
                "no Changie execution, no Go, no release mutation, no editor hosting".to_string(),
                "a clean parse says static contract satisfied only; the pinned upstream \
                 binary owns rendering"
                    .to_string(),
            ],
            claim_boundary: "base classification plus the static Changie sensor; enabling \
                 the feature proves parser inclusion and selected fixtures, never execution"
                .to_string(),
        },
        SupportedFeatureConfigurationV1 {
            configuration_id: "cargo-proof.default".to_string(),
            product: FeatureConfigurationProductV1::CargoProof,
            product_family: PackageCandidateFamilyV2::Shared01,
            root_package_name: "cargo-proof".to_string(),
            root_package_version: "0.1.0".to_string(),
            default_features_selected: Vec::new(),
            explicit_features: Vec::new(),
            no_default_features: NoDefaultFeaturesPostureV1::WithDefaultFeatures,
            target_class: FeatureConfigurationTargetClassV1::HostWorkspaceMsrv,
            rust_version: WORKSPACE_RUST_VERSION.to_string(),
            proof_depth: FeatureConfigurationProofDepthV1::AllTargets,
            support_tier: FeatureConfigurationSupportTierV1::Experimental,
            enforcement: FeatureConfigurationEnforcementPostureV1::Advisory,
            expected_capabilities: vec![
                "product shell, renderer framework, and plan/dry-run wiring".to_string(),
                "empty provider registry: no provider code selected".to_string(),
            ],
            expected_assets: vec!["crates/cargo-proof/README.md".to_string()],
            known_exclusions: vec![
                "no built-in provider module selected; provider absence is the declared \
                 posture, not a failure"
                    .to_string(),
            ],
            claim_boundary: "default-empty provider set; shell identity and orchestration \
                 wiring only, experimental and advisory"
                .to_string(),
        },
        provider_row("cargo-proof.provider-cargo-allow", "provider-cargo-allow"),
        provider_row("cargo-proof.provider-hawk", "provider-hawk"),
        provider_row("cargo-proof.provider-ripr", "provider-ripr"),
        SupportedFeatureConfigurationV1 {
            configuration_id: "cargo-proof.all-providers".to_string(),
            product: FeatureConfigurationProductV1::CargoProof,
            product_family: PackageCandidateFamilyV2::Shared01,
            root_package_name: "cargo-proof".to_string(),
            root_package_version: "0.1.0".to_string(),
            default_features_selected: Vec::new(),
            explicit_features: vec!["all-providers".to_string()],
            no_default_features: NoDefaultFeaturesPostureV1::WithDefaultFeatures,
            target_class: FeatureConfigurationTargetClassV1::HostWorkspaceMsrv,
            rust_version: WORKSPACE_RUST_VERSION.to_string(),
            proof_depth: FeatureConfigurationProofDepthV1::AllTargets,
            support_tier: FeatureConfigurationSupportTierV1::Experimental,
            enforcement: FeatureConfigurationEnforcementPostureV1::Advisory,
            expected_capabilities: vec![
                "all three built-in provider modules selected in one build".to_string(),
                "full selected-union qualification row for routed CI".to_string(),
            ],
            expected_assets: vec!["crates/cargo-proof/README.md".to_string()],
            known_exclusions: vec![
                "selecting every provider proves code/package inclusion only; it does not \
                 prove any external provider is installed or semantically current"
                    .to_string(),
            ],
            claim_boundary: "all built-in providers compiled and fixture-proven together; \
                 provider availability and semantics remain external, experimental and advisory"
                .to_string(),
        },
    ]
}

/// Provider rows share one posture template: code/package inclusion plus
/// selected fixtures, never provider availability (#3905).
fn provider_row(configuration_id: &str, feature: &str) -> SupportedFeatureConfigurationV1 {
    SupportedFeatureConfigurationV1 {
        configuration_id: configuration_id.to_string(),
        product: FeatureConfigurationProductV1::CargoProof,
        product_family: PackageCandidateFamilyV2::Shared01,
        root_package_name: "cargo-proof".to_string(),
        root_package_version: "0.1.0".to_string(),
        default_features_selected: Vec::new(),
        explicit_features: vec![feature.to_string()],
        no_default_features: NoDefaultFeaturesPostureV1::WithDefaultFeatures,
        target_class: FeatureConfigurationTargetClassV1::HostWorkspaceMsrv,
        rust_version: WORKSPACE_RUST_VERSION.to_string(),
        proof_depth: FeatureConfigurationProofDepthV1::AllTargets,
        support_tier: FeatureConfigurationSupportTierV1::Experimental,
        enforcement: FeatureConfigurationEnforcementPostureV1::Advisory,
        expected_capabilities: vec![
            format!("{feature} provider module and fixtures included in the build"),
            format!("{feature} provider registered in the feature-selected registry"),
        ],
        expected_assets: vec!["crates/cargo-proof/README.md".to_string()],
        known_exclusions: vec![
            "provider-unavailable/unsupported posture is preserved: enabling the feature \
             does not install, reach, or semantically qualify the external provider"
                .to_string(),
        ],
        claim_boundary: format!(
            "{feature} proves code/package inclusion and selected fixtures only; the \
             external provider stays unproven, experimental and advisory"
        ),
    }
}

/// Feature combinations Cargo can express that are deliberately unsupported.
/// They are recorded so "no implicit powerset" is checkable (#3905).
fn explicit_non_selections() -> Vec<FeatureConfigurationNonSelectionV1> {
    let reason = "expressible pair combination deliberately unselected; not a supported \
         configuration and must not be scheduled as one";
    vec![
        FeatureConfigurationNonSelectionV1 {
            package_name: "cargo-proof".to_string(),
            selected_features: vec![
                "provider-cargo-allow".to_string(),
                "provider-hawk".to_string(),
            ],
            reason: reason.to_string(),
        },
        FeatureConfigurationNonSelectionV1 {
            package_name: "cargo-proof".to_string(),
            selected_features: vec![
                "provider-cargo-allow".to_string(),
                "provider-ripr".to_string(),
            ],
            reason: reason.to_string(),
        },
        FeatureConfigurationNonSelectionV1 {
            package_name: "cargo-proof".to_string(),
            selected_features: vec!["provider-hawk".to_string(), "provider-ripr".to_string()],
            reason: reason.to_string(),
        },
    ]
}

/// Look up one row by configuration ID from the checked-in matrix.
pub fn row_for_supported_feature_configuration(
    configuration_id: &str,
) -> Option<SupportedFeatureConfigurationV1> {
    supported_feature_configuration_rows()
        .into_iter()
        .find(|row| row.configuration_id == configuration_id)
}

/// Effective feature set of a row: selected defaults (when the posture keeps
/// them) plus explicit features, transitively closed over the inventory's
/// implication edges. Two rows with the same (package, posture, effective
/// set) are the same configuration regardless of their IDs.
pub fn effective_feature_set(
    row: &SupportedFeatureConfigurationV1,
    inventory: &[CrateFeatureInventoryV1],
) -> BTreeSet<String> {
    let entry = inventory
        .iter()
        .find(|candidate| candidate.crate_name == row.root_package_name);
    let mut selected: BTreeSet<String> = BTreeSet::new();
    if row.no_default_features == NoDefaultFeaturesPostureV1::WithDefaultFeatures {
        for feature in &row.default_features_selected {
            selected.insert(feature.clone());
        }
    }
    for feature in &row.explicit_features {
        selected.insert(feature.clone());
    }
    // Fixpoint expansion of implied features; the set only grows and is
    // bounded by the declared feature names, so this terminates.
    let mut grew = true;
    while grew {
        grew = false;
        let current: Vec<String> = selected.iter().cloned().collect();
        for feature in current {
            let Some(entry) = entry else {
                continue;
            };
            for implied in entry.implies(&feature) {
                if selected.insert(implied) {
                    grew = true;
                }
            }
        }
    }
    selected
}

/// Current inventory gaps reported by #3905 PR A: facts that are declared
/// here but not yet proven, without changing CI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeatureConfigurationGapV1 {
    /// Configuration the gap belongs to, when row-scoped.
    pub configuration_id: Option<String>,
    pub gap: String,
    /// Lane expected to close the gap.
    pub owner: String,
}

/// Current gaps as of #3905 PR A. Declared depths are targets, not executed
/// proofs; receipts land with PR B/C and CI consumption with PR D.
pub fn current_feature_configuration_gaps() -> Vec<FeatureConfigurationGapV1> {
    vec![
        FeatureConfigurationGapV1 {
            configuration_id: None,
            gap: "no proof receipts are checked in yet; every row's proof depth is a \
                 declared target, not an executed proof"
                .to_string(),
            owner: "#3905 PR B/C".to_string(),
        },
        FeatureConfigurationGapV1 {
            configuration_id: Some("allow-rust.minimal-model".to_string()),
            gap: "compile-only target; the parser-dependent API non-exposure list is \
                 declared but not yet asserted by a proven receipt or closure negative"
                .to_string(),
            owner: "#3905 PR B".to_string(),
        },
        FeatureConfigurationGapV1 {
            configuration_id: Some("allow-files.changie".to_string()),
            gap: "capability and asset projections use the configuration ID, but \
                 packaged-manifest agreement and the yaml-rust2 closure negative are not \
                 yet reconciled with exact cargo metadata evidence (#2922)"
                .to_string(),
            owner: "#3905 PR B".to_string(),
        },
        FeatureConfigurationGapV1 {
            configuration_id: Some("cargo-proof.all-providers".to_string()),
            gap: "provider rows bind the provider-unavailable posture through the row \
                 claim boundary; no receipt yet proves a provider row without fabricating \
                 provider availability"
                .to_string(),
            owner: "#3905 PR C".to_string(),
        },
        FeatureConfigurationGapV1 {
            configuration_id: None,
            gap: "matrix-to-manifest reconciliation (exact versions, feature sets, \
                 optional-dependency closure) is manually maintained in the checked-in \
                 inventory until the drift guard binds manifests to this source"
                .to_string(),
            owner: "#3905 PR D".to_string(),
        },
        FeatureConfigurationGapV1 {
            configuration_id: None,
            gap: "no CI lane consumes the matrix yet; workflows do not derive jobs from \
                 configuration IDs (deliberate in PR A)"
                .to_string(),
            owner: "#3905 PR D".to_string(),
        },
    ]
}

/// Closed validation vocabulary for the matrix contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureConfigurationMatrixResultV1 {
    Complete,
    MalformedRow,
    UnknownFeature,
    DuplicateConfiguration,
    EmptyRow,
    PowersetOverreach,
    StaleInventory,
    UncoveredFeature,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureConfigurationMatrixValidationV1 {
    pub result: FeatureConfigurationMatrixResultV1,
    pub gaps: Vec<String>,
}

/// Closed validation vocabulary for the proof-request contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureConfigurationRequestResultV1 {
    Complete,
    MalformedRequest,
    UnknownConfiguration,
    DepthPostureConflict,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureConfigurationRequestValidationV1 {
    pub result: FeatureConfigurationRequestResultV1,
    pub gaps: Vec<String>,
}

/// Closed validation vocabulary for the proof-receipt contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureConfigurationReceiptResultV1 {
    Complete,
    MalformedReceipt,
    UnknownConfiguration,
    DepthPostureConflict,
    UnexecutedProof,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureConfigurationReceiptValidationV1 {
    pub result: FeatureConfigurationReceiptResultV1,
    pub gaps: Vec<String>,
}

/// Status of one executed proof command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureConfigurationCommandStatusV1 {
    Green,
    Red,
    Skipped,
    InstrumentFailure,
}

/// Closed outcome vocabulary for a receipt. `Proven` is only admissible at
/// the executed depth; shallower greens stay `PartialDepth` and can never
/// render as deeper proof (#3905).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureConfigurationProofOutcomeV1 {
    Proven,
    PartialDepth,
    Failed,
    Skipped,
    InstrumentFailure,
}

/// One request to prove a matrix row at a depth. Requests inherit the row's
/// identity and claim boundary from the checked-in matrix: an independent
/// claim text is a validation failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeatureConfigurationProofRequestV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub configuration_id: String,
    pub root_package_name: String,
    pub root_package_version: String,
    pub requested_depth: FeatureConfigurationProofDepthV1,
    /// Exact command surface generated from the row, beginning with `cargo`.
    pub cargo_command_tokens: Vec<String>,
    /// Lane or issue identity demanding the proof.
    pub requested_by: String,
    pub claim_boundary: String,
}

/// One executed command inside a receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeatureConfigurationCommandResultV1 {
    pub command_tokens: Vec<String>,
    pub status: FeatureConfigurationCommandStatusV1,
    pub note: String,
}

/// The executed proof record for one configuration. Validation enforces the
/// honesty law: empty, skipped, failed, stale, and shallower records cannot
/// render as proven, and no receipt may exceed its row's declared depth.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeatureConfigurationProofReceiptV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub configuration_id: String,
    pub root_package_name: String,
    pub root_package_version: String,
    pub requested_depth: FeatureConfigurationProofDepthV1,
    pub executed_depth: FeatureConfigurationProofDepthV1,
    pub outcome: FeatureConfigurationProofOutcomeV1,
    pub command_results: Vec<FeatureConfigurationCommandResultV1>,
    /// Retained evidence binding; required for journey-depth proof.
    pub evidence_digest: Option<String>,
    pub gaps: Vec<String>,
    pub claim_boundary: String,
}

/// Render the checked-in matrix deterministically. Serde's declaration
/// order is canonical.
pub fn render_supported_feature_configuration_matrix_v1(
    matrix: &SupportedFeatureConfigurationMatrixV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(matrix)
}

pub fn render_supported_feature_configuration_matrix_v1_bytes(
    matrix: &SupportedFeatureConfigurationMatrixV1,
) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(matrix)
}

pub fn render_feature_configuration_proof_request_v1(
    request: &FeatureConfigurationProofRequestV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(request)
}

pub fn render_feature_configuration_proof_receipt_v1(
    receipt: &FeatureConfigurationProofReceiptV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(receipt)
}

pub fn render_feature_configuration_proof_receipt_v1_bytes(
    receipt: &FeatureConfigurationProofReceiptV1,
) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(receipt)
}

/// Validate a matrix against the inventory. Laws: unique configuration IDs,
/// unique (package, posture, effective selection) per package, every named
/// feature declared by the inventoried manifest, posture agreement with the
/// manifest defaults, non-empty capability and claim bindings, packages and
/// versions matching the inventory, and every declared feature covered by a
/// row or an implication. Re-declaring an existing selection under a new ID
/// is rejected as a powerset overreach.
pub fn validate_supported_feature_configuration_matrix_v1(
    matrix: &SupportedFeatureConfigurationMatrixV1,
    inventory: &[CrateFeatureInventoryV1],
) -> FeatureConfigurationMatrixValidationV1 {
    let mut gaps: Vec<String> = Vec::new();
    let mut classes: Vec<FeatureConfigurationMatrixResultV1> = Vec::new();
    let schema_current = matrix.schema_id == SUPPORTED_FEATURE_CONFIGURATION_V1_SCHEMA_ID
        && matrix.schema_version == SUPPORTED_FEATURE_CONFIGURATION_V1_SCHEMA_VERSION;
    if !schema_current {
        classes.push(FeatureConfigurationMatrixResultV1::MalformedRow);
        gaps.push("matrix uses a non-current schema generation".to_string());
    }
    if matrix.rows.is_empty() {
        classes.push(FeatureConfigurationMatrixResultV1::EmptyRow);
        gaps.push("matrix carries no rows".to_string());
    }
    if matrix.matrix_id.trim().is_empty() {
        classes.push(FeatureConfigurationMatrixResultV1::MalformedRow);
        gaps.push("matrix_id is missing".to_string());
    }

    let mut configuration_ids: BTreeSet<String> = BTreeSet::new();
    let mut selections: BTreeSet<String> = BTreeSet::new();
    for (index, row) in matrix.rows.iter().enumerate() {
        let label = format!("rows[{index}]");
        if row.configuration_id.trim().is_empty() {
            classes.push(FeatureConfigurationMatrixResultV1::MalformedRow);
            gaps.push(format!("{label} configuration_id is missing"));
        }
        let package_inventory = inventory
            .iter()
            .find(|entry| entry.crate_name == row.root_package_name);
        let Some(entry) = package_inventory else {
            classes.push(FeatureConfigurationMatrixResultV1::StaleInventory);
            gaps.push(format!(
                "{label} root package {} is absent from the inventory",
                row.root_package_name
            ));
            continue;
        };
        if row.root_package_version != entry.package_version {
            classes.push(FeatureConfigurationMatrixResultV1::StaleInventory);
            gaps.push(format!(
                "{label} pins {} but the inventory records {}",
                row.root_package_version, entry.package_version
            ));
        }
        let expected_prefix = format!("{}.", row.root_package_name);
        if !row.configuration_id.starts_with(&expected_prefix) {
            classes.push(FeatureConfigurationMatrixResultV1::MalformedRow);
            gaps.push(format!(
                "{label} configuration_id must be prefixed by its package name"
            ));
        }
        if row.rust_version.trim().is_empty() {
            classes.push(FeatureConfigurationMatrixResultV1::MalformedRow);
            gaps.push(format!("{label} rust_version is missing"));
        }
        // Posture law: with-defaults rows carry exactly the manifest
        // defaults; no-default rows carry none. Naming `default` explicitly
        // is a posture bypass and is rejected.
        if row
            .explicit_features
            .iter()
            .any(|feature| feature == "default")
        {
            classes.push(FeatureConfigurationMatrixResultV1::MalformedRow);
            gaps.push(format!(
                "{label} names the default feature explicitly; posture owns defaults"
            ));
        }
        match row.no_default_features {
            NoDefaultFeaturesPostureV1::WithDefaultFeatures => {
                let selected: BTreeSet<&String> = row.default_features_selected.iter().collect();
                let manifest: BTreeSet<&String> = entry.default_features.iter().collect();
                if selected != manifest {
                    classes.push(FeatureConfigurationMatrixResultV1::MalformedRow);
                    gaps.push(format!(
                        "{label} default selection disagrees with the manifest default set"
                    ));
                }
            }
            NoDefaultFeaturesPostureV1::NoDefaultFeatures => {
                if !row.default_features_selected.is_empty() {
                    classes.push(FeatureConfigurationMatrixResultV1::MalformedRow);
                    gaps.push(format!(
                        "{label} no-default posture carries selected defaults"
                    ));
                }
            }
        }
        // Unknown features: every named selection must exist on the
        // inventoried manifest (including the implicit `default`).
        let selectable = entry.selectable_features();
        for feature in row
            .default_features_selected
            .iter()
            .chain(row.explicit_features.iter())
        {
            if !selectable.contains(feature) {
                classes.push(FeatureConfigurationMatrixResultV1::UnknownFeature);
                gaps.push(format!(
                    "{label} selects feature {feature} which the manifest does not declare"
                ));
            }
        }
        // Empty-row law: a row must bind capabilities, assets, and a claim
        // boundary; a row that selects nothing to prove is rejected.
        if row.expected_capabilities.is_empty()
            || row.expected_assets.is_empty()
            || row.claim_boundary.trim().is_empty()
        {
            classes.push(FeatureConfigurationMatrixResultV1::EmptyRow);
            gaps.push(format!(
                "{label} lacks capabilities, assets, or a claim boundary"
            ));
        }
        for (field, values) in [
            ("expected_capabilities", &row.expected_capabilities),
            ("expected_assets", &row.expected_assets),
            ("known_exclusions", &row.known_exclusions),
        ] {
            if values.iter().any(|value| value.trim().is_empty()) {
                classes.push(FeatureConfigurationMatrixResultV1::MalformedRow);
                gaps.push(format!("{label} carries a blank {field} entry"));
            }
        }
        // Duplicate configuration IDs.
        if !configuration_ids.insert(row.configuration_id.clone()) {
            classes.push(FeatureConfigurationMatrixResultV1::DuplicateConfiguration);
            gaps.push(format!(
                "{label} configuration_id {} is duplicated",
                row.configuration_id
            ));
        }
        // Powerset law: the same (package, posture, effective selection)
        // may be owned by exactly one configuration ID. A fabricated row
        // that re-declares an owned selection under a new ID is rejected.
        let effective = effective_feature_set(row, inventory);
        let selection_key = format!(
            "{}|{:?}|{}",
            row.root_package_name,
            row.no_default_features,
            effective.iter().cloned().collect::<Vec<_>>().join(","),
        );
        if !selections.insert(selection_key) {
            classes.push(FeatureConfigurationMatrixResultV1::PowersetOverreach);
            gaps.push(format!(
                "{label} re-declares a selection already owned by another configuration; \
                 a fabricated powerset row is not a supported configuration"
            ));
        }
    }

    // Coverage law: every declared feature of every inventoried crate must
    // be reachable from some row's effective set. A new manifest feature
    // without a row fails here instead of silently joining the powerset.
    for entry in inventory {
        let mut covered: BTreeSet<String> = BTreeSet::new();
        for row in matrix
            .rows
            .iter()
            .filter(|row| row.root_package_name == entry.crate_name)
        {
            for feature in effective_feature_set(row, inventory) {
                covered.insert(feature.clone());
                for implied in entry.implies(&feature) {
                    covered.insert(implied);
                }
            }
        }
        for feature in &entry.declared_features {
            if !covered.contains(feature) {
                classes.push(FeatureConfigurationMatrixResultV1::UncoveredFeature);
                gaps.push(format!(
                    "inventory feature {} of {} is not selected or implied by any row",
                    feature, entry.crate_name
                ));
            }
        }
    }

    // Explicit non-selections must reference real packages and features.
    let mut non_selection_keys: BTreeSet<String> = BTreeSet::new();
    for non_selection in &matrix.explicit_non_selections {
        let package_inventory = inventory
            .iter()
            .find(|entry| entry.crate_name == non_selection.package_name);
        let Some(entry) = package_inventory else {
            classes.push(FeatureConfigurationMatrixResultV1::StaleInventory);
            gaps.push(format!(
                "explicit non-selection names package {} absent from the inventory",
                non_selection.package_name
            ));
            continue;
        };
        let selectable = entry.selectable_features();
        for feature in &non_selection.selected_features {
            if !selectable.contains(feature) {
                classes.push(FeatureConfigurationMatrixResultV1::UnknownFeature);
                gaps.push(format!(
                    "explicit non-selection names undeclared feature {feature}"
                ));
            }
        }
        if non_selection.reason.trim().is_empty() {
            classes.push(FeatureConfigurationMatrixResultV1::EmptyRow);
            gaps.push("explicit non-selection lacks a reason".to_string());
        }
        let key = format!(
            "{}|{}",
            non_selection.package_name,
            non_selection.selected_features.to_vec().join(","),
        );
        if !non_selection_keys.insert(key) {
            classes.push(FeatureConfigurationMatrixResultV1::DuplicateConfiguration);
            gaps.push("explicit non-selection is duplicated".to_string());
        }
    }

    if gaps.is_empty() {
        FeatureConfigurationMatrixValidationV1 {
            result: FeatureConfigurationMatrixResultV1::Complete,
            gaps,
        }
    } else {
        FeatureConfigurationMatrixValidationV1 {
            result: classify_matrix_result(&classes),
            gaps,
        }
    }
}

fn classify_matrix_result(
    classes: &[FeatureConfigurationMatrixResultV1],
) -> FeatureConfigurationMatrixResultV1 {
    let priority = [
        FeatureConfigurationMatrixResultV1::MalformedRow,
        FeatureConfigurationMatrixResultV1::UnknownFeature,
        FeatureConfigurationMatrixResultV1::DuplicateConfiguration,
        FeatureConfigurationMatrixResultV1::EmptyRow,
        FeatureConfigurationMatrixResultV1::StaleInventory,
        FeatureConfigurationMatrixResultV1::PowersetOverreach,
        FeatureConfigurationMatrixResultV1::UncoveredFeature,
    ];
    for candidate in priority {
        if classes.contains(&candidate) {
            return candidate;
        }
    }
    FeatureConfigurationMatrixResultV1::MalformedRow
}

/// Validate a proof request against a matrix: the configuration must exist,
/// identity and claim boundary must match the row, the command surface must
/// name cargo, and the request may not demand deeper proof than the row
/// declares. Callers pass [`supported_feature_configuration_matrix`] to bind
/// the single checked-in source.
pub fn validate_feature_configuration_proof_request_v1(
    request: &FeatureConfigurationProofRequestV1,
    matrix: &SupportedFeatureConfigurationMatrixV1,
) -> FeatureConfigurationRequestValidationV1 {
    let mut gaps: Vec<String> = Vec::new();
    let schema_current = request.schema_id == FEATURE_CONFIGURATION_PROOF_REQUEST_V1_SCHEMA_ID
        && request.schema_version == FEATURE_CONFIGURATION_PROOF_REQUEST_V1_SCHEMA_VERSION;
    if !schema_current {
        gaps.push("request uses a non-current schema generation".to_string());
    }
    if request.cargo_command_tokens.is_empty() {
        gaps.push("request names no cargo command".to_string());
    } else if request.cargo_command_tokens.first() != Some(&"cargo".to_string()) {
        gaps.push("request command surface must begin with cargo".to_string());
    }
    if request.requested_by.trim().is_empty() {
        gaps.push("request must name the lane or issue demanding the proof".to_string());
    }
    let Some(row) = matrix
        .rows
        .iter()
        .find(|row| row.configuration_id == request.configuration_id)
    else {
        return FeatureConfigurationRequestValidationV1 {
            result: FeatureConfigurationRequestResultV1::UnknownConfiguration,
            gaps: prepend_gap(
                gaps,
                format!(
                    "configuration {} is not part of the checked-in matrix",
                    request.configuration_id
                ),
            ),
        };
    };
    if request.root_package_name != row.root_package_name
        || request.root_package_version != row.root_package_version
    {
        gaps.push("request identity drifts from the matrix row".to_string());
    }
    if request.claim_boundary != row.claim_boundary {
        gaps.push(
            "requests inherit the row claim boundary; independent claim text is forbidden"
                .to_string(),
        );
    }
    if request.requested_depth.rank() > row.proof_depth.rank() {
        gaps.push(format!(
            "request demands deeper proof than the row declares: {} requested but the \
             row only declares {}",
            request.requested_depth.as_str(),
            row.proof_depth.as_str()
        ));
    }
    let result = if gaps.iter().any(|gap| gap.contains("deeper proof")) {
        FeatureConfigurationRequestResultV1::DepthPostureConflict
    } else if gaps.is_empty() {
        FeatureConfigurationRequestResultV1::Complete
    } else {
        FeatureConfigurationRequestResultV1::MalformedRequest
    };
    FeatureConfigurationRequestValidationV1 { result, gaps }
}

/// Validate a proof receipt against a matrix: identity and claim boundary
/// must match the row, the executed depth may not exceed the row's declared
/// depth, a green shallower run stays partial_depth, and empty/skipped/failed
/// records can never render as proven. Journey-depth proof must bind
/// retained evidence. Callers pass [`supported_feature_configuration_matrix`]
/// to bind the single checked-in source.
pub fn validate_feature_configuration_proof_receipt_v1(
    receipt: &FeatureConfigurationProofReceiptV1,
    matrix: &SupportedFeatureConfigurationMatrixV1,
) -> FeatureConfigurationReceiptValidationV1 {
    let mut gaps: Vec<String> = Vec::new();
    let schema_current = receipt.schema_id == FEATURE_CONFIGURATION_PROOF_RECEIPT_V1_SCHEMA_ID
        && receipt.schema_version == FEATURE_CONFIGURATION_PROOF_RECEIPT_V1_SCHEMA_VERSION;
    if !schema_current {
        gaps.push("receipt uses a non-current schema generation".to_string());
    }
    let Some(row) = matrix
        .rows
        .iter()
        .find(|row| row.configuration_id == receipt.configuration_id)
    else {
        return FeatureConfigurationReceiptValidationV1 {
            result: FeatureConfigurationReceiptResultV1::UnknownConfiguration,
            gaps: prepend_gap(
                gaps,
                format!(
                    "configuration {} is not part of the checked-in matrix",
                    receipt.configuration_id
                ),
            ),
        };
    };
    if receipt.root_package_name != row.root_package_name
        || receipt.root_package_version != row.root_package_version
    {
        gaps.push("receipt identity drifts from the matrix row".to_string());
    }
    if receipt.claim_boundary != row.claim_boundary {
        gaps.push(
            "receipts inherit the row claim boundary; independent claim text is forbidden"
                .to_string(),
        );
    }
    if receipt.executed_depth.rank() > row.proof_depth.rank() {
        gaps.push(format!(
            "receipt claims {} execution past the row's declared {} depth",
            receipt.executed_depth.as_str(),
            row.proof_depth.as_str()
        ));
    }

    let any_green = receipt
        .command_results
        .iter()
        .any(|command| command.status == FeatureConfigurationCommandStatusV1::Green);
    let all_green = !receipt.command_results.is_empty()
        && receipt
            .command_results
            .iter()
            .all(|command| command.status == FeatureConfigurationCommandStatusV1::Green);
    match receipt.outcome {
        FeatureConfigurationProofOutcomeV1::Proven => {
            if receipt.command_results.is_empty() {
                gaps.push("an empty receipt cannot render as proven".to_string());
            }
            if !all_green && !receipt.command_results.is_empty() {
                gaps.push("proven receipt carries a non-green command".to_string());
            }
            if !receipt.requested_depth.is_proven_by(receipt.executed_depth) {
                gaps.push(format!(
                    "a green shallower run cannot render as deeper proof; record \
                     partial_depth instead of {} from {} execution",
                    receipt.requested_depth.as_str(),
                    receipt.executed_depth.as_str()
                ));
            }
            if receipt.executed_depth.rank() > receipt.requested_depth.rank() {
                gaps.push("receipt claims deeper execution than requested".to_string());
            }
            let evidence_bound = match receipt.evidence_digest.as_deref() {
                Some(digest) => is_sha256_digest(digest),
                None => false,
            };
            if receipt.requested_depth.rank()
                >= FeatureConfigurationProofDepthV1::PackageCandidate.rank()
                && !evidence_bound
            {
                gaps.push(
                    "journey-depth proof must bind retained evidence as a sha256 digest"
                        .to_string(),
                );
            }
        }
        FeatureConfigurationProofOutcomeV1::PartialDepth => {
            if receipt.executed_depth.rank() >= receipt.requested_depth.rank() {
                gaps.push(
                    "partial_depth requires a shallower execution than requested".to_string(),
                );
            }
        }
        FeatureConfigurationProofOutcomeV1::Failed => {
            let any_red = receipt
                .command_results
                .iter()
                .any(|command| command.status == FeatureConfigurationCommandStatusV1::Red);
            if !any_red && receipt.gaps.is_empty() {
                gaps.push("failed receipt must record its failure".to_string());
            }
        }
        FeatureConfigurationProofOutcomeV1::Skipped => {
            if receipt.gaps.is_empty() {
                gaps.push("skipped receipt must record why it was skipped".to_string());
            }
            if any_green {
                gaps.push("skipped receipt cannot carry green commands".to_string());
            }
        }
        FeatureConfigurationProofOutcomeV1::InstrumentFailure => {
            if receipt.gaps.is_empty() {
                gaps.push("instrument-failure receipt must record the instrument gap".to_string());
            }
        }
    }

    let result = if gaps.iter().any(|gap| {
        gap.contains("deeper proof")
            || gap.contains("past the row")
            || gap.contains("deeper execution")
    }) {
        FeatureConfigurationReceiptResultV1::DepthPostureConflict
    } else if gaps
        .iter()
        .any(|gap| gap.contains("cannot render as proven") || gap.contains("non-green command"))
    {
        FeatureConfigurationReceiptResultV1::UnexecutedProof
    } else if gaps.is_empty() {
        FeatureConfigurationReceiptResultV1::Complete
    } else {
        FeatureConfigurationReceiptResultV1::MalformedReceipt
    };
    FeatureConfigurationReceiptValidationV1 { result, gaps }
}

fn prepend_gap(gaps: Vec<String>, first: String) -> Vec<String> {
    let mut ordered = vec![first];
    ordered.extend(gaps);
    ordered
}

fn is_sha256_digest(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return false;
    };
    hex.len() == 64 && hex.chars().all(|character| character.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inventory() -> Vec<CrateFeatureInventoryV1> {
        crate_feature_inventory()
    }

    fn row(configuration_id: &str) -> Result<SupportedFeatureConfigurationV1, String> {
        row_for_supported_feature_configuration(configuration_id)
            .ok_or_else(|| format!("fixture row {configuration_id} is missing from the matrix"))
    }

    fn matrix() -> SupportedFeatureConfigurationMatrixV1 {
        supported_feature_configuration_matrix()
    }

    fn require(condition: bool, message: &str) -> Result<(), String> {
        if condition {
            Ok(())
        } else {
            Err(message.to_string())
        }
    }

    fn matrix_validation_message(validation: &FeatureConfigurationMatrixValidationV1) -> String {
        format!(
            "matrix validation {:?}: {:?}",
            validation.result, validation.gaps
        )
    }

    #[test]
    fn checked_in_matrix_and_inventory_validate_complete() -> Result<(), String> {
        let validation =
            validate_supported_feature_configuration_matrix_v1(&matrix(), &inventory());
        require(
            validation.result == FeatureConfigurationMatrixResultV1::Complete,
            &matrix_validation_message(&validation),
        )?;
        require(
            matrix().rows.len() == 10,
            "the finite matrix must carry the ten selected rows",
        )?;
        require(
            !current_feature_configuration_gaps().is_empty(),
            "PR A must report its current gaps",
        )
    }

    #[test]
    fn depth_vocabulary_orders_shallow_to_deep_and_gates_proof_rendering() -> Result<(), String> {
        let ordered = [
            FeatureConfigurationProofDepthV1::CompileOnly,
            FeatureConfigurationProofDepthV1::UnitAndDocTests,
            FeatureConfigurationProofDepthV1::AllTargets,
            FeatureConfigurationProofDepthV1::PackageCandidate,
            FeatureConfigurationProofDepthV1::InstalledJourney,
            FeatureConfigurationProofDepthV1::InteropJourney,
        ];
        let mut previous: Option<FeatureConfigurationProofDepthV1> = None;
        for depth in ordered {
            if let Some(shallow) = previous {
                require(
                    shallow.rank() < depth.rank(),
                    "depth vocabulary must order shallow to deep",
                )?;
                require(
                    !depth.is_proven_by(shallow),
                    "a green shallower run must not prove a deeper depth",
                )?;
                require(
                    shallow.is_proven_by(depth),
                    "deeper execution proves shallower claims",
                )?;
            }
            previous = Some(depth);
        }
        require(
            FeatureConfigurationProofDepthV1::AllTargets
                .is_proven_by(FeatureConfigurationProofDepthV1::AllTargets),
            "equal depth proves equal claims",
        )
    }

    #[test]
    fn matrix_rejects_a_fabricated_full_powerset_row() -> Result<(), String> {
        let mut fabricated = matrix();
        let mut powerset = row("cargo-proof.all-providers")?;
        powerset.configuration_id = "cargo-proof.full-powerset".to_string();
        powerset.explicit_features = vec![
            "provider-cargo-allow".to_string(),
            "provider-hawk".to_string(),
            "provider-ripr".to_string(),
            "all-providers".to_string(),
        ];
        fabricated.rows.push(powerset);
        let validation =
            validate_supported_feature_configuration_matrix_v1(&fabricated, &inventory());
        require(
            validation.result == FeatureConfigurationMatrixResultV1::PowersetOverreach,
            &matrix_validation_message(&validation),
        )?;
        require(
            validation
                .gaps
                .iter()
                .any(|gap| gap.contains("re-declares a selection already owned")),
            "powerset rejection must name the ownership law",
        )
    }

    #[test]
    fn matrix_rejects_a_rebranded_duplicate_selection() -> Result<(), String> {
        let mut fabricated = matrix();
        let mut rebranded = row("allow-rust.syntax-explicit")?;
        rebranded.configuration_id = "allow-rust.syntax-again".to_string();
        fabricated.rows.push(rebranded);
        let validation =
            validate_supported_feature_configuration_matrix_v1(&fabricated, &inventory());
        require(
            validation.result == FeatureConfigurationMatrixResultV1::PowersetOverreach,
            &matrix_validation_message(&validation),
        )
    }

    #[test]
    fn matrix_rejects_duplicate_configuration_ids() -> Result<(), String> {
        let mut fabricated = matrix();
        let duplicated = row("allow-files.changie")?;
        fabricated.rows.push(duplicated);
        let validation =
            validate_supported_feature_configuration_matrix_v1(&fabricated, &inventory());
        require(
            validation.result == FeatureConfigurationMatrixResultV1::DuplicateConfiguration,
            &matrix_validation_message(&validation),
        )
    }

    #[test]
    fn matrix_rejects_unknown_feature_names() -> Result<(), String> {
        let mut fabricated = matrix();
        let mut unknown = row("allow-rust.default")?;
        unknown.configuration_id = "allow-rust.heavy".to_string();
        unknown.no_default_features = NoDefaultFeaturesPostureV1::NoDefaultFeatures;
        unknown.default_features_selected = Vec::new();
        unknown.explicit_features = vec!["heavy".to_string()];
        fabricated.rows.push(unknown);
        let validation =
            validate_supported_feature_configuration_matrix_v1(&fabricated, &inventory());
        require(
            validation.result == FeatureConfigurationMatrixResultV1::UnknownFeature,
            &matrix_validation_message(&validation),
        )?;
        require(
            validation
                .gaps
                .iter()
                .any(|gap| gap.contains("does not declare")),
            "unknown-feature rejection must name the manifest law",
        )
    }

    #[test]
    fn matrix_rejects_empty_rows() -> Result<(), String> {
        let mut fabricated = matrix();
        let mut empty = row("allow-rust.default")?;
        empty.configuration_id = "allow-rust.empty".to_string();
        empty.expected_capabilities = Vec::new();
        empty.expected_assets = Vec::new();
        empty.claim_boundary = String::new();
        fabricated.rows.push(empty);
        let validation =
            validate_supported_feature_configuration_matrix_v1(&fabricated, &inventory());
        require(
            validation.result == FeatureConfigurationMatrixResultV1::EmptyRow,
            &matrix_validation_message(&validation),
        )
    }

    #[test]
    fn matrix_rejects_posture_disagreement_with_manifest_defaults() -> Result<(), String> {
        let mut fabricated = matrix();
        let mut drifted = row("allow-rust.default")?;
        drifted.default_features_selected = Vec::new();
        fabricated.rows.push(drifted);
        let validation =
            validate_supported_feature_configuration_matrix_v1(&fabricated, &inventory());
        require(
            validation.result == FeatureConfigurationMatrixResultV1::MalformedRow,
            &matrix_validation_message(&validation),
        )?;

        let mut bypass = matrix();
        let mut named_default = row("allow-rust.minimal-model")?;
        named_default.configuration_id = "allow-rust.default-named".to_string();
        named_default.explicit_features = vec!["default".to_string()];
        bypass.rows.push(named_default);
        let validation = validate_supported_feature_configuration_matrix_v1(&bypass, &inventory());
        require(
            validation.result == FeatureConfigurationMatrixResultV1::MalformedRow,
            &matrix_validation_message(&validation),
        )
    }

    #[test]
    fn matrix_rejects_rows_for_packages_outside_the_inventory() -> Result<(), String> {
        let mut fabricated = matrix();
        let mut stranger = row("allow-rust.default")?;
        stranger.configuration_id = "stranger.default".to_string();
        stranger.root_package_name = "stranger".to_string();
        fabricated.rows.push(stranger);
        let validation =
            validate_supported_feature_configuration_matrix_v1(&fabricated, &inventory());
        require(
            validation.result == FeatureConfigurationMatrixResultV1::StaleInventory,
            &matrix_validation_message(&validation),
        )
    }

    #[test]
    fn matrix_rejects_version_drift_from_the_inventory() -> Result<(), String> {
        let mut fabricated = matrix();
        let mut stale = row("allow-rust.default")?;
        stale.configuration_id = "allow-rust.default-stale".to_string();
        stale.root_package_version = "0.1.11".to_string();
        fabricated.rows.push(stale);
        let validation =
            validate_supported_feature_configuration_matrix_v1(&fabricated, &inventory());
        require(
            validation.result == FeatureConfigurationMatrixResultV1::StaleInventory,
            &matrix_validation_message(&validation),
        )
    }

    #[test]
    fn matrix_rejects_inventory_features_without_a_selected_row() -> Result<(), String> {
        let mut extended_inventory = inventory();
        let allow_files = extended_inventory
            .iter_mut()
            .find(|entry| entry.crate_name == "allow-files")
            .ok_or_else(|| "inventory lost allow-files".to_string())?;
        allow_files.declared_features.push("future".to_string());
        let validation =
            validate_supported_feature_configuration_matrix_v1(&matrix(), &extended_inventory);
        require(
            validation.result == FeatureConfigurationMatrixResultV1::UncoveredFeature,
            &matrix_validation_message(&validation),
        )?;
        require(
            validation
                .gaps
                .iter()
                .any(|gap| gap.contains("not selected or implied")),
            "coverage rejection must name the coverage law",
        )
    }

    #[test]
    fn row_lookups_bind_configuration_ids_to_one_source() -> Result<(), String> {
        let looked_up = row_for_supported_feature_configuration("allow-rust.minimal-model")
            .ok_or_else(|| "minimal-model row missing from the matrix".to_string())?;
        require(
            looked_up.no_default_features == NoDefaultFeaturesPostureV1::NoDefaultFeatures,
            "minimal-model must be a no-default row",
        )?;
        require(
            looked_up.manifest_identity() == "allow-rust:0.2.0-rc.1",
            "row identity must bind package and exact version",
        )?;
        require(
            row_for_supported_feature_configuration("allow-rust.not-a-row").is_none(),
            "unknown configuration ids must not resolve",
        )
    }

    fn sample_request(
        target: &SupportedFeatureConfigurationV1,
    ) -> FeatureConfigurationProofRequestV1 {
        FeatureConfigurationProofRequestV1 {
            schema_id: FEATURE_CONFIGURATION_PROOF_REQUEST_V1_SCHEMA_ID.to_string(),
            schema_version: FEATURE_CONFIGURATION_PROOF_REQUEST_V1_SCHEMA_VERSION,
            configuration_id: target.configuration_id.clone(),
            root_package_name: target.root_package_name.clone(),
            root_package_version: target.root_package_version.clone(),
            requested_depth: target.proof_depth,
            cargo_command_tokens: vec![
                "cargo".to_string(),
                "test".to_string(),
                "-p".to_string(),
                target.root_package_name.clone(),
                "--all-targets".to_string(),
                "--locked".to_string(),
            ],
            requested_by: "#3905 PR B".to_string(),
            claim_boundary: target.claim_boundary.clone(),
        }
    }

    fn request_validation_message(validation: &FeatureConfigurationRequestValidationV1) -> String {
        format!(
            "request validation {:?}: {:?}",
            validation.result, validation.gaps
        )
    }

    #[test]
    fn request_accepts_a_row_bound_request() -> Result<(), String> {
        let request = sample_request(&row("allow-rust.default")?);
        let validation = validate_feature_configuration_proof_request_v1(&request, &matrix());
        require(
            validation.result == FeatureConfigurationRequestResultV1::Complete,
            &request_validation_message(&validation),
        )
    }

    #[test]
    fn request_rejects_deeper_proof_than_the_row_declares() -> Result<(), String> {
        let mut request = sample_request(&row("allow-rust.minimal-model")?);
        request.requested_depth = FeatureConfigurationProofDepthV1::AllTargets;
        let validation = validate_feature_configuration_proof_request_v1(&request, &matrix());
        require(
            validation.result == FeatureConfigurationRequestResultV1::DepthPostureConflict,
            &request_validation_message(&validation),
        )
    }

    #[test]
    fn request_rejects_unknown_configuration_and_identity_drift() -> Result<(), String> {
        let mut request = sample_request(&row("allow-rust.default")?);
        request.configuration_id = "allow-rust.not-a-row".to_string();
        let validation = validate_feature_configuration_proof_request_v1(&request, &matrix());
        require(
            validation.result == FeatureConfigurationRequestResultV1::UnknownConfiguration,
            &request_validation_message(&validation),
        )?;

        let mut drifted = sample_request(&row("allow-rust.default")?);
        drifted.root_package_version = "0.1.11".to_string();
        let validation = validate_feature_configuration_proof_request_v1(&drifted, &matrix());
        require(
            validation.result == FeatureConfigurationRequestResultV1::MalformedRequest,
            &request_validation_message(&validation),
        )
    }

    #[test]
    fn request_rejects_independent_claim_boundary_text() -> Result<(), String> {
        let mut request = sample_request(&row("cargo-proof.provider-hawk")?);
        request.claim_boundary = "provider hawk fully proven and installed".to_string();
        let validation = validate_feature_configuration_proof_request_v1(&request, &matrix());
        require(
            validation.result == FeatureConfigurationRequestResultV1::MalformedRequest,
            &request_validation_message(&validation),
        )?;
        require(
            validation
                .gaps
                .iter()
                .any(|gap| gap.contains("independent claim text is forbidden")),
            "claim-boundary rejection must name the single-source law",
        )
    }

    fn sample_receipt(
        target: &SupportedFeatureConfigurationV1,
    ) -> FeatureConfigurationProofReceiptV1 {
        FeatureConfigurationProofReceiptV1 {
            schema_id: FEATURE_CONFIGURATION_PROOF_RECEIPT_V1_SCHEMA_ID.to_string(),
            schema_version: FEATURE_CONFIGURATION_PROOF_RECEIPT_V1_SCHEMA_VERSION,
            configuration_id: target.configuration_id.clone(),
            root_package_name: target.root_package_name.clone(),
            root_package_version: target.root_package_version.clone(),
            requested_depth: target.proof_depth,
            executed_depth: target.proof_depth,
            outcome: FeatureConfigurationProofOutcomeV1::Proven,
            command_results: vec![FeatureConfigurationCommandResultV1 {
                command_tokens: vec![
                    "cargo".to_string(),
                    "test".to_string(),
                    "-p".to_string(),
                    target.root_package_name.clone(),
                ],
                status: FeatureConfigurationCommandStatusV1::Green,
                note: String::new(),
            }],
            evidence_digest: Some(format!("sha256:{:064x}", 7)),
            gaps: Vec::new(),
            claim_boundary: target.claim_boundary.clone(),
        }
    }

    fn receipt_validation_message(validation: &FeatureConfigurationReceiptValidationV1) -> String {
        format!(
            "receipt validation {:?}: {:?}",
            validation.result, validation.gaps
        )
    }

    #[test]
    fn receipt_accepts_an_honest_proven_record() -> Result<(), String> {
        let receipt = sample_receipt(&row("allow-rust.default")?);
        let validation = validate_feature_configuration_proof_receipt_v1(&receipt, &matrix());
        require(
            validation.result == FeatureConfigurationReceiptResultV1::Complete,
            &receipt_validation_message(&validation),
        )
    }

    #[test]
    fn receipt_rejects_empty_and_skipped_receipts_rendered_as_proven() -> Result<(), String> {
        let mut empty = sample_receipt(&row("allow-rust.default")?);
        empty.command_results = Vec::new();
        let validation = validate_feature_configuration_proof_receipt_v1(&empty, &matrix());
        require(
            validation.result == FeatureConfigurationReceiptResultV1::UnexecutedProof,
            &receipt_validation_message(&validation),
        )?;
        require(
            validation
                .gaps
                .iter()
                .any(|gap| gap.contains("cannot render as proven")),
            "empty-receipt rejection must name the honesty law",
        )?;

        let mut skipped = sample_receipt(&row("allow-files.changie")?);
        skipped.outcome = FeatureConfigurationProofOutcomeV1::Skipped;
        skipped.command_results = Vec::new();
        skipped.gaps = vec!["lane skipped".to_string()];
        let validation = validate_feature_configuration_proof_receipt_v1(&skipped, &matrix());
        require(
            validation.result == FeatureConfigurationReceiptResultV1::Complete,
            &receipt_validation_message(&validation),
        )?;

        let mut skipped_as_proven = sample_receipt(&row("allow-files.changie")?);
        skipped_as_proven.command_results = vec![FeatureConfigurationCommandResultV1 {
            command_tokens: vec!["cargo".to_string(), "check".to_string()],
            status: FeatureConfigurationCommandStatusV1::Skipped,
            note: String::new(),
        }];
        let validation =
            validate_feature_configuration_proof_receipt_v1(&skipped_as_proven, &matrix());
        require(
            validation.result == FeatureConfigurationReceiptResultV1::UnexecutedProof,
            &receipt_validation_message(&validation),
        )
    }

    #[test]
    fn receipt_cannot_render_a_green_shallower_run_as_deeper_proof() -> Result<(), String> {
        let mut overreach = sample_receipt(&row("allow-rust.minimal-model")?);
        overreach.requested_depth = FeatureConfigurationProofDepthV1::AllTargets;
        overreach.command_results = vec![FeatureConfigurationCommandResultV1 {
            command_tokens: vec![
                "cargo".to_string(),
                "check".to_string(),
                "-p".to_string(),
                "allow-rust".to_string(),
                "--no-default-features".to_string(),
            ],
            status: FeatureConfigurationCommandStatusV1::Green,
            note: String::new(),
        }];
        overreach.evidence_digest = None;
        let validation = validate_feature_configuration_proof_receipt_v1(&overreach, &matrix());
        require(
            validation.result == FeatureConfigurationReceiptResultV1::DepthPostureConflict,
            &receipt_validation_message(&validation),
        )?;
        require(
            validation
                .gaps
                .iter()
                .any(|gap| gap.contains("cannot render as deeper proof")),
            "depth rejection must name the render law",
        )?;

        let mut partial = overreach;
        partial.outcome = FeatureConfigurationProofOutcomeV1::PartialDepth;
        let validation = validate_feature_configuration_proof_receipt_v1(&partial, &matrix());
        require(
            validation.result == FeatureConfigurationReceiptResultV1::Complete,
            &receipt_validation_message(&validation),
        )
    }

    #[test]
    fn receipt_rejects_overclaims_past_the_row_depth() -> Result<(), String> {
        let mut overclaimed = sample_receipt(&row("allow-rust.minimal-model")?);
        overclaimed.requested_depth = FeatureConfigurationProofDepthV1::InteropJourney;
        overclaimed.executed_depth = FeatureConfigurationProofDepthV1::InteropJourney;
        let validation = validate_feature_configuration_proof_receipt_v1(&overclaimed, &matrix());
        require(
            validation.result == FeatureConfigurationReceiptResultV1::DepthPostureConflict,
            &receipt_validation_message(&validation),
        )
    }

    #[test]
    fn receipt_accepts_failed_records_that_tell_the_truth() -> Result<(), String> {
        let mut failed = sample_receipt(&row("cargo-proof.provider-hawk")?);
        failed.outcome = FeatureConfigurationProofOutcomeV1::Failed;
        failed.command_results = vec![FeatureConfigurationCommandResultV1 {
            command_tokens: vec![
                "cargo".to_string(),
                "test".to_string(),
                "-p".to_string(),
                "cargo-proof".to_string(),
            ],
            status: FeatureConfigurationCommandStatusV1::Red,
            note: "provider fixture compile failure".to_string(),
        }];
        let validation = validate_feature_configuration_proof_receipt_v1(&failed, &matrix());
        require(
            validation.result == FeatureConfigurationReceiptResultV1::Complete,
            &receipt_validation_message(&validation),
        )?;

        let mut silent_failure = sample_receipt(&row("cargo-proof.provider-hawk")?);
        silent_failure.outcome = FeatureConfigurationProofOutcomeV1::Failed;
        silent_failure.command_results = Vec::new();
        silent_failure.gaps = Vec::new();
        let validation =
            validate_feature_configuration_proof_receipt_v1(&silent_failure, &matrix());
        require(
            validation.result == FeatureConfigurationReceiptResultV1::MalformedReceipt,
            &receipt_validation_message(&validation),
        )
    }

    #[test]
    fn journey_depth_proofs_must_bind_retained_evidence() -> Result<(), String> {
        // No current row declares a journey depth; extend a copy of the
        // checked-in matrix the way a future row would.
        let mut journey_matrix = matrix();
        let journey_row = journey_matrix
            .rows
            .iter_mut()
            .find(|row| row.configuration_id == "cargo-proof.default")
            .ok_or_else(|| "matrix lost the cargo-proof.default row".to_string())?;
        journey_row.proof_depth = FeatureConfigurationProofDepthV1::PackageCandidate;

        let mut journey = sample_receipt(&row("cargo-proof.default")?);
        journey.requested_depth = FeatureConfigurationProofDepthV1::PackageCandidate;
        journey.executed_depth = FeatureConfigurationProofDepthV1::PackageCandidate;
        journey.evidence_digest = None;
        let validation = validate_feature_configuration_proof_receipt_v1(&journey, &journey_matrix);
        require(
            validation.result == FeatureConfigurationReceiptResultV1::MalformedReceipt,
            &receipt_validation_message(&validation),
        )?;
        journey.evidence_digest = Some(format!("sha256:{:064x}", 9));
        let validation = validate_feature_configuration_proof_receipt_v1(&journey, &journey_matrix);
        require(
            validation.result == FeatureConfigurationReceiptResultV1::Complete,
            &receipt_validation_message(&validation),
        )?;

        // Against the checked-in matrix the same receipt overclaims: no
        // row owns a journey depth yet.
        let validation = validate_feature_configuration_proof_receipt_v1(&journey, &matrix());
        require(
            validation.result == FeatureConfigurationReceiptResultV1::DepthPostureConflict,
            &receipt_validation_message(&validation),
        )
    }

    #[test]
    fn contracts_roundtrip_and_reject_unknown_fields() -> Result<(), String> {
        let matrix_json = render_supported_feature_configuration_matrix_v1(&matrix())
            .map_err(|error| error.to_string())?;
        let parsed: SupportedFeatureConfigurationMatrixV1 =
            serde_json::from_str(&matrix_json).map_err(|error| error.to_string())?;
        require(
            parsed == matrix(),
            "matrix roundtrip must preserve the checked-in set",
        )?;
        let tampered = matrix_json.replace(
            "\"matrix_id\"",
            "\"unexpected_field\": true,\n  \"matrix_id\"",
        );
        require(
            serde_json::from_str::<SupportedFeatureConfigurationMatrixV1>(&tampered).is_err(),
            "matrix must deny unknown fields",
        )?;

        let receipt = sample_receipt(&row("allow-rust.syntax-explicit")?);
        let receipt_json = render_feature_configuration_proof_receipt_v1(&receipt)
            .map_err(|error| error.to_string())?;
        let reparsed: FeatureConfigurationProofReceiptV1 =
            serde_json::from_str(&receipt_json).map_err(|error| error.to_string())?;
        require(
            reparsed == receipt,
            "receipt roundtrip must preserve the record",
        )?;

        let first = render_feature_configuration_proof_receipt_v1_bytes(&receipt)
            .map_err(|error| error.to_string())?;
        let second = render_feature_configuration_proof_receipt_v1_bytes(&receipt)
            .map_err(|error| error.to_string())?;
        require(
            first == second,
            "receipt rendering must be deterministic across equal records",
        )?;
        let matrix_bytes = render_supported_feature_configuration_matrix_v1_bytes(&matrix())
            .map_err(|error| error.to_string())?;
        require(
            !matrix_bytes.is_empty(),
            "matrix rendering must produce bytes",
        )
    }

    #[test]
    fn serialization_uses_the_documented_snake_case_names() -> Result<(), String> {
        let depth = serde_json::to_string(&FeatureConfigurationProofDepthV1::UnitAndDocTests)
            .map_err(|error| error.to_string())?;
        require(
            depth == "\"unit_and_doc_tests\"",
            format!("depth serialization drifted: {depth}").as_str(),
        )?;
        let posture = serde_json::to_string(&NoDefaultFeaturesPostureV1::NoDefaultFeatures)
            .map_err(|error| error.to_string())?;
        require(
            posture == "\"no_default_features\"",
            format!("posture serialization drifted: {posture}").as_str(),
        )?;
        let product = serde_json::to_string(&FeatureConfigurationProductV1::CargoProof)
            .map_err(|error| error.to_string())?;
        require(
            product == "\"cargo_proof\"",
            format!("product serialization drifted: {product}").as_str(),
        )
    }
}
