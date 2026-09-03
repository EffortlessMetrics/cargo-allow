use super::*;
use crate::artifact_contract_support::parse_json_artifact;
use crate::init::cmd_init;
use crate::{CargoAllowCli, CargoAllowCommand, HumanJsonFormat, ProfileArg, RootArgs};
use allow_core::{CargoAllowErrorKind, SOURCE_FILE_READ_MAX_BYTES};
use clap::Parser;
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::Command;

fn argv(items: Vec<&str>) -> Vec<String> {
    items.into_iter().map(String::from).collect()
}

#[test]
fn clap_parses_doctor_json_output() {
    let parsed = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "doctor",
        "--root",
        ".",
        "--config",
        "policy/custom.toml",
        "--format",
        "json",
        "--output",
        "target/doctor.json",
    ]))
    .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse: {err}")));

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::Doctor(DoctorArgs {
            root: RootArgs { root: Some(root) },
            config: Some(config),
            profile: None,
            format: HumanJsonFormat::Json,
            require_clean: false,
            output: Some(output),
            support_bundle: None,
        })) if root == Path::new(".")
            && config == Path::new("policy/custom.toml")
            && output == Path::new("target/doctor.json")
    ));
}

#[test]
fn intent_provider_failure_is_explicitly_non_clean_and_actionable() {
    let unavailable =
        render_intent_provider_failure(&crate::intent_provider::IntentProviderFailure {
            class: crate::intent_provider::IntentProviderFailureClass::Absent,
            detail: "provider not found".to_string(),
        });
    assert!(unavailable.contains("status: unavailable"));
    assert!(unavailable.contains("required version range: 0.1.x"));
    assert!(unavailable.contains("required protocol: repo.analysis-receipt.v1"));
    assert!(unavailable.contains("canonical command:"));
    assert!(unavailable.contains("intent evaluation: not performed"));
    assert!(unavailable.contains("clean claim: not available"));

    let incompatible =
        render_intent_provider_failure(&crate::intent_provider::IntentProviderFailure {
            class: crate::intent_provider::IntentProviderFailureClass::WrongProductName,
            detail: "wrong product".to_string(),
        });
    assert!(incompatible.contains("status: incompatible"));
    assert!(incompatible.contains("binary detected, version not probed"));
}

#[test]
fn clap_parses_support_bundle_output() {
    let parsed = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "doctor",
        "--support-bundle",
        "target/cargo-allow/support-bundle.json",
    ]))
    .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse: {err}")));

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::Doctor(DoctorArgs {
            support_bundle: Some(path),
            ..
        })) if path == Path::new("target/cargo-allow/support-bundle.json")
    ));
}

#[test]
fn doctor_writes_redacted_support_bundle() -> Result<(), String> {
    let root = doctor_fixture_dir();
    fs::create_dir_all(root.join("policy")).map_err(|error| error.to_string())?;
    fs::write(root.join("source.rs"), "fn source() {}\n").map_err(|error| error.to_string())?;
    let policy = root.join("policy/allow.toml");
    fs::write(
        &policy,
        "schema_version = \"0.1\"\npolicy = \"cargo-allow\"\nowner = \"core/policy\"\nstatus = \"active\"\n",
    )
    .map_err(|error| error.to_string())?;
    let doctor_output = root.join("doctor.txt");
    let bundle_output = root.join("target/cargo-allow/support-bundle.json");

    let result = cmd_doctor(&DoctorArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: Some(policy),
        profile: None,
        format: HumanJsonFormat::Human,
        require_clean: false,
        output: Some(doctor_output),
        support_bundle: Some(bundle_output.clone()),
    })
    .map_err(|error| error.to_string());
    if let Err(error) = result {
        remove_doctor_fixture_dir(root);
        return Err(error);
    }

    let bundle = fs::read_to_string(&bundle_output).map_err(|error| error.to_string())?;
    let value: Value = serde_json::from_str(&bundle).map_err(|error| error.to_string())?;
    if value.pointer("/schema_id").and_then(Value::as_str) != Some("cargo-allow.support-bundle.v1")
    {
        return Err("support bundle schema id did not match".to_string());
    }
    if value.pointer("/root/path").and_then(Value::as_str) != Some("<redacted>") {
        return Err("support bundle root was not redacted".to_string());
    }
    if value.pointer("/config/path").and_then(Value::as_str) != Some("policy/allow.toml") {
        return Err("support bundle config path was not repository-relative".to_string());
    }
    let digest = value
        .pointer("/config/digest")
        .and_then(Value::as_str)
        .ok_or_else(|| "support bundle omitted the selected policy digest".to_string())?;
    if !digest.starts_with("sha256:v1:") {
        return Err("support bundle policy digest was not canonical".to_string());
    }
    if value.pointer("/config/precedence").and_then(Value::as_str) != Some("cli_override") {
        return Err("support bundle precedence was not canonical".to_string());
    }
    if bundle.contains(root.to_string_lossy().as_ref()) || bundle.contains("fn source") {
        return Err("support bundle leaked an absolute root or source contents".to_string());
    }

    remove_doctor_fixture_dir(root);
    Ok(())
}

#[test]
fn spec_system_doctor_rejects_support_bundle_output() -> Result<(), String> {
    let root = doctor_fixture_dir();
    let result = cmd_doctor(&DoctorArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: None,
        profile: Some(ProfileArg::SpecSystem),
        format: HumanJsonFormat::Human,
        require_clean: false,
        output: None,
        support_bundle: Some(root.join("support-bundle.json")),
    });
    remove_doctor_fixture_dir(root);
    match result {
        Ok(()) => Err("spec-system doctor unexpectedly accepted support bundle".to_string()),
        Err(error)
            if error.kind() == CargoAllowErrorKind::Usage
                && error.code() == "E0001_USAGE"
                && error.to_string().contains("only supported") =>
        {
            Ok(())
        }
        Err(error) => Err(format!("unexpected error: {error}")),
    }
}

