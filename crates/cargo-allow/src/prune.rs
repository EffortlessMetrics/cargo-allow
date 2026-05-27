use allow_core::{
    AllowConfig, CargoAllowError, CargoAllowResult, FindingKind, MatchOutcome, MatchStatus,
    json_escape,
};
use allow_match::{CheckMode, evaluate};
use allow_policy::{render_policy, validate_policy};
use clap::{Parser, ValueEnum};
use std::path::{Path, PathBuf};

use crate::{
    RootArgs, config_path, json_string_array, load_world, markdown_cell, option_json_string,
    source_tree_root_text, write_file,
};

#[derive(Debug, Clone, Parser)]
pub(crate) struct PruneArgs {
    #[command(flatten)]
    root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    config: Option<PathBuf>,
    /// Preview stale allow entries.
    #[arg(long)]
    stale: bool,
    /// Explicitly run without writing policy changes.
    #[arg(long, conflicts_with = "write")]
    dry_run: bool,
    /// Remove stale entries from the policy file.
    #[arg(long, conflicts_with = "dry_run")]
    write: bool,
    /// Include untracked files when determining stale entries.
    #[arg(long)]
    include_untracked: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = PruneFormat::Human)]
    format: PruneFormat,
    /// Write prune preview/result to a file instead of stdout.
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum PruneFormat {
    Human,
    Json,
}

pub(crate) fn cmd_prune(args: &PruneArgs) -> CargoAllowResult<()> {
    if !args.stale {
        return Err(CargoAllowError::new(
            "prune currently supports only --stale",
        ));
    }
    if args.dry_run && args.write {
        return Err(CargoAllowError::new(
            "pass either --dry-run or --write, not both",
        ));
    }
    let (root, cfg, findings, inventory_facts) = load_world(
        args.root.root.as_deref(),
        args.config.as_deref(),
        true,
        None,
        args.include_untracked,
    )?;
    let outcomes = evaluate(&cfg, &findings, CheckMode::NoNew);
    let candidates = prune_stale_candidates(&cfg, &outcomes);
    let written_path = if args.write && !candidates.is_empty() {
        let path = config_path(&root, args.config.as_deref()).ok_or_else(|| {
            CargoAllowError::new("no policy config found; run `cargo-allow init` or pass --config")
        })?;
        let pruned = config_without_prune_candidates(&cfg, &candidates);
        validate_policy(&pruned)?;
        write_file(&path, &render_policy(&pruned))?;
        Some(path)
    } else {
        None
    };
    let root_text = source_tree_root_text(&root);
    let context = PruneContext {
        inventory_source: inventory_facts.source.as_str(),
        source_tree_root: Some(&root_text),
        inventory_files: inventory_facts.files_scanned,
    };
    let text = match args.format {
        PruneFormat::Human => render_prune_stale_result(
            &candidates,
            args.dry_run,
            args.write,
            written_path.as_deref(),
        ),
        PruneFormat::Json => render_prune_stale_json(
            &candidates,
            args.dry_run,
            args.write,
            written_path.as_deref(),
            context,
        ),
    };
    if let Some(path) = &args.output {
        write_file(path, &text)?;
    } else {
        println!("{text}");
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct PruneContext<'a> {
    inventory_source: &'a str,
    source_tree_root: Option<&'a str>,
    inventory_files: Option<usize>,
}

impl Default for PruneContext<'static> {
    fn default() -> Self {
        Self {
            inventory_source: "unknown",
            source_tree_root: None,
            inventory_files: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PruneCandidate {
    id: String,
    kind: FindingKind,
    family: Option<String>,
    owner: String,
    classification: String,
    scope: String,
    reason: String,
}

fn prune_stale_candidates(cfg: &AllowConfig, outcomes: &[MatchOutcome]) -> Vec<PruneCandidate> {
    outcomes
        .iter()
        .filter(|outcome| outcome.status == MatchStatus::Stale)
        .filter_map(|outcome| {
            let id = outcome.allow_id.as_deref()?;
            let entry = cfg.allow.iter().find(|entry| entry.id == id)?;
            Some(PruneCandidate {
                id: entry.id.clone(),
                kind: entry.kind,
                family: entry.family.clone(),
                owner: entry.owner.clone(),
                classification: entry.classification.clone(),
                scope: entry.path_or_glob(),
                reason: entry.reason.clone(),
            })
        })
        .collect()
}

fn config_without_prune_candidates(
    cfg: &AllowConfig,
    candidates: &[PruneCandidate],
) -> AllowConfig {
    let mut pruned = cfg.clone();
    pruned
        .allow
        .retain(|entry| !candidates.iter().any(|candidate| candidate.id == entry.id));
    pruned
}

fn render_prune_stale_result(
    candidates: &[PruneCandidate],
    explicit_dry_run: bool,
    write_requested: bool,
    written_path: Option<&Path>,
) -> String {
    let mut out = String::new();
    out.push_str("cargo-allow prune\n\n");
    if write_requested {
        out.push_str("mode: write\n");
    } else {
        out.push_str("mode: dry-run\n");
    }
    if explicit_dry_run {
        out.push_str("requested: --dry-run\n");
    }
    out.push_str(&format!("stale entries: {}\n\n", candidates.len()));
    if candidates.is_empty() {
        out.push_str("No stale allow entries found.\n");
        return out;
    }
    out.push_str("| Allow ID | Kind | Family | Owner | Classification | Scope | Reason |\n");
    out.push_str("|---|---|---|---|---|---|---|\n");
    for candidate in candidates {
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | {} |\n",
            markdown_cell(&candidate.id),
            candidate.kind,
            markdown_cell(candidate.family.as_deref().unwrap_or("-")),
            markdown_cell(&candidate.owner),
            markdown_cell(&candidate.classification),
            markdown_cell(&candidate.scope),
            markdown_cell(&candidate.reason)
        ));
    }
    if let Some(path) = written_path {
        out.push_str(&format!(
            "\nRemoved stale entries from `{}`.\n",
            markdown_cell(&path.display().to_string())
        ));
    } else {
        out.push_str(
            "\nNo files were changed. Remove these entries only after confirming the exception is gone.\n",
        );
    }
    out
}

fn render_prune_stale_json(
    candidates: &[PruneCandidate],
    explicit_dry_run: bool,
    write_requested: bool,
    written_path: Option<&Path>,
    context: PruneContext<'_>,
) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "  \"schema_version\": {},\n",
        allow_report::PRUNE_SCHEMA_VERSION
    ));
    out.push_str(&format!(
        "  \"schema_id\": \"{}\",\n",
        allow_report::PRUNE_SCHEMA_ID
    ));
    out.push_str("  \"tool\": \"cargo-allow\",\n");
    out.push_str("  \"command\": \"prune\",\n");
    out.push_str(&format!(
        "  \"claim_boundary\": {},\n",
        json_string_array(allow_report::CLAIM_BOUNDARY)
    ));
    out.push_str(&format!(
        "  \"scanner_limitations\": {},\n",
        json_string_array(allow_report::SCANNER_LIMITATIONS)
    ));
    out.push_str("  \"inventory\": ");
    out.push_str(&prune_inventory_json(context, "  "));
    out.push_str(",\n");
    out.push_str("  \"mode\": {\n");
    out.push_str(&format!("    \"dry_run\": {},\n", !write_requested));
    out.push_str(&format!("    \"write_requested\": {},\n", write_requested));
    out.push_str(&format!(
        "    \"explicit_dry_run\": {},\n",
        explicit_dry_run
    ));
    let written = written_path.map(|path| path.display().to_string());
    out.push_str(&format!(
        "    \"written_path\": {}\n",
        option_json_string(written.as_deref())
    ));
    out.push_str("  },\n");
    out.push_str(&format!(
        "  \"summary\": {{\n    \"stale_entries\": {}\n  }},\n",
        candidates.len()
    ));
    out.push_str("  \"stale_entries\": [\n");
    for (index, candidate) in candidates.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str(&prune_candidate_json(candidate, "  "));
    }
    out.push_str("\n  ]\n");
    out.push_str("}\n");
    out
}

