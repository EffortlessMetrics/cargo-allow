use super::{PruneArgs, cmd_prune};
use crate::{HumanJsonFormat, RootArgs};
use allow_core::{
    AllowConfig, AllowEntry, CargoAllowErrorKind, FindingKind, LastSeen, Lifecycle, Selector,
};
use allow_policy::{load_policy, render_policy};
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

type TestResult = Result<(), Box<dyn std::error::Error>>;
static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    policy: PathBuf,
    output: PathBuf,
}

impl Fixture {
    fn args(&self, id: Option<&str>, write: bool) -> PruneArgs {
        PruneArgs {
            root: RootArgs {
                root: Some(self.root.clone()),
            },
            config: Some(self.policy.clone()),
            stale: true,
            allow_id: id.map(str::to_owned),
            dry_run: !write,
            write,
            include_untracked: false,
            format: HumanJsonFormat::Json,
            output: Some(self.output.clone()),
        }
    }

    fn artifact(&self) -> Result<Value, Box<dyn std::error::Error>> {
        Ok(serde_json::from_slice(&fs::read(&self.output)?)?)
    }
}

fn require(condition: bool, message: &str) -> TestResult {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

fn with_fixture(test: impl FnOnce(&Fixture) -> TestResult) -> TestResult {
    let parent = std::env::temp_dir().canonicalize()?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let root = parent.join(format!(
        "cargo-allow-prune-selection-{}-{stamp}-{}",
        std::process::id(),
        NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root)?;
    let result = (|| {
        fs::create_dir(root.join("policy"))?;
        fs::create_dir(root.join("docs"))?;
        fs::create_dir(root.join("target"))?;
        fs::write(root.join("docs/live.md"), "# retained live document\n")?;
        let fixture = Fixture {
            policy: root.join("policy/allow.toml"),
            output: root.join("target/prune.json"),
            root: root.clone(),
        };
        let mut config = AllowConfig::empty();
        for (id, path) in [
            ("allow-stale-a", "docs/missing-a.md"),
            ("allow-stale-b", "docs/missing-b.md"),
            ("allow-live", "docs/live.md"),
        ] {
            config.allow.push(AllowEntry {
                id: id.to_owned(),
                kind: FindingKind::NonRustFile,
                family: Some("documentation".to_owned()),
                path: Some(path.into()),
                glob: None,
                owner: format!("owner/{id}"),
                classification: "reviewed_exception".to_owned(),
                reason: format!("retained judgment for {id}"),
                evidence: Vec::new(),
                links: Vec::new(),
                occurrence_limit: Some(1),
                lifecycle: Lifecycle {
                    review_after: Some("2027-01-01".to_owned()),
                    ..Lifecycle::empty()
                },
                selector: Selector {
                    ast_kind: Some("tracked_file".to_owned()),
                    ..Selector::default()
                },
                last_seen: None,
            });
        }
        fs::write(&fixture.policy, render_policy(&config))?;
        test(&fixture)
    })();
    require(
        root.canonicalize()?.starts_with(&parent),
        "fixture escaped its temporary parent",
    )?;
    let cleanup = fs::remove_dir_all(&root);
    result?;
    cleanup?;
    Ok(())
}

fn require_candidates(artifact: &Value, ids: &[&str]) -> TestResult {
    require(
        artifact
            .pointer("/summary/stale_entries")
            .and_then(Value::as_u64)
            == Some(ids.len() as u64),
        "summary must count only selected candidates",
    )?;
    let entries = artifact
        .get("stale_entries")
        .and_then(Value::as_array)
        .ok_or("missing stale entries")?;
    let actual = entries
        .iter()
        .map(|entry| entry.get("id").and_then(Value::as_str))
        .collect::<Vec<_>>();
    require(
        actual == ids.iter().copied().map(Some).collect::<Vec<_>>(),
        "preview candidate IDs differ",
    )?;
    require(
        artifact.pointer("/mutation_receipt/changed_allow_ids") == Some(&json!(ids)),
        "receipt must name exactly the selected candidates",
    )?;
    for field in ["before_fingerprints", "after_fingerprints"] {
        require(
            artifact
                .pointer(&format!("/mutation_receipt/{field}"))
                .and_then(Value::as_array)
                .map(Vec::len)
                == Some(ids.len()),
            "receipt fingerprint denominator differs from selection",
        )?;
    }
    require(
        artifact
            .get("removed_toml_blocks")
            .and_then(Value::as_array)
            .map(Vec::len)
            == Some(ids.len()),
        "TOML removal denominator differs from selection",
    )
}

#[test]
fn prune_selection_preview_limits_json_human_and_receipt() -> TestResult {
    with_fixture(|fixture| {
        let before = fs::read(&fixture.policy)?;
        let config = load_policy(&fixture.policy)?;
        let selected = config
            .allow
            .iter()
            .find(|entry| entry.id == "allow-stale-a")
            .ok_or("missing selected fixture entry")?;
        let fingerprint = allow_core::allow_entry_content_fingerprint(selected);
        cmd_prune(&fixture.args(Some("allow-stale-a"), false))?;
        let artifact = fixture.artifact()?;
        require_candidates(&artifact, &["allow-stale-a"])?;
        require(
            artifact.pointer("/mutation_receipt/before_fingerprints")
                == Some(&json!([fingerprint])),
            "wrong selected fingerprint",
        )?;
        require(
            artifact.pointer("/mutation_receipt/after_fingerprints") == Some(&json!([null])),
            "prune receipt should remove the selected fingerprint",
        )?;
        let block = artifact
            .pointer("/removed_toml_blocks/0")
            .and_then(Value::as_str)
            .ok_or("missing removal block")?;
        require(
            block.contains("id = \"allow-stale-a\"") && !block.contains("allow-stale-b"),
            "wrong TOML removal block",
        )?;
        let mut args = fixture.args(Some("allow-stale-a"), false);
        args.format = HumanJsonFormat::Human;
        args.dry_run = false; // The default preview must preserve selection too.
        cmd_prune(&args)?;
        let human = fs::read_to_string(&fixture.output)?;
        require(
            human.contains("allow-stale-a")
                && !human.contains("allow-stale-b")
                && !human.contains("allow-live"),
            "human preview widened selection",
        )?;
        require(
            fs::read(&fixture.policy)? == before,
            "preview changed policy",
        )
    })
}

#[test]
fn prune_selection_summary_apply_command_retains_selected_id() -> TestResult {
    use crate::core_command_summary::{PruneSummaryFactsV1, core_command_summary_from_prune};
    let summary = core_command_summary_from_prune(PruneSummaryFactsV1 {
        repository_identity: "local-repository:test".to_owned(),
        portable_identity: "worktree:prune:policy/allow.toml:1".to_owned(),
        policy_path: "policy/allow.toml".to_owned(),
        candidate_count: 1,
        allow_id: Some("allow-stale-a".to_owned()),
        write_requested: false,
        dry_run: true,
        completeness: effortless_repo_protocol::CompletenessV1::Complete,
    })?;
    let action = summary.primary_action.ok_or("missing apply command")?;
    require(
        action.args
            == [
                "prune",
                "--stale",
                "--config",
                "policy/allow.toml",
                "--allow-id",
                "allow-stale-a",
                "--write",
            ],
        "summary apply command widened selected preview to bulk removal",
    )?;
    with_fixture(|fixture| {
        use clap::Parser;
        let mut argv = vec!["cargo-allow".to_owned()];
        argv.extend(action.args.clone());
        argv.extend([
            "--root".to_owned(),
            fixture.root.to_string_lossy().into_owned(),
        ]);
        let parsed = crate::CargoAllowCli::try_parse_from(argv)?;
        let Some(crate::CargoAllowCommand::Prune(args)) = parsed.command else {
            return Err("summary action did not parse as prune".into());
        };
        let mut expected = load_policy(&fixture.policy)?;
        expected.allow.retain(|entry| entry.id != "allow-stale-a");
        cmd_prune(&args)?;
        require(
            load_policy(&fixture.policy)? == expected,
            "executing the summary action changed an unselected entry",
        )
    })
}

#[test]
fn prune_selection_write_preserves_every_unselected_entry() -> TestResult {
    with_fixture(|fixture| {
        let mut expected = load_policy(&fixture.policy)?;
        expected.allow.retain(|entry| entry.id != "allow-stale-a");
        cmd_prune(&fixture.args(Some("allow-stale-a"), true))?;
        require(
            load_policy(&fixture.policy)? == expected,
            "selected write changed an unselected entry or global policy",
        )?;
        require_candidates(&fixture.artifact()?, &["allow-stale-a"])?;
        let bytes = fs::read(&fixture.policy)?;
        fs::write(&fixture.output, "preserve existing output")?;
        let error = cmd_prune(&fixture.args(Some("allow-stale-a"), true))
            .err()
            .ok_or("removed ID must not select remaining stale entries")?;
        require(
            error.kind() == CargoAllowErrorKind::Usage,
            "missing selected ID must be a usage error",
        )?;
        require(
            fs::read(&fixture.policy)? == bytes,
            "second selected write changed policy",
        )?;
        require(
            fs::read_to_string(&fixture.output)? == "preserve existing output",
            "failed second selection replaced output",
        )
    })
}

#[test]
fn prune_selection_live_and_location_drift_are_no_ops() -> TestResult {
    for last_seen in [
        None,
        Some(LastSeen {
            line: 42,
            column: 1,
        }),
    ] {
        with_fixture(|fixture| {
            let mut config = load_policy(&fixture.policy)?;
            config
                .allow
                .iter_mut()
                .find(|entry| entry.id == "allow-live")
                .ok_or("missing live fixture entry")?
                .last_seen = last_seen;
            fs::write(&fixture.policy, render_policy(&config))?;
            let before = fs::read(&fixture.policy)?;
            cmd_prune(&fixture.args(Some("allow-live"), true))?;
            let artifact = fixture.artifact()?;
            require_candidates(&artifact, &[])?;
            require(
                artifact.pointer("/mode/written_path") == Some(&Value::Null),
                "non-stale selection wrote policy",
            )?;
            require(
                fs::read(&fixture.policy)? == before,
                "non-stale selection removed other stale rows",
            )
        })?;
    }
    Ok(())
}

#[test]
fn prune_selection_unknown_id_preserves_policy_and_output() -> TestResult {
    with_fixture(|fixture| {
        let before = fs::read(&fixture.policy)?;
        for write in [false, true] {
            fs::write(&fixture.output, "preserve existing output")?;
            let error = cmd_prune(&fixture.args(Some("allow-missing"), write))
                .err()
                .ok_or("unknown ID must fail")?;
            require(
                error.kind() == CargoAllowErrorKind::Usage
                    && error.to_string().contains("allow-missing"),
                "unknown ID needs a typed diagnostic naming the selection",
            )?;
            require(
                fs::read(&fixture.policy)? == before,
                "unknown selection changed policy",
            )?;
            require(
                fs::read_to_string(&fixture.output)? == "preserve existing output",
                "unknown selection replaced output",
            )?;
        }
        Ok(())
    })
}

#[test]
fn prune_selection_default_bulk_and_second_no_op_remain_available() -> TestResult {
    with_fixture(|fixture| {
        let mut expected = load_policy(&fixture.policy)?;
        expected.allow.retain(|entry| entry.id == "allow-live");
        cmd_prune(&fixture.args(None, true))?;
        require_candidates(&fixture.artifact()?, &["allow-stale-a", "allow-stale-b"])?;
        require(
            load_policy(&fixture.policy)? == expected,
            "bulk prune did not preserve the live row",
        )?;
        let before = fs::read(&fixture.policy)?;
        cmd_prune(&fixture.args(None, true))?;
        require_candidates(&fixture.artifact()?, &[])?;
        require(
            fs::read(&fixture.policy)? == before,
            "second bulk prune rewrote a no-op",
        )
    })
}

#[test]
fn prune_selection_write_validates_evidence_of_unselected_stale_rows() -> TestResult {
    with_fixture(|fixture| {
        let mut config = load_policy(&fixture.policy)?;
        config
            .allow
            .iter_mut()
            .find(|entry| entry.id == "allow-stale-b")
            .ok_or("missing retained stale fixture entry")?
            .evidence
            .push("doc:docs/missing-evidence.md".to_owned());
        fs::write(&fixture.policy, render_policy(&config))?;
        let before = fs::read(&fixture.policy)?;
        fs::write(&fixture.output, "preserve existing output")?;
        let error = cmd_prune(&fixture.args(Some("allow-stale-a"), true))
            .err()
            .ok_or("selected prune must validate retained broken evidence")?;
        require(
            error.to_string().contains("allow-stale-b")
                && error.to_string().contains("missing-evidence.md"),
            "diagnostic must identify the retained broken evidence",
        )?;
        require(
            fs::read(&fixture.policy)? == before,
            "evidence failure changed policy",
        )?;
        require(
            fs::read_to_string(&fixture.output)? == "preserve existing output",
            "evidence failure replaced output",
        )
    })
}
