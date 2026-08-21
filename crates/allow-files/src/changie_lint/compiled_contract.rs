//! The compiled effective fragment contract (#3620 PR B2).
//!
//! `compile_contract` turns one parsed configuration (plus its B1
//! findings) into a deterministic, source-located contract that owns
//! every config-derived persisted-fragment rule. Fragment validation
//! consumes the contract, never the raw configuration, so #3613 can
//! project the contract directly without another rule evaluator. The
//! compile fails closed on ambiguities B1 already surfaced — a falsely
//! complete contract is never produced.

use crate::changie::{
    ChangieConfigDocument, ChangieContentIdentity, ChangieMapping, ChangieSourceRange, ChangieValue,
};

/// Authority behind each diagnostic: which layer's observed behavior the
/// rule encodes. Message templates may claim upstream rejection only
/// when the exact retained observation establishes it; hand-edited
/// authoring checks stronger than current batch loading are
/// `RustStaticCompanion` and say so.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangieProvenance {
    /// The official Changie configuration schema states the constraint.
    OfficialConfigSchema,
    /// Upstream configuration loading rejects the input.
    UpstreamConfigLoad,
    /// Upstream `changie new` authoring enforces the constraint.
    UpstreamNewAuthoring,
    /// Upstream `changie batch` fragment loading rejects the input.
    UpstreamBatchLoad,
    /// This sensor's static companion check, stronger than current
    /// upstream batch loading.
    RustStaticCompanion,
    /// Source-acquisition safety (paths, entry states, population).
    SourceSafety,
}

impl ChangieProvenance {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OfficialConfigSchema => "official_config_schema",
            Self::UpstreamConfigLoad => "upstream_config_load",
            Self::UpstreamNewAuthoring => "upstream_new_authoring",
            Self::UpstreamBatchLoad => "upstream_batch_load",
            Self::RustStaticCompanion => "rust_static_companion",
            Self::SourceSafety => "source_safety",
        }
    }
}

/// One configured project declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledProject {
    /// Canonical persisted key: fragments must carry the key, never the
    /// display label, as the persisted identity.
    pub key: String,
    pub label: Option<String>,
    pub declaration_range: ChangieSourceRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledKind {
    /// Kind labels are their own canonical persisted identity in the
    /// modeled generation (kinds carry no separate key).
    pub label: String,
    pub skip_body: bool,
    pub skip_global_choices: bool,
    pub declaration_range: ChangieSourceRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledBodyPosture {
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
    pub block: bool,
    /// Upstream measures body length in UTF-8 bytes (Go `len`); pinned
    /// by the retained Unicode fixtures.
    pub length_semantics: &'static str,
}

/// One active custom choice and its kind scoping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledChoice {
    pub key: String,
    pub choice_type: ChoiceType,
    pub optional: bool,
    pub min_int: Option<i64>,
    pub max_int: Option<i64>,
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
    pub enum_options: Vec<String>,
    pub declaration_range: ChangieSourceRange,
    /// `Global` choices are active for every kind unless the kind sets
    /// `skipGlobalChoices`; `KindSpecific` choices are additional
    /// choices active only for one kind label.
    pub scope: ChoiceScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChoiceType {
    String,
    Block,
    Int,
    Enum,
}

impl ChoiceType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Block => "block",
            Self::Int => "int",
            Self::Enum => "enum",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChoiceScope {
    Global,
    KindSpecific { kind_label: String },
}

impl ChoiceScope {
    pub const fn is_global(&self) -> bool {
        matches!(self, Self::Global)
    }
}

/// The deterministic effective contract. All config-derived
/// persisted-fragment rules live here; validation consumes only this.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangieCompiledFragmentContractV1 {
    pub generation: &'static str,
    pub config_identity: ChangieContentIdentity,
    pub time_format: Option<String>,
    pub projects: Vec<CompiledProject>,
    pub components: Vec<String>,
    pub kinds: Vec<CompiledKind>,
    pub body: CompiledBodyPosture,
    pub choices: Vec<CompiledChoice>,
    /// Recognized operation-affecting configuration the static contract
    /// does not evaluate (templates, post, replacements).
    pub opaque_limitations: Vec<&'static str>,
    /// Digest over the canonical serialization: equal configurations
    /// compile to equal digests, and any semantic change changes it.
    pub digest: ChangieContentIdentity,
}

