use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use toml::Value;

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

/// Wildcard version requirements resolve across every later release in the
/// wildcard position. A dependency is a wildcard carrier when its `version`
/// key (shorthand string value or table value) contains `*` or is a bare
/// major with no minor/patch. Entries without a `version` key (pure path
/// edges) are cargo-deny's surface, not this scanner's: the packaging
/// contracts require those to stay version-less (#3908).
fn is_wildcard_requirement(value: &Value) -> bool {
    match value {
        Value::String(text) => is_wildcard_text(text),
        Value::Table(table) => table
            .get("version")
            .and_then(Value::as_str)
            .map(is_wildcard_text)
            .unwrap_or(false),
        _ => false,
    }
}

fn is_wildcard_text(text: &str) -> bool {
    text.contains('*') || (!text.is_empty() && text.chars().all(|c| c.is_ascii_digit()))
}

const DEPENDENCY_SECTIONS: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];

/// Collect the (section, name, version) triples of every wildcard
/// requirement in a parsed manifest, walking target-specific tables too.
fn wildcard_requirements(
    manifest: &Value,
    manifest_label: &str,
) -> Result<Vec<String>, Box<dyn Error>> {
    let Some(table) = manifest.as_table() else {
        return Ok(Vec::new());
    };
    let mut hits = Vec::new();
    let mut dep_tables: Vec<(String, &Value)> = Vec::new();
    for (key, value) in table {
        if DEPENDENCY_SECTIONS.contains(&key.as_str()) {
            dep_tables.push((key.clone(), value));
        } else if key == "target" {
            if let Some(targets) = value.as_table() {
                for (target_name, target_value) in targets {
                    if let Some(target_table) = target_value.as_table() {
                        for (key, value) in target_table {
                            if DEPENDENCY_SECTIONS.contains(&key.as_str()) {
                                dep_tables.push((format!("{key} (target {target_name})"), value));
                            }
                        }
                    }
                }
            }
        }
    }
    for (section, section_value) in dep_tables {
        let Some(entries) = section_value.as_table() else {
            continue;
        };
        for (name, value) in entries {
            if is_wildcard_requirement(value) {
                hits.push(format!("{manifest_label}: {section}/{name}"));
            }
        }
    }
    Ok(hits)
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
/// cargo-deny's wildcard posture is warning-only while the deny flip waits
/// on the packaging contracts (#3908), so this contract is the visible
/// no-wildcards guarantee in the test suite until that flip can land.
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
        let label = manifest.display().to_string();
        let text = fs::read_to_string(manifest)?;
        let parsed: Value = toml::from_str(&text)
            .map_err(|error| io::Error::other(format!("parse {label}: {error}")))?;
        violations.extend(wildcard_requirements(&parsed, &label)?);
    }
    assert!(
        violations.is_empty(),
        "wildcard dependency requirements found: {violations:?}"
    );
    Ok(())
}

/// Negative control: reordered inline tables, table-form bare majors, and
/// shorthand bare majors are all flagged, while caret, exact, and the
/// package's own version stay clean.
#[test]
fn wildcard_scan_flags_seeded_requirements() -> Result<(), Box<dyn Error>> {
    let seeded: Value = toml::from_str(
        r#"
[dependencies]
serde = "1"
libc = "0.2"
tokio = { path = "../tokio", version = "1.*" }
log = { version = "0.4.*", features = [] }
"#,
    )
    .map_err(|error| io::Error::other(format!("seed fixture: {error}")))?;
    let hits = wildcard_requirements(&seeded, "seeded")?;
    let mut carriers = hits.clone();
    carriers.sort();
    assert_eq!(
        carriers,
        vec![
            "seeded: dependencies/log",
            "seeded: dependencies/serde",
            "seeded: dependencies/tokio",
        ],
        "seeded wildcard carriers"
    );

    let clean: Value = toml::from_str(
        r#"
[package]
version = "0.2.0-rc.1"

[dependencies]
libc = "0.2"
serde = { version = "1.0.219", features = ["derive"] }
tokio = { path = "../tokio", version = "1.38.1" }
"#,
    )
    .map_err(|error| io::Error::other(format!("clean fixture: {error}")))?;
    assert!(
        wildcard_requirements(&clean, "clean")?.is_empty(),
        "caret, exact, and pinned path requirements stay clean"
    );
    Ok(())
}

/// Drift guard: the wildcard posture stays warning-only (documented blocker:
/// the exact-candidate packaging contracts #2372/#2925 require version-less
/// path edges, which the deny posture would forbid) and the duplicate-version
/// posture stays separately advisory (negative control for accidental scope
/// expansion of the bans rules). Parsed as TOML so a comment cannot satisfy
/// the assertion.
#[test]
fn deny_config_documents_the_wildcard_posture() -> Result<(), Box<dyn Error>> {
    let root = repo_root()?;
    let deny_path = root.join("deny.toml");
    assert!(deny_path.exists(), "deny.toml must exist");
    let deny: Value = toml::from_str(&fs::read_to_string(&deny_path)?)
        .map_err(|error| io::Error::other(format!("parse deny.toml: {error}")))?;
    let wildcards = deny
        .get("bans")
        .and_then(|bans| bans.get("wildcards"))
        .and_then(Value::as_str)
        .ok_or_else(|| io::Error::other("deny.toml bans.wildcards must be set"))?;
    assert_eq!(
        wildcards, "warn",
        "the wildcard posture must stay explicitly warning-only while the \
         deny flip is blocked on the packaging contracts (#3908)"
    );
    let multiple_versions = deny
        .get("bans")
        .and_then(|bans| bans.get("multiple-versions"))
        .and_then(Value::as_str)
        .ok_or_else(|| io::Error::other("deny.toml bans.multiple-versions must be set"))?;
    assert_eq!(
        multiple_versions, "warn",
        "duplicate versions must stay separately advisory"
    );
    Ok(())
}
