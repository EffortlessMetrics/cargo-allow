use super::*;

#[test]
fn migrate_json_renderer_records_io_summary_and_notes() {
    let report = MigrateReport {
        inventory: InventoryContext::new(
            "source_tree",
            "policy_migration",
            "git_tracked",
            Some("H:/Code/Rust/cargo-allow"),
            Some(76),
        ),
        input_kind: "repo_policy",
        input_path: "policy",
        output_path: "policy/allow.toml",
        force: true,
        allow_entries: 12,
        baseline_debt: 5,
        unsafe_entries: 2,
        lint_exception_entries: 4,
        entries_with_evidence: 3,
        evidence_entries: 5,
        entries_with_links: 6,
        link_entries: 7,
        broken_evidence_links: Some(3),
        unsafe_broken_evidence_links: Some(1),
        weak_evidence_references: Some(2),
        unsafe_weak_evidence_references: Some(1),
        notes: "migration notes",
    };

    let closeout_input = sample_closeout_input(report, 0, &[]);
    let json = render_migrate_json(report, closeout_input, &sample_mutation_receipt());

    assert!(json.contains("\"schema_id\": \"cargo-allow.migrate.v1\""));
    assert!(json.contains("\"command\": \"migrate\""));
    assert!(json.contains("\"scanner\": \"policy_migration\""));
    assert!(json.contains("\"source\": \"git_tracked\""));
    assert!(json.contains("\"files_scanned\": 76"));
    assert!(json.contains("\"kind\": \"repo_policy\""));
    assert!(json.contains("\"path\": \"policy\""));
    assert!(json.contains("\"path\": \"policy/allow.toml\""));
    assert!(json.contains("\"force\": true"));
    assert!(json.contains("\"allow_entries\": 12"));
    assert!(json.contains("\"baseline_debt\": 5"));
    assert!(json.contains("\"unsafe_entries\": 2"));
    assert!(json.contains("\"lint_exception_entries\": 4"));
    assert!(json.contains("\"entries_with_evidence\": 3"));
    assert!(json.contains("\"evidence_entries\": 5"));
    assert!(json.contains("\"entries_with_links\": 6"));
    assert!(json.contains("\"link_entries\": 7"));
    assert!(json.contains("\"broken_evidence_links\": 3"));
    assert!(json.contains("\"unsafe_broken_evidence_links\": 1"));
    assert!(json.contains("\"weak_evidence_references\": 2"));
    assert!(json.contains("\"unsafe_weak_evidence_references\": 1"));
    assert!(json.contains("\"follow_up_queues\""));
    assert!(json.contains("\"signal\": \"baseline_debt\""));
    assert!(json.contains("\"label\": \"baseline debt entries\""));
    assert!(json.contains("\"item_kind\": \"baseline_debt\""));
    assert!(
        json.contains(
            "\"command\": \"cargo-allow worklist --item-kind baseline_debt --format json\""
        )
    );
    assert!(json.contains("\"evidence_repair_queues\""));
    assert!(json.contains("\"signal\": \"broken_evidence_links\""));
    assert!(json.contains("\"label\": \"broken evidence links\""));
    assert!(json.contains("\"route_kind\": \"worklist_item_kind\""));
    assert!(json.contains("\"item_kind\": \"broken_evidence_link\""));
    assert!(json.contains("\"signal\": \"weak_evidence_references\""));
    assert!(json.contains("\"label\": \"weak evidence references\""));
    assert!(json.contains("\"item_kind\": \"weak_evidence_reference\""));
    assert!(json.contains(
        "\"command\": \"cargo-allow worklist --item-kind broken_evidence_link --format json\""
    ));
    assert!(json.contains(
        "\"command\": \"cargo-allow worklist --item-kind weak_evidence_reference --format json\""
    ));
    assert!(json.contains(
        "\"unsafe_command\": \"cargo-allow worklist --item-kind broken_evidence_link --kind unsafe --format json\""
    ));
    assert!(json.contains(
        "\"unsafe_command\": \"cargo-allow worklist --item-kind weak_evidence_reference --kind unsafe --format json\""
    ));
    assert!(json.contains("\"closeout\""));
    assert!(json.contains("\"legacy_retirement\""));
    assert!(json.contains("\"next_queues\""));
    assert!(json.contains("\"notes\": \"migration notes\""));

    let closeout_input = sample_closeout_input(report, 0, &[]);
    let text = render_migrate_human(report, closeout_input);

    assert!(text.contains("cargo-allow migrate summary"));
    assert!(text.contains("input_kind: repo_policy"));
    assert!(text.contains("input: policy"));
    assert!(text.contains("output: policy/allow.toml"));
    assert!(text.contains("force: true"));
    assert!(text.contains("allow_entries: 12"));
    assert!(text.contains("baseline_debt: 5"));
    assert!(text.contains("unsafe_entries: 2"));
    assert!(text.contains("lint_exception_entries: 4"));
    assert!(text.contains("entries_with_evidence: 3"));
    assert!(text.contains("evidence_entries: 5"));
    assert!(text.contains("entries_with_links: 6"));
    assert!(text.contains("link_entries: 7"));
    assert!(text.contains("broken_evidence_links: 3"));
    assert!(text.contains("unsafe_broken_evidence_links: 1"));
    assert!(text.contains("weak_evidence_references: 2"));
    assert!(text.contains("unsafe_weak_evidence_references: 1"));
    assert!(text.contains("follow_up_queues:"));
    assert!(text.contains("cargo-allow worklist --item-kind baseline_debt --format json"));
    assert!(text.contains("evidence_repair_queues:"));
    assert!(text.contains("cargo-allow worklist --item-kind broken_evidence_link --format json"));
    assert!(text.contains(
        "cargo-allow worklist --item-kind broken_evidence_link --kind unsafe --format json"
    ));
    assert!(
        text.contains("cargo-allow worklist --item-kind weak_evidence_reference --format json")
    );
    assert!(text.contains(
        "cargo-allow worklist --item-kind weak_evidence_reference --kind unsafe --format json"
    ));
    assert!(
        text.contains("inventory: source_tree/policy_migration via git_tracked; files scanned: 76")
    );
    assert!(text.contains("source_tree_root: H:/Code/Rust/cargo-allow"));
    assert!(text.contains("closeout:"));
    assert!(text.contains("legacy_retirement.ready: false"));
    assert!(text.contains("migration notes"));
    assert!(text.contains(CLAIM_BOUNDARY_TEXT));

    let styled =
        render_migrate_human_styled(report, sample_closeout_input(report, 0, &[]), Style::ANSI);
    assert!(styled.contains("migration posture: \u{1b}[31mblocked\u{1b}[0m"));
    assert_eq!(styled.matches('\u{1b}').count(), 2);

    let mut ready_report = report;
    ready_report.baseline_debt = 0;
    ready_report.broken_evidence_links = None;
    ready_report.unsafe_broken_evidence_links = None;
    ready_report.weak_evidence_references = None;
    ready_report.unsafe_weak_evidence_references = None;
    let ready = render_migrate_human_styled(
        ready_report,
        sample_closeout_input(ready_report, 0, &[]),
        Style::ANSI,
    );
    assert!(ready.contains("migration posture: \u{1b}[32mready\u{1b}[0m"));
}