#[test]
fn doctor_require_clean_classifies_missing_configuration() -> Result<(), String> {
    let root = doctor_fixture_dir();
    let result = cmd_doctor(&DoctorArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: None,
        profile: None,
        format: HumanJsonFormat::Human,
        require_clean: true,
        output: None,
        support_bundle: None,
    });
    remove_doctor_fixture_dir(root);

    match result {
        Err(error)
            if error.kind() == CargoAllowErrorKind::InvalidConfig
                && error.code() == "E0002_INVALID_CONFIG" =>
        {
            Ok(())
        }
        Err(error) => Err(format!("unexpected error: {error}")),
        Ok(()) => Err("doctor --require-clean unexpectedly passed without config".to_string()),
    }
}

#[test]
fn doctor_require_clean_classifies_malformed_policy() -> Result<(), String> {
    let root = doctor_fixture_dir();
    let policy = root.join("allow.toml");
    fs::write(
        &policy,
        "schema_version = \"\"\npolicy = \"cargo-allow\"\nowner = \"core/policy\"\nstatus = \"active\"\n",
    )
    .map_err(|error| error.to_string())?;
    let result = cmd_doctor(&DoctorArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: Some(policy),
        profile: None,
        format: HumanJsonFormat::Human,
        require_clean: true,
        output: None,
        support_bundle: None,
    });
    remove_doctor_fixture_dir(root);

    match result {
        Err(error)
            if error.kind() == CargoAllowErrorKind::InvalidPolicy
                && error.code() == "E0003_INVALID_POLICY" =>
        {
            Ok(())
        }
        Err(error) => Err(format!("unexpected error: {error}")),
        Ok(()) => Err("doctor --require-clean unexpectedly passed malformed policy".to_string()),
    }
}

#[test]
fn doctor_require_clean_rejects_malformed_federation_fallback() -> Result<(), String> {
    let root = doctor_fixture_dir();
    fs::create_dir_all(root.join(".allow")).map_err(|error| error.to_string())?;
    fs::write(
        root.join(".allow/config.toml"),
        "schema_version = \"1.0\"\n\n[[ledgers]]\nid = \"dup\"\npath = \"policy/allow.toml\"\ndialect = \"cargo-allow\"\nrole = \"canonical\"\npriority = 10\n\n[[ledgers]]\nid = \"dup\"\npath = \"policy/other.toml\"\ndialect = \"cargo-allow\"\nrole = \"imported\"\npriority = 20\n",
    )
    .map_err(|error| error.to_string())?;
    let policy = root.join("allow.toml");
    fs::write(
        &policy,
        "policy = \"cargo-allow\"\nowner = \"core/policy\"\nstatus = \"active\"\n",
    )
    .map_err(|error| error.to_string())?;

    let result = cmd_doctor(&DoctorArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: None,
        profile: None,
        format: HumanJsonFormat::Human,
        require_clean: true,
        output: None,
        support_bundle: None,
    });
    remove_doctor_fixture_dir(root);

    match result {
        Err(error)
            if error.kind() == CargoAllowErrorKind::PolicyViolation
                && error
                    .to_string()
                    .contains("policy config is invalid or missing") =>
        {
            Ok(())
        }
        Err(error) => Err(format!("unexpected federation fallback error: {error}")),
        Ok(()) => Err(
            "doctor --require-clean unexpectedly accepted malformed federation fallback"
                .to_string(),
        ),
    }
}

#[test]
fn doctor_require_clean_classifies_broken_evidence() -> Result<(), String> {
    let root = doctor_fixture_dir();
    let policy = root.join("allow.toml");
    fs::write(
        &policy,
        r#"
policy = "cargo-allow"

[[allow]]
id = "allow-doc"
kind = "non_rust_file"
path = "docs/policy.md"
owner = "docs"
classification = "reviewed"
reason = "Tracked documentation policy."
evidence = ["doc:docs/missing.md"]
review_after = "2026-06-30"

[allow.selector]
ast_kind = "tracked_file"
"#,
    )
    .map_err(|error| error.to_string())?;
    let result = cmd_doctor(&DoctorArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: Some(policy),
        profile: None,
        format: HumanJsonFormat::Human,
        require_clean: true,
        output: None,
        support_bundle: None,
    });
    remove_doctor_fixture_dir(root);

    match result {
        Err(error)
            if error.kind() == CargoAllowErrorKind::PolicyViolation
                && error.code() == "E0006_POLICY_VIOLATION" =>
        {
            Ok(())
        }
        Err(error) => Err(format!("unexpected error: {error}")),
        Ok(()) => Err("doctor --require-clean unexpectedly passed broken evidence".to_string()),
    }
}

