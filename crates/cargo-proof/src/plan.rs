//! Proof plan command wired to proof-engine (#2589-B).
//!
//! The plan CLI consumes intent-protocol obligation input via the intent
//! planner entry point (#3310/#3312). The legacy proof-owned obligation
//! path has been deleted (#3314) and is guarded against reintroduction
//! (#3317).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use proof_engine::{
    CapturedReceiptStoreV1, IntentPlannerError, ProviderRegistryV1, intent_obligation_plan_digest,
    plan_proof_execution_from_intent, plan_proof_v2_from_intent,
};

/// The provider this product intends to select once the feature-gated
/// registry lands (#2938). Named in every unavailable result so output
/// states exactly what is missing; never constructed as a fake.
pub const INTENDED_PROVIDER_ID: &str = "cargo-allow";
use proof_protocol::{
    PROOF_PLAN_SCHEMA_ID, ProofCapabilityCatalogV1, ProofPlanV1, ProofPlanV2, ProofResultStateV1,
};

use crate::render::{OutputFormat, PlanFrameV1, emit_frame};

pub const PLAN_FRAME_SCHEMA_ID: &str = "cargo-proof.plan-frame.v1";
pub const PLAN_CLAIM_BOUNDARY: &str =
    "Obligation-to-proof-plan projection only; process execution remains caller-owned.";

/// Plan outcome binding the exact intent plan identity (#3316).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanOutcomeV1 {
    pub plan: ProofPlanV1,
    pub intent_plan_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanV2OutcomeV1 {
    pub plan: ProofPlanV2,
    pub output: String,
}

/// Plan failure carrying the proof-corpus result class so the CLI maps
/// the exit family from the vocabulary instead of treating every error
/// as usage (#3598 exit-family follow-up).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanErrorV1 {
    pub result_state: ProofResultStateV1,
    pub message: String,
}

impl std::fmt::Display for PlanErrorV1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

/// Plan proof execution from an intent obligation plan file (JSON).
pub fn plan_from_obligation_path(path: &Path) -> Result<PlanOutcomeV1, PlanErrorV1> {
    let text = std::fs::read_to_string(path).map_err(|err| PlanErrorV1 {
        result_state: ProofResultStateV1::Missing,
        message: format!("read {}: {err}", path.display()),
    })?;
    let envelope: intent_protocol::IntentObligationPlanEnvelopeV1 = serde_json::from_str(&text)
        .map_err(|err| PlanErrorV1 {
            result_state: ProofResultStateV1::Missing,
            message: format!("parse intent envelope JSON: {err}"),
        })?;
    plan_from_intent_envelope(&envelope)
}

/// Generate and retain the complete deterministic `proof.plan.v2` artifact.
pub fn plan_v2_from_paths(
    obligation_path: &Path,
    catalog_path: &Path,
    receipt_path: &Path,
    output_path: &Path,
) -> Result<PlanV2OutcomeV1, PlanErrorV1> {
    let envelope_text = std::fs::read_to_string(obligation_path).map_err(|err| PlanErrorV1 {
        result_state: ProofResultStateV1::Missing,
        message: format!("read {}: {err}", obligation_path.display()),
    })?;
    let envelope: intent_protocol::IntentObligationPlanEnvelopeV1 =
        serde_json::from_str(&envelope_text).map_err(|err| PlanErrorV1 {
            result_state: ProofResultStateV1::Missing,
            message: format!("parse intent envelope JSON: {err}"),
        })?;
    let catalog_text = std::fs::read_to_string(catalog_path).map_err(|err| PlanErrorV1 {
        result_state: ProofResultStateV1::Missing,
        message: format!("read {}: {err}", catalog_path.display()),
    })?;
    let catalogs: Vec<ProofCapabilityCatalogV1> =
        serde_json::from_str(&catalog_text).map_err(|err| PlanErrorV1 {
            result_state: ProofResultStateV1::Missing,
            message: format!("parse provider catalog JSON: {err}"),
        })?;
    let receipt_text = std::fs::read_to_string(receipt_path).map_err(|err| PlanErrorV1 {
        result_state: ProofResultStateV1::Missing,
        message: format!("read {}: {err}", receipt_path.display()),
    })?;
    let receipts: CapturedReceiptStoreV1 =
        serde_json::from_str(&receipt_text).map_err(|err| PlanErrorV1 {
            result_state: ProofResultStateV1::Missing,
            message: format!("parse receipt inventory JSON: {err}"),
        })?;
    let plan = plan_proof_v2_from_intent(&envelope, &catalogs, &receipts).map_err(|message| {
        PlanErrorV1 {
            result_state: ProofResultStateV1::Unsupported,
            message: format!("generate proof.plan.v2: {message}"),
        }
    })?;
    write_plan_artifact(&plan, output_path)?;
    Ok(PlanV2OutcomeV1 {
        plan,
        output: output_path.display().to_string(),
    })
}

