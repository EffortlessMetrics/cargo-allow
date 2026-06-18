use super::*;
use std::io;
use std::path::PathBuf;

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn new(label: &str) -> io::Result<Self> {
        let unique = format!(
            "cargo-allow-profile-resolution-{label}-{}-{}",
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
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn write_file(root: &Path, rel: &str, contents: &str) -> io::Result<()> {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, contents)
}

const PROFILE: &str = "spec-system";
const MINIMAL_CONFIG: &str = r#"
schema_version = "1.0"
profile = "spec-system"
mode = "advisory"

[roots]
proposals = "docs/proposals"
specs = "docs/specs"
adrs = "docs/adr"
plans = "plans"
goals = ".codex/goals"
support_tiers = "docs/status/SUPPORT_TIERS.md"
artifact_ledger = "policy/doc-artifacts.toml"
"#;

#[test]
fn resolve_profile_config_uses_explicit_override() -> io::Result<()> {
    let root = TempRoot::new("explicit")?;
    write_file(&root.path, "policy/spec-system.toml", MINIMAL_CONFIG)?;
    write_file(&root.path, &allow_profile_rel_path(PROFILE), MINIMAL_CONFIG)?;

    let resolved = resolve_profile_config(&root.path, PROFILE, Some(Path::new("custom.toml")));
    assert_eq!(resolved.path.as_deref(), Some("custom.toml"));
    assert_eq!(resolved.provenance, ProfileConfigProvenance::ExplicitConfig);
    assert!(resolved.legacy_conflict_path.is_none());
    Ok(())
}

#[test]
fn resolve_profile_config_prefers_allow_profiles() -> io::Result<()> {
    let root = TempRoot::new("allow-profiles")?;
    write_file(&root.path, &allow_profile_rel_path(PROFILE), MINIMAL_CONFIG)?;
    write_file(&root.path, ALLOW_CONFIG_REL_PATH, MINIMAL_CONFIG)?;
    write_file(
        &root.path,
        &legacy_profile_rel_path(PROFILE),
        MINIMAL_CONFIG,
    )?;

    let resolved = resolve_profile_config(&root.path, PROFILE, None);
    assert_eq!(
        resolved.path.as_deref(),
        Some(allow_profile_rel_path(PROFILE).as_str())
    );
    assert_eq!(resolved.provenance, ProfileConfigProvenance::AllowProfiles);
    assert_eq!(
        resolved.legacy_conflict_path.as_deref(),
        Some(legacy_profile_rel_path(PROFILE).as_str())
    );
    Ok(())
}

#[test]
fn resolve_profile_config_uses_allow_config_before_legacy() -> io::Result<()> {
    let root = TempRoot::new("allow-config")?;
    write_file(&root.path, ALLOW_CONFIG_REL_PATH, MINIMAL_CONFIG)?;
    write_file(
        &root.path,
        &legacy_profile_rel_path(PROFILE),
        MINIMAL_CONFIG,
    )?;

    let resolved = resolve_profile_config(&root.path, PROFILE, None);
    assert_eq!(resolved.path.as_deref(), Some(ALLOW_CONFIG_REL_PATH));
    assert_eq!(resolved.provenance, ProfileConfigProvenance::AllowConfig);
    assert_eq!(
        resolved.legacy_conflict_path.as_deref(),
        Some(legacy_profile_rel_path(PROFILE).as_str())
    );
    Ok(())
}

#[test]
fn resolve_profile_config_falls_back_to_legacy_policy() -> io::Result<()> {
    let root = TempRoot::new("legacy-only")?;
    write_file(
        &root.path,
        &legacy_profile_rel_path(PROFILE),
        MINIMAL_CONFIG,
    )?;

    let resolved = resolve_profile_config(&root.path, PROFILE, None);
    assert_eq!(
        resolved.path.as_deref(),
        Some(legacy_profile_rel_path(PROFILE).as_str())
    );
    assert_eq!(resolved.provenance, ProfileConfigProvenance::LegacyPolicy);
    assert!(resolved.legacy_conflict_path.is_none());
    Ok(())
}

#[test]
fn resolve_profile_config_uses_builtin_default_when_missing() -> io::Result<()> {
    let root = TempRoot::new("builtin-default")?;

    let resolved = resolve_profile_config(&root.path, PROFILE, None);
    assert!(resolved.path.is_none());
    assert_eq!(resolved.provenance, ProfileConfigProvenance::BuiltInDefault);
    assert!(resolved.legacy_conflict_path.is_none());
    Ok(())
}

#[test]
fn profile_config_conflict_message_describes_ambiguity() {
    let resolved = ResolvedProfileConfig {
        path: Some(".allow/profiles/spec-system.toml".to_string()),
        provenance: ProfileConfigProvenance::AllowProfiles,
        legacy_conflict_path: Some("policy/spec-system.toml".to_string()),
    };
    let message = profile_config_conflict_message(&resolved).unwrap_or_default();
    assert!(message.contains(".allow/profiles/spec-system.toml"));
    assert!(message.contains("policy/spec-system.toml"));
    assert!(message.contains("remove or migrate"));
}