#[test]
fn doctor_projects_partial_rust_scan_and_require_clean_rejects_it() -> Result<(), String> {
    let root = doctor_fixture_dir();
    fs::create_dir_all(root.join("policy")).map_err(|error| error.to_string())?;
    fs::write(root.join("source.rs"), "fn source() {}\n").map_err(|error| error.to_string())?;
    fs::write(
        root.join("oversized.rs"),
        vec![b'a'; (SOURCE_FILE_READ_MAX_BYTES as usize).saturating_add(1)],
    )
    .map_err(|error| error.to_string())?;
    let policy = root.join("policy/allow.toml");
    fs::write(
        &policy,
        "policy = \"cargo-allow\"\n\n[workspace]\nignored = [\"policy/**\"]\n",
    )
    .map_err(|error| error.to_string())?;
    let output = root.join("doctor.json");

    cmd_doctor(&DoctorArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: Some(policy.clone()),
        profile: None,
        format: HumanJsonFormat::Json,
        require_clean: false,
        output: Some(output.clone()),
        support_bundle: None,
    })
    .map_err(|error| error.to_string())?;

    let json = fs::read_to_string(&output).map_err(|error| error.to_string())?;
    let value = parse_json_artifact("doctor", &json, allow_report::DOCTOR_SCHEMA_ID, "doctor");
    if value
        .pointer("/scanner/completeness")
        .and_then(Value::as_str)
        != Some("partial")
        || value
            .pointer("/scanner/rust/files_considered")
            .and_then(Value::as_u64)
            != Some(2)
        || value
            .pointer("/scanner/rust/files_scanned")
            .and_then(Value::as_u64)
            != Some(1)
        || value
            .pointer("/scanner/rust/files_skipped")
            .and_then(Value::as_u64)
            != Some(1)
        || value
            .pointer("/scanner/rust/skipped_by_reason/read_failed_or_unsupported")
            .and_then(Value::as_u64)
            != Some(1)
    {
        remove_doctor_fixture_dir(root);
        return Err(format!("doctor did not project partial Rust scan: {value}"));
    }

    let result = cmd_doctor(&DoctorArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: Some(policy),
        profile: None,
        format: HumanJsonFormat::Human,
        require_clean: true,
        output: None,
        support_bundle: None,
    });
    remove_doctor_fixture_dir(root);
    match result {
        Err(error)
            if error.kind() == CargoAllowErrorKind::PolicyViolation
                && error
                    .to_string()
                    .contains("Rust scanner coverage is partial") =>
        {
            Ok(())
        }
        Err(error) => Err(format!("unexpected error: {error}")),
        Ok(()) => Err("doctor --require-clean unexpectedly accepted partial Rust coverage".into()),
    }
}

#[test]
fn clap_parses_spec_system_profile_for_doctor() {
    let parsed = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "doctor",
        "--profile",
        "spec-system",
        "--format",
        "json",
    ]))
    .unwrap_or_else(|err| {
        std::panic::panic_any(format!("CLI should parse spec-system doctor: {err}"))
    });

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::Doctor(DoctorArgs {
            profile: Some(ProfileArg::SpecSystem),
            format: HumanJsonFormat::Json,
            require_clean: false,
            ..
        }))
    ));
}

#[test]
fn render_doctor_json_records_setup_context() {
    let json = allow_report::render_doctor_json(allow_report::DoctorReport {
        source_tree_root: "H:/Code/Rust/cargo-allow",
        root_discovery: "nearest_git_root",
        config_path: Some("H:/Code/Rust/cargo-allow/policy/allow.toml"),
        config_schema_version: Some("0.1"),
        config_policy: Some("cargo-allow"),
        config_owner: Some("core/policy"),
        config_status: Some("active"),
        config_provenance: None,
        config_valid: Some(true),
        config_diagnostic: None,
        broken_evidence_links: Some(0),
        weak_evidence_references: Some(0),
        inventory_source: "git_tracked",
        inventory_completeness: "scoped",
        files_scanned: 50,
        empty_git_tracked: false,
        deleted_tracked_files: 0,
        git_inventory_error: None,
        skipped_paths: 0,
        submodule_paths: 0,
        rust_scanner_completeness: "unknown",
        rust_files_considered: 0,
        rust_files_scanned: 0,
        rust_files_skipped: 0,
        rust_files_with_parse_errors: 0,
        rust_files_skipped_by_read_or_unsupported: 0,
        federation_config_path: None,
        federation_config_found: false,
        federation_config_valid: None,
        configured_ledgers: None,
        federation_diagnostics: None,
        federation_divergences: None,
        file_family_rules: &[],
        file_family_conflicts: &[],
    });
    let value = parse_json_artifact("doctor", &json, allow_report::DOCTOR_SCHEMA_ID, "doctor");

    assert_eq!(
        value.pointer("/root/path").and_then(Value::as_str),
        Some("H:/Code/Rust/cargo-allow")
    );
    assert_eq!(
        value.pointer("/root/discovery").and_then(Value::as_str),
        Some("nearest_git_root")
    );
    assert_eq!(
        value.pointer("/config/found").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        value.pointer("/config/path").and_then(Value::as_str),
        Some("H:/Code/Rust/cargo-allow/policy/allow.toml")
    );
    assert_eq!(
        value
            .pointer("/config/schema_version")
            .and_then(Value::as_str),
        Some("0.1")
    );
    assert_eq!(
        value.pointer("/config/policy").and_then(Value::as_str),
        Some("cargo-allow")
    );
    assert_eq!(
        value.pointer("/config/owner").and_then(Value::as_str),
        Some("core/policy")
    );
    assert_eq!(
        value.pointer("/config/status").and_then(Value::as_str),
        Some("active")
    );
    assert_eq!(
        value.pointer("/config/valid").and_then(Value::as_bool),
        Some(true)
    );
    assert!(value.pointer("/config/diagnostic").is_none());
    assert_eq!(
        value
            .pointer("/config/broken_evidence_links")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        value
            .pointer("/config/weak_evidence_references")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        value.pointer("/inventory/scope").and_then(Value::as_str),
        Some("source_tree")
    );
    assert_eq!(
        value.pointer("/inventory/scanner").and_then(Value::as_str),
        Some("source_syntax")
    );
    assert_eq!(
        value.pointer("/inventory/source").and_then(Value::as_str),
        Some("git_tracked")
    );
    assert_eq!(
        value.pointer("/inventory/root").and_then(Value::as_str),
        Some("H:/Code/Rust/cargo-allow")
    );
    assert_eq!(
        value
            .pointer("/inventory/files_scanned")
            .and_then(Value::as_u64),
        Some(50)
    );
}