/// Why a contract could not be compiled: the configuration is ambiguous
/// or incomplete in a way B1 already surfaced. Fragment semantics are
/// skipped fail-honestly rather than guessed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractCompileBlocker {
    MalformedConfiguration,
    AmbiguousConfiguration(String),
}

pub fn compile_contract(
    config: &ChangieConfigDocument,
) -> Result<ChangieCompiledFragmentContractV1, ContractCompileBlocker> {
    if config.root.is_none() {
        return Err(ContractCompileBlocker::MalformedConfiguration);
    }
    if let Some(unsupported) = config.unsupported_fields.first() {
        return Err(ContractCompileBlocker::AmbiguousConfiguration(format!(
            "configuration uses an unevaluated alias at {}",
            unsupported.path
        )));
    }
    let mapping = match config.root.as_ref().map(|node| &node.value) {
        Some(ChangieValue::Mapping(mapping)) => mapping,
        _ => {
            return Err(ContractCompileBlocker::AmbiguousConfiguration(
                "configuration root is not a mapping".into(),
            ));
        }
    };
    // Duplicate top-level keys make the effective configuration
    // ambiguous (last-writer-wins): refuse a falsely complete contract.
    for entry in &mapping.entries {
        if mapping.count(&entry.key) > 1 {
            return Err(ContractCompileBlocker::AmbiguousConfiguration(format!(
                "duplicate configuration key `{}`",
                entry.key
            )));
        }
    }

    let projects = compile_projects(mapping)?;
    let components = compile_components(mapping)?;
    let kinds = compile_kinds(mapping)?;
    let body = compile_body(mapping);
    let choices = compile_choices(mapping, &kinds)?;

    let mut opaque_limitations = Vec::new();
    for field in [
        "headerFormat",
        "footerFormat",
        "changeFormat",
        "versionFormat",
        "fragmentFileFormat",
        "versionFileFormat",
    ] {
        if mapping.first(field).is_some() {
            opaque_limitations.push("template_render_semantics");
            break;
        }
    }
    if mapping.first("post").is_some() {
        opaque_limitations.push("post_execution");
    }
    if mapping.first("replacements").is_some() {
        opaque_limitations.push("replacement_execution");
    }

    let contract = ChangieCompiledFragmentContractV1 {
        generation: crate::changie::CHANGIE_COMPATIBILITY_GENERATION,
        config_identity: config.source.content_identity(),
        time_format: string_at(mapping, "timeFormat"),
        projects,
        components,
        kinds,
        body,
        choices,
        opaque_limitations,
        digest: ChangieContentIdentity::of(&[]),
    };
    let mut complete = contract.clone();
    complete.digest = ChangieContentIdentity::of(canonical_contract_text(&complete).as_bytes());
    Ok(complete)
}

