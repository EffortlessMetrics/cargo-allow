//! Semantic fragment validation against the parsed configuration
//! (#3589 PR B2). Every rule is config-derived: no hidden repository
//! house rule decides requiredness, and unknown kinds do not silently
//! fall back to global semantics.

use super::{
    ChangieAction, ChangieCandidateEntry, ChangieConfigDocument, ChangieDiagnostic,
    ChangieExpectedActual, ChangieFragmentDocument, ChangieResultClass, ChangieRule,
};
use crate::changie::{ChangieFieldPath, ChangieMapping, ChangieNode, ChangieValue};

/// Upstream measures body and custom string lengths in UTF-8 bytes (Go
/// `len` on a string), not runes. Unicode fixtures pin this choice so a
/// future generation change must be a conscious compatibility update.
pub const BODY_LENGTH_SEMANTICS: &str = "utf8-bytes";

pub(super) fn validate_fragment(
    config: &ChangieConfigDocument,
    entry: &ChangieCandidateEntry,
    fragment: &ChangieFragmentDocument,
    diagnostics: &mut Vec<ChangieDiagnostic>,
) {
    let root = match fragment.root.as_ref().map(|node| &node.value) {
        Some(ChangieValue::Mapping(mapping)) => mapping,
        Some(_) => {
            // A non-mapping fragment root is malformed at the parse level
            // for this surface; report it and stop semantic validation.
            diagnostics.push(ChangieDiagnostic {
                rule: ChangieRule::FragmentMalformed,
                result_class: ChangieResultClass::Malformed,
                repo_path: entry.repo_path.clone(),
                field_path: None,
                range: fragment.root.as_ref().map(|n| n.range),
                related_config_ranges: Vec::new(),
                expected_actual: None,
                message: "fragment root is not a mapping".into(),
                actions: vec![ChangieAction::InsertMissingField],
            });
            return;
        }
        None => return,
    };

    let kind = resolve_kind(config, entry, root, diagnostics);
    let components = configured_components(config);
    let projects_enabled = config_has_projects(config);

    validate_component(&components, entry, root, diagnostics);
    validate_project(projects_enabled, config, entry, root, diagnostics);
    validate_body(config, &kind, entry, root, diagnostics);
    validate_time(config, entry, root, diagnostics);
    validate_custom_values(config, &kind, entry, root, diagnostics);
}

// ---------------------------------------------------------------------------
// Config-derived declarations
// ---------------------------------------------------------------------------

pub(super) struct KindDeclaration {
    pub label: String,
    pub skip_body: bool,
    pub skip_global_choices: bool,
}

fn resolve_kind<'a>(
    config: &'a ChangieConfigDocument,
    entry: &ChangieCandidateEntry,
    root: &'a ChangieMapping,
    diagnostics: &mut Vec<ChangieDiagnostic>,
) -> Option<KindDeclaration> {
    let configured = configured_kinds(config);
    if configured.is_empty() {
        // No kinds configured: upstream does not gate on kind, so neither
        // does the static contract.
        return None;
    }
    let kind_node = root.first("kind");
    let label = match kind_node.map(|node| &node.value) {
        Some(ChangieValue::String(label)) => Some(label.clone()),
        Some(ChangieValue::EmptyString) | Some(ChangieValue::Null) | None => None,
        Some(other) => {
            diagnostics.push(ChangieDiagnostic {
                rule: ChangieRule::FragmentKindUnknown,
                result_class: ChangieResultClass::Finding,
                repo_path: entry.repo_path.clone(),
                field_path: Some(ChangieFieldPath(vec!["kind".into()])),
                range: kind_node.map(|n| n.range),
                related_config_ranges: kind_label_ranges(config),
                expected_actual: Some(ChangieExpectedActual {
                    expected: "kind string".into(),
                    actual: shape(other).into(),
                }),
                message: "fragment `kind` must be a string".into(),
                actions: vec![ChangieAction::ChooseConfiguredValue],
            });
            return None;
        }
    };
    let Some(label) = label else {
        diagnostics.push(ChangieDiagnostic {
            rule: ChangieRule::FragmentKindMissing,
            result_class: ChangieResultClass::Finding,
            repo_path: entry.repo_path.clone(),
            field_path: Some(ChangieFieldPath(vec!["kind".into()])),
            range: None,
            related_config_ranges: kind_label_ranges(config),
            expected_actual: None,
            message: "fragment `kind` is required but absent".into(),
            actions: vec![ChangieAction::InsertMissingField],
        });
        return None;
    };
    match configured.into_iter().find(|kind| kind.label == label) {
        Some(kind) => Some(kind),
        None => {
            diagnostics.push(ChangieDiagnostic {
                rule: ChangieRule::FragmentKindUnknown,
                result_class: ChangieResultClass::Finding,
                repo_path: entry.repo_path.clone(),
                field_path: Some(ChangieFieldPath(vec!["kind".into()])),
                range: kind_node.map(|n| n.range),
                related_config_ranges: kind_label_ranges(config),
                expected_actual: Some(ChangieExpectedActual {
                    expected: "a configured kind label".into(),
                    actual: label.clone(),
                }),
                message: format!(
                    "fragment kind `{label}` is not configured; kind-dependent rules do not fall back to global semantics"
                ),
                actions: vec![ChangieAction::ChooseConfiguredValue],
            });
            None
        }
    }
}

