//! Seeded negative controls for the source-scanning shim findings (#3376b).
//!
//! Each test builds a minimal fixture tree (crate src dir + manifest) and
//! drives one finding kind; a clean facade passes all four checks.

use super::*;

fn subject(is_family: bool) -> ShimSourceSubject {
    ShimSourceSubject {
        shim_id: "shim-test".to_string(),
        old_identity: "allow-fixture::legacy_facade".to_string(),
        new_identity: "shared-new::replacement".to_string(),
        host_crate: "allow-fixture".to_string(),
        module_path: "legacy_facade".to_string(),
        is_family,
    }
}

struct FixtureDir(std::path::PathBuf);

impl Drop for FixtureDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn write_fixture(facade: &str, manifest: &str, tag: &str) -> Result<FixtureDir, String> {
    let dir = std::env::temp_dir().join(format!("shim-src-guard-{}-{}", std::process::id(), tag));
    let _ = std::fs::remove_dir_all(&dir);
    let src = dir.join("src");
    std::fs::create_dir_all(&src).map_err(|err| format!("mkdir: {err}"))?;
    std::fs::write(src.join("legacy_facade.rs"), facade)
        .map_err(|err| format!("write facade: {err}"))?;
    std::fs::write(dir.join("Cargo.toml"), manifest)
        .map_err(|err| format!("write manifest: {err}"))?;
    Ok(FixtureDir(dir))
}

const CLEAN_MANIFEST: &str = r#"
[package]
name = "allow-fixture"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = "1"
"#;

#[test]
fn clean_facade_passes_all_four_checks() -> Result<(), String> {
    let facade = "#[path = \"snapshot/legacy.rs\"]\nmod legacy_impl;\npub use legacy_impl::*;\n";
    let fixture = write_fixture(facade, CLEAN_MANIFEST, "clean")?;
    let findings = scan_shim_source(
        &subject(false),
        &fixture.0.join("src"),
        CLEAN_MANIFEST,
        &BTreeSet::new(),
    )
    .map_err(|e| format!("{e}"))?;
    if !findings.is_empty() {
        return Err(format!("clean facade must pass, got: {findings:?}"));
    }
    Ok(())
}

#[test]
fn missing_facade_and_module_dir_is_unregistered() -> Result<(), String> {
    let fixture = write_fixture("// placeholder\n", CLEAN_MANIFEST, "missing")?;
    std::fs::remove_file(fixture.0.join("src/legacy_facade.rs"))
        .map_err(|err| format!("remove facade: {err}"))?;
    let findings = scan_shim_source(
        &subject(false),
        &fixture.0.join("src"),
        CLEAN_MANIFEST,
        &BTreeSet::new(),
    )
    .map_err(|e| format!("{e}"))?;
    if !findings
        .iter()
        .any(|f| f.kind == ShimSourceFindingKind::Unregistered)
    {
        return Err(format!("missing path must be unregistered: {findings:?}"));
    }
    Ok(())
}

#[test]
fn facade_without_reexport_is_unregistered() -> Result<(), String> {
    let facade = "// comment only, no forwarding\npub struct Marker;\n";
    let fixture = write_fixture(facade, CLEAN_MANIFEST, "noreexport")?;
    let findings = scan_shim_source(
        &subject(false),
        &fixture.0.join("src"),
        CLEAN_MANIFEST,
        &BTreeSet::new(),
    )
    .map_err(|e| format!("{e}"))?;
    if !findings
        .iter()
        .any(|f| f.kind == ShimSourceFindingKind::Unregistered)
    {
        return Err(format!(
            "non-forwarding facade must be unregistered: {findings:?}"
        ));
    }
    Ok(())
}

#[test]
fn facade_with_public_function_has_semantic_logic() -> Result<(), String> {
    let facade = "pub use other::*;\npub fn compute(value: u32) -> u32 { value + 1 }\n";
    let fixture = write_fixture(facade, CLEAN_MANIFEST, "pubfn")?;
    let findings = scan_shim_source(
        &subject(false),
        &fixture.0.join("src"),
        CLEAN_MANIFEST,
        &BTreeSet::new(),
    )
    .map_err(|e| format!("{e}"))?;
    if !findings
        .iter()
        .any(|f| f.kind == ShimSourceFindingKind::SemanticLogic)
    {
        return Err(format!(
            "pub fn facade must flag semantic logic: {findings:?}"
        ));
    }
    Ok(())
}

#[test]
fn family_shim_exempt_from_facade_checks() -> Result<(), String> {
    let facade = "pub fn delegate() -> u32 { 1 }\n";
    let fixture = write_fixture(facade, CLEAN_MANIFEST, "family")?;
    let findings = scan_shim_source(
        &subject(true),
        &fixture.0.join("src"),
        CLEAN_MANIFEST,
        &BTreeSet::new(),
    )
    .map_err(|e| format!("{e}"))?;
    if findings
        .iter()
        .any(|f| f.kind == ShimSourceFindingKind::SemanticLogic)
    {
        return Err(format!(
            "family shims are exempt from facade checks: {findings:?}"
        ));
    }
    Ok(())
}

