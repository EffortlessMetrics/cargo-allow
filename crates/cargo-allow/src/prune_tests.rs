use super::*;
use crate::{CargoAllowCli, CargoAllowCommand};
use allow_core::{AllowEntry, Lifecycle, Selector};
use allow_policy::load_policy;
use clap::Parser;
use std::fs;
use std::path::Path;
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
            format: PruneFormat::Json,
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
fn render_prune_stale_preview_is_dry_run_first() {
    let candidates = vec![PruneCandidate {
        id: "allow-stale".to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        owner: "parser".to_string(),
        classification: "reviewed_exception".to_string(),
        scope: "src/lib.rs".to_string(),
        reason: "The old exception is gone.".to_string(),
    }];

    let text = render_prune_stale_result(&candidates, true, false, None);

    assert!(text.contains("mode: dry-run"));
    assert!(text.contains("requested: --dry-run"));
    assert!(text.contains("stale entries: 1"));
    assert!(text.contains("allow-stale"));
    assert!(text.contains("No files were changed"));
}

#[test]
fn render_prune_stale_json_records_context_and_candidates() {
    let candidates = vec![PruneCandidate {
        id: "allow-stale".to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        owner: "parser".to_string(),
        classification: "baseline_debt".to_string(),
        scope: "crates/parser/src/lib.rs".to_string(),
        reason: "old baseline entry".to_string(),
    }];

    let json = render_prune_stale_json(
        &candidates,
        true,
        false,
        None,
        PruneContext {
            inventory_source: "git_tracked",
            source_tree_root: Some("H:/Code/Rust/cargo-allow"),
            inventory_files: Some(49),
        },
    );

    assert!(json.contains("\"schema_version\": 1"));
    assert!(json.contains(&format!(
        "\"schema_id\": \"{}\"",
        allow_report::PRUNE_SCHEMA_ID
    )));
    assert!(json.contains("\"command\": \"prune\""));
    assert!(json.contains("\"claim_boundary\""));
    assert!(json.contains("\"scanner_limitations\""));
    assert!(json.contains("\"source\": \"git_tracked\""));
    assert!(json.contains("\"root\": \"H:/Code/Rust/cargo-allow\""));
    assert!(json.contains("\"files_scanned\": 49"));
    assert!(json.contains("\"dry_run\": true"));
    assert!(json.contains("\"write_requested\": false"));
    assert!(json.contains("\"explicit_dry_run\": true"));
    assert!(json.contains("\"written_path\": null"));
    assert!(json.contains("\"stale_entries\": 1"));
    assert!(json.contains("\"id\": \"allow-stale\""));
    assert!(json.contains("\"kind\": \"panic\""));
    assert!(json.contains("\"family\": \"unwrap\""));
}

#[test]
fn render_prune_stale_result_reports_written_policy() {
    let candidates = vec![PruneCandidate {
        id: "allow-stale".to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        owner: "parser".to_string(),
        classification: "reviewed_exception".to_string(),
        scope: "src/lib.rs".to_string(),
        reason: "The old exception is gone.".to_string(),
    }];

    let text = render_prune_stale_result(
        &candidates,
        false,
        true,
        Some(Path::new("policy/allow.toml")),
    );

    assert!(text.contains("mode: write"));
    assert!(text.contains("Removed stale entries from `policy/allow.toml`"));
    assert!(!text.contains("No files were changed"));
}

#[test]
fn render_prune_stale_result_reports_write_mode_with_no_candidates() {
    let text = render_prune_stale_result(&[], false, true, None);

    assert!(text.contains("mode: write"));
    assert!(text.contains("No stale allow entries found."));
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
        format: PruneFormat::Human,
        output: None,
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

fn test_outcome(
    status: MatchStatus,
    allow_id: Option<&str>,
    finding_index: Option<usize>,
    message: &str,
) -> MatchOutcome {
    MatchOutcome {
        status,
        allow_id: allow_id.map(str::to_string),
        finding_index,
        message: message.to_string(),
        score: 100,
    }
}
