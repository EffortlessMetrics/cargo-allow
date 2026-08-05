//! Explicit precommit tool identity, selection, and compatibility contracts.
//!
//! This module does not discover, build, download, or execute a tool. Callers
//! provide the executable path and the machine-safe identity reported by that
//! prebuilt executable. The byte digest is checked locally before evaluation
//! and again before publishing a result.

use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult, sha256_v1_bytes};
use clap::{Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

pub const TOOL_IDENTITY_SCHEMA_ID: &str = "cargo-allow.tool-identity.v1";
pub const TOOL_IDENTITY_SCHEMA_VERSION: u32 = 1;
pub const COMMAND_API_GENERATION: &str = "cargo-allow.command-api.v1";
pub const PROFILE_GENERATION: &str = "current-v2";
pub const PRECOMMIT_RESULT_GENERATION: &str = "cargo-allow.precommit-result.v1";
pub const STAGED_GIT_CAPABILITY_GENERATION: &str = "cargo-allow.staged-git.v1";

const IMPLEMENTATION_SLICE_SCHEMA: &str = "implementation-slice.v2.0";
const REQUIREMENT_BLOCK_SCHEMA: &str = "requirement-block.v1.0";
const AUTHORED_MAPPING_SCHEMA: &str = "authored-mapping.v1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ToolChannel {
    PublishedRelease,
    SourcePreview,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CargoAllowToolIdentityV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub executable_digest: String,
    pub reported_version: String,
    pub build_source_commit: Option<String>,
    pub compiler_identity: Option<String>,
    pub target: String,
    pub profile: Option<String>,
    pub command_api_generation: String,
    pub supported_profile_generations: Vec<String>,
    pub supported_schema_generations: Vec<String>,
    pub supported_result_generations: Vec<String>,
    pub supported_receipt_generations: Vec<String>,
    pub staged_git_capability_generation: String,
    pub channel: ToolChannel,
}

