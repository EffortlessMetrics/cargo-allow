//! Authored evidence-anchor resolution through the neutral Rust source index.
//!
//! The compiler owns the meaning of an authored evidence anchor. The shared
//! index supplies structural subject identity and ambiguity only; it does not
//! decide intent graph validity, evidence sufficiency, or proof currentness.

use effortless_rust_source_index::{
    RustTestInventory, RustTestResolution, RustTestSelector, RustTestSubject,
    RustTestTargetIdentity, RustTestTargetKind, resolve_rust_test_selector,
};
use intent_model::EvidenceSubjectRegistration;

/// Intent-owned interpretation of one authored Rust evidence anchor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntentSubjectResolutionClassV1 {
    ExactCurrent,
    ExactBodyChanged,
    Ambiguous,
    Missing,
    Ignored,
    GeneratedOrParameterized,
    ConditionalUnknown,
    PartialInventory,
    MalformedAnchor,
}

impl IntentSubjectResolutionClassV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExactCurrent => "exact_current",
            Self::ExactBodyChanged => "exact_body_changed",
            Self::Ambiguous => "ambiguous",
            Self::Missing => "missing",
            Self::Ignored => "ignored",
            Self::GeneratedOrParameterized => "generated_or_parameterized",
            Self::ConditionalUnknown => "conditional_unknown",
            Self::PartialInventory => "partial_inventory",
            Self::MalformedAnchor => "malformed_anchor",
        }
    }
}

/// Product projection of a neutral structural resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntentSubjectResolutionV1 {
    pub subject_id: String,
    pub class: IntentSubjectResolutionClassV1,
    pub selector: Option<String>,
    pub source_path: Option<String>,
    pub observed_body_identity: Option<String>,
    pub candidates: Vec<String>,
    pub limitations: Vec<String>,
}

/// Resolve an authored evidence subject against an exact supplied inventory.
///
/// This function does not read the repository, invoke Cargo, or treat a
/// partial inventory as missing evidence. Product semantics are applied only
/// after the neutral index has returned a structural result.
pub fn resolve_authored_rust_subject(
    anchor: &EvidenceSubjectRegistration,
    inventory: &RustTestInventory,
) -> IntentSubjectResolutionV1 {
    let subject_id = anchor.id.as_str().to_string();
    let Some(selector) = selector_from_anchor(anchor) else {
        return IntentSubjectResolutionV1 {
            subject_id,
            class: IntentSubjectResolutionClassV1::MalformedAnchor,
            selector: None,
            source_path: None,
            observed_body_identity: None,
            candidates: Vec::new(),
            limitations: vec!["authored target must use lib:, bin:, or test: identity".to_string()],
        };
    };
    let display = selector.display_name();

    match resolve_rust_test_selector(inventory, &selector) {
        RustTestResolution::ResolvedExact(subject) => {
            exact_resolution(anchor, subject_id, display, subject)
        }
        RustTestResolution::Ambiguous(candidates) => IntentSubjectResolutionV1 {
            subject_id,
            class: IntentSubjectResolutionClassV1::Ambiguous,
            selector: Some(display),
            source_path: None,
            observed_body_identity: None,
            candidates: candidates
                .into_iter()
                .map(|candidate| candidate.display_name())
                .collect(),
            limitations: Vec::new(),
        },
        RustTestResolution::NotFound => IntentSubjectResolutionV1 {
            subject_id,
            class: IntentSubjectResolutionClassV1::Missing,
            selector: Some(display),
            source_path: None,
            observed_body_identity: None,
            candidates: Vec::new(),
            limitations: Vec::new(),
        },
        RustTestResolution::Ignored(subject) => classified_subject(
            subject_id,
            display,
            subject,
            IntentSubjectResolutionClassV1::Ignored,
        ),
        RustTestResolution::GeneratedOrParameterized(subject) => classified_subject(
            subject_id,
            display,
            subject,
            IntentSubjectResolutionClassV1::GeneratedOrParameterized,
        ),
        RustTestResolution::CfgOrFeatureUnknown(subject) => classified_subject(
            subject_id,
            display,
            subject,
            IntentSubjectResolutionClassV1::ConditionalUnknown,
        ),
        RustTestResolution::PartialInventory => IntentSubjectResolutionV1 {
            subject_id,
            class: IntentSubjectResolutionClassV1::PartialInventory,
            selector: Some(display),
            source_path: None,
            observed_body_identity: None,
            candidates: Vec::new(),
            limitations: vec!["structural inventory is partial".to_string()],
        },
        RustTestResolution::MalformedSelector => IntentSubjectResolutionV1 {
            subject_id,
            class: IntentSubjectResolutionClassV1::MalformedAnchor,
            selector: Some(display),
            source_path: None,
            observed_body_identity: None,
            candidates: Vec::new(),
            limitations: vec!["authored structural selector is malformed".to_string()],
        },
    }
}

