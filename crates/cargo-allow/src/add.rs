use allow_core::{
    AllowConfig, AllowEntry, CargoAllowError, CargoAllowResult, Finding, FindingKind, LastSeen,
    Lifecycle, MatchStatus, SimpleDate, json_escape, normalize_path,
};
use allow_match::{CheckMode, evaluate, finding_location};
use allow_policy::{render_policy, validate_local_evidence_references, validate_policy};
use clap::{Parser, ValueEnum};
use std::path::{Path, PathBuf};

use crate::{
    KindFilter, RootArgs, explain_finding_json, last_seen_json, load_world, option_json_string,
    parse_kind_filter, selector_from_finding, selector_json, source_tree_root_text, write_file,
    write_file_no_overwrite,
};

#[derive(Debug, Clone, Parser)]
pub(crate) struct AddArgs {
    #[command(flatten)]
    root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    config: Option<PathBuf>,
    /// Finding kind to add.
    #[arg(long)]
    kind: String,
    /// Path containing the finding.
    #[arg(long)]
    path: PathBuf,
    /// Line near the finding.
    #[arg(long)]
    line: u32,
    /// Owner for the retained exception.
    #[arg(long)]
    owner: String,
    /// Reason this exception is acceptable.
    #[arg(long)]
    reason: String,
    /// Classification for the retained exception.
    #[arg(long, default_value = "reviewed_exception")]
    classification: String,
    /// Review date for the retained exception.
    #[arg(long, default_value = "2026-11-01")]
    review_after: String,
    /// Optional expiry date for the retained exception.
    #[arg(long)]
    expires: Option<String>,
    /// Evidence reference supporting this exception.
    #[arg(long)]
    evidence: Vec<String>,
    /// Entry ID. Defaults to the next allow-NNNN ID.
    #[arg(long)]
    id: Option<String>,
    /// Include untracked files in addition to git-tracked files.
    #[arg(long)]
    include_untracked: bool,
    /// Write proposed policy to this path.
    #[arg(long)]
    write: Option<PathBuf>,
    /// Overwrite an existing output policy file.
    #[arg(long)]
    force: bool,
    /// Summary output format. Policy output remains TOML.
    #[arg(long, value_enum, default_value_t = AddSummaryFormat::Human)]
    summary_format: AddSummaryFormat,
    /// Write add summary to a file instead of stderr.
    #[arg(long)]
    summary_output: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum AddSummaryFormat {
    Human,
    Json,
}

pub(crate) fn cmd_add(args: &AddArgs) -> CargoAllowResult<()> {
    let parsed_kind = parse_kind_filter(&args.kind)?;
    let (root, mut cfg, findings, inventory_facts) = load_world(
        args.root.root.as_deref(),
        args.config.as_deref(),
        true,
        Some(args.kind.as_str()),
        args.include_untracked,
    )?;
    let outcomes = evaluate(&cfg, &findings, CheckMode::Audit);
    let (finding_index, finding) =
        select_add_finding(&findings, parsed_kind, &args.path, args.line)?;
    let selected_outcome = outcomes
        .iter()
        .find(|outcome| outcome.finding_index == Some(finding_index))
        .ok_or_else(|| CargoAllowError::new("selected finding did not produce a match outcome"))?;
    ensure_addable_outcome(selected_outcome.status)?;
    if finding.kind == FindingKind::Unsafe && args.evidence.is_empty() {
        return Err(CargoAllowError::new(
            "unsafe allow entries require at least one --evidence reference",
        ));
    }
    let id = args.id.clone().unwrap_or_else(|| next_allow_id(&cfg));
    if cfg.allow.iter().any(|entry| entry.id == id) {
        return Err(CargoAllowError::new(format!(
            "allow entry id `{id}` already exists"
        )));
    }
    let entry = allow_entry_from_finding(AddEntryRequest {
        finding,
        id,
        owner: args.owner.clone(),
        classification: args.classification.clone(),
        reason: args.reason.clone(),
        evidence: args.evidence.clone(),
        review_after: args.review_after.clone(),
        expires: args.expires.clone(),
    });
    let root_text = source_tree_root_text(&root);
    let context = AddContext {
        inventory_source: inventory_facts.source.as_str(),
        source_tree_root: Some(&root_text),
        inventory_files: inventory_facts.files_scanned,
    };
    let summary = match args.summary_format {
        AddSummaryFormat::Human => render_add_summary(&entry, finding, args.write.as_deref()),
        AddSummaryFormat::Json => {
            render_add_summary_json(&entry, finding, args.write.as_deref(), args.force, context)
        }
    };
    cfg.allow.push(entry);
    validate_policy(&cfg)?;
    validate_local_evidence_references(&root, &cfg)?;
    let rendered = render_policy(&cfg);
    if let Some(path) = &args.write {
        write_file_no_overwrite(path, &rendered, args.force)?;
    } else {
        println!("{rendered}");
    }
    if let Some(path) = &args.summary_output {
        write_file(path, &summary)?;
    } else {
        eprintln!("{summary}");
    }
    Ok(())
}

fn select_add_finding<'a>(
    findings: &'a [Finding],
    kind: KindFilter,
    path: &Path,
    line: u32,
) -> CargoAllowResult<(usize, &'a Finding)> {
    let normalized_path = normalize_path(path);
    let mut candidates = findings
        .iter()
        .enumerate()
        .filter(|(_, finding)| kind.matches_finding(finding))
        .filter(|(_, finding)| normalize_path(&finding.path) == normalized_path)
        .filter_map(|(index, finding)| {
            finding
                .span
                .as_ref()
                .map(|span| (span.line.abs_diff(line), index, finding))
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|(distance, _, finding)| (*distance, normalize_path(&finding.path)));
    let Some((distance, index, finding)) = candidates.first().copied() else {
        return Err(CargoAllowError::new(format!(
            "no current {} finding found near {}:{}",
            kind.kind, normalized_path, line
        )));
    };
    let tied = candidates
        .iter()
        .filter(|(candidate_distance, _, _)| *candidate_distance == distance)
        .count();
    if tied > 1 {
        return Err(CargoAllowError::new(format!(
            "ambiguous add request: {tied} findings are equally near {}:{}",
            normalized_path, line
        )));
    }
    Ok((index, finding))
}

fn ensure_addable_outcome(status: MatchStatus) -> CargoAllowResult<()> {
    if status == MatchStatus::New {
        return Ok(());
    }
    Err(CargoAllowError::new(format!(
        "selected finding is already receipted or blocked with status `{}`; use list or explain before editing policy",
        status.as_str()
    )))
}

struct AddEntryRequest<'a> {
    finding: &'a Finding,
    id: String,
    owner: String,
    classification: String,
    reason: String,
    evidence: Vec<String>,
    review_after: String,
    expires: Option<String>,
}

