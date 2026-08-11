//! Requested/observed Rust subject reconciliation through the neutral index.
//!
//! Structural resolution belongs to effortless-rust-source-index. This module
//! applies proof-owned binding semantics without importing intent graphs or
//! cargo-allow findings.

use effortless_rust_source_index::{
    RustTestInventory, RustTestResolution, RustTestSelector, RustTestSubject,
    resolve_rust_test_selector,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProofSubjectReconciliationClassV1 {
    ExactCurrent,
    RequestedMissing,
    ObservedMissing,
    RequestedAmbiguous,
    ObservedAmbiguous,
    SelectorMismatch,
    BodyIdentityMismatch,
    PartialInventory,
    UnsupportedSubject,
    MalformedSelector,
}

impl ProofSubjectReconciliationClassV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExactCurrent => "exact_current",
            Self::RequestedMissing => "requested_missing",
            Self::ObservedMissing => "observed_missing",
            Self::RequestedAmbiguous => "requested_ambiguous",
            Self::ObservedAmbiguous => "observed_ambiguous",
            Self::SelectorMismatch => "selector_mismatch",
            Self::BodyIdentityMismatch => "body_identity_mismatch",
            Self::PartialInventory => "partial_inventory",
            Self::UnsupportedSubject => "unsupported_subject",
            Self::MalformedSelector => "malformed_selector",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedRustSubjectV1 {
    pub selector: RustTestSelector,
    pub body_identity: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofSubjectReconciliationV1 {
    pub class: ProofSubjectReconciliationClassV1,
    pub requested_selector: String,
    pub observed_selector: String,
    pub resolved_source_path: Option<String>,
    pub resolved_body_identity: Option<String>,
    pub limitations: Vec<String>,
}

/// Reconcile a provider's observed subject against a requested structural
/// selector and the exact inventory used for the proof plan.
pub fn reconcile_rust_subject_binding(
    inventory: &RustTestInventory,
    requested: &RustTestSelector,
    observed: &ObservedRustSubjectV1,
) -> ProofSubjectReconciliationV1 {
    let requested_name = requested.display_name();
    let observed_name = observed.selector.display_name();

    let requested_subject = match resolve_for_proof(inventory, requested, true) {
        Ok(subject) => subject,
        Err(class) => {
            return result(class, requested_name, observed_name, None);
        }
    };
    let observed_subject = match resolve_for_proof(inventory, &observed.selector, false) {
        Ok(subject) => subject,
        Err(class) => {
            return result(class, requested_name, observed_name, None);
        }
    };

    if requested != &observed.selector || requested_subject.selector != observed_subject.selector {
        return result(
            ProofSubjectReconciliationClassV1::SelectorMismatch,
            requested_name,
            observed_name,
            Some(observed_subject),
        );
    }
    if observed.body_identity != observed_subject.body_identity
        || requested_subject.body_identity != observed_subject.body_identity
    {
        return result(
            ProofSubjectReconciliationClassV1::BodyIdentityMismatch,
            requested_name,
            observed_name,
            Some(observed_subject),
        );
    }

    result(
        ProofSubjectReconciliationClassV1::ExactCurrent,
        requested_name,
        observed_name,
        Some(observed_subject),
    )
}

fn resolve_for_proof(
    inventory: &RustTestInventory,
    selector: &RustTestSelector,
    requested: bool,
) -> Result<RustTestSubject, ProofSubjectReconciliationClassV1> {
    match resolve_rust_test_selector(inventory, selector) {
        RustTestResolution::ResolvedExact(subject) => Ok(subject),
        RustTestResolution::NotFound => Err(if requested {
            ProofSubjectReconciliationClassV1::RequestedMissing
        } else {
            ProofSubjectReconciliationClassV1::ObservedMissing
        }),
        RustTestResolution::Ambiguous(_) => Err(if requested {
            ProofSubjectReconciliationClassV1::RequestedAmbiguous
        } else {
            ProofSubjectReconciliationClassV1::ObservedAmbiguous
        }),
        RustTestResolution::PartialInventory => {
            Err(ProofSubjectReconciliationClassV1::PartialInventory)
        }
        RustTestResolution::MalformedSelector => {
            Err(ProofSubjectReconciliationClassV1::MalformedSelector)
        }
        RustTestResolution::Ignored(_)
        | RustTestResolution::GeneratedOrParameterized(_)
        | RustTestResolution::CfgOrFeatureUnknown(_) => {
            Err(ProofSubjectReconciliationClassV1::UnsupportedSubject)
        }
    }
}

fn result(
    class: ProofSubjectReconciliationClassV1,
    requested_selector: String,
    observed_selector: String,
    subject: Option<RustTestSubject>,
) -> ProofSubjectReconciliationV1 {
    let (resolved_source_path, resolved_body_identity, limitations) = match subject {
        Some(subject) => (
            Some(subject.source_path),
            Some(subject.body_identity),
            subject.limitations,
        ),
        None => (None, None, Vec::new()),
    };
    ProofSubjectReconciliationV1 {
        class,
        requested_selector,
        observed_selector,
        resolved_source_path,
        resolved_body_identity,
        limitations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use effortless_rust_source_index::{
        RustTestInventoryStatus, RustTestSourceRange, RustTestTargetIdentity, RustTestTargetKind,
    };

    #[test]
    fn exact_requested_and_observed_subject_is_current() {
        let selector = selector("alpha");
        let inventory = inventory(selector.clone(), "fnv1a64:current");
        let observed = ObservedRustSubjectV1 {
            selector: selector.clone(),
            body_identity: "fnv1a64:current".to_string(),
        };
        let result = reconcile_rust_subject_binding(&inventory, &selector, &observed);
        assert_eq!(
            result.class,
            ProofSubjectReconciliationClassV1::ExactCurrent
        );
    }

    #[test]
    fn observed_body_drift_is_not_current() {
        let selector = selector("alpha");
        let inventory = inventory(selector.clone(), "fnv1a64:current");
        let observed = ObservedRustSubjectV1 {
            selector: selector.clone(),
            body_identity: "fnv1a64:old".to_string(),
        };
        let result = reconcile_rust_subject_binding(&inventory, &selector, &observed);
        assert_eq!(
            result.class,
            ProofSubjectReconciliationClassV1::BodyIdentityMismatch
        );
    }

    #[test]
    fn provider_cannot_substitute_a_different_subject() {
        let requested = selector("alpha");
        let observed_selector = selector("beta");
        let mut inventory = inventory(requested.clone(), "fnv1a64:alpha");
        inventory
            .subjects
            .push(subject(observed_selector.clone(), "fnv1a64:beta"));
        let observed = ObservedRustSubjectV1 {
            selector: observed_selector,
            body_identity: "fnv1a64:beta".to_string(),
        };
        let result = reconcile_rust_subject_binding(&inventory, &requested, &observed);
        assert_eq!(
            result.class,
            ProofSubjectReconciliationClassV1::SelectorMismatch
        );
    }

    fn selector(function: &str) -> RustTestSelector {
        RustTestSelector {
            package: "demo".to_string(),
            target: RustTestTargetIdentity {
                kind: RustTestTargetKind::Library,
                name: "demo".to_string(),
            },
            module_path: vec!["tests".to_string()],
            function: function.to_string(),
        }
    }

    fn subject(selector: RustTestSelector, identity: &str) -> RustTestSubject {
        RustTestSubject {
            selector,
            source_path: "src/lib.rs".to_string(),
            source_range: RustTestSourceRange {
                start_line: 1,
                start_column: 1,
                end_line: 3,
                end_column: 2,
            },
            body_identity: identity.to_string(),
            attributes: vec!["test".to_string()],
            generated_or_parameterized: false,
            cfg_or_feature_unknown: false,
            ignored: false,
            limitations: Vec::new(),
        }
    }

    fn inventory(selector: RustTestSelector, identity: &str) -> RustTestInventory {
        RustTestInventory {
            subjects: vec![subject(selector, identity)],
            status: RustTestInventoryStatus::Complete,
            diagnostics: Vec::new(),
        }
    }
}
