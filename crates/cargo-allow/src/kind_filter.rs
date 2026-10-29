use allow_core::{AllowEntry, CargoAllowResult, Finding, FindingKind};
use std::str::FromStr;

#[derive(Debug, Clone, Copy)]
pub(crate) struct KindFilter {
    pub(crate) kind: FindingKind,
    pub(crate) family: FamilyFilter,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum FamilyFilter {
    Any,
    Exact(&'static str),
    Workflow,
}

impl KindFilter {
    pub(crate) fn matches_finding(self, finding: &Finding) -> bool {
        finding.kind == self.kind && self.family.matches(finding.family.as_deref())
    }

    pub(crate) fn matches_entry(self, entry: &AllowEntry) -> bool {
        entry.kind == self.kind && self.family.matches(entry.family.as_deref())
    }
}

impl FamilyFilter {
    pub(crate) fn matches(self, family: Option<&str>) -> bool {
        match self {
            Self::Any => true,
            Self::Exact(expected) => family == Some(expected),
            Self::Workflow => {
                matches!(family, Some("github_workflow" | "workflow_external_action"))
            }
        }
    }
}

pub(crate) fn parse_kind_filter(kind: &str) -> CargoAllowResult<KindFilter> {
    let normalized = kind.trim().to_ascii_lowercase();
    if is_panic_compat_kind(&normalized) {
        return Ok(KindFilter {
            kind: FindingKind::Panic,
            family: FamilyFilter::Any,
        });
    }
    if is_no_panic_allowlist_compat_kind(&normalized) {
        return Ok(KindFilter {
            kind: FindingKind::Panic,
            family: FamilyFilter::Any,
        });
    }
    if is_clippy_compat_kind(&normalized) {
        return Ok(KindFilter {
            kind: FindingKind::LintException,
            family: FamilyFilter::Any,
        });
    }
    if is_unsafe_compat_kind(&normalized) {
        return Ok(KindFilter {
            kind: FindingKind::Unsafe,
            family: FamilyFilter::Any,
        });
    }
    if is_executable_compat_kind(&normalized) {
        return Ok(KindFilter {
            kind: FindingKind::PolicyException,
            family: FamilyFilter::Exact("executable_file"),
        });
    }
    if is_workflow_compat_kind(&normalized) {
        return Ok(KindFilter {
            kind: FindingKind::PolicyException,
            family: FamilyFilter::Workflow,
        });
    }
    if is_dependency_surface_compat_kind(&normalized) {
        return Ok(KindFilter {
            kind: FindingKind::PolicyException,
            family: FamilyFilter::Exact("dependency_surface"),
        });
    }
    if is_process_compat_kind(&normalized) {
        return Ok(KindFilter {
            kind: FindingKind::PolicyException,
            family: FamilyFilter::Exact("process_spawn"),
        });
    }
    if is_network_compat_kind(&normalized) {
        return Ok(KindFilter {
            kind: FindingKind::PolicyException,
            family: FamilyFilter::Exact("network_destination"),
        });
    }
    Ok(KindFilter {
        kind: FindingKind::from_str(kind)?,
        family: FamilyFilter::Any,
    })
}

pub(crate) fn parse_kind_filter_arg(kind: &str) -> Result<String, String> {
    parse_kind_filter(kind)
        .map(|_| kind.to_string())
        .map_err(|_| format!("unknown kind `{kind}`; supported kinds: {SUPPORTED_KIND_FILTERS}"))
}

/// Whether a parsed kind filter selects one of the compat-only surfaces:
/// `executable`, `workflow`, `dependency-surface`, `process`, or `network`
/// (any alias). Each maps to a `policy_exception` family whose findings are
/// produced only by the legacy allowlist sensors behind `--compat`.
pub(crate) fn is_compat_only_kind_filter(parsed: KindFilter) -> bool {
    parsed.kind == FindingKind::PolicyException && !matches!(parsed.family, FamilyFilter::Any)
}

/// Fail closed when a compat-only kind filter reaches a lane that cannot wire
/// the kind's sensor in (#4632), mirroring the #1948 flag-with-no-effect
/// rejection: filtering `workflow` (or executable/process/network/
/// dependency-surface) into a non-compat check/audit accepts a kind whose
/// sensor lives behind `--compat`, so the gate scans nothing for it and stays
/// permanently green. `check`/`audit` pass `compat_available: args.compat`;
/// `diff` and `why` have no compat lane at all and always pass `false`.
///
/// Kinds that fail to parse return `Ok` so the caller's own kind parsing
/// keeps producing the canonical unknown-kind error. The compat lane itself
/// is untouched: a missing legacy policy still surfaces as the compat
/// loader's E0009 policy-demand error.
pub(crate) fn reject_compat_only_kind_filter(
    command: &str,
    compat_available: bool,
    kind: Option<&str>,
) -> CargoAllowResult<()> {
    let Some(kind) = kind else {
        return Ok(());
    };
    let Ok(parsed) = parse_kind_filter(kind) else {
        return Ok(());
    };
    if !is_compat_only_kind_filter(parsed) {
        return Ok(());
    }
    // The compat lane wires the sensor in, so it keeps its #2433-era
    // behavior: the missing legacy policy surfaces as the E0009 policy-demand
    // error from the compat loader.
    if compat_available {
        return Ok(());
    }
    Err(allow_core::CargoAllowError::with_kind(
        allow_core::CargoAllowErrorKind::Usage,
        format!(
            "--kind {kind} has no findings outside the --compat lane for {command}: this kind's findings come from the legacy allowlist sensors behind --compat, so a non-compat run would scan nothing and report a permanently green gate; this kind is reachable only from `check --compat --kind {kind}` or `audit --compat --kind {kind}`"
        ),
    ))
}

const SUPPORTED_KIND_FILTERS: &str = "panic, unsafe, lint-exception, non-rust, generated, policy-exception, no-panic-allowlist, executable, workflow, dependency-surface, process, network";

pub(crate) fn is_panic_compat_kind(kind: &str) -> bool {
    matches!(
        kind.trim(),
        "panic"
            | "panic-family"
            | "panic_family"
            | "no-panic"
            | "no_panic"
            | "no-panic-baseline"
            | "no_panic_baseline"
    )
}

pub(crate) fn is_no_panic_allowlist_compat_kind(kind: &str) -> bool {
    matches!(
        kind.trim(),
        "no-panic-allowlist" | "no_panic_allowlist" | "panic-allowlist" | "panic_allowlist"
    )
}

pub(crate) fn is_clippy_compat_kind(kind: &str) -> bool {
    matches!(
        kind.trim(),
        "clippy"
            | "clippy-exception"
            | "clippy-exceptions"
            | "clippy_exception"
            | "clippy_exceptions"
            | "lint"
            | "lint-exception"
            | "lint_exception"
            | "lint-suppression"
            | "lint_suppression"
    )
}

pub(crate) fn is_unsafe_compat_kind(kind: &str) -> bool {
    matches!(
        kind.trim(),
        "unsafe" | "unsafe-allowlist" | "unsafe_allowlist" | "unsafe-policy" | "unsafe_policy"
    )
}

pub(crate) fn is_executable_compat_kind(kind: &str) -> bool {
    matches!(
        kind.trim(),
        "executable" | "executable_file" | "executable-file" | "executable-bit" | "exec"
    )
}

pub(crate) fn is_workflow_compat_kind(kind: &str) -> bool {
    matches!(
        kind.trim(),
        "workflow" | "workflows" | "github_workflow" | "github-workflow" | "workflow-action"
    )
}

pub(crate) fn is_dependency_surface_compat_kind(kind: &str) -> bool {
    matches!(
        kind.trim(),
        "dependency"
            | "dependencies"
            | "dependency_surface"
            | "dependency-surface"
            | "dependency-surfaces"
            | "dep-surface"
            | "dep"
    )
}

pub(crate) fn is_process_compat_kind(kind: &str) -> bool {
    matches!(
        kind.trim(),
        "process" | "processes" | "process-policy" | "process_spawn" | "process-spawn" | "proc"
    )
}

pub(crate) fn is_network_compat_kind(kind: &str) -> bool {
    matches!(
        kind.trim(),
        "network" | "net" | "network-policy" | "network_destination" | "network-destination"
    )
}

/// A grouped kind entry for the vocabulary command. Each group maps a
/// canonical kind filter name to the aliases an operator can type.
pub(crate) struct KindGroup {
    pub(crate) canonical: &'static str,
    pub(crate) aliases: &'static [&'static str],
    /// True when the kind's findings are produced only by the legacy
    /// allowlist sensors behind `--compat` (#4632). A non-compat
    /// check/audit/diff/why run filtered to such a kind scans nothing for it
    /// and would report a permanently green gate, so those lanes reject it.
    pub(crate) compat_only: bool,
}

/// The complete kind-filter vocabulary, grouped by canonical name. This is the
/// single source of truth for the `vocabulary` command and for help text.
pub(crate) const KIND_GROUPS: &[KindGroup] = &[
    KindGroup {
        canonical: "panic",
        aliases: &[
            "panic",
            "panic-family",
            "panic_family",
            "no-panic",
            "no_panic",
            "no-panic-baseline",
            "no_panic_baseline",
            "no-panic-allowlist",
            "no_panic_allowlist",
            "panic-allowlist",
            "panic_allowlist",
        ],
        compat_only: false,
    },
    KindGroup {
        canonical: "unsafe",
        aliases: &[
            "unsafe",
            "unsafe-allowlist",
            "unsafe_allowlist",
            "unsafe-policy",
            "unsafe_policy",
        ],
        compat_only: false,
    },
    KindGroup {
        canonical: "lint-exception",
        aliases: &[
            "lint-exception",
            "lint_exception",
            "clippy",
            "clippy-exception",
            "clippy-exceptions",
            "clippy_exception",
            "clippy_exceptions",
            "lint",
            "lint-suppression",
            "lint_suppression",
        ],
        compat_only: false,
    },
    KindGroup {
        canonical: "non-rust",
        aliases: &[
            "non-rust",
            "non_rust",
            "non-rust-file",
            "non_rust_file",
            "file",
        ],
        compat_only: false,
    },
    KindGroup {
        canonical: "generated",
        aliases: &["generated", "generated-code", "generated_code"],
        compat_only: false,
    },
    KindGroup {
        canonical: "policy-exception",
        aliases: &["policy-exception", "policy_exception", "policy"],
        compat_only: false,
    },
    KindGroup {
        canonical: "executable",
        aliases: &[
            "executable",
            "executable-file",
            "executable_file",
            "executable-bit",
            "exec",
        ],
        compat_only: true,
    },
    KindGroup {
        canonical: "workflow",
        aliases: &[
            "workflow",
            "workflows",
            "github-workflow",
            "github_workflow",
            "workflow-action",
        ],
        compat_only: true,
    },
    KindGroup {
        canonical: "dependency-surface",
        aliases: &[
            "dependency-surface",
            "dependency_surface",
            "dependency-surfaces",
            "dependency",
            "dependencies",
            "dep-surface",
            "dep",
        ],
        compat_only: true,
    },
    KindGroup {
        canonical: "process",
        aliases: &[
            "process",
            "processes",
            "process-policy",
            "process-spawn",
            "process_spawn",
            "proc",
        ],
        compat_only: true,
    },
    KindGroup {
        canonical: "network",
        aliases: &[
            "network",
            "net",
            "network-policy",
            "network-destination",
            "network_destination",
        ],
        compat_only: true,
    },
];

#[cfg(test)]
#[path = "kind_filter_tests.rs"]
mod tests;
