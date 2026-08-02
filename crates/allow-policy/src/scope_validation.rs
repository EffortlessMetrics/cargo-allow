use allow_core::{
    AllowEntry, BUILTIN_FILE_FAMILY_CODES, CargoAllowError, CargoAllowResult, FileFamilyRule,
    WorkspaceConfig, WorkspaceMode, normalize_path,
};
use std::collections::BTreeSet;
use std::path::Path;
use std::str::FromStr;

use crate::source_tree_scope::{normalize_source_tree_scope, validate_glob, validate_path_scope};
use crate::text_validation::validate_required_text;

pub(crate) fn validate_workspace(workspace: &WorkspaceConfig) -> CargoAllowResult<()> {
    validate_path_scope("workspace root", Path::new(&workspace.root))?;
    validate_required_text("workspace inventory", &workspace.inventory)?;
    if !matches!(
        workspace.inventory.as_str(),
        "git-tracked" | "filesystem-fallback" | "filesystem-include-untracked"
    ) {
        return Err(CargoAllowError::new(format!(
            "unsupported workspace inventory `{}`",
            workspace.inventory
        )));
    }
    // default_mode is validated against the canonical WorkspaceMode enum in
    // allow-core (single source of truth) so typos like `no_new` are rejected
    // here and in `AllowConfig::validate`.
    validate_required_text("workspace default_mode", &workspace.default_mode)?;
    WorkspaceMode::from_str(&workspace.default_mode)?;
    for pattern in &workspace.ignored {
        validate_glob("source-tree ignored glob", pattern)?;
    }
    for pattern in &workspace.generated {
        validate_glob("source-tree generated glob", pattern)?;
    }
    validate_unique_workspace_globs("source-tree ignored glob", &workspace.ignored)?;
    validate_unique_workspace_globs("source-tree generated glob", &workspace.generated)?;
    validate_file_family_rules(&workspace.file_families)?;
    Ok(())
}

fn validate_file_family_rules(rules: &[FileFamilyRule]) -> CargoAllowResult<()> {
    let mut ids = BTreeSet::new();
    let mut definitions = BTreeSet::new();
    for (index, rule) in rules.iter().enumerate() {
        let label = format!("workspace file_family[{}]", index + 1);
        validate_identifier(&format!("{label} id"), &rule.id)?;
        validate_family_code(&label, &rule.family)?;
        validate_required_text(&format!("{label} reason"), &rule.reason)?;
        validate_glob(&format!("{label} glob"), &rule.glob)?;

        if !ids.insert(rule.id.clone()) {
            return Err(CargoAllowError::new(format!(
                "duplicate workspace file_family id `{}`",
                rule.id
            )));
        }
        let definition = (rule.family.clone(), normalize_source_tree_scope(&rule.glob));
        if !definitions.insert(definition) {
            return Err(CargoAllowError::new(format!(
                "duplicate workspace file_family definition `{}` / `{}`",
                rule.family,
                normalize_source_tree_scope(&rule.glob)
            )));
        }
    }
    Ok(())
}

fn validate_identifier(label: &str, value: &str) -> CargoAllowResult<()> {
    validate_required_text(label, value)?;
    let mut chars = value.chars();
    let valid = chars.next().is_some_and(|ch| ch.is_ascii_lowercase())
        && chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_');
    if !valid {
        return Err(CargoAllowError::new(format!(
            "{label} must start with a lowercase ASCII letter and contain only lowercase ASCII letters, digits, `-`, or `_`"
        )));
    }
    Ok(())
}

fn validate_family_code(label: &str, family: &str) -> CargoAllowResult<()> {
    validate_required_text(&format!("{label} family"), family)?;
    let valid = family.split('.').all(|segment| {
        let mut chars = segment.chars();
        chars.next().is_some_and(|ch| ch.is_ascii_lowercase())
            && chars
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '-' | '_'))
    });
    if !valid {
        return Err(CargoAllowError::new(format!(
            "{label} family must start with a lowercase ASCII letter and contain only lowercase ASCII letters, digits, `.`, `-`, or `_`"
        )));
    }
    if BUILTIN_FILE_FAMILY_CODES.contains(&family) {
        return Err(CargoAllowError::new(format!(
            "{label} family `{family}` is reserved for the built-in file classifier"
        )));
    }
    Ok(())
}

