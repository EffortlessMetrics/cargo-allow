use super::test_support::{list_row, row_status, test_entry, test_finding, test_outcome};
use super::*;
use crate::{CargoAllowCli, CargoAllowCommand, HumanJsonFormat};
use clap::Parser;
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

fn argv(items: Vec<&str>) -> Vec<String> {
    items.into_iter().map(String::from).collect()
}

#[test]
fn clap_parses_list_json_filters() {
    let parsed = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "list",
        "--kind",
        "unsafe",
        "--family",
        "unsafe_fn",
        "--owner",
        "runtime",
        "--classification",
        "baseline_debt",
        "--path",
        "crates/allow-core",
        "--source-package",
        "allow-core",
        "--allow-id",
        "allow-runtime",
        "--status",
        "baseline_debt",
        "--baseline-debt",
        "--broad-scope",
        "--missing-evidence",
        "--broken-evidence",
        "--weak-evidence",
        "--format",
        "json",
        "--output",
        "target/list.json",
    ]))
    .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse list args: {err}")));

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::List(ListArgs {
            kind: Some(kind),
            family: Some(family),
            owner: Some(owner),
            classification: Some(classification),
            path: Some(path_filter),
            source_package: Some(source_package),
            allow_id: Some(allow_id),
            status: Some(status),
            expired: false,
            review_due: false,
            stale: false,
            location_drift: false,
            baseline_debt: true,
            broad_scope: true,
            missing_evidence: true,
            broken_evidence: true,
            weak_evidence: true,
            format: HumanJsonFormat::Json,
            output: Some(path),
            columns: None,
            ..
        })) if kind == "unsafe"
            && family == "unsafe_fn"
            && owner == "runtime"
            && classification == "baseline_debt"
            && path_filter == "crates/allow-core"
            && source_package == "allow-core"
            && allow_id == "allow-runtime"
            && status == "baseline_debt"
            && path == Path::new("target/list.json")
    ));
}

#[test]
fn clap_parses_list_columns_arg() {
    // #2595: --columns is accepted as a free-form string; parse_csv
    // validation happens downstream in cmd_list.
    let parsed = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "list",
        "--columns",
        "id,status,reason",
    ]))
    .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse --columns: {err}")));

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::List(ListArgs {
            columns: Some(cols),
            ..
        })) if cols == "id,status,reason"
    ));
}

#[test]
fn clap_parses_list_wide_arg() {
    let parsed = CargoAllowCli::try_parse_from(argv(vec!["cargo-allow", "list", "--wide"]))
        .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse --wide: {err}")));

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::List(ListArgs { wide: true, .. }))
    ));
}

#[test]
fn clap_parses_list_width_arg() {
    let parsed = CargoAllowCli::try_parse_from(argv(vec!["cargo-allow", "list", "--width", "60"]))
        .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse --width: {err}")));

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::List(ListArgs {
            width: Some(60),
            wide: false,
            columns: None,
            ..
        }))
    ));
}

#[test]
fn clap_rejects_list_width_below_minimum_and_with_full_views() {
    let narrow = CargoAllowCli::try_parse_from(argv(vec!["cargo-allow", "list", "--width", "39"]))
        .expect_err("width below the minimum should fail");
    assert!(
        narrow.to_string().contains("at least 40"),
        "unexpected narrow-width diagnostic: {narrow}"
    );

    for view in [vec!["--wide"], vec!["--columns", "id,status"]] {
        let mut args = vec!["cargo-allow", "list", "--width", "60"];
        args.extend(view);
        let err = CargoAllowCli::try_parse_from(argv(args))
            .expect_err("explicit width should be limited to concise cards");
        assert!(
            err.to_string().contains("cannot be used with") || err.to_string().contains("conflict"),
            "unexpected width/view conflict diagnostic: {err}"
        );
    }
}

