use allow_core::{CargoAllowError, CargoAllowResult, FindingKind};
use allow_match::{CheckMode, evaluate};
use allow_policy::{render_policy, validate_local_evidence_references, validate_policy};

#[path = "add_args.rs"]
mod add_args;
#[path = "add_entry.rs"]
mod add_entry;
#[path = "add_render.rs"]
mod add_render;
#[path = "add_types.rs"]
mod add_types;
pub(crate) use add_args::AddArgs;
use add_args::AddSummaryFormat;
use add_entry::{
    AddEntryRequest, allow_entry_from_finding, ensure_addable_outcome, next_allow_id,
    select_add_finding,
};
use add_render::{render_add_summary, render_add_summary_json};
pub(super) use add_types::AddContext;

use crate::{
    SourceTreeReportContext, load_world, parse_kind_filter, write_file, write_file_no_overwrite,
};

#[cfg(test)]
use allow_core::{Finding, MatchStatus};
#[cfg(test)]
use std::path::{Path, PathBuf};

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
    let source_context = SourceTreeReportContext::new(&root, inventory_facts);
    let context = AddContext {
        inventory: source_context.inventory(),
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
            inventory: allow_report::InventoryContext::source_syntax(
                "git_tracked",
                Some("H:/Code/Rust/cargo-allow"),
                Some(48),
            ),
        },
    )
}

#[cfg(test)]
#[path = "add_artifact_tests.rs"]
mod artifact_tests;
#[cfg(test)]
#[path = "add_test_support.rs"]
mod test_support;
#[cfg(test)]
#[path = "add_tests.rs"]
mod tests;
