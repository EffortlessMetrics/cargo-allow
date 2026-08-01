/// Finding families whose evidence is derived from repository-wide context.
///
/// Keep this registry beside the shared finding model so diagnostic commands
/// do not silently treat a newly added companion family as path-local.
pub const REPOSITORY_WIDE_FAMILIES: &[&str] = &[
    "generated_code",
    "executable_file",
    "github_workflow",
    "workflow_external_action",
    "dependency_surface",
    "process_spawn",
    "network_destination",
];

pub fn is_repository_wide_family(family: &str) -> bool {
    REPOSITORY_WIDE_FAMILIES.contains(&family)
}

#[cfg(test)]
mod tests {
    use super::is_repository_wide_family;

    #[test]
    fn companion_family_registry_covers_repository_wide_context() {
        assert!(is_repository_wide_family("generated_code"));
        assert!(is_repository_wide_family("network_destination"));
        assert!(!is_repository_wide_family("unwrap"));
    }
}