fn prune_inventory_json(context: PruneContext<'_>, indent: &str) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("{indent}  \"scope\": \"source_tree\",\n"));
    out.push_str(&format!("{indent}  \"scanner\": \"source_syntax\",\n"));
    out.push_str(&format!(
        "{indent}  \"source\": \"{}\"",
        json_escape(context.inventory_source)
    ));
    if let Some(root) = context.source_tree_root {
        out.push_str(&format!(",\n{indent}  \"root\": \"{}\"", json_escape(root)));
    }
    if let Some(files) = context.inventory_files {
        out.push_str(&format!(",\n{indent}  \"files_scanned\": {files}"));
    }
    out.push_str(&format!("\n{indent}}}"));
    out
}

fn prune_candidate_json(candidate: &PruneCandidate, indent: &str) -> String {
    format!(
        "{indent}  {{\n{indent}    \"id\": \"{}\",\n{indent}    \"kind\": \"{}\",\n{indent}    \"family\": {},\n{indent}    \"owner\": \"{}\",\n{indent}    \"classification\": \"{}\",\n{indent}    \"scope\": \"{}\",\n{indent}    \"reason\": \"{}\"\n{indent}  }}",
        json_escape(&candidate.id),
        candidate.kind,
        option_json_string(candidate.family.as_deref()),
        json_escape(&candidate.owner),
        json_escape(&candidate.classification),
        json_escape(&candidate.scope),
        json_escape(&candidate.reason)
    )
}

#[cfg(test)]
pub(crate) fn sample_prune_json_for_contract_test() -> String {
    let candidates = Vec::new();
    render_prune_stale_json(
        &candidates,
        true,
        false,
        None,
        PruneContext {
            inventory_source: "git_tracked",
            source_tree_root: Some("H:/Code/Rust/cargo-allow"),
            inventory_files: Some(49),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CargoAllowCli, CargoAllowCommand};
    use allow_core::{AllowEntry, Lifecycle, Selector};
    use allow_policy::load_policy;
    use clap::Parser;
    use std::fs;
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
}
