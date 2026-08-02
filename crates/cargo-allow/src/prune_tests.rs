use super::*;
use crate::{CargoAllowCli, CargoAllowCommand, HumanJsonFormat};
use allow_core::{AllowEntry, Lifecycle, Selector};
use allow_policy::load_policy;
use clap::Parser;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

fn argv(items: Vec<&str>) -> Vec<String> {
    items.into_iter().map(String::from).collect()
}

#[test]
fn clap_parses_prune_stale_dry_run() {
    let parsed = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "prune",
        "--stale",
        "--dry-run",
        "--include-untracked",
        "--format",
        "json",
        "--output",
        "target/prune.json",
    ]))
    .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse: {err}")));

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::Prune(PruneArgs {
            stale: true,
            dry_run: true,
            include_untracked: true,
            format: HumanJsonFormat::Json,
            output: Some(path),
            ..
        })) if path == Path::new("target/prune.json")
    ));
}

#[test]
fn clap_parses_prune_stale_write() {
    let parsed =
        CargoAllowCli::try_parse_from(argv(vec!["cargo-allow", "prune", "--stale", "--write"]))
            .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse: {err}")));

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::Prune(PruneArgs {
            stale: true,
            write: true,
            ..
        }))
    ));
}

#[test]
fn clap_rejects_prune_dry_run_and_write_together() {
    let parsed = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "prune",
        "--stale",
        "--dry-run",
        "--write",
    ]));

    assert!(parsed.is_err());
}

#[test]
fn prune_stale_candidates_only_include_stale_entries() {
    let mut cfg = AllowConfig::empty();
    cfg.allow
        .push(test_entry("allow-stale", FindingKind::Panic));
    cfg.allow.push(test_entry("allow-live", FindingKind::Panic));
    let outcomes = vec![
        test_outcome(MatchStatus::Stale, Some("allow-stale"), None, "stale"),
        test_outcome(MatchStatus::Matched, Some("allow-live"), Some(0), "matched"),
    ];

    let candidates = prune_stale_candidates(&cfg, &outcomes);

    assert_eq!(candidates.len(), 1);
    let candidate = candidates
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one prune candidate"));
    assert_eq!(candidate.id, "allow-stale");
}

#[test]
fn config_without_prune_candidates_removes_only_stale_entries() {
    let mut cfg = AllowConfig::empty();
    cfg.allow
        .push(test_entry("allow-stale", FindingKind::Panic));
    cfg.allow.push(test_entry("allow-live", FindingKind::Panic));
    let candidates = vec![PruneCandidate {
        id: "allow-stale".to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        owner: "parser".to_string(),
        classification: "reviewed_exception".to_string(),
        scope: "src/lib.rs".to_string(),
        reason: "The old exception is gone.".to_string(),
    }];

    let pruned = config_without_prune_candidates(&cfg, &candidates);

    assert_eq!(pruned.allow.len(), 1);
    assert!(pruned.allow.iter().any(|entry| entry.id == "allow-live"));
    assert!(!pruned.allow.iter().any(|entry| entry.id == "allow-stale"));
}

#[test]
fn removed_toml_blocks_extracts_only_stale_allow_entries() {
    let mut cfg = AllowConfig::empty();
    cfg.allow
        .push(non_rust_prune_fixture_entry("allow-live", "docs/live.md"));
    cfg.allow
        .push(non_rust_prune_fixture_entry("allow-stale", "docs/stale.md"));
    let rendered = render_policy(&cfg);
    let candidates = vec![PruneCandidate {
        id: "allow-stale".to_string(),
        kind: FindingKind::NonRustFile,
        family: Some("documentation".to_string()),
        owner: "owner".to_string(),
        classification: "classification".to_string(),
        scope: "docs/stale.md".to_string(),
        reason: "reason".to_string(),
    }];

    let blocks = stale_removed_toml_blocks(&rendered, &candidates);

    assert_eq!(blocks.len(), 1);
    let block = blocks
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected stale TOML block"));
    assert!(block.contains("[[allow]]"));
    assert!(block.contains("id = \"allow-stale\""));
    assert!(block.contains("path = \"docs/stale.md\""));
    assert!(!block.contains("allow-live"));
}