fn configured_kinds(config: &ChangieConfigDocument) -> Vec<KindDeclaration> {
    let mut kinds = Vec::new();
    if let Some(ChangieValue::Sequence(items)) = field(config, "kinds").map(|n| &n.value) {
        for item in items {
            if let ChangieValue::Mapping(mapping) = &item.value {
                let Some(ChangieValue::String(label)) = mapping.first("label").map(|n| &n.value)
                else {
                    continue;
                };
                kinds.push(KindDeclaration {
                    label: label.clone(),
                    skip_body: flag(mapping, "skipBody"),
                    skip_global_choices: flag(mapping, "skipGlobalChoices"),
                });
            }
        }
    }
    kinds
}

fn flag(mapping: &ChangieMapping, key: &str) -> bool {
    matches!(
        mapping.first(key).map(|n| &n.value),
        Some(ChangieValue::Boolean(true))
    )
}

fn configured_components(config: &ChangieConfigDocument) -> Vec<String> {
    let mut components = Vec::new();
    if let Some(ChangieValue::Sequence(items)) = field(config, "components").map(|n| &n.value) {
        for item in items {
            if let ChangieValue::String(value) = &item.value {
                components.push(value.clone());
            }
        }
    }
    components
}

fn config_has_projects(config: &ChangieConfigDocument) -> bool {
    matches!(
        field(config, "projects").map(|n| &n.value),
        Some(ChangieValue::Sequence(items)) if !items.is_empty()
    )
}

fn field<'a>(config: &'a ChangieConfigDocument, key: &str) -> Option<&'a ChangieNode> {
    config.root.as_ref().and_then(|node| match &node.value {
        ChangieValue::Mapping(mapping) => mapping.first(key),
        _ => None,
    })
}

fn kind_label_ranges(config: &ChangieConfigDocument) -> Vec<crate::changie::ChangieSourceRange> {
    let mut ranges = Vec::new();
    if let Some(ChangieValue::Sequence(items)) = field(config, "kinds").map(|n| &n.value) {
        for item in items {
            if let ChangieValue::Mapping(mapping) = &item.value
                && let Some(label) = mapping.first("label")
            {
                ranges.push(label.range);
            }
        }
    }
    ranges
}

// ---------------------------------------------------------------------------
// Field rules
// ---------------------------------------------------------------------------

fn validate_component(
    components: &[String],
    entry: &ChangieCandidateEntry,
    root: &ChangieMapping,
    diagnostics: &mut Vec<ChangieDiagnostic>,
) {
    if components.is_empty() {
        // No configured components: no hidden repository policy applies.
        return;
    }
    let component = root.first("component");
    let configured_range = field_declaration_range_of("components");
    let _ = configured_range;
    match component.map(|n| &n.value) {
        None | Some(ChangieValue::Null) => diagnostics.push(ChangieDiagnostic {
            rule: ChangieRule::FragmentComponentMissing,
            result_class: ChangieResultClass::Finding,
            repo_path: entry.repo_path.clone(),
            field_path: Some(ChangieFieldPath(vec!["component".into()])),
            range: component.map(|n| n.range),
            related_config_ranges: components_config_ranges(components),
            expected_actual: None,
            message: "fragment `component` is required while components are configured".into(),
            actions: vec![ChangieAction::ChooseConfiguredValue],
        }),
        Some(ChangieValue::EmptyString) => diagnostics.push(ChangieDiagnostic {
            rule: ChangieRule::FragmentComponentMissing,
            result_class: ChangieResultClass::Finding,
            repo_path: entry.repo_path.clone(),
            field_path: Some(ChangieFieldPath(vec!["component".into()])),
            range: component.map(|n| n.range),
            related_config_ranges: components_config_ranges(components),
            expected_actual: Some(ChangieExpectedActual {
                expected: "non-empty component".into(),
                actual: "empty".into(),
            }),
            message: "fragment `component` is authored empty".into(),
            actions: vec![ChangieAction::ChooseConfiguredValue],
        }),
        Some(ChangieValue::String(value)) => {
            if !components.contains(value) {
                diagnostics.push(ChangieDiagnostic {
                    rule: ChangieRule::FragmentComponentUnknown,
                    result_class: ChangieResultClass::Finding,
                    repo_path: entry.repo_path.clone(),
                    field_path: Some(ChangieFieldPath(vec!["component".into()])),
                    range: component.map(|n| n.range),
                    related_config_ranges: components_config_ranges(components),
                    expected_actual: Some(ChangieExpectedActual {
                        expected: "a configured component".into(),
                        actual: value.clone(),
                    }),
                    message: format!("fragment component `{value}` is not configured"),
                    actions: vec![ChangieAction::ChooseConfiguredValue],
                });
            }
        }
        Some(other) => diagnostics.push(ChangieDiagnostic {
            rule: ChangieRule::FragmentComponentUnknown,
            result_class: ChangieResultClass::Finding,
            repo_path: entry.repo_path.clone(),
            field_path: Some(ChangieFieldPath(vec!["component".into()])),
            range: component.map(|n| n.range),
            related_config_ranges: components_config_ranges(components),
            expected_actual: Some(ChangieExpectedActual {
                expected: "component string".into(),
                actual: shape(other).into(),
            }),
            message: "fragment `component` must be a string".into(),
            actions: vec![ChangieAction::ChooseConfiguredValue],
        }),
    }
}

