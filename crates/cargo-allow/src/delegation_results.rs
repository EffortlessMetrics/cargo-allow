//! Typed delegation result classes for spec-system delegation (#3366).
//!
//! Each result class is independently testable and maps to a stable error code.
//! No result class silently falls back to the embedded evaluator.

use allow_core::CargoAllowError;

/// Stable error codes for delegation failures (#2901 / #3366).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DelegationResultCode {
    /// Delegation completed successfully with findings.
    DelegatedCompleted,
    /// Delegation completed with actionable findings.
    DelegatedFindings,
    /// Provider binary not found or not executable.
    ProviderUnavailable,
    /// Provider version or protocol mismatch.
    ProviderIncompatible,
    /// Provider requires a newer generation than configured.
    LegacyGenerationUnsupported,
    /// Provider output failed to parse or validate.
    MalformedProviderOutput,
    /// Provider timed out or crashed during execution.
    ProviderInstrumentFailure,
    /// Source tree identity changed between scan and delegation.
    SourceIdentityMismatch,
    /// Input was modified after delegation started.
    StaleInput,
    /// The compatibility operation has been removed in this version.
    CompatibilityOperationRemoved,
    /// Migration is required before delegation can proceed.
    MigrationRequired,
}

impl DelegationResultCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DelegatedCompleted => "delegated_completed",
            Self::DelegatedFindings => "delegated_findings",
            Self::ProviderUnavailable => "provider_unavailable",
            Self::ProviderIncompatible => "provider_incompatible",
            Self::LegacyGenerationUnsupported => "legacy_generation_unsupported",
            Self::MalformedProviderOutput => "malformed_provider_output",
            Self::ProviderInstrumentFailure => "provider_instrument_failure",
            Self::SourceIdentityMismatch => "source_identity_mismatch",
            Self::StaleInput => "stale_input",
            Self::CompatibilityOperationRemoved => "compatibility_operation_removed",
            Self::MigrationRequired => "migration_required",
        }
    }

    /// Map to CargoAllowErrorKind for typed error propagation.
    pub fn error_kind(self) -> allow_core::CargoAllowErrorKind {
        match self {
            Self::DelegatedCompleted | Self::DelegatedFindings => {
                allow_core::CargoAllowErrorKind::Unknown
            }
            Self::ProviderUnavailable | Self::ProviderInstrumentFailure => {
                allow_core::CargoAllowErrorKind::InstrumentFailure
            }
            Self::ProviderIncompatible
            | Self::LegacyGenerationUnsupported
            | Self::CompatibilityOperationRemoved => allow_core::CargoAllowErrorKind::Unsupported,
            Self::MalformedProviderOutput => allow_core::CargoAllowErrorKind::Artifact,
            Self::SourceIdentityMismatch | Self::StaleInput => {
                allow_core::CargoAllowErrorKind::InvalidConfig
            }
            Self::MigrationRequired => allow_core::CargoAllowErrorKind::Unsupported,
        }
    }

    /// Convert to a CargoAllowError with context.
    pub fn to_error(self, context: impl Into<String>) -> CargoAllowError {
        CargoAllowError::with_kind(
            self.error_kind(),
            format!("{}: {}", self.as_str(), context.into()),
        )
    }
}

/// The outcome of a delegation attempt.
#[derive(Debug)]
pub enum DelegationOutcome {
    /// Delegation completed; carries the result code and optional finding count.
    Completed {
        code: DelegationResultCode,
        finding_count: usize,
    },
    /// Delegation failed; carries the typed result code and error.
    Failed {
        code: DelegationResultCode,
        error: CargoAllowError,
    },
}

impl DelegationOutcome {
    /// True if delegation succeeded (completed, not failed).
    pub fn is_completed(&self) -> bool {
        matches!(self, Self::Completed { .. })
    }

