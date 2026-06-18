//! Shared descriptors for supported legacy migration lanes.
//!
//! One table drives compat-kind identity, legacy filenames, policy dialect keys,
//! and characterization metadata for agents and tests. Migration behavior is
//! unchanged; this module only centralizes lane metadata.

use allow_core::FindingKind;

/// Supported legacy migration lane identity.
pub type MigrationLane = CompatKind;

/// Stable compat-kind identifier used in migration guides, gap inventory, and
/// `legacy_compat_kind` lookups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompatKind {
    NonRust,
    Generated,
    NoPanicAllowlist,
    PanicBaseline,
    LintException,
    Unsafe,
    Executable,
    Workflow,
    DependencySurface,
    Process,
    Network,
}

impl CompatKind {
    pub const ALL: &[Self] = &[
        Self::NonRust,
        Self::Generated,
        Self::NoPanicAllowlist,
        Self::PanicBaseline,
        Self::LintException,
        Self::Unsafe,
        Self::Executable,
        Self::Workflow,
        Self::DependencySurface,
        Self::Process,
        Self::Network,
    ];

    pub const fn compat_kind_id(self) -> &'static str {
        match self {
            Self::NonRust => "non-rust",
            Self::Generated => "generated",
            Self::NoPanicAllowlist => "no-panic-allowlist",
            Self::PanicBaseline => "panic",
            Self::LintException => "lint-exception",
            Self::Unsafe => "unsafe",
            Self::Executable => "executable",
            Self::Workflow => "workflow",
            Self::DependencySurface => "dependency-surface",
            Self::Process => "process",
            Self::Network => "network",
        }
    }

    pub fn from_compat_kind_id(id: &str) -> Option<Self> {
        Some(match id {
            "non-rust" => Self::NonRust,
            "generated" => Self::Generated,
            "no-panic-allowlist" => Self::NoPanicAllowlist,
            "panic" => Self::PanicBaseline,
            "lint-exception" => Self::LintException,
            "unsafe" => Self::Unsafe,
            "executable" => Self::Executable,
            "workflow" => Self::Workflow,
            "dependency-surface" => Self::DependencySurface,
            "process" => Self::Process,
            "network" => Self::Network,
            _ => return None,
        })
    }
}

/// How a legacy policy file encodes its entries before conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyInputKind {
    AllowRules,
    BaselineEntries,
    ClippyPolicyTable,
}

/// Evidence import posture for a lane's happy-path fixture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidencePolicy {
    Required,
    OptionalWithDebtFallback,
}

/// Lifecycle field import posture for a lane's happy-path fixture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecyclePolicy {
    Full,
    Partial,
    DebtDefaults,
}

/// Baseline-debt visibility posture for a lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebtPolicy {
    None,
    VisibleBaselineDebt,
    MissingEvidenceTodo,
}

/// Minimal canonical output shape expected after migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpectedCanonicalShape {
    pub finding_kind: FindingKind,
    pub policy_dialect: &'static str,
}

/// Optional closeout-queue routing hints for migration summaries (metadata only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CloseoutQueueHints {
    pub phase: &'static str,
    pub queue_id: &'static str,
}

/// Descriptor for one supported legacy migration lane.
#[derive(Debug, Clone, Copy)]
pub struct LegacyLaneDescriptor {
    pub lane: CompatKind,
    pub legacy_filename: &'static str,
    pub legacy_policy_key: &'static str,
    pub legacy_input_kind: LegacyInputKind,
    pub evidence_policy: EvidencePolicy,
    pub lifecycle_policy: LifecyclePolicy,
    pub debt_policy: DebtPolicy,
    pub canonical_shape: ExpectedCanonicalShape,
    pub closeout_queue: Option<CloseoutQueueHints>,
    pub primary_fixture_file: &'static str,
}

impl LegacyLaneDescriptor {
    pub const fn compat_kind_id(self) -> &'static str {
        self.lane.compat_kind_id()
    }

    pub const fn primary_lane(self) -> bool {
        true
    }
}

const CANONICAL_POLICY: &str = "cargo-allow";

