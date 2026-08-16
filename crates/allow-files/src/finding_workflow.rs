use allow_core::{
    CargoAllowError, CargoAllowResult, Finding, FindingKind, normalize_path, read_text_file_capped,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

pub fn workflow_findings_from_files(root: impl AsRef<Path>) -> CargoAllowResult<Vec<Finding>> {
    let root = root.as_ref();
    let workflows_dir = root.join(".github").join("workflows");
    if !workflows_dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut paths = Vec::new();
    for entry in fs::read_dir(&workflows_dir).map_err(|e| {
        CargoAllowError::new(format!("failed to read {}: {e}", workflows_dir.display()))
    })? {
        let entry = entry.map_err(|e| {
            CargoAllowError::new(format!(
                "failed to read {} entry: {e}",
                workflows_dir.display()
            ))
        })?;
        let path = entry.path();
        if is_workflow_path(&path) {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            paths.push(PathBuf::from(rel));
        }
    }
    paths.sort();

    let mut findings = Vec::new();
    for path in paths {
        findings.push(workflow_file_finding(path.clone()));
        let full_path = root.join(
            path.to_string_lossy()
                .replace('/', std::path::MAIN_SEPARATOR_STR),
        );
        let text = read_text_file_capped(&full_path).map_err(|e| {
            CargoAllowError::new(format!("failed to read {}: {e}", full_path.display()))
        })?;
        let uses = text
            .lines()
            .filter_map(extract_workflow_uses)
            .collect::<BTreeSet<_>>();
        findings.extend(
            uses.into_iter()
                .map(|action| workflow_action_finding(path.clone(), action)),
        );
    }
    Ok(findings)
}

pub fn workflow_findings_from_sources(sources: Vec<(PathBuf, String)>) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (path, text) in sources {
        findings.push(workflow_file_finding(path.clone()));
        let uses = text
            .lines()
            .filter_map(extract_workflow_uses)
            .collect::<BTreeSet<_>>();
        findings.extend(
            uses.into_iter()
                .map(|action| workflow_action_finding(path.clone(), action)),
        );
    }
    findings
}

pub fn workflow_file_finding(path: PathBuf) -> Finding {
    let normalized = normalize_path(&path);
    let mut identity = allow_core::StructuralIdentity::new("workflow", "github_workflow");
    identity.symbol = Some(normalized);
    Finding {
        kind: FindingKind::PolicyException,
        family: Some("github_workflow".to_string()),
        path,
        span: Some(allow_core::Span { line: 1, column: 1 }),
        identity,
        message: "GitHub Actions workflow file".to_string(),
        ledger: None,
    }
}

pub fn workflow_action_finding(path: PathBuf, action: String) -> Finding {
    let normalized = normalize_path(&path);
    let mut identity = allow_core::StructuralIdentity::new("workflow", "github_action_uses");
    identity.symbol = Some(workflow_action_symbol(&normalized, &action));
    identity.target_fingerprint = Some(format!("action:{action}"));
    Finding {
        kind: FindingKind::PolicyException,
        family: Some("workflow_external_action".to_string()),
        path,
        span: Some(allow_core::Span { line: 1, column: 1 }),
        identity,
        message: format!("GitHub Actions workflow uses external action {action}"),
        ledger: None,
    }
}

fn extract_workflow_uses(line: &str) -> Option<String> {
    let trimmed = line.trim().trim_start_matches('-').trim_start();
    let stripped = trimmed.strip_prefix("uses:")?;
    let value = stripped.trim();
    if value.is_empty() {
        return None;
    }
    let no_comment = value.split('#').next().unwrap_or(value).trim();
    if no_comment.is_empty() {
        None
    } else {
        Some(no_comment.to_string())
    }
}

pub fn workflow_action_symbol(path: &str, action: &str) -> String {
    format!("{path} uses {action}")
}