/// Generate a V2 plan using the compile-time selected provider registry.
/// An explicitly supplied catalog remains supported for reproducible replay;
/// this path is the normal product route and preserves an empty-registry
/// `ProviderUnavailable` result when no provider feature is enabled.
pub fn plan_v2_from_selected_registry(
    obligation_path: &Path,
    receipt_path: &Path,
    output_path: &Path,
) -> Result<PlanV2OutcomeV1, PlanErrorV1> {
    let registry =
        crate::providers::StaticProviderRegistryV1::selected().map_err(|error| PlanErrorV1 {
            result_state: ProofResultStateV1::Unsupported,
            message: format!("select provider registry: {}", error.as_str()),
        })?;
    let envelope_text = std::fs::read_to_string(obligation_path).map_err(|err| PlanErrorV1 {
        result_state: ProofResultStateV1::Missing,
        message: format!("read {}: {err}", obligation_path.display()),
    })?;
    let envelope: intent_protocol::IntentObligationPlanEnvelopeV1 =
        serde_json::from_str(&envelope_text).map_err(|err| PlanErrorV1 {
            result_state: ProofResultStateV1::Missing,
            message: format!("parse intent envelope JSON: {err}"),
        })?;
    let receipt_text = std::fs::read_to_string(receipt_path).map_err(|err| PlanErrorV1 {
        result_state: ProofResultStateV1::Missing,
        message: format!("read {}: {err}", receipt_path.display()),
    })?;
    let receipts: CapturedReceiptStoreV1 =
        serde_json::from_str(&receipt_text).map_err(|err| PlanErrorV1 {
            result_state: ProofResultStateV1::Missing,
            message: format!("parse receipt inventory JSON: {err}"),
        })?;
    let plan = plan_proof_v2_from_intent(&envelope, &registry.catalogs(), &receipts).map_err(
        |message| PlanErrorV1 {
            result_state: ProofResultStateV1::Unsupported,
            message: format!("generate proof.plan.v2: {message}"),
        },
    )?;
    write_plan_artifact(&plan, output_path)?;
    Ok(PlanV2OutcomeV1 {
        plan,
        output: output_path.display().to_string(),
    })
}

static PLAN_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn write_plan_artifact(plan: &ProofPlanV2, output_path: &Path) -> Result<(), PlanErrorV1> {
    let serialized = serde_json::to_string_pretty(plan).map_err(|err| PlanErrorV1 {
        result_state: ProofResultStateV1::Unsupported,
        message: format!("serialize proof.plan.v2: {err}"),
    })?;
    let parent = output_path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = output_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("proof-plan.json");
    let mut temporary = PathBuf::new();
    let mut file = None;
    for _ in 0..128 {
        let sequence = PLAN_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!(
            ".{file_name}.{}.{}.tmp",
            std::process::id(),
            sequence
        ));
        match std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&candidate)
        {
            Ok(handle) => {
                temporary = candidate;
                file = Some(handle);
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(PlanErrorV1 {
                    result_state: ProofResultStateV1::Unsupported,
                    message: format!("create temporary plan artifact: {error}"),
                });
            }
        }
    }
    let Some(mut file) = file else {
        return Err(PlanErrorV1 {
            result_state: ProofResultStateV1::Unsupported,
            message: "allocate unique temporary plan artifact".to_string(),
        });
    };
    if let Err(error) = file.write_all(format!("{serialized}\n").as_bytes()) {
        let _ = std::fs::remove_file(&temporary);
        return Err(PlanErrorV1 {
            result_state: ProofResultStateV1::Unsupported,
            message: format!("write temporary plan artifact: {error}"),
        });
    }
    drop(file);
    match std::fs::hard_link(&temporary, output_path) {
        Ok(()) => {
            let _ = std::fs::remove_file(&temporary);
            Ok(())
        }
        Err(error) => {
            let _ = std::fs::remove_file(&temporary);
            Err(PlanErrorV1 {
                result_state: ProofResultStateV1::Unsupported,
                message: format!(
                    "commit plan artifact without overwriting an existing path: {error}"
                ),
            })
        }
    }
}

