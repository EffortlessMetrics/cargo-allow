use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

fn repo_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let crates_dir = manifest_dir
        .parent()
        .ok_or_else(|| io::Error::other("no crates dir parent"))?;
    let root = crates_dir
        .parent()
        .ok_or_else(|| io::Error::other("no repo root"))?;
    Ok(root.to_path_buf())
}

/// Report the line numbers and quoted values of version requirements that
/// cargo treats as wildcard requirements: a value containing `*`, or a bare
/// major version with no minor/patch (both resolve across every later
/// release in that position, which is exactly what the deny posture
/// forbids).
fn wildcard_requirements(manifest_text: &str) -> Vec<(usize, String)> {
    let mut hits = Vec::new();
    let mut in_dependency_section = false;
    for (offset, line) in manifest_text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_dependency_section = trimmed.contains("dependencies");
            continue;
        }
        if trimmed.starts_with('#') || !trimmed.contains('=') {
            continue;
        }
        // Table form (`version = "1"` inside a dependency table) and
        // shorthand form (`serde = "1"` in a dependency section) are both
        // wildcard carriers; a version key outside a dependency section
        // (e.g. the package's own version) is not a requirement.
        let version_key = trimmed.starts_with("version");
        let shorthand_requirement = in_dependency_section;
        if !version_key && !shorthand_requirement {
            continue;
        }
        let value = match trimmed.split('"').nth(1) {
            Some(value) => value,
            None => continue,
        };
        let bare_major =
            !value.is_empty() && value.chars().all(|c| c.is_ascii_digit()) && !value.contains('.');
        if value.contains('*') || bare_major {
            hits.push((offset + 1, value.to_string()));
        }
    }
    hits
}

fn manifest_paths(root: &Path, out: &mut Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
    let skipped = ["target", ".git", ".changes"];
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        if path.is_dir() {
            if !skipped.contains(&file_name.as_str()) {
                manifest_paths(&path, out)?;
            }
        } else if file_name == "Cargo.toml" {
            out.push(path);
        }
    }
    Ok(())
}

/// The shipped workspace must contain no wildcard dependency requirement:
/// with `deny.toml` pinning `wildcards = "deny"`, one would fail the
/// cargo-deny gate, and the contract here keeps that posture visible in the
/// test suite as well.
#[test]
fn workspace_manifests_have_no_wildcard_requirements() -> Result<(), Box<dyn Error>> {
    let root = repo_root()?;
    let mut manifests = Vec::new();
    manifest_paths(&root, &mut manifests)?;
    assert!(
        manifests.len() >= 10,
        "expected to scan the full workspace, found {} manifests",
        manifests.len()
    );
    let mut violations = Vec::new();
    for manifest in &manifests {
        let text = fs::read_to_string(manifest)?;
        for (line, value) in wildcard_requirements(&text) {
            violations.push(format!("{}:{line} = {value}", manifest.display()));
        }
    }
    assert!(
        violations.is_empty(),
        "wildcard dependency requirements found: {violations:?}"
    );
    Ok(())
}

/// Negative control: the seeded wildcard requirements (shorthand bare
/// major, inline star, and table-form bare major) are exactly what the
/// scanner flags, with actionable line numbers.
#[test]
fn wildcard_scan_flags_seeded_requirements() {
    let seeded = "[dependencies]\nserde = \"1\"\ntokio = { version = \"1.*\", features = [] }\nlibc = \"0.2\"\nlog = \"0.4.*\"\n";
    let hits = wildcard_requirements(seeded);
    let flagged_lines: Vec<usize> = hits.iter().map(|(line, _)| *line).collect();
    assert_eq!(flagged_lines, vec![2, 3, 5], "seeded wildcard hits");
    let flagged_values: Vec<String> = hits.iter().map(|(_, value)| value.clone()).collect();
    assert_eq!(
        flagged_values,
        vec!["1", "1.*", "0.4.*"],
        "seeded wildcard values"
    );
    let clean = "[dependencies]\nlibc = \"0.2\"\nserde = { version = \"1.0.219\" }\n[package]\nversion = \"0.2.0-rc.1\"\n";
    assert!(
        wildcard_requirements(clean).is_empty(),
        "caret, exact, and package-version requirements stay clean"
    );
}

/// Drift guard: the wildcard posture stays denied and the duplicate-version
/// posture stays separately advisory (negative control for accidental scope
/// expansion of the bans rules).
#[test]
fn deny_config_pins_wildcards_to_deny() -> Result<(), Box<dyn Error>> {
    let root = repo_root()?;
    let deny_path = root.join("deny.toml");
    assert!(deny_path.exists(), "deny.toml must exist");
    let content = fs::read_to_string(&deny_path)?;
    assert!(
        content.contains("wildcards = \"deny\""),
        "wildcard dependency requirements must stay denied"
    );
    assert!(
        content.contains("multiple-versions = \"warn\""),
        "duplicate versions must stay separately advisory"
    );
    Ok(())
}