#[test]
fn clap_rejects_list_wide_with_columns() {
    let err = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "list",
        "--wide",
        "--columns",
        "id,status",
    ]))
    .expect_err("--wide and --columns should be mutually exclusive");

    let message = err.to_string();
    assert!(
        message.contains("cannot be used with") || message.contains("conflict"),
        "unexpected conflict diagnostic: {message}"
    );
}

#[test]
fn list_wide_resolves_to_the_complete_human_projection() {
    let parsed = CargoAllowCli::try_parse_from(argv(vec!["cargo-allow", "list", "--wide"]))
        .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse --wide: {err}")));
    let args = match parsed.command {
        Some(CargoAllowCommand::List(args)) => args,
        _ => std::panic::panic_any("expected list command"),
    };

    let columns = super::list_columns(&args);
    let columns = columns
        .unwrap_or_else(|err| std::panic::panic_any(format!("--wide should resolve: {err}")));
    assert_eq!(columns, allow_report::ListColumn::ALL);
}

#[test]
fn list_columns_preserve_concise_default_and_explicit_selection() {
    let default_args = match CargoAllowCli::try_parse_from(argv(vec!["cargo-allow", "list"]))
        .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse list: {err}")))
        .command
    {
        Some(CargoAllowCommand::List(args)) => args,
        _ => std::panic::panic_any("expected list command"),
    };
    assert_eq!(
        super::list_columns(&default_args)
            .unwrap_or_else(|err| std::panic::panic_any(format!("default columns: {err}"))),
        allow_report::ListColumn::DEFAULT
    );

    let explicit_args = match CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "list",
        "--columns",
        "id,status",
    ]))
    .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse columns: {err}")))
    .command
    {
        Some(CargoAllowCommand::List(args)) => args,
        _ => std::panic::panic_any("expected list command"),
    };
    assert_eq!(
        super::list_columns(&explicit_args)
            .unwrap_or_else(|err| std::panic::panic_any(format!("explicit columns: {err}"))),
        vec![
            allow_report::ListColumn::Id,
            allow_report::ListColumn::Status
        ]
    );
}

#[test]
fn render_list_rows_with_columns_projects_subset() {
    // #2595: the adapter threads the column selection through to the
    // allow-report renderer. Exercises the same path cmd_list uses.
    let rows = vec![list_row(
        "allow-0001",
        FindingKind::Panic,
        "parser",
        "reviewed_exception",
    )];
    let filters = ListFilters {
        kind: None,
        family: None,
        owner: None,
        classification: None,
        path: None,
        source_package: None,
        allow_id: None,
        status: None,
        expired: false,
        review_due: false,
        stale: false,
        location_drift: false,
        baseline_debt: false,
        broad_scope: false,
        missing_evidence: false,
        broken_evidence: false,
        weak_evidence: false,
    };
    let context = ListContext {
        inventory: allow_report::InventoryContext::unknown_source_syntax(),
        kind_arg: None,
    };
    let columns = allow_report::ListColumn::parse_csv("id,kind,owner")
        .unwrap_or_else(|err| std::panic::panic_any(format!("parse_csv: {err}")));

    let text = render_list_rows_with_columns(&rows, &filters, context, &columns);

    assert!(
        text.contains("id\tkind\towner\n"),
        "header should be projected to id\\tkind\\towner: {text}"
    );
    // list_row sets owner="parser", id="allow-0001"; FindingKind::Panic
    // renders as "panic" via as_str() in the adapter.
    assert!(
        text.contains("allow-0001\tpanic\tparser\n"),
        "row should be projected to the three selected columns: {text}"
    );
}

