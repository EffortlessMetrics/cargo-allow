//! Typed dependency graph delta contract (#3920 PR A): one exact
//! base-to-head explanation of what the dependency graph changed,
//! preserving native Cargo identities (package name, version, source,
//! checksum) alongside the delta kind so a review bot or human cannot
//! silently omit a lockfile change, describe a downgrade as a
//! compatible update, or lose count parity through replacement.
//!
//! Report only: the contract defines the vocabulary and the receipt
//! shape; a separate base/head delta compiler (PR B) populates it.

use serde::{Deserialize, Serialize};

/// Distinct from allow-diff's `cargo-allow.dependency-graph-delta.v1`
/// evaluation receipt: this identity names the allow-report compiler
/// receipt whose rows are compiled from exact manifest/lockfile text.
pub const DEPENDENCY_GRAPH_DELTA_SCHEMA_ID: &str = "cargo-allow.dependency-graph-delta-compiler.v1";
pub const DEPENDENCY_GRAPH_DELTA_SCHEMA_VERSION: u32 = 1;

/// The delta kind for one dependency graph row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyGraphDeltaKindV1 {
    DirectRequirementAdded,
    DirectRequirementRemoved,
    DirectRequirementRaised,
    DirectRequirementLowered,
    RequirementRangeBroadened,
    RequirementRangeNarrowed,
    LockOnlyResolutionChanged,
    PackageAdded,
    PackageRemoved,
    PackageUpgraded,
    PackageDowngraded,
    SourceOrChecksumChanged,
    FeatureActivationChanged,
    DuplicateVersionMovement,
    TargetOrDependencyClassChanged,
    ManifestLockMismatch,
    NoSemanticGraphChange,
    UnsupportedOrInstrumentFailure,
}

impl DependencyGraphDeltaKindV1 {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DirectRequirementAdded => "direct_requirement_added",
            Self::DirectRequirementRemoved => "direct_requirement_removed",
            Self::DirectRequirementRaised => "direct_requirement_raised",
            Self::DirectRequirementLowered => "direct_requirement_lowered",
            Self::RequirementRangeBroadened => "requirement_range_broadened",
            Self::RequirementRangeNarrowed => "requirement_range_narrowed",
            Self::LockOnlyResolutionChanged => "lock_only_resolution_changed",
            Self::PackageAdded => "package_added",
            Self::PackageRemoved => "package_removed",
            Self::PackageUpgraded => "package_upgraded",
            Self::PackageDowngraded => "package_downgraded",
            Self::SourceOrChecksumChanged => "source_or_checksum_changed",
            Self::FeatureActivationChanged => "feature_activation_changed",
            Self::DuplicateVersionMovement => "duplicate_version_movement",
            Self::TargetOrDependencyClassChanged => "target_or_dependency_class_changed",
            Self::ManifestLockMismatch => "manifest_lock_mismatch",
            Self::NoSemanticGraphChange => "no_semantic_graph_change",
            Self::UnsupportedOrInstrumentFailure => "unsupported_or_instrument_failure",
        }
    }

    /// Whether the delta kind represents a semantic graph change (some
    /// kind like ManifestLockMismatch or
    /// UnsupportedOrInstrumentFailure describe lane health rather than
    /// graph movement).
    #[must_use]
    pub const fn is_semantic(self) -> bool {
        !matches!(
            self,
            Self::ManifestLockMismatch | Self::UnsupportedOrInstrumentFailure
        )
    }

    /// Whether the delta kind represents a direct manifest requirement
    /// change (as opposed to a lock-only resolution or transitive
    /// movement).
    #[must_use]
    pub const fn is_direct_requirement(self) -> bool {
        matches!(
            self,
            Self::DirectRequirementAdded
                | Self::DirectRequirementRemoved
                | Self::DirectRequirementRaised
                | Self::DirectRequirementLowered
                | Self::RequirementRangeBroadened
                | Self::RequirementRangeNarrowed
        )
    }
}

/// The dependency class: normal, development, build, or target-specific.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyClassV1 {
    Normal,
    Development,
    Build,
    TargetSpecific,
    Optional,
}

/// One row in the delta: a single package's base-to-head movement with
/// its native Cargo identity preserved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyGraphDeltaRowV1 {
    pub kind: DependencyGraphDeltaKindV1,
    pub class: DependencyClassV1,
    pub package_name: String,
    /// The base-side resolved version (empty when added).
    pub base_version: String,
    /// The head-side resolved version (empty when removed).
    pub head_version: String,
    /// The base-side requirement range from the manifest (empty when
    /// the package was not a direct requirement at base).
    pub base_requirement: String,
    /// The head-side requirement range from the manifest (empty when
    /// the package was removed from the manifest).
    pub head_requirement: String,
    /// The base-side source identity (registry, git, path).
    pub base_source: String,
    /// The head-side source identity.
    pub head_source: String,
    /// The base-side checksum (empty for path/git sources).
    pub base_checksum: String,
    /// The head-side checksum.
    pub head_checksum: String,
}

/// The exact base/head identity the delta was compiled against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyGraphDeltaIdentityV1 {
    pub base_commit: String,
    pub head_commit: String,
    pub base_manifest_set_digest: String,
    pub head_manifest_set_digest: String,
    pub base_lock_digest: String,
    pub head_lock_digest: String,
    pub product: String,
    pub target: String,
}

/// The typed dependency graph delta receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyGraphDeltaReceiptV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub identity: DependencyGraphDeltaIdentityV1,
    pub rows: Vec<DependencyGraphDeltaRowV1>,
    /// True when the delta compiler could read both manifests and both
    /// lockfiles in full; false when any input was missing, stale, or
    /// malformed.
    pub complete: bool,
    pub limitations: Vec<String>,
    pub claim_boundary: String,
}