fn allow_entry_from_finding(request: AddEntryRequest<'_>) -> AllowEntry {
    let selector = selector_from_finding(request.finding);
    AllowEntry {
        id: request.id,
        kind: request.finding.kind,
        family: request.finding.family.clone(),
        path: Some(request.finding.path.clone()),
        glob: None,
        owner: request.owner,
        classification: request.classification,
        reason: request.reason,
        evidence: request.evidence,
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: Some(SimpleDate::today_utc_approx().to_string()),
            review_after: Some(request.review_after),
            expires: request.expires,
        },
        selector,
        last_seen: request.finding.span.as_ref().map(|s| LastSeen {
            line: s.line,
            column: s.column,
        }),
    }
}

fn render_add_summary(entry: &AllowEntry, finding: &Finding, output: Option<&Path>) -> String {
    let mut out = String::new();
    out.push_str("cargo-allow add summary\n");
    out.push_str(&format!("id: {}\n", entry.id));
    out.push_str(&format!("kind: {}\n", entry.kind));
    if let Some(family) = &entry.family {
        out.push_str(&format!("family: {family}\n"));
    }
    out.push_str(&format!("scope: {}\n", entry.path_or_glob()));
    out.push_str(&format!("owner: {}\n", entry.owner));
    out.push_str(&format!("classification: {}\n", entry.classification));
    out.push_str(&format!("matched finding: {}\n", finding_location(finding)));
    if let Some(output) = output {
        out.push_str(&format!("output: {}\n", output.display()));
    } else {
        out.push_str("output: stdout\n");
    }
    out.push_str("claim boundary: generated policy entry requires human review before merge.\n");
    out
}