fn components_config_ranges(components: &[String]) -> Vec<crate::changie::ChangieSourceRange> {
    let _ = components;
    Vec::new()
}

fn field_declaration_range_of(_key: &str) -> Option<crate::changie::ChangieSourceRange> {
    None
}

fn validate_project(
    projects_enabled: bool,
    config: &ChangieConfigDocument,
    entry: &ChangieCandidateEntry,
    root: &ChangieMapping,
    diagnostics: &mut Vec<ChangieDiagnostic>,
) {
    if !projects_enabled {
        return;
    }
    let project = root.first("project");
    let project_keys = configured_project_keys(config);
    match project.map(|n| &n.value) {
        None | Some(ChangieValue::Null) => diagnostics.push(ChangieDiagnostic {
            rule: ChangieRule::FragmentProjectMissing,
            result_class: ChangieResultClass::Finding,
            repo_path: entry.repo_path.clone(),
            field_path: Some(ChangieFieldPath(vec!["project".into()])),
            range: project.map(|n| n.range),
            related_config_ranges: Vec::new(),
            expected_actual: None,
            message: "fragment `project` is required while projects are configured".into(),
            actions: vec![ChangieAction::ChooseConfiguredValue],
        }),
        Some(ChangieValue::String(value)) => {
            // Upstream lookup is by project key; a label-only match is
            // still unknown for the modeled generation.
            if !project_keys.contains(value) {
                diagnostics.push(ChangieDiagnostic {
                    rule: ChangieRule::FragmentProjectUnknown,
                    result_class: ChangieResultClass::Finding,
                    repo_path: entry.repo_path.clone(),
                    field_path: Some(ChangieFieldPath(vec!["project".into()])),
                    range: project.map(|n| n.range),
                    related_config_ranges: Vec::new(),
                    expected_actual: Some(ChangieExpectedActual {
                        expected: "a configured project key".into(),
                        actual: value.clone(),
                    }),
                    message: format!(
                        "fragment project `{value}` does not match a configured project key"
                    ),
                    actions: vec![ChangieAction::ChooseConfiguredValue],
                });
            }
        }
        Some(other) => diagnostics.push(ChangieDiagnostic {
            rule: ChangieRule::FragmentProjectUnknown,
            result_class: ChangieResultClass::Finding,
            repo_path: entry.repo_path.clone(),
            field_path: Some(ChangieFieldPath(vec!["project".into()])),
            range: project.map(|n| n.range),
            related_config_ranges: Vec::new(),
            expected_actual: Some(ChangieExpectedActual {
                expected: "project string".into(),
                actual: shape(other).into(),
            }),
            message: "fragment `project` must be a string".into(),
            actions: vec![ChangieAction::ChooseConfiguredValue],
        }),
    }
}

fn configured_project_keys(config: &ChangieConfigDocument) -> Vec<String> {
    let mut keys = Vec::new();
    if let Some(ChangieValue::Sequence(items)) = field(config, "projects").map(|n| &n.value) {
        for item in items {
            if let ChangieValue::Mapping(mapping) = &item.value
                && let Some(ChangieValue::String(key)) = mapping.first("key").map(|n| &n.value)
            {
                keys.push(key.clone());
            }
        }
    }
    keys
}

