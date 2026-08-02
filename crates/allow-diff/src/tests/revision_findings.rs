use super::*;
use allow_core::FileFamilyRule;

#[test]
fn findings_at_revision_preserves_source_package_context() {
    let root = temp_root("revision-package-context");
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("src dir: {err}")));
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("manifest write: {err}")));
    fs::write(
        root.join("src").join("lib.rs"),
        "fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("rust write: {err}")));
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "initial"]);

    let findings = findings_at_revision(&root, "HEAD", &AllowConfig::empty())
        .unwrap_or_else(|err| std::panic::panic_any(format!("findings: {err}")));

    let unwrap = findings
        .iter()
        .find(|finding| finding.family.as_deref() == Some("unwrap"))
        .unwrap_or_else(|| std::panic::panic_any("expected unwrap finding"));
    assert_eq!(unwrap.identity.crate_name.as_deref(), Some("demo"));
    fs::remove_dir_all(root).unwrap_or_else(|err| std::panic::panic_any(format!("cleanup: {err}")));
}

#[test]
fn findings_at_revision_applies_workspace_ignored_globs() {
    let root = temp_root("revision-ignored");
    fs::create_dir_all(root.join("ignored"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("ignored dir: {err}")));
    fs::write(
        root.join("ignored").join("panic.rs"),
        "fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("ignored rust write: {err}")));
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "initial"]);
    let mut cfg = AllowConfig::empty();
    cfg.workspace.ignored.push("ignored/**".to_string());

    let findings = findings_at_revision(&root, "HEAD", &cfg)
        .unwrap_or_else(|err| std::panic::panic_any(format!("findings: {err}")));

    assert!(
        findings
            .iter()
            .all(|finding| finding.path.as_path() != Path::new("ignored/panic.rs"))
    );
    assert!(
        !findings
            .iter()
            .any(|finding| finding.family.as_deref() == Some("unwrap"))
    );
    fs::remove_dir_all(root).unwrap_or_else(|err| std::panic::panic_any(format!("cleanup: {err}")));
}

#[test]
fn findings_at_revision_applies_custom_file_family_rules() {
    let root = temp_root("revision-custom-file-family");
    fs::create_dir_all(root.join("models").join("release"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("model dir: {err}")));
    fs::write(
        root.join("models").join("release").join("manifest.json"),
        "{}\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("model manifest: {err}")));
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "initial"]);

    let mut cfg = AllowConfig::empty();
    cfg.workspace.file_families.push(FileFamilyRule {
        id: "release-manifest".to_string(),
        family: "release_metadata".to_string(),
        glob: "models/release/*.json".to_string(),
        reason: "Release metadata is governed separately.".to_string(),
    });

    let findings = findings_at_revision(&root, "HEAD", &cfg)
        .unwrap_or_else(|err| std::panic::panic_any(format!("findings: {err}")));
    let finding = findings
        .iter()
        .find(|finding| finding.path == Path::new("models/release/manifest.json"))
        .unwrap_or_else(|| std::panic::panic_any("expected custom family finding"));

    assert_eq!(finding.family.as_deref(), Some("release_metadata"));
    assert!(finding.message.contains("rule release-manifest"));
    fs::remove_dir_all(root).unwrap_or_else(|err| std::panic::panic_any(format!("cleanup: {err}")));
}