#[test]
fn removed_toml_blocks_ignore_allow_headers_inside_string_values() {
    let mut cfg = AllowConfig::empty();
    cfg.allow
        .push(non_rust_prune_fixture_entry("allow-live", "docs/live.md"));
    let mut stale = non_rust_prune_fixture_entry("allow-stale", "docs/stale.md");
    stale.reason = "Stale reason mentions [[allow]] as TOML text.".to_string();
    cfg.allow.push(stale);
    let rendered = render_policy(&cfg);
    let candidates = vec![PruneCandidate {
        id: "allow-stale".to_string(),
        kind: FindingKind::NonRustFile,
        family: Some("documentation".to_string()),
        owner: "owner".to_string(),
        classification: "classification".to_string(),
        scope: "docs/stale.md".to_string(),
        reason: "Stale reason mentions [[allow]] as TOML text.".to_string(),
    }];

    let blocks = stale_removed_toml_blocks(&rendered, &candidates);

    assert_eq!(blocks.len(), 1);
    let block = blocks
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected stale TOML block"));
    assert!(block.contains("id = \"allow-stale\""));
    assert!(block.contains("reason = \"Stale reason mentions [[allow]] as TOML text.\""));
    assert!(block.contains("[allow.selector]"));
    assert!(block.contains("path = \"docs/stale.md\""));
    assert!(!block.contains("allow-live"));
}