#[test]
fn render_list_rows_with_context_still_emits_full_row() {
    // Backward-compat: the pre-#2595 adapter still renders the full
    // 17-column header when no projection is supplied.
    let rows = vec![list_row(
        "allow-full",
        FindingKind::Panic,
        "parser",
        "reviewed_exception",
    )];
    let filters = ListFilters {
        kind: None,
        family: None,
        owner: None,
        classification: None,
        path: None,
        source_package: None,
        allow_id: None,
        status: None,
        expired: false,
        review_due: false,
        stale: false,
        location_drift: false,
        baseline_debt: false,
        broad_scope: false,
        missing_evidence: false,
        broken_evidence: false,
        weak_evidence: false,
    };
    let context = ListContext {
        inventory: allow_report::InventoryContext::unknown_source_syntax(),
        kind_arg: None,
    };
    let text = render_list_rows_with_context(&rows, &filters, context);
    assert!(
        text.contains(
            "id\tstatus\tmatches\tkind\tfamily\towner\tclassification\tscope\tsource_package\tevidence_count\tbroken_evidence_references\tweak_evidence_references\tselector_precision\tbroad_scope\treview_after\texpires\treason\n"
        ),
        "full header should still be emitted by the legacy adapter: {text}"
    );
}

#[test]
fn list_view_parity_preserves_filtered_rows_and_order() {
    let rows = vec![
        list_row("allow-keep", FindingKind::Panic, "parser", "approved"),
        list_row("allow-skip", FindingKind::Unsafe, "runtime", "approved"),
    ];
    let filters = ListFilters {
        kind: Some(
            parse_kind_filter("panic")
                .unwrap_or_else(|err| std::panic::panic_any(format!("kind filter: {err}"))),
        ),
        ..ListFilters::default()
    };
    let context = ListContext {
        inventory: allow_report::InventoryContext::unknown_source_syntax(),
        kind_arg: Some("panic"),
    };

    let concise =
        render_list_rows_concise(&rows, &filters, context, allow_report::ListColumn::DEFAULT);
    let wide =
        render_list_rows_with_columns(&rows, &filters, context, allow_report::ListColumn::ALL);
    let json = render_list_rows_json(&rows, &filters, context);
    let value = serde_json::from_str::<Value>(&json)
        .unwrap_or_else(|err| std::panic::panic_any(format!("list JSON should parse: {err}")));

    for text in [&concise, &wide] {
        assert!(text.contains("allow-keep"));
        assert!(!text.contains("allow-skip"));
    }
    assert_eq!(
        value.pointer("/allow_entries/0/id").and_then(Value::as_str),
        Some("allow-keep")
    );
    assert!(value.pointer("/allow_entries/1").is_none());
}

#[test]
fn clap_rejects_list_status_combined_with_status_shortcuts() {
    for shortcut in ["--expired", "--review-due", "--stale", "--location-drift"] {
        let err = CargoAllowCli::try_parse_from(argv(vec![
            "cargo-allow",
            "list",
            "--status",
            "expired",
            shortcut,
        ]))
        .expect_err("status and status shortcuts should conflict");
        let message = err.to_string();
        assert!(
            message.contains("cannot be used with")
                || message.contains("conflict")
                || message.contains("--status"),
            "expected conflict diagnostic for {shortcut}, got: {message}"
        );
    }
}

#[test]
fn clap_parses_single_list_status_shortcut_with_baseline_debt() {
    let parsed = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "list",
        "--expired",
        "--baseline-debt",
    ]))
    .unwrap_or_else(|err| {
        std::panic::panic_any(format!(
            "single status shortcut with --baseline-debt should parse: {err}"
        ))
    });

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::List(ListArgs {
            status: None,
            expired: true,
            review_due: false,
            stale: false,
            location_drift: false,
            baseline_debt: true,
            ..
        }))
    ));
}

#[test]
fn clap_parses_list_location_drift_status() {
    let parsed = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "list",
        "--status",
        "location_drift",
    ]))
    .unwrap_or_else(|err| {
        std::panic::panic_any(format!("CLI should accept location_drift status: {err}"))
    });

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::List(ListArgs {
            status: Some(status),
            ..
        })) if status == "location_drift"
    ));
}

