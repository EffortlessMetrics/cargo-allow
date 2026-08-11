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
    let Some(selector) = selector_from_anchor(anchor, inventory) else {
        return IntentSubjectResolutionV1 {
            subject_id,
            class: IntentSubjectResolutionClassV1::MalformedAnchor,
            selector: None,
            source_path: None,
            observed_body_identity: None,
            candidates: Vec::new(),
            limitations: vec![
                "authored target must use lib:, bin:, or integration_test: identity, or match one exact inventory target"
                    .to_string(),
            ],
        };
    };
    let display = selector.display_name();

    match resolve_rust_test_selector(inventory, &selector) {
        RustTestResolution::ResolvedExact(subject) => {
            exact_resolution(anchor, subject_id, display, subject)
        }
        RustTestResolution::Ambiguous(candidate_selectors) => IntentSubjectResolutionV1 {
            subject_id,
            class: IntentSubjectResolutionClassV1::Ambiguous,
            selector: Some(display),
            source_path: None,
            observed_body_identity: None,
            candidates: inventory
                .subjects
                .iter()
                .filter(|subject| {
                    candidate_selectors
                        .iter()
                        .any(|candidate| candidate == &subject.selector)
                })
                .map(ambiguous_candidate_display)
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
            limitations: partial_inventory_limitations(inventory),
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

fn ambiguous_candidate_display(candidate: &RustTestSubject) -> String {
    let selector = candidate.selector.display_name();
    let range = &candidate.source_range;
    format!(
        "{selector} @ {}:{}:{}-{}:{}",
        candidate.source_path,
        range.start_line,
        range.start_column,
        range.end_line,
        range.end_column
    )
}

fn selector_from_anchor(
    anchor: &EvidenceSubjectRegistration,
    inventory: &RustTestInventory,
) -> Option<RustTestSelector> {
    let module_path = anchor
        .module_path
        .split("::")
        .filter(|segment| !segment.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    let target = match anchor.target.split_once(':') {
        Some((kind, target_name)) => {
            let kind = match kind {
                "lib" => RustTestTargetKind::Library,
                "bin" => RustTestTargetKind::Binary,
                "integration_test" | "test" => RustTestTargetKind::IntegrationTest,
                _ => return None,
            };
            RustTestTargetIdentity {
                kind,
                name: target_name.to_string(),
            }
        }
        None => {
            let mut targets = inventory
                .subjects
                .iter()
                .filter(|subject| {
                    subject.selector.package == anchor.package
                        && subject.selector.target.name == anchor.target
                        && subject.selector.module_path == module_path
                        && subject.selector.function == anchor.test_name
                })
                .map(|subject| subject.selector.target.clone());
            let target = targets.next()?;
            if targets.any(|candidate| candidate != target) {
                return None;
            }
            target
        }
    };
    let selector = RustTestSelector {
        package: anchor.package.clone(),
        target,
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

fn partial_inventory_limitations(inventory: &RustTestInventory) -> Vec<String> {
    let mut limitations = vec!["structural inventory is partial".to_string()];
    limitations.extend(inventory.diagnostics.iter().map(|diagnostic| {
        diagnostic.path.as_deref().map_or_else(
            || diagnostic.message.clone(),
            |path| format!("{path}: {}", diagnostic.message),
        )
    }));
    limitations
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
        assert!(resolution
            .limitations
            .iter()
            .any(|limitation| limitation == "structural inventory is partial"));
        assert!(resolution
            .limitations
            .iter()
            .any(|limitation| limitation == "src/lib.rs: parse error"));
    }

    #[test]
    fn missing_and_malformed_authored_subjects_remain_distinct() {
        let missing = resolve_authored_rust_subject(
            &anchor_for_target("lib:demo", "fnv1a64:current"),
            &empty_inventory(),
        );
        assert_eq!(missing.class, IntentSubjectResolutionClassV1::Missing);
        assert!(missing.selector.is_some());

        let malformed = resolve_authored_rust_subject(
            &anchor_for_target("bench:demo", "fnv1a64:current"),
            &empty_inventory(),
        );
        assert_eq!(
            malformed.class,
            IntentSubjectResolutionClassV1::MalformedAnchor
        );
        assert!(malformed.selector.is_none());
        assert_eq!(malformed.limitations.len(), 1);
    }

    #[test]
    fn ambiguous_authored_subject_preserves_candidates() {
        let selector = selector(RustTestTargetKind::Library, "demo", "alpha");
        let first = subject(selector.clone());
        let mut second = subject(selector.clone());
        second.source_path = "src/other.rs".to_string();
        second.source_range = RustTestSourceRange {
            start_line: 10,
            start_column: 2,
            end_line: 12,
            end_column: 3,
        };
        let inventory = RustTestInventory {
            subjects: vec![first, second],
            status: RustTestInventoryStatus::Complete,
            diagnostics: Vec::new(),
        };

        let resolution = resolve_authored_rust_subject(&anchor("fnv1a64:current"), &inventory);
        assert_eq!(resolution.class, IntentSubjectResolutionClassV1::Ambiguous);
        assert_eq!(resolution.candidates.len(), 2);
        assert_ne!(resolution.candidates[0], resolution.candidates[1]);
        assert!(resolution.candidates[0].contains("src/lib.rs:1:1-3:2"));
        assert!(resolution.candidates[1].contains("src/other.rs:10:2-12:3"));
        assert!(resolution.source_path.is_none());
    }

    #[test]
    fn bare_target_name_from_self_hosted_registration_resolves_exactly() {
        let inventory = inventory(false, false, false);
        let resolution = resolve_authored_rust_subject(
            &anchor_for_target("demo", "fnv1a64:current"),
            &inventory,
        );
        assert_eq!(
            resolution.class,
            IntentSubjectResolutionClassV1::ExactCurrent
        );
    }

    #[test]
    fn subject_postures_are_projected_without_becoming_current() {
        for (ignored, generated, conditional, expected) in [
            (true, false, false, IntentSubjectResolutionClassV1::Ignored),
            (
                false,
                true,
                false,
                IntentSubjectResolutionClassV1::GeneratedOrParameterized,
            ),
            (
                false,
                false,
                true,
                IntentSubjectResolutionClassV1::ConditionalUnknown,
            ),
        ] {
            let mut subject = subject(selector(RustTestTargetKind::Library, "demo", "alpha"));
            subject.ignored = ignored;
            subject.generated_or_parameterized = generated;
            subject.cfg_or_feature_unknown = conditional;
            subject.limitations = vec!["structural limitation".to_string()];
            let inventory = RustTestInventory {
                subjects: vec![subject],
                status: RustTestInventoryStatus::Complete,
                diagnostics: Vec::new(),
            };

            let resolution = resolve_authored_rust_subject(&anchor("fnv1a64:current"), &inventory);
            assert_eq!(resolution.class, expected);
            assert_eq!(resolution.source_path.as_deref(), Some("src/lib.rs"));
            assert_eq!(
                resolution.limitations,
                vec!["structural limitation".to_string()]
            );
        }
    }

    #[test]
    fn binary_and_integration_test_targets_resolve_exactly() {
        for (target, kind, name) in [
            ("bin:runner", RustTestTargetKind::Binary, "runner"),
            (
                "integration_test:api",
                RustTestTargetKind::IntegrationTest,
                "api",
            ),
            ("test:api", RustTestTargetKind::IntegrationTest, "api"),
        ] {
            let selector = selector(kind, name, "alpha");
            let inventory = RustTestInventory {
                subjects: vec![subject(selector)],
                status: RustTestInventoryStatus::Complete,
                diagnostics: Vec::new(),
            };
            let resolution = resolve_authored_rust_subject(
                &anchor_for_target(target, "fnv1a64:current"),
                &inventory,
            );
            assert_eq!(
                resolution.class,
                IntentSubjectResolutionClassV1::ExactCurrent
            );
        }
    }

    #[test]
    fn resolution_class_strings_are_stable() {
        for (class, expected) in [
            (
                IntentSubjectResolutionClassV1::ExactCurrent,
                "exact_current",
            ),
            (
                IntentSubjectResolutionClassV1::ExactBodyChanged,
                "exact_body_changed",
            ),
            (IntentSubjectResolutionClassV1::Ambiguous, "ambiguous"),
            (IntentSubjectResolutionClassV1::Missing, "missing"),
            (IntentSubjectResolutionClassV1::Ignored, "ignored"),
            (
                IntentSubjectResolutionClassV1::GeneratedOrParameterized,
                "generated_or_parameterized",
            ),
            (
                IntentSubjectResolutionClassV1::ConditionalUnknown,
                "conditional_unknown",
            ),
            (
                IntentSubjectResolutionClassV1::PartialInventory,
                "partial_inventory",
            ),
            (
                IntentSubjectResolutionClassV1::MalformedAnchor,
                "malformed_anchor",
            ),
        ] {
            assert_eq!(class.as_str(), expected);
        }
    }

    fn anchor(source_identity: &str) -> EvidenceSubjectRegistration {
        anchor_for_target("lib:demo", source_identity)
    }

    fn anchor_for_target(target: &str, source_identity: &str) -> EvidenceSubjectRegistration {
        EvidenceSubjectRegistration {
            id: EvidenceSubjectId("subject:demo:alpha".to_string()),
            role: EvidenceSubjectRole::ExactEvidence,
            package: "demo".to_string(),
            target: target.to_string(),
            module_path: "tests".to_string(),
            test_name: "alpha".to_string(),
            source: SourceLocation::new("src/lib.rs"),
            source_identity: source_identity.to_string(),
        }
    }

    fn empty_inventory() -> RustTestInventory {
        RustTestInventory {
            subjects: Vec::new(),
            status: RustTestInventoryStatus::Complete,
            diagnostics: Vec::new(),
        }
    }

    fn selector(kind: RustTestTargetKind, name: &str, function: &str) -> RustTestSelector {
        RustTestSelector {
            package: "demo".to_string(),
            target: RustTestTargetIdentity {
                kind,
                name: name.to_string(),
            },
            module_path: vec!["tests".to_string()],
            function: function.to_string(),
        }
    }

    fn subject(selector: RustTestSelector) -> RustTestSubject {
        RustTestSubject {
            selector,
            source_path: "src/lib.rs".to_string(),
            source_range: RustTestSourceRange {
                start_line: 1,
                start_column: 1,
                end_line: 3,
                end_column: 2,
            },
            body_identity: "fnv1a64:current".to_string(),
            attributes: vec!["test".to_string()],
            generated_or_parameterized: false,
            cfg_or_feature_unknown: false,
            ignored: false,
            limitations: Vec::new(),
        }
    }

    fn inventory(
        ignored: bool,
        generated_or_parameterized: bool,
        cfg_or_feature_unknown: bool,
    ) -> RustTestInventory {
        let mut subject = subject(selector(RustTestTargetKind::Library, "demo", "alpha"));
        subject.generated_or_parameterized = generated_or_parameterized;
        subject.cfg_or_feature_unknown = cfg_or_feature_unknown;
        subject.ignored = ignored;
        RustTestInventory {
            subjects: vec![subject],
            status: RustTestInventoryStatus::Complete,
            diagnostics: Vec::new(),
        }
    }
}