pub(crate) fn validate_allow_entry_scope(entry: &AllowEntry) -> CargoAllowResult<()> {
    if entry.path.is_none() && entry.glob.is_none() && entry.selector.glob.is_none() {
        return Err(CargoAllowError::new(format!(
            "{} has no path or glob",
            entry.id
        )));
    }
    if let Some(path) = &entry.path {
        validate_path_scope(&entry.id, path)?;
        // #1835: `path = "."` covers the entire source tree, which is the
        // same escape valve that `glob = "**"` is already rejected for (see
        // validate_supported_glob_syntax). Reject it here so the "narrower
        // scope" rule is not bypassable via path. The workspace root
        // legitimately uses `.` and is validated separately in
        // validate_workspace, not through this entry-level check.
        if path.to_string_lossy() == "." {
            return Err(CargoAllowError::new(format!(
                "{} path `.` covers the entire source tree; use a narrower path or glob scope",
                entry.id
            )));
        }
    }
    if let Some(glob) = &entry.glob {
        validate_glob(&format!("{} glob", entry.id), glob)?;
    }
    if let Some(glob) = &entry.selector.glob {
        validate_glob(&format!("{} selector glob", entry.id), glob)?;
    }
    validate_scope_consistency(entry)
}

pub(crate) fn validate_scope_consistency(entry: &AllowEntry) -> CargoAllowResult<()> {
    if entry.path.is_some() && entry.glob.is_some() {
        return Err(CargoAllowError::new(format!(
            "{} must not define both path and glob",
            entry.id
        )));
    }
    if let (Some(path), Some(selector_glob)) = (&entry.path, &entry.selector.glob) {
        let path = normalize_path(path);
        let selector_glob = normalize_source_tree_scope(selector_glob);
        if selector_glob != path {
            return Err(CargoAllowError::new(format!(
                "{} selector glob `{selector_glob}` must match path `{path}` or omit one scope",
                entry.id
            )));
        }
    }
    if let (Some(glob), Some(selector_glob)) = (&entry.glob, &entry.selector.glob) {
        let glob = normalize_source_tree_scope(glob);
        let selector_glob = normalize_source_tree_scope(selector_glob);
        if selector_glob != glob {
            return Err(CargoAllowError::new(format!(
                "{} selector glob `{selector_glob}` must match glob `{glob}` or omit one scope",
                entry.id
            )));
        }
    }
    Ok(())
}

