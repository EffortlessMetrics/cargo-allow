use super::*;
use crate::{CargoAllowCli, CargoAllowCommand, HumanJsonFormat};
use clap::Parser;
use serde_json::Value;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(unix)]
fn write_external_migrate_output_with_test_hook(
    held_target: &effortless_repo_edit::MutationTarget,
    requested: &std::path::Path,
    repository_root: &std::path::Path,
    contents: &str,
    force: bool,
    hook: &mut dyn FnMut(),
) -> CargoAllowResult<()> {
    write_external_migrate_output_with_hook(
        held_target,
        requested,
        repository_root,
        contents,
        force,
        Some(hook),
    )
}

#[test]
fn clap_parses_repo_policy_migrate() {
    let parsed = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "migrate",
        "--repo-policy",
        "policy",
        "--out",
        "target/allow.toml",
        "--force",
        "--summary-format",
        "json",
        "--summary-output",
        "target/migrate-summary.json",
    ]))
    .unwrap_or_else(|err| {
        std::panic::panic_any(format!("CLI should parse repo-policy migrate: {err}"))
    });

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::Migrate(MigrateArgs {
            repo_policy: Some(dir),
            out,
            force: true,
            summary_format: HumanJsonFormat::Json,
            summary_output: Some(summary_output),
            ..
        })) if dir == Path::new("policy")
            && out == Path::new("target/allow.toml")
            && summary_output == Path::new("target/migrate-summary.json")
    ));
}

#[test]
fn migrate_requires_one_input_source() {
    let missing = cmd_migrate(&MigrateArgs {
        root: RootArgs::default(),
        from: None,
        repo_policy: None,
        out: PathBuf::from("target/unused.toml"),
        force: false,
        update: false,
        summary_format: HumanJsonFormat::Human,
        summary_output: None,
    })
    .expect_err("missing input source should fail");
    assert!(
        missing
            .to_string()
            .contains("pass --from <file> or --repo-policy <dir>")
    );

    let conflicting = cmd_migrate(&MigrateArgs {
        root: RootArgs::default(),
        from: Some(PathBuf::from("legacy.toml")),
        repo_policy: Some(PathBuf::from("policy")),
        out: PathBuf::from("target/unused.toml"),
        force: false,
        update: false,
        summary_format: HumanJsonFormat::Human,
        summary_output: None,
    })
    .expect_err("conflicting input sources should fail");
    assert!(
        conflicting
            .to_string()
            .contains("pass either --from or --repo-policy")
    );
}

