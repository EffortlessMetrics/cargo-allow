use allow_core::{CargoAllowError, CargoAllowResult};
use allow_match::{CheckMode, evaluate};
use allow_policy::{render_policy, validate_policy};

#[path = "refresh_args.rs"]
mod refresh_args;
#[path = "refresh_render.rs"]
mod refresh_render;
#[path = "refresh_select.rs"]
mod refresh_select;
#[path = "refresh_types.rs"]
mod refresh_types;
use allow_report::MutationReceipt;
pub(crate) use refresh_args::RefreshArgs;
use refresh_render::{render_refresh_json, render_refresh_result_styled};
use refresh_select::{apply_last_seen_refresh, select_location_drift_refresh};
use refresh_types::{RefreshContext, RefreshEmitInput, RefreshRenderInput};

use crate::{
    EvidenceValidationMode, HumanJsonFormat, MutationLock, SourceTreeReportContext, config_path,
    emit_text,
    evidence_inventory::{
        current_evidence_source_tree_files, validate_evidence_references_for_source_tree,
    },
    git_relative_config_path, load_world_with_evidence_mode, resolve_source_tree_root,
};
use repo_edit::{SingleTargetApplyMode, SingleTargetApplyRequest, apply_single_target};

pub(crate) fn cmd_refresh(args: &RefreshArgs) -> CargoAllowResult<()> {
    if args.dry_run && args.write {
        return Err(CargoAllowError::new(
            "pass either --dry-run or --write, not both",
        ));
    }
    let mutation_lock = if args.write {
        let cwd = std::env::current_dir()
            .map_err(|error| CargoAllowError::new(format!("failed to read cwd: {error}")))?;
        let root = resolve_source_tree_root(args.root.root.as_deref(), cwd)?;
        let path = config_path(&root, args.config.as_deref()).ok_or_else(|| {
            CargoAllowError::new("no policy config found; run `cargo-allow init` or pass --config")
        })?;
        crate::policy_config::assert_path_within_root(&root, &path)?;
        Some(MutationLock::acquire(path)?)
    } else {
        None
    };
    let (root, mut cfg, findings, inventory_facts, _federation) = load_world_with_evidence_mode(
        args.root.root.as_deref(),
        args.config.as_deref(),
        true,
        None,
        args.include_untracked,
        EvidenceValidationMode::ReportOnly,
    )?;
    let _mutation_lock = mutation_lock;
    let outcomes = evaluate(&cfg, &findings, CheckMode::NoNew);
    let (entry_index, finding_index, drift_message) =
        select_location_drift_refresh(&cfg, &outcomes, &findings, &args.allow_id)?;
    let finding = findings.get(finding_index).ok_or_else(|| {
        CargoAllowError::new("internal error: selected finding index out of range")
    })?;
    let policy_path = config_path(&root, args.config.as_deref()).ok_or_else(|| {
        CargoAllowError::new("no policy config found; run `cargo-allow init` or pass --config")
    })?;
    let original_entry = cfg
        .allow
        .get(entry_index)
        .ok_or_else(|| {
            CargoAllowError::new("internal error: selected allow entry index out of range")
        })?
        .clone();
    let previous_last_seen = cfg
        .allow
        .get(entry_index)
        .ok_or_else(|| {
            CargoAllowError::new("internal error: selected allow entry index out of range")
        })?
        .last_seen
        .clone();
    let mut preview_entry = cfg
        .allow
        .get(entry_index)
        .ok_or_else(|| {
            CargoAllowError::new("internal error: selected allow entry index out of range")
        })?
        .clone();
    apply_last_seen_refresh(&mut preview_entry, finding);
    let written_path = if args.write {
        let entry = cfg.allow.get_mut(entry_index).ok_or_else(|| {
            CargoAllowError::new("internal error: selected allow entry index out of range")
        })?;
        apply_last_seen_refresh(entry, finding);
        validate_policy(&cfg)?;
        let evidence_source_tree_files =
            current_evidence_source_tree_files(&root, args.include_untracked);
        validate_evidence_references_for_source_tree(
            &root,
            &cfg,
            evidence_source_tree_files.as_ref(),
        )?;
        let policy_target = git_relative_config_path(&root, args.config.as_deref())?;
        let rendered = render_policy(&cfg);
        apply_single_target(SingleTargetApplyRequest {
            repository_root: &root,
            target: &policy_target,
            contents: &rendered,
            caller_reference: Some("cargo-allow:refresh"),
            lock_identity: Some(
                policy_target
                    .to_string_lossy()
                    .replace(std::path::MAIN_SEPARATOR, "/"),
            ),
            mode: SingleTargetApplyMode::AtomicReplace,
        })
        .into_result()?;
        Some(policy_path.clone())
    } else {
        None
    };
    let entry_for_render = if args.write {
        cfg.allow.get(entry_index).ok_or_else(|| {
            CargoAllowError::new("internal error: selected allow entry index out of range")
        })?
    } else {
        &preview_entry
    };
    let repo_root = root.display().to_string();
    let config_source = policy_path.display().to_string();
    let mutation_receipt = MutationReceipt {
        operation: "refresh",
        tool_version: env!("CARGO_PKG_VERSION"),
        repo_root: Some(&repo_root),
        config_source: Some(&config_source),
        ledger_ids: Vec::new(),
        changed_allow_ids: vec![original_entry.id.as_str()],
        before_fingerprints: vec![Some(allow_core::allow_entry_content_fingerprint(
            &original_entry,
        ))],
        after_fingerprints: vec![Some(allow_core::allow_entry_content_fingerprint(
            &preview_entry,
        ))],
        result: if args.write { "written" } else { "stdout" },
        next_commands: vec![
            format!("cargo-allow explain {}", original_entry.id),
            "cargo-allow check --mode no-new".to_string(),
        ],
    };
    render_and_emit(
        args,
        RefreshEmitInput {
            entry: entry_for_render,
            finding,
            previous_last_seen,
            drift_message: &drift_message,
            root: &root,
            inventory_facts,
            written_path: written_path.as_deref(),
            mutation_receipt,
        },
    )
}