fn validate_body(
    config: &ChangieConfigDocument,
    kind: &Option<KindDeclaration>,
    entry: &ChangieCandidateEntry,
    root: &ChangieMapping,
    diagnostics: &mut Vec<ChangieDiagnostic>,
) {
    if kind.as_ref().is_some_and(|kind| kind.skip_body) {
        return;
    }
    let (min_length, max_length) = configured_body_bounds(config);
    let body = root.first("body");
    let text = match body.map(|n| &n.value) {
        None | Some(ChangieValue::Null) => {
            if kind.is_some() || min_length.is_some() || max_length.is_some() {
                diagnostics.push(ChangieDiagnostic {
                    rule: ChangieRule::FragmentBodyMissing,
                    result_class: ChangieResultClass::Finding,
                    repo_path: entry.repo_path.clone(),
                    field_path: Some(ChangieFieldPath(vec!["body".into()])),
                    range: body.map(|n| n.range),
                    related_config_ranges: body_config_ranges(config),
                    expected_actual: None,
                    message: "fragment `body` is required but absent".into(),
                    actions: vec![ChangieAction::InsertMissingField],
                });
            }
            return;
        }
        Some(ChangieValue::EmptyString) => {
            diagnostics.push(ChangieDiagnostic {
                rule: ChangieRule::FragmentBodyMissing,
                result_class: ChangieResultClass::Finding,
                repo_path: entry.repo_path.clone(),
                field_path: Some(ChangieFieldPath(vec!["body".into()])),
                range: body.map(|n| n.range),
                related_config_ranges: body_config_ranges(config),
                expected_actual: Some(ChangieExpectedActual {
                    expected: "non-empty body".into(),
                    actual: "empty".into(),
                }),
                message: "fragment `body` is authored empty".into(),
                actions: vec![ChangieAction::InsertMissingField],
            });
            return;
        }
        Some(ChangieValue::String(text)) => text,
        Some(other) => {
            diagnostics.push(ChangieDiagnostic {
                rule: ChangieRule::FragmentBodyWrongType,
                result_class: ChangieResultClass::Finding,
                repo_path: entry.repo_path.clone(),
                field_path: Some(ChangieFieldPath(vec!["body".into()])),
                range: body.map(|n| n.range),
                related_config_ranges: body_config_ranges(config),
                expected_actual: Some(ChangieExpectedActual {
                    expected: "body string".into(),
                    actual: shape(other).into(),
                }),
                message: "fragment `body` must be a string".into(),
                actions: vec![ChangieAction::InsertMissingField],
            });
            return;
        }
    };
    if let Some(min) = min_length {
        let length = text.len();
        if length < min {
            diagnostics.push(ChangieDiagnostic {
                rule: ChangieRule::FragmentBodyTooShort,
                result_class: ChangieResultClass::Finding,
                repo_path: entry.repo_path.clone(),
                field_path: Some(ChangieFieldPath(vec!["body".into()])),
                range: body.map(|n| n.range),
                related_config_ranges: body_config_ranges(config),
                expected_actual: Some(ChangieExpectedActual {
                    expected: format!("at least {min} {BODY_LENGTH_SEMANTICS}"),
                    actual: format!("{length}"),
                }),
                message: format!(
                    "fragment body is {length} {BODY_LENGTH_SEMANTICS}; the configured minimum is {min}"
                ),
                actions: vec![ChangieAction::OpenRelatedConfigValue],
            });
        }
    }
    if let Some(max) = max_length {
        let length = text.len();
        if length > max {
            diagnostics.push(ChangieDiagnostic {
                rule: ChangieRule::FragmentBodyTooLong,
                result_class: ChangieResultClass::Finding,
                repo_path: entry.repo_path.clone(),
                field_path: Some(ChangieFieldPath(vec!["body".into()])),
                range: body.map(|n| n.range),
                related_config_ranges: body_config_ranges(config),
                expected_actual: Some(ChangieExpectedActual {
                    expected: format!("at most {max} {BODY_LENGTH_SEMANTICS}"),
                    actual: format!("{length}"),
                }),
                message: format!(
                    "fragment body is {length} {BODY_LENGTH_SEMANTICS}; the configured maximum is {max}"
                ),
                actions: vec![ChangieAction::OpenRelatedConfigValue],
            });
        }
    }
}

fn configured_body_bounds(config: &ChangieConfigDocument) -> (Option<usize>, Option<usize>) {
    let mut minimum = None;
    let mut maximum = None;
    if let Some(ChangieValue::Mapping(mapping)) = field(config, "body").map(|n| &n.value) {
        if let Some(ChangieValue::Integer(value)) = mapping.first("minLength").map(|n| &n.value) {
            minimum = Some((*value).max(0) as usize);
        }
        if let Some(ChangieValue::Integer(value)) = mapping.first("maxLength").map(|n| &n.value) {
            maximum = Some((*value).max(0) as usize);
        }
    }
    (minimum, maximum)
}

fn body_config_ranges(config: &ChangieConfigDocument) -> Vec<crate::changie::ChangieSourceRange> {
    field(config, "body")
        .map(|n| vec![n.range])
        .unwrap_or_default()
}