#[test]
fn doctor_config_status_reports_invalid_policy_without_failing() {
    let root = doctor_fixture_dir();
    let policy = root.join("allow.toml");
    fs::write(
        &policy,
        r#"
schema_version = ""
policy = "cargo-allow"
owner = "core/policy"
status = "active"
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write invalid policy: {err}")));

    let policy = load_doctor_policy(Some(&policy));
    let (valid, diagnostic) = config_status(&root, policy.as_ref(), None);

    assert_eq!(valid, Some(false));
    assert!(
        diagnostic
            .is_some_and(|message| message.contains("policy schema_version must not be empty"))
    );
    remove_doctor_fixture_dir(root);
}

#[test]
fn doctor_inventory_respects_policy_ignored_globs() {
    let root = doctor_fixture_dir();
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::create_dir_all(root.join("ignored"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create ignored dir: {err}")));
    fs::write(root.join("kept.rs"), "fn kept() {}\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write kept source: {err}")));
    fs::write(root.join("ignored/skipped.rs"), "fn skipped() {}\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write ignored source: {err}")));
    let policy = root.join("policy/allow.toml");
    fs::write(
        &policy,
        r#"
policy = "cargo-allow"

[workspace]
ignored = ["policy/**", "ignored/**"]
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
    let output = root.join("doctor.json");

    cmd_doctor(&DoctorArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: Some(policy),
        profile: None,
        format: HumanJsonFormat::Json,
        require_clean: false,
        output: Some(output.clone()),
        support_bundle: None,
    })
    .unwrap_or_else(|err| std::panic::panic_any(format!("doctor should pass: {err}")));

    let json = fs::read_to_string(&output)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read doctor output: {err}")));
    let value = parse_json_artifact("doctor", &json, allow_report::DOCTOR_SCHEMA_ID, "doctor");
    assert_eq!(
        value
            .pointer("/inventory/files_scanned")
            .and_then(Value::as_u64),
        Some(1),
        "doctor should use policy ignored globs for source-tree inventory"
    );
    assert_eq!(
        value.pointer("/config/valid").and_then(Value::as_bool),
        Some(true),
        "policy should remain valid"
    );
    remove_doctor_fixture_dir(root);
}

#[test]
fn doctor_reports_policy_evidence_health_counts() {
    let root = doctor_fixture_dir();
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    let policy = root.join("policy/allow.toml");
    fs::write(
        &policy,
        r#"
policy = "cargo-allow"

[[allow]]
id = "allow-doc"
kind = "non_rust_file"
path = "docs/policy.md"
owner = "docs"
classification = "reviewed"
reason = "Tracked documentation policy."
evidence = ["doc:docs/missing.md", "spreadsheet:manual-review"]
review_after = "2026-06-30"

[allow.selector]
ast_kind = "tracked_file"
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
    let output = root.join("doctor.json");

    cmd_doctor(&DoctorArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: Some(policy),
        profile: None,
        format: HumanJsonFormat::Json,
        require_clean: false,
        output: Some(output.clone()),
        support_bundle: None,
    })
    .unwrap_or_else(|err| std::panic::panic_any(format!("doctor should pass: {err}")));

    let json = fs::read_to_string(&output)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read doctor output: {err}")));
    let value = parse_json_artifact("doctor", &json, allow_report::DOCTOR_SCHEMA_ID, "doctor");
    assert_eq!(
        value.pointer("/config/valid").and_then(Value::as_bool),
        Some(false),
        "broken local evidence should make doctor config invalid"
    );
    assert_eq!(
        value
            .pointer("/config/broken_evidence_links")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        value
            .pointer("/config/weak_evidence_references")
            .and_then(Value::as_u64),
        Some(1)
    );
    remove_doctor_fixture_dir(root);
}

#[test]
fn doctor_reports_policy_link_health_counts() {
    let root = doctor_fixture_dir();
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    let policy = root.join("policy/allow.toml");
    fs::write(
        &policy,
        r#"
policy = "cargo-allow"

[[allow]]
id = "allow-doc-link"
kind = "non_rust_file"
path = "docs/policy.md"
owner = "docs"
classification = "reviewed"
reason = "Tracked documentation policy."
evidence = ["test:doctor_reports_policy_link_health_counts"]
links = ["doc:docs/missing-rationale.md", "spreadsheet:manual-review"]
review_after = "2026-06-30"

[allow.selector]
ast_kind = "tracked_file"
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
    let output = root.join("doctor.json");

    cmd_doctor(&DoctorArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: Some(policy),
        profile: None,
        format: HumanJsonFormat::Json,
        require_clean: false,
        output: Some(output.clone()),
        support_bundle: None,
    })
    .unwrap_or_else(|err| std::panic::panic_any(format!("doctor should pass: {err}")));

    let json = fs::read_to_string(&output)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read doctor output: {err}")));
    let value = parse_json_artifact("doctor", &json, allow_report::DOCTOR_SCHEMA_ID, "doctor");
    assert_eq!(
        value.pointer("/config/valid").and_then(Value::as_bool),
        Some(false),
        "broken local links should make doctor config invalid"
    );
    assert_eq!(
        value
            .pointer("/config/broken_evidence_links")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        value
            .pointer("/config/weak_evidence_references")
            .and_then(Value::as_u64),
        Some(1)
    );
    let diagnostic = value
        .pointer("/config/diagnostic")
        .and_then(Value::as_str)
        .unwrap_or_else(|| std::panic::panic_any("doctor diagnostic should be a string"));
    assert!(
        diagnostic.contains("allow-doc-link link `doc:docs/missing-rationale.md`"),
        "doctor diagnostic should identify the broken traceability link: {diagnostic}"
    );
    assert!(
        diagnostic.contains("local link file is missing"),
        "doctor diagnostic should use link-specific wording: {diagnostic}"
    );
    remove_doctor_fixture_dir(root);
}