/// Plan proof execution from an intent obligation plan envelope.
fn plan_from_intent_envelope(
    envelope: &intent_protocol::IntentObligationPlanEnvelopeV1,
) -> Result<PlanOutcomeV1, PlanErrorV1> {
    // Provider selection is not yet established (#3598/#2938): the
    // product registry is empty and no provider is fabricated. The
    // intent digest stays available, and the failure names the exact
    // missing provider and the limitation.
    let registry = ProviderRegistryV1::new(Vec::new());
    // Digest validation still runs first: an invalid envelope fails with
    // the missing-input state rather than a provider result.
    intent_obligation_plan_digest(envelope).map_err(|err| PlanErrorV1 {
        result_state: ProofResultStateV1::Missing,
        message: err,
    })?;
    let Err(err) = plan_proof_execution_from_intent(envelope, &registry) else {
        return Err(PlanErrorV1 {
            result_state: ProofResultStateV1::Unsupported,
            message:
                "planner produced a plan from an empty provider registry; provider selection must not fabricate"
                    .to_string(),
        });
    };
    Err(PlanErrorV1 {
        result_state: ProofResultStateV1::ProviderUnavailable,
        message: format!(
            "provider unavailable: executable provider selection is not yet established;          intended provider `{INTENDED_PROVIDER_ID}` is not registered and no provider is fabricated;          planner result: {}",
            planner_result_detail(&err),
        ),
    })
}

fn planner_result_detail(err: &IntentPlannerError) -> String {
    match err {
        IntentPlannerError::ProviderRegistry(detail) => {
            format!("{} ({})", err.as_str(), detail.as_str())
        }
        other => other.as_str().to_string(),
    }
}

pub fn render_plan_frame(outcome: &PlanOutcomeV1, format: OutputFormat) -> Result<String, String> {
    let frame = PlanFrameV1 {
        schema_id: PLAN_FRAME_SCHEMA_ID.to_string(),
        plan_id: outcome.plan.plan_id.clone(),
        intent_plan_digest: outcome.intent_plan_digest.clone(),
        command_count: outcome.plan.commands.len(),
        claim_boundary: PLAN_CLAIM_BOUNDARY.to_string(),
    };
    let rendered = emit_frame(&frame, format)?;
    if format == OutputFormat::Json {
        return Ok(rendered);
    }
    let mut output = rendered;
    output.push_str(&format!("schema: {PROOF_PLAN_SCHEMA_ID}\n"));
    Ok(output)
}

pub fn render_plan_v2_frame(
    outcome: &PlanV2OutcomeV1,
    format: OutputFormat,
) -> Result<String, String> {
    let frame = PlanFrameV1 {
        schema_id: outcome.plan.schema_id.clone(),
        plan_id: outcome.plan.plan_id.clone(),
        intent_plan_digest: outcome.plan.intent_plan_digest.clone(),
        command_count: outcome
            .plan
            .items
            .iter()
            .filter(|item| item.disposition.lowers_to_command())
            .count(),
        claim_boundary: format!(
            "Complete proof.plan.v2 artifact retained at {}; no provider execution occurred.",
            outcome.output
        ),
    };
    emit_frame(&frame, format)
}

#[cfg(test)]
mod tests {
    use super::*;
    use intent_protocol::{
        IntentArtifactKindV1, IntentIdentityEnvelopeV1, IntentObligationPlanEnvelopeV1,
        IntentObligationPostureV1, IntentPhaseObligationKindV1, IntentPhaseObligationV1,
        RepositorySnapshotV1, ResolvedRevisionV1,
    };
    use proof_engine::CapturedReceiptStoreV1;

