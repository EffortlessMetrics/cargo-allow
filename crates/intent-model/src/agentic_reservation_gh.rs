//! Thin `gh` CLI adapter for the candidate-ref transport boundary (#3975).
//!
//! Process spawn only: every transport operation builds one `gh api`
//! invocation from typed arguments, spawns it, and maps the exit status and
//! output bytes into the typed transport vocabulary. Unit tests exercise
//! argument construction and output parsing against fixture strings only;
//! no live GitHub network behavior is part of this crate's test suite.
//!
//! Failure classification is conservative and text-signature based over the
//! lowercased stderr: rate-limit/abuse and 429 signatures classify
//! `RateLimitedOrAbuseProtected`, `404` classifies `ValidationRejected`
//! (read operations map it to an absent ref before this), other `400`, `409`,
//! `422`, and validation signatures classify `ValidationRejected`, plain
//! `403`, `5xx`, connection, and timeout signatures classify
//! `ProviderUnavailable`, and everything else — including an exit status
//! without a code (terminated by a signal) — classifies
//! `InstrumentFailure`. Response text alone never grants ownership; the
//! transition's read-back decides classification.

use std::process::Command;

use serde::Deserialize;

use crate::agentic_reservation::{
    CandidateAnchorReadBackV1, CandidateRefTransport, CreateRefCommandV1, CreateRefOutcomeV1,
    RefReadBackV1, TransportFailureV1, validate_object_id,
};

/// Default external program invoked by this adapter.
pub const GH_PROGRAM: &str = "gh";

/// Minimum arguments every invocation shares.
fn base_args() -> Vec<String> {
    vec!["api".into()]
}

#[derive(Deserialize)]
struct GhRefPayload {
    object: GhRefObjectPayload,
}

#[derive(Deserialize)]
struct GhRefObjectPayload {
    sha: String,
}

#[derive(Deserialize)]
struct GhPullPayload {
    base: GhPullBasePayload,
}

#[derive(Deserialize)]
struct GhPullBasePayload {
    sha: String,
}

/// `gh` CLI transport for one repository. Requires only the narrow ability
/// to read repository refs/candidate state and create the selected canonical
/// ref; it never deletes, retargets, or mutates anything else.
#[derive(Debug, Clone)]
pub struct GhCandidateRefTransport {
    program: String,
    repository: String,
}

impl GhCandidateRefTransport {
    pub fn new(repository: impl Into<String>) -> Self {
        Self {
            program: GH_PROGRAM.into(),
            repository: repository.into(),
        }
    }