const LEGACY_LANE_DESCRIPTORS: &[LegacyLaneDescriptor] = &[
    LegacyLaneDescriptor {
        lane: CompatKind::NonRust,
        legacy_filename: "non-rust-allowlist.toml",
        legacy_policy_key: "non-rust-allowlist",
        legacy_input_kind: LegacyInputKind::AllowRules,
        evidence_policy: EvidencePolicy::Required,
        lifecycle_policy: LifecyclePolicy::Full,
        debt_policy: DebtPolicy::None,
        canonical_shape: ExpectedCanonicalShape {
            finding_kind: FindingKind::NonRustFile,
            policy_dialect: CANONICAL_POLICY,
        },
        closeout_queue: None,
        primary_fixture_file: "non-rust.toml",
    },
    LegacyLaneDescriptor {
        lane: CompatKind::Generated,
        legacy_filename: "generated-allowlist.toml",
        legacy_policy_key: "generated-allowlist",
        legacy_input_kind: LegacyInputKind::AllowRules,
        evidence_policy: EvidencePolicy::Required,
        lifecycle_policy: LifecyclePolicy::Full,
        debt_policy: DebtPolicy::None,
        canonical_shape: ExpectedCanonicalShape {
            finding_kind: FindingKind::GeneratedCode,
            policy_dialect: CANONICAL_POLICY,
        },
        closeout_queue: None,
        primary_fixture_file: "generated.toml",
    },
    LegacyLaneDescriptor {
        lane: CompatKind::NoPanicAllowlist,
        legacy_filename: "no-panic-allowlist.toml",
        legacy_policy_key: "no-panic-allowlist",
        legacy_input_kind: LegacyInputKind::AllowRules,
        evidence_policy: EvidencePolicy::Required,
        lifecycle_policy: LifecyclePolicy::Partial,
        debt_policy: DebtPolicy::None,
        canonical_shape: ExpectedCanonicalShape {
            finding_kind: FindingKind::Panic,
            policy_dialect: CANONICAL_POLICY,
        },
        closeout_queue: None,
        primary_fixture_file: "no-panic-allowlist.toml",
    },
    LegacyLaneDescriptor {
        lane: CompatKind::PanicBaseline,
        legacy_filename: "no-panic-baseline.toml",
        legacy_policy_key: "no-panic-baseline",
        legacy_input_kind: LegacyInputKind::BaselineEntries,
        evidence_policy: EvidencePolicy::OptionalWithDebtFallback,
        lifecycle_policy: LifecyclePolicy::DebtDefaults,
        debt_policy: DebtPolicy::VisibleBaselineDebt,
        canonical_shape: ExpectedCanonicalShape {
            finding_kind: FindingKind::Panic,
            policy_dialect: CANONICAL_POLICY,
        },
        closeout_queue: Some(CloseoutQueueHints {
            phase: "baseline_debt",
            queue_id: "panic-baseline",
        }),
        primary_fixture_file: "panic-baseline.toml",
    },
    LegacyLaneDescriptor {
        lane: CompatKind::LintException,
        legacy_filename: "clippy-exceptions.toml",
        legacy_policy_key: "clippy-exceptions",
        legacy_input_kind: LegacyInputKind::ClippyPolicyTable,
        evidence_policy: EvidencePolicy::Required,
        lifecycle_policy: LifecyclePolicy::Partial,
        debt_policy: DebtPolicy::VisibleBaselineDebt,
        canonical_shape: ExpectedCanonicalShape {
            finding_kind: FindingKind::LintException,
            policy_dialect: CANONICAL_POLICY,
        },
        closeout_queue: None,
        primary_fixture_file: "lint-exception.toml",
    },
    LegacyLaneDescriptor {
        lane: CompatKind::Unsafe,
        legacy_filename: "unsafe-allowlist.toml",
        legacy_policy_key: "unsafe-allowlist",
        legacy_input_kind: LegacyInputKind::AllowRules,
        evidence_policy: EvidencePolicy::Required,
        lifecycle_policy: LifecyclePolicy::Partial,
        debt_policy: DebtPolicy::MissingEvidenceTodo,
        canonical_shape: ExpectedCanonicalShape {
            finding_kind: FindingKind::Unsafe,
            policy_dialect: CANONICAL_POLICY,
        },
        closeout_queue: None,
        primary_fixture_file: "unsafe.toml",
    },
    LegacyLaneDescriptor {
        lane: CompatKind::Executable,
        legacy_filename: "executable-allowlist.toml",
        legacy_policy_key: "executable-allowlist",
        legacy_input_kind: LegacyInputKind::AllowRules,
        evidence_policy: EvidencePolicy::Required,
        lifecycle_policy: LifecyclePolicy::Full,
        debt_policy: DebtPolicy::None,
        canonical_shape: ExpectedCanonicalShape {
            finding_kind: FindingKind::PolicyException,
            policy_dialect: CANONICAL_POLICY,
        },
        closeout_queue: None,
        primary_fixture_file: "executable.toml",
    },
    LegacyLaneDescriptor {
        lane: CompatKind::Workflow,
        legacy_filename: "workflow-allowlist.toml",
        legacy_policy_key: "workflow-allowlist",
        legacy_input_kind: LegacyInputKind::BaselineEntries,
        evidence_policy: EvidencePolicy::Required,
        lifecycle_policy: LifecyclePolicy::Full,
        debt_policy: DebtPolicy::None,
        canonical_shape: ExpectedCanonicalShape {
            finding_kind: FindingKind::PolicyException,
            policy_dialect: CANONICAL_POLICY,
        },
        closeout_queue: None,
        primary_fixture_file: "workflow.toml",
    },
    LegacyLaneDescriptor {
        lane: CompatKind::DependencySurface,
        legacy_filename: "dependency-surface-allowlist.toml",
        legacy_policy_key: "dependency-surface-allowlist",
        legacy_input_kind: LegacyInputKind::AllowRules,
        evidence_policy: EvidencePolicy::Required,
        lifecycle_policy: LifecyclePolicy::Full,
        debt_policy: DebtPolicy::None,
        canonical_shape: ExpectedCanonicalShape {
            finding_kind: FindingKind::PolicyException,
            policy_dialect: CANONICAL_POLICY,
        },
        closeout_queue: None,
        primary_fixture_file: "dependency-surface.toml",
    },
    LegacyLaneDescriptor {
        lane: CompatKind::Process,
        legacy_filename: "process-allowlist.toml",
        legacy_policy_key: "process-allowlist",
        legacy_input_kind: LegacyInputKind::AllowRules,
        evidence_policy: EvidencePolicy::Required,
        lifecycle_policy: LifecyclePolicy::Full,
        debt_policy: DebtPolicy::None,
        canonical_shape: ExpectedCanonicalShape {
            finding_kind: FindingKind::PolicyException,
            policy_dialect: CANONICAL_POLICY,
        },
        closeout_queue: None,
        primary_fixture_file: "process.toml",
    },
    LegacyLaneDescriptor {
        lane: CompatKind::Network,
        legacy_filename: "network-allowlist.toml",
        legacy_policy_key: "network-allowlist",
        legacy_input_kind: LegacyInputKind::AllowRules,
        evidence_policy: EvidencePolicy::Required,
        lifecycle_policy: LifecyclePolicy::Full,
        debt_policy: DebtPolicy::None,
        canonical_shape: ExpectedCanonicalShape {
            finding_kind: FindingKind::PolicyException,
            policy_dialect: CANONICAL_POLICY,
        },
        closeout_queue: None,
        primary_fixture_file: "network.toml",
    },
];