#[test]
fn forbidden_production_dependency_is_reverse_dependency() -> Result<(), String> {
    let manifest = r#"
[package]
name = "allow-fixture"
version = "0.1.0"

[dependencies]
cargo-intent = { path = "../cargo-intent" }
"#;
    let fixture = write_fixture("pub use x::*;\n", manifest, "revdep")?;
    let forbidden: BTreeSet<String> = ["cargo-intent".to_string()].into_iter().collect();
    let findings = scan_shim_source(
        &subject(false),
        &fixture.0.join("src"),
        manifest,
        &forbidden,
    )
    .map_err(|e| format!("{e}"))?;
    if !findings
        .iter()
        .any(|f| f.kind == ShimSourceFindingKind::ReverseDependency)
    {
        return Err(format!("forbidden production dep must flag: {findings:?}"));
    }
    Ok(())
}

#[test]
fn dev_dependency_bypass_is_not_reverse_dependency() -> Result<(), String> {
    let manifest = r#"
[package]
name = "allow-fixture"
version = "0.1.0"

[dev-dependencies]
cargo-intent = { path = "../cargo-intent" }
"#;
    let fixture = write_fixture("pub use x::*;\n", manifest, "devbypass")?;
    let forbidden: BTreeSet<String> = ["cargo-intent".to_string()].into_iter().collect();
    let findings = scan_shim_source(
        &subject(false),
        &fixture.0.join("src"),
        manifest,
        &forbidden,
    )
    .map_err(|e| format!("{e}"))?;
    if findings
        .iter()
        .any(|f| f.kind == ShimSourceFindingKind::ReverseDependency)
    {
        return Err(format!("dev bypass must stay allowed: {findings:?}"));
    }
    Ok(())
}

#[test]
fn feature_edge_naming_old_module_is_hidden_feature() -> Result<(), String> {
    let manifest = r#"
[package]
name = "allow-fixture"
version = "0.1.0"

[features]
default = []
legacy = ["legacy-facade-reborn"]
"#;
    let fixture = write_fixture("pub use x::*;\n", manifest, "hiddenfeat")?;
    let findings = scan_shim_source(
        &subject(false),
        &fixture.0.join("src"),
        manifest,
        &BTreeSet::new(),
    )
    .map_err(|e| format!("{e}"))?;
    if !findings
        .iter()
        .any(|f| f.kind == ShimSourceFindingKind::HiddenFeature)
    {
        return Err(format!("old-module feature edge must flag: {findings:?}"));
    }
    Ok(())
}

#[test]
fn subjects_parse_only_active_workspace_shims() -> Result<(), String> {
    let registry = r#"
registry_id = "test"
controlling_issue = 2607
linked_move_ledger = "test"

[[shim]]
id = "shim-active"
old_identity = "allow-fixture::mod_a"
new_identity = "new::a"
status = "active"
kind = "ModuleFacade"
posture = "private"
move_ledger_entry = "entry"
controlling_issue = 2607
latest_allowed_stage = 1
removal_condition = "test"
claim_boundary = "test"

[[shim]]
id = "shim-removed"
old_identity = "allow-fixture::mod_b"
new_identity = "new::b"
status = "removed"
kind = "ModuleFacade"
posture = "private"
move_ledger_entry = "entry"
controlling_issue = 2607
latest_allowed_stage = 1
removal_condition = "test"
claim_boundary = "test"

[[shim]]
id = "shim-external"
old_identity = "ripr::proof_route"
new_identity = "new::c"
status = "active"
kind = "ModuleFacade"
posture = "private"
move_ledger_entry = "entry"
controlling_issue = 2607
latest_allowed_stage = 1
removal_condition = "test"
claim_boundary = "test"
"#;
    let workspace: BTreeSet<String> = ["allow-fixture".to_string()].into_iter().collect();
    let subjects = shim_source_subjects(registry, &workspace).map_err(|e| format!("{e}"))?;
    let active = subjects
        .iter()
        .find(|s| s.shim_id == "shim-active")
        .ok_or("the active workspace shim must be a subject")?;
    if subjects.len() != 1 {
        return Err(format!(
            "only the active workspace shim must be a subject, got {:?}",
            subjects.iter().map(|s| &s.shim_id).collect::<Vec<_>>()
        ));
    }
    let _ = active;
    Ok(())
}

#[test]
fn live_tree_active_shims_pass_the_source_scan() -> Result<(), String> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let findings = scan_shim_sources_at(&root).map_err(|e| format!("{e}"))?;
    if !findings.is_empty() {
        return Err(format!(
            "the live tree's active shims must be clean: {:?}",
            findings.iter().map(|f| f.kind.as_str()).collect::<Vec<_>>()
        ));
    }
    Ok(())
}
