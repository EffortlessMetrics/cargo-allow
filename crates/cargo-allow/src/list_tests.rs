use super::*;
use crate::{CargoAllowCli, CargoAllowCommand};
use allow_core::{Lifecycle, Selector, Span, StructuralIdentity};
use clap::Parser;
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
        "--status",
        "baseline_debt",
        "--expired",
        "--review-due",
        "--stale",
        "--baseline-debt",
        "--broad-scope",
        "--missing-evidence",
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
            status: Some(status),
            expired: true,
            review_due: true,
            stale: true,
            baseline_debt: true,
            broad_scope: true,
            missing_evidence: true,
            format: ListFormat::Json,
            output: Some(path),
            ..
        })) if kind == "unsafe"
            && family == "unsafe_fn"
            && owner == "runtime"
            && classification == "baseline_debt"
            && path_filter == "crates/allow-core"
            && source_package == "allow-core"
            && status == "baseline_debt"
            && path == Path::new("target/list.json")
    ));
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

    let rows = list_rows(&cfg, &findings, &outcomes);

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
fn render_list_rows_filters_owner_kind_classification_and_baseline_debt() {
    let rows = vec![
        list_row(
            "allow-runtime",
            FindingKind::Unsafe,
            "runtime",
            "baseline_debt",
        ),
        list_row(
            "allow-parser",
            FindingKind::Panic,
            "parser",
            "reviewed_exception",
        ),
    ];
    let filters = ListFilters {
        kind: Some(parse_kind_filter("unsafe").unwrap_or_else(|err| {
            std::panic::panic_any(format!("kind filter should parse: {err}"))
        })),
        family: None,
        owner: Some("runtime"),
        classification: Some("baseline_debt"),
        path: None,
        source_package: None,
        status: None,
        expired: false,
        review_due: false,
        stale: false,
        baseline_debt: true,
        broad_scope: false,
        missing_evidence: false,
    };

    let text = render_list_rows(&rows, &filters);

    assert!(text.contains("allow-runtime"));
    assert!(!text.contains("allow-parser"));
    assert!(text.contains("baseline_debt"));
}

#[test]
fn render_list_rows_filters_classification_without_baseline_shortcut() {
    let rows = vec![
        list_row(
            "allow-runtime",
            FindingKind::Unsafe,
            "runtime",
            "baseline_debt",
        ),
        list_row(
            "allow-parser",
            FindingKind::Panic,
            "parser",
            "reviewed_exception",
        ),
    ];
    let filters = ListFilters {
        kind: None,
        family: None,
        owner: None,
        classification: Some("reviewed_exception"),
        path: None,
        source_package: None,
        status: None,
        expired: false,
        review_due: false,
        stale: false,
        baseline_debt: false,
        broad_scope: false,
        missing_evidence: false,
    };

    let text = render_list_rows(&rows, &filters);

    assert!(!text.contains("allow-runtime"));
    assert!(text.contains("allow-parser"));
    assert!(text.contains("reviewed_exception"));
}

#[test]
fn render_list_rows_filters_source_package() {
    let mut allow_core = list_row("allow-core", FindingKind::Panic, "parser", "baseline_debt");
    allow_core.source_package = Some("allow-core".to_string());
    let mut allow_rust = list_row("allow-rust", FindingKind::Panic, "scanner", "baseline_debt");
    allow_rust.source_package = Some("allow-rust".to_string());
    let rows = vec![allow_core, allow_rust];
    let filters = ListFilters {
        kind: None,
        family: None,
        owner: None,
        classification: None,
        path: None,
        source_package: Some("allow-core"),
        status: None,
        expired: false,
        review_due: false,
        stale: false,
        baseline_debt: false,
        broad_scope: false,
        missing_evidence: false,
    };

    let text = render_list_rows(&rows, &filters);

    assert!(text.contains("allow-core"));
    assert!(!text.contains("allow-rust"));
}

#[test]
fn render_list_rows_filters_family() {
    let mut indexing = list_row("allow-index", FindingKind::Panic, "parser", "baseline_debt");
    indexing.family = Some("indexing".to_string());
    let mut unwrap = list_row(
        "allow-unwrap",
        FindingKind::Panic,
        "parser",
        "baseline_debt",
    );
    unwrap.family = Some("unwrap".to_string());
    let rows = vec![indexing, unwrap];
    let filters = ListFilters {
        kind: None,
        family: Some("unwrap"),
        owner: None,
        classification: None,
        path: None,
        source_package: None,
        status: None,
        expired: false,
        review_due: false,
        stale: false,
        baseline_debt: false,
        broad_scope: false,
        missing_evidence: false,
    };

    let text = render_list_rows(&rows, &filters);

    assert!(!text.contains("allow-index"));
    assert!(text.contains("allow-unwrap"));
}