pub fn all_legacy_lane_descriptors() -> &'static [LegacyLaneDescriptor] {
    LEGACY_LANE_DESCRIPTORS
}

pub fn legacy_lane_descriptor(lane: CompatKind) -> Option<&'static LegacyLaneDescriptor> {
    LEGACY_LANE_DESCRIPTORS
        .iter()
        .find(|descriptor| descriptor.lane == lane)
}

pub fn descriptor_for_compat_kind_id(id: &str) -> Option<&'static LegacyLaneDescriptor> {
    CompatKind::from_compat_kind_id(id).and_then(legacy_lane_descriptor)
}

pub fn descriptor_for_legacy_filename(filename: &str) -> Option<&'static LegacyLaneDescriptor> {
    LEGACY_LANE_DESCRIPTORS
        .iter()
        .find(|descriptor| descriptor.legacy_filename == filename)
}

pub fn descriptor_for_legacy_policy_key(key: &str) -> Option<&'static LegacyLaneDescriptor> {
    LEGACY_LANE_DESCRIPTORS
        .iter()
        .find(|descriptor| descriptor.legacy_policy_key == key)
}

pub fn legacy_policy_filenames() -> impl Iterator<Item = &'static str> {
    LEGACY_LANE_DESCRIPTORS
        .iter()
        .map(|descriptor| descriptor.legacy_filename)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_table_covers_all_compat_kinds() {
        assert_eq!(LEGACY_LANE_DESCRIPTORS.len(), CompatKind::ALL.len());
        for lane in CompatKind::ALL {
            assert!(
                legacy_lane_descriptor(*lane).is_some(),
                "descriptor table missing lane {:?}",
                lane
            );
        }
        for descriptor in LEGACY_LANE_DESCRIPTORS {
            assert!(
                CompatKind::ALL.contains(&descriptor.lane),
                "descriptor table has unexpected lane {:?}",
                descriptor.lane
            );
            assert_eq!(
                descriptor.compat_kind_id(),
                descriptor.lane.compat_kind_id()
            );
        }
    }

    #[test]
    fn descriptor_table_matches_legacy_compat_kind_lookup() {
        for descriptor in LEGACY_LANE_DESCRIPTORS {
            assert_eq!(
                crate::legacy_sources::legacy_compat_kind(descriptor.legacy_filename),
                Some(descriptor.compat_kind_id())
            );
        }
    }

    #[test]
    fn descriptor_filenames_are_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for descriptor in LEGACY_LANE_DESCRIPTORS {
            assert!(
                seen.insert(descriptor.legacy_filename),
                "duplicate legacy filename {}",
                descriptor.legacy_filename
            );
        }
    }

    #[test]
    fn descriptor_policy_keys_are_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for descriptor in LEGACY_LANE_DESCRIPTORS {
            assert!(
                seen.insert(descriptor.legacy_policy_key),
                "duplicate legacy policy key {}",
                descriptor.legacy_policy_key
            );
        }
    }
}