#[test]
fn doctor_reports_untracked_local_evidence_as_broken_by_default() {
    let root = doctor_fixture_dir();
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    let policy = root.join("policy/allow.toml");
    fs::write(&policy, policy_with_untracked_local_evidence())
        .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "policy/allow.toml"]);
    git(&root, &["commit", "-m", "base policy"]);
    fs::write(root.join("policy/evidence.md"), "untracked evidence")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write evidence: {err}")));
    let output = root.join("doctor.json");

    cmd_doctor(&DoctorArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: Some(policy),
        profile: None,
        format: HumanJsonFormat::Json,
        require_clean: false,
        output: Some(output.clone()),
        support_bundle: None,
    })
    .unwrap_or_else(|err| std::panic::panic_any(format!("doctor should pass: {err}")));

    let json = fs::read_to_string(&output)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read doctor output: {err}")));
    let value = parse_json_artifact("doctor", &json, allow_report::DOCTOR_SCHEMA_ID, "doctor");
    assert_eq!(
        value.pointer("/config/valid").and_then(Value::as_bool),
        Some(false),
        "untracked local evidence should make doctor config invalid by default"
    );
    assert_eq!(
        value
            .pointer("/config/broken_evidence_links")
            .and_then(Value::as_u64),
        Some(1)
    );
    let diagnostic = value
        .pointer("/config/diagnostic")
        .and_then(Value::as_str)
        .unwrap_or_else(|| std::panic::panic_any("doctor diagnostic should be a string"));
    assert!(
        diagnostic.contains("not in the default source-tree inventory"),
        "doctor diagnostic should explain the source-tree inventory boundary: {diagnostic}"
    );
    remove_doctor_fixture_dir(root);
}

#[test]
fn spec_system_doctor_reports_missing_readiness() {
    let root = doctor_fixture_dir();
    let output = root.join("doctor.json");

    cmd_doctor(&DoctorArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: None,
        profile: Some(ProfileArg::SpecSystem),
        format: HumanJsonFormat::Json,
        require_clean: false,
        output: Some(output.clone()),
        support_bundle: None,
    })
    .unwrap_or_else(|err| {
        std::panic::panic_any(format!("spec-system doctor should pass advisory: {err}"))
    });

    let json = fs::read_to_string(&output)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read doctor output: {err}")));
    let value = parse_json_artifact(
        "spec-system doctor",
        &json,
        allow_report::SPEC_SYSTEM_SCHEMA_ID,
        "doctor",
    );
    assert_eq!(
        value.pointer("/readiness/ready").and_then(Value::as_bool),
        Some(false)
    );
    assert!(
        readiness_check(&value, "profile_config")
            .is_some_and(
                |check| check.pointer("/status").and_then(Value::as_str) == Some("missing")
            ),
        "missing profile config should be reported: {json}"
    );
    assert!(
        readiness_check(&value, "artifact_ledger")
            .is_some_and(
                |check| check.pointer("/status").and_then(Value::as_str) == Some("missing")
            ),
        "missing doc artifact ledger should be reported: {json}"
    );
    remove_doctor_fixture_dir(root);
}

#[test]
fn spec_system_doctor_reports_ready_when_bootstrap_files_exist() {
    let root = doctor_fixture_dir();
    write_valid_spec_system_readiness_fixture(&root);
    let output = root.join("doctor.json");

    cmd_doctor(&DoctorArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: None,
        profile: Some(ProfileArg::SpecSystem),
        format: HumanJsonFormat::Json,
        require_clean: false,
        output: Some(output.clone()),
        support_bundle: None,
    })
    .unwrap_or_else(|err| {
        std::panic::panic_any(format!("spec-system doctor should pass advisory: {err}"))
    });

    let json = fs::read_to_string(&output)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read doctor output: {err}")));
    let value = parse_json_artifact(
        "spec-system doctor",
        &json,
        allow_report::SPEC_SYSTEM_SCHEMA_ID,
        "doctor",
    );
    assert_eq!(
        value.pointer("/readiness/ready").and_then(Value::as_bool),
        Some(true),
        "complete profile bootstrap should be ready: {json}"
    );
    assert_eq!(
        value.pointer("/readiness/mode").and_then(Value::as_str),
        Some("advisory")
    );
    assert!(
        readiness_check(&value, "templates")
            .is_some_and(|check| check.pointer("/status").and_then(Value::as_str) == Some("ready")),
        "templates should be ready: {json}"
    );
    remove_doctor_fixture_dir(root);
}

fn spec_system_init_fixture(root: &Path) {
    let parsed = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "init",
        "--root",
        &root.display().to_string(),
        "--profile",
        "spec-system",
    ]))
    .unwrap_or_else(|err| {
        std::panic::panic_any(format!("CLI should parse spec-system init: {err}"))
    });
    let Some(CargoAllowCommand::Init(args)) = parsed.command else {
        std::panic::panic_any("expected init command");
    };
    cmd_init(&args).unwrap_or_else(|err| {
        std::panic::panic_any(format!("spec-system init should pass: {err}"))
    });
}

#[test]
fn spec_system_doctor_recognizes_allow_init_layout() {
    let root = doctor_fixture_dir();
    spec_system_init_fixture(&root);
    let output = root.join("doctor-allow.json");

    cmd_doctor(&DoctorArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: None,
        profile: Some(ProfileArg::SpecSystem),
        format: HumanJsonFormat::Json,
        require_clean: false,
        output: Some(output.clone()),
        support_bundle: None,
    })
    .unwrap_or_else(|err| {
        std::panic::panic_any(format!("spec-system doctor should pass advisory: {err}"))
    });

    let json = fs::read_to_string(&output)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read doctor output: {err}")));
    let value = parse_json_artifact(
        "spec-system doctor allow layout",
        &json,
        allow_report::SPEC_SYSTEM_SCHEMA_ID,
        "doctor",
    );
    assert_eq!(
        value.get("config_source").and_then(Value::as_str),
        Some(".allow/profiles/spec-system.toml")
    );
    assert_eq!(
        value.get("config_provenance").and_then(Value::as_str),
        Some("allow_profiles")
    );
    assert!(
        readiness_check(&value, "allow_imports").is_some_and(|check| {
            check.pointer("/status").and_then(Value::as_str) == Some("ready")
        }),
        "doctor should recognize owned import root: {json}"
    );
    remove_doctor_fixture_dir(root);
}

