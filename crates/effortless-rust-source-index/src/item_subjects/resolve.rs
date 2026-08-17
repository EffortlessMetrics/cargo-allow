use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::{
    RustItemDefinitionKindV1, RustItemInventoryStatusV1, RustItemInventoryV1,
    RustItemSubjectIdV1, RustItemSubjectV1, RustItemTargetKindV1,
};

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
    if !inventory.validate() {
        return resolution(RustItemResolutionClassV1::NotProven, Vec::new());
    }

    let mut matching = inventory
        .subjects
        .iter()
        .filter(|subject| selector.matches(subject))
        .collect::<Vec<_>>();

    if matching.len() > 1 {
        matching.sort_by(|left, right| left.subject_id.cmp(&right.subject_id));
        return resolution(
            RustItemResolutionClassV1::Ambiguous,
            matching.into_iter().cloned().collect(),
        );
    }

    if let Some(subject) = matching.first() {
        let class = if subject.generated_or_macro_owned {
            RustItemResolutionClassV1::GeneratedOrMacroOwned
        } else if !subject.source_available {
            RustItemResolutionClassV1::SourceUnavailable
        } else if matches!(
            &subject.definition_kind,
            RustItemDefinitionKindV1::Other(_)
        ) {
            RustItemResolutionClassV1::UnsupportedDefinitionKind
        } else if !subject.cfg_expressions.is_empty() {
            RustItemResolutionClassV1::CfgOrFeatureUnknown
        } else {
            RustItemResolutionClassV1::Exact
        };
        return resolution(class, vec![(**subject).clone()]);
    }

    let class = match inventory.status {
        RustItemInventoryStatusV1::Complete => {
            RustItemResolutionClassV1::MissingWithinCompleteScope
        }
        RustItemInventoryStatusV1::Partial => RustItemResolutionClassV1::Partial,
        RustItemInventoryStatusV1::Unsupported => RustItemResolutionClassV1::NotProven,
    };
    resolution(class, Vec::new())
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
    RustItemResolutionV1 {
        class,
        subjects,
        candidate_ids,
        limitations,
    }
}

fn optional_eq<T: PartialEq>(selected: Option<&T>, actual: &T) -> bool {
    selected.map_or(true, |selected| selected == actual)
}