impl CargoAllowToolIdentityV1 {
    fn for_digest(executable_digest: String) -> Self {
        Self {
            schema_id: TOOL_IDENTITY_SCHEMA_ID.to_string(),
            schema_version: TOOL_IDENTITY_SCHEMA_VERSION,
            executable_digest,
            reported_version: env!("CARGO_PKG_VERSION").to_string(),
            build_source_commit: option_env!("CARGO_ALLOW_BUILD_COMMIT").map(str::to_owned),
            compiler_identity: option_env!("CARGO_ALLOW_RUSTC_IDENTITY").map(str::to_owned),
            target: format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS),
            profile: option_env!("CARGO_ALLOW_BUILD_PROFILE").map(str::to_owned),
            command_api_generation: COMMAND_API_GENERATION.to_string(),
            supported_profile_generations: vec![PROFILE_GENERATION.to_string()],
            supported_schema_generations: vec![
                allow_report::SPEC_SYSTEM_SCHEMA_ID.to_string(),
                IMPLEMENTATION_SLICE_SCHEMA.to_string(),
                REQUIREMENT_BLOCK_SCHEMA.to_string(),
                AUTHORED_MAPPING_SCHEMA.to_string(),
            ],
            supported_result_generations: vec![PRECOMMIT_RESULT_GENERATION.to_string()],
            supported_receipt_generations: vec![allow_report::RECEIPT_SCHEMA_ID.to_string()],
            staged_git_capability_generation: STAGED_GIT_CAPABILITY_GENERATION.to_string(),
            channel: match option_env!("CARGO_ALLOW_TOOL_CHANNEL") {
                Some("published-release") => ToolChannel::PublishedRelease,
                _ => ToolChannel::SourcePreview,
            },
        }
    }

    pub fn supports(&self, requirement: &ToolCompatibilityRequirement) -> bool {
        self.command_api_generation == requirement.command_api_generation
            && self
                .supported_profile_generations
                .contains(&requirement.profile_generation)
            && self
                .supported_schema_generations
                .contains(&requirement.schema_generation)
            && self
                .supported_result_generations
                .contains(&requirement.result_generation)
            && self
                .supported_receipt_generations
                .contains(&requirement.receipt_generation)
            && self.staged_git_capability_generation == requirement.staged_git_capability_generation
    }

    fn validate_shape(&self) -> Result<(), ToolSelectionFailure> {
        if self.schema_id != TOOL_IDENTITY_SCHEMA_ID
            || self.schema_version != TOOL_IDENTITY_SCHEMA_VERSION
            || !self.executable_digest.starts_with("sha256:v1:")
            || self.reported_version.trim().is_empty()
            || self.target.trim().is_empty()
            || self.command_api_generation.trim().is_empty()
            || self.supported_profile_generations.is_empty()
            || self.supported_schema_generations.is_empty()
            || self.supported_result_generations.is_empty()
            || self.supported_receipt_generations.is_empty()
            || self.staged_git_capability_generation.trim().is_empty()
        {
            return Err(ToolSelectionFailure::new(
                ToolResultClass::MalformedToolIdentity,
                "tool identity schema or required capability fields are malformed",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCompatibilityRequirement {
    pub command_api_generation: String,
    pub profile_generation: String,
    pub schema_generation: String,
    pub result_generation: String,
    pub receipt_generation: String,
    pub staged_git_capability_generation: String,
}

impl ToolCompatibilityRequirement {
    pub fn current() -> Self {
        Self {
            command_api_generation: COMMAND_API_GENERATION.to_string(),
            profile_generation: PROFILE_GENERATION.to_string(),
            schema_generation: allow_report::SPEC_SYSTEM_SCHEMA_ID.to_string(),
            result_generation: PRECOMMIT_RESULT_GENERATION.to_string(),
            receipt_generation: allow_report::RECEIPT_SCHEMA_ID.to_string(),
            staged_git_capability_generation: STAGED_GIT_CAPABILITY_GENERATION.to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "PascalCase")]
pub enum ToolSelectionMode {
    InstalledPinned,
    ExplicitToolUnderTest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolSelectionRequest {
    pub mode: ToolSelectionMode,
    pub executable: PathBuf,
    pub expected_digest: Option<String>,
    pub expected_build_source_commit: Option<String>,
    pub preview_authorized: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ToolResultClass {
    ToolPrebuiltAndSelected,
    ToolMissing,
    ToolIdentityMismatch,
    ToolChangedDuringRun,
    ToolGenerationUnsupported,
    CandidateSchemaUnsupported,
    PreviewToolNotAuthorized,
    MalformedToolIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolSelectionReceiptV1 {
    pub result: ToolResultClass,
    pub mode: ToolSelectionMode,
    /// Local diagnostic only; consumers must use `identity.executable_digest`
    /// as the portable identity.
    pub executable_path: String,
    pub executable_digest: String,
    pub identity: CargoAllowToolIdentityV1,
    pub preview_evidence: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolSelectionFailure {
    pub result: ToolResultClass,
    pub message: String,
}

impl ToolSelectionFailure {
    fn new(result: ToolResultClass, message: impl Into<String>) -> Self {
        Self {
            result,
            message: message.into(),
        }
    }
}

impl fmt::Display for ToolSelectionFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.result, self.message)
    }
}

impl std::error::Error for ToolSelectionFailure {}

fn tool_identity_artifact(message: impl Into<String>, source: &std::io::Error) -> CargoAllowError {
    CargoAllowError::with_kind(CargoAllowErrorKind::Artifact, message).with_cause(source)
}

pub fn current_tool_identity() -> CargoAllowResult<CargoAllowToolIdentityV1> {
    let executable = std::env::current_exe().map_err(|error| {
        tool_identity_artifact("failed to resolve cargo-allow executable", &error)
    })?;
    let bytes = fs::read(&executable).map_err(|error| {
        tool_identity_artifact(
            format!(
                "failed to read cargo-allow executable `{}`",
                executable.display()
            ),
            &error,
        )
    })?;
    Ok(identity_for_bytes(&bytes))
}

pub fn identity_for_bytes(bytes: &[u8]) -> CargoAllowToolIdentityV1 {
    CargoAllowToolIdentityV1::for_digest(sha256_v1_bytes(bytes))
}

pub fn select_tool(
    request: &ToolSelectionRequest,
    identity: CargoAllowToolIdentityV1,
    requirement: &ToolCompatibilityRequirement,
) -> Result<ToolSelectionReceiptV1, ToolSelectionFailure> {
    if matches!(request.mode, ToolSelectionMode::ExplicitToolUnderTest)
        && !request.preview_authorized
    {
        return Err(ToolSelectionFailure::new(
            ToolResultClass::PreviewToolNotAuthorized,
            "explicit tool-under-test selection requires preview authorization",
        ));
    }

    let bytes = fs::read(&request.executable).map_err(|error| {
        let result = if error.kind() == std::io::ErrorKind::NotFound {
            ToolResultClass::ToolMissing
        } else {
            ToolResultClass::MalformedToolIdentity
        };
        ToolSelectionFailure::new(
            result,
            format!(
                "failed to read selected executable `{}`: {error}",
                request.executable.display()
            ),
        )
    })?;
    let actual_digest = sha256_v1_bytes(&bytes);
    identity.validate_shape()?;

    if identity.executable_digest != actual_digest {
        return Err(ToolSelectionFailure::new(
            ToolResultClass::MalformedToolIdentity,
            "reported executable digest does not match selected executable bytes",
        ));
    }
    let Some(expected_digest) = request.expected_digest.as_deref() else {
        return Err(ToolSelectionFailure::new(
            ToolResultClass::MalformedToolIdentity,
            "selected executable has no expected immutable digest",
        ));
    };
    if expected_digest != actual_digest {
        return Err(ToolSelectionFailure::new(
            ToolResultClass::ToolIdentityMismatch,
            "selected executable digest differs from the expected digest",
        ));
    }
    if let Some(expected_commit) = request.expected_build_source_commit.as_deref()
        && identity.build_source_commit.as_deref() != Some(expected_commit)
    {
        return Err(ToolSelectionFailure::new(
            ToolResultClass::ToolIdentityMismatch,
            "selected executable build/source identity differs from the expected identity",
        ));
    }
    if !identity.supports(requirement) {
        let schema_supported = identity
            .supported_schema_generations
            .contains(&requirement.schema_generation);
        return Err(ToolSelectionFailure::new(
            if schema_supported {
                ToolResultClass::ToolGenerationUnsupported
            } else {
                ToolResultClass::CandidateSchemaUnsupported
            },
            "selected executable does not support every required generation",
        ));
    }
    if matches!(request.mode, ToolSelectionMode::InstalledPinned)
        && identity.channel != ToolChannel::PublishedRelease
    {
        return Err(ToolSelectionFailure::new(
            ToolResultClass::PreviewToolNotAuthorized,
            "installed-pinned selection requires a published-release identity",
        ));
    }

    Ok(ToolSelectionReceiptV1 {
        result: ToolResultClass::ToolPrebuiltAndSelected,
        mode: request.mode,
        executable_path: request.executable.display().to_string(),
        executable_digest: actual_digest,
        identity,
        preview_evidence: matches!(request.mode, ToolSelectionMode::ExplicitToolUnderTest),
    })
}

pub fn verify_tool_unchanged(
    executable: impl AsRef<Path>,
    captured_digest: &str,
) -> Result<(), ToolSelectionFailure> {
    let executable = executable.as_ref();
    let bytes = fs::read(executable).map_err(|error| {
        ToolSelectionFailure::new(
            ToolResultClass::ToolChangedDuringRun,
            format!("selected executable is no longer readable: {error}"),
        )
    })?;
    let current_digest = sha256_v1_bytes(&bytes);
    if current_digest != captured_digest {
        return Err(ToolSelectionFailure::new(
            ToolResultClass::ToolChangedDuringRun,
            "selected executable changed after identity capture",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Parser)]
pub(crate) struct ToolArgs {
    #[command(subcommand)]
    pub(crate) command: ToolCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum ToolCommand {
    /// Print the current executable's machine-safe identity and capabilities.
    Identity(ToolIdentityArgs),
}

#[derive(Debug, Clone, Parser)]
pub(crate) struct ToolIdentityArgs {
    /// Output format.
    #[arg(long, value_enum, default_value_t = ToolIdentityFormat::Json)]
    pub(crate) format: ToolIdentityFormat,
    /// Write output to a file instead of stdout.
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum ToolIdentityFormat {
    Human,
    Json,
}

pub(crate) fn cmd_tool(args: &ToolArgs) -> CargoAllowResult<()> {
    let identity = current_tool_identity()?;
    let output = match &args.command {
        ToolCommand::Identity(identity_args) => match identity_args.format {
            ToolIdentityFormat::Json => {
                serde_json::to_string_pretty(&identity).map_err(|error| {
                    CargoAllowError::with_kind(
                        CargoAllowErrorKind::Artifact,
                        format!("failed to render tool identity JSON: {error}"),
                    )
                })?
            }
            ToolIdentityFormat::Human => {
                let style = if identity_args.output.is_none() {
                    crate::reporting::output_style()
                } else {
                    allow_report::Style::PLAIN
                };
                render_human_identity_styled(&identity, style)
            }
        },
    };
    match &args.command {
        ToolCommand::Identity(identity_args) => {
            crate::emit_text(identity_args.output.as_deref(), &output)
        }
    }
}

fn render_human_identity_styled(
    identity: &CargoAllowToolIdentityV1,
    style: allow_report::Style,
) -> String {
    let source = identity.build_source_commit.as_deref().unwrap_or("unknown");
    let compiler = identity.compiler_identity.as_deref().unwrap_or("unknown");
    let channel = match identity.channel {
        ToolChannel::PublishedRelease => style.ok("PublishedRelease"),
        ToolChannel::SourcePreview => style.advisory("SourcePreview"),
    };
    format!(
        "{}\n\
schema: {}\n\
version: {}\n\
digest: {}\n\
channel: {channel}\n\
target: {}\n\
source: {}\n\
compiler: {}\n\
command_api: {}\n\
profile_generations: {}\n\
schema_generations: {}\n\
result_generations: {}\n\
receipt_generations: {}\n\
staged_git: {}",
        style.strong("cargo-allow tool identity"),
        identity.schema_id,
        identity.reported_version,
        identity.executable_digest,
        identity.target,
        source,
        compiler,
        identity.command_api_generation,
        identity.supported_profile_generations.join(", "),
        identity.supported_schema_generations.join(", "),
        identity.supported_result_generations.join(", "),
        identity.supported_receipt_generations.join(", "),
        identity.staged_git_capability_generation,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn human_tool_identity_styles_fixed_channel_only() {
        let identity = identity_for_bytes(b"tool identity style fixture");
        let styled = render_human_identity_styled(&identity, allow_report::Style::ANSI);

        assert!(styled.starts_with("\u{1b}[1mcargo-allow tool identity\u{1b}[0m\n"));
        assert!(styled.contains("channel: \u{1b}[33mSourcePreview\u{1b}[0m\n"));
        assert!(!styled.contains(&format!("{}\u{1b}", identity.executable_digest)));

        let plain = render_human_identity_styled(&identity, allow_report::Style::PLAIN);
        assert!(!plain.contains('\u{1b}'));
    }

    fn selected_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("cargo-allow-tool-{name}-{}", std::process::id()))
    }

    fn write_selected_file(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
        let _ = fs::remove_file(path);
        fs::write(path, bytes)?;
        Ok(())
    }

    fn request(path: &Path, mode: ToolSelectionMode, digest: &str) -> ToolSelectionRequest {
        ToolSelectionRequest {
            mode,
            executable: path.to_path_buf(),
            expected_digest: Some(digest.to_string()),
            expected_build_source_commit: None,
            preview_authorized: true,
        }
    }

    #[test]
    fn precommit_tool_identity() -> Result<(), Box<dyn Error>> {
        let identity = identity_for_bytes(b"prebuilt cargo-allow");
        if identity.schema_id != TOOL_IDENTITY_SCHEMA_ID
            || identity.executable_digest != sha256_v1_bytes(b"prebuilt cargo-allow")
            || identity.supported_profile_generations != vec![PROFILE_GENERATION]
        {
            return Err(
                "identity did not preserve its schema, digest, or profile generation".into(),
            );
        }
        let json = serde_json::to_string(&identity)?;
        let decoded: CargoAllowToolIdentityV1 = serde_json::from_str(&json)?;
        if decoded != identity {
            return Err("identity JSON round-trip changed the capability document".into());
        }
        Ok(())
    }

    #[test]
    fn current_tool_identity_io_failures_are_artifacts() {
        let source = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "permission denied");
        let error = tool_identity_artifact("failed to read cargo-allow executable", &source);

        assert_eq!(error.kind(), CargoAllowErrorKind::Artifact);
        assert_eq!(error.code(), "E0007_ARTIFACT");
        assert!(
            error
                .to_string()
                .contains("failed to read cargo-allow executable")
        );
        assert!(error.source().is_some());
    }

    #[test]
    fn precommit_tool_under_test_selection() -> Result<(), Box<dyn Error>> {
        let path = selected_path("explicit");
        let bytes = b"explicit prebuilt tool";
        write_selected_file(&path, bytes)?;
        let mut identity = identity_for_bytes(bytes);
        identity.channel = ToolChannel::SourcePreview;
        let receipt = select_tool(
            &request(
                &path,
                ToolSelectionMode::ExplicitToolUnderTest,
                &sha256_v1_bytes(bytes),
            ),
            identity,
            &ToolCompatibilityRequirement::current(),
        )?;
        let _ = fs::remove_file(&path);
        if receipt.result != ToolResultClass::ToolPrebuiltAndSelected
            || !receipt.preview_evidence
            || receipt.executable_digest != sha256_v1_bytes(bytes)
        {
            return Err(
                "explicit tool-under-test selection did not produce preview evidence".into(),
            );
        }
        Ok(())
    }

    #[test]
    fn precommit_schema_transition_compatibility() -> Result<(), Box<dyn Error>> {
        let path = selected_path("schema-transition");
        let bytes = b"schema transition tool";
        write_selected_file(&path, bytes)?;
        let identity = identity_for_bytes(bytes);
        let mut requirement = ToolCompatibilityRequirement::current();
        requirement.profile_generation = "next-v3".to_string();
        let failure = select_tool(
            &request(
                &path,
                ToolSelectionMode::ExplicitToolUnderTest,
                &sha256_v1_bytes(bytes),
            ),
            identity,
            &requirement,
        )
        .err();
        let _ = fs::remove_file(&path);
        let Some(failure) = failure else {
            return Err("unsupported profile generation was accepted".into());
        };
        if failure.result != ToolResultClass::ToolGenerationUnsupported {
            return Err("unsupported profile generation returned the wrong result class".into());
        }
        Ok(())
    }

    #[test]
    fn precommit_tool_substitution_fails_closed() -> Result<(), Box<dyn Error>> {
        let path = selected_path("substitution");
        let original = b"original prebuilt tool";
        let replacement = b"replacement prebuilt tool";
        write_selected_file(&path, original)?;
        fs::write(&path, replacement)?;
        let failure = verify_tool_unchanged(&path, &sha256_v1_bytes(original)).err();
        let _ = fs::remove_file(&path);
        let Some(failure) = failure else {
            return Err("replacement executable was incorrectly accepted".into());
        };
        if failure.result != ToolResultClass::ToolChangedDuringRun {
            return Err("replacement executable returned the wrong result class".into());
        }
        Ok(())
    }

    #[test]
    fn precommit_tool_missing_is_not_green() -> Result<(), Box<dyn Error>> {
        let path = selected_path("missing");
        let _ = fs::remove_file(&path);
        let request = request(
            &path,
            ToolSelectionMode::ExplicitToolUnderTest,
            &sha256_v1_bytes(b"missing"),
        );
        let failure = select_tool(
            &request,
            identity_for_bytes(b"missing"),
            &ToolCompatibilityRequirement::current(),
        )
        .err();
        let Some(failure) = failure else {
            return Err("missing selected executable was accepted".into());
        };
        if failure.result != ToolResultClass::ToolMissing {
            return Err("missing selected executable returned the wrong result class".into());
        }
        Ok(())
    }

    #[test]
    fn precommit_tool_digest_mismatch_is_not_green() -> Result<(), Box<dyn Error>> {
        let path = selected_path("digest-mismatch");
        let bytes = b"digest-bound tool";
        write_selected_file(&path, bytes)?;
        let request = request(
            &path,
            ToolSelectionMode::ExplicitToolUnderTest,
            &sha256_v1_bytes(b"different tool"),
        );
        let failure = select_tool(
            &request,
            identity_for_bytes(bytes),
            &ToolCompatibilityRequirement::current(),
        )
        .err();
        let _ = fs::remove_file(&path);
        let Some(failure) = failure else {
            return Err("digest-mismatched selected executable was accepted".into());
        };
        if failure.result != ToolResultClass::ToolIdentityMismatch {
            return Err("digest mismatch returned the wrong result class".into());
        }
        Ok(())
    }

    #[test]
    fn precommit_tool_malformed_identity_is_not_green() -> Result<(), Box<dyn Error>> {
        let path = selected_path("malformed-identity");
        let bytes = b"malformed identity tool";
        write_selected_file(&path, bytes)?;
        let mut identity = identity_for_bytes(bytes);
        identity.schema_id = "cargo-allow.tool-identity.v0".to_string();
        let failure = select_tool(
            &request(
                &path,
                ToolSelectionMode::ExplicitToolUnderTest,
                &sha256_v1_bytes(bytes),
            ),
            identity,
            &ToolCompatibilityRequirement::current(),
        )
        .err();
        let _ = fs::remove_file(&path);
        let Some(failure) = failure else {
            return Err("malformed tool identity was accepted".into());
        };
        if failure.result != ToolResultClass::MalformedToolIdentity {
            return Err("malformed identity returned the wrong result class".into());
        }
        Ok(())
    }

    #[test]
    fn precommit_installed_pinned_release_is_selectable() -> Result<(), Box<dyn Error>> {
        let path = selected_path("installed-pinned");
        let bytes = b"published pinned tool";
        write_selected_file(&path, bytes)?;
        let mut identity = identity_for_bytes(bytes);
        identity.channel = ToolChannel::PublishedRelease;
        let receipt = select_tool(
            &request(
                &path,
                ToolSelectionMode::InstalledPinned,
                &sha256_v1_bytes(bytes),
            ),
            identity,
            &ToolCompatibilityRequirement::current(),
        )?;
        let _ = fs::remove_file(&path);
        if receipt.result != ToolResultClass::ToolPrebuiltAndSelected || receipt.preview_evidence {
            return Err(
                "published installed-pinned selection was not classified as released".into(),
            );
        }
        Ok(())
    }

    #[test]
    fn precommit_candidate_schema_generation_is_rejected() -> Result<(), Box<dyn Error>> {
        let path = selected_path("candidate-schema");
        let bytes = b"schema generation tool";
        write_selected_file(&path, bytes)?;
        let mut requirement = ToolCompatibilityRequirement::current();
        requirement.schema_generation = "cargo-allow.spec-system.v2".to_string();
        let failure = select_tool(
            &request(
                &path,
                ToolSelectionMode::ExplicitToolUnderTest,
                &sha256_v1_bytes(bytes),
            ),
            identity_for_bytes(bytes),
            &requirement,
        )
        .err();
        let _ = fs::remove_file(&path);
        let Some(failure) = failure else {
            return Err("unsupported candidate schema generation was accepted".into());
        };
        if failure.result != ToolResultClass::CandidateSchemaUnsupported {
            return Err("candidate schema mismatch returned the wrong result class".into());
        }
        Ok(())
    }

    #[test]
    fn explicit_tool_without_authorization_is_not_green() -> Result<(), Box<dyn Error>> {
        let path = selected_path("unauthorized");
        let bytes = b"unauthorized prebuilt tool";
        write_selected_file(&path, bytes)?;
        let mut request = request(
            &path,
            ToolSelectionMode::ExplicitToolUnderTest,
            &sha256_v1_bytes(bytes),
        );
        request.preview_authorized = false;
        let failure = select_tool(
            &request,
            identity_for_bytes(bytes),
            &ToolCompatibilityRequirement::current(),
        )
        .err();
        let _ = fs::remove_file(&path);
        let Some(failure) = failure else {
            return Err("unauthorized preview tool was accepted".into());
        };
        if failure.result != ToolResultClass::PreviewToolNotAuthorized {
            return Err("unauthorized preview tool returned the wrong result class".into());
        }
        Ok(())
    }
}