    #[test]
    fn empty_registry_plan_fails_explicitly() -> Result<(), String> {
        let identity = IntentIdentityEnvelopeV1::new(
            RepositorySnapshotV1::new_committed_head(
                "test",
                "sha1",
                ResolvedRevisionV1 {
                    requested: "HEAD".to_string(),
                    commit: "abc".to_string(),
                    tree: String::new(),
                },
            ),
            IntentArtifactKindV1::RequirementDocument,
            "test-artifact",
            "test/source.md",
            "test-content",
        );
        let envelope = IntentObligationPlanEnvelopeV1::new(
            identity,
            "precommit",
            vec![IntentPhaseObligationV1 {
                handoff: None,
                obligation_id: "obl-direct".to_string(),
                phase: "precommit".to_string(),
                kind: IntentPhaseObligationKindV1::EvidenceReview,
                statement: "Review evidence".to_string(),
                posture: IntentObligationPostureV1::Blocking,
                evidence_refs: vec![],
            }],
        );
        let Err(error) = plan_from_intent_envelope(&envelope) else {
            return Err("empty registry must not produce a plan".into());
        };
        if error.result_state != ProofResultStateV1::ProviderUnavailable {
            return Err(format!(
                "unavailable result must carry the provider_unavailable state: {:?}",
                error.result_state
            ));
        }
        for required in [
            "provider unavailable",
            INTENDED_PROVIDER_ID,
            "not yet established",
            "no provider is fabricated",
        ] {
            if !error.message.contains(required) {
                return Err(format!(
                    "unavailable result missing {required:?}: {}",
                    error.message
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn selected_registry_plan_writes_a_deterministic_artifact() -> Result<(), String> {
        let identity = IntentIdentityEnvelopeV1::new(
            RepositorySnapshotV1::new_committed_head(
                "test",
                "sha1",
                ResolvedRevisionV1 {
                    requested: "HEAD".to_string(),
                    commit: "abc".to_string(),
                    tree: String::new(),
                },
            ),
            IntentArtifactKindV1::RequirementDocument,
            "test-artifact",
            "test/source.md",
            "test-content",
        );
        let envelope = IntentObligationPlanEnvelopeV1::new(
            identity,
            "precommit",
            vec![IntentPhaseObligationV1 {
                handoff: None,
                obligation_id: "obl-direct".to_string(),
                phase: "precommit".to_string(),
                kind: IntentPhaseObligationKindV1::EvidenceReview,
                statement: "Review evidence".to_string(),
                posture: IntentObligationPostureV1::Blocking,
                evidence_refs: vec![],
            }],
        );
        let directory =
            std::env::temp_dir().join(format!("cargo-proof-plan-{}", std::process::id()));
        std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        let obligation = directory.join("obligation.json");
        let receipts = directory.join("receipts.json");
        let output = directory.join("plan.json");
        std::fs::write(
            &obligation,
            serde_json::to_vec(&envelope).map_err(|e| e.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        std::fs::write(
            &receipts,
            serde_json::to_vec(&CapturedReceiptStoreV1::new()).map_err(|e| e.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        let outcome = plan_v2_from_selected_registry(&obligation, &receipts, &output)
            .map_err(|error| error.message)?;
        if !output.is_file() || outcome.output != output.display().to_string() {
            return Err("selected-registry plan did not write its artifact".to_string());
        }
        let temporary_count = std::fs::read_dir(&directory)
            .map_err(|error| error.to_string())?
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".plan.json.")
            })
            .count();
        if temporary_count != 0 {
            return Err("successful plan publication left a temporary artifact".to_string());
        }
        let repeat = plan_v2_from_selected_registry(&obligation, &receipts, &output)
            .expect_err("existing plan artifacts are not overwritten");
        if repeat.result_state != ProofResultStateV1::Unsupported || !output.is_file() {
            return Err("existing plan artifact should be preserved".to_string());
        }
        std::fs::remove_dir_all(&directory).map_err(|error| error.to_string())?;
        Ok(())
    }

    #[test]
    fn selected_registry_plan_reports_input_and_output_failures() -> Result<(), String> {
        let missing =
            std::env::temp_dir().join(format!("cargo-proof-missing-{}", std::process::id()));
        let output = missing.with_extension("json");
        let error = plan_v2_from_selected_registry(&missing, &missing, &output)
            .expect_err("missing obligation must fail");
        if error.result_state != ProofResultStateV1::Missing {
            return Err("missing obligation should be classified as missing".to_string());
        }

        let directory =
            std::env::temp_dir().join(format!("cargo-proof-input-{}", std::process::id()));
        std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        let obligation = directory.join("obligation.json");
        let receipts = directory.join("receipts.json");
        std::fs::write(&obligation, b"not-json").map_err(|error| error.to_string())?;
        std::fs::write(&receipts, b"{}").map_err(|error| error.to_string())?;
        let error = plan_v2_from_selected_registry(&obligation, &receipts, &output)
            .expect_err("malformed obligation must fail");
        if error.result_state != ProofResultStateV1::Missing {
            return Err("malformed obligation should be classified as missing".to_string());
        }
        std::fs::write(&obligation, b"{}").map_err(|error| error.to_string())?;
        let error = plan_v2_from_selected_registry(&obligation, &receipts, &output)
            .expect_err("invalid envelope must fail");
        if error.result_state != ProofResultStateV1::Missing {
            return Err("invalid envelope should be classified as missing".to_string());
        }
        let identity = IntentIdentityEnvelopeV1::new(
            RepositorySnapshotV1::new_committed_head(
                "test",
                "sha1",
                ResolvedRevisionV1 {
                    requested: "HEAD".to_string(),
                    commit: "abc".to_string(),
                    tree: String::new(),
                },
            ),
            IntentArtifactKindV1::RequirementDocument,
            "test-artifact",
            "test/source.md",
            "test-content",
        );
        let valid_envelope = serde_json::to_vec(&IntentObligationPlanEnvelopeV1::new(
            identity,
            "precommit",
            vec![],
        ))
        .map_err(|error| error.to_string())?;
        std::fs::write(&obligation, valid_envelope).map_err(|error| error.to_string())?;
        std::fs::remove_file(&receipts).map_err(|error| error.to_string())?;
        let error = plan_v2_from_selected_registry(&obligation, &receipts, &output)
            .expect_err("missing receipt inventory must fail");
        if error.result_state != ProofResultStateV1::Missing {
            return Err("missing receipt inventory should be classified as missing".to_string());
        }
        std::fs::write(&receipts, b"not-json").map_err(|error| error.to_string())?;
        let error = plan_v2_from_selected_registry(&obligation, &receipts, &output)
            .expect_err("malformed receipt inventory must fail");
        if error.result_state != ProofResultStateV1::Missing {
            return Err("malformed receipt inventory should be classified as missing".to_string());
        }
        std::fs::write(
            &receipts,
            serde_json::to_vec(&CapturedReceiptStoreV1::new())
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        let unwritable = directory.join("missing-parent").join("plan.json");
        let error = plan_v2_from_selected_registry(&obligation, &receipts, &unwritable)
            .expect_err("missing output parent must fail");
        if error.result_state != ProofResultStateV1::Unsupported {
            return Err("output write failure should be unsupported".to_string());
        }
        let output_directory = directory.join("output-directory");
        std::fs::create_dir_all(&output_directory).map_err(|error| error.to_string())?;
        let error = plan_v2_from_selected_registry(&obligation, &receipts, &output_directory)
            .expect_err("directory output target must fail");
        if error.result_state != ProofResultStateV1::Unsupported {
            return Err("output rename failure should be unsupported".to_string());
        }
        std::fs::write(&receipts, br#"{"schema_id":"wrong","sets":[]}"#)
            .map_err(|error| error.to_string())?;
        let error = plan_v2_from_selected_registry(&obligation, &receipts, &output)
            .expect_err("invalid receipt store must fail");
        if error.result_state != ProofResultStateV1::Unsupported {
            return Err("invalid receipt store should be unsupported".to_string());
        }
        std::fs::remove_dir_all(&directory).map_err(|error| error.to_string())?;
        Ok(())
    }
}
