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

impl RustSourceRangeV1 {
    fn validate(&self) -> bool {
        self.start_line > 0
            && self.start_column > 0
            && self.end_line > 0
            && self.end_column > 0
            && self.start_line <= self.end_line
            && (self.start_line < self.end_line || self.start_column <= self.end_column)
    }
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

impl RustLintDeclarationSubjectV1 {
    fn validate_for(&self, item_subject_id: &RustItemSubjectIdV1) -> bool {
        self.schema_version == RUST_ITEM_SUBJECT_SCHEMA_VERSION
            && !self.subject_id.as_str().trim().is_empty()
            && &self.item_subject_id == item_subject_id
            && !self.lint_names.is_empty()
            && self
                .lint_names
                .iter()
                .all(|lint_name| !lint_name.trim().is_empty())
            && self.source_range.validate()
            && self
                .cfg_expression
                .as_ref()
                .is_none_or(|expression| !expression.trim().is_empty())
    }
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
        let path = self
            .module_path
            .iter()
            .chain(&self.item_path)
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join("::");
        format!(
            "{}:{}:{}::{}",
            self.target.package,
            target_kind_name(&self.target.kind),
            self.target.name,
            path
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
            && !self.item_path.is_empty()
            && !self.source.source_path.trim().is_empty()
            && !self.source.declaration_identity.trim().is_empty()
            && self.source.declaration_range.validate()
            && self
                .source
                .identifier_range
                .as_ref()
                .is_none_or(RustSourceRangeV1::validate)
            && self
                .module_path
                .iter()
                .chain(&self.item_path)
                .all(|segment| !segment.trim().is_empty())
            && self
                .cfg_expressions
                .iter()
                .all(|expression| !expression.trim().is_empty())
            && self
                .lint_declarations
                .iter()
                .all(|declaration| declaration.validate_for(&self.subject_id))
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
    pub generation_identity: String,
    pub status: RustItemInventoryStatusV1,
    pub subjects: Vec<RustItemSubjectV1>,
    pub diagnostics: Vec<String>,
}

impl RustItemInventoryV1 {
    pub fn validate(&self) -> bool {
        if self.schema_version != RUST_ITEM_SUBJECT_SCHEMA_VERSION
            || self.repository_id.trim().is_empty()
            || self.snapshot_id.trim().is_empty()
            || self.generation_identity.trim().is_empty()
        {
            return false;
        }

        let mut subject_ids = BTreeSet::new();
        let mut declaration_ids = BTreeSet::new();
        let valid_subjects = self.subjects.iter().all(|subject| {
            subject.validate()
                && subject.repository_id == self.repository_id
                && subject.snapshot_id == self.snapshot_id
                && subject_ids.insert(&subject.subject_id)
                && subject
                    .lint_declarations
                    .iter()
                    .all(|declaration| declaration_ids.insert(&declaration.subject_id))
        });
        valid_subjects && self.valid_container_links()
    }

    fn valid_container_links(&self) -> bool {
        let ids = self
            .subjects
            .iter()
            .map(|subject| subject.subject_id.clone())
            .collect::<BTreeSet<_>>();
        self.subjects.iter().all(|subject| {
            let Some(container) = subject.container_subject_id.as_ref() else {
                return true;
            };
            if container == &subject.subject_id || !ids.contains(container) {
                return false;
            }
            let mut current = container.clone();
            let mut seen = BTreeSet::new();
            while seen.insert(current.clone()) {
                let Some(next) = self
                    .subjects
                    .iter()
                    .find(|candidate| candidate.subject_id == current)
                    .and_then(|candidate| candidate.container_subject_id.clone())
                else {
                    return true;
                };
                if next == subject.subject_id {
                    return false;
                }
                current = next;
            }
            false
        })
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
