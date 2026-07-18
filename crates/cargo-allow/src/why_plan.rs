use std::collections::BTreeMap;
use std::path::Path;

use allow_core::{
    AllowConfig, CargoAllowError, CargoAllowResult, Finding, MatchOutcome, MatchStatus,
    finding_identity_key, normalize_path, read_file_capped, sha256_v1_bytes,
};
use allow_report::{
    AddFindingPlanCandidate, AddFindingPlanFinding, AddFindingPlanOutcome, AddFindingPlanPolicy,
    AddFindingPlanProofPlan, AddFindingPlanRepository, AddFindingPlanV1,
};
use serde_json::{Value, json};

use super::why_render::{WhyCandidate, why_next_steps};
use crate::{SourceTreeReportContext, config_path, selector::selector_from_finding};

pub(super) struct AddFindingPlanInput<'a> {
    pub root: &'a Path,
    pub config: Option<&'a Path>,
    pub cfg: &'a AllowConfig,
    pub include_untracked: bool,
    pub source_context: &'a SourceTreeReportContext,
    pub finding: &'a Finding,
    pub outcome: &'a MatchOutcome,
    pub candidates: &'a [WhyCandidate<'a>],
}

pub(super) fn render_add_finding_plan(input: AddFindingPlanInput<'_>) -> CargoAllowResult<String> {
    let AddFindingPlanInput {
        root,
        config,
        cfg,
        include_untracked,
        source_context,
        finding,
        outcome,
        candidates,
    } = input;
    if outcome.status != MatchStatus::New {
        return Err(CargoAllowError::new(format!(
            "cannot produce an add-finding plan for status `{}`; use ordinary `why --format json` or `explain` for diagnosis",
            outcome.status.as_str()
        )));
    }

    let policy_path = config_path(root, config).ok_or_else(|| {
        CargoAllowError::new("no policy config found for add-finding plan; run `cargo-allow init`")
    })?;
    let policy_bytes = read_bound_file(&policy_path, "policy")?;
    let relative_policy = crate::policy_config::git_relative_config_path(root, Some(&policy_path))?;
    let source_path = root.join(&finding.path);
    let source_bytes = read_bound_file(&source_path, "source file")?;
    let finding_key = finding_identity_key(finding);
    let selector = selector_from_finding(finding);
    let inventory = source_context.inventory();
    let inventory_basis_identity = inventory_identity(root, cfg, include_untracked)?;
    let root_text = source_context.source_tree_root().to_string();
    let repository_identity = sha256_v1_bytes(
        format!(
            "cargo-allow.repository.v1\n{}\n{}",
            inventory_basis_identity,
            sha256_v1_bytes(&policy_bytes)
        )
        .as_bytes(),
    );

    let plan = AddFindingPlanV1 {
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        repository: AddFindingPlanRepository {
            identity: repository_identity,
            root: root_text,
        },
        inventory,
        inventory_basis_identity,
        policy: AddFindingPlanPolicy {
            path: normalize_path(&relative_policy),
            digest: sha256_v1_bytes(&policy_bytes),
        },
        finding: AddFindingPlanFinding {
            kind: finding.kind.as_str().to_string(),
            family: finding.family.clone(),
            path: normalize_path(&finding.path),
            line: finding.span.as_ref().map(|span| span.line as usize),
            column: finding.span.as_ref().map(|span| span.column as usize),
            identity: identity_values(finding),
            digest: sha256_v1_bytes(finding_key.as_bytes()),
            source_file_digest: sha256_v1_bytes(&source_bytes),
            selector: selector_values(&selector),
        },
        outcome: AddFindingPlanOutcome {
            status: outcome.status.as_str().to_string(),
            allow_id: outcome.allow_id.clone(),
            message: outcome.message.clone(),
        },
        candidates: candidates
            .iter()
            .map(|candidate| AddFindingPlanCandidate {
                allow_id: candidate.entry.id.clone(),
                mismatch_reasons: candidate.reasons.clone(),
            })
            .collect(),
        required_fields: required_human_fields(cfg, finding),
        proof_plans: why_next_steps(finding, outcome, candidates)
            .proof_plans
            .into_iter()
            .map(|proof| AddFindingPlanProofPlan {
                program: proof.program,
                args: proof.args,
            })
            .collect(),
    };
    Ok(allow_report::render_add_finding_plan_json(&plan))
}

fn required_human_fields(cfg: &AllowConfig, finding: &Finding) -> Vec<String> {
    let requirements = &cfg.requirements;
    let mut fields = Vec::new();
    if requirements.owner_required {
        fields.push("owner".to_string());
    }
    if requirements.classification_required {
        fields.push("classification".to_string());
    }
    if requirements.reason_required {
        fields.push("reason".to_string());
    }
    if requirements.evidence_required
        || (finding.kind == allow_core::FindingKind::Unsafe
            && requirements.unsafe_evidence_required)
    {
        fields.push("evidence".to_string());
    }
    if requirements.expires_or_review_after_required {
        fields.push("review_after_or_expires".to_string());
    }
    fields
}

fn read_bound_file(path: &Path, label: &str) -> CargoAllowResult<Vec<u8>> {
    read_file_capped(path).map_err(|error| {
        CargoAllowError::new(format!(
            "failed to read {label} {} for add-finding plan: {error}",
            path.display()
        ))
    })
}