fn render_and_emit(args: &RefreshArgs, input: RefreshEmitInput<'_>) -> CargoAllowResult<()> {
    let source_context = SourceTreeReportContext::new(input.root, input.inventory_facts);
    let context = RefreshContext {
        inventory: source_context.inventory(),
    };
    let written = input.written_path.map(|path| path.display().to_string());
    let written_ref = written.as_deref();
    let render_input = RefreshRenderInput {
        entry: input.entry,
        finding: input.finding,
        previous_last_seen: input.previous_last_seen,
        drift_message: input.drift_message,
        dry_run: args.dry_run,
        write_requested: args.write,
        written_path: written_ref,
        context,
        mutation_receipt: input.mutation_receipt,
    };
    let text = match args.format {
        HumanJsonFormat::Human => {
            let style = if args.output.is_none() {
                crate::reporting::output_style()
            } else {
                allow_report::Style::PLAIN
            };
            render_refresh_result_styled(render_input, style)
        }
        HumanJsonFormat::Json => render_refresh_json(render_input),
    };
    emit_text(args.output.as_deref(), &text)?;
    Ok(())
}

#[cfg(test)]
pub(crate) fn sample_refresh_json_for_contract_test() -> String {
    use allow_core::{Finding, FindingKind, LastSeen, Selector, Span, StructuralIdentity};
    let entry = allow_core::AllowEntry {
        id: "allow-0250".to_string(),
        kind: FindingKind::LintException,
        family: Some("expect".to_string()),
        path: Some("src/lib.rs".into()),
        glob: None,
        owner: "lint".to_string(),
        classification: "reviewed_lint_exception".to_string(),
        reason: "Fixture refresh receipt".to_string(),
        evidence: vec!["test:refresh-receipt-fixture".to_string()],
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: allow_core::Lifecycle {
            created: Some("2026-05-09".to_string()),
            review_after: Some("2026-09-09".to_string()),
            expires: None,
        },
        selector: Selector::default(),
        last_seen: Some(LastSeen {
            line: 22,
            column: 4,
        }),
    };
    let finding = Finding {
        kind: FindingKind::LintException,
        family: Some("expect".to_string()),
        path: "src/lib.rs".into(),
        identity: StructuralIdentity::new("rust", "attribute"),
        message: "fixture refresh drift".to_string(),
        ledger: None,
        span: Some(Span {
            line: 22,
            column: 4,
        }),
    };
    render_refresh_json(RefreshRenderInput {
        entry: &entry,
        finding: &finding,
        previous_last_seen: Some(LastSeen {
            line: 14,
            column: 8,
        }),
        drift_message: "allow-drift last_seen changed from 14:8 to 22:4",
        dry_run: true,
        write_requested: false,
        written_path: None,
        context: RefreshContext {
            inventory: allow_report::InventoryContext::source_syntax(
                "filesystem_include_untracked",
                Some("tests/fixtures/refresh/advisory-drift"),
                Some(2),
            ),
        },
        mutation_receipt: allow_report::MutationReceipt {
            operation: "refresh",
            tool_version: env!("CARGO_PKG_VERSION"),
            repo_root: Some("tests/fixtures/refresh/advisory-drift"),
            config_source: Some("policy/allow.toml"),
            ledger_ids: Vec::new(),
            changed_allow_ids: vec!["allow-0250"],
            before_fingerprints: vec![Some(
                "sha256:v1:0000000000000000000000000000000000000000000000000000000000000000"
                    .to_string(),
            )],
            after_fingerprints: vec![Some(
                "sha256:v1:1111111111111111111111111111111111111111111111111111111111111111"
                    .to_string(),
            )],
            result: "stdout",
            next_commands: vec!["cargo-allow check --mode no-new".to_string()],
        },
    })
}

#[cfg(test)]
#[path = "refresh_tests.rs"]
mod tests;