#[test]
fn spec_system_doctor_reports_profile_config_provenance() {
    let root = doctor_fixture_dir();
    write_valid_spec_system_readiness_fixture(&root);
    let output = root.join("doctor-provenance.json");

    cmd_doctor(&DoctorArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: None,
        profile: Some(ProfileArg::SpecSystem),
        format: HumanJsonFormat::Json,
        require_clean: false,
        output: Some(output.clone()),
        support_bundle: None,
    })
    .unwrap_or_else(|err| {
        std::panic::panic_any(format!("spec-system doctor should pass advisory: {err}"))
    });

    let json = fs::read_to_string(&output)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read doctor output: {err}")));
    let value = parse_json_artifact(
        "spec-system doctor provenance",
        &json,
        allow_report::SPEC_SYSTEM_SCHEMA_ID,
        "doctor",
    );
    assert_eq!(
        value.get("config_source").and_then(Value::as_str),
        Some("policy/spec-system.toml")
    );
    assert_eq!(
        value.get("config_provenance").and_then(Value::as_str),
        Some("legacy_policy")
    );
    assert!(
        readiness_check(&value, "profile_config")
            .and_then(|check| check.get("message"))
            .and_then(Value::as_str)
            .is_some_and(|message| message.contains("provenance: legacy_policy")),
        "doctor readiness should report provenance: {json}"
    );
    remove_doctor_fixture_dir(root);
}