fn inventory_identity(
    root: &Path,
    cfg: &AllowConfig,
    include_untracked: bool,
) -> CargoAllowResult<String> {
    let options = allow_inventory::InventoryOptions {
        ignored: cfg.workspace.ignored.clone(),
        generated: cfg.workspace.generated.clone(),
        include_untracked,
    };
    let mut inventory = allow_inventory::inventory(root, &options)?;
    inventory.files.sort_by_key(|path| normalize_path(path));
    let mut canonical = Vec::new();
    push_bound_value(&mut canonical, "cargo-allow.inventory-basis.v1");
    push_bound_value(&mut canonical, inventory.source.as_str());
    push_bound_value(&mut canonical, inventory.completeness.as_str());
    for path in &inventory.files {
        let relative = path.strip_prefix(root).unwrap_or(path);
        push_bound_value(&mut canonical, &normalize_path(relative));
        let source_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            root.join(path)
        };
        let bytes = read_bound_file(&source_path, "inventory file")?;
        push_bound_value(&mut canonical, &sha256_v1_bytes(&bytes));
    }
    for path in &inventory.deleted_tracked {
        push_bound_value(&mut canonical, &format!("deleted:{}", normalize_path(path)));
    }
    for path in &inventory.skipped_paths {
        push_bound_value(&mut canonical, &format!("skipped:{}", normalize_path(path)));
    }
    for path in &inventory.submodule_paths {
        push_bound_value(
            &mut canonical,
            &format!("submodule:{}", normalize_path(path)),
        );
    }
    Ok(sha256_v1_bytes(&canonical))
}

fn push_bound_value(output: &mut Vec<u8>, value: &str) {
    output.extend_from_slice(&(value.len() as u64).to_be_bytes());
    output.extend_from_slice(value.as_bytes());
}

fn identity_values(finding: &Finding) -> BTreeMap<String, Value> {
    let identity = &finding.identity;
    BTreeMap::from([
        ("language".to_string(), json!(identity.language)),
        ("crate_name".to_string(), json!(identity.crate_name)),
        ("module".to_string(), json!(identity.module)),
        ("container".to_string(), json!(identity.container)),
        ("ast_kind".to_string(), json!(identity.ast_kind)),
        ("symbol".to_string(), json!(identity.symbol)),
        ("callee".to_string(), json!(identity.callee)),
        ("macro_name".to_string(), json!(identity.macro_name)),
        ("lint".to_string(), json!(identity.lint)),
        (
            "receiver_fingerprint".to_string(),
            json!(identity.receiver_fingerprint),
        ),
        (
            "target_fingerprint".to_string(),
            json!(identity.target_fingerprint),
        ),
        (
            "normalized_snippet_hash".to_string(),
            json!(identity.normalized_snippet_hash),
        ),
        ("line_hint".to_string(), json!(identity.line_hint)),
        ("column_hint".to_string(), json!(identity.column_hint)),
    ])
}

fn selector_values(selector: &allow_core::Selector) -> BTreeMap<String, Value> {
    BTreeMap::from([
        ("ast_kind".to_string(), json!(selector.ast_kind)),
        ("container".to_string(), json!(selector.container)),
        ("callee".to_string(), json!(selector.callee)),
        ("macro_name".to_string(), json!(selector.macro_name)),
        ("lint".to_string(), json!(selector.lint)),
        ("symbol".to_string(), json!(selector.symbol)),
        (
            "receiver_fingerprint".to_string(),
            json!(selector.receiver_fingerprint),
        ),
        (
            "target_fingerprint".to_string(),
            json!(selector.target_fingerprint),
        ),
        (
            "normalized_snippet_hash".to_string(),
            json!(selector.normalized_snippet_hash),
        ),
        ("line_hint".to_string(), json!(selector.line_hint)),
        ("glob".to_string(), json!(selector.glob)),
    ])
}

#[cfg(test)]
pub(crate) fn sample_add_finding_plan_json_for_contract_test() -> String {
    use allow_core::{FindingKind, Span, StructuralIdentity};

    let mut structural = StructuralIdentity::new("rust", "method_call");
    structural.container = Some("load".to_string());
    structural.callee = Some("unwrap".to_string());
    structural.line_hint = Some(10);
    structural.column_hint = Some(5);
    let finding = Finding {
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: "src/lib.rs".into(),
        span: Some(Span {
            line: 10,
            column: 5,
        }),
        identity: structural,
        message: "unwrap call".to_string(),
        ledger: None,
    };
    let digest = "sha256:v1:0000000000000000000000000000000000000000000000000000000000000000";
    let plan = AddFindingPlanV1 {
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        repository: AddFindingPlanRepository {
            identity: digest.to_string(),
            root: "H:/repo".to_string(),
        },
        inventory: allow_report::InventoryContext::source_syntax(
            "git_tracked",
            Some("H:/repo"),
            Some(3),
        )
        .with_completeness("complete"),
        inventory_basis_identity: digest.to_string(),
        policy: AddFindingPlanPolicy {
            path: "policy/allow.toml".to_string(),
            digest: digest.to_string(),
        },
        finding: AddFindingPlanFinding {
            kind: finding.kind.as_str().to_string(),
            family: finding.family.clone(),
            path: normalize_path(&finding.path),
            line: Some(10),
            column: Some(5),
            identity: identity_values(&finding),
            digest: digest.to_string(),
            source_file_digest: digest.to_string(),
            selector: selector_values(&selector_from_finding(&finding)),
        },
        outcome: AddFindingPlanOutcome {
            status: "new".to_string(),
            allow_id: None,
            message: "unreceipted panic.unwrap".to_string(),
        },
        candidates: vec![AddFindingPlanCandidate {
            allow_id: "allow-near".to_string(),
            mismatch_reasons: vec!["callee mismatch".to_string()],
        }],
        required_fields: vec!["owner".to_string(), "reason".to_string()],
        proof_plans: vec![AddFindingPlanProofPlan {
            program: "cargo-allow".to_string(),
            args: vec![
                "check".to_string(),
                "--mode".to_string(),
                "no-new".to_string(),
            ],
        }],
    };
    allow_report::render_add_finding_plan_json(&plan)
}