fn selector_from_anchor(anchor: &EvidenceSubjectRegistration) -> Option<RustTestSelector> {
    let (kind, target_name) = anchor.target.split_once(':')?;
    let kind = match kind {
        "lib" => RustTestTargetKind::Library,
        "bin" => RustTestTargetKind::Binary,
        "test" => RustTestTargetKind::IntegrationTest,
        _ => return None,
    };
    let module_path = anchor
        .module_path
        .split("::")
        .filter(|segment| !segment.is_empty())
        .map(str::to_string)
        .collect();
    let selector = RustTestSelector {
        package: anchor.package.clone(),
        target: RustTestTargetIdentity {
            kind,
            name: target_name.to_string(),
        },
        module_path,
        function: anchor.test_name.clone(),
    };
    selector.validate().then_some(selector)
}

fn exact_resolution(
    anchor: &EvidenceSubjectRegistration,
    subject_id: String,
    display: String,
    subject: RustTestSubject,
) -> IntentSubjectResolutionV1 {
    let class = if subject.body_identity == anchor.source_identity {
        IntentSubjectResolutionClassV1::ExactCurrent
    } else {
        IntentSubjectResolutionClassV1::ExactBodyChanged
    };
    classified_subject(subject_id, display, subject, class)
}

fn classified_subject(
    subject_id: String,
    display: String,
    subject: RustTestSubject,
    class: IntentSubjectResolutionClassV1,
) -> IntentSubjectResolutionV1 {
    IntentSubjectResolutionV1 {
        subject_id,
        class,
        selector: Some(display),
        source_path: Some(subject.source_path),
        observed_body_identity: Some(subject.body_identity),
        candidates: Vec::new(),
        limitations: subject.limitations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use effortless_rust_source_index::{
        RustTestInventoryDiagnostic, RustTestInventoryStatus, RustTestSourceRange,
    };
    use intent_model::{EvidenceSubjectId, EvidenceSubjectRole, SourceLocation};

    #[test]
    fn exact_authored_anchor_resolves_without_product_findings() {
        let anchor = anchor("fnv1a64:current");
        let inventory = inventory(false, false, false);
        let resolution = resolve_authored_rust_subject(&anchor, &inventory);
        assert_eq!(
            resolution.class,
            IntentSubjectResolutionClassV1::ExactCurrent
        );
        assert_eq!(resolution.source_path.as_deref(), Some("src/lib.rs"));
    }

    #[test]
    fn body_identity_change_is_not_promoted_to_current() {
        let anchor = anchor("fnv1a64:old");
        let inventory = inventory(false, false, false);
        let resolution = resolve_authored_rust_subject(&anchor, &inventory);
        assert_eq!(
            resolution.class,
            IntentSubjectResolutionClassV1::ExactBodyChanged
        );
    }

    #[test]
    fn partial_inventory_remains_not_proven() {
        let mut inventory = inventory(false, false, false);
        inventory.status = RustTestInventoryStatus::Partial;
        inventory.diagnostics.push(RustTestInventoryDiagnostic {
            kind: effortless_rust_source_index::RustTestInventoryDiagnosticKind::SourceParseFailed,
            path: Some("src/lib.rs".to_string()),
            message: "parse error".to_string(),
        });
        let resolution = resolve_authored_rust_subject(&anchor("fnv1a64:current"), &inventory);
        assert_eq!(
            resolution.class,
            IntentSubjectResolutionClassV1::PartialInventory
        );
    }

    fn anchor(source_identity: &str) -> EvidenceSubjectRegistration {
        EvidenceSubjectRegistration {
            id: EvidenceSubjectId("subject:demo:alpha".to_string()),
            role: EvidenceSubjectRole::ExactEvidence,
            package: "demo".to_string(),
            target: "lib:demo".to_string(),
            module_path: "tests".to_string(),
            test_name: "alpha".to_string(),
            source: SourceLocation::new("src/lib.rs"),
            source_identity: source_identity.to_string(),
        }
    }

    fn inventory(
        ignored: bool,
        generated_or_parameterized: bool,
        cfg_or_feature_unknown: bool,
    ) -> RustTestInventory {
        RustTestInventory {
            subjects: vec![RustTestSubject {
                selector: RustTestSelector {
                    package: "demo".to_string(),
                    target: RustTestTargetIdentity {
                        kind: RustTestTargetKind::Library,
                        name: "demo".to_string(),
                    },
                    module_path: vec!["tests".to_string()],
                    function: "alpha".to_string(),
                },
                source_path: "src/lib.rs".to_string(),
                source_range: RustTestSourceRange {
                    start_line: 1,
                    start_column: 1,
                    end_line: 3,
                    end_column: 2,
                },
                body_identity: "fnv1a64:current".to_string(),
                attributes: vec!["test".to_string()],
                generated_or_parameterized,
                cfg_or_feature_unknown,
                ignored,
                limitations: Vec::new(),
            }],
            status: RustTestInventoryStatus::Complete,
            diagnostics: Vec::new(),
        }
    }
}
