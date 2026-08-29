//! Typed evaluation artifact set (#3879): one exact cargo-allow evaluation
//! produces one immutable semantic result and a deterministic plan for zero
//! or more renderers. Renderers cannot re-evaluate findings, policy, or exit
//! posture; renderer failure and artifact truncation never change the
//! underlying semantic result.

use serde::{Deserialize, Serialize};

pub const EVALUATION_ARTIFACT_SET_V1_SCHEMA_VERSION: u32 = 1;
pub const EVALUATION_ARTIFACT_SET_V1_SCHEMA_ID: &str = "cargo-allow.evaluation-artifact-set.v1";

/// Closed vocabulary of supported renderer formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RendererFormatV1 {
    HumanSummary,
    Markdown,
    Json,
    Html,
    Sarif,
    Receipt,
    CoreCommandSummary,
    ArtifactSetManifest,
}

impl RendererFormatV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HumanSummary => "human_summary",
            Self::Markdown => "markdown",
            Self::Json => "json",
            Self::Html => "html",
            Self::Sarif => "sarif",
            Self::Receipt => "receipt",
            Self::CoreCommandSummary => "core_command_summary",
            Self::ArtifactSetManifest => "artifact_set_manifest",
        }
    }

    /// Deterministic filename stem derived from operation + format.
    pub fn file_extension(self) -> &'static str {
        match self {
            Self::HumanSummary => "txt",
            Self::Markdown => "md",
            Self::Json | Self::Sarif | Self::CoreCommandSummary | Self::ArtifactSetManifest => {
                "json"
            }
            Self::Html => "html",
            Self::Receipt => "receipt.json",
        }
    }
}

/// The semantic result class of the underlying evaluation (not the artifact
/// production status). Only exact `Passed` is clean.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationResultClassV2 {
    Passed,
    Blocking,
    Advisory,
    Partial,
    UnknownAttribution,
    Stale,
    Unsupported,
    InstrumentFailure,
}

/// One planned or produced artifact in the set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluationArtifactEntryV1 {
    pub format: RendererFormatV1,
    /// Deterministic filename derived from operation + format.
    pub file_name: String,
    /// sha256 of the artifact content; absent until written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    /// Written / RenderFailed / Omitted.
    pub status: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub render_errors: Vec<String>,
}

/// Closed completeness vocabulary for the artifact set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationArtifactSetResultV2 {
    Complete,
    SemanticNonGreen,
    PartialArtifacts,
    RenderFailure,
    OutputConflict,
    InstrumentFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluationArtifactSetV1Validation {
    pub result: EvaluationArtifactSetResultV2,
    pub gaps: Vec<String>,
}

/// The canonical typed artifact set. One evaluation → one set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluationArtifactSetV1 {
    pub schema_id: String,
    pub schema_version: u32,
    /// Tool/product identity.
    pub tool: String,
    pub tool_version: String,
    /// Operation and mode (e.g. "check" / "no-new").
    pub operation: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    /// Exact repository source subject identity (commit or worktree).
    pub source_subject: String,
    /// Identity of the resolved config the evaluation consumed.
    pub resolved_config_identity: String,
    /// sha256 of the semantic result digest (one digest across all artifacts).
    pub semantic_result_digest: String,
    /// The underlying semantic result class.
    pub result_class: EvaluationResultClassV2,
    /// The blocking posture of the semantic result (independent of renderer).
    pub blocking: bool,
    pub requested_formats: Vec<RendererFormatV1>,
    pub artifacts: Vec<EvaluationArtifactEntryV1>,
    /// Reference to the common command summary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub core_command_summary_ref: Option<String>,
    /// Reference to the command-specific detailed result.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detailed_result_ref: Option<String>,
    pub limitations: Vec<String>,
    pub claim_boundary: String,
}

impl EvaluationArtifactSetV1 {
    /// Deterministic filename for an operation + format pair.
    pub fn artifact_file_name(operation: &str, format: RendererFormatV1) -> String {
        format!(
            "{}-{}.{}",
            operation,
            format.as_str(),
            format.file_extension()
        )
    }

    /// Detect output collisions between requested artifact paths. No two
    /// artifacts, the selected policy, or the input source may collide.
    pub fn detect_collisions(artifact_paths: &[String], reserved_paths: &[String]) -> Vec<String> {
        let mut collisions = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for path in artifact_paths {
            let normalized = path.replace('\\', "/");
            if !seen.insert(normalized.clone()) {
                collisions.push(format!("duplicate artifact path: {normalized}"));
            }
        }
        for reserved in reserved_paths {
            let normalized = reserved.replace('\\', "/");
            if artifact_paths
                .iter()
                .any(|a| a.replace('\\', "/") == normalized)
            {
                collisions.push(format!(
                    "artifact path collides with reserved file: {normalized}"
                ));
            }
        }
        collisions
    }

