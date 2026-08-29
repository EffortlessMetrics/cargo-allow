use super::*;
use crate::{CargoAllowCli, CargoAllowCommand, OutputFormat, RootArgs};
use allow_core::{AllowConfig, AllowEntry, FindingKind, Lifecycle, Selector};
use allow_policy::render_policy;
use clap::Parser;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn clap_parses_include_untracked_audit_flag() {
    let parsed =
        CargoAllowCli::try_parse_from(argv(vec!["cargo-allow", "audit", "--include-untracked"]))
            .unwrap_or_else(|err| {
                std::panic::panic_any(format!("CLI should parse include-untracked: {err}"))
            });

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::Audit(ReportArgs {
            artifact_dir: None,
            emit: None,
            include_untracked: true,
            ..
        }))
    ));
}

#[test]
fn audit_json_reports_broken_evidence_links_without_aborting() {
    let root = audit_fixture_dir();
    let policy_dir = root.join("policy");
    fs::create_dir_all(&policy_dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));

    let mut cfg = AllowConfig::empty();
    cfg.allow.push(AllowEntry {
        id: "allow-doc".to_string(),
        kind: FindingKind::NonRustFile,
        family: Some("documentation".to_string()),
        path: Some(PathBuf::from("docs/missing.md")),
        glob: None,
        owner: "core/docs".to_string(),
        classification: "reviewed_exception".to_string(),
        reason: "Fixture keeps a broken local evidence link for audit reporting.".to_string(),
        evidence: vec![
            "doc:docs/missing-evidence.md".to_string(),
            "custom-ticket:source-review".to_string(),
        ],
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: Some("2026-05-29".to_string()),
            review_after: Some("2026-08-01".to_string()),
            expires: None,
        },
        selector: Selector {
            ast_kind: Some("tracked_file".to_string()),
            ..Selector::default()
        },
        last_seen: None,
    });
    let policy_path = policy_dir.join("allow.toml");
    let output_path = root.join("audit.json");
    fs::write(&policy_path, render_policy(&cfg))
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy write: {err}")));

    cmd_audit(&ReportArgs {
        artifact_dir: None,
        emit: None,
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: Some(policy_path),
        profile: None,
        compat: false,
        kind: None,
        include_untracked: false,
        format: OutputFormat::Json,
        output: Some(output_path.clone()),
    })
    .unwrap_or_else(|err| std::panic::panic_any(format!("audit should not abort: {err}")));

    let json = fs::read_to_string(&output_path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("audit output read: {err}")));
    assert!(json.contains("\"broken_evidence_links\": 1"));
    assert!(json.contains("\"weak_evidence_references\": 1"));

    fs::remove_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

fn argv(items: Vec<&str>) -> Vec<String> {
    items.into_iter().map(String::from).collect()
}

static NEXT_AUDIT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

fn audit_fixture_dir() -> PathBuf {
    let id = NEXT_AUDIT_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "cargo-allow-cli-audit-{}-{stamp}-{id}",
        std::process::id()
    ));
    fs::create_dir_all(&dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture dir: {err}")));
    dir
}