#[test]
fn cmd_prune_write_reports_missing_policy_config_with_exact_error() {
    let root = prune_fixture_dir();

    let err = cmd_prune(&PruneArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: None,
        stale: true,
        dry_run: false,
        write: true,
        include_untracked: false,
        format: HumanJsonFormat::Human,
        output: None,
    })
    .expect_err("prune write without policy config should fail");

    assert_eq!(err.kind(), allow_core::CargoAllowErrorKind::Unknown);

    fs::remove_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn cmd_prune_write_removes_only_stale_entries_from_policy_file() {
    let root = prune_fixture_dir();
    let policy_dir = root.join("policy");
    let docs_dir = root.join("docs");
    fs::create_dir_all(&policy_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
    fs::create_dir_all(&docs_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("docs dir: {err}")));
    fs::write(docs_dir.join("live.md"), "# live\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("live doc: {err}")));

    let mut cfg = AllowConfig::empty();
    cfg.allow
        .push(non_rust_prune_fixture_entry("allow-live", "docs/live.md"));
    cfg.allow
        .push(non_rust_prune_fixture_entry("allow-stale", "docs/stale.md"));
    let policy_path = policy_dir.join("allow.toml");
    fs::write(&policy_path, render_policy(&cfg))
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy write: {err}")));

    cmd_prune(&PruneArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: Some(policy_path.clone()),
        stale: true,
        dry_run: false,
        write: true,
        include_untracked: false,
        format: HumanJsonFormat::Json,
        output: Some(root.join("prune.json")),
    })
    .unwrap_or_else(|err| std::panic::panic_any(format!("prune write: {err}")));

    let rendered = fs::read_to_string(&policy_path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy read: {err}")));
    let loaded = load_policy(&policy_path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy reload: {err}")));

    assert!(rendered.contains("allow-live"));
    assert!(!rendered.contains("allow-stale"));
    assert_eq!(loaded.allow.len(), 1);
    assert!(loaded.allow.iter().any(|entry| entry.id == "allow-live"));
    let artifact_path = root.join("prune.json");
    let artifact = fs::read_to_string(&artifact_path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("prune artifact read: {err}")));
    let parsed = serde_json::from_str::<serde_json::Value>(&artifact);
    assert!(
        parsed.is_ok(),
        "prune write artifact must remain valid JSON"
    );
    let Some(parsed) = parsed.ok() else {
        return;
    };
    assert_eq!(
        parsed
            .pointer("/mode/write_requested")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        parsed
            .pointer("/mutation_receipt/config_source")
            .and_then(serde_json::Value::as_str),
        Some("policy/allow.toml")
    );
    assert_eq!(
        parsed
            .pointer("/mutation_receipt/result")
            .and_then(serde_json::Value::as_str),
        Some("written")
    );
    assert_eq!(
        parsed
            .pointer("/mutation_receipt/next_commands/0")
            .and_then(serde_json::Value::as_str),
        Some("git diff -- policy/allow.toml")
    );
    assert_eq!(
        parsed
            .pointer("/mutation_receipt/changed_allow_ids/0")
            .and_then(serde_json::Value::as_str),
        Some("allow-stale")
    );
    assert_eq!(
        parsed.pointer("/mutation_receipt/after_fingerprints/0"),
        Some(&serde_json::Value::Null)
    );

    fs::remove_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn cmd_prune_write_can_remove_stale_broken_evidence_entry() {
    let root = prune_fixture_dir();
    let policy_dir = root.join("policy");
    let docs_dir = root.join("docs");
    fs::create_dir_all(&policy_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
    fs::create_dir_all(&docs_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("docs dir: {err}")));
    fs::write(docs_dir.join("live.md"), "# live\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("live doc: {err}")));

    let mut cfg = AllowConfig::empty();
    cfg.allow
        .push(non_rust_prune_fixture_entry("allow-live", "docs/live.md"));
    let mut stale = non_rust_prune_fixture_entry("allow-stale", "docs/stale.md");
    stale.evidence = vec!["doc:docs/missing-stale-evidence.md".to_string()];
    cfg.allow.push(stale);
    let policy_path = policy_dir.join("allow.toml");
    fs::write(&policy_path, render_policy(&cfg))
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy write: {err}")));

    cmd_prune(&PruneArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: Some(policy_path.clone()),
        stale: true,
        dry_run: false,
        write: true,
        include_untracked: false,
        format: HumanJsonFormat::Human,
        output: None,
    })
    .unwrap_or_else(|err| std::panic::panic_any(format!("prune stale broken evidence: {err}")));

    let rendered = fs::read_to_string(&policy_path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy read: {err}")));
    assert!(rendered.contains("allow-live"));
    assert!(!rendered.contains("allow-stale"));
    assert!(!rendered.contains("missing-stale-evidence"));

    fs::remove_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn cmd_prune_write_rejects_broken_evidence_that_would_remain() {
    let root = prune_fixture_dir();
    let policy_dir = root.join("policy");
    let docs_dir = root.join("docs");
    fs::create_dir_all(&policy_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
    fs::create_dir_all(&docs_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("docs dir: {err}")));
    fs::write(docs_dir.join("live.md"), "# live\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("live doc: {err}")));

    let mut cfg = AllowConfig::empty();
    let mut live = non_rust_prune_fixture_entry("allow-live", "docs/live.md");
    live.evidence = vec!["doc:docs/missing-live-evidence.md".to_string()];
    cfg.allow.push(live);
    cfg.allow
        .push(non_rust_prune_fixture_entry("allow-stale", "docs/stale.md"));
    let policy_path = policy_dir.join("allow.toml");
    fs::write(&policy_path, render_policy(&cfg))
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy write: {err}")));

    let err = cmd_prune(&PruneArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: Some(policy_path.clone()),
        stale: true,
        dry_run: false,
        write: true,
        include_untracked: false,
        format: HumanJsonFormat::Human,
        output: None,
    })
    .expect_err("prune write should reject broken evidence that remains");

    assert!(
        err.to_string().contains("allow-live evidence"),
        "diagnostic should identify the remaining allow entry: {err}"
    );
    assert!(
        err.to_string().contains("docs/missing-live-evidence.md"),
        "diagnostic should identify the missing evidence path: {err}"
    );
    let rendered = fs::read_to_string(&policy_path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy read: {err}")));
    assert!(
        rendered.contains("allow-stale"),
        "policy should not be rewritten when remaining evidence is broken"
    );

    fs::remove_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn cmd_prune_write_rejects_broken_link_that_would_remain() {
    let root = prune_fixture_dir();
    let policy_dir = root.join("policy");
    let docs_dir = root.join("docs");
    fs::create_dir_all(&policy_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
    fs::create_dir_all(&docs_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("docs dir: {err}")));
    fs::write(docs_dir.join("live.md"), "# live\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("live doc: {err}")));

    let mut cfg = AllowConfig::empty();
    let mut live = non_rust_prune_fixture_entry("allow-live", "docs/live.md");
    live.links = vec!["doc:docs/missing-live-rationale.md".to_string()];
    cfg.allow.push(live);
    cfg.allow
        .push(non_rust_prune_fixture_entry("allow-stale", "docs/stale.md"));
    let policy_path = policy_dir.join("allow.toml");
    fs::write(&policy_path, render_policy(&cfg))
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy write: {err}")));

    let err = cmd_prune(&PruneArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: Some(policy_path.clone()),
        stale: true,
        dry_run: false,
        write: true,
        include_untracked: false,
        format: HumanJsonFormat::Human,
        output: None,
    })
    .expect_err("prune write should reject broken links that remain");

    assert!(
        err.to_string().contains("allow-live link"),
        "diagnostic should identify the remaining allow entry link: {err}"
    );
    assert!(
        err.to_string().contains("docs/missing-live-rationale.md"),
        "diagnostic should identify the missing link path: {err}"
    );
    let rendered = fs::read_to_string(&policy_path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy read: {err}")));
    assert!(
        rendered.contains("allow-stale"),
        "policy should not be rewritten when remaining links are broken"
    );

    fs::remove_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn cmd_prune_write_rejects_untracked_evidence_that_would_remain_by_default() {
    let root = prune_fixture_dir();
    let policy_dir = root.join("policy");
    let docs_dir = root.join("docs");
    fs::create_dir_all(&policy_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
    fs::create_dir_all(&docs_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("docs dir: {err}")));
    fs::write(docs_dir.join("live.md"), "# live\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("live doc: {err}")));

    let mut cfg = AllowConfig::empty();
    let mut live = non_rust_prune_fixture_entry("allow-live", "docs/live.md");
    live.evidence = vec!["doc:policy/evidence.md".to_string()];
    cfg.allow.push(live);
    cfg.allow
        .push(non_rust_prune_fixture_entry("allow-stale", "docs/stale.md"));
    cfg.workspace.ignored = vec!["policy/evidence.md".to_string()];
    let policy_path = policy_dir.join("allow.toml");
    fs::write(&policy_path, render_policy(&cfg))
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy write: {err}")));
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "policy/allow.toml", "docs/live.md"]);
    git(&root, &["commit", "-m", "base policy"]);
    fs::write(policy_dir.join("evidence.md"), "untracked evidence")
        .unwrap_or_else(|err| std::panic::panic_any(format!("evidence doc: {err}")));

    let err = cmd_prune(&PruneArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: Some(policy_path.clone()),
        stale: true,
        dry_run: false,
        write: true,
        include_untracked: false,
        format: HumanJsonFormat::Human,
        output: None,
    })
    .expect_err("prune write should reject untracked evidence that remains by default");

    assert!(
        err.to_string()
            .contains("not in the default source-tree inventory"),
        "diagnostic should explain source-tree evidence boundary: {err}"
    );
    let rendered = fs::read_to_string(&policy_path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy read: {err}")));
    assert!(
        rendered.contains("allow-stale"),
        "policy should not be rewritten when remaining evidence is untracked"
    );

    fs::remove_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn cmd_prune_write_include_untracked_accepts_untracked_evidence_that_would_remain() {
    let root = prune_fixture_dir();
    let policy_dir = root.join("policy");
    let docs_dir = root.join("docs");
    fs::create_dir_all(&policy_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
    fs::create_dir_all(&docs_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("docs dir: {err}")));
    fs::write(docs_dir.join("live.md"), "# live\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("live doc: {err}")));

    let mut cfg = AllowConfig::empty();
    let mut live = non_rust_prune_fixture_entry("allow-live", "docs/live.md");
    live.evidence = vec!["doc:policy/evidence.md".to_string()];
    cfg.allow.push(live);
    cfg.allow
        .push(non_rust_prune_fixture_entry("allow-stale", "docs/stale.md"));
    cfg.workspace.ignored = vec!["policy/evidence.md".to_string()];
    let policy_path = policy_dir.join("allow.toml");
    fs::write(&policy_path, render_policy(&cfg))
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy write: {err}")));
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "policy/allow.toml", "docs/live.md"]);
    git(&root, &["commit", "-m", "base policy"]);
    fs::write(policy_dir.join("evidence.md"), "untracked evidence")
        .unwrap_or_else(|err| std::panic::panic_any(format!("evidence doc: {err}")));

    cmd_prune(&PruneArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: Some(policy_path.clone()),
        stale: true,
        dry_run: false,
        write: true,
        include_untracked: true,
        format: HumanJsonFormat::Human,
        output: None,
    })
    .unwrap_or_else(|err| {
        std::panic::panic_any(format!(
            "prune write should accept include-untracked evidence: {err}"
        ))
    });

    let rendered = fs::read_to_string(&policy_path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy read: {err}")));
    assert!(rendered.contains("allow-live"));
    assert!(!rendered.contains("allow-stale"));

    fs::remove_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

static NEXT_PRUNE_FIXTURE: AtomicUsize = AtomicUsize::new(0);

fn prune_fixture_dir() -> PathBuf {
    let id = NEXT_PRUNE_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "cargo-allow-cli-prune-{}-{stamp}-{id}",
        std::process::id()
    ));
    fs::create_dir_all(&dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture dir: {err}")));
    dir
}

fn test_entry(id: &str, kind: FindingKind) -> AllowEntry {
    AllowEntry {
        id: id.to_string(),
        kind,
        family: None,
        path: Some(PathBuf::from("tracked.file")),
        glob: None,
        owner: "owner".to_string(),
        classification: "classification".to_string(),
        reason: "reason".to_string(),
        evidence: Vec::new(),
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle::empty(),
        selector: Selector {
            ast_kind: Some("tracked_file".to_string()),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn non_rust_prune_fixture_entry(id: &str, path: &str) -> AllowEntry {
    let mut entry = test_entry(id, FindingKind::NonRustFile);
    entry.family = Some("documentation".to_string());
    entry.path = Some(PathBuf::from(path));
    entry.lifecycle.review_after = Some("2026-11-01".to_string());
    entry
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

fn test_outcome(
    status: MatchStatus,
    allow_id: Option<&str>,
    finding_index: Option<usize>,
    message: &str,
) -> MatchOutcome {
    MatchOutcome {
        status,
        allow_id: allow_id.map(str::to_string),
        candidate_ids: Vec::new(),
        finding_index,
        message: message.to_string(),
        score: 100,
    }
}