/// Deterministic canonical serialization of the contract for #3613
/// projection and the digest: field order is fixed, choices are sorted
/// by (scope, key), enum options keep authored order.
pub fn canonical_contract_text(contract: &ChangieCompiledFragmentContractV1) -> String {
    let mut out = String::new();
    out.push_str("changie.compiled-fragment-contract.v1\n");
    out.push_str(&format!("generation={}\n", contract.generation));
    out.push_str(&format!("config_identity={}\n", contract.config_identity));
    if let Some(time_format) = &contract.time_format {
        out.push_str(&format!("time_format={time_format}\n"));
    }
    for project in &contract.projects {
        out.push_str(&format!(
            "project key={} label={}\n",
            project.key,
            project.label.as_deref().unwrap_or("")
        ));
    }
    for component in &contract.components {
        out.push_str(&format!("component={component}\n"));
    }
    for kind in &contract.kinds {
        out.push_str(&format!(
            "kind={} skip_body={} skip_global_choices={}\n",
            kind.label, kind.skip_body, kind.skip_global_choices
        ));
    }
    out.push_str(&format!(
        "body min={:?} max={:?} block={} semantics={}\n",
        contract.body.min_length,
        contract.body.max_length,
        contract.body.block,
        contract.body.length_semantics
    ));
    let mut sorted: Vec<&CompiledChoice> = contract.choices.iter().collect();
    sorted.sort_by_key(|choice| match &choice.scope {
        ChoiceScope::Global => (0, String::new(), choice.key.clone()),
        ChoiceScope::KindSpecific { kind_label } => (1, kind_label.clone(), choice.key.clone()),
    });
    for choice in sorted {
        let scope = match &choice.scope {
            ChoiceScope::Global => "global".to_string(),
            ChoiceScope::KindSpecific { kind_label } => format!("kind:{kind_label}"),
        };
        out.push_str(&format!(
            "choice key={} type={} optional={} scope={} min_int={:?} max_int={:?} min_len={:?} max_len={:?} enum={}\n",
            choice.key,
            choice.choice_type.as_str(),
            choice.optional,
            scope,
            choice.min_int,
            choice.max_int,
            choice.min_length,
            choice.max_length,
            choice.enum_options.join(",")
        ));
    }
    for limitation in &contract.opaque_limitations {
        out.push_str(&format!("limitation={limitation}\n"));
    }
    out
}

/// Choices active for one kind label: global choices unless the kind
/// skips them, plus the kind's additional choices.
pub fn active_choices<'a>(
    contract: &'a ChangieCompiledFragmentContractV1,
    kind_label: &str,
) -> Vec<&'a CompiledChoice> {
    let skip_global = contract
        .kinds
        .iter()
        .find(|kind| kind.label == kind_label)
        .is_some_and(|kind| kind.skip_global_choices);
    contract
        .choices
        .iter()
        .filter(|choice| match &choice.scope {
            ChoiceScope::Global => !skip_global,
            ChoiceScope::KindSpecific { kind_label: label } => label == kind_label,
        })
        .collect()
}

fn compile_projects(
    mapping: &ChangieMapping,
) -> Result<Vec<CompiledProject>, ContractCompileBlocker> {
    let mut projects = Vec::new();
    let Some(node) = mapping.first("projects") else {
        return Ok(projects);
    };
    let ChangieValue::Sequence(items) = &node.value else {
        return Err(ContractCompileBlocker::AmbiguousConfiguration(
            "projects is not a sequence".into(),
        ));
    };
    let mut seen_keys = std::collections::BTreeSet::new();
    for item in items {
        let ChangieValue::Mapping(project) = &item.value else {
            return Err(ContractCompileBlocker::AmbiguousConfiguration(
                "a project declaration is not a mapping".into(),
            ));
        };
        let Some(ChangieValue::String(key)) = project.first("key").map(|n| &n.value) else {
            return Err(ContractCompileBlocker::AmbiguousConfiguration(
                "a project declaration has no string key".into(),
            ));
        };
        if !seen_keys.insert(key.clone()) {
            return Err(ContractCompileBlocker::AmbiguousConfiguration(format!(
                "duplicate project key `{key}`"
            )));
        }
        projects.push(CompiledProject {
            key: key.clone(),
            label: match project.first("label").map(|n| &n.value) {
                Some(ChangieValue::String(label)) => Some(label.clone()),
                _ => None,
            },
            declaration_range: project.first("key").map(|n| n.range).unwrap_or(item.range),
        });
    }
    Ok(projects)
}

fn compile_components(mapping: &ChangieMapping) -> Result<Vec<String>, ContractCompileBlocker> {
    let mut components = Vec::new();
    let Some(node) = mapping.first("components") else {
        return Ok(components);
    };
    let ChangieValue::Sequence(items) = &node.value else {
        return Err(ContractCompileBlocker::AmbiguousConfiguration(
            "components is not a sequence".into(),
        ));
    };
    for item in items {
        match &item.value {
            ChangieValue::String(value) => {
                if !components.contains(value) {
                    components.push(value.clone());
                }
            }
            other => {
                return Err(ContractCompileBlocker::AmbiguousConfiguration(format!(
                    "a configured component is a {}",
                    shape(other)
                )));
            }
        }
    }
    Ok(components)
}