fn sample_closeout_input<'a>(
    report: MigrateReport<'a>,
    missing_evidence_entries: usize,
    legacy_sources: &'a [MigrateLegacySource],
) -> MigrateCloseoutInput<'a> {
    MigrateCloseoutInput {
        report,
        missing_evidence_entries,
        legacy_sources,
        baseline_debt_projection:
            crate::migrate_closeout::MigrateBaselineDebtProjection::default_projection(),
    }
}

#[test]
fn migrate_report_from_config_counts_summary_fields() {
    let mut cfg = allow_core::AllowConfig::empty();
    cfg.allow = vec![
        allow_entry(
            "allow-baseline",
            allow_core::FindingKind::Panic,
            "baseline_debt",
            &[],
            &["legacy-policy:allow-baseline"],
        ),
        allow_entry(
            "allow-unsafe",
            allow_core::FindingKind::Unsafe,
            "ffi_boundary",
            &["doc:docs/safety.md"],
            &["legacy-policy:allow-unsafe"],
        ),
        allow_entry(
            "allow-non-rust",
            allow_core::FindingKind::NonRustFile,
            "release_script",
            &["issue:123"],
            &[],
        ),
        allow_entry(
            "allow-lint",
            allow_core::FindingKind::LintException,
            "lint_exception",
            &[],
            &[],
        ),
    ];

    let report = MigrateReport::from_config(
        InventoryContext::new(
            "source_tree",
            "policy_migration",
            "filesystem_fallback",
            Some("snapshot"),
            Some(3),
        ),
        &cfg,
        "repo_policy",
        "policy",
        "policy/allow.toml",
        false,
        "migration notes",
    );

    assert_eq!(report.allow_entries, 4);
    assert_eq!(report.baseline_debt, 1);
    assert_eq!(report.unsafe_entries, 1);
    assert_eq!(report.lint_exception_entries, 1);
    assert_eq!(report.entries_with_evidence, 2);
    assert_eq!(report.evidence_entries, 2);
    assert_eq!(report.entries_with_links, 2);
    assert_eq!(report.link_entries, 2);
    assert_eq!(report.broken_evidence_links, None);
    assert_eq!(report.unsafe_broken_evidence_links, None);
    assert_eq!(report.weak_evidence_references, None);
    assert_eq!(report.unsafe_weak_evidence_references, None);
    assert_eq!(report.inventory.scanner, "policy_migration");

    let closeout_input = sample_closeout_input(report, 0, &[]);
    let text = render_migrate_human(report, closeout_input);
    assert!(
        text.contains("follow_up_queues:"),
        "baseline-debt migration summaries should route follow-up queues"
    );
    assert!(
        text.contains("cargo-allow worklist --item-kind baseline_debt --format json"),
        "baseline-debt migration summaries should point at the baseline-debt worklist"
    );
    assert!(
        !text.contains("evidence_repair_queues:"),
        "clean migration summaries should not route repair queues"
    );
    let closeout_input = sample_closeout_input(report, 0, &[]);
    let json = render_migrate_json(report, closeout_input, &sample_mutation_receipt());
    assert!(
        json.contains("\"follow_up_queues\""),
        "baseline-debt migration JSON should emit follow-up queues"
    );
    assert!(
        json.contains("\"signal\": \"baseline_debt\""),
        "baseline-debt migration JSON should identify the summary signal"
    );
    assert!(
        !json.contains("\"evidence_repair_queues\""),
        "clean migration JSON should not emit repair queues"
    );
}