#[test]
fn spec_system_doctor_treats_bootstrap_active_goal_as_optional() {
    let root = doctor_fixture_dir();
    write_valid_spec_system_readiness_fixture(&root);
    fs::write(
        root.join("policy/spec-system.toml"),
        spec_system_config().replace(
            "active_goal_required = true",
            "active_goal_required = false",
        ),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write spec-system config: {err}")));
    fs::write(
        root.join("policy/doc-artifacts.toml"),
        r#"
schema_version = "1.0"
policy = "cargo-allow-doc-artifacts"
owner = "repo-infra"
status = "advisory"
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write doc artifacts: {err}")));
    fs::write(
        root.join(".codex/goals/active.toml"),
        r#"
schema_version = "1.0"
id = "spec-system-profile"
title = "Spec-system profile"
status = "active"
owner = "codex"
created = "YYYY-MM-DD"
linked_plan = "plans/spec-system/implementation-plan.md"
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write active goal: {err}")));
    let output = root.join("doctor.json");

    cmd_doctor(&DoctorArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: None,
        profile: Some(ProfileArg::SpecSystem),
        format: HumanJsonFormat::Json,
        require_clean: false,
        output: Some(output.clone()),
        support_bundle: None,
    })
    .unwrap_or_else(|err| {
        std::panic::panic_any(format!("spec-system doctor should pass advisory: {err}"))
    });

    let json = fs::read_to_string(&output)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read doctor output: {err}")));
    let value = parse_json_artifact(
        "spec-system doctor",
        &json,
        allow_report::SPEC_SYSTEM_SCHEMA_ID,
        "doctor",
    );
    assert_eq!(
        value.pointer("/readiness/ready").and_then(Value::as_bool),
        Some(true),
        "optional bootstrap active goal should not make readiness fail: {json}"
    );
    assert!(
        readiness_check(&value, "active_goal").is_some_and(|check| {
            check.pointer("/status").and_then(Value::as_str) == Some("ready")
                && check
                    .pointer("/message")
                    .and_then(Value::as_str)
                    .is_some_and(|message| message.contains("active_goal_required = false"))
        }),
        "active goal readiness should explain optional validation: {json}"
    );
    remove_doctor_fixture_dir(root);
}

#[test]
fn doctor_reports_custom_file_families_and_conflicts() {
    let root = doctor_fixture_dir();
    fs::create_dir_all(root.join("models"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create model fixture: {err}")));
    for name in ["plain.onnx", "release.onnx", "conflict.onnx"] {
        fs::write(root.join("models").join(name), b"fixture")
            .unwrap_or_else(|err| std::panic::panic_any(format!("write model fixture: {err}")));
    }
    let config = root.join("allow.toml");
    fs::write(
        &config,
        r#"schema_version = "0.1"
policy = "cargo-allow"

[workspace]
ignored = []
generated = []

[[workspace.file_family]]
id = "model-artifact"
family = "ml_model"
glob = "models/*.onnx"
reason = "Govern model artifacts."

[[workspace.file_family]]
id = "release-metadata"
family = "release_metadata"
glob = "models/release.onnx"
reason = "Govern release metadata."

[[workspace.file_family]]
id = "conflict-a"
family = "family_a"
glob = "models/conflict.onnx"
reason = "Conflict fixture A."

[[workspace.file_family]]
id = "conflict-b"
family = "family_b"
glob = "models/conflict.onnx"
reason = "Conflict fixture B."
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write doctor policy: {err}")));
    let output = root.join("doctor-file-families.json");

    cmd_doctor(&DoctorArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: Some(config),
        profile: None,
        format: HumanJsonFormat::Json,
        require_clean: false,
        output: Some(output.clone()),
        support_bundle: None,
    })
    .unwrap_or_else(|err| std::panic::panic_any(format!("doctor should pass: {err}")));

    let json = fs::read_to_string(&output)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read doctor output: {err}")));
    let value = parse_json_artifact(
        "doctor custom file families",
        &json,
        allow_report::DOCTOR_SCHEMA_ID,
        "doctor",
    );
    let configured = value
        .pointer("/file_families/configured")
        .and_then(Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("configured file families should be an array"));
    assert_eq!(configured.len(), 4, "{json}");
    assert!(configured.iter().any(|rule| {
        rule.get("id").and_then(Value::as_str) == Some("release-metadata")
            && rule.get("matched_files").and_then(Value::as_u64) == Some(1)
    }));
    let conflicts = value
        .pointer("/file_families/conflicts")
        .and_then(Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("file family conflicts should be an array"));
    assert_eq!(conflicts.len(), 1, "{json}");
    assert_eq!(
        conflicts[0].get("path").and_then(Value::as_str),
        Some("models/conflict.onnx")
    );
    assert_eq!(
        conflicts[0]
            .get("rule_ids")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    let human_output = root.join("doctor-file-families.txt");
    cmd_doctor(&DoctorArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: Some(root.join("allow.toml")),
        profile: None,
        format: HumanJsonFormat::Human,
        require_clean: false,
        output: Some(human_output.clone()),
        support_bundle: None,
    })
    .unwrap_or_else(|err| std::panic::panic_any(format!("human doctor should pass: {err}")));
    let human = fs::read_to_string(human_output)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read human doctor output: {err}")));
    assert!(
        human.contains("custom file families: 4 configured"),
        "{human}"
    );
    assert!(human.contains("matched files=1"), "{human}");
    assert!(human.contains("custom file family conflicts: 1"), "{human}");
    assert!(human.contains("models/conflict.onnx"), "{human}");
    remove_doctor_fixture_dir(root);
}

fn federation_fixture_path(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/federation")
        .join(name)
}

#[test]
fn doctor_reports_configured_federation_ledgers() {
    let root = doctor_fixture_dir();
    fs::create_dir_all(root.join(".allow"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create .allow dir: {err}")));
    fs::copy(
        federation_fixture_path("multi-ledger-config.toml"),
        root.join(".allow/config.toml"),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("copy federation config: {err}")));
    let output = root.join("doctor-federation.json");

    cmd_doctor(&DoctorArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: None,
        profile: None,
        format: HumanJsonFormat::Json,
        require_clean: false,
        output: Some(output.clone()),
        support_bundle: None,
    })
    .unwrap_or_else(|err| std::panic::panic_any(format!("doctor should pass: {err}")));

    let json = fs::read_to_string(&output)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read doctor output: {err}")));
    let value = parse_json_artifact(
        "doctor federation",
        &json,
        allow_report::DOCTOR_SCHEMA_ID,
        "doctor",
    );
    assert_eq!(
        value.pointer("/federation/found").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        value.pointer("/federation/valid").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        value
            .pointer("/federation/provenance")
            .and_then(Value::as_str),
        Some("fixed_allow_config")
    );
    let ledgers = value
        .pointer("/federation/configured_ledgers")
        .and_then(Value::as_array);
    assert!(
        ledgers.as_ref().is_some_and(|entries| {
            entries.len() == 3
                && entries
                    .iter()
                    .any(|ledger| ledger.get("id").and_then(Value::as_str) == Some("source-policy"))
        }),
        "expected three configured ledgers including source-policy: {json}"
    );
    remove_doctor_fixture_dir(root);
}

#[test]
fn spec_system_doctor_reports_federation_ledgers_readiness() {
    let root = doctor_fixture_dir();
    spec_system_init_fixture(&root);
    fs::copy(
        federation_fixture_path("multi-ledger-config.toml"),
        root.join(".allow/config.toml"),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("copy federation config: {err}")));
    let output = root.join("doctor-federation-readiness.json");

    cmd_doctor(&DoctorArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: None,
        profile: Some(ProfileArg::SpecSystem),
        format: HumanJsonFormat::Json,
        require_clean: false,
        output: Some(output.clone()),
        support_bundle: None,
    })
    .unwrap_or_else(|err| {
        std::panic::panic_any(format!("spec-system doctor should pass advisory: {err}"))
    });

    let json = fs::read_to_string(&output)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read doctor output: {err}")));
    let value = parse_json_artifact(
        "spec-system doctor federation readiness",
        &json,
        allow_report::SPEC_SYSTEM_SCHEMA_ID,
        "doctor",
    );
    assert!(
        readiness_check(&value, "federation_ledgers").is_some_and(|check| {
            check.pointer("/status").and_then(Value::as_str) == Some("ready")
                && check
                    .pointer("/message")
                    .and_then(Value::as_str)
                    .is_some_and(|message| message.contains("3 configured ledger"))
        }),
        "federation readiness should report configured ledgers: {json}"
    );
    remove_doctor_fixture_dir(root);
}

fn doctor_fixture_dir() -> std::path::PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let dir =
        std::env::temp_dir().join(format!("cargo-allow-doctor-{}-{stamp}", std::process::id()));
    remove_doctor_fixture_dir(dir.clone());
    fs::create_dir_all(&dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("create doctor fixture: {err}")));
    dir
}

fn readiness_check<'a>(value: &'a Value, kind: &str) -> Option<&'a Value> {
    value
        .pointer("/readiness/checks")
        .and_then(Value::as_array)
        .and_then(|checks| {
            checks
                .iter()
                .find(|check| check.get("kind").and_then(Value::as_str) == Some(kind))
        })
}

