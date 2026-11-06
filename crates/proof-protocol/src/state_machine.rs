//! Identity carried by state-machine evidence, not a state-machine language.
//!
//! Native providers own model syntax, scope, results and trust semantics. This
//! contract keeps their load-bearing identities separate so model checking can
//! never be silently substituted for evidence about a production implementation.

use serde::{Deserialize, Serialize};

pub const STATE_MACHINE_EVIDENCE_BINDING_SCHEMA_ID: &str =
    "proof.state-machine-evidence-binding.v1";

/// The kind of evidence requested or captured. No variant implies that a
/// provider ran, completed its search, or established the requested property.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateMachineEvidenceKindV1 {
    ModelCheck,
    ModelProof,
    ImplementationConformance,
    ImplementationProof,
    ScheduleExploration,
}

impl StateMachineEvidenceKindV1 {
    pub const fn binds_implementation(self) -> bool {
        matches!(
            self,
            Self::ImplementationConformance | Self::ImplementationProof | Self::ScheduleExploration
        )
    }
}

/// Expected or observed identity for one machine/property evidence edge.
///
/// Digests use the shared `sha256:v1:` format. Their preimages are retained
/// provider-owned artifacts, not labels or a new generic specification DSL.
/// Matching these declarations establishes structural binding only; the native
/// provider must still validate execution, currentness, scope and result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateMachineEvidenceBindingV1 {
    pub schema_id: String,
    pub machine_id: String,
    pub property_id: String,
    pub evidence_kind: StateMachineEvidenceKindV1,
    /// Content-complete model and imported-module closure.
    pub model_digest: String,
    /// Exact selected property, including its required claim strength.
    pub property_digest: String,
    /// Explicit environment, fairness, abstraction and trusted-code assumptions.
    pub assumptions_digest: String,
    /// Native search/proof scope: domains, bounds, fairness, scheduler settings
    /// and completion obligations. No bound is inferred from the evidence kind.
    pub scope_digest: String,
    /// Exact prover, solver, compiler, target and adapter-generation identities
    /// used by the provider, including applicable dependency/configuration data.
    pub toolchain_digest: String,
    /// Production implementation and its load-bearing dependency closure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub implementation_digest: Option<String>,
    /// The actual implementation projection/driver or verification harness.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapter_digest: Option<String>,
}

impl StateMachineEvidenceBindingV1 {
    /// Structural validation only. In particular, a syntactically valid digest
    /// is not evidence that its preimage exists or that a native proof ran.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != STATE_MACHINE_EVIDENCE_BINDING_SCHEMA_ID {
            return Err(format!(
                "unexpected state-machine binding schema_id {}",
                self.schema_id
            ));
        }
        for (name, identity) in [
            ("machine_id", self.machine_id.as_str()),
            ("property_id", self.property_id.as_str()),
        ] {
            if identity.is_empty()
                || !identity.chars().all(|value| {
                    value.is_ascii_alphanumeric() || matches!(value, '_' | '-' | '.' | ':' | '/')
                })
            {
                return Err(format!("state-machine {name} is not a stable identifier"));
            }
        }
        for (name, digest) in [
            ("model_digest", self.model_digest.as_str()),
            ("property_digest", self.property_digest.as_str()),
            ("assumptions_digest", self.assumptions_digest.as_str()),
            ("scope_digest", self.scope_digest.as_str()),
            ("toolchain_digest", self.toolchain_digest.as_str()),
        ] {
            validate_digest(name, digest)?;
        }
        match (&self.implementation_digest, &self.adapter_digest) {
            (Some(implementation), Some(adapter)) if self.evidence_kind.binds_implementation() => {
                validate_digest("implementation_digest", implementation)?;
                validate_digest("adapter_digest", adapter)?;
            }
            (None, None) if !self.evidence_kind.binds_implementation() => {}
            _ => {
                return Err(
                    "state-machine implementation evidence requires both implementation_digest and adapter_digest; model-only evidence must omit both"
                        .to_string(),
                );
            }
        }
        Ok(())
    }
}

fn validate_digest(name: &str, digest: &str) -> Result<(), String> {
    let Some(hex) = digest.strip_prefix("sha256:v1:") else {
        return Err(format!("state-machine {name} must use sha256:v1"));
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|value| value.is_ascii_digit() || matches!(value, b'a'..=b'f'))
    {
        return Err(format!(
            "state-machine {name} must contain 64 lowercase hexadecimal digits"
        ));
    }
    Ok(())
}