#[test]
fn render_list_rows_filters_status() {
    let mut baseline = list_row(
        "allow-baseline",
        FindingKind::Panic,
        "parser",
        "baseline_debt",
    );
    baseline.status = MatchStatus::BaselineDebt;
    let mut stale = list_row(
        "allow-stale",
        FindingKind::Panic,
        "parser",
        "reviewed_exception",
    );
    stale.status = MatchStatus::Stale;
    let rows = vec![baseline, stale];
    let filters = ListFilters {
        kind: None,
        family: None,
        owner: None,
        classification: None,
        path: None,
        source_package: None,
        status: Some("stale"),
        expired: false,
        review_due: false,
        stale: false,
        baseline_debt: false,
        broad_scope: false,
        missing_evidence: false,
    };

    let text = render_list_rows(&rows, &filters);

    assert!(!text.contains("allow-baseline"));
    assert!(text.contains("allow-stale"));
}

#[test]
fn render_list_rows_filters_path_prefix_and_covering_glob() {
    let mut allow_core = list_row("allow-core", FindingKind::Panic, "parser", "baseline_debt");
    allow_core.scope = "crates/allow-core/src/lib.rs".to_string();
    let mut broad = list_row(
        "allow-broad",
        FindingKind::NonRustFile,
        "tools",
        "baseline_debt",
    );
    broad.scope = "crates/allow-core/**".to_string();
    let mut allow_rust = list_row("allow-rust", FindingKind::Panic, "scanner", "baseline_debt");
    allow_rust.scope = "crates/allow-rust/src/lib.rs".to_string();
    let rows = vec![allow_core, broad, allow_rust];
    let filters = ListFilters {
        kind: None,
        family: None,
        owner: None,
        classification: None,
        path: Some(r"crates\allow-core"),
        source_package: None,
        status: None,
        expired: false,
        review_due: false,
        stale: false,
        baseline_debt: false,
        broad_scope: false,
        missing_evidence: false,
    };

    let text = render_list_rows(&rows, &filters);

    assert!(text.contains("allow-core"));
    assert!(text.contains("allow-broad"));
    assert!(!text.contains("allow-rust"));
}

#[test]
fn render_list_rows_filters_broad_scope() {
    let mut exact = list_row("allow-exact", FindingKind::Panic, "parser", "baseline_debt");
    exact.scope = "crates/allow-core/src/lib.rs".to_string();
    let mut broad = list_row(
        "allow-broad",
        FindingKind::NonRustFile,
        "tools",
        "baseline_debt",
    );
    broad.scope = "crates/allow-core/**".to_string();
    let rows = vec![exact, broad];
    let filters = ListFilters {
        kind: None,
        family: None,
        owner: None,
        classification: None,
        path: None,
        source_package: None,
        status: None,
        expired: false,
        review_due: false,
        stale: false,
        baseline_debt: false,
        broad_scope: true,
        missing_evidence: false,
    };

    let text = render_list_rows(&rows, &filters);

    assert!(!text.contains("allow-exact"));
    assert!(text.contains("allow-broad"));
}

#[test]
fn render_list_rows_reports_and_filters_missing_evidence() {
    let mut missing = list_row(
        "allow-missing",
        FindingKind::Panic,
        "parser",
        "baseline_debt",
    );
    missing.evidence_count = 0;
    let mut evidenced = list_row(
        "allow-evidenced",
        FindingKind::Unsafe,
        "runtime",
        "reviewed_exception",
    );
    evidenced.evidence_count = 2;
    let rows = vec![missing, evidenced];
    let filters = ListFilters {
        kind: None,
        family: None,
        owner: None,
        classification: None,
        path: None,
        source_package: None,
        status: None,
        expired: false,
        review_due: false,
        stale: false,
        baseline_debt: false,
        broad_scope: false,
        missing_evidence: true,
    };

    let text = render_list_rows(&rows, &filters);

    assert!(text.contains("evidence_count"));
    assert!(text.contains("allow-missing"));
    assert!(!text.contains("allow-evidenced"));
}

#[test]
fn render_list_rows_json_records_context_filters_and_rows() {
    let json = sample_list_json_for_contract_test();

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
}

fn row_status(rows: &[ListRow], id: &str) -> MatchStatus {
    rows.iter()
        .find(|row| row.id == id)
        .map(|row| row.status)
        .unwrap_or_else(|| std::panic::panic_any(format!("missing row {id}")))
}

fn list_row(id: &str, kind: FindingKind, owner: &str, classification: &str) -> ListRow {
    ListRow {
        id: id.to_string(),
        status: if classification == "baseline_debt" {
            MatchStatus::BaselineDebt
        } else {
            MatchStatus::Matched
        },
        matches: 1,
        kind,
        family: None,
        owner: owner.to_string(),
        classification: classification.to_string(),
        scope: "src/lib.rs".to_string(),
        source_package: None,
        evidence_count: 0,
        review_after: "-".to_string(),
        expires: "-".to_string(),
        reason: "reason".to_string(),
    }
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

fn test_finding(kind: FindingKind, family: Option<&str>, path: &str, ast_kind: &str) -> Finding {
    Finding {
        kind,
        family: family.map(str::to_string),
        path: PathBuf::from(path),
        span: Some(Span { line: 1, column: 1 }),
        identity: StructuralIdentity::new("file", ast_kind),
        message: "test finding".to_string(),
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
        finding_index,
        message: message.to_string(),
        score: 100,
    }
}