fn compile_kinds(mapping: &ChangieMapping) -> Result<Vec<CompiledKind>, ContractCompileBlocker> {
    let mut kinds = Vec::new();
    let Some(node) = mapping.first("kinds") else {
        return Ok(kinds);
    };
    let ChangieValue::Sequence(items) = &node.value else {
        return Err(ContractCompileBlocker::AmbiguousConfiguration(
            "kinds is not a sequence".into(),
        ));
    };
    let mut seen = std::collections::BTreeSet::new();
    for item in items {
        let ChangieValue::Mapping(kind) = &item.value else {
            return Err(ContractCompileBlocker::AmbiguousConfiguration(
                "a kind declaration is not a mapping".into(),
            ));
        };
        let Some(ChangieValue::String(label)) = kind.first("label").map(|n| &n.value) else {
            return Err(ContractCompileBlocker::AmbiguousConfiguration(
                "a kind declaration has no string label".into(),
            ));
        };
        if !seen.insert(label.clone()) {
            return Err(ContractCompileBlocker::AmbiguousConfiguration(format!(
                "duplicate kind label `{label}`"
            )));
        }
        kinds.push(CompiledKind {
            label: label.clone(),
            skip_body: matches!(
                kind.first("skipBody").map(|n| &n.value),
                Some(ChangieValue::Boolean(true))
            ),
            skip_global_choices: matches!(
                kind.first("skipGlobalChoices").map(|n| &n.value),
                Some(ChangieValue::Boolean(true))
            ),
            declaration_range: kind.first("label").map(|n| n.range).unwrap_or(item.range),
        });
    }
    Ok(kinds)
}

fn compile_body(mapping: &ChangieMapping) -> CompiledBodyPosture {
    let mut posture = CompiledBodyPosture {
        min_length: None,
        max_length: None,
        block: false,
        length_semantics: crate::changie_lint::fragment_rules::BODY_LENGTH_SEMANTICS,
    };
    if let Some(ChangieValue::Mapping(body)) = mapping.first("body").map(|n| &n.value) {
        if let Some(ChangieValue::Integer(value)) = body.first("minLength").map(|n| &n.value) {
            posture.min_length = Some((*value).max(0) as usize);
        }
        if let Some(ChangieValue::Integer(value)) = body.first("maxLength").map(|n| &n.value) {
            posture.max_length = Some((*value).max(0) as usize);
        }
        posture.block = matches!(
            body.first("block").map(|n| &n.value),
            Some(ChangieValue::Boolean(true))
        );
    }
    posture
}