#[test]
fn migrate_repair_queues_omit_unsafe_command_without_unsafe_count() {
    let report = MigrateReport {
        inventory: InventoryContext::new(
            "source_tree",
            "policy_migration",
            "git_tracked",
            Some("H:/Code/Rust/cargo-allow"),
            Some(76),
        ),
        input_kind: "repo_policy",
        input_path: "policy",
        output_path: "policy/allow.toml",
        force: true,
        allow_entries: 1,
        baseline_debt: 0,
        unsafe_entries: 0,
        lint_exception_entries: 0,
        entries_with_evidence: 1,
        evidence_entries: 1,
        entries_with_links: 0,
        link_entries: 0,
        broken_evidence_links: Some(1),
        unsafe_broken_evidence_links: None,
        weak_evidence_references: None,
        unsafe_weak_evidence_references: None,
        notes: "migration notes",
    };

    let closeout_input = sample_closeout_input(report, 0, &[]);
    let json = render_migrate_json(report, closeout_input, &sample_mutation_receipt());

    assert!(json.contains("\"evidence_repair_queues\""));
    assert!(
        !json.contains("\"follow_up_queues\""),
        "migration JSON without baseline debt should not emit follow-up queues"
    );
    assert!(json.contains(
        "\"command\": \"cargo-allow worklist --item-kind broken_evidence_link --format json\""
    ));
    assert!(
        !json.contains("\"unsafe_command\""),
        "non-unsafe evidence repair queues should not emit unsafe-scoped routing"
    );

    let closeout_input = sample_closeout_input(report, 0, &[]);
    let text = render_migrate_human(report, closeout_input);
    assert!(text.contains("cargo-allow worklist --item-kind broken_evidence_link --format json"));
    assert!(
        !text.contains("--kind unsafe"),
        "non-unsafe human repair queues should not include unsafe-scoped routing"
    );
}

#[test]
fn migrate_repair_queues_normalize_unsafe_subset_counts() {
    let report = MigrateReport {
        inventory: InventoryContext::new(
            "source_tree",
            "policy_migration",
            "git_tracked",
            Some("H:/Code/Rust/cargo-allow"),
            Some(76),
        ),
        input_kind: "repo_policy",
        input_path: "policy",
        output_path: "policy/allow.toml",
        force: true,
        allow_entries: 1,
        baseline_debt: 0,
        unsafe_entries: 1,
        lint_exception_entries: 0,
        entries_with_evidence: 1,
        evidence_entries: 1,
        entries_with_links: 0,
        link_entries: 0,
        broken_evidence_links: None,
        unsafe_broken_evidence_links: Some(1),
        weak_evidence_references: None,
        unsafe_weak_evidence_references: None,
        notes: "migration notes",
    };

    let closeout_input = sample_closeout_input(report, 0, &[]);
    let json = render_migrate_json(report, closeout_input, &sample_mutation_receipt());

    assert!(json.contains("\"count\": 1"));
    assert!(json.contains("\"unsafe_count\": 1"));
    assert!(json.contains(
        "\"unsafe_command\": \"cargo-allow worklist --item-kind broken_evidence_link --kind unsafe --format json\""
    ));
}

fn allow_entry(
    id: &str,
    kind: allow_core::FindingKind,
    classification: &str,
    evidence: &[&str],
    links: &[&str],
) -> allow_core::AllowEntry {
    allow_core::AllowEntry {
        id: id.to_string(),
        kind,
        family: None,
        path: None,
        glob: None,
        owner: "owner".to_string(),
        classification: classification.to_string(),
        reason: "reason".to_string(),
        evidence: evidence.iter().map(|item| (*item).to_string()).collect(),
        links: links.iter().map(|item| (*item).to_string()).collect(),
        occurrence_limit: None,
        lifecycle: allow_core::Lifecycle::empty(),
        selector: allow_core::Selector::default(),
        last_seen: None,
    }
}

fn sample_mutation_receipt() -> MutationReceipt<'static> {
    MutationReceipt {
        operation: "migrate",
        tool_version: "0.1.10",
        repo_root: Some("H:/Code/Rust/cargo-allow"),
        config_source: Some("policy/allow.toml"),
        ledger_ids: Vec::new(),
        changed_allow_ids: vec!["allow-migrated"],
        before_fingerprints: vec![None],
        after_fingerprints: vec![Some(
            "sha256:v1:0000000000000000000000000000000000000000000000000000000000000000"
                .to_string(),
        )],
        result: "written",
        next_commands: vec!["cargo-allow check --mode no-new".to_string()],
    }
}