/// Time is expected only when the configuration declares `timeFormat`:
/// upstream authors the fragment `time` field in that layout. Without
/// it, unreleased fragments carry no time and none is required — the
/// static contract follows the configuration, not a house rule.
fn validate_time(
    config: &ChangieConfigDocument,
    entry: &ChangieCandidateEntry,
    root: &ChangieMapping,
    diagnostics: &mut Vec<ChangieDiagnostic>,
) {
    let Some(ChangieValue::String(time_format)) = field(config, "timeFormat").map(|n| &n.value)
    else {
        return;
    };
    let time = root.first("time");
    let authored = match time.map(|n| &n.value) {
        None | Some(ChangieValue::Null) => {
            diagnostics.push(ChangieDiagnostic {
                rule: ChangieRule::FragmentTimeMissing,
                result_class: ChangieResultClass::Finding,
                repo_path: entry.repo_path.clone(),
                field_path: Some(ChangieFieldPath(vec!["time".into()])),
                range: time.map(|n| n.range),
                related_config_ranges: field(config, "timeFormat")
                    .map(|n| n.range)
                    .into_iter()
                    .collect(),
                expected_actual: None,
                message: format!(
                    "fragment `time` is required while timeFormat `{time_format}` is configured"
                ),
                actions: vec![ChangieAction::InsertMissingField],
            });
            return;
        }
        Some(ChangieValue::String(text)) => text,
        Some(other) => {
            diagnostics.push(ChangieDiagnostic {
                rule: ChangieRule::FragmentTimeInvalid,
                result_class: ChangieResultClass::Finding,
                repo_path: entry.repo_path.clone(),
                field_path: Some(ChangieFieldPath(vec!["time".into()])),
                range: time.map(|n| n.range),
                related_config_ranges: field(config, "timeFormat")
                    .map(|n| n.range)
                    .into_iter()
                    .collect(),
                expected_actual: Some(ChangieExpectedActual {
                    expected: "timestamp string".into(),
                    actual: shape(other).into(),
                }),
                message: "fragment `time` must be a string".into(),
                actions: vec![ChangieAction::InsertMissingField],
            });
            return;
        }
    };
    if !rfc3339_shaped(authored) {
        diagnostics.push(ChangieDiagnostic {
            rule: ChangieRule::FragmentTimeInvalid,
            result_class: ChangieResultClass::Finding,
            repo_path: entry.repo_path.clone(),
            field_path: Some(ChangieFieldPath(vec!["time".into()])),
            range: time.map(|n| n.range),
            related_config_ranges: field(config, "timeFormat").map(|n| n.range).into_iter().collect(),
            expected_actual: Some(ChangieExpectedActual {
                expected: "RFC3339-shaped timestamp".into(),
                actual: authored.clone(),
            }),
            message: format!(
                "fragment `time` is not parseable as a timestamp (modeled layout: RFC3339; configured `{time_format}`)"
            ),
            actions: vec![ChangieAction::InsertMissingField],
        });
    }
}