fn write_valid_spec_system_readiness_fixture(root: &Path) {
    for dir in [
        "docs/proposals",
        "docs/specs",
        "docs/adr",
        "plans",
        ".codex/goals",
        "docs/templates",
        "docs/status",
        "policy",
    ] {
        fs::create_dir_all(root.join(dir))
            .unwrap_or_else(|err| std::panic::panic_any(format!("create {dir}: {err}")));
    }
    fs::write(root.join("policy/spec-system.toml"), spec_system_config())
        .unwrap_or_else(|err| std::panic::panic_any(format!("write spec-system config: {err}")));
    fs::write(root.join("policy/doc-artifacts.toml"), doc_artifacts())
        .unwrap_or_else(|err| std::panic::panic_any(format!("write doc artifacts: {err}")));
    fs::write(root.join("docs/status/SUPPORT_TIERS.md"), support_tiers())
        .unwrap_or_else(|err| std::panic::panic_any(format!("write support tiers: {err}")));
    fs::write(
        root.join(".codex/goals/active.toml"),
        active_goal_manifest(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write active goal: {err}")));
    for path in [
        "docs/templates/proposal.md",
        "docs/templates/spec.md",
        "docs/templates/adr.md",
        "docs/templates/implementation-plan.md",
        "docs/templates/plan-item.md",
        "docs/templates/closeout.md",
        "docs/templates/pr-body.md",
    ] {
        fs::write(root.join(path), "template\n")
            .unwrap_or_else(|err| std::panic::panic_any(format!("write {path}: {err}")));
    }
}

fn spec_system_config() -> &'static str {
    r#"
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

[requirements]
ledger_required = true
templates_required = true
support_tiers_required = true
active_goal_required = true
closeout_required_for_done_items = true
"#
}

fn doc_artifacts() -> &'static str {
    r#"
schema_version = "1.0"
policy = "cargo-allow-doc-artifacts"
owner = "repo-infra"
status = "advisory"

[[artifact]]
id = "CARGO-ALLOW-PROP-0001"
kind = "proposal"
path = "docs/proposals/CARGO-ALLOW-PROP-0001-example.md"
status = "accepted"
owner = "repo-infra"
created = "2026-06-12"

[[artifact]]
id = "CARGO-ALLOW-SPEC-0001"
kind = "spec"
path = "docs/specs/CARGO-ALLOW-SPEC-0001-example.md"
status = "accepted"
owner = "repo-infra"
created = "2026-06-12"
linked_proposal = "CARGO-ALLOW-PROP-0001"

[[artifact]]
id = "CARGO-ALLOW-GOAL-0001"
kind = "active_goal"
path = ".codex/goals/active.toml"
status = "active"
owner = "codex"
created = "2026-06-12"
linked_proposal = "CARGO-ALLOW-PROP-0001"
linked_spec = "CARGO-ALLOW-SPEC-0001"
linked_plan = "plans/spec-system/implementation-plan.md"

[[artifact]]
id = "CARGO-ALLOW-PLAN-0001"
kind = "implementation_plan"
path = "plans/spec-system/implementation-plan.md"
status = "active"
owner = "repo-infra"
created = "2026-06-12"
linked_proposal = "CARGO-ALLOW-PROP-0001"
linked_spec = "CARGO-ALLOW-SPEC-0001"
"#
}

fn active_goal_manifest() -> &'static str {
    r#"
schema_version = "1.0"
id = "CARGO-ALLOW-GOAL-0001"
title = "Spec-system profile"
status = "active"
owner = "codex"
created = "2026-06-12"
linked_proposal = "CARGO-ALLOW-PROP-0001"
linked_spec = "CARGO-ALLOW-SPEC-0001"
linked_plan = "plans/spec-system/implementation-plan.md"
linked_plan_status = "active"

[[work_item]]
id = "spec-system-pr-001"
status = "ready"
title = "Keep graph valid"
linked_spec = "CARGO-ALLOW-SPEC-0001"
linked_plan = "plans/spec-system/implementation-plan.md"
proof_commands = [
  "cargo-allow check --profile spec-system --mode audit",
]
"#
}

fn support_tiers() -> &'static str {
    r#"
# Support Tiers

| Surface | Tier | Claim | Proof command | Notes |
| --- | --- | --- | --- | --- |
| Spec-system profile | Advisory | Source-of-truth graph artifacts can be linted. | cargo-allow check --profile spec-system --mode audit | Structural only. |
"#
}

fn git(root: &std::path::Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("git {args:?}: {err}")));
    if !output.status.success() {
        std::panic::panic_any(format!(
            "git {args:?} failed: stdout=`{}` stderr=`{}`",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
}

fn policy_with_untracked_local_evidence() -> &'static str {
    r#"policy = "cargo-allow"

[workspace]
ignored = ["policy/evidence.md"]

[[allow]]
id = "allow-policy"
kind = "non_rust_file"
family = "configuration"
path = "policy/allow.toml"
owner = "core"
classification = "fixture"
reason = "fixture policy file"
evidence = ["doc:policy/evidence.md"]
review_after = "2026-08-01"

[allow.selector]
ast_kind = "tracked_file"
"#
}

fn remove_doctor_fixture_dir(path: std::path::PathBuf) {
    // Under parallel suite load (notably on Windows), fixture files can be
    // held open momentarily and removal fails with a transient sharing
    // violation. Retry with a short backoff before giving up: a leaked
    // fixture directory is unique to this test process and preferable to
    // failing an otherwise green suite run.
    const ATTEMPTS: usize = 8;
    for attempt in 0..ATTEMPTS {
        match fs::remove_dir_all(&path) {
            Ok(()) => return,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return,
            Err(err) => {
                if attempt + 1 == ATTEMPTS {
                    std::panic::panic_any(format!(
                        "remove doctor fixture {}: {err}",
                        path.display()
                    ));
                }
                std::thread::sleep(std::time::Duration::from_millis(50 * (attempt as u64 + 1)));
            }
        }
    }
}