    /// True if delegation failed.
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }

    /// The stable result code.
    pub fn code(&self) -> DelegationResultCode {
        match self {
            Self::Completed { code, .. } | Self::Failed { code, .. } => *code,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_codes_are_distinct_strings() {
        let codes = [
            DelegationResultCode::DelegatedCompleted,
            DelegationResultCode::DelegatedFindings,
            DelegationResultCode::ProviderUnavailable,
            DelegationResultCode::ProviderIncompatible,
            DelegationResultCode::LegacyGenerationUnsupported,
            DelegationResultCode::MalformedProviderOutput,
            DelegationResultCode::ProviderInstrumentFailure,
            DelegationResultCode::SourceIdentityMismatch,
            DelegationResultCode::StaleInput,
            DelegationResultCode::CompatibilityOperationRemoved,
            DelegationResultCode::MigrationRequired,
        ];
        let strings: Vec<&str> = codes.iter().map(|c| c.as_str()).collect();
        let unique: std::collections::BTreeSet<&str> = strings.iter().copied().collect();
        assert_eq!(strings.len(), unique.len(), "result codes must be distinct");
        assert_eq!(codes.len(), 11, "exactly 11 result codes required");
    }

    #[test]
    fn provider_unavailable_is_not_clean() {
        let code = DelegationResultCode::ProviderUnavailable;
        assert_ne!(code.error_kind(), allow_core::CargoAllowErrorKind::Unknown);
        let outcome = DelegationOutcome::Failed {
            code,
            error: code.to_error("cargo-intent not found"),
        };
        assert!(outcome.is_failed());
        assert!(!outcome.is_completed());
    }

    #[test]
    fn incompatible_maps_to_unsupported() {
        assert_eq!(
            DelegationResultCode::ProviderIncompatible.error_kind(),
            allow_core::CargoAllowErrorKind::Unsupported
        );
    }

    #[test]
    fn malformed_output_maps_to_artifact() {
        assert_eq!(
            DelegationResultCode::MalformedProviderOutput.error_kind(),
            allow_core::CargoAllowErrorKind::Artifact
        );
    }

    #[test]
    fn instrument_failure_maps_to_instrument() {
        assert_eq!(
            DelegationResultCode::ProviderInstrumentFailure.error_kind(),
            allow_core::CargoAllowErrorKind::InstrumentFailure
        );
    }

    #[test]
    fn source_mismatch_maps_to_invalid_config() {
        assert_eq!(
            DelegationResultCode::SourceIdentityMismatch.error_kind(),
            allow_core::CargoAllowErrorKind::InvalidConfig
        );
    }

    #[test]
    fn stale_input_maps_to_invalid_config() {
        assert_eq!(
            DelegationResultCode::StaleInput.error_kind(),
            allow_core::CargoAllowErrorKind::InvalidConfig
        );
    }

    #[test]
    fn completed_carries_finding_count() {
        let outcome = DelegationOutcome::Completed {
            code: DelegationResultCode::DelegatedFindings,
            finding_count: 3,
        };
        assert!(outcome.is_completed());
        assert_eq!(outcome.code(), DelegationResultCode::DelegatedFindings);
        let finding_count = match outcome {
            DelegationOutcome::Completed { finding_count, .. } => Some(finding_count),
            DelegationOutcome::Failed { .. } => None,
        };
        assert_eq!(finding_count, Some(3));
    }

    #[test]
    fn error_includes_code_string() {
        let error = DelegationResultCode::ProviderIncompatible.to_error("version 0.1 vs 0.2");
        let expected_error = error.to_string();
        let outcome = DelegationOutcome::Failed {
            code: DelegationResultCode::ProviderIncompatible,
            error,
        };
        let msg = match outcome {
            DelegationOutcome::Failed { error, .. } => error.to_string(),
            DelegationOutcome::Completed { .. } => String::new(),
        };
        assert!(msg.contains("provider_incompatible"), "msg: {msg}");
        assert!(msg.contains("version 0.1 vs 0.2"), "msg: {msg}");
        assert_eq!(msg, expected_error);
    }
}