#[test]
fn clap_rejects_conflicting_list_status_shortcuts() {
    for pair in [
        ["--expired", "--stale"],
        ["--expired", "--review-due"],
        ["--stale", "--review-due"],
        ["--stale", "--location-drift"],
        ["--location-drift", "--expired"],
    ] {
        let err =
            CargoAllowCli::try_parse_from(argv(vec!["cargo-allow", "list", pair[0], pair[1]]))
                .expect_err("status shortcuts should be mutually exclusive");
        let message = err.to_string();
        assert!(
            message.contains("cannot be used with")
                || message.contains("conflict")
                || message.contains(pair[0])
                || message.contains(pair[1]),
            "expected conflict diagnostic for {} {}, got: {message}",
            pair[0],
            pair[1]
        );
    }
}

#[test]
fn clap_parses_list_location_drift_shortcut() {
    let parsed =
        CargoAllowCli::try_parse_from(argv(vec!["cargo-allow", "list", "--location-drift"]))
            .unwrap_or_else(|err| {
                std::panic::panic_any(format!("CLI should accept --location-drift: {err}"))
            });

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::List(ListArgs {
            location_drift: true,
            status: None,
            expired: false,
            review_due: false,
            stale: false,
            ..
        }))
    ));
}

#[test]
fn clap_rejects_unknown_list_kind() {
    let err = CargoAllowCli::try_parse_from(argv(vec!["cargo-allow", "list", "--kind", "unsfae"]))
        .expect_err("unknown list kind should fail closed");

    assert!(
        err.to_string().contains("unknown kind"),
        "unexpected parse error: {err}"
    );
}

#[test]
fn list_rows_report_lifecycle_stale_and_baseline_status() {
    let mut cfg = AllowConfig::empty();
    let mut expired = test_entry("allow-expired", FindingKind::Panic);
    expired.lifecycle.expires = Some("2000-01-01".to_string());
    let mut review_due = test_entry("allow-review", FindingKind::Panic);
    review_due.lifecycle.review_after = Some("2000-01-01".to_string());
    let mut baseline = test_entry("allow-baseline", FindingKind::Panic);
    baseline.classification = "baseline_debt".to_string();
    let stale = test_entry("allow-stale", FindingKind::Panic);
    cfg.allow = vec![expired, review_due, baseline, stale];
    let outcomes = vec![
        test_outcome(
            MatchStatus::Matched,
            Some("allow-expired"),
            Some(0),
            "matched",
        ),
        test_outcome(
            MatchStatus::Matched,
            Some("allow-review"),
            Some(1),
            "matched",
        ),
        test_outcome(
            MatchStatus::Matched,
            Some("allow-baseline"),
            Some(2),
            "matched",
        ),
        test_outcome(MatchStatus::Stale, Some("allow-stale"), None, "stale"),
    ];
    let expired_finding = test_finding(
        FindingKind::NonRustFile,
        None,
        "tracked-expired.file",
        "tracked_file",
    );
    let mut review_finding = test_finding(
        FindingKind::NonRustFile,
        None,
        "tracked-review.file",
        "tracked_file",
    );
    review_finding.identity.crate_name = Some("review-package".to_string());
    let stale_finding = test_finding(
        FindingKind::NonRustFile,
        None,
        "tracked-stale.file",
        "tracked_file",
    );
    let findings = vec![expired_finding, review_finding, stale_finding];

    let rows = list_rows(Path::new("."), &cfg, &findings, &outcomes);

    assert_eq!(row_status(&rows, "allow-expired"), MatchStatus::Expired);
    assert_eq!(row_status(&rows, "allow-review"), MatchStatus::ReviewDue);
    assert_eq!(
        rows.iter()
            .find(|row| row.id == "allow-review")
            .and_then(|row| row.source_package.as_deref()),
        Some("review-package")
    );
    assert_eq!(
        row_status(&rows, "allow-baseline"),
        MatchStatus::BaselineDebt
    );
    assert_eq!(row_status(&rows, "allow-stale"), MatchStatus::Stale);
}

