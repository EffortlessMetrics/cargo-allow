use allow_core::{
    CargoAllowError, CargoAllowResult, Finding, FindingKind, normalize_path, read_text_file_capped,
};
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn generated_findings_from_gitattributes(
    root: impl AsRef<Path>,
) -> CargoAllowResult<Vec<Finding>> {
    let path = root.as_ref().join(".gitattributes");
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let text = read_text_file_capped(&path)
        .map_err(|e| CargoAllowError::new(format!("failed to read {}: {e}", path.display())))?;
    Ok(generated_findings_from_gitattributes_text(&text))
}

pub fn generated_findings_from_gitattributes_text(input: &str) -> Vec<Finding> {
    generated_paths_from_gitattributes(input)
        .into_iter()
        .map(generated_finding)
        .collect()
}

pub fn executable_findings_from_git(root: impl AsRef<Path>) -> CargoAllowResult<Vec<Finding>> {
    let output = Command::new("git")
        .args(["ls-files", "--stage"])
        .current_dir(root.as_ref())
        .output()
        .map_err(|e| CargoAllowError::new(format!("failed to run git ls-files --stage: {e}")))?;
    if !output.status.success() {
        return Err(CargoAllowError::new(format!(
            "git ls-files --stage failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let text = String::from_utf8(output.stdout)
        .map_err(|e| CargoAllowError::new(format!("git ls-files output was not UTF-8: {e}")))?;
    Ok(executable_findings_from_git_stage(&text))
}

fn generated_paths_from_gitattributes(input: &str) -> Vec<PathBuf> {
    input
        .lines()
        .filter_map(generated_path_from_gitattributes_line)
        .map(PathBuf::from)
        .collect()
}

fn generated_path_from_gitattributes_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('#')
        || !trimmed.contains("linguist-generated=true")
    {
        return None;
    }
    trimmed
        .split_whitespace()
        .next()
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(str::to_string)
}

pub fn generated_finding(path: PathBuf) -> Finding {
    let normalized = normalize_path(&path);
    let mut identity = allow_core::StructuralIdentity::new("file", "tracked_file");
    identity.symbol = Some(normalized);
    identity.target_fingerprint = file_fingerprint(&path);
    Finding {
        kind: FindingKind::GeneratedCode,
        family: Some("generated_code".to_string()),
        path,
        span: Some(allow_core::Span { line: 1, column: 1 }),
        identity,
        message: "tracked generated file from .gitattributes".to_string(),
        ledger: None,
    }
}

pub fn executable_findings_from_git_stage(input: &str) -> Vec<Finding> {
    input
        .lines()
        .filter_map(executable_path_from_git_stage_line)
        .map(executable_finding)
        .collect()
}

pub fn executable_findings_from_paths(paths: &[PathBuf]) -> Vec<Finding> {
    paths.iter().cloned().map(executable_finding).collect()
}

fn executable_path_from_git_stage_line(line: &str) -> Option<PathBuf> {
    let (meta, path) = line.split_once('\t')?;
    let mode = meta.split_whitespace().next()?;
    if mode == "100755" && !path.trim().is_empty() {
        Some(PathBuf::from(path.trim()))
    } else {
        None
    }
}

pub fn executable_finding(path: PathBuf) -> Finding {
    let normalized = normalize_path(&path);
    let mut identity = allow_core::StructuralIdentity::new("file", "git_executable_file");
    identity.symbol = Some(normalized);
    identity.target_fingerprint = Some("git-mode:100755".to_string());
    Finding {
        kind: FindingKind::PolicyException,
        family: Some("executable_file".to_string()),
        path,
        span: Some(allow_core::Span { line: 1, column: 1 }),
        identity,
        message: "tracked file has git executable bit".to_string(),
        ledger: None,
    }
}

pub fn file_fingerprint(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .or_else(|| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.to_ascii_lowercase())
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{FindingKind, Span};

    #[test]
    fn generated_gitattributes_text_keeps_only_generated_paths() {
        let findings = generated_findings_from_gitattributes_text(
            "\
# generated outputs
generated/schema.json linguist-generated=true
README.md linguist-documentation=true
 dist/bundle.js   filter=lfs   linguist-generated=true
empty-marker linguist-generated=false
",
        );

        let paths: Vec<PathBuf> = findings.into_iter().map(|finding| finding.path).collect();
        assert_eq!(
            paths,
            vec![
                PathBuf::from("generated/schema.json"),
                PathBuf::from("dist/bundle.js"),
            ]
        );
    }

    #[test]
    fn generated_finding_records_generated_file_identity() {
        let finding = generated_finding(PathBuf::from("generated\\schema.JSON"));

        assert_eq!(finding.kind, FindingKind::GeneratedCode);
        assert_eq!(finding.family.as_deref(), Some("generated_code"));
        assert_eq!(finding.path, PathBuf::from("generated\\schema.JSON"));
        assert_eq!(finding.span, Some(Span { line: 1, column: 1 }));
        assert_eq!(finding.identity.language, "file");
        assert_eq!(finding.identity.ast_kind, "tracked_file");
        assert_eq!(
            finding.identity.symbol.as_deref(),
            Some("generated/schema.JSON")
        );
        assert_eq!(finding.identity.target_fingerprint.as_deref(), Some("json"));
        assert_eq!(
            finding.message,
            "tracked generated file from .gitattributes"
        );
    }

    #[test]
    fn executable_git_stage_keeps_only_executable_file_paths() {
        let findings = executable_findings_from_git_stage(
            "\
100644 abc123 0\tREADME.md
100755 def456 0\tscripts/package-proof.sh
100755 ghi789 0\t
120000 jkl012 0\tscripts/link
malformed without tab
",
        );

        let paths: Vec<PathBuf> = findings.into_iter().map(|finding| finding.path).collect();
        assert_eq!(paths, vec![PathBuf::from("scripts/package-proof.sh")]);
    }

    #[test]
    fn executable_finding_records_executable_file_identity() {
        let finding = executable_finding(PathBuf::from("scripts\\package-proof.sh"));

        assert_eq!(finding.kind, FindingKind::PolicyException);
        assert_eq!(finding.family.as_deref(), Some("executable_file"));
        assert_eq!(finding.path, PathBuf::from("scripts\\package-proof.sh"));
        assert_eq!(finding.span, Some(Span { line: 1, column: 1 }));
        assert_eq!(finding.identity.language, "file");
        assert_eq!(finding.identity.ast_kind, "git_executable_file");
        assert_eq!(
            finding.identity.symbol.as_deref(),
            Some("scripts/package-proof.sh")
        );
        assert_eq!(
            finding.identity.target_fingerprint.as_deref(),
            Some("git-mode:100755")
        );
        assert_eq!(finding.message, "tracked file has git executable bit");
    }

    #[test]
    fn file_fingerprint_prefers_lowercase_extension_then_filename() {
        assert_eq!(
            file_fingerprint(Path::new("generated/schema.JSON")).as_deref(),
            Some("json")
        );
        assert_eq!(
            file_fingerprint(Path::new("Makefile")).as_deref(),
            Some("makefile")
        );
        assert_eq!(file_fingerprint(Path::new("")).as_deref(), None);
    }

    #[test]
    fn generated_findings_from_gitattributes_rejects_oversized_file() {
        let root = temp_root("gitattributes-oversized");
        let path = root.join(".gitattributes");
        let oversized_len = (allow_core::SOURCE_FILE_READ_MAX_BYTES as usize).saturating_add(1);
        let mut bytes = Vec::with_capacity(oversized_len);
        bytes.extend_from_slice(b"generated/schema.json linguist-generated=true\n#");
        bytes.resize(oversized_len, b'g');
        std::fs::write(&path, bytes)
            .unwrap_or_else(|err| std::panic::panic_any(format!("write gitattributes: {err}")));

        let err = generated_findings_from_gitattributes(&root)
            .expect_err("oversized .gitattributes should fail closed");
        let message = err.to_string();
        assert!(
            message.contains("source-read limit") || message.contains("exceeds"),
            "expected size-limit diagnostic, got: {message}"
        );
        let _ = std::fs::remove_dir_all(&root);
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
        std::fs::create_dir_all(&root)
            .unwrap_or_else(|err| std::panic::panic_any(format!("create temp root: {err}")));
        root
    }
}
