use super::*;
use crate::revision::scan_at_revision_with_after_resolve;
use allow_core::FileFamilyRule;
use std::process::Command;

#[test]
fn scan_at_revision_keeps_independent_revision_and_rust_facts() {
    let root = temp_root("revision-scan-completeness");
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("src dir: {err}")));
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("manifest write: {err}")));
    fs::write(root.join("src/lib.rs"), "fn valid() {}\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("base Rust write: {err}")));
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "base"]);

    let base = scan_at_revision(&root, "HEAD", &AllowConfig::empty())
        .unwrap_or_else(|err| std::panic::panic_any(format!("base scan: {err}")));
    fs::write(root.join("src/lib.rs"), "fn invalid( {}\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("head Rust write: {err}")));
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "head"]);

    let head = scan_at_revision(&root, "HEAD", &AllowConfig::empty())
        .unwrap_or_else(|err| std::panic::panic_any(format!("head scan: {err}")));
    assert_ne!(base.revision.commit, head.revision.commit);
    assert_ne!(base.selected_source_closure, head.selected_source_closure);
    assert_eq!(base.rust_files_considered, 1);
    assert_eq!(base.rust_files_scanned, 1);
    assert_eq!(base.rust_files_skipped, 0);
    assert_eq!(base.rust_files_with_parse_errors, 0);
    assert_eq!(base.scanner_completeness, "complete");
    assert_eq!(head.rust_files_considered, 1);
    assert_eq!(head.rust_files_scanned, 1);
    assert_eq!(head.rust_files_skipped, 0);
    assert_eq!(head.rust_files_with_parse_errors, 1);
    assert_eq!(head.scanner_completeness, "partial");
    assert_eq!(base.inventory_completeness, "complete");
    assert_eq!(head.inventory_completeness, "complete");

    fs::remove_dir_all(root).unwrap_or_else(|err| std::panic::panic_any(format!("cleanup: {err}")));
}

#[test]
fn scan_binds_all_outputs_to_revision_resolved_before_ref_retarget() -> Result<(), String> {
    let trace_path = std::env::temp_dir().join(format!(
        "cargo-allow-2515-trace-{}.json",
        std::process::id()
    ));
    let executable = std::env::current_exe().map_err(|err| format!("test executable: {err}"))?;
    let output = Command::new(executable)
        .args([
            "--exact",
            "tests::revision_findings::trace_child_scan_binds_all_outputs_to_revision_resolved_before_ref_retarget",
            "--ignored",
            "--nocapture",
        ])
        .env("GIT_TRACE2_EVENT", &trace_path)
        .output()
        .map_err(|err| format!("trace child: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "trace child failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let trace = fs::read_to_string(&trace_path).map_err(|err| format!("trace read: {err}"))?;
    let resolution_events = trace
        .lines()
        .filter(|line| {
            line.contains("refs/heads/moving^{commit}") && line.contains("\"event\":\"start\"")
        })
        .count();
    if resolution_events != 2 {
        return Err(format!(
            "expected one symbolic revision resolution per scan, observed {resolution_events}"
        ));
    }
    for _ in 0..20 {
        match fs::remove_file(&trace_path) {
            Ok(()) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(25)),
        }
    }
    Ok(())
}

#[test]
#[ignore = "invoked by the parent Trace2 isolation test"]
fn trace_child_scan_binds_all_outputs_to_revision_resolved_before_ref_retarget() {
    let root = temp_root("revision-after-resolve-retarget");
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("src dir: {err}")));
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("manifest: {err}")));
    fs::write(root.join("src/lib.rs"), "fn stable() {}\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("source: {err}")));
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "initial"]);
    git(&root, &["branch", "moving"]);

    let baseline = scan_at_revision(&root, "refs/heads/moving", &AllowConfig::empty())
        .unwrap_or_else(|err| std::panic::panic_any(format!("baseline scan: {err}")));
    let resolved_commit = baseline.revision.commit.clone();
    let resolved_tree = baseline.revision.tree.clone();
    let resolved_closure = baseline.selected_source_closure.clone();
    let retargeted = scan_at_revision_with_after_resolve(
        &root,
        "refs/heads/moving",
        &AllowConfig::empty(),
        |root| {
            fs::write(root.join("src/lib.rs"), "fn broken( {}\n")
                .unwrap_or_else(|err| std::panic::panic_any(format!("retarget source: {err}")));
            git(&root.to_path_buf(), &["add", "."]);
            git(&root.to_path_buf(), &["commit", "-m", "retarget"]);
            git(
                &root.to_path_buf(),
                &["update-ref", "refs/heads/moving", "HEAD"],
            );
        },
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("retargeted scan: {err}")));
    assert_eq!(retargeted.revision.commit, resolved_commit);
    assert_eq!(retargeted.revision.requested, "refs/heads/moving");
    assert_eq!(retargeted.revision.tree, resolved_tree);
    assert_eq!(retargeted.selected_source_closure, resolved_closure);
    assert_eq!(retargeted.findings, baseline.findings);
    assert_eq!(retargeted.rust_files_with_parse_errors, 0);
    for _ in 0..20 {
        match fs::remove_dir_all(&root) {
            Ok(()) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            Err(error) => std::panic::panic_any(format!("cleanup: {error}")),
        }
    }
}

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