    /// Validate the artifact set's structural law (#3879): one semantic
    /// digest across all entries, no duplicate formats, no private paths,
    /// and renderer status separate from semantic posture.
    pub fn validate(&self) -> EvaluationArtifactSetV1Validation {
        let mut gaps = Vec::new();
        let generation_current = self.schema_id == EVALUATION_ARTIFACT_SET_V1_SCHEMA_ID
            && self.schema_version == EVALUATION_ARTIFACT_SET_V1_SCHEMA_VERSION;
        if !generation_current {
            gaps.push("non-current evaluation-artifact-set generation".to_string());
        }

        for (field, value) in [
            ("tool", self.tool.as_str()),
            ("tool_version", self.tool_version.as_str()),
            ("operation", self.operation.as_str()),
            ("source_subject", self.source_subject.as_str()),
            (
                "resolved_config_identity",
                self.resolved_config_identity.as_str(),
            ),
            (
                "semantic_result_digest",
                self.semantic_result_digest.as_str(),
            ),
            ("claim_boundary", self.claim_boundary.as_str()),
        ] {
            if value.trim().is_empty() {
                gaps.push(format!("{field} is missing"));
            }
        }
        if !self.semantic_result_digest.starts_with("sha256:") {
            gaps.push("semantic_result_digest is not a sha256 digest".to_string());
        }

        if self.requested_formats.is_empty() {
            gaps.push("requested_formats is empty".to_string());
        }

        // No duplicate artifact entries per format.
        let mut formats = std::collections::BTreeSet::new();
        for (index, entry) in self.artifacts.iter().enumerate() {
            if !formats.insert(entry.format) {
                gaps.push(format!(
                    "artifacts[{index}] duplicate format {}",
                    entry.format.as_str()
                ));
            }
            if entry.file_name.trim().is_empty() {
                gaps.push(format!("artifacts[{index}] file_name is missing"));
            }
            for error in &entry.render_errors {
                if error.trim().is_empty() {
                    gaps.push(format!("artifacts[{index}] carries a blank render error"));
                }
            }
        }

        // Every produced artifact must carry a digest; failed renders must
        // carry at least one render error.
        for (index, entry) in self.artifacts.iter().enumerate() {
            if entry.status == "Written" && entry.content_digest.is_none() {
                gaps.push(format!(
                    "artifacts[{index}] status Written but no content digest"
                ));
            }
            if entry.status == "RenderFailed" && entry.render_errors.is_empty() {
                gaps.push(format!(
                    "artifacts[{index}] status RenderFailed but no errors listed"
                ));
            }
        }

        if !self.claim_boundary.trim().is_empty() && self.claim_boundary.contains("/home/") {
            gaps.push("claim_boundary contains private path data".to_string());
        }

        let any_render_failure = self
            .artifacts
            .iter()
            .any(|entry| entry.status == "RenderFailed");
        let any_collision = self
            .limitations
            .iter()
            .any(|limit| limit.contains("collision"));

        if !generation_current {
            return EvaluationArtifactSetV1Validation {
                result: EvaluationArtifactSetResultV2::InstrumentFailure,
                gaps,
            };
        }
        if any_collision {
            return EvaluationArtifactSetV1Validation {
                result: EvaluationArtifactSetResultV2::OutputConflict,
                gaps,
            };
        }
        if any_render_failure {
            return EvaluationArtifactSetV1Validation {
                result: EvaluationArtifactSetResultV2::RenderFailure,
                gaps,
            };
        }
        if self.artifacts.is_empty() {
            return EvaluationArtifactSetV1Validation {
                result: EvaluationArtifactSetResultV2::PartialArtifacts,
                gaps,
            };
        }
        let all_written = self.artifacts.iter().all(|entry| entry.status == "Written");
        if all_written && gaps.is_empty() {
            let result = if self.result_class == EvaluationResultClassV2::Passed {
                EvaluationArtifactSetResultV2::Complete
            } else {
                EvaluationArtifactSetResultV2::SemanticNonGreen
            };
            EvaluationArtifactSetV1Validation { result, gaps }
        } else if gaps.is_empty() {
            EvaluationArtifactSetV1Validation {
                result: EvaluationArtifactSetResultV2::PartialArtifacts,
                gaps,
            }
        } else {
            EvaluationArtifactSetV1Validation {
                result: EvaluationArtifactSetResultV2::InstrumentFailure,
                gaps,
            }
        }
    }
}

/// Render only the semantic payload. Serde's declaration order is canonical.
pub fn render_evaluation_artifact_set_v1(
    payload: &EvaluationArtifactSetV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(payload)
}