impl DependencyGraphDeltaReceiptV1 {
    /// Whether the receipt contains at least one semantic graph change.
    #[must_use]
    pub fn has_semantic_changes(&self) -> bool {
        self.rows.iter().any(|row| row.kind.is_semantic())
    }

    /// The number of rows by kind.
    #[must_use]
    pub fn count_by_kind(&self, kind: DependencyGraphDeltaKindV1) -> u32 {
        self.rows.iter().filter(|row| row.kind == kind).count() as u32
    }
}

/// A falsifying fixture: one compact (base lock, head lock, base
/// manifest, head manifest) tuple that exercises a specific delta kind
/// or negative control. The delta compiler (PR B) consumes these to
/// prove it distinguishes every family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyGraphDeltaFixtureV1 {
    pub id: &'static str,
    pub description: &'static str,
    /// The expected dominant delta kind.
    pub expected_kind: DependencyGraphDeltaKindV1,
    pub base_lock: &'static str,
    pub head_lock: &'static str,
    pub base_manifest: &'static str,
    pub head_manifest: &'static str,
}

/// The qualification fixture corpus covering the delta kinds and the
/// required negative controls. Each fixture is a minimal pair that a
/// correct delta compiler must distinguish.
#[must_use]
pub fn dependency_graph_delta_fixtures() -> Vec<DependencyGraphDeltaFixtureV1> {
    vec![
        DependencyGraphDeltaFixtureV1 {
            id: "dep-upgrade-lock-only",
            description: "Lockfile resolution moved up but no manifest requirement changed: the delta must show LockOnlyResolutionChanged, not a manifest edit.",
            expected_kind: DependencyGraphDeltaKindV1::LockOnlyResolutionChanged,
            base_lock: "[[package]]\nname = \"serde\"\nversion = \"1.0.200\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"aaa\"\n",
            head_lock: "[[package]]\nname = \"serde\"\nversion = \"1.0.228\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"bbb\"\n",
            base_manifest: "[dependencies]\nserde = \"1\"\n",
            head_manifest: "[dependencies]\nserde = \"1\"\n",
        },
        DependencyGraphDeltaFixtureV1 {
            id: "dep-downgrade-manifest",
            description: "A manifest requirement was lowered (e.g. TOML 1.x to 0.8): the delta must show DirectRequirementLowered, never a compatible update.",
            expected_kind: DependencyGraphDeltaKindV1::DirectRequirementLowered,
            base_lock: "[[package]]\nname = \"toml\"\nversion = \"1.1.4\"\n",
            head_lock: "[[package]]\nname = \"toml\"\nversion = \"0.8.1\"\n",
            base_manifest: "[dependencies]\ntoml = \"1\"",
            head_manifest: "[dependencies]\ntoml = \"0.8\"",
        },
        DependencyGraphDeltaFixtureV1 {
            id: "dep-source-checksum-change",
            description: "Same package name and version but the source or checksum changed: the delta must show SourceOrChecksumChanged; count parity does not establish graph identity.",
            expected_kind: DependencyGraphDeltaKindV1::SourceOrChecksumChanged,
            base_lock: "[[package]]\nname = \"widget\"\nversion = \"1.0.0\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"old\"\n",
            head_lock: "[[package]]\nname = \"widget\"\nversion = \"1.0.0\"\nsource = \"git+https://github.com/example/widget\"\nchecksum = \"new\"\n",
            base_manifest: "[dependencies]\nwidget = \"1\"",
            head_manifest: "[dependencies]\nwidget = { git = \"https://github.com/example/widget\" }",
        },
        DependencyGraphDeltaFixtureV1 {
            id: "dep-range-narrowed",
            description: "A manifest requirement range was narrowed (e.g. 1.0 to 1.0.5): the delta must show RequirementRangeNarrowed.",
            expected_kind: DependencyGraphDeltaKindV1::RequirementRangeNarrowed,
            base_lock: "[[package]]\nname = \"cli\"\nversion = \"4.6.1\"\n",
            head_lock: "[[package]]\nname = \"cli\"\nversion = \"4.6.8\"\n",
            base_manifest: "[dependencies]\ncli = \"4\"",
            head_manifest: "[dependencies]\ncli = \"4.6\"",
        },
        DependencyGraphDeltaFixtureV1 {
            id: "dep-feature-activated",
            description: "A feature was activated (e.g. serde derive enabled): the delta must show FeatureActivationChanged.",
            expected_kind: DependencyGraphDeltaKindV1::FeatureActivationChanged,
            base_lock: "[[package]]\nname = \"serde\"\nversion = \"1.0.228\"\n",
            head_lock: "[[package]]\nname = \"serde\"\nversion = \"1.0.228\"\n",
            base_manifest: "[dependencies]\nserde = \"1\"",
            head_manifest: "[dependencies]\nserde = { version = \"1\", features = [\"derive\"] }",
        },
        DependencyGraphDeltaFixtureV1 {
            id: "dep-transitive-replaced",
            description: "Transitive package A was replaced with unrelated B while the aggregate package count stayed equal: the delta must show both PackageRemoved and PackageAdded; count parity does not establish graph identity.",
            expected_kind: DependencyGraphDeltaKindV1::PackageRemoved,
            base_lock: "[[package]]\nname = \"alpha\"\nversion = \"1.0\"\n\n[[package]]\nname = \"beta\"\nversion = \"2.0\"\n",
            head_lock: "[[package]]\nname = \"alpha\"\nversion = \"1.0\"\n\n[[package]]\nname = \"gamma\"\nversion = \"2.0\"\n",
            base_manifest: "",
            head_manifest: "",
        },
    ]
}