    /// Override the spawned program (used by tests to prove spawn failures
    /// map to a typed transport failure without touching the network).
    pub fn with_program(repository: impl Into<String>, program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            repository: repository.into(),
        }
    }

    pub fn repository(&self) -> &str {
        &self.repository
    }

    /// Build the `gh api` arguments for the create-only ref creation:
    /// `gh api --method POST repos/<repository>/git/refs --raw-field
    /// ref=<fully qualified ref> --raw-field sha=<exact base>`.
    pub fn create_ref_args(&self, command: &CreateRefCommandV1) -> Result<Vec<String>, String> {
        if command.repository != self.repository {
            return Err(format!(
                "adapter is bound to repository {} and cannot create refs in {}",
                self.repository, command.repository
            ));
        }
        validate_object_id(&command.target_sha, "target_sha")?;
        if !command.reference.starts_with("refs/heads/") {
            return Err("candidate ref must be fully qualified under refs/heads/".into());
        }
        let mut args = base_args();
        args.push("--method".into());
        args.push("POST".into());
        args.push(format!("repos/{}/git/refs", command.repository));
        args.push("--raw-field".into());
        args.push(format!("ref={}", command.reference));
        args.push("--raw-field".into());
        args.push(format!("sha={}", command.target_sha));
        Ok(args)
    }

    /// Build the `gh api` arguments for one exact ref read-back:
    /// `gh api repos/<repository>/git/ref/<escaped fully qualified ref>`.
    /// Slash separators of the ref are percent-escaped as the API requires.
    pub fn read_ref_args(&self, repository: &str, reference: &str) -> Result<Vec<String>, String> {
        if repository != self.repository {
            return Err(format!(
                "adapter is bound to repository {} and cannot read refs in {repository}",
                self.repository
            ));
        }
        if !reference.starts_with("refs/heads/") {
            return Err("candidate ref must be fully qualified under refs/heads/".into());
        }
        let escaped = reference.replace('/', "%2F");
        let mut args = base_args();
        args.push(format!("repos/{repository}/git/ref/{escaped}"));
        Ok(args)
    }

    /// Build the `gh api` arguments for the candidate-anchor read-back:
    /// `gh api repos/<repository>/pulls -f head=<owner>:<branch>
    /// -f state=open -f per_page=1`. For GET requests `gh api` turns field
    /// flags into query parameters.
    pub fn read_candidate_anchor_args(
        &self,
        repository: &str,
        reference: &str,
    ) -> Result<Vec<String>, String> {
        if repository != self.repository {
            return Err(format!(
                "adapter is bound to repository {} and cannot read candidates in {repository}",
                self.repository
            ));
        }
        let branch = reference
            .strip_prefix("refs/heads/")
            .ok_or_else(|| "candidate ref must be fully qualified under refs/heads/".to_string())?;
        let mut segments = repository.split('/');
        let owner = segments.next().unwrap_or_default();
        let name = segments.next().unwrap_or_default();
        if owner.is_empty() || name.is_empty() || segments.next().is_some() {
            return Err("repository must have the owner/name form".into());
        }
        let mut args = base_args();
        args.push(format!("repos/{repository}/pulls"));
        args.push("-f".into());
        args.push(format!("head={owner}:{branch}"));
        args.push("-f".into());
        args.push("state=open".into());
        args.push("-f".into());
        args.push("per_page=1".into());
        Ok(args)
    }

    /// Map one create-ref invocation into the typed create-only outcome.
    /// `already exists` stderr signatures classify as [`CreateRefOutcomeV1::AlreadyExists`]
    /// evidence; the caller still owes an exact read-back.
    pub fn parse_create_ref_response(
        &self,
        exit_code: Option<i32>,
        stdout: &str,
        stderr: &str,
    ) -> Result<CreateRefOutcomeV1, TransportFailureV1> {
        if exit_code == Some(0) {
            let payload: GhRefPayload = serde_json::from_str(stdout).map_err(|error| {
                TransportFailureV1::InstrumentFailure(format!(
                    "create-ref output is not decodable: {error}"
                ))
            })?;
            if let Err(reason) = validate_object_id(&payload.object.sha, "created ref object") {
                return Err(TransportFailureV1::InstrumentFailure(reason));
            }
            return Ok(CreateRefOutcomeV1::Created);
        }
        let stderr_lower = stderr.to_ascii_lowercase();
        if stderr_lower.contains("already exists") {
            return Ok(CreateRefOutcomeV1::AlreadyExists);
        }
        Err(classify_failure(exit_code, &stderr_lower))
    }

    /// Map one ref read-back invocation into absent or exact ref facts.
    /// `404`/`not found` signatures are a legitimate absent answer.
    pub fn parse_read_ref_response(
        &self,
        exit_code: Option<i32>,
        stdout: &str,
        stderr: &str,
    ) -> Result<Option<RefReadBackV1>, TransportFailureV1> {
        if exit_code == Some(0) {
            let payload: GhRefPayload = serde_json::from_str(stdout).map_err(|error| {
                TransportFailureV1::InstrumentFailure(format!(
                    "read-ref output is not decodable: {error}"
                ))
            })?;
            if let Err(reason) = validate_object_id(&payload.object.sha, "read-back object") {
                return Err(TransportFailureV1::InstrumentFailure(reason));
            }
            return Ok(Some(RefReadBackV1 {
                target_sha: payload.object.sha,
            }));
        }
        let stderr_lower = stderr.to_ascii_lowercase();
        if stderr_lower.contains("http 404") || stderr_lower.contains("not found") {
            return Ok(None);
        }
        if stderr_lower.contains("already exists") {
            return Err(TransportFailureV1::InstrumentFailure(
                "read-back cannot already exist".into(),
            ));
        }
        Err(classify_failure(exit_code, &stderr_lower))
    }

    /// Map one candidate-anchor invocation into candidate facts. An empty
    /// list is a legitimate absent anchor.
    pub fn parse_candidate_anchor_response(
        &self,
        exit_code: Option<i32>,
        stdout: &str,
        stderr: &str,
    ) -> Result<CandidateAnchorReadBackV1, TransportFailureV1> {
        if exit_code == Some(0) {
            let payloads: Vec<GhPullPayload> = serde_json::from_str(stdout).map_err(|error| {
                TransportFailureV1::InstrumentFailure(format!(
                    "candidate-anchor output is not decodable: {error}"
                ))
            })?;
            return Ok(match payloads.first() {
                Some(payload) => CandidateAnchorReadBackV1 {
                    candidate_exists: true,
                    candidate_base: payload.base.sha.clone(),
                },
                None => CandidateAnchorReadBackV1::default(),
            });
        }
        let stderr_lower = stderr.to_ascii_lowercase();
        Err(classify_failure(exit_code, &stderr_lower))
    }

    fn run(&self, args: &[String]) -> Result<(Option<i32>, String, String), TransportFailureV1> {
        let output = Command::new(&self.program)
            .args(args)
            .output()
            .map_err(|error| {
                TransportFailureV1::ProviderUnavailable(format!(
                    "failed to spawn {}: {error}",
                    self.program
                ))
            })?;
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        Ok((output.status.code(), stdout, stderr))
    }
}