fn is_workflow_path(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("yml" | "yaml")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_file_finding_preserves_normalized_identity() {
        let finding = workflow_file_finding(PathBuf::from(".github\\workflows\\ci.yml"));

        assert_eq!(finding.kind, FindingKind::PolicyException);
        assert_eq!(finding.family.as_deref(), Some("github_workflow"));
        assert_eq!(finding.path, PathBuf::from(".github\\workflows\\ci.yml"));
        assert_eq!(
            finding.span.as_ref().map(|span| (span.line, span.column)),
            Some((1, 1))
        );
        assert_eq!(finding.identity.language, "workflow");
        assert_eq!(finding.identity.ast_kind, "github_workflow");
        assert_eq!(
            finding.identity.symbol.as_deref(),
            Some(".github/workflows/ci.yml")
        );
        assert_eq!(finding.message, "GitHub Actions workflow file");
    }

    #[test]
    fn workflow_action_finding_preserves_action_identity() {
        let finding = workflow_action_finding(
            PathBuf::from(".github\\workflows\\ci.yml"),
            "actions/checkout@v6.0.2".to_string(),
        );

        assert_eq!(finding.kind, FindingKind::PolicyException);
        assert_eq!(finding.family.as_deref(), Some("workflow_external_action"));
        assert_eq!(finding.identity.language, "workflow");
        assert_eq!(finding.identity.ast_kind, "github_action_uses");
        assert_eq!(
            finding.identity.symbol.as_deref(),
            Some(".github/workflows/ci.yml uses actions/checkout@v6.0.2")
        );
        assert_eq!(
            finding.identity.target_fingerprint.as_deref(),
            Some("action:actions/checkout@v6.0.2")
        );
        assert_eq!(
            finding.message,
            "GitHub Actions workflow uses external action actions/checkout@v6.0.2"
        );
    }

    #[test]
    fn extract_workflow_uses_trims_yaml_prefixes_and_comments() {
        assert_eq!(
            extract_workflow_uses("  - uses: actions/checkout@v4 # checkout"),
            Some("actions/checkout@v4".to_string())
        );
        assert_eq!(
            extract_workflow_uses("uses: dtolnay/rust-toolchain@stable"),
            Some("dtolnay/rust-toolchain@stable".to_string())
        );
        assert_eq!(extract_workflow_uses("  - name: build"), None);
        assert_eq!(
            extract_workflow_uses("  - uses:   # empty after comment"),
            None
        );
        assert_eq!(extract_workflow_uses("  - uses:"), None);
    }

    #[test]
    fn workflow_action_symbol_and_path_filter_are_stable() {
        assert_eq!(
            workflow_action_symbol(".github/workflows/ci.yml", "actions/checkout@v4"),
            ".github/workflows/ci.yml uses actions/checkout@v4"
        );
        assert!(is_workflow_path(Path::new(".github/workflows/ci.yml")));
        assert!(is_workflow_path(Path::new(
            ".github/workflows/release.yaml"
        )));
        assert!(!is_workflow_path(Path::new(".github/workflows/readme.md")));
        assert!(!is_workflow_path(Path::new(
            ".github/workflows/no-extension"
        )));
    }

    #[test]
    fn workflow_findings_from_files_rejects_oversized_workflow() {
        let root = temp_root("workflow-oversized");
        let workflows = root.join(".github").join("workflows");
        fs::create_dir_all(&workflows)
            .unwrap_or_else(|err| std::panic::panic_any(format!("create workflows dir: {err}")));
        let workflow_path = workflows.join("ci.yml");
        let oversized_len = (allow_core::SOURCE_FILE_READ_MAX_BYTES as usize).saturating_add(1);
        let mut bytes = Vec::with_capacity(oversized_len);
        bytes.extend_from_slice(b"name: ci\n#");
        bytes.resize(oversized_len, b'w');
        fs::write(&workflow_path, bytes)
            .unwrap_or_else(|err| std::panic::panic_any(format!("write workflow: {err}")));

        let err =
            workflow_findings_from_files(&root).expect_err("oversized workflow should fail closed");
        let message = err.to_string();
        assert!(
            message.contains("source-read limit") || message.contains("exceeds"),
            "expected size-limit diagnostic, got: {message}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    fn temp_root(label: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!(
            "cargo-allow-legacy-{label}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&root)
            .unwrap_or_else(|err| std::panic::panic_any(format!("create temp root: {err}")));
        root
    }
}