fn compile_choices(
    mapping: &ChangieMapping,
    kinds: &[CompiledKind],
) -> Result<Vec<CompiledChoice>, ContractCompileBlocker> {
    let mut choices = Vec::new();
    let mut seen_keys = std::collections::BTreeSet::new();
    let add = |declarations: Option<&ChangieValue>,
               scope: ChoiceScope,
               choices: &mut Vec<CompiledChoice>,
               seen_keys: &mut std::collections::BTreeSet<String>|
     -> Result<(), ContractCompileBlocker> {
        let Some(ChangieValue::Sequence(items)) = declarations else {
            return Ok(());
        };
        for item in items {
            let ChangieValue::Mapping(choice) = &item.value else {
                return Err(ContractCompileBlocker::AmbiguousConfiguration(
                    "a custom choice declaration is not a mapping".into(),
                ));
            };
            let (Some(ChangieValue::String(key)), Some(ChangieValue::String(choice_type))) = (
                choice.first("key").map(|n| &n.value),
                choice.first("type").map(|n| &n.value),
            ) else {
                return Err(ContractCompileBlocker::AmbiguousConfiguration(
                    "a custom choice declaration lacks key/type".into(),
                ));
            };
            if !seen_keys.insert(key.clone()) {
                return Err(ContractCompileBlocker::AmbiguousConfiguration(format!(
                    "duplicate custom choice key `{key}`"
                )));
            }
            let parsed_type = match choice_type.as_str() {
                "string" => ChoiceType::String,
                "block" => ChoiceType::Block,
                "int" => ChoiceType::Int,
                "enum" => ChoiceType::Enum,
                other => {
                    return Err(ContractCompileBlocker::AmbiguousConfiguration(format!(
                        "custom choice `{key}` has unsupported type `{other}`"
                    )));
                }
            };
            let mut enum_options = Vec::new();
            if parsed_type == ChoiceType::Enum {
                match choice.first("enum").map(|n| &n.value) {
                    Some(ChangieValue::Sequence(options)) => {
                        for option in options {
                            if let ChangieValue::String(option_value) = &option.value {
                                enum_options.push(option_value.clone());
                            }
                        }
                    }
                    _ => {
                        return Err(ContractCompileBlocker::AmbiguousConfiguration(format!(
                            "enum choice `{key}` has no option sequence"
                        )));
                    }
                }
            }
            choices.push(CompiledChoice {
                key: key.clone(),
                choice_type: parsed_type,
                optional: matches!(
                    choice.first("optional").map(|n| &n.value),
                    Some(ChangieValue::Boolean(true))
                ),
                min_int: int_at(choice, "minInt"),
                max_int: int_at(choice, "maxInt"),
                min_length: int_at(choice, "minLength").map(|v| v.max(0) as usize),
                max_length: int_at(choice, "maxLength").map(|v| v.max(0) as usize),
                enum_options,
                declaration_range: choice.first("key").map(|n| n.range).unwrap_or(item.range),
                scope: scope.clone(),
            });
        }
        Ok(())
    };
    add(
        mapping.first("custom").map(|n| &n.value),
        ChoiceScope::Global,
        &mut choices,
        &mut seen_keys,
    )?;
    for kind in kinds {
        let declarations = kind_choice_declarations(mapping, &kind.label);
        if let Some(declarations) = declarations.as_ref() {
            let scope = ChoiceScope::KindSpecific {
                kind_label: kind.label.clone(),
            };
            add(Some(declarations), scope, &mut choices, &mut seen_keys)?;
        }
    }
    Ok(choices)
}

fn kind_choice_declarations(mapping: &ChangieMapping, label: &str) -> Option<ChangieValue> {
    let items = mapping.first("kinds").map(|n| &n.value)?;
    let ChangieValue::Sequence(items) = items else {
        return None;
    };
    for item in items {
        if let ChangieValue::Mapping(kind) = &item.value
            && matches!(
                kind.first("label").map(|n| &n.value),
                Some(ChangieValue::String(value)) if value == label
            )
        {
            return kind.first("additionalChoices").map(|n| n.value.clone());
        }
    }
    None
}

fn string_at(mapping: &ChangieMapping, key: &str) -> Option<String> {
    match mapping.first(key).map(|n| &n.value) {
        Some(ChangieValue::String(value)) => Some(value.clone()),
        _ => None,
    }
}

fn int_at(mapping: &ChangieMapping, key: &str) -> Option<i64> {
    match mapping.first(key).map(|n| &n.value) {
        Some(ChangieValue::Integer(value)) => Some(*value),
        _ => None,
    }
}

fn shape(value: &ChangieValue) -> &'static str {
    match value {
        ChangieValue::Null => "null",
        ChangieValue::EmptyString => "empty string",
        ChangieValue::String(_) => "string",
        ChangieValue::Integer(_) => "integer",
        ChangieValue::Boolean(_) => "boolean",
        ChangieValue::Sequence(_) => "sequence",
        ChangieValue::Mapping(_) => "mapping",
        ChangieValue::UnsupportedAlias => "alias",
    }
}

#[cfg(test)]
#[path = "compiled_contract_tests.rs"]
mod compiled_contract_tests;