impl CandidateRefTransport for GhCandidateRefTransport {
    fn create_ref(
        &mut self,
        command: &CreateRefCommandV1,
    ) -> Result<CreateRefOutcomeV1, TransportFailureV1> {
        let args = self
            .create_ref_args(command)
            .map_err(TransportFailureV1::ValidationRejected)?;
        let (exit_code, stdout, stderr) = self.run(&args)?;
        self.parse_create_ref_response(exit_code, &stdout, &stderr)
    }

    fn read_ref(
        &mut self,
        repository: &str,
        reference: &str,
    ) -> Result<Option<RefReadBackV1>, TransportFailureV1> {
        let args = self
            .read_ref_args(repository, reference)
            .map_err(TransportFailureV1::ValidationRejected)?;
        let (exit_code, stdout, stderr) = self.run(&args)?;
        self.parse_read_ref_response(exit_code, &stdout, &stderr)
    }

    fn read_candidate_anchor(
        &mut self,
        repository: &str,
        reference: &str,
    ) -> Result<CandidateAnchorReadBackV1, TransportFailureV1> {
        let args = self
            .read_candidate_anchor_args(repository, reference)
            .map_err(TransportFailureV1::ValidationRejected)?;
        let (exit_code, stdout, stderr) = self.run(&args)?;
        self.parse_candidate_anchor_response(exit_code, &stdout, &stderr)
    }
}

/// Map one non-zero `gh api` invocation into the provider-failure vocabulary
/// using lowercased stderr text signatures. See the module documentation for
/// the ordered rules.
fn classify_failure(exit_code: Option<i32>, stderr_lower: &str) -> TransportFailureV1 {
    let detail = format!("gh api failed (exit {exit_code:?}): {stderr_lower}");
    let rate_limited = ["rate limit", "secondary rate", "abuse", "http 429"]
        .iter()
        .any(|signature| stderr_lower.contains(signature));
    if rate_limited {
        return TransportFailureV1::RateLimitedOrAbuseProtected(detail);
    }
    let validation = ["http 404", "http 400", "http 409", "http 422", "validation"]
        .iter()
        .any(|signature| stderr_lower.contains(signature));
    if validation {
        return TransportFailureV1::ValidationRejected(detail);
    }
    let unavailable = [
        "http 403",
        "http 500",
        "http 502",
        "http 503",
        "http 504",
        "connection",
        "unable to connect",
        "timed out",
        "temporarily unavailable",
    ]
    .iter()
    .any(|signature| stderr_lower.contains(signature));
    if unavailable {
        return TransportFailureV1::ProviderUnavailable(detail);
    }
    TransportFailureV1::InstrumentFailure(detail)
}

#[cfg(test)]
mod tests {
    use super::*;

    const OWNER: &str = "EffortlessMetrics";
    const REPOSITORY: &str = "EffortlessMetrics/cargo-allow";
    const BASE: &str = "0123456789abcdef0123456789abcdef01234567";
    const REFERENCE: &str = "refs/heads/cargo-allow/claims/0123456789abcdef";

    fn transport() -> GhCandidateRefTransport {
        GhCandidateRefTransport::new(REPOSITORY)
    }

    fn command() -> CreateRefCommandV1 {
        CreateRefCommandV1 {
            repository: REPOSITORY.into(),
            reference: REFERENCE.into(),
            target_sha: BASE.into(),
        }
    }

    const CREATED_BODY: &str = "{\"ref\":\"refs/heads/cargo-allow/claims/0123456789abcdef\",\"node_id\":\"N_1\",\"url\":\"https://api.github.com/repos/EffortlessMetrics/cargo-allow/git/refs/refs%2Fheads%2Fcargo-allow%2Fclaims%2F0123456789abcdef\",\"object\":{\"sha\":\"0123456789abcdef0123456789abcdef01234567\",\"type\":\"commit\",\"url\":\"https://api.github.com/repos/EffortlessMetrics/cargo-allow/git/commits/0123456789abcdef0123456789abcdef01234567\"}}";

