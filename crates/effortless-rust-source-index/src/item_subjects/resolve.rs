use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::{
    RustItemDefinitionKindV1, RustItemInventoryStatusV1, RustItemInventoryV1, RustItemSubjectIdV1,
    RustItemSubjectV1, RustItemTargetKindV1,
};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RustItemSelectorV1 {
    pub repository_id: Option<String>,
    pub snapshot_id: Option<String>,
    pub package: Option<String>,
    pub crate_name: Option<String>,
    pub target_kind: Option<RustItemTargetKindV1>,
    pub target_name: Option<String>,
    pub module_path: Option<Vec<String>>,
    pub item_path: Option<Vec<String>>,
    pub definition_kind: Option<RustItemDefinitionKindV1>,
    pub source_path: Option<String>,
    pub declaration_identity: Option<String>,
    pub subject_id: Option<RustItemSubjectIdV1>,
    pub generation_identity: Option<String>,
}

impl RustItemSelectorV1 {
    pub fn is_empty(&self) -> bool {
        self.repository_id.is_none()
            && self.snapshot_id.is_none()
            && self.package.is_none()
            && self.crate_name.is_none()
            && self.target_kind.is_none()
            && self.target_name.is_none()
            && self.module_path.is_none()
            && self.item_path.is_none()
            && self.definition_kind.is_none()
            && self.source_path.is_none()
            && self.declaration_identity.is_none()
            && self.subject_id.is_none()
            && self.generation_identity.is_none()
    }

    fn is_malformed(&self) -> bool {
        self.item_path.as_ref().is_some_and(|path| {
            path.is_empty() || path.iter().any(|segment| segment.trim().is_empty())
        }) || self
            .module_path
            .as_ref()
            .is_some_and(|path| path.iter().any(|segment| segment.trim().is_empty()))
            || self
                .package
                .as_ref()
                .is_some_and(|value| value.trim().is_empty())
            || self
                .repository_id
                .as_ref()
                .is_some_and(|value| value.trim().is_empty())
            || self
                .snapshot_id
                .as_ref()
                .is_some_and(|value| value.trim().is_empty())
            || self
                .crate_name
                .as_ref()
                .is_some_and(|value| value.trim().is_empty())
            || self
                .target_name
                .as_ref()
                .is_some_and(|value| value.trim().is_empty())
            || self
                .subject_id
                .as_ref()
                .is_some_and(|value| value.as_str().trim().is_empty())
            || self
                .source_path
                .as_ref()
                .is_some_and(|value| value.trim().is_empty())
            || self
                .declaration_identity
                .as_ref()
                .is_some_and(|value| value.trim().is_empty())
            || self
                .generation_identity
                .as_ref()
                .is_some_and(|value| value.trim().is_empty())
    }

    fn matches(&self, subject: &RustItemSubjectV1) -> bool {
        optional_eq(self.repository_id.as_ref(), &subject.repository_id)
            && optional_eq(self.snapshot_id.as_ref(), &subject.snapshot_id)
            && optional_eq(self.package.as_ref(), &subject.target.package)
            && optional_eq(self.crate_name.as_ref(), &subject.target.crate_name)
            && optional_eq(self.target_kind.as_ref(), &subject.target.kind)
            && optional_eq(self.target_name.as_ref(), &subject.target.name)
            && optional_eq(self.module_path.as_ref(), &subject.module_path)
            && optional_eq(self.item_path.as_ref(), &subject.item_path)
            && optional_eq(self.definition_kind.as_ref(), &subject.definition_kind)
            && self.source_path.as_ref().is_none_or(|selected| {
                subject
                    .source
                    .as_ref()
                    .is_some_and(|source| selected == &source.source_path)
            })
            && self.declaration_identity.as_ref().is_none_or(|selected| {
                subject
                    .source
                    .as_ref()
                    .is_some_and(|source| selected == &source.declaration_identity)
            })
            && optional_eq(self.subject_id.as_ref(), &subject.subject_id)
            && optional_eq(
                self.generation_identity.as_ref(),
                &subject.generation_identity,
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
    if selector.is_empty() || selector.is_malformed() {
        return resolution(RustItemResolutionClassV1::MalformedSelector, Vec::new());
    }
    if selector.repository_id.is_none()
        || selector.snapshot_id.is_none()
        || selector.package.is_none()
        || selector.crate_name.is_none()
        || selector.target_kind.is_none()
        || selector.target_name.is_none()
        || selector.item_path.is_none()
        || selector.definition_kind.is_none()
        || selector.module_path.is_none()
        || selector.subject_id.is_none()
        || selector.generation_identity.is_none()
    {
        return resolution(RustItemResolutionClassV1::MalformedSelector, Vec::new());
    }
    if !inventory.validate()
        || selector.repository_id.as_deref() != Some(&inventory.repository_id)
        || selector.snapshot_id.as_deref() != Some(&inventory.snapshot_id)
        || selector.generation_identity.as_deref() != Some(&inventory.generation_identity)
    {
        return resolution(RustItemResolutionClassV1::NotProven, Vec::new());
    }
    if inventory.status != RustItemInventoryStatusV1::Complete {
        return resolution(
            match inventory.status {
                RustItemInventoryStatusV1::Partial => RustItemResolutionClassV1::Partial,
                RustItemInventoryStatusV1::Unsupported => RustItemResolutionClassV1::NotProven,
                RustItemInventoryStatusV1::Complete => RustItemResolutionClassV1::NotProven,
            },
            Vec::new(),
        );
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
        let exact_capable = !subject.generated_or_macro_owned
            && subject.source_available
            && !matches!(&subject.definition_kind, RustItemDefinitionKindV1::Other(_))
            && subject.cfg_expressions.is_empty()
            && subject.limitations.is_empty();
        if exact_capable && selector.declaration_identity.is_none() {
            return resolution(RustItemResolutionClassV1::MalformedSelector, Vec::new());
        }
        let class = if subject.generated_or_macro_owned {
            RustItemResolutionClassV1::GeneratedOrMacroOwned
        } else if !subject.source_available {
            RustItemResolutionClassV1::SourceUnavailable
        } else if matches!(&subject.definition_kind, RustItemDefinitionKindV1::Other(_)) {
            RustItemResolutionClassV1::UnsupportedDefinitionKind
        } else if !subject.cfg_expressions.is_empty() {
            RustItemResolutionClassV1::CfgOrFeatureUnknown
        } else if !subject.limitations.is_empty() {
            RustItemResolutionClassV1::NotProven
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
    selected.is_none_or(|selected| selected == actual)
}
