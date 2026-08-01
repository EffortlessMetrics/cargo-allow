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
    },
    KindGroup {
        canonical: "generated",
        aliases: &["generated", "generated-code", "generated_code"],
    },
    KindGroup {
        canonical: "policy-exception",
        aliases: &["policy-exception", "policy_exception", "policy"],
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
    },
];

#[cfg(test)]
#[path = "kind_filter_tests.rs"]
mod tests;