    #[test]
    fn create_ref_args_match_the_gh_api_contract() -> Result<(), String> {
        let expected = vec![
            "api".to_string(),
            "--method".to_string(),
            "POST".to_string(),
            format!("repos/{REPOSITORY}/git/refs"),
            "--raw-field".to_string(),
            format!("ref={REFERENCE}"),
            "--raw-field".to_string(),
            format!("sha={BASE}"),
        ];
        assert_eq!(transport().create_ref_args(&command())?, expected);
        Ok(())
    }

    #[test]
    fn create_ref_args_reject_foreign_repository_and_malformed_base() -> Result<(), String> {
        let mut foreign = command();
        foreign.repository = "EffortlessMetrics/other-repo".into();
        assert!(transport().create_ref_args(&foreign).is_err());
        let mut abbreviated = command();
        abbreviated.target_sha = "deadbee".into();
        assert!(transport().create_ref_args(&abbreviated).is_err());
        let mut unqualified = command();
        unqualified.reference = "refs/tags/v1".into();
        assert!(transport().create_ref_args(&unqualified).is_err());
        Ok(())
    }

    #[test]
    fn read_ref_args_escape_the_fully_qualified_ref() -> Result<(), String> {
        let args = transport().read_ref_args(REPOSITORY, REFERENCE)?;
        assert_eq!(
            args,
            vec![
                "api".to_string(),
                format!(
                    "repos/{REPOSITORY}/git/ref/refs%2Fheads%2Fcargo-allow%2Fclaims%2F0123456789abcdef"
                ),
            ]
        );
        assert!(
            transport()
                .read_ref_args("EffortlessMetrics/other-repo", REFERENCE)
                .is_err()
        );
        assert!(transport().read_ref_args(REPOSITORY, "heads/main").is_err());
        Ok(())
    }

