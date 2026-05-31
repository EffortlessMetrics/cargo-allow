use super::*;

#[test]
fn findings_at_revision_includes_dependency_surface_companions() {
    let root = temp_root("revision-dependency-surface");
    fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = []\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("manifest write: {err}")));
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "initial"]);
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(dependency_surface_entry("Cargo.toml"));

    let findings = findings_at_revision(&root, "HEAD", &cfg)
        .unwrap_or_else(|err| std::panic::panic_any(format!("findings: {err}")));

    assert!(findings.iter().any(|finding| {
        finding.kind == FindingKind::PolicyException
            && finding.family.as_deref() == Some("dependency_surface")
            && finding.path.as_path() == Path::new("Cargo.toml")
    }));
    fs::remove_dir_all(root).unwrap_or_else(|err| std::panic::panic_any(format!("cleanup: {err}")));
}

#[test]
fn findings_at_revision_includes_generated_gitattributes_companions() {
    let root = temp_root("revision-generated-gitattributes");
    fs::create_dir_all(root.join("generated"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("generated dir: {err}")));
    fs::write(
        root.join(".gitattributes"),
        "generated/schema.json linguist-generated=true\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("gitattributes write: {err}")));
    fs::write(root.join("generated").join("schema.json"), "{}\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("generated file write: {err}")));
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "initial"]);
    let mut cfg = AllowConfig::empty();
    cfg.allow
        .push(generated_code_entry("generated/schema.json"));

    let findings = findings_at_revision(&root, "HEAD", &cfg)
        .unwrap_or_else(|err| std::panic::panic_any(format!("findings: {err}")));

    assert!(findings.iter().any(|finding| {
        finding.kind == FindingKind::GeneratedCode
            && finding.family.as_deref() == Some("generated_code")
            && finding.path.as_path() == Path::new("generated/schema.json")
    }));
    fs::remove_dir_all(root).unwrap_or_else(|err| std::panic::panic_any(format!("cleanup: {err}")));
}

#[test]
fn findings_at_revision_includes_workflow_companions() {
    let root = temp_root("revision-workflow");
    let workflow_dir = root.join(".github").join("workflows");
    fs::create_dir_all(&workflow_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("workflow dir: {err}")));
    fs::write(
        workflow_dir.join("ci.yml"),
        "steps:\n  - uses: actions/checkout@v4\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("workflow write: {err}")));
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "initial"]);
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(workflow_entry(
        "workflow-ci",
        "github_workflow",
        "github_workflow",
        ".github/workflows/ci.yml",
        None,
    ));
    cfg.allow.push(workflow_entry(
        "workflow-action-checkout",
        "workflow_external_action",
        "github_action_uses",
        ".github/workflows/ci.yml",
        Some("action:actions/checkout@v4"),
    ));

    let findings = findings_at_revision(&root, "HEAD", &cfg)
        .unwrap_or_else(|err| std::panic::panic_any(format!("findings: {err}")));

    assert!(findings.iter().any(|finding| {
        finding.kind == FindingKind::PolicyException
            && finding.family.as_deref() == Some("github_workflow")
            && finding.path.as_path() == Path::new(".github/workflows/ci.yml")
    }));
    assert!(findings.iter().any(|finding| {
        finding.kind == FindingKind::PolicyException
            && finding.family.as_deref() == Some("workflow_external_action")
            && finding.identity.target_fingerprint.as_deref() == Some("action:actions/checkout@v4")
    }));
    fs::remove_dir_all(root).unwrap_or_else(|err| std::panic::panic_any(format!("cleanup: {err}")));
}

#[test]
fn findings_at_revision_includes_config_companions() {
    let root = temp_root("revision-config-companions");
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["commit", "--allow-empty", "-m", "initial"]);
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(config_policy_entry(
        "proc-cargo-test",
        "process_spawn",
        ".github/workflows/ci.yml",
        "cargo test",
        "process:cargo test",
    ));
    cfg.allow.push(config_policy_entry(
        "net-crates-io",
        "network_destination",
        "policy/network-allowlist.toml",
        "crates.io lane build",
        "network:crates.io:auth:false:lane:build",
    ));

    let findings = findings_at_revision(&root, "HEAD", &cfg)
        .unwrap_or_else(|err| std::panic::panic_any(format!("findings: {err}")));

    assert!(findings.iter().any(|finding| {
        finding.kind == FindingKind::PolicyException
            && finding.family.as_deref() == Some("process_spawn")
            && finding.identity.target_fingerprint.as_deref() == Some("process:cargo test")
    }));
    assert!(findings.iter().any(|finding| {
        finding.kind == FindingKind::PolicyException
            && finding.family.as_deref() == Some("network_destination")
            && finding.identity.target_fingerprint.as_deref()
                == Some("network:crates.io:auth:false:lane:build")
    }));
    fs::remove_dir_all(root).unwrap_or_else(|err| std::panic::panic_any(format!("cleanup: {err}")));
}

#[test]
fn findings_at_revision_includes_executable_tree_mode_companions() {
    let root = temp_root("revision-executable");
    let script_dir = root.join("scripts");
    fs::create_dir_all(&script_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("script dir: {err}")));
    fs::write(script_dir.join("package-proof.sh"), "#!/usr/bin/env bash\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("script write: {err}")));
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "."]);
    git(
        &root,
        &["update-index", "--chmod=+x", "scripts/package-proof.sh"],
    );
    git(&root, &["commit", "-m", "initial"]);
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(executable_entry("scripts/package-proof.sh"));

    let findings = findings_at_revision(&root, "HEAD", &cfg)
        .unwrap_or_else(|err| std::panic::panic_any(format!("findings: {err}")));

    assert!(findings.iter().any(|finding| {
        finding.kind == FindingKind::PolicyException
            && finding.family.as_deref() == Some("executable_file")
            && finding.path.as_path() == Path::new("scripts/package-proof.sh")
            && finding.identity.target_fingerprint.as_deref() == Some("git-mode:100755")
    }));
    fs::remove_dir_all(root).unwrap_or_else(|err| std::panic::panic_any(format!("cleanup: {err}")));
}
