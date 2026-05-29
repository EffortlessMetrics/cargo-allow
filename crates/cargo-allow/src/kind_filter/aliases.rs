pub(crate) fn is_panic_compat_kind(kind: &str) -> bool {
    matches_alias(
        kind,
        &[
            "panic",
            "panic-family",
            "panic_family",
            "no-panic",
            "no_panic",
            "no-panic-baseline",
            "no_panic_baseline",
        ],
    )
}

pub(crate) fn is_no_panic_allowlist_compat_kind(kind: &str) -> bool {
    matches_alias(
        kind,
        &[
            "no-panic-allowlist",
            "no_panic_allowlist",
            "panic-allowlist",
            "panic_allowlist",
        ],
    )
}

pub(crate) fn is_clippy_compat_kind(kind: &str) -> bool {
    matches_alias(
        kind,
        &[
            "clippy",
            "clippy-exception",
            "clippy-exceptions",
            "clippy_exception",
            "clippy_exceptions",
            "lint",
            "lint-exception",
            "lint_exception",
            "lint-suppression",
            "lint_suppression",
        ],
    )
}

pub(crate) fn is_unsafe_compat_kind(kind: &str) -> bool {
    matches_alias(
        kind,
        &[
            "unsafe",
            "unsafe-allowlist",
            "unsafe_allowlist",
            "unsafe-policy",
            "unsafe_policy",
        ],
    )
}

pub(crate) fn is_executable_compat_kind(kind: &str) -> bool {
    matches_alias(
        kind,
        &[
            "executable",
            "executable_file",
            "executable-file",
            "executable-bit",
            "exec",
        ],
    )
}

pub(crate) fn is_workflow_compat_kind(kind: &str) -> bool {
    matches_alias(
        kind,
        &[
            "workflow",
            "workflows",
            "github_workflow",
            "github-workflow",
            "workflow-action",
        ],
    )
}

pub(crate) fn is_dependency_surface_compat_kind(kind: &str) -> bool {
    matches_alias(
        kind,
        &[
            "dependency",
            "dependencies",
            "dependency_surface",
            "dependency-surface",
            "dependency-surfaces",
            "dep-surface",
            "dep",
        ],
    )
}

pub(crate) fn is_process_compat_kind(kind: &str) -> bool {
    matches_alias(
        kind,
        &[
            "process",
            "processes",
            "process-policy",
            "process_spawn",
            "process-spawn",
            "proc",
        ],
    )
}

pub(crate) fn is_network_compat_kind(kind: &str) -> bool {
    matches_alias(
        kind,
        &[
            "network",
            "net",
            "network-policy",
            "network_destination",
            "network-destination",
        ],
    )
}

fn matches_alias(kind: &str, aliases: &[&str]) -> bool {
    aliases.contains(&kind.trim())
}