#[test]
fn migrate_refuses_existing_output_without_force() {
    let dir = migrate_fixture_dir();
    let policy_dir = dir.join("policy");
    fs::create_dir_all(&policy_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
    fs::write(
        policy_dir.join("network-allowlist.toml"),
        network_policy_fixture_text(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("network fixture write: {err}")));
    let out = dir.join("allow.toml");
    fs::write(&out, "existing")
        .unwrap_or_else(|err| std::panic::panic_any(format!("existing output write: {err}")));

    let err = cmd_migrate(&MigrateArgs {
        root: RootArgs::default(),
        from: None,
        repo_policy: Some(policy_dir),
        out,
        force: false,
        update: false,
        summary_format: HumanJsonFormat::Human,
        summary_output: None,
    })
    .expect_err("existing output should require --force");
    assert!(err.to_string().contains("use --force to overwrite"));
}

#[test]
fn migrate_repo_policy_writes_combined_canonical_policy() {
    let dir = migrate_fixture_dir();
    let policy_dir = dir.join("policy");
    fs::create_dir_all(&policy_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
    fs::write(
        policy_dir.join("process-allowlist.toml"),
        process_policy_fixture_text(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("process fixture write: {err}")));
    fs::write(
        policy_dir.join("network-allowlist.toml"),
        network_policy_fixture_text(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("network fixture write: {err}")));
    let out = dir.join("allow.toml");

    cmd_migrate(&MigrateArgs {
        root: RootArgs::default(),
        from: None,
        repo_policy: Some(policy_dir),
        out: out.clone(),
        force: false,
        update: false,
        summary_format: HumanJsonFormat::Human,
        summary_output: None,
    })
    .unwrap_or_else(|err| std::panic::panic_any(format!("repo-policy migrate: {err}")));

    let rendered = fs::read_to_string(&out)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read migrated policy: {err}")));
    assert!(rendered.contains("process_spawn"));
    assert!(rendered.contains("network_destination"));
}

#[test]
fn migrate_external_output_uses_held_target_and_preserves_force_backup() {
    let dir = migrate_fixture_dir();
    let policy_dir = dir.join("policy");
    fs::create_dir_all(&policy_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
    fs::write(
        policy_dir.join("network-allowlist.toml"),
        network_policy_fixture_text(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("network fixture write: {err}")));
    let dir_name = dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| std::panic::panic_any("fixture directory has no final component"));
    let out = dir.with_file_name(format!("{dir_name}-external.toml"));
    let out_parent = out
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::panic::panic_any("external output has no parent"));
    let out_name = out
        .file_name()
        .map(OsStr::to_os_string)
        .unwrap_or_else(|| std::panic::panic_any("external output has no final component"));
    let out_alias = out_parent.join(".").join(out_name);
    let _ = fs::remove_file(&out);
    let _ = fs::remove_file(out.with_extension("toml.bak"));

    cmd_migrate(&MigrateArgs {
        root: RootArgs {
            root: Some(dir.clone()),
        },
        from: None,
        repo_policy: Some(policy_dir.clone()),
        out: out_alias.clone(),
        force: false,
        update: false,
        summary_format: HumanJsonFormat::Human,
        summary_output: None,
    })
    .unwrap_or_else(|err| std::panic::panic_any(format!("external create: {err}")));
    let first = fs::read_to_string(&out)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read external output: {err}")));
    assert!(first.contains("network_destination"));
    let changed_network = network_policy_fixture_text().replace("crates.io", "example.invalid");
    fs::write(policy_dir.join("network-allowlist.toml"), changed_network)
        .unwrap_or_else(|err| std::panic::panic_any(format!("change network fixture: {err}")));

    cmd_migrate(&MigrateArgs {
        root: RootArgs { root: Some(dir) },
        from: None,
        repo_policy: Some(policy_dir),
        out: out_alias,
        force: true,
        update: false,
        summary_format: HumanJsonFormat::Human,
        summary_output: None,
    })
    .unwrap_or_else(|err| std::panic::panic_any(format!("external force replace: {err}")));
    assert!(out.with_extension("toml.bak").is_file());
    assert_eq!(
        fs::read_to_string(out.with_extension("toml.bak")).ok(),
        Some(first.clone())
    );
    let second = fs::read_to_string(&out)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read replaced output: {err}")));
    assert!(second.contains("example.invalid"));
    assert!(!second.contains("crates.io"));
    assert_ne!(second, first);
    let _ = fs::remove_file(&out);
    let _ = fs::remove_file(out.with_extension("toml.bak"));
}

#[test]
fn migrate_summary_collision_rejects_before_touching_destination_or_backup() {
    let dir = migrate_fixture_dir();
    let policy_dir = dir.join("policy");
    fs::create_dir_all(&policy_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
    fs::write(
        policy_dir.join("network-allowlist.toml"),
        network_policy_fixture_text(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("network fixture write: {err}")));
    let dir_name = dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| std::panic::panic_any("fixture directory has no final component"));
    let out = dir.with_file_name(format!("{dir_name}-summary-collision-output.toml"));
    fs::write(&out, "destination sentinel\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("destination write: {err}")));
    let summary_alias = out
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::panic::panic_any("destination has no parent"))
        .join(".")
        .join(
            out.file_name()
                .map(OsStr::to_os_string)
                .unwrap_or_else(|| std::panic::panic_any("destination has no name")),
        );
    let error = cmd_migrate(&MigrateArgs {
        root: RootArgs {
            root: Some(dir.clone()),
        },
        from: None,
        repo_policy: Some(policy_dir),
        out: out.clone(),
        force: true,
        update: false,
        summary_format: HumanJsonFormat::Human,
        summary_output: Some(summary_alias),
    })
    .expect_err("summary collision must reject before migration");
    assert!(error.to_string().contains("--summary-output"));
    assert_eq!(
        fs::read_to_string(&out).ok().as_deref(),
        Some("destination sentinel\n")
    );
    assert!(!out.with_extension("toml.bak").exists());
    let _ = fs::remove_file(&out);
    let _ = fs::remove_file(out.with_extension("toml.bak"));
    let _ = fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn migrate_external_output_rejects_leaf_symlink_without_touching_sentinel() {
    use std::os::unix::fs::symlink;

    let dir = migrate_fixture_dir();
    let policy_dir = dir.join("policy");
    fs::create_dir_all(&policy_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
    fs::write(
        policy_dir.join("network-allowlist.toml"),
        network_policy_fixture_text(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("network fixture write: {err}")));
    let sentinel = dir.with_file_name(format!(
        "{}-sentinel.toml",
        dir.file_name().unwrap().to_string_lossy()
    ));
    let out = dir.with_file_name(format!(
        "{}-symlink.toml",
        dir.file_name().unwrap().to_string_lossy()
    ));
    fs::write(&sentinel, "sentinel\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("sentinel write: {err}")));
    let _ = fs::remove_file(&out);
    symlink(&sentinel, &out)
        .unwrap_or_else(|err| std::panic::panic_any(format!("symlink setup: {err}")));

    let error = cmd_migrate(&MigrateArgs {
        root: RootArgs { root: Some(dir) },
        from: None,
        repo_policy: Some(policy_dir),
        out: out.clone(),
        force: true,
        update: false,
        summary_format: HumanJsonFormat::Human,
        summary_output: None,
    })
    .expect_err("external leaf symlink must fail closed");
    assert!(error.to_string().contains("symlink"));
    assert_eq!(
        fs::read_to_string(&sentinel).ok().as_deref(),
        Some("sentinel\n")
    );
    let _ = fs::remove_file(&out);
    let _ = fs::remove_file(&sentinel);
}

#[test]
fn migrate_external_output_rejects_directory_without_touching_neighbor() {
    let dir = migrate_fixture_dir();
    let policy_dir = dir.join("policy");
    fs::create_dir_all(&policy_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
    fs::write(
        policy_dir.join("network-allowlist.toml"),
        network_policy_fixture_text(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("network fixture write: {err}")));
    let out = dir.with_file_name(format!(
        "{}-directory.toml",
        dir.file_name().unwrap().to_string_lossy()
    ));
    let neighbor = dir.with_file_name(format!(
        "{}-neighbor.txt",
        dir.file_name().unwrap().to_string_lossy()
    ));
    let _ = fs::remove_dir_all(&out);
    fs::create_dir_all(&out)
        .unwrap_or_else(|err| std::panic::panic_any(format!("directory setup: {err}")));
    fs::write(&neighbor, "neighbor\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("neighbor write: {err}")));

    let error = cmd_migrate(&MigrateArgs {
        root: RootArgs { root: Some(dir) },
        from: None,
        repo_policy: Some(policy_dir),
        out: out.clone(),
        force: true,
        update: false,
        summary_format: HumanJsonFormat::Human,
        summary_output: None,
    })
    .expect_err("external directory output must fail closed");
    assert!(error.to_string().contains("regular file"));
    assert!(out.is_dir());
    assert_eq!(
        fs::read_to_string(&neighbor).ok().as_deref(),
        Some("neighbor\n")
    );
    let _ = fs::remove_dir_all(&out);
    let _ = fs::remove_file(&neighbor);
}

#[cfg(unix)]
#[test]
fn migrate_external_final_recheck_rejects_parent_retarget() {
    use std::os::unix::fs::symlink;

    let root = migrate_fixture_dir();
    let parent_a = root.with_file_name(format!(
        "{}-parent-a",
        root.file_name().unwrap().to_string_lossy()
    ));
    let parent_b = root.with_file_name(format!(
        "{}-parent-b",
        root.file_name().unwrap().to_string_lossy()
    ));
    fs::create_dir_all(&parent_a)
        .unwrap_or_else(|err| std::panic::panic_any(format!("parent A: {err}")));
    fs::create_dir_all(&parent_b)
        .unwrap_or_else(|err| std::panic::panic_any(format!("parent B: {err}")));
    let sentinel = parent_b.join("sentinel.txt");
    fs::write(&sentinel, "foreign\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("foreign sentinel: {err}")));
    let requested = parent_a.join("candidate.toml");
    let held = effortless_repo_edit::resolve_mutation_target(&requested, &root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("held target: {err}")));
    let _lock = effortless_repo_edit::MutationLock::acquire_for_target(&held)
        .unwrap_or_else(|err| std::panic::panic_any(format!("held lock: {err}")));
    let parent_a_old = root.with_file_name(format!(
        "{}-parent-a-old",
        root.file_name().unwrap().to_string_lossy()
    ));
    let mut retarget = || {
        fs::rename(&parent_a, &parent_a_old)
            .unwrap_or_else(|err| std::panic::panic_any(format!("retarget rename: {err}")));
        symlink(&parent_b, &parent_a)
            .unwrap_or_else(|err| std::panic::panic_any(format!("retarget symlink: {err}")));
    };
    let error = write_external_migrate_output_with_test_hook(
        &held,
        &requested,
        &root,
        "new\n",
        false,
        &mut retarget,
    )
    .expect_err("parent retarget must fail closed");
    assert!(error.to_string().contains("#2491"));
    assert_eq!(
        fs::read_to_string(&sentinel).ok().as_deref(),
        Some("foreign\n")
    );
    assert!(!parent_b.join("candidate.toml").exists());
    assert!(!parent_a_old.join("candidate.toml").exists());
    let _ = fs::remove_file(&parent_a);
    let _ = fs::remove_dir_all(&parent_a_old);
    let _ = fs::remove_dir_all(&parent_b);
    let _ = fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn migrate_external_final_recheck_rejects_file_to_directory_substitution() {
    let root = migrate_fixture_dir();
    let requested = root.join("candidate.toml");
    fs::write(&requested, "original\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("original output: {err}")));
    let held = effortless_repo_edit::resolve_mutation_target(&requested, &root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("held target: {err}")));
    let _lock = effortless_repo_edit::MutationLock::acquire_for_target(&held)
        .unwrap_or_else(|err| std::panic::panic_any(format!("held lock: {err}")));
    let sentinel = requested.join("sentinel.txt");
    let mut substitute = || {
        fs::remove_file(&requested)
            .unwrap_or_else(|err| std::panic::panic_any(format!("remove output: {err}")));
        fs::create_dir(&requested)
            .unwrap_or_else(|err| std::panic::panic_any(format!("directory substitute: {err}")));
        fs::write(&sentinel, "directory sentinel\n")
            .unwrap_or_else(|err| std::panic::panic_any(format!("directory sentinel: {err}")));
    };
    let error = write_external_migrate_output_with_test_hook(
        &held,
        &requested,
        &root,
        "replacement\n",
        true,
        &mut substitute,
    )
    .expect_err("file-to-directory substitution must fail closed");
    assert!(error.to_string().contains("#2491"));
    assert!(requested.is_dir());
    assert_eq!(
        fs::read_to_string(&sentinel).ok().as_deref(),
        Some("directory sentinel\n")
    );
    assert!(!requested.with_extension("toml.bak").exists());
    let _ = fs::remove_dir_all(&requested);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn migrate_repo_policy_writes_json_summary_with_inventory_context() {
    let dir = migrate_fixture_dir();
    let policy_dir = dir.join("policy");
    fs::create_dir_all(&policy_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
    fs::write(
        policy_dir.join("process-allowlist.toml"),
        process_policy_fixture_text(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("process fixture write: {err}")));
    fs::write(
        policy_dir.join("network-allowlist.toml"),
        network_policy_fixture_text(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("network fixture write: {err}")));
    let out = dir.join("allow.toml");
    let summary_output = dir.join("migrate-summary.json");

    cmd_migrate(&MigrateArgs {
        root: RootArgs::default(),
        from: None,
        repo_policy: Some(policy_dir),
        out: out.clone(),
        force: false,
        update: false,
        summary_format: HumanJsonFormat::Json,
        summary_output: Some(summary_output.clone()),
    })
    .unwrap_or_else(|err| std::panic::panic_any(format!("repo-policy migrate: {err}")));

    let summary = fs::read_to_string(&summary_output)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read migrate summary: {err}")));
    let value = serde_json::from_str::<Value>(&summary)
        .unwrap_or_else(|err| std::panic::panic_any(format!("parse migrate summary: {err}")));

    assert_eq!(
        value.pointer("/schema_id").and_then(Value::as_str),
        Some(allow_report::MIGRATE_SCHEMA_ID),
        "migrate schema id"
    );
    assert_eq!(
        value.pointer("/command").and_then(Value::as_str),
        Some("migrate"),
        "migrate command"
    );
    assert_eq!(
        value.pointer("/inventory/scope").and_then(Value::as_str),
        Some("source_tree"),
        "migrate inventory scope"
    );
    assert_eq!(
        value.pointer("/inventory/scanner").and_then(Value::as_str),
        Some("policy_migration"),
        "migrate inventory scanner"
    );
    assert_eq!(
        value.pointer("/inventory/source").and_then(Value::as_str),
        Some("filesystem_fallback"),
        "migrate inventory source"
    );
    assert!(
        value
            .pointer("/inventory/files_scanned")
            .and_then(Value::as_u64)
            .is_some_and(|count| count >= 2),
        "repo-policy migration summary should include source-tree inventory file count"
    );
    assert_eq!(
        value.pointer("/input/kind").and_then(Value::as_str),
        Some("repo_policy"),
        "migrate input kind"
    );
    assert_eq!(
        value
            .pointer("/summary/allow_entries")
            .and_then(Value::as_u64),
        Some(2),
        "migrate allow entries"
    );
    assert_eq!(
        value
            .pointer("/summary/entries_with_evidence")
            .and_then(Value::as_u64),
        Some(2),
        "migrate evidence-bearing entries"
    );
    assert!(
        value
            .pointer("/summary/evidence_entries")
            .and_then(Value::as_u64)
            .is_some_and(|count| count > 2),
        "repo-policy migration summary should count migrated evidence references"
    );
    assert_eq!(
        value
            .pointer("/summary/entries_with_links")
            .and_then(Value::as_u64),
        Some(2),
        "repo-policy migration summary should count link-bearing migrated entries"
    );
    assert_eq!(
        value
            .pointer("/summary/link_entries")
            .and_then(Value::as_u64),
        Some(2),
        "repo-policy migration summary should count canonical traceability links"
    );
    assert!(
        value
            .pointer("/summary/weak_evidence_references")
            .and_then(Value::as_u64)
            .is_some_and(|count| count > 0),
        "repo-policy migration summary should surface weak evidence references"
    );
}

#[test]
fn migrate_repo_policy_summary_counts_unsafe_weak_evidence() {
    let dir = migrate_fixture_dir();
    let policy_dir = dir.join("policy");
    fs::create_dir_all(&policy_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
    fs::write(
        policy_dir.join("unsafe-allowlist.toml"),
        unsafe_policy_missing_evidence_fixture_text(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("unsafe fixture write: {err}")));
    let out = dir.join("allow.toml");
    let summary_output = dir.join("migrate-summary.json");

    cmd_migrate(&MigrateArgs {
        root: RootArgs::default(),
        from: None,
        repo_policy: Some(policy_dir),
        out,
        force: false,
        update: false,
        summary_format: HumanJsonFormat::Json,
        summary_output: Some(summary_output.clone()),
    })
    .unwrap_or_else(|err| std::panic::panic_any(format!("unsafe repo-policy migrate: {err}")));

    let summary = fs::read_to_string(&summary_output)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read migrate summary: {err}")));
    let value = serde_json::from_str::<Value>(&summary)
        .unwrap_or_else(|err| std::panic::panic_any(format!("parse migrate summary: {err}")));

    assert_eq!(
        value
            .pointer("/summary/unsafe_entries")
            .and_then(Value::as_u64),
        Some(1),
        "unsafe migration summary should count unsafe entries"
    );
    assert_eq!(
        value
            .pointer("/summary/weak_evidence_references")
            .and_then(Value::as_u64),
        Some(1),
        "unsafe migration summary should count weak evidence references"
    );
    assert_eq!(
        value
            .pointer("/summary/unsafe_weak_evidence_references")
            .and_then(Value::as_u64),
        Some(1),
        "unsafe migration summary should count unsafe weak evidence references"
    );
}

#[test]
fn migrate_repo_policy_summary_counts_unsafe_broken_evidence() {
    let dir = migrate_fixture_dir();
    let policy_dir = dir.join("policy");
    fs::create_dir_all(&policy_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
    fs::write(
        policy_dir.join("unsafe-allowlist.toml"),
        unsafe_policy_broken_evidence_fixture_text(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("unsafe fixture write: {err}")));
    let out = dir.join("allow.toml");
    let summary_output = dir.join("migrate-summary.json");

    cmd_migrate(&MigrateArgs {
        root: RootArgs::default(),
        from: None,
        repo_policy: Some(policy_dir),
        out,
        force: false,
        update: false,
        summary_format: HumanJsonFormat::Json,
        summary_output: Some(summary_output.clone()),
    })
    .unwrap_or_else(|err| std::panic::panic_any(format!("unsafe repo-policy migrate: {err}")));

    let summary = fs::read_to_string(&summary_output)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read migrate summary: {err}")));
    let value = serde_json::from_str::<Value>(&summary)
        .unwrap_or_else(|err| std::panic::panic_any(format!("parse migrate summary: {err}")));

    assert_eq!(
        value
            .pointer("/summary/broken_evidence_links")
            .and_then(Value::as_u64),
        Some(1),
        "unsafe migration summary should count broken local evidence references"
    );
    assert_eq!(
        value
            .pointer("/summary/unsafe_broken_evidence_links")
            .and_then(Value::as_u64),
        Some(1),
        "unsafe migration summary should count unsafe broken local evidence references"
    );
    assert!(
        value.pointer("/summary/weak_evidence_references").is_none(),
        "typed missing local evidence should not be classified as weak evidence"
    );
    assert!(
        value
            .pointer("/summary/unsafe_weak_evidence_references")
            .is_none(),
        "typed missing local unsafe evidence should not be classified as weak evidence"
    );
}

#[test]
fn migrate_repo_policy_human_summary_routes_evidence_repair_queues() {
    let dir = migrate_fixture_dir();
    let policy_dir = dir.join("policy");
    fs::create_dir_all(&policy_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
    fs::write(
        policy_dir.join("unsafe-allowlist.toml"),
        unsafe_policy_broken_and_weak_evidence_fixture_text(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("unsafe fixture write: {err}")));
    let out = dir.join("allow.toml");
    let summary_output = dir.join("migrate-summary.txt");

    cmd_migrate(&MigrateArgs {
        root: RootArgs::default(),
        from: None,
        repo_policy: Some(policy_dir),
        out,
        force: false,
        update: false,
        summary_format: HumanJsonFormat::Human,
        summary_output: Some(summary_output.clone()),
    })
    .unwrap_or_else(|err| std::panic::panic_any(format!("unsafe repo-policy migrate: {err}")));

    let summary = fs::read_to_string(&summary_output)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read migrate summary: {err}")));

    assert!(summary.contains("broken_evidence_links: 1"));
    assert!(summary.contains("unsafe_broken_evidence_links: 1"));
    assert!(summary.contains("weak_evidence_references: 1"));
    assert!(summary.contains("unsafe_weak_evidence_references: 1"));
    assert!(summary.contains("evidence_repair_queues:"));
    assert!(
        summary.contains("cargo-allow worklist --item-kind broken_evidence_link --format json")
    );
    assert!(summary.contains(
        "cargo-allow worklist --item-kind broken_evidence_link --kind unsafe --format json"
    ));
    assert!(
        summary.contains("cargo-allow worklist --item-kind weak_evidence_reference --format json")
    );
    assert!(summary.contains(
        "cargo-allow worklist --item-kind weak_evidence_reference --kind unsafe --format json"
    ));
}

#[test]
fn migrate_from_uses_explicit_root_for_evidence_diagnostics() {
    let dir = migrate_fixture_dir();
    let docs_dir = dir.join("docs/safety");
    fs::create_dir_all(&docs_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("docs dir: {err}")));
    fs::write(
        docs_dir.join("migrated-boundary.md"),
        "reviewed migration boundary",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("evidence write: {err}")));
    let from = dir.join("legacy.allow.toml");
    fs::write(&from, canonical_policy_with_present_evidence_fixture_text())
        .unwrap_or_else(|err| std::panic::panic_any(format!("canonical fixture write: {err}")));
    let out = dir.join("allow.toml");
    let summary_output = dir.join("migrate-summary.json");

    cmd_migrate(&MigrateArgs {
        root: RootArgs {
            root: Some(dir.clone()),
        },
        from: Some(from),
        repo_policy: None,
        out,
        force: false,
        update: false,
        summary_format: HumanJsonFormat::Json,
        summary_output: Some(summary_output.clone()),
    })
    .unwrap_or_else(|err| std::panic::panic_any(format!("single-file migrate: {err}")));

    let summary = fs::read_to_string(&summary_output)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read migrate summary: {err}")));
    let value = serde_json::from_str::<Value>(&summary)
        .unwrap_or_else(|err| std::panic::panic_any(format!("parse migrate summary: {err}")));
    // #1825: On Windows the source-tree root is canonicalized by
    // resolve_source_tree_root (yielding long names), while the test's temp
    // dir from std::env::temp_dir() may use 8.3 short names (RUNNER~1).
    // Canonicalize before computing the expected root so both sides match.
    let canonical_dir = dir
        .canonicalize()
        .unwrap_or_else(|err| std::panic::panic_any(format!("canonicalize dir: {err}")));
    let expected_root = allow_report::source_tree_path_text(&canonical_dir);

    assert_eq!(
        value.pointer("/input/kind").and_then(Value::as_str),
        Some("from"),
        "single-file migrate input kind"
    );
    assert_eq!(
        value.pointer("/inventory/root").and_then(Value::as_str),
        Some(expected_root.as_str()),
        "single-file migrate should record explicit source-tree root"
    );
    assert_eq!(
        value
            .pointer("/summary/entries_with_evidence")
            .and_then(Value::as_u64),
        Some(1),
        "single-file migrate evidence-bearing entries"
    );
    assert_eq!(
        value
            .pointer("/summary/evidence_entries")
            .and_then(Value::as_u64),
        Some(1),
        "single-file migrate evidence reference entries"
    );
    assert!(
        value.pointer("/summary/broken_evidence_links").is_none(),
        "present local evidence under --root should not be reported as broken"
    );
    assert!(
        value.pointer("/evidence_repair_queues").is_none(),
        "present local evidence under --root should not route repair work"
    );
}

#[test]
fn migrate_from_infers_root_for_evidence_diagnostics() {
    let dir = migrate_fixture_dir();
    let docs_dir = dir.join("docs/safety");
    fs::create_dir_all(&docs_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("docs dir: {err}")));
    fs::write(
        docs_dir.join("migrated-boundary.md"),
        "reviewed migration boundary",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("evidence write: {err}")));
    let from = dir.join("legacy.allow.toml");
    fs::write(&from, canonical_policy_with_present_evidence_fixture_text())
        .unwrap_or_else(|err| std::panic::panic_any(format!("canonical fixture write: {err}")));
    let out = dir.join("allow.toml");
    let summary_output = dir.join("migrate-summary.json");

    cmd_migrate(&MigrateArgs {
        root: RootArgs::default(),
        from: Some(from),
        repo_policy: None,
        out,
        force: false,
        update: false,
        summary_format: HumanJsonFormat::Json,
        summary_output: Some(summary_output.clone()),
    })
    .unwrap_or_else(|err| std::panic::panic_any(format!("single-file migrate: {err}")));

    let summary = fs::read_to_string(&summary_output)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read migrate summary: {err}")));
    let value = serde_json::from_str::<Value>(&summary)
        .unwrap_or_else(|err| std::panic::panic_any(format!("parse migrate summary: {err}")));
    // #1825: On Windows the source-tree root is canonicalized by
    // resolve_source_tree_root (yielding long names), while the test's temp
    // dir from std::env::temp_dir() may use 8.3 short names (RUNNER~1).
    // Canonicalize before computing the expected root so both sides match.
    let canonical_dir = dir
        .canonicalize()
        .unwrap_or_else(|err| std::panic::panic_any(format!("canonicalize dir: {err}")));
    let expected_root = allow_report::source_tree_path_text(&canonical_dir);

    assert_eq!(
        value.pointer("/inventory/source").and_then(Value::as_str),
        Some("filesystem_fallback"),
        "single-file migrate should report inferred inventory source"
    );
    assert_eq!(
        value.pointer("/inventory/root").and_then(Value::as_str),
        Some(expected_root.as_str()),
        "single-file migrate should infer source-tree root from --from"
    );
    assert!(
        value
            .pointer("/inventory/files_scanned")
            .and_then(Value::as_u64)
            .is_some_and(|count| count >= 2),
        "single-file migrate should report inferred inventory file count"
    );
    assert!(
        value.pointer("/summary/broken_evidence_links").is_none(),
        "present local evidence under inferred root should not be reported as broken"
    );
    assert!(
        value.pointer("/evidence_repair_queues").is_none(),
        "present local evidence under inferred root should not route repair work"
    );
}

fn migrate_fixture_dir() -> PathBuf {
    static NEXT_MIGRATE_FIXTURE: AtomicUsize = AtomicUsize::new(0);
    let id = NEXT_MIGRATE_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "cargo-allow-cli-migrate-{}-{stamp}-{id}",
        std::process::id()
    ));
    fs::create_dir_all(&dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture dir: {err}")));
    dir
}

fn canonical_policy_with_present_evidence_fixture_text() -> &'static str {
    r#"schema_version = "0.1"
policy = "cargo-allow"
owner = "EffortlessMetrics"
status = "active"

[[allow]]
id = "allow-migrated-doc"
kind = "non_rust_file"
family = "documentation"
path = "README.md"
owner = "docs"
classification = "reviewed_documentation"
reason = "Retained documentation file carried forward from legacy migration."
created = "2026-06-02"
review_after = "2026-11-01"
evidence = ["doc:docs/safety/migrated-boundary.md"]

[allow.selector]
ast_kind = "tracked_file"
symbol = "README.md"
target_fingerprint = "md"
line_hint = 1
"#
}

fn unsafe_policy_missing_evidence_fixture_text() -> &'static str {
    r#"schema_version = 1
policy = "unsafe-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
path = "src/lib.rs"
family = "unsafe_fn"

[allow.selector]
kind = "unsafe-fn"
"#
}

fn unsafe_policy_broken_evidence_fixture_text() -> &'static str {
    r#"schema_version = 1
policy = "unsafe-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "legacy-unsafe-missing-doc"
path = "src/lib.rs"
family = "unsafe_fn"
kind = "unsafe-fn"
evidence = ["doc:docs/safety/missing-ffi.md"]

[allow.selector]
kind = "unsafe-fn"
"#
}

fn unsafe_policy_broken_and_weak_evidence_fixture_text() -> &'static str {
    r#"schema_version = 1
policy = "unsafe-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "legacy-unsafe-missing-and-todo"
path = "src/lib.rs"
family = "unsafe_fn"
kind = "unsafe-fn"
evidence = [
  "doc:docs/safety/missing-ffi.md",
  "TODO: add unsafe-review or boundary-test evidence"
]

[allow.selector]
kind = "unsafe-fn"
"#
}

fn process_policy_fixture_text() -> &'static str {
    r#"schema_version = 1
policy = "process-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "proc-cargo-install-cargo-deny"
binary = "cargo"
argv_shape = ["install", "cargo-deny", "--locked"]
network_reach = true
called_by = [".github/workflows/ci.yml"]
owner = "release/ci"
reason = "Installs cargo-deny in the deny job."
created = "2026-05-09"
review_after = "2026-09-09"
"#
}

fn network_policy_fixture_text() -> &'static str {
    r#"schema_version = 1
policy = "network-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "net-crates-io-fetch"
destination = "crates.io"
auth_required = false
lane = "build"
owner = "release"
reason = "cargo fetch resolves and downloads crate dependencies."
created = "2026-05-09"
expires = "permanent"
"#
}

fn argv(items: Vec<&str>) -> Vec<String> {
    items.into_iter().map(String::from).collect()
}
