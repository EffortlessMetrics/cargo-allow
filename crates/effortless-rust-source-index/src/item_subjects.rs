//! Exact structural Rust item identities for cross-provider evidence joins (#3607).
//!
//! This module deliberately stops at source-structural facts. It does not
//! establish that an item compiled, is live, is dead, is externally consumed,
//! or is safe to change.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

pub const RUST_ITEM_SUBJECT_SCHEMA_VERSION: &str = "rust_item_subject.v1";

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct RustItemSubjectIdV1(pub String);

impl RustItemSubjectIdV1 {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct RustLintDeclarationSubjectIdV1(pub String);

impl RustLintDeclarationSubjectIdV1 {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RustItemDefinitionKindV1 {
    Module,
    Function,
    Method,
    Struct,
    Enum,
    Union,
    Trait,
    Impl,
    Field,
    Variant,
    Const,
    Static,
    TypeAlias,
    AssociatedType,
    AssociatedConst,
    Macro,
    Other(String),
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RustItemTargetKindV1 {
    Library,
    Binary,
    IntegrationTest,
    Bench,
    Example,
    BuildScript,
    Other(String),
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RustItemTargetIdentityV1 {
    pub package: String,
    pub crate_name: String,
    pub kind: RustItemTargetKindV1,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RustVisibilityShapeV1 {
    Private,
    PubSelf,
    PubSuper,
    PubCrate,
    PubIn(String),
    Public,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RustSourceRangeV1 {
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RustItemSourceIdentityV1 {
    pub source_path: String,
    pub declaration_range: RustSourceRangeV1,
    pub identifier_range: Option<RustSourceRangeV1>,
    pub declaration_identity: String,
    pub signature_identity: Option<String>,
    pub body_identity: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RustLintDeclarationFamilyV1 {
    Allow,
    Expect,
    Warn,
    Deny,
    Forbid,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RustLintDeclarationSubjectV1 {
    pub schema_version: String,
    pub subject_id: RustLintDeclarationSubjectIdV1,
    pub item_subject_id: RustItemSubjectIdV1,
    pub family: RustLintDeclarationFamilyV1,
    pub lint_names: Vec<String>,
    pub source_range: RustSourceRangeV1,
    pub cfg_expression: Option<String>,
    pub conditional_source_only: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RustItemSubjectV1 {
    pub schema_version: String,
    pub subject_id: RustItemSubjectIdV1,
    pub repository_id: String,
    pub snapshot_id: String,
    pub target: RustItemTargetIdentityV1,
    pub module_path: Vec<String>,
    pub item_path: Vec<String>,
    pub definition_kind: RustItemDefinitionKindV1,
    pub source: RustItemSourceIdentityV1,
    pub container_subject_id: Option<RustItemSubjectIdV1>,
    pub visibility: RustVisibilityShapeV1,
    pub cfg_expressions: Vec<String>,
    pub lint_declarations: Vec<RustLintDeclarationSubjectV1>,
    pub source_available: bool,
    pub generated_or_macro_owned: bool,
    pub limitations: Vec<String>,
}

impl RustItemSubjectV1 {
    pub fn display_name(&self) -> String {
        let mut path = self.module_path.clone();
        path.extend(self.item_path.clone());
        format!(
            "{}:{}:{}::{}",
            self.target.package,
            target_kind_name(&self.target.kind),
            self.target.name,
            path.join("::")
        )
    }

    pub fn validate(&self) -> bool {
        self.schema_version == RUST_ITEM_SUBJECT_SCHEMA_VERSION
            && !self.subject_id.as_str().trim().is_empty()
            && !self.repository_id.trim().is_empty()
            && !self.snapshot_id.trim().is_empty()
            && !self.target.package.trim().is_empty()
            && !self.target.crate_name.trim().is_empty()
            && !self.target.name.trim().is_empty()
            && !self.source.source_path.trim().is_empty()
            && !self.source.declaration_identity.trim().is_empty()
            && self
                .module_path
                .iter()
                .chain(self.item_path.iter())
                .all(|segment| !segment.trim().is_empty())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RustItemInventoryStatusV1 {
    Complete,
    Partial,
    Unsupported,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RustItemInventoryV1 {
    pub schema_version: String,
    pub repository_id: String,
    pub snapshot_id: String,
    pub status: RustItemInventoryStatusV1,
    pub subjects: Vec<RustItemSubjectV1>,
    pub diagnostics: Vec<String>,
}

impl RustItemInventoryV1 {
    pub fn validate(&self) -> bool {
        if self.schema_version != RUST_ITEM_SUBJECT_SCHEMA_VERSION
            || self.repository_id.trim().is_empty()
            || self.snapshot_id.trim().is_empty()
            || self.subjects.iter().any(|subject| !subject.validate())
        {
            return false;
        }

        let unique_ids = self
            .subjects
            .iter()
            .map(|subject| subject.subject_id.clone())
            .collect::<BTreeSet<_>>();
        unique_ids.len() == self.subjects.len()
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RustItemSelectorV1 {
    pub package: Option<String>,
    pub target_kind: Option<RustItemTargetKindV1>,
    pub target_name: Option<String>,
    pub module_path: Option<Vec<String>>,
    pub item_path: Option<Vec<String>>,
    pub definition_kind: Option<RustItemDefinitionKindV1>,
    pub source_path: Option<String>,
    pub declaration_identity: Option<String>,
}

impl RustItemSelectorV1 {
    pub fn is_empty(&self) -> bool {
        self.package.is_none()
            && self.target_kind.is_none()
            && self.target_name.is_none()
            && self.module_path.is_none()
            && self.item_path.is_none()
            && self.definition_kind.is_none()
            && self.source_path.is_none()
            && self.declaration_identity.is_none()
    }

    fn matches(&self, subject: &RustItemSubjectV1) -> bool {
        optional_eq(self.package.as_ref(), &subject.target.package)
            && optional_eq(self.target_kind.as_ref(), &subject.target.kind)
            && optional_eq(self.target_name.as_ref(), &subject.target.name)
            && optional_eq(self.module_path.as_ref(), &subject.module_path)
            && optional_eq(self.item_path.as_ref(), &subject.item_path)
            && optional_eq(self.definition_kind.as_ref(), &subject.definition_kind)
            && optional_eq(self.source_path.as_ref(), &subject.source.source_path)
            && optional_eq(
                self.declaration_identity.as_ref(),
                &subject.source.declaration_identity,
            )
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RustItemResolutionClassV1 {
    Exact,
    Ambiguous,
    MissingWithinCompleteScope,
    Partial,
    CfgOrFeatureUnknown,
    GeneratedOrMacroOwned,
    SourceUnavailable,
    UnsupportedDefinitionKind,
    MalformedSelector,
    NotProven,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RustItemResolutionV1 {
    pub class: RustItemResolutionClassV1,
    pub subjects: Vec<RustItemSubjectV1>,
    pub candidate_ids: Vec<RustItemSubjectIdV1>,
    pub limitations: Vec<String>,
}

pub fn resolve_rust_item_subject(
    inventory: &RustItemInventoryV1,
    selector: &RustItemSelectorV1,
) -> RustItemResolutionV1 {
    if selector.is_empty() {
        return resolution(RustItemResolutionClassV1::MalformedSelector, Vec::new());
    }

    let mut subjects = inventory
        .subjects
        .iter()
        .filter(|subject| selector.matches(subject))
        .cloned()
        .collect::<Vec<_>>();
    subjects.sort_by(|left, right| left.subject_id.cmp(&right.subject_id));

    if subjects.len() > 1 {
        return resolution(RustItemResolutionClassV1::Ambiguous, subjects);
    }

    if let Some(subject) = subjects.first() {
        let class = if subject.generated_or_macro_owned {
            RustItemResolutionClassV1::GeneratedOrMacroOwned
        } else if !subject.source_available {
            RustItemResolutionClassV1::SourceUnavailable
        } else if matches!(&subject.definition_kind, RustItemDefinitionKindV1::Other(_)) {
            RustItemResolutionClassV1::UnsupportedDefinitionKind
        } else if !subject.cfg_expressions.is_empty() {
            RustItemResolutionClassV1::CfgOrFeatureUnknown
        } else {
            RustItemResolutionClassV1::Exact
        };
        return resolution(class, subjects);
    }

    let class = match inventory.status {
        RustItemInventoryStatusV1::Complete => {
            RustItemResolutionClassV1::MissingWithinCompleteScope
        }
        RustItemInventoryStatusV1::Partial => RustItemResolutionClassV1::Partial,
        RustItemInventoryStatusV1::Unsupported => RustItemResolutionClassV1::NotProven,
    };
    resolution(class, subjects)
}

fn resolution(
    class: RustItemResolutionClassV1,
    subjects: Vec<RustItemSubjectV1>,
) -> RustItemResolutionV1 {
    let candidate_ids = subjects
        .iter()
        .map(|subject| subject.subject_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let limitations = subjects
        .iter()
        .flat_map(|subject| subject.limitations.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    RustItemResolutionV1 { class, subjects, candidate_ids, limitations }
}

fn optional_eq<T: PartialEq>(selected: Option<&T>, actual: &T) -> bool {
    match selected {
        Some(selected) => selected == actual,
        None => true,
    }
}

fn target_kind_name(kind: &RustItemTargetKindV1) -> &'static str {
    match kind {
        RustItemTargetKindV1::Library => "lib",
        RustItemTargetKindV1::Binary => "bin",
        RustItemTargetKindV1::IntegrationTest => "test",
        RustItemTargetKindV1::Bench => "bench",
        RustItemTargetKindV1::Example => "example",
        RustItemTargetKindV1::BuildScript => "build",
        RustItemTargetKindV1::Other(_) => "other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, module: &[&str]) -> RustItemSubjectV1 {
        RustItemSubjectV1 {
            schema_version: RUST_ITEM_SUBJECT_SCHEMA_VERSION.into(),
            subject_id: RustItemSubjectIdV1::new(id),
            repository_id: "repo".into(),
            snapshot_id: "tree:abc".into(),
            target: RustItemTargetIdentityV1 {
                package: "demo".into(),
                crate_name: "demo".into(),
                kind: RustItemTargetKindV1::Library,
                name: "demo".into(),
            },
            module_path: module.iter().map(|segment| (*segment).into()).collect(),
            item_path: vec!["run".into()],
            definition_kind: RustItemDefinitionKindV1::Function,
            source: RustItemSourceIdentityV1 {
                source_path: format!("src/{}.rs", module.join("_")),
                declaration_range: RustSourceRangeV1 {
                    start_line: 1,
                    start_column: 1,
                    end_line: 3,
                    end_column: 2,
                },
                identifier_range: None,
                declaration_identity: format!("decl:{id}"),
                signature_identity: Some(format!("sig:{id}")),
                body_identity: Some(format!("body:{id}")),
            },
            container_subject_id: None,
            visibility: RustVisibilityShapeV1::Private,
            cfg_expressions: Vec::new(),
            lint_declarations: Vec::new(),
            source_available: true,
            generated_or_macro_owned: false,
            limitations: Vec::new(),
        }
    }

    fn inventory(
        status: RustItemInventoryStatusV1,
        subjects: Vec<RustItemSubjectV1>,
    ) -> RustItemInventoryV1 {
        RustItemInventoryV1 {
            schema_version: RUST_ITEM_SUBJECT_SCHEMA_VERSION.into(),
            repository_id: "repo".into(),
            snapshot_id: "tree:abc".into(),
            status,
            subjects,
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn same_name_items_require_module_identity() {
        let inventory = inventory(
            RustItemInventoryStatusV1::Complete,
            vec![item("left", &["left"]), item("right", &["right"])],
        );
        let ambiguous = resolve_rust_item_subject(
            &inventory,
            &RustItemSelectorV1 { item_path: Some(vec!["run".into()]), ..Default::default() },
        );
        assert_eq!(ambiguous.class, RustItemResolutionClassV1::Ambiguous);
        assert_eq!(ambiguous.candidate_ids.len(), 2);

        let exact = resolve_rust_item_subject(
            &inventory,
            &RustItemSelectorV1 {
                module_path: Some(vec!["right".into()]),
                item_path: Some(vec!["run".into()]),
                ..Default::default()
            },
        );
        assert_eq!(exact.class, RustItemResolutionClassV1::Exact);
        assert_eq!(exact.candidate_ids, vec![RustItemSubjectIdV1::new("right")]);
    }

    #[test]
    fn partial_inventory_never_returns_authoritative_missing() {
        let result = resolve_rust_item_subject(
            &inventory(RustItemInventoryStatusV1::Partial, Vec::new()),
            &RustItemSelectorV1 { item_path: Some(vec!["missing".into()]), ..Default::default() },
        );
        assert_eq!(result.class, RustItemResolutionClassV1::Partial);
    }

    #[test]
    fn complete_inventory_can_return_scoped_missing() {
        let result = resolve_rust_item_subject(
            &inventory(RustItemInventoryStatusV1::Complete, Vec::new()),
            &RustItemSelectorV1 { item_path: Some(vec!["missing".into()]), ..Default::default() },
        );
        assert_eq!(result.class, RustItemResolutionClassV1::MissingWithinCompleteScope);
    }

    #[test]
    fn generated_cfg_and_source_unavailable_items_are_not_exact() {
        let mut generated = item("generated", &["generated"]);
        generated.generated_or_macro_owned = true;
        let generated_result = resolve_rust_item_subject(
            &inventory(RustItemInventoryStatusV1::Complete, vec![generated]),
            &RustItemSelectorV1 { item_path: Some(vec!["run".into()]), ..Default::default() },
        );
        assert_eq!(generated_result.class, RustItemResolutionClassV1::GeneratedOrMacroOwned);

        let mut cfg = item("cfg", &["cfg"]);
        cfg.cfg_expressions.push("feature = \"special\"".into());
        let cfg_result = resolve_rust_item_subject(
            &inventory(RustItemInventoryStatusV1::Complete, vec![cfg]),
            &RustItemSelectorV1 { item_path: Some(vec!["run".into()]), ..Default::default() },
        );
        assert_eq!(cfg_result.class, RustItemResolutionClassV1::CfgOrFeatureUnknown);

        let mut unavailable = item("unavailable", &["unavailable"]);
        unavailable.source_available = false;
        let unavailable_result = resolve_rust_item_subject(
            &inventory(RustItemInventoryStatusV1::Complete, vec![unavailable]),
            &RustItemSelectorV1 { item_path: Some(vec!["run".into()]), ..Default::default() },
        );
        assert_eq!(unavailable_result.class, RustItemResolutionClassV1::SourceUnavailable);
    }

    #[test]
    fn count_preserving_item_substitution_changes_identity() {
        let first = inventory(RustItemInventoryStatusV1::Complete, vec![item("first", &["left"])]);
        let second = inventory(RustItemInventoryStatusV1::Complete, vec![item("second", &["left"])]);
        assert_eq!(first.subjects.len(), second.subjects.len());
        assert_ne!(first.subjects[0].subject_id, second.subjects[0].subject_id);
        assert_ne!(
            first.subjects[0].source.declaration_identity,
            second.subjects[0].source.declaration_identity
        );
    }

    #[test]
    fn empty_selector_is_malformed() {
        let result = resolve_rust_item_subject(
            &inventory(RustItemInventoryStatusV1::Complete, vec![item("one", &["one"])]),
            &RustItemSelectorV1::default(),
        );
        assert_eq!(result.class, RustItemResolutionClassV1::MalformedSelector);
    }

    #[test]
    fn inventory_validation_rejects_duplicate_subject_ids() {
        let subject = item("same", &["one"]);
        let invalid = inventory(
            RustItemInventoryStatusV1::Complete,
            vec![subject.clone(), subject],
        );
        assert!(!invalid.validate());
    }
}