fn validate_unique_workspace_globs(label: &str, patterns: &[String]) -> CargoAllowResult<()> {
    let mut seen = BTreeSet::new();
    for (index, pattern) in patterns.iter().enumerate() {
        let normalized = normalize_source_tree_scope(pattern);
        if !seen.insert(normalized.clone()) {
            return Err(CargoAllowError::new(format!(
                "duplicate {label} `{normalized}` at position {}",
                index + 1
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{FindingKind, Lifecycle, Selector};
    use std::path::PathBuf;

    fn err_text(result: CargoAllowResult<()>) -> String {
        match result {
            Ok(()) => String::new(),
            Err(err) => err.to_string(),
        }
    }

    fn entry(id: &str) -> AllowEntry {
        AllowEntry {
            id: id.to_string(),
            kind: FindingKind::NonRustFile,
            family: Some("documentation".to_string()),
            path: Some(PathBuf::from("docs/policy.md")),
            glob: None,
            owner: "docs".to_string(),
            classification: "documentation".to_string(),
            reason: "Policy doc is tracked for review.".to_string(),
            evidence: Vec::new(),
            links: Vec::new(),
            occurrence_limit: None,
            lifecycle: Lifecycle::empty(),
            selector: Selector::default(),
            last_seen: None,
        }
    }

    #[test]
    fn validate_workspace_accepts_supported_modes_and_unique_globs() {
        let workspace = WorkspaceConfig {
            root: ".".to_string(),
            inventory: "git-tracked".to_string(),
            ignored: vec!["target/**".to_string(), ".git/**".to_string()],
            generated: vec!["vendor/**".to_string(), "target/generated/**".to_string()],
            default_mode: "no-new".to_string(),
            file_families: Vec::new(),
        };

        assert!(validate_workspace(&workspace).is_ok());

        for default_mode in ["audit", "strict", "release"] {
            let workspace = WorkspaceConfig {
                default_mode: default_mode.to_string(),
                ..workspace.clone()
            };
            assert!(validate_workspace(&workspace).is_ok(), "{default_mode}");
        }
    }

    #[test]
    fn validate_workspace_rejects_unsupported_inventory_and_default_mode() {
        let workspace = WorkspaceConfig {
            inventory: "filesystem".to_string(),
            ..WorkspaceConfig::default()
        };
        assert_eq!(
            err_text(validate_workspace(&workspace)),
            "unsupported workspace inventory `filesystem`"
        );

        let workspace = WorkspaceConfig {
            default_mode: "permissive".to_string(),
            ..WorkspaceConfig::default()
        };
        assert_eq!(
            err_text(validate_workspace(&workspace)),
            "unsupported workspace default_mode `permissive`"
        );
    }

    #[test]
    fn validate_workspace_accepts_safe_custom_file_family_rules() {
        let workspace = WorkspaceConfig {
            file_families: vec![FileFamilyRule {
                id: "model-artifact".to_string(),
                family: "ml_model".to_string(),
                glob: "models/**/*.onnx".to_string(),
                reason: "Govern versioned model artifacts.".to_string(),
            }],
            ..WorkspaceConfig::default()
        };

        assert!(validate_workspace(&workspace).is_ok());
    }

    #[test]
    fn validate_workspace_rejects_invalid_custom_file_family_identity() {
        let invalid_id = WorkspaceConfig {
            file_families: vec![FileFamilyRule {
                id: "Model Artifact".to_string(),
                family: "ml_model".to_string(),
                glob: "models/**/*.onnx".to_string(),
                reason: "Govern versioned model artifacts.".to_string(),
            }],
            ..WorkspaceConfig::default()
        };
        assert!(
            err_text(validate_workspace(&invalid_id)).contains("id must start with a lowercase")
        );

        let invalid_family = WorkspaceConfig {
            file_families: vec![FileFamilyRule {
                id: "model-artifact".to_string(),
                family: "ml..model".to_string(),
                glob: "models/**/*.onnx".to_string(),
                reason: "Govern versioned model artifacts.".to_string(),
            }],
            ..WorkspaceConfig::default()
        };
        assert!(err_text(validate_workspace(&invalid_family)).contains("family must start"));
    }

    #[test]
    fn validate_workspace_rejects_reserved_duplicate_and_unsafe_file_family_rules() {
        let reserved = WorkspaceConfig {
            file_families: vec![FileFamilyRule {
                id: "docs".to_string(),
                family: "documentation".to_string(),
                glob: "custom/**/*.md".to_string(),
                reason: "Reserved family test.".to_string(),
            }],
            ..WorkspaceConfig::default()
        };
        assert!(err_text(validate_workspace(&reserved)).contains("reserved"));

        let duplicate = WorkspaceConfig {
            file_families: vec![
                FileFamilyRule {
                    id: "model-a".to_string(),
                    family: "ml_model".to_string(),
                    glob: "models/**/*.onnx".to_string(),
                    reason: "First rule.".to_string(),
                },
                FileFamilyRule {
                    id: "model-a".to_string(),
                    family: "ml_model".to_string(),
                    glob: "models/**/*.onnx".to_string(),
                    reason: "Duplicate rule.".to_string(),
                },
            ],
            ..WorkspaceConfig::default()
        };
        assert!(err_text(validate_workspace(&duplicate)).contains("duplicate"));

        let unsafe_glob = WorkspaceConfig {
            file_families: vec![FileFamilyRule {
                id: "outside".to_string(),
                family: "external_artifact".to_string(),
                glob: "../**/*.onnx".to_string(),
                reason: "Unsafe path test.".to_string(),
            }],
            ..WorkspaceConfig::default()
        };
        assert!(err_text(validate_workspace(&unsafe_glob)).contains("parent directory"));
    }

    #[test]
    fn validate_allow_entry_scope_accepts_path_glob_and_selector_glob_forms() {
        let path_entry = entry("path-entry");
        assert!(validate_allow_entry_scope(&path_entry).is_ok());

        let mut matching_path_selector = entry("path-selector");
        matching_path_selector.selector.glob = Some(r"docs\policy.md".to_string());
        assert!(validate_allow_entry_scope(&matching_path_selector).is_ok());

        let mut glob_entry = entry("glob-entry");
        glob_entry.path = None;
        glob_entry.glob = Some(r"docs\**".to_string());
        glob_entry.selector.glob = Some("docs/**".to_string());
        assert!(validate_allow_entry_scope(&glob_entry).is_ok());
    }

    #[test]
    fn validate_allow_entry_scope_rejects_missing_and_invalid_scope() {
        let mut missing = entry("missing-scope");
        missing.path = None;
        missing.glob = None;
        missing.selector.glob = None;
        assert_eq!(
            err_text(validate_allow_entry_scope(&missing)),
            "missing-scope has no path or glob"
        );

        let mut invalid_path = entry("invalid-path");
        invalid_path.path = Some(PathBuf::from("../outside.md"));
        assert_eq!(
            err_text(validate_allow_entry_scope(&invalid_path)),
            "invalid-path path must not contain parent directory segments"
        );
    }

    #[test]
    fn validate_allow_entry_scope_rejects_entire_source_tree_path_dot() {
        // #1835: `path = "."` covers the entire source tree, which is the
        // same escape valve that `glob = "**"` is rejected for. The entry
        // must use a narrower scope. The workspace root legitimately uses
        // `.` and is validated separately via validate_workspace.
        let mut dot_entry = entry("dot-entry");
        dot_entry.path = Some(PathBuf::from("."));
        dot_entry.selector.glob = Some(".".to_string());
        assert_eq!(
            err_text(validate_allow_entry_scope(&dot_entry)),
            "dot-entry path `.` covers the entire source tree; use a narrower path or glob scope"
        );
    }

    #[test]
    fn validate_scope_consistency_rejects_conflicts_and_accepts_normalized_matches() {
        let mut path_and_glob = entry("path-and-glob");
        path_and_glob.glob = Some("docs/**".to_string());
        assert_eq!(
            err_text(validate_scope_consistency(&path_and_glob)),
            "path-and-glob must not define both path and glob"
        );

        let mut path_selector_mismatch = entry("path-selector-mismatch");
        path_selector_mismatch.selector.glob = Some("docs/**".to_string());
        assert_eq!(
            err_text(validate_scope_consistency(&path_selector_mismatch)),
            "path-selector-mismatch selector glob `docs/**` must match path `docs/policy.md` or omit one scope"
        );

        let mut glob_selector_mismatch = entry("glob-selector-mismatch");
        glob_selector_mismatch.path = None;
        glob_selector_mismatch.glob = Some("docs/**".to_string());
        glob_selector_mismatch.selector.glob = Some("scripts/**".to_string());
        assert_eq!(
            err_text(validate_scope_consistency(&glob_selector_mismatch)),
            "glob-selector-mismatch selector glob `scripts/**` must match glob `docs/**` or omit one scope"
        );

        let mut normalized_match = entry("normalized-match");
        normalized_match.selector.glob = Some(r"docs\policy.md".to_string());
        assert!(validate_scope_consistency(&normalized_match).is_ok());
    }

    #[test]
    fn validate_unique_workspace_globs_accepts_unique_and_reports_normalized_duplicates() {
        let unique = vec!["target/**".to_string(), "vendor/**".to_string()];
        assert!(validate_unique_workspace_globs("ignored", &unique).is_ok());

        let duplicate = vec!["vendor/**".to_string(), r"vendor\**".to_string()];
        assert_eq!(
            err_text(validate_unique_workspace_globs("generated", &duplicate)),
            "duplicate generated `vendor/**` at position 2"
        );
    }
}
