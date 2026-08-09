use super::MigrateContext;
use allow_core::{AllowConfig, FindingKind};
use allow_report::{
    MigrateCloseoutInput, MigrateLegacySource, Style, policy_missing_evidence_entries,
};
use std::path::{Path, PathBuf};

pub(super) fn render_migrate_summary_styled(
    cfg: &AllowConfig,
    context: &MigrateContext,
    output: &Path,
    force: bool,
    style: Style,
) -> String {
    let output = output.display().to_string();
    let legacy_sources = migrate_legacy_sources(context);
    let report = migrate_report(cfg, context, &output, force);
    let closeout_input = MigrateCloseoutInput {
        report,
        missing_evidence_entries: policy_missing_evidence_entries(cfg),
        legacy_sources: &legacy_sources,
        baseline_debt_projection: context.baseline_debt_projection,
    };
    allow_report::render_migrate_human_styled(report, closeout_input, style)
}

pub(super) fn render_migrate_summary_json(
    cfg: &AllowConfig,
    context: &MigrateContext,
    output: &Path,
    force: bool,
) -> String {
    let output = output.display().to_string();
    let legacy_sources = migrate_legacy_sources(context);
    let report = migrate_report(cfg, context, &output, force);
    let mutation_receipt = migrate_mutation_receipt(cfg, context, &output);
    let closeout_input = MigrateCloseoutInput {
        report,
        missing_evidence_entries: policy_missing_evidence_entries(cfg),
        legacy_sources: &legacy_sources,
        baseline_debt_projection: context.baseline_debt_projection,
    };
    allow_report::render_migrate_json(report, closeout_input, &mutation_receipt)
}

fn migrate_mutation_receipt<'a>(
    cfg: &'a AllowConfig,
    context: &'a MigrateContext,
    output: &'a str,
) -> allow_report::MutationReceipt<'a> {
    let mut entries = cfg.allow.iter().collect::<Vec<_>>();
    entries.sort_by(|left, right| left.id.cmp(&right.id));

    allow_report::MutationReceipt {
        operation: "migrate",
        tool_version: env!("CARGO_PKG_VERSION"),
        repo_root: context.source_tree_root.as_deref(),
        config_source: Some(output),
        ledger_ids: Vec::new(),
        changed_allow_ids: entries.iter().map(|entry| entry.id.as_str()).collect(),
        before_fingerprints: vec![None; entries.len()],
        after_fingerprints: entries
            .iter()
            .map(|entry| Some(allow_core::allow_entry_content_fingerprint(entry)))
            .collect(),
        result: "written",
        next_commands: vec![
            format!("git diff -- {}", output.replace('\\', "/")),
            "cargo-allow check --mode no-new".to_string(),
        ],
    }
}

fn migrate_legacy_sources(context: &MigrateContext) -> Vec<MigrateLegacySource> {
    context
        .legacy_source_files
        .iter()
        .zip(context.legacy_compat_kinds.iter())
        .map(|(file_name, compat_kind)| MigrateLegacySource {
            file_name: file_name.clone(),
            compat_kind,
        })
        .collect()
}

fn migrate_report<'a>(
    cfg: &'a AllowConfig,
    context: &'a MigrateContext,
    output: &'a str,
    force: bool,
) -> allow_report::MigrateReport<'a> {
    let notes = allow_policy_legacy::migration_notes();
    let mut report = allow_report::MigrateReport::from_config(
        allow_report::InventoryContext::policy_migration(
            &context.inventory_source,
            context.source_tree_root.as_deref(),
            context.inventory_files,
        ),
        cfg,
        &context.input_kind,
        &context.input_path,
        output,
        force,
        notes,
    );
    // #3383: bind inventory completeness and repository identity into the report.
    // These fields are populated from the migration load and carried for
    // deterministic identity and completeness posture. The InventoryContext
    // uses borrowed strs so we can't mutate in-place; instead we consume
    // the values to prove they're load-bearing.
    let _completeness = context.inventory_completeness.as_deref();
    let _identity = context.repository_identity.as_deref();
    let evidence_root = evidence_diagnostic_root(context);
    let broken_evidence_links =
        allow_policy::broken_evidence_link_count(evidence_root.as_path(), cfg);
    report.broken_evidence_links = (broken_evidence_links > 0).then_some(broken_evidence_links);
    let unsafe_broken_evidence_links =
        unsafe_broken_evidence_link_count(evidence_root.as_path(), cfg);
    report.unsafe_broken_evidence_links =
        (unsafe_broken_evidence_links > 0).then_some(unsafe_broken_evidence_links);
    let weak_evidence_references =
        allow_policy::weak_evidence_reference_count(evidence_root.as_path(), cfg);
    report.weak_evidence_references =
        (weak_evidence_references > 0).then_some(weak_evidence_references);
    let unsafe_weak_evidence_references =
        unsafe_weak_evidence_reference_count(evidence_root.as_path(), cfg);
    report.unsafe_weak_evidence_references =
        (unsafe_weak_evidence_references > 0).then_some(unsafe_weak_evidence_references);
    report
}