#[derive(Debug, Clone, Copy)]
struct AddContext<'a> {
    inventory_source: &'a str,
    source_tree_root: Option<&'a str>,
    inventory_files: Option<usize>,
}

impl Default for AddContext<'static> {
    fn default() -> Self {
        Self {
            inventory_source: "unknown",
            source_tree_root: None,
            inventory_files: None,
        }
    }
}

fn render_add_summary_json(
    entry: &AllowEntry,
    finding: &Finding,
    output: Option<&Path>,
    force: bool,
    context: AddContext<'_>,
) -> String {
    let policy_output = output.map(|path| path.display().to_string());
    let path = entry.path.as_ref().map(normalize_path);
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "  \"schema_version\": {},\n",
        allow_report::ADD_SCHEMA_VERSION
    ));
    out.push_str(&format!(
        "  \"schema_id\": \"{}\",\n",
        allow_report::ADD_SCHEMA_ID
    ));
    out.push_str("  \"tool\": \"cargo-allow\",\n");
    out.push_str("  \"command\": \"add\",\n");
    out.push_str(&format!(
        "  \"claim_boundary\": {},\n",
        allow_report::render_claim_boundary_json()
    ));
    out.push_str(&format!(
        "  \"scanner_limitations\": {},\n",
        allow_report::render_scanner_limitations_json()
    ));
    out.push_str("  \"inventory\": ");
    out.push_str(&allow_report::render_inventory_json(
        allow_report::InventoryContext::source_syntax(
            context.inventory_source,
            context.source_tree_root,
            context.inventory_files,
        ),
        "  ",
    ));
    out.push_str(",\n");
    out.push_str("  \"options\": {\n");
    out.push_str(&format!(
        "    \"policy_output\": {},\n",
        option_json_string(policy_output.as_deref())
    ));
    out.push_str(&format!("    \"force\": {}\n", force));
    out.push_str("  },\n");
    out.push_str("  \"summary\": {\n");
    out.push_str(&format!(
        "    \"entry_id\": \"{}\",\n",
        json_escape(&entry.id)
    ));
    out.push_str(&format!(
        "    \"selected_finding\": \"{}\",\n",
        json_escape(&finding_location(finding))
    ));
    out.push_str("    \"human_review_required\": true\n");
    out.push_str("  },\n");
    out.push_str("  \"allow_entry\": {\n");
    out.push_str(&format!("    \"id\": \"{}\",\n", json_escape(&entry.id)));
    out.push_str(&format!("    \"kind\": \"{}\",\n", entry.kind));
    out.push_str(&format!(
        "    \"family\": {},\n",
        option_json_string(entry.family.as_deref())
    ));
    out.push_str(&format!(
        "    \"path\": {},\n",
        option_json_string(path.as_deref())
    ));
    out.push_str(&format!(
        "    \"glob\": {},\n",
        option_json_string(entry.glob.as_deref())
    ));
    out.push_str(&format!(
        "    \"owner\": \"{}\",\n",
        json_escape(&entry.owner)
    ));
    out.push_str(&format!(
        "    \"classification\": \"{}\",\n",
        json_escape(&entry.classification)
    ));
    out.push_str(&format!(
        "    \"reason\": \"{}\",\n",
        json_escape(&entry.reason)
    ));
    out.push_str(&format!(
        "    \"review_after\": {},\n",
        option_json_string(entry.lifecycle.review_after.as_deref())
    ));
    out.push_str(&format!(
        "    \"expires\": {},\n",
        option_json_string(entry.lifecycle.expires.as_deref())
    ));
    out.push_str(&format!(
        "    \"evidence_count\": {},\n",
        entry.evidence.len()
    ));
    out.push_str("    \"selector\": ");
    out.push_str(&selector_json(&entry.selector, "    "));
    out.push_str(",\n");
    out.push_str("    \"last_seen\": ");
    out.push_str(&last_seen_json(entry.last_seen.as_ref(), "    "));
    out.push_str("\n  },\n");
    out.push_str("  \"selected_finding\": ");
    out.push_str(&explain_finding_json(finding, "selected", "  "));
    out.push_str("\n}\n");
    out
}

