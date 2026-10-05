use super::*;
use std::fs;

#[test]
fn saved_prune_output_allows_broken_evidence_preview() {
    let fixture = SourceTreeFixture::new("saved-prune-broken-evidence");
    fixture.write_policy_with_broken_evidence();

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let prune = artifact_dir.join("prune.json");

    run_cargo_allow(&[
        "prune",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--stale",
        "--format",
        "json",
        "--output",
        path_arg(&prune),
    ]);
    let value = assert_source_syntax_artifact(&prune, allow_report::PRUNE_SCHEMA_ID, "prune");
    assert_eq!(
        value
            .pointer("/summary/stale_entries")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "prune dry-run should still preview stale broken-evidence entries"
    );
    assert_eq!(
        value
            .pointer("/stale_entries/0/id")
            .and_then(serde_json::Value::as_str),
        Some("allow-broken-evidence"),
        "prune should include the stale broken-evidence allow entry"
    );
    assert_eq!(
        value
            .pointer("/mode/dry_run")
            .and_then(serde_json::Value::as_bool),
        Some(true),
        "prune should remain dry-run first"
    );
}

#[test]
fn saved_prune_write_output_records_written_policy() {
    let fixture = SourceTreeFixture::new("saved-prune-write-output");
    fixture.write_minimal_policy();
    fixture.write_panic_source();
    fixture.append_saved_artifact_allow_entries();

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let prune = artifact_dir.join("prune-write.json");

    run_cargo_allow(&[
        "prune",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--stale",
        "--write",
        "--format",
        "json",
        "--output",
        path_arg(&prune),
    ]);
    let value = assert_source_syntax_artifact(&prune, allow_report::PRUNE_SCHEMA_ID, "prune");
    assert_eq!(
        value
            .pointer("/mode/dry_run")
            .and_then(serde_json::Value::as_bool),
        Some(false),
        "prune write artifact should not report dry-run mode"
    );
    assert_eq!(
        value
            .pointer("/mode/write_requested")
            .and_then(serde_json::Value::as_bool),
        Some(true),
        "prune write artifact should record write mode"
    );
    let written_path = value
        .pointer("/mode/written_path")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| std::panic::panic_any("prune write artifact should include path"));
    assert!(
        written_path.ends_with("policy\\allow.toml") || written_path.ends_with("policy/allow.toml"),
        "prune written_path should identify policy/allow.toml: {written_path}"
    );
    assert_eq!(
        value
            .pointer("/summary/stale_entries")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "prune write artifact should preserve the stale-entry count"
    );
    let policy = fs::read_to_string(fixture.root.join("policy/allow.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read pruned policy: {err}")));
    assert!(policy.contains("allow-panic-fixture"));
    assert!(!policy.contains("allow-stale-fixture"));
}

#[test]
fn saved_prune_sole_headerless_entry_is_valid_empty_ledger()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = SourceTreeFixture::new("saved-prune-headerless-only");
    let prefix = "\r\n \t";
    let suffix = "\n \r\n";
    let review_after = allow_core::SimpleDate::today_utc_approx().add_days(30);
    let input = format!(
        "{prefix}[[allow]]\nid = 'allow-only'\nkind = 'panic'\npath = 'src/missing.rs'\nowner = 'core/tests'\nclassification = 'reviewed_fixture'\nreason = 'Sole headerless stale entry regression.'\nreview_after = '{review_after}'\n[allow.selector]\nast_kind = 'method_call'\ncallee = 'unwrap'\n{suffix}"
    );
    assert_eq!(allow_policy::parse_policy(&input)?.allow.len(), 1);
    let policy = fixture.root.join("policy/allow.toml");
    fs::write(&policy, &input)?;
    let artifacts = fixture.root.join("target/cargo-allow");
    let preview = artifacts.join("preview.json");
    run_cargo_allow(&[
        "prune",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--stale",
        "--format",
        "json",
        "--output",
        path_arg(&preview),
    ]);
    let preview_value =
        assert_source_syntax_artifact(&preview, allow_report::PRUNE_SCHEMA_ID, "prune");
    assert_eq!(
        preview_value
            .pointer("/summary/stale_entries")
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
    assert_eq!(fs::read(&policy)?, input.as_bytes());
    let preview_bytes = fs::read(&preview)?;

    let output_path = artifacts.join("write.json");
    let sentinel = b"existing operator output\n";
    fs::write(&output_path, sentinel)?;
    let output = super::support::cargo_allow_command()
        .args([
            "prune",
            "--root",
            fixture.root_str(),
            "--config",
            "policy/allow.toml",
            "--stale",
            "--write",
            "--format",
            "json",
            "--output",
            path_arg(&output_path),
        ])
        .output()?;
    if !output.status.success() {
        assert_eq!(
            fs::read(&policy)?,
            input.as_bytes(),
            "refusal must preserve policy"
        );
        assert_eq!(
            fs::read(&output_path)?,
            sentinel,
            "refusal must preserve existing output"
        );
        eprintln!(
            "headerless prune refused before writing; policy and output preserved; stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert!(
        output.status.success(),
        "sole valid stale entry must prune to a readable empty ledger: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected = format!("{prefix}{suffix}policy = \"cargo-allow\"\n");
    assert_eq!(fs::read(&policy)?, expected.as_bytes());
    assert!(
        allow_policy::parse_policy(&fs::read_to_string(&policy)?)?
            .allow
            .is_empty()
    );
    let written =
        assert_source_syntax_artifact(&output_path, allow_report::PRUNE_SCHEMA_ID, "prune");
    assert_eq!(
        written
            .pointer("/summary/stale_entries")
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
    assert!(
        written
            .pointer("/mode/written_path")
            .and_then(serde_json::Value::as_str)
            .is_some()
    );

    // A fixed old timestamp makes an identical-byte rewrite detectable even
    // when repeated commands finish within the filesystem's clock precision.
    fs::File::options().write(true).open(&policy)?.set_times(
        fs::FileTimes::new()
            .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000)),
    )?;
    let modified = fs::metadata(&policy)?.modified()?;
    for name in ["noop-first.json", "noop-second.json"] {
        let noop = artifacts.join(name);
        run_cargo_allow(&[
            "prune",
            "--root",
            fixture.root_str(),
            "--config",
            "policy/allow.toml",
            "--stale",
            "--write",
            "--format",
            "json",
            "--output",
            path_arg(&noop),
        ]);
        let value = assert_source_syntax_artifact(&noop, allow_report::PRUNE_SCHEMA_ID, "prune");
        assert_eq!(
            value
                .pointer("/summary/stale_entries")
                .and_then(serde_json::Value::as_u64),
            Some(0)
        );
        assert!(
            value
                .pointer("/mode/written_path")
                .is_none_or(serde_json::Value::is_null)
        );
        assert_eq!(fs::read(&policy)?, expected.as_bytes());
        assert_eq!(
            fs::metadata(&policy)?.modified()?,
            modified,
            "no-op must not replace policy"
        );
        assert_eq!(
            fs::read(&preview)?,
            preview_bytes,
            "saved preview stays unchanged"
        );
    }
    Ok(())
}