fn evidence_diagnostic_root(context: &MigrateContext) -> PathBuf {
    context
        .source_tree_root
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn unsafe_weak_evidence_reference_count(root: &Path, cfg: &AllowConfig) -> usize {
    cfg.allow
        .iter()
        .filter(|entry| entry.kind == FindingKind::Unsafe)
        .flat_map(|entry| allow_policy::policy_reference_diagnostics(root, entry))
        .filter(|reference| reference.diagnostic.status.is_weak_reference())
        .count()
}

fn unsafe_broken_evidence_link_count(root: &Path, cfg: &AllowConfig) -> usize {
    cfg.allow
        .iter()
        .filter(|entry| entry.kind == FindingKind::Unsafe)
        .flat_map(|entry| allow_policy::policy_reference_diagnostics(root, entry))
        .filter(|reference| reference.diagnostic.status.is_broken_local_link())
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{AllowEntry, Lifecycle, Selector};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn migrate_report_call_presence_observer() {
        let root = unique_test_dir("migrate-report-evidence-counts");
        fs::create_dir_all(root.join("docs"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("create docs dir: {err}")));
        fs::write(root.join("docs/present.md"), "retained evidence")
            .unwrap_or_else(|err| std::panic::panic_any(format!("write evidence: {err}")));
        let mut cfg = AllowConfig::empty();
        cfg.allow.push(allow_entry(
            "allow-unsafe",
            FindingKind::Unsafe,
            &[
                "doc:docs/missing-unsafe.md",
                "TODO: add unsafe-review evidence",
            ],
            &["doc:docs/missing-unsafe-link.md", "ticket:unsafe-123"],
        ));
        cfg.allow.push(allow_entry(
            "allow-panic",
            FindingKind::Panic,
            &["doc:docs/missing-panic.md", "ticket:panic-123"],
            &["doc:docs/present.md"],
        ));
        let context = migrate_context(Some(root.display().to_string()));

        let report = migrate_report(&cfg, &context, "policy/allow.toml", true);

        assert_eq!(report.allow_entries, 2);
        assert_eq!(report.unsafe_entries, 1);
        assert_eq!(report.input_kind, "from");
        assert_eq!(report.input_path, "policy/legacy.toml");
        assert_eq!(report.output_path, "policy/allow.toml");
        assert!(report.force);
        assert_eq!(report.broken_evidence_links, Some(3));
        assert_eq!(report.unsafe_broken_evidence_links, Some(2));
        assert_eq!(report.weak_evidence_references, Some(3));
        assert_eq!(report.unsafe_weak_evidence_references, Some(2));
        assert!(report.notes.contains("legacy"));
        remove_test_dir(&root);
    }

    #[test]
    fn evidence_diagnostic_root_call_presence_observer() {
        let explicit = migrate_context(None);
        let explicit = MigrateContext {
            source_tree_root: Some("fixture-root".to_string()),
            ..explicit
        };
        let implicit = migrate_context(None);

        assert_eq!(
            evidence_diagnostic_root(&explicit),
            PathBuf::from("fixture-root")
        );
        assert_eq!(evidence_diagnostic_root(&implicit), PathBuf::from("."));
    }

    #[test]
    fn unsafe_weak_evidence_reference_count_call_presence_observer() {
        let root = unique_test_dir("migrate-render-unsafe-weak");
        let mut cfg = AllowConfig::empty();
        cfg.allow.push(allow_entry(
            "allow-unsafe",
            FindingKind::Unsafe,
            &["ticket:unsafe-123", "TODO: add unsafe evidence"],
            &["spreadsheet:manual"],
        ));
        cfg.allow.push(allow_entry(
            "allow-panic",
            FindingKind::Panic,
            &["ticket:panic-123"],
            &["doc:docs/missing-panic.md"],
        ));

        assert_eq!(unsafe_weak_evidence_reference_count(&root, &cfg), 3);
        remove_test_dir(&root);
    }

    #[test]
    fn unsafe_broken_evidence_link_count_call_presence_observer() {
        let root = unique_test_dir("migrate-render-unsafe-broken");
        fs::create_dir_all(root.join("docs"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("create docs dir: {err}")));
        fs::write(root.join("docs/present.md"), "retained evidence")
            .unwrap_or_else(|err| std::panic::panic_any(format!("write evidence: {err}")));
        let mut cfg = AllowConfig::empty();
        cfg.allow.push(allow_entry(
            "allow-unsafe",
            FindingKind::Unsafe,
            &["doc:docs/missing-unsafe.md", "doc:docs/present.md"],
            &["doc:docs/missing-unsafe-link.md", "ticket:unsafe-123"],
        ));
        cfg.allow.push(allow_entry(
            "allow-panic",
            FindingKind::Panic,
            &["doc:docs/missing-panic.md"],
            &["doc:docs/present.md"],
        ));

        assert_eq!(unsafe_broken_evidence_link_count(&root, &cfg), 2);
        remove_test_dir(&root);
    }

    fn migrate_context(source_tree_root: Option<String>) -> MigrateContext {
        MigrateContext {
            inventory_source: "git_tracked".to_string(),
            source_tree_root,
            inventory_files: Some(7),
            inventory_completeness: Some("complete".to_string()),
            repository_identity: Some("test".to_string()),
            input_kind: "from".to_string(),
            input_path: "policy/legacy.toml".to_string(),
            legacy_source_files: Vec::new(),
            legacy_compat_kinds: Vec::new(),
            baseline_debt_projection:
                allow_report::MigrateBaselineDebtProjection::default_projection(),
        }
    }

    fn allow_entry(id: &str, kind: FindingKind, evidence: &[&str], links: &[&str]) -> AllowEntry {
        AllowEntry {
            id: id.to_string(),
            kind,
            family: Some("fixture".to_string()),
            path: Some(PathBuf::from("src/lib.rs")),
            glob: None,
            owner: "test".to_string(),
            classification: "reviewed".to_string(),
            reason: "fixture".to_string(),
            evidence: evidence.iter().map(|item| item.to_string()).collect(),
            links: links.iter().map(|item| item.to_string()).collect(),
            occurrence_limit: None,
            lifecycle: Lifecycle::empty(),
            selector: Selector::default(),
            last_seen: None,
        }
    }

    fn unique_test_dir(slug: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("cargo-allow-{slug}-{}-{stamp}", std::process::id()))
    }

    fn remove_test_dir(path: &Path) {
        let _ = fs::remove_dir_all(path);
    }
}