fn next_allow_id(cfg: &AllowConfig) -> String {
    let mut index = cfg.allow.len() + 1;
    loop {
        let candidate = format!("allow-{index:04}");
        if !cfg.allow.iter().any(|entry| entry.id == candidate) {
            return candidate;
        }
        index += 1;
    }
}

#[cfg(test)]
pub(crate) fn sample_add_json_for_contract_test() -> String {
    let add_finding = Finding {
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: PathBuf::from("src/lib.rs"),
        span: Some(allow_core::Span { line: 1, column: 1 }),
        identity: allow_core::StructuralIdentity::new("file", "method_call"),
        message: "test finding".to_string(),
    };
    let add_entry = allow_entry_from_finding(AddEntryRequest {
        finding: &add_finding,
        id: "allow-add-json".to_string(),
        owner: "parser".to_string(),
        classification: "reviewed_exception".to_string(),
        reason: "Parser validates the input before unwrapping.".to_string(),
        evidence: vec!["test:parser_validates_input".to_string()],
        review_after: "2026-11-01".to_string(),
        expires: None,
    });
    render_add_summary_json(
        &add_entry,
        &add_finding,
        Some(Path::new("policy/allow.proposed.toml")),
        false,
        AddContext {
            inventory_source: "git_tracked",
            source_tree_root: Some("H:/Code/Rust/cargo-allow"),
            inventory_files: Some(48),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CargoAllowCli, CargoAllowCommand};
    use allow_core::{Span, StructuralIdentity};
    use clap::Parser;
    use serde_json::Value;

    #[test]
    fn clap_parses_add_from_finding() {
        let parsed = CargoAllowCli::try_parse_from(argv(vec![
            "cargo-allow",
            "add",
            "--kind",
            "panic",
            "--path",
            "src/lib.rs",
            "--line",
            "42",
            "--owner",
            "parser",
            "--reason",
            "validated invariant",
            "--evidence",
            "test:parser_invariant",
            "--write",
            "policy/allow.proposed.toml",
            "--force",
            "--summary-format",
            "json",
            "--summary-output",
            "target/add-summary.json",
        ]))
        .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse: {err}")));

        assert!(matches!(
            parsed.command,
            Some(CargoAllowCommand::Add(AddArgs {
                kind,
                path,
                line: 42,
                owner,
                reason,
                evidence,
                write: Some(write),
                force: true,
                summary_format: AddSummaryFormat::Json,
                summary_output: Some(summary_output),
                ..
            })) if kind == "panic"
                && path == Path::new("src/lib.rs")
                && owner == "parser"
                && reason == "validated invariant"
                && evidence == vec!["test:parser_invariant".to_string()]
                && write == Path::new("policy/allow.proposed.toml")
                && summary_output == Path::new("target/add-summary.json")
        ));
    }

    #[test]
    fn select_add_finding_picks_nearest_path_and_kind() {
        let findings = vec![
            test_finding_at_line(
                FindingKind::Panic,
                Some("unwrap"),
                "src/lib.rs",
                "method_call",
                10,
            ),
            test_finding_at_line(
                FindingKind::Panic,
                Some("expect"),
                "src/lib.rs",
                "method_call",
                40,
            ),
            test_finding_at_line(
                FindingKind::Unsafe,
                Some("unsafe_fn"),
                "src/lib.rs",
                "unsafe_fn",
                39,
            ),
        ];
        let kind = parse_kind_filter("panic")
            .unwrap_or_else(|err| std::panic::panic_any(format!("kind should parse: {err}")));

        let (_index, selected) = select_add_finding(&findings, kind, Path::new("src/lib.rs"), 39)
            .unwrap_or_else(|err| std::panic::panic_any(format!("finding should select: {err}")));

        assert_eq!(selected.family.as_deref(), Some("expect"));
        assert_eq!(selected.span.as_ref().map(|span| span.line), Some(40));
    }

    #[test]
    fn select_add_finding_fails_closed_on_equal_nearest_findings() {
        let findings = vec![
            test_finding_at_line(
                FindingKind::Panic,
                Some("unwrap"),
                "src/lib.rs",
                "method_call",
                40,
            ),
            test_finding_at_line(
                FindingKind::Panic,
                Some("expect"),
                "src/lib.rs",
                "method_call",
                42,
            ),
        ];
        let kind = parse_kind_filter("panic")
            .unwrap_or_else(|err| std::panic::panic_any(format!("kind should parse: {err}")));

        let err = select_add_finding(&findings, kind, Path::new("src/lib.rs"), 41)
            .expect_err("equally near findings should be ambiguous");

        assert!(err.to_string().contains("ambiguous add request"));
    }

    #[test]
    fn ensure_addable_outcome_rejects_already_matched_findings() {
        assert!(ensure_addable_outcome(MatchStatus::New).is_ok());

        let err = ensure_addable_outcome(MatchStatus::Matched)
            .expect_err("matched finding should not be addable");

        assert!(err.to_string().contains("already receipted"));
    }

    #[test]
    fn allow_entry_from_finding_uses_structural_selector_and_review_metadata() {
        let mut finding = test_finding_at_line(
            FindingKind::Panic,
            Some("unwrap"),
            "src/lib.rs",
            "method_call",
            42,
        );
        finding.identity.container = Some("parse_span".to_string());
        finding.identity.callee = Some("unwrap".to_string());
        finding.identity.normalized_snippet_hash = Some("fnv1a64:1234".to_string());

        let entry = allow_entry_from_finding(AddEntryRequest {
            finding: &finding,
            id: "allow-0099".to_string(),
            owner: "parser".to_string(),
            classification: "validated_invariant".to_string(),
            reason: "Parser validates the span before unwrapping.".to_string(),
            evidence: vec!["test:parser_validates_span".to_string()],
            review_after: "2026-11-01".to_string(),
            expires: None,
        });

        assert_eq!(entry.id, "allow-0099");
        assert_eq!(entry.owner, "parser");
        assert_eq!(entry.selector.container.as_deref(), Some("parse_span"));
        assert_eq!(entry.selector.callee.as_deref(), Some("unwrap"));
        assert_eq!(
            entry.selector.normalized_snippet_hash.as_deref(),
            Some("fnv1a64:1234")
        );
        assert_eq!(entry.last_seen.as_ref().map(|last| last.line), Some(42));
    }

    #[test]
    fn render_add_summary_json_records_entry_and_selected_finding() {
        let mut finding = test_finding_at_line(
            FindingKind::Panic,
            Some("unwrap"),
            "src/lib.rs",
            "method_call",
            42,
        );
        finding.identity.crate_name = Some("parser".to_string());
        finding.identity.container = Some("parse_span".to_string());
        finding.identity.callee = Some("unwrap".to_string());
        let mut entry = allow_entry_from_finding(AddEntryRequest {
            finding: &finding,
            id: "allow-0101".to_string(),
            owner: "parser".to_string(),
            classification: "validated_invariant".to_string(),
            reason: "Parser validates the span before unwrapping.".to_string(),
            evidence: vec!["test:parser_validates_span".to_string()],
            review_after: "2026-11-01".to_string(),
            expires: Some("2027-01-01".to_string()),
        });
        entry.selector.normalized_snippet_hash = Some("fnv1a64:1234".to_string());

        let json = render_add_summary_json(
            &entry,
            &finding,
            Some(Path::new("policy/allow.proposed.toml")),
            true,
            AddContext {
                inventory_source: "git_tracked",
                source_tree_root: Some("H:/Code/Rust/cargo-allow"),
                inventory_files: Some(52),
            },
        );
        let value = parse_json_artifact("add", &json, allow_report::ADD_SCHEMA_ID, "add");

        assert_inventory_contract(
            "add",
            &value,
            "git_tracked",
            Some("H:/Code/Rust/cargo-allow"),
            Some(52),
        );
        assert_eq!(
            value
                .pointer("/options/policy_output")
                .and_then(Value::as_str),
            Some("policy/allow.proposed.toml"),
            "add policy output"
        );
        assert_eq!(
            value.pointer("/options/force").and_then(Value::as_bool),
            Some(true),
            "add force"
        );
        assert_eq!(
            value.pointer("/summary/entry_id").and_then(Value::as_str),
            Some("allow-0101"),
            "add summary entry id"
        );
        assert_eq!(
            value
                .pointer("/summary/human_review_required")
                .and_then(Value::as_bool),
            Some(true),
            "add human_review_required"
        );
        assert_eq!(
            value.pointer("/allow_entry/id").and_then(Value::as_str),
            Some("allow-0101"),
            "add allow id"
        );
        assert_eq!(
            value
                .pointer("/allow_entry/evidence_count")
                .and_then(Value::as_u64),
            Some(1),
            "add evidence count"
        );
        assert_eq!(
            value
                .pointer("/selected_finding/source_package")
                .and_then(Value::as_str),
            Some("parser"),
            "add selected finding source package"
        );
    }

    #[test]
    fn add_schema_documents_current_contract() {
        let schema = include_str!("../../../docs/schemas/add.schema.json");

        assert!(schema.contains(allow_report::ADD_SCHEMA_ID));
        assert!(schema.contains("\"options\""));
        assert!(schema.contains("\"policy_output\""));
        assert!(schema.contains("\"allow_entry\""));
        assert!(schema.contains("\"selected_finding\""));
        assert!(schema.contains("\"human_review_required\""));
        assert!(schema.contains("\"scanner_limitations\""));
        assert!(schema.contains("\"scanner_limitation\""));
        assert!(schema.contains("\"cargo_metadata_not_invoked\""));
        assert!(schema.contains("\"repository_code_not_executed\""));
    }

    fn parse_json_artifact(
        name: &str,
        json: &str,
        expected_schema: &str,
        expected_command: &str,
    ) -> Value {
        let value: Value = serde_json::from_str(json)
            .unwrap_or_else(|err| std::panic::panic_any(format!("{name} json: {err}\n{json}")));
        assert_eq!(
            value.pointer("/schema_id").and_then(Value::as_str),
            Some(expected_schema),
            "{name} schema id"
        );
        assert_eq!(
            value.pointer("/command").and_then(Value::as_str),
            Some(expected_command),
            "{name} command"
        );
        value
    }

    fn assert_inventory_contract(
        name: &str,
        value: &Value,
        expected_source: &str,
        expected_root: Option<&str>,
        expected_files: Option<u64>,
    ) {
        assert_eq!(
            value.pointer("/inventory/scope").and_then(Value::as_str),
            Some("source_tree"),
            "{name} inventory scope"
        );
        assert_eq!(
            value.pointer("/inventory/scanner").and_then(Value::as_str),
            Some("source_syntax"),
            "{name} inventory scanner"
        );
        assert_eq!(
            value.pointer("/inventory/source").and_then(Value::as_str),
            Some(expected_source),
            "{name} inventory source"
        );
        assert_eq!(
            value.pointer("/inventory/root").and_then(Value::as_str),
            expected_root,
            "{name} inventory root"
        );
        assert_eq!(
            value
                .pointer("/inventory/files_scanned")
                .and_then(Value::as_u64),
            expected_files,
            "{name} inventory files"
        );
    }

    fn test_finding_at_line(
        kind: FindingKind,
        family: Option<&str>,
        path: &str,
        ast_kind: &str,
        line: u32,
    ) -> Finding {
        Finding {
            kind,
            family: family.map(str::to_string),
            path: PathBuf::from(path),
            span: Some(Span { line, column: 1 }),
            identity: StructuralIdentity::new("file", ast_kind),
            message: "test finding".to_string(),
        }
    }

    fn argv(items: Vec<&str>) -> Vec<String> {
        items.into_iter().map(String::from).collect()
    }
}