#[test]
fn list_rows_report_broad_scope_from_selector_glob() {
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-broad", FindingKind::NonRustFile);
    entry.selector.glob = Some("scripts/**".to_string());
    cfg.allow.push(entry);
    let findings = vec![test_finding(
        FindingKind::NonRustFile,
        None,
        "scripts/release.sh",
        "tracked_file",
    )];
    let outcomes = vec![test_outcome(
        MatchStatus::Matched,
        Some("allow-broad"),
        Some(0),
        "matched",
    )];

    let rows = list_rows(Path::new("."), &cfg, &findings, &outcomes);

    assert!(
        rows.iter()
            .find(|row| row.id == "allow-broad")
            .is_some_and(|row| row.broad_scope)
    );
}

#[test]
fn list_rows_report_evidence_health_counts() {
    let root = list_fixture_dir();
    fs::create_dir_all(root.join("docs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture docs dir: {err}")));
    fs::write(root.join("docs/present.md"), "evidence")
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture evidence file: {err}")));
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-evidence-health", FindingKind::Unsafe);
    entry.evidence = vec![
        "doc:docs/present.md".to_string(),
        "doc:docs/missing.md".to_string(),
        "spreadsheet:manual-review".to_string(),
        "unstructured evidence note".to_string(),
    ];
    cfg.allow.push(entry);

    let rows = list_rows(&root, &cfg, &[], &[]);
    let row = rows
        .iter()
        .find(|row| row.id == "allow-evidence-health")
        .unwrap_or_else(|| std::panic::panic_any("expected evidence health row"));

    assert_eq!(row.evidence_count, 4);
    assert_eq!(row.broken_evidence_references, 1);
    assert_eq!(row.weak_evidence_references, 2);
    fs::remove_dir_all(root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn list_rows_report_link_health_counts() {
    let root = list_fixture_dir();
    fs::create_dir_all(root.join("docs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture docs dir: {err}")));
    fs::write(root.join("docs/present-link.md"), "rationale")
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture link file: {err}")));
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-link-health", FindingKind::NonRustFile);
    entry.evidence = vec!["test:list_rows_report_link_health_counts".to_string()];
    entry.links = vec![
        "doc:docs/present-link.md".to_string(),
        "doc:docs/missing-link.md".to_string(),
        "spreadsheet:manual-review".to_string(),
        "unstructured link note".to_string(),
    ];
    cfg.allow.push(entry);

    let rows = list_rows(&root, &cfg, &[], &[]);
    let row = rows
        .iter()
        .find(|row| row.id == "allow-link-health")
        .unwrap_or_else(|| std::panic::panic_any("expected link health row"));

    assert_eq!(
        row.evidence_count, 1,
        "list evidence_count remains the number of evidence entries"
    );
    assert_eq!(row.broken_evidence_references, 1);
    assert_eq!(row.weak_evidence_references, 2);
    fs::remove_dir_all(root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn list_rows_count_local_evidence_outside_source_tree_inventory_as_broken() {
    let root = list_fixture_dir();
    fs::create_dir_all(root.join("docs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture docs dir: {err}")));
    fs::write(root.join("docs/untracked.md"), "evidence")
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture evidence file: {err}")));
    let mut cfg = AllowConfig::empty();
    let mut entry = test_entry("allow-evidence-health", FindingKind::Unsafe);
    entry.evidence = vec!["doc:docs/untracked.md".to_string()];
    cfg.allow.push(entry);
    let source_tree_files = BTreeSet::new();

    let rows = list_rows_with_source_tree_files(&root, &cfg, &[], &[], Some(&source_tree_files));
    let row = rows
        .iter()
        .find(|row| row.id == "allow-evidence-health")
        .unwrap_or_else(|| std::panic::panic_any("expected evidence health row"));

    assert_eq!(row.evidence_count, 1);
    assert_eq!(row.broken_evidence_references, 1);
    assert_eq!(row.weak_evidence_references, 0);
    fs::remove_dir_all(root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
}

#[test]
fn render_list_rows_json_records_context_filters_and_rows() {
    let json = sample_list_json_for_contract_test();
    let value = parse_json("list artifact", &json);

    assert!(json.contains("\"schema_version\": 1"));
    assert!(json.contains(&format!(
        "\"schema_id\": \"{}\"",
        allow_report::LIST_SCHEMA_ID
    )));
    assert!(json.contains("\"command\": \"list\""));
    assert!(json.contains("\"claim_boundary\""));
    assert!(json.contains("\"scanner_limitations\""));
    assert!(json.contains("\"source\": \"git_tracked\""));
    assert!(json.contains("\"root\": \"H:/Code/Rust/cargo-allow\""));
    assert!(json.contains("\"files_scanned\": 46"));
    assert!(json.contains("\"kind\": \"panic\""));
    assert!(json.contains("\"family\": \"unwrap\""));
    assert!(json.contains("\"baseline_debt\": true"));
    assert!(json.contains("\"allow_entries\": 1"));
    assert!(json.contains("\"id\": \"allow-json\""));
    assert!(json.contains("\"source_package\": \"allow-core\""));
    assert!(json.contains("\"evidence_count\": 2"));
    assert!(json.contains("\"broken_evidence_references\": 1"));
    assert!(json.contains("\"weak_evidence_references\": 1"));
    assert!(json.contains("\"selector_precision\": 7"));
    assert!(json.contains("\"broad_scope\": false"));
    assert_eq!(
        value.pointer("/filters/kind").and_then(Value::as_str),
        Some("panic")
    );
    assert_eq!(
        value.pointer("/filters/family").and_then(Value::as_str),
        Some("unwrap")
    );
    assert_eq!(
        value.pointer("/filters/owner").and_then(Value::as_str),
        Some("parser")
    );
    assert_eq!(
        value
            .pointer("/filters/classification")
            .and_then(Value::as_str),
        Some("baseline_debt")
    );
    assert_eq!(
        value.pointer("/filters/path").and_then(Value::as_str),
        Some("src/lib.rs")
    );
    assert_eq!(
        value
            .pointer("/filters/source_package")
            .and_then(Value::as_str),
        Some("allow-core")
    );
    assert_eq!(
        value.pointer("/filters/allow_id").and_then(Value::as_str),
        Some("allow-json")
    );
    assert_eq!(
        value.pointer("/filters/status").and_then(Value::as_str),
        Some("baseline_debt")
    );
    assert_eq!(
        value.pointer("/filters/expired").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        value
            .pointer("/filters/review_due")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        value.pointer("/filters/stale").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        value
            .pointer("/filters/baseline_debt")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        value
            .pointer("/filters/broad_scope")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        value
            .pointer("/filters/missing_evidence")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        value
            .pointer("/filters/broken_evidence")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        value
            .pointer("/filters/weak_evidence")
            .and_then(Value::as_bool),
        Some(true)
    );
}

#[test]
fn render_list_rows_with_context_filters_rows_and_reports_inventory() {
    let rows = vec![
        list_row("allow-keep", FindingKind::Panic, "parser", "approved"),
        list_row("allow-skip", FindingKind::Unsafe, "runtime", "approved"),
    ];
    let filters = ListFilters {
        kind: Some(
            parse_kind_filter("panic")
                .unwrap_or_else(|err| std::panic::panic_any(format!("kind filter: {err}"))),
        ),
        family: None,
        owner: Some("parser"),
        classification: None,
        path: None,
        source_package: None,
        allow_id: None,
        status: None,
        expired: false,
        review_due: false,
        stale: false,
        location_drift: false,
        baseline_debt: false,
        broad_scope: false,
        missing_evidence: false,
        broken_evidence: false,
        weak_evidence: false,
    };
    let context = ListContext {
        inventory: allow_report::InventoryContext::source_syntax(
            "git_tracked",
            Some("H:/Code/Rust/cargo-allow"),
            Some(2),
        ),
        kind_arg: Some("panic"),
    };

    let text = render_list_rows_with_context(&rows, &filters, context);

    assert!(
        text.contains("inventory: source_tree/source_syntax via git_tracked; files scanned: 2")
    );
    assert!(text.contains("source_tree_root: H:/Code/Rust/cargo-allow"));
    assert!(text.contains("allow-keep\tmatched\t1\tpanic"));
    assert!(!text.contains("allow-skip"));
}

#[test]
fn render_list_rows_json_projects_rows_filters_and_dash_lifecycle_fields() {
    let mut keep = list_row("allow-keep", FindingKind::Panic, "parser", "approved");
    keep.family = Some("unwrap".to_string());
    keep.source_package = Some("allow-core".to_string());
    keep.evidence_count = 2;
    keep.broken_evidence_references = 1;
    keep.weak_evidence_references = 1;
    keep.selector_precision = 7;
    keep.broad_scope = true;
    keep.expires = "2026-12-01".to_string();
    let skip = list_row("allow-skip", FindingKind::Unsafe, "runtime", "approved");
    let filters = ListFilters {
        kind: Some(
            parse_kind_filter("panic")
                .unwrap_or_else(|err| std::panic::panic_any(format!("kind filter: {err}"))),
        ),
        family: Some("unwrap"),
        owner: Some("parser"),
        classification: Some("approved"),
        path: Some("src/lib.rs"),
        source_package: Some("allow-core"),
        allow_id: Some("allow-keep"),
        status: Some("matched"),
        expired: false,
        review_due: false,
        stale: false,
        location_drift: false,
        baseline_debt: false,
        broad_scope: true,
        missing_evidence: false,
        broken_evidence: true,
        weak_evidence: true,
    };
    let context = ListContext {
        inventory: allow_report::InventoryContext::source_syntax("git_tracked", None, Some(2)),
        kind_arg: Some("panic"),
    };

    let json = render_list_rows_json(&[keep, skip], &filters, context);
    let value = parse_json("list render rows json", &json);

    assert_eq!(
        value.pointer("/summary/allow_entries"),
        Some(&Value::from(1))
    );
    assert_eq!(
        value.pointer("/allow_entries/0/id").and_then(Value::as_str),
        Some("allow-keep")
    );
    assert_eq!(
        value.pointer("/filters/kind").and_then(Value::as_str),
        Some("panic")
    );
    assert_eq!(
        value.pointer("/filters/allow_id").and_then(Value::as_str),
        Some("allow-keep")
    );
    assert_eq!(
        value
            .pointer("/allow_entries/0/source_package")
            .and_then(Value::as_str),
        Some("allow-core")
    );
    assert!(
        value.pointer("/allow_entries/0/review_after").is_none(),
        "unavailable review_after should be omitted"
    );
    assert_eq!(
        value
            .pointer("/allow_entries/0/expires")
            .and_then(Value::as_str),
        Some("2026-12-01")
    );
    assert_eq!(
        value
            .pointer("/allow_entries/0/broken_evidence_references")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert!(
        !json.contains("allow-skip"),
        "filtered rows should not be projected into list JSON"
    );
}

fn list_fixture_dir() -> std::path::PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "cargo-allow-list-evidence-health-{}-{stamp}",
        std::process::id()
    ))
}

fn parse_json(name: &str, json: &str) -> Value {
    match serde_json::from_str(json) {
        Ok(value) => value,
        Err(err) => std::panic::panic_any(format!("{name} should parse as JSON: {err}\n{json}")),
    }
}