pub fn render_evaluation_artifact_set_v1_bytes(
    payload: &EvaluationArtifactSetV1,
) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(format: RendererFormatV1, status: &str) -> EvaluationArtifactEntryV1 {
        EvaluationArtifactEntryV1 {
            format,
            file_name: EvaluationArtifactSetV1::artifact_file_name("check", format),
            content_digest: if status == "Written" {
                Some(format!("sha256:{:064x}", 42))
            } else {
                None
            },
            size_bytes: if status == "Written" {
                Some(1024)
            } else {
                None
            },
            status: status.to_string(),
            render_errors: Vec::new(),
        }
    }

    fn artifact_set() -> EvaluationArtifactSetV1 {
        EvaluationArtifactSetV1 {
            schema_id: EVALUATION_ARTIFACT_SET_V1_SCHEMA_ID.to_string(),
            schema_version: EVALUATION_ARTIFACT_SET_V1_SCHEMA_VERSION,
            tool: "cargo-allow".to_string(),
            tool_version: "0.2.0-rc.1".to_string(),
            operation: "check".to_string(),
            mode: Some("no-new".to_string()),
            source_subject: "worktree:clean".to_string(),
            resolved_config_identity: "sha256:".to_string() + &"a".repeat(64),
            semantic_result_digest: format!("sha256:{}", "b".repeat(64)),
            result_class: EvaluationResultClassV2::Passed,
            blocking: false,
            requested_formats: vec![
                RendererFormatV1::Markdown,
                RendererFormatV1::Json,
                RendererFormatV1::Sarif,
            ],
            artifacts: vec![
                entry(RendererFormatV1::Markdown, "Written"),
                entry(RendererFormatV1::Json, "Written"),
                entry(RendererFormatV1::Sarif, "Written"),
            ],
            core_command_summary_ref: Some("core-command-summary.json".to_string()),
            detailed_result_ref: Some("check-receipt.json".to_string()),
            limitations: Vec::new(),
            claim_boundary: "one evaluation, many renderers".to_string(),
        }
    }

    #[test]
    fn complete_set_passes_validation() -> Result<(), String> {
        let set = artifact_set();
        let validation = set.validate();
        if validation.result != EvaluationArtifactSetResultV2::Complete {
            return Err(format!("complete set rejected: {validation:?}"));
        }
        Ok(())
    }

    #[test]
    fn renderer_failure_is_fail_honest() -> Result<(), String> {
        let mut set = artifact_set();
        set.artifacts[2].status = "RenderFailed".to_string();
        set.artifacts[2].render_errors = vec!["SARIF renderer failed: disk full".to_string()];
        set.artifacts[2].content_digest = None;
        let validation = set.validate();
        if validation.result != EvaluationArtifactSetResultV2::RenderFailure {
            return Err(format!(
                "renderer failure was not classified: {validation:?}"
            ));
        }
        // The semantic result is unchanged despite renderer failure.
        if set.result_class != EvaluationResultClassV2::Passed {
            return Err("semantic result was mutated by renderer failure".to_string());
        }
        Ok(())
    }

    #[test]
    fn duplicate_formats_are_rejected() -> Result<(), String> {
        let mut set = artifact_set();
        set.artifacts.push(entry(RendererFormatV1::Json, "Written"));
        let validation = set.validate();
        if validation.result == EvaluationArtifactSetResultV2::Complete {
            return Err("duplicate format was accepted".to_string());
        }
        if !validation.gaps.iter().any(|gap| gap.contains("duplicate")) {
            return Err("duplicate format gap was not reported".to_string());
        }
        Ok(())
    }

    #[test]
    fn output_collision_is_detected() -> Result<(), String> {
        let collisions = EvaluationArtifactSetV1::detect_collisions(
            &["out/report.md".to_string(), "out/report.md".to_string()],
            &["policy/allow.toml".to_string()],
        );
        if collisions.len() != 1 {
            return Err("duplicate artifact path was not detected".to_string());
        }
        let collisions = EvaluationArtifactSetV1::detect_collisions(
            &["policy/allow.toml".to_string()],
            &["policy/allow.toml".to_string()],
        );
        if collisions.is_empty() {
            return Err("policy collision was not detected".to_string());
        }
        Ok(())
    }

    #[test]
    fn semantic_non_green_does_not_become_complete() -> Result<(), String> {
        let mut set = artifact_set();
        set.result_class = EvaluationResultClassV2::Blocking;
        set.blocking = true;
        let validation = set.validate();
        if validation.result != EvaluationArtifactSetResultV2::SemanticNonGreen {
            return Err(format!(
                "semantic non-green was not classified: {validation:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn deterministic_filename_is_derived_from_operation_and_format() {
        let name = EvaluationArtifactSetV1::artifact_file_name("check", RendererFormatV1::Markdown);
        assert_eq!(name, "check-markdown.md");
        let name = EvaluationArtifactSetV1::artifact_file_name("diff", RendererFormatV1::Sarif);
        assert_eq!(name, "diff-sarif.json");
    }

    #[test]
    fn rendering_is_deterministic_across_equal_payloads() -> Result<(), String> {
        let first = render_evaluation_artifact_set_v1_bytes(&artifact_set())
            .map_err(|error| error.to_string())?;
        let second = render_evaluation_artifact_set_v1_bytes(&artifact_set())
            .map_err(|error| error.to_string())?;
        if first != second {
            return Err("equal payloads rendered different bytes".to_string());
        }
        Ok(())
    }
}