/// Bounded RFC3339 shape check: date `YYYY-MM-DD`, `T`/space separator,
/// `HH:MM:SS`, optional fraction, and `Z` or `±HH:MM` offset. Component
/// ranges are validated; no host timezone or locale is consulted.
fn rfc3339_shaped(text: &str) -> bool {
    let bytes = text.as_bytes();
    let digits = |range: std::ops::Range<usize>| {
        bytes
            .get(range)
            .map(|slice| slice.iter().all(|b| b.is_ascii_digit()))
            .unwrap_or(false)
    };
    if bytes.len() < 20 {
        return false;
    }
    if !digits(0..4) || bytes[4] != b'-' || !digits(5..7) || bytes[7] != b'-' || !digits(8..10) {
        return false;
    }
    if bytes[10] != b'T' && bytes[10] != b't' && bytes[10] != b' ' {
        return false;
    }
    if !digits(11..13)
        || bytes[13] != b':'
        || !digits(14..16)
        || bytes[16] != b':'
        || !digits(17..19)
    {
        return false;
    }
    let month = text.get(5..7).and_then(|v| v.parse::<u32>().ok());
    let day = text.get(8..10).and_then(|v| v.parse::<u32>().ok());
    let hour = text.get(11..13).and_then(|v| v.parse::<u32>().ok());
    let minute = text.get(14..16).and_then(|v| v.parse::<u32>().ok());
    let second = text.get(17..19).and_then(|v| v.parse::<u32>().ok());
    let (Some(month), Some(day), Some(hour), Some(minute), Some(second)) =
        (month, day, hour, minute, second)
    else {
        return false;
    };
    if !(1..=12).contains(&month) || day == 0 || day > 31 || hour > 23 || minute > 59 || second > 60
    {
        return false;
    }
    let mut cursor = 19;
    if bytes.get(cursor) == Some(&b'.') {
        cursor += 1;
        while bytes.get(cursor).is_some_and(|b| b.is_ascii_digit()) {
            cursor += 1;
        }
    }
    match bytes.get(cursor) {
        Some(b'Z') | Some(b'z') => bytes.len() == cursor + 1,
        Some(b'+') | Some(b'-') => {
            bytes.len() == cursor + 6
                && bytes.get(cursor + 3) == Some(&b':')
                && digits(cursor + 1..cursor + 3)
                && digits(cursor + 4..cursor + 6)
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Custom values
// ---------------------------------------------------------------------------

pub(super) struct ActiveChoice {
    pub key: String,
    pub choice_type: String,
    pub optional: bool,
    pub min_int: Option<i64>,
    pub max_int: Option<i64>,
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
    pub enum_options: Vec<String>,
    pub range: crate::changie::ChangieSourceRange,
}

fn active_choices(
    config: &ChangieConfigDocument,
    kind: &Option<KindDeclaration>,
) -> Vec<ActiveChoice> {
    let mut choices = Vec::new();
    let include_global = !kind.as_ref().is_some_and(|kind| kind.skip_global_choices);
    if include_global {
        choices.extend(choice_declarations(
            config,
            field(config, "custom").map(|n| &n.value),
        ));
    }
    if let Some(kind) = kind.as_ref()
        && let Some(ChangieValue::Sequence(items)) = kind_additional_choices(config, &kind.label)
    {
        choices.extend(choice_declarations(
            config,
            Some(&ChangieValue::Sequence(items.clone())),
        ));
    }
    choices
}

fn kind_additional_choices(config: &ChangieConfigDocument, label: &str) -> Option<ChangieValue> {
    let items = field(config, "kinds").map(|n| &n.value)?;
    let ChangieValue::Sequence(items) = items else {
        return None;
    };
    for item in items {
        if let ChangieValue::Mapping(mapping) = &item.value {
            let matches = matches!(
                mapping.first("label").map(|n| &n.value),
                Some(ChangieValue::String(value)) if value == label
            );
            if matches {
                return mapping.first("additionalChoices").map(|n| n.value.clone());
            }
        }
    }
    None
}

fn choice_declarations(
    config: &ChangieConfigDocument,
    value: Option<&ChangieValue>,
) -> Vec<ActiveChoice> {
    let _ = config;
    let mut choices = Vec::new();
    let Some(ChangieValue::Sequence(items)) = value else {
        return choices;
    };
    for item in items {
        let ChangieValue::Mapping(mapping) = &item.value else {
            continue;
        };
        let (Some(ChangieValue::String(key)), Some(ChangieValue::String(choice_type))) = (
            mapping.first("key").map(|n| &n.value),
            mapping.first("type").map(|n| &n.value),
        ) else {
            continue;
        };
        let mut enum_options = Vec::new();
        if let Some(ChangieValue::Sequence(options)) = mapping.first("enum").map(|n| &n.value) {
            for option in options {
                if let ChangieValue::String(option_value) = &option.value {
                    enum_options.push(option_value.clone());
                }
            }
        }
        choices.push(ActiveChoice {
            key: key.clone(),
            choice_type: choice_type.clone(),
            optional: flag(mapping, "optional"),
            min_int: int_option(mapping, "minInt"),
            max_int: int_option(mapping, "maxInt"),
            min_length: int_option(mapping, "minLength").map(|v| v.max(0) as usize),
            max_length: int_option(mapping, "maxLength").map(|v| v.max(0) as usize),
            enum_options,
            range: mapping.first("key").map(|n| n.range).unwrap_or(item.range),
        });
    }
    choices
}

fn int_option(mapping: &ChangieMapping, key: &str) -> Option<i64> {
    match mapping.first(key).map(|n| &n.value) {
        Some(ChangieValue::Integer(value)) => Some(*value),
        _ => None,
    }
}

fn validate_custom_values(
    config: &ChangieConfigDocument,
    kind: &Option<KindDeclaration>,
    entry: &ChangieCandidateEntry,
    root: &ChangieMapping,
    diagnostics: &mut Vec<ChangieDiagnostic>,
) {
    let choices = active_choices(config, kind);
    let authored = root.first("custom").map(|n| &n.value);
    let authored_mapping = match authored {
        Some(ChangieValue::Mapping(mapping)) => Some(mapping),
        Some(ChangieValue::Null) | None => None,
        Some(_) => {
            diagnostics.push(ChangieDiagnostic {
                rule: ChangieRule::FragmentCustomWrongType,
                result_class: ChangieResultClass::Finding,
                repo_path: entry.repo_path.clone(),
                field_path: Some(ChangieFieldPath(vec!["custom".into()])),
                range: root.first("custom").map(|n| n.range),
                related_config_ranges: choices.iter().map(|c| c.range).collect(),
                expected_actual: None,
                message: "fragment `custom` must be a mapping of choice values".into(),
                actions: vec![ChangieAction::OpenRelatedConfigValue],
            });
            return;
        }
    };
    for choice in &choices {
        let value = authored_mapping.and_then(|mapping| mapping.first(&choice.key));
        let present = value.is_some()
            && !matches!(
                value.map(|n| &n.value),
                Some(ChangieValue::Null) | Some(ChangieValue::EmptyString)
            );
        if !present {
            if !choice.optional {
                diagnostics.push(ChangieDiagnostic {
                    rule: ChangieRule::FragmentCustomMissing,
                    result_class: ChangieResultClass::Finding,
                    repo_path: entry.repo_path.clone(),
                    field_path: Some(ChangieFieldPath(vec!["custom".into(), choice.key.clone()])),
                    range: value.map(|n| n.range),
                    related_config_ranges: vec![choice.range],
                    expected_actual: None,
                    message: format!("required custom choice `{}` is absent or empty", choice.key),
                    actions: vec![ChangieAction::InsertMissingField],
                });
            }
            continue;
        }
        let value = value.unwrap_or_else(|| std::panic::panic_any("checked above"));
        match choice.choice_type.as_str() {
            "int" => validate_int_choice(choice, value, entry, diagnostics),
            "enum" => validate_enum_choice(choice, value, entry, diagnostics),
            "string" | "block" => validate_string_choice(choice, value, entry, diagnostics),
            _ => {
                // Unsupported choice types are config-level findings from
                // B1; values under them cannot be judged here.
            }
        }
    }
    // Unconfigured custom keys stay visible rather than disappearing.
    if let Some(mapping) = authored_mapping {
        for authored_entry in &mapping.entries {
            if !choices
                .iter()
                .any(|choice| choice.key == authored_entry.key)
            {
                diagnostics.push(ChangieDiagnostic {
                    rule: ChangieRule::FragmentCustomUnconfigured,
                    result_class: ChangieResultClass::Finding,
                    repo_path: entry.repo_path.clone(),
                    field_path: Some(ChangieFieldPath(vec![
                        "custom".into(),
                        authored_entry.key.clone(),
                    ])),
                    range: Some(authored_entry.key_range),
                    related_config_ranges: choices.iter().map(|c| c.range).collect(),
                    expected_actual: None,
                    message: format!(
                        "custom key `{}` is not an active configured choice for this kind",
                        authored_entry.key
                    ),
                    actions: vec![ChangieAction::ShowStaticVersusRenderLimitation],
                });
            }
        }
    }
}

fn validate_int_choice(
    choice: &ActiveChoice,
    value: &ChangieNode,
    entry: &ChangieCandidateEntry,
    diagnostics: &mut Vec<ChangieDiagnostic>,
) {
    let parsed: Option<i64> = match &value.value {
        ChangieValue::Integer(number) => Some(*number),
        ChangieValue::String(text) => text.parse::<i64>().ok(),
        _ => None,
    };
    let Some(number) = parsed else {
        diagnostics.push(ChangieDiagnostic {
            rule: ChangieRule::FragmentCustomWrongType,
            result_class: ChangieResultClass::Finding,
            repo_path: entry.repo_path.clone(),
            field_path: Some(ChangieFieldPath(vec!["custom".into(), choice.key.clone()])),
            range: Some(value.range),
            related_config_ranges: vec![choice.range],
            expected_actual: Some(ChangieExpectedActual {
                expected: "parseable base-10 signed integer".into(),
                actual: scalar_text(&value.value),
            }),
            message: format!(
                "custom choice `{}` is not a parseable base-10 integer",
                choice.key
            ),
            actions: vec![ChangieAction::InsertMissingField],
        });
        return;
    };
    if let Some(min) = choice.min_int
        && number < min
    {
        let mut diagnostic = out_of_range(choice, value, number, min, "minimum");
        diagnostic.repo_path = entry.repo_path.clone();
        diagnostics.push(diagnostic);
    }
    if let Some(max) = choice.max_int
        && number > max
    {
        let mut diagnostic = out_of_range(choice, value, number, max, "maximum");
        diagnostic.repo_path = entry.repo_path.clone();
        diagnostics.push(diagnostic);
    }
}

fn out_of_range(
    choice: &ActiveChoice,
    value: &ChangieNode,
    number: i64,
    bound: i64,
    which: &str,
) -> ChangieDiagnostic {
    ChangieDiagnostic {
        rule: ChangieRule::FragmentCustomOutOfRange,
        result_class: ChangieResultClass::Finding,
        repo_path: String::new(),
        field_path: Some(ChangieFieldPath(vec!["custom".into(), choice.key.clone()])),
        range: Some(value.range),
        related_config_ranges: vec![choice.range],
        expected_actual: Some(ChangieExpectedActual {
            expected: format!("{which} {bound}"),
            actual: number.to_string(),
        }),
        message: format!(
            "custom choice `{}` value {number} violates the configured {which} {bound}",
            choice.key
        ),
        actions: vec![ChangieAction::OpenRelatedConfigValue],
    }
}

fn validate_enum_choice(
    choice: &ActiveChoice,
    value: &ChangieNode,
    entry: &ChangieCandidateEntry,
    diagnostics: &mut Vec<ChangieDiagnostic>,
) {
    if let ChangieValue::String(text) = &value.value {
        if !choice.enum_options.contains(text) {
            diagnostics.push(ChangieDiagnostic {
                rule: ChangieRule::FragmentCustomUnknownValue,
                result_class: ChangieResultClass::Finding,
                repo_path: entry.repo_path.clone(),
                field_path: Some(ChangieFieldPath(vec!["custom".into(), choice.key.clone()])),
                range: Some(value.range),
                related_config_ranges: vec![choice.range],
                expected_actual: Some(ChangieExpectedActual {
                    expected: format!("one of {}", choice.enum_options.join(" | ")),
                    actual: text.clone(),
                }),
                message: format!(
                    "custom choice `{}` value `{text}` is not a configured option",
                    choice.key
                ),
                actions: vec![ChangieAction::ChooseConfiguredValue],
            });
        }
        return;
    }
    diagnostics.push(ChangieDiagnostic {
        rule: ChangieRule::FragmentCustomWrongType,
        result_class: ChangieResultClass::Finding,
        repo_path: entry.repo_path.clone(),
        field_path: Some(ChangieFieldPath(vec!["custom".into(), choice.key.clone()])),
        range: Some(value.range),
        related_config_ranges: vec![choice.range],
        expected_actual: Some(ChangieExpectedActual {
            expected: "enum option string".into(),
            actual: shape(&value.value).into(),
        }),
        message: format!("custom choice `{}` must be a string option", choice.key),
        actions: vec![ChangieAction::ChooseConfiguredValue],
    });
}

fn validate_string_choice(
    choice: &ActiveChoice,
    value: &ChangieNode,
    entry: &ChangieCandidateEntry,
    diagnostics: &mut Vec<ChangieDiagnostic>,
) {
    let text = match &value.value {
        ChangieValue::String(text) => text.clone(),
        ChangieValue::EmptyString => String::new(),
        other => {
            diagnostics.push(ChangieDiagnostic {
                rule: ChangieRule::FragmentCustomWrongType,
                result_class: ChangieResultClass::Finding,
                repo_path: entry.repo_path.clone(),
                field_path: Some(ChangieFieldPath(vec!["custom".into(), choice.key.clone()])),
                range: Some(value.range),
                related_config_ranges: vec![choice.range],
                expected_actual: Some(ChangieExpectedActual {
                    expected: "string".into(),
                    actual: shape(other).into(),
                }),
                message: format!("custom choice `{}` must be a string", choice.key),
                actions: vec![ChangieAction::InsertMissingField],
            });
            return;
        }
    };
    let length = text.len();
    if let Some(min) = choice.min_length
        && length < min
    {
        diagnostics.push(ChangieDiagnostic {
                rule: ChangieRule::FragmentCustomOutOfRange,
                result_class: ChangieResultClass::Finding,
                repo_path: entry.repo_path.clone(),
                field_path: Some(ChangieFieldPath(vec!["custom".into(), choice.key.clone()])),
                range: Some(value.range),
                related_config_ranges: vec![choice.range],
                expected_actual: Some(ChangieExpectedActual {
                    expected: format!("at least {min} {BODY_LENGTH_SEMANTICS}"),
                    actual: format!("{length}"),
                }),
                message: format!(
                    "custom choice `{}` is {length} {BODY_LENGTH_SEMANTICS}; the configured minimum is {min}",
                    choice.key
                ),
                actions: vec![ChangieAction::OpenRelatedConfigValue],
            });
    }
    if let Some(max) = choice.max_length
        && length > max
    {
        diagnostics.push(ChangieDiagnostic {
                rule: ChangieRule::FragmentCustomOutOfRange,
                result_class: ChangieResultClass::Finding,
                repo_path: entry.repo_path.clone(),
                field_path: Some(ChangieFieldPath(vec!["custom".into(), choice.key.clone()])),
                range: Some(value.range),
                related_config_ranges: vec![choice.range],
                expected_actual: Some(ChangieExpectedActual {
                    expected: format!("at most {max} {BODY_LENGTH_SEMANTICS}"),
                    actual: format!("{length}"),
                }),
                message: format!(
                    "custom choice `{}` is {length} {BODY_LENGTH_SEMANTICS}; the configured maximum is {max}",
                    choice.key
                ),
                actions: vec![ChangieAction::OpenRelatedConfigValue],
            });
    }
}

fn scalar_text(value: &ChangieValue) -> String {
    match value {
        ChangieValue::Boolean(flag) => flag.to_string(),
        ChangieValue::Integer(number) => number.to_string(),
        ChangieValue::String(text) => text.clone(),
        ChangieValue::EmptyString => String::new(),
        ChangieValue::Null => "null".into(),
        other => shape(other).into(),
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
#[path = "fragment_rules_tests.rs"]
mod fragment_rules_tests;