    #[test]
    fn candidate_anchor_args_filter_head_state_and_page() -> Result<(), String> {
        let args = transport().read_candidate_anchor_args(REPOSITORY, REFERENCE)?;
        assert_eq!(
            args,
            vec![
                "api".to_string(),
                format!("repos/{REPOSITORY}/pulls"),
                "-f".to_string(),
                format!("head={OWNER}:cargo-allow/claims/0123456789abcdef"),
                "-f".to_string(),
                "state=open".to_string(),
                "-f".to_string(),
                "per_page=1".to_string(),
            ]
        );
        assert!(
            transport()
                .read_candidate_anchor_args("EffortlessMetrics", REFERENCE)
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn create_ref_parses_created_body() {
        let parsed = transport().parse_create_ref_response(Some(0), CREATED_BODY, "");
        assert_eq!(parsed.ok(), Some(CreateRefOutcomeV1::Created));
    }

    #[test]
    fn create_ref_parses_existing_reference_signature() {
        let parsed = transport().parse_create_ref_response(
            Some(1),
            "",
            "gh: Reference already exists (http 422)",
        );
        assert_eq!(parsed.ok(), Some(CreateRefOutcomeV1::AlreadyExists));
    }

    #[test]
    fn create_ref_classifies_validation_rate_limit_and_provider_failures() {
        let validation =
            transport().parse_create_ref_response(Some(1), "", "gh: Validation Failed (http 422)");
        assert!(matches!(
            validation,
            Err(TransportFailureV1::ValidationRejected(_))
        ));
        let rate_limited = transport().parse_create_ref_response(
            Some(1),
            "",
            "gh: API rate limit exceeded for installation (http 403)",
        );
        assert!(matches!(
            rate_limited,
            Err(TransportFailureV1::RateLimitedOrAbuseProtected(_))
        ));
        let abuse = transport().parse_create_ref_response(
            Some(1),
            "",
            "gh: You have triggered an abuse detection rule (http 403)",
        );
        assert!(matches!(
            abuse,
            Err(TransportFailureV1::RateLimitedOrAbuseProtected(_))
        ));
        let unavailable = transport().parse_create_ref_response(
            Some(1),
            "",
            "gh: unable to connect to api.github.com",
        );
        assert!(matches!(
            unavailable,
            Err(TransportFailureV1::ProviderUnavailable(_))
        ));
        let server =
            transport().parse_create_ref_response(Some(1), "", "gh: Server Error (http 502)");
        assert!(matches!(
            server,
            Err(TransportFailureV1::ProviderUnavailable(_))
        ));
        let forbidden = transport().parse_create_ref_response(
            Some(1),
            "",
            "gh: Must have admin rights (http 403)",
        );
        assert!(matches!(
            forbidden,
            Err(TransportFailureV1::ProviderUnavailable(_))
        ));
    }

    #[test]
    fn create_ref_classifies_undecodable_and_signal_answers_as_instrument_failure() {
        let undecodable = transport().parse_create_ref_response(Some(0), "{\"ref\":", "");
        assert!(matches!(
            undecodable,
            Err(TransportFailureV1::InstrumentFailure(_))
        ));
        let abbreviated = transport().parse_create_ref_response(
            Some(0),
            "{\"object\":{\"sha\":\"deadbee\",\"type\":\"commit\"}}",
            "",
        );
        assert!(matches!(
            abbreviated,
            Err(TransportFailureV1::InstrumentFailure(_))
        ));
        let unclassified =
            transport().parse_create_ref_response(Some(1), "", "gh: unexpected content type");
        assert!(matches!(
            unclassified,
            Err(TransportFailureV1::InstrumentFailure(_))
        ));
        let signalled = transport().parse_create_ref_response(None, "", "");
        assert!(matches!(
            signalled,
            Err(TransportFailureV1::InstrumentFailure(_))
        ));
    }

    #[test]
    fn read_ref_parses_present_absent_and_failures() {
        let present = transport().parse_read_ref_response(Some(0), CREATED_BODY, "");
        assert_eq!(
            present.ok(),
            Some(Some(RefReadBackV1 {
                target_sha: BASE.into(),
            }))
        );
        let absent = transport().parse_read_ref_response(Some(1), "", "gh: Not Found (http 404)");
        assert_eq!(absent.ok(), Some(None));
        let rate_limited = transport().parse_read_ref_response(
            Some(1),
            "",
            "gh: API rate limit exceeded (http 403)",
        );
        assert!(matches!(
            rate_limited,
            Err(TransportFailureV1::RateLimitedOrAbuseProtected(_))
        ));
        let undecodable = transport().parse_read_ref_response(Some(0), "not json", "");
        assert!(matches!(
            undecodable,
            Err(TransportFailureV1::InstrumentFailure(_))
        ));
        let incoherent = transport().parse_read_ref_response(
            Some(1),
            "",
            "gh: Reference already exists (http 422)",
        );
        assert!(matches!(
            incoherent,
            Err(TransportFailureV1::InstrumentFailure(_))
        ));
    }

    #[test]
    fn candidate_anchor_parses_present_absent_and_failures() {
        let present_body = "[{\"number\":5,\"title\":\"candidate\",\"base\":{\"sha\":\"fedcba9876543210fedcba9876543210fedcba98\",\"ref\":\"main\"},\"head\":{\"ref\":\"cargo-allow/claims/0123456789abcdef\"}}]";
        let present = transport().parse_candidate_anchor_response(Some(0), present_body, "");
        assert_eq!(
            present.ok(),
            Some(CandidateAnchorReadBackV1 {
                candidate_exists: true,
                candidate_base: "fedcba9876543210fedcba9876543210fedcba98".into(),
            })
        );
        let absent = transport().parse_candidate_anchor_response(Some(0), "[]", "");
        assert_eq!(
            absent.ok(),
            Some(CandidateAnchorReadBackV1 {
                candidate_exists: false,
                candidate_base: String::new(),
            })
        );
        let undecodable = transport().parse_candidate_anchor_response(Some(0), "{\"nope\":1}", "");
        assert!(matches!(
            undecodable,
            Err(TransportFailureV1::InstrumentFailure(_))
        ));
        let unavailable =
            transport().parse_candidate_anchor_response(Some(1), "", "gh: Not Found (http 404)");
        assert!(matches!(
            unavailable,
            Err(TransportFailureV1::ValidationRejected(_))
        ));
    }

    #[test]
    fn missing_program_maps_to_provider_unavailable() {
        let mut adapter =
            GhCandidateRefTransport::with_program(REPOSITORY, "cargo-allow-missing-gh-binary-3975");
        let outcome = adapter.create_ref(&command());
        assert!(matches!(
            outcome,
            Err(TransportFailureV1::ProviderUnavailable(_))
        ));
    }

    #[test]
    fn adapter_rejects_foreign_repository_before_spawning() {
        let mut adapter = transport();
        let mut command = command();
        command.repository = "EffortlessMetrics/other-repo".into();
        let outcome = adapter.create_ref(&command);
        assert!(matches!(
            outcome,
            Err(TransportFailureV1::ValidationRejected(_))
        ));
        let outcome = adapter.read_ref("EffortlessMetrics/other-repo", REFERENCE);
        assert!(matches!(
            outcome,
            Err(TransportFailureV1::ValidationRejected(_))
        ));
    }

    #[test]
    fn repository_accessor_binds_the_adapter() {
        assert_eq!(transport().repository(), REPOSITORY);
    }
}
