use super::*;
use std::io;

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn new(label: &str) -> io::Result<Self> {
        let unique = format!(
            "cargo-allow-discovery-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let path = std::env::temp_dir().join(unique);
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn write_policy(path: &Path, contents: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, contents)
}

#[test]
fn discover_config_prefers_native_ledger_over_foreign_allow_toml() -> io::Result<()> {
    let root = TempRoot::new("prefer-native")?;
    write_policy(
        &root.path().join("policy/allow.toml"),
        r#"
schema_version = "1"
owner = "repo-policy"
"#,
    )?;
    write_policy(
        &root.path().join("policy/cargo-allow.toml"),
        r#"
schema_version = "0.1"
policy = "cargo-allow"
"#,
    )?;

    let result = discover_config(root.path());
    assert_eq!(
        result
            .selected
            .as_ref()
            .and_then(|path| path.file_name().and_then(|name| name.to_str())),
        Some("cargo-allow.toml")
    );
    assert!(
        result.skipped.is_empty(),
        "native ledger wins without needing to record skipped siblings: {:?}",
        result.skipped
    );
    Ok(())
}

#[test]
fn discover_config_skips_foreign_allow_toml_when_no_native_ledger_exists() -> io::Result<()> {
    let root = TempRoot::new("foreign-only")?;
    write_policy(
        &root.path().join("policy/allow.toml"),
        r#"
schema_version = "1"
owner = "repo-policy"
"#,
    )?;

    let result = discover_config(root.path());
    assert_eq!(result.selected, None);
    let canonical_root = root.path().canonicalize()?;
    let skipped_foreign = result
        .skipped
        .iter()
        .filter(|candidate| candidate.path.starts_with(&canonical_root))
        .collect::<Vec<_>>();
    assert_eq!(
        skipped_foreign.len(),
        1,
        "expected the local foreign candidate to be skipped; root={}, skipped={:?}",
        root.path().display(),
        result.skipped
    );
    assert!(skipped_foreign[0].path.ends_with("policy/allow.toml"));
    assert!(
        skipped_foreign[0]
            .reason
            .contains("missing policy = \"cargo-allow\" marker")
    );
    Ok(())
}

#[test]
fn discover_config_accepts_legacy_allow_toml_without_explicit_policy_marker() -> io::Result<()> {
    let root = TempRoot::new("legacy-marker")?;
    write_policy(
        &root.path().join("policy/allow.toml"),
        r#"
schema_version = "0.1"
"#,
    )?;

    let result = discover_config(root.path());
    assert!(result.selected.is_some());
    assert!(result.skipped.is_empty());
    Ok(())
}

#[test]
fn discover_config_honors_nearest_candidate_order() -> io::Result<()> {
    let root = TempRoot::new("find-config-order")?;
    let workspace = root.path().join("workspace");
    let start = workspace.join("member/src");
    std::fs::create_dir_all(&start)?;
    write_policy(
        &root.path().join("policy/allow.toml"),
        r#"
schema_version = "0.1"
policy = "cargo-allow"
"#,
    )?;
    write_policy(
        &workspace.join(".cargo/allow.toml"),
        r#"
schema_version = "0.1"
policy = "cargo-allow"
"#,
    )?;

    let found = crate::find_config(&start).unwrap_or_else(|| {
        std::panic::panic_any(format!("expected config for {}", start.display()))
    });
    assert!(found.ends_with(".cargo/allow.toml"));
    Ok(())
}

#[test]
fn discover_config_walks_up_before_selecting() -> io::Result<()> {
    let root = TempRoot::new("walk-up")?;
    let start = root.path().join("workspace/member/src");
    std::fs::create_dir_all(&start)?;
    write_policy(
        &root.path().join("policy/cargo-allow.toml"),
        r#"
schema_version = "0.1"
policy = "cargo-allow"
"#,
    )?;

    let result = discover_config(&start);
    assert_eq!(
        result
            .selected
            .as_ref()
            .and_then(|path| path.file_name().and_then(|name| name.to_str())),
        Some("cargo-allow.toml")
    );
    Ok(())
}

#[test]
fn discover_config_prefers_package_metadata_over_conventional_paths() -> io::Result<()> {
    let root = TempRoot::new("package-metadata")?;
    write_policy(
        &root.path().join("policy/allow.toml"),
        "schema_version = \"0.1\"\npolicy = \"cargo-allow\"\n",
    )?;
    let metadata_path = root.path().join("config/cargo-allow.toml");
    write_policy(
        &metadata_path,
        "schema_version = \"0.1\"\npolicy = \"cargo-allow\"\n",
    )?;
    write_policy(
        &root.path().join("Cargo.toml"),
        "[package]\nname = \"metadata-root\"\nversion = \"0.1.0\"\n\n[package.metadata.cargo-allow]\nconfig = \"config/cargo-allow.toml\"\n",
    )?;

    let result = discover_config(root.path());
    assert_eq!(result.selected, Some(metadata_path.canonicalize()?));
    assert!(result.skipped.is_empty());
    Ok(())
}

#[test]
fn discover_config_uses_workspace_metadata_when_package_metadata_is_absent() -> io::Result<()> {
    let root = TempRoot::new("workspace-metadata")?;
    let metadata_path = root.path().join("policy/workspace.toml");
    write_policy(
        &metadata_path,
        "schema_version = \"0.1\"\npolicy = \"cargo-allow\"\n",
    )?;
    write_policy(
        &root.path().join("Cargo.toml"),
        "[workspace]\nmembers = []\n\n[workspace.metadata.cargo-allow]\nconfig = \"policy/workspace.toml\"\n",
    )?;

    let result = discover_config(root.path());
    assert_eq!(result.selected, Some(metadata_path.canonicalize()?));
    assert!(result.skipped.is_empty());
    Ok(())
}

#[test]
fn discover_config_skips_unsafe_metadata_path_and_falls_back() -> io::Result<()> {
    let root = TempRoot::new("metadata-path-safety")?;
    let conventional = root.path().join("policy/allow.toml");
    write_policy(
        &conventional,
        "schema_version = \"0.1\"\npolicy = \"cargo-allow\"\n",
    )?;
    write_policy(
        &root.path().join("Cargo.toml"),
        "[workspace.metadata.cargo-allow]\nconfig = \"../outside.toml\"\n",
    )?;

    let result = discover_config(root.path());
    assert_eq!(result.selected, Some(conventional.canonicalize()?));
    assert_eq!(result.skipped.len(), 1);
    assert!(result.skipped[0].reason.contains("without `..`"));
    Ok(())
}

#[test]
fn discover_config_falls_back_when_metadata_config_is_missing() -> io::Result<()> {
    let root = TempRoot::new("metadata-missing")?;
    let conventional = root.path().join("policy/allow.toml");
    write_policy(
        &conventional,
        "schema_version = \"0.1\"\npolicy = \"cargo-allow\"\n",
    )?;
    write_policy(
        &root.path().join("Cargo.toml"),
        "[workspace.metadata.cargo-allow]\nconfig = \"missing.toml\"\n",
    )?;

    let result = discover_config(root.path());
    assert_eq!(result.selected, Some(conventional.canonicalize()?));
    assert_eq!(result.skipped.len(), 1);
    assert!(result.skipped[0].reason.contains("does not exist"));
    Ok(())
}

#[test]
fn discover_config_falls_back_when_metadata_config_is_foreign() -> io::Result<()> {
    let root = TempRoot::new("metadata-foreign")?;
    let conventional = root.path().join("policy/allow.toml");
    write_policy(
        &conventional,
        "schema_version = \"0.1\"\npolicy = \"cargo-allow\"\n",
    )?;
    write_policy(
        &root.path().join("config/foreign.toml"),
        "schema_version = \"0.1\"\npolicy = \"other-policy\"\n",
    )?;
    write_policy(
        &root.path().join("Cargo.toml"),
        "[workspace.metadata.cargo-allow]\nconfig = \"config/foreign.toml\"\n",
    )?;

    let result = discover_config(root.path());
    assert_eq!(result.selected, Some(conventional.canonicalize()?));
    assert_eq!(result.skipped.len(), 1);
    assert!(result.skipped[0].reason.contains("metadata config"));
    Ok(())
}

#[test]
fn discover_config_rejects_empty_metadata_config() -> io::Result<()> {
    let root = TempRoot::new("metadata-empty")?;
    let conventional = root.path().join("policy/allow.toml");
    write_policy(
        &conventional,
        "schema_version = \"0.1\"\npolicy = \"cargo-allow\"\n",
    )?;
    write_policy(
        &root.path().join("Cargo.toml"),
        "[workspace.metadata.cargo-allow]\nconfig = \"\"\n",
    )?;

    let result = discover_config(root.path());
    assert_eq!(result.selected, Some(conventional.canonicalize()?));
    assert_eq!(result.skipped.len(), 1);
    assert!(result.skipped[0].reason.contains("non-empty"));
    Ok(())
}

#[test]
fn discover_config_falls_back_on_malformed_manifest() -> io::Result<()> {
    let root = TempRoot::new("metadata-malformed")?;
    let conventional = root.path().join("policy/allow.toml");
    write_policy(
        &conventional,
        "schema_version = \"0.1\"\npolicy = \"cargo-allow\"\n",
    )?;
    write_policy(&root.path().join("Cargo.toml"), "[workspace\n")?;

    let result = discover_config(root.path());
    assert_eq!(result.selected, Some(conventional.canonicalize()?));
    assert_eq!(result.skipped.len(), 1);
    assert!(result.skipped[0].reason.contains("could not be parsed"));
    Ok(())
}

#[cfg(unix)]
#[test]
fn discover_config_skips_external_conventional_symlink_before_probe() -> io::Result<()> {
    let root = TempRoot::new("external-conventional-link")?;
    let external = TempRoot::new("external-conventional-target")?;
    write_policy(
        &external.path().join("outside.toml"),
        "schema_version = \"0.1\"\npolicy = \"cargo-allow\"\n",
    )?;
    std::fs::create_dir_all(root.path().join("policy"))?;
    std::os::unix::fs::symlink(
        external.path().join("outside.toml"),
        root.path().join("policy/allow.toml"),
    )?;
    write_policy(
        &root.path().join(".cargo/allow.toml"),
        "schema_version = \"0.1\"\npolicy = \"cargo-allow\"\n",
    )?;

    let result = discover_config(root.path());
    if result.selected != Some(root.path().join(".cargo/allow.toml"))
        || !result.skipped.iter().any(|candidate| {
            candidate.path == root.path().join("policy/allow.toml")
                && candidate.reason.contains("outside its discovery anchor")
        })
    {
        return Err(io::Error::other(format!(
            "external conventional link should be skipped before probing: {result:?}"
        )));
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn discover_config_skips_external_manifest_symlink_before_probe() -> io::Result<()> {
    let root = TempRoot::new("external-manifest-link")?;
    let external = TempRoot::new("external-manifest-target")?;
    write_policy(
        &root.path().join("policy/metadata.toml"),
        "schema_version = \"0.1\"\npolicy = \"cargo-allow\"\n",
    )?;
    write_policy(
        &root.path().join("policy/allow.toml"),
        "schema_version = \"0.1\"\npolicy = \"cargo-allow\"\n",
    )?;
    write_policy(
        &external.path().join("Cargo.toml"),
        "[workspace.metadata.cargo-allow]\nconfig = \"policy/metadata.toml\"\n",
    )?;
    std::os::unix::fs::symlink(
        external.path().join("Cargo.toml"),
        root.path().join("Cargo.toml"),
    )?;

    let result = discover_config(root.path());
    if result.selected != Some(root.path().join("policy/allow.toml"))
        || !result.skipped.iter().any(|candidate| {
            candidate.path == root.path().join("Cargo.toml")
                && candidate.source == SOURCE_CARGO_METADATA
                && candidate.reason.contains("outside its discovery anchor")
        })
    {
        return Err(io::Error::other(format!(
            "external manifest link should be skipped before probing: {result:?}"
        )));
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn discover_config_skips_external_metadata_policy_symlink_before_probe() -> io::Result<()> {
    let root = TempRoot::new("external-metadata-policy-link")?;
    let external = TempRoot::new("external-metadata-policy-target")?;
    write_policy(
        &root.path().join("Cargo.toml"),
        "[package.metadata.cargo-allow]\nconfig = \"policy/metadata.toml\"\n",
    )?;
    write_policy(
        &external.path().join("outside.toml"),
        "schema_version = \"0.1\"\npolicy = \"cargo-allow\"\n",
    )?;
    std::fs::create_dir_all(root.path().join("policy"))?;
    std::os::unix::fs::symlink(
        external.path().join("outside.toml"),
        root.path().join("policy/metadata.toml"),
    )?;
    write_policy(
        &root.path().join("policy/allow.toml"),
        "schema_version = \"0.1\"\npolicy = \"cargo-allow\"\n",
    )?;

    let result = discover_config(root.path());
    if result.selected != Some(root.path().join("policy/allow.toml"))
        || !result.skipped.iter().any(|candidate| {
            candidate.path == root.path().join("policy/metadata.toml")
                && candidate.source == SOURCE_PACKAGE_METADATA
                && candidate.reason.contains("outside its discovery anchor")
                && !candidate
                    .reason
                    .contains(&external.path().display().to_string())
        })
    {
        return Err(io::Error::other(format!(
            "external metadata policy link should be skipped before probing: {result:?}"
        )));
    }
    Ok(())
}
