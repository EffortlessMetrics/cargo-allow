//! Persisted-fragment validation against the compiled effective contract
//! (#3620). Fragment semantics consume the contract only — never the
//! raw configuration — so the contract is the single rule authority and
//! #3613 can project it without another evaluator.

use super::compiled_contract::{
    ChangieCompiledFragmentContractV1, ChoiceType, CompiledChoice, CompiledKind, active_choices,
};
use super::{
    ChangieAction, ChangieCandidateEntry, ChangieDiagnostic, ChangieExpectedActual,
    ChangieFragmentDocument, ChangieResultClass, ChangieRule,
};
use crate::changie::{ChangieFieldPath, ChangieMapping, ChangieNode, ChangieValue};

/// Upstream measures body and custom string lengths in UTF-8 bytes (Go
/// `len` on a string), not runes. Unicode fixtures pin this choice so a
/// future generation change must be a conscious compatibility update.
pub const BODY_LENGTH_SEMANTICS: &str = "utf8-bytes";

pub(super) fn validate_fragment(
    contract: &ChangieCompiledFragmentContractV1,
    entry: &ChangieCandidateEntry,
    fragment: &ChangieFragmentDocument,
    diagnostics: &mut Vec<ChangieDiagnostic>,
) {
    let root = match fragment.root.as_ref().map(|node| &node.value) {
        Some(ChangieValue::Mapping(mapping)) => mapping,
        Some(_) => {
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

    let kind = resolve_kind(contract, entry, root, diagnostics);
    validate_component(contract, entry, root, diagnostics);
    validate_project(contract, entry, root, diagnostics);
    validate_body(contract, kind.as_ref(), entry, root, diagnostics);
    validate_time(contract, entry, root, diagnostics);
    validate_custom_values(contract, kind.as_ref(), entry, root, diagnostics);
}

fn resolve_kind(
    contract: &ChangieCompiledFragmentContractV1,
    entry: &ChangieCandidateEntry,
    root: &ChangieMapping,
    diagnostics: &mut Vec<ChangieDiagnostic>,
) -> Option<CompiledKind> {
    if contract.kinds.is_empty() {
        // No kinds configured: upstream does not gate on kind, so
        // neither does the static contract.
        return None;
    }
    let kind_node = root.first("kind");
    let related: Vec<_> = contract
        .kinds
        .iter()
        .map(|kind| kind.declaration_range)
        .collect();
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
                related_config_ranges: related,
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
            related_config_ranges: related,
            expected_actual: None,
            message: "fragment `kind` is required but absent".into(),
            actions: vec![ChangieAction::InsertMissingField],
        });
        return None;
    };
    // Kind labels are their own canonical persisted identity in the
    // modeled generation (kinds carry no separate key).
    match contract.kinds.iter().find(|kind| kind.label == label) {
        Some(kind) => Some(kind.clone()),
        None => {
            diagnostics.push(ChangieDiagnostic {
                rule: ChangieRule::FragmentKindUnknown,
                result_class: ChangieResultClass::Finding,
                repo_path: entry.repo_path.clone(),
                field_path: Some(ChangieFieldPath(vec!["kind".into()])),
                range: kind_node.map(|n| n.range),
                related_config_ranges: related,
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

fn validate_component(
    contract: &ChangieCompiledFragmentContractV1,
    entry: &ChangieCandidateEntry,
    root: &ChangieMapping,
    diagnostics: &mut Vec<ChangieDiagnostic>,
) {
    if contract.components.is_empty() {
        // No configured components: no hidden repository policy applies.
        return;
    }
    let component = root.first("component");
    match component.map(|n| &n.value) {
        None | Some(ChangieValue::Null) => diagnostics.push(ChangieDiagnostic {
            rule: ChangieRule::FragmentComponentMissing,
            result_class: ChangieResultClass::Finding,
            repo_path: entry.repo_path.clone(),
            field_path: Some(ChangieFieldPath(vec!["component".into()])),
            range: component.map(|n| n.range),
            related_config_ranges: Vec::new(),
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
            related_config_ranges: Vec::new(),
            expected_actual: Some(ChangieExpectedActual {
                expected: "non-empty component".into(),
                actual: "empty".into(),
            }),
            message: "fragment `component` is authored empty".into(),
            actions: vec![ChangieAction::ChooseConfiguredValue],
        }),
        Some(ChangieValue::String(value)) => {
            if !contract.components.contains(value) {
                diagnostics.push(ChangieDiagnostic {
                    rule: ChangieRule::FragmentComponentUnknown,
                    result_class: ChangieResultClass::Finding,
                    repo_path: entry.repo_path.clone(),
                    field_path: Some(ChangieFieldPath(vec!["component".into()])),
                    range: component.map(|n| n.range),
                    related_config_ranges: Vec::new(),
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
            related_config_ranges: Vec::new(),
            expected_actual: Some(ChangieExpectedActual {
                expected: "component string".into(),
                actual: shape(other).into(),
            }),
            message: "fragment `component` must be a string".into(),
            actions: vec![ChangieAction::ChooseConfiguredValue],
        }),
    }
}

fn validate_project(
    contract: &ChangieCompiledFragmentContractV1,
    entry: &ChangieCandidateEntry,
    root: &ChangieMapping,
    diagnostics: &mut Vec<ChangieDiagnostic>,
) {
    if contract.projects.is_empty() {
        return;
    }
    let project = root.first("project");
    let related: Vec<_> = contract
        .projects
        .iter()
        .map(|project| project.declaration_range)
        .collect();
    match project.map(|n| &n.value) {
        None | Some(ChangieValue::Null) => diagnostics.push(ChangieDiagnostic {
            rule: ChangieRule::FragmentProjectMissing,
            result_class: ChangieResultClass::Finding,
            repo_path: entry.repo_path.clone(),
            field_path: Some(ChangieFieldPath(vec!["project".into()])),
            range: project.map(|n| n.range),
            related_config_ranges: related,
            expected_actual: None,
            message: "fragment `project` is required while projects are configured".into(),
            actions: vec![ChangieAction::ChooseConfiguredValue],
        }),
        Some(ChangieValue::String(value)) => {
            if contract.projects.iter().any(|p| p.key == *value) {
                return;
            }
            // Canonical identity law: `changie new` may accept a label,
            // but persisted fragments carry the canonical key. A label
            // match is a source-located finding with a canonicalization
            // action — never a silent normalization.
            if let Some(declared) = contract
                .projects
                .iter()
                .find(|p| p.label.as_deref() == Some(value.as_str()))
            {
                diagnostics.push(ChangieDiagnostic {
                    rule: ChangieRule::FragmentProjectNotCanonical,
                    result_class: ChangieResultClass::Finding,
                    repo_path: entry.repo_path.clone(),
                    field_path: Some(ChangieFieldPath(vec!["project".into()])),
                    range: project.map(|n| n.range),
                    related_config_ranges: vec![declared.declaration_range],
                    expected_actual: Some(ChangieExpectedActual {
                        expected: declared.key.clone(),
                        actual: value.clone(),
                    }),
                    message: format!(
                        "fragment `project` carries the label `{value}`; persisted fragments use the canonical key `{}`",
                        declared.key
                    ),
                    actions: vec![ChangieAction::CanonicalizeConfiguredValue],
                });
                return;
            }
            diagnostics.push(ChangieDiagnostic {
                rule: ChangieRule::FragmentProjectUnknown,
                result_class: ChangieResultClass::Finding,
                repo_path: entry.repo_path.clone(),
                field_path: Some(ChangieFieldPath(vec!["project".into()])),
                range: project.map(|n| n.range),
                related_config_ranges: related,
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
        Some(other) => diagnostics.push(ChangieDiagnostic {
            rule: ChangieRule::FragmentProjectUnknown,
            result_class: ChangieResultClass::Finding,
            repo_path: entry.repo_path.clone(),
            field_path: Some(ChangieFieldPath(vec!["project".into()])),
            range: project.map(|n| n.range),
            related_config_ranges: related,
            expected_actual: Some(ChangieExpectedActual {
                expected: "project string".into(),
                actual: shape(other).into(),
            }),
            message: "fragment `project` must be a string".into(),
            actions: vec![ChangieAction::ChooseConfiguredValue],
        }),
    }
}

fn validate_body(
    contract: &ChangieCompiledFragmentContractV1,
    kind: Option<&CompiledKind>,
    entry: &ChangieCandidateEntry,
    root: &ChangieMapping,
    diagnostics: &mut Vec<ChangieDiagnostic>,
) {
    if kind.is_some_and(|kind| kind.skip_body) {
        return;
    }
    let body = root.first("body");
    let text = match body.map(|n| &n.value) {
        None | Some(ChangieValue::Null) => {
            diagnostics.push(ChangieDiagnostic {
                rule: ChangieRule::FragmentBodyMissing,
                result_class: ChangieResultClass::Finding,
                repo_path: entry.repo_path.clone(),
                field_path: Some(ChangieFieldPath(vec!["body".into()])),
                range: body.map(|n| n.range),
                related_config_ranges: Vec::new(),
                expected_actual: None,
                message: "fragment `body` is required but absent".into(),
                actions: vec![ChangieAction::InsertMissingField],
            });
            return;
        }
        Some(ChangieValue::EmptyString) => {
            diagnostics.push(ChangieDiagnostic {
                rule: ChangieRule::FragmentBodyMissing,
                result_class: ChangieResultClass::Finding,
                repo_path: entry.repo_path.clone(),
                field_path: Some(ChangieFieldPath(vec!["body".into()])),
                range: body.map(|n| n.range),
                related_config_ranges: Vec::new(),
                expected_actual: Some(ChangieExpectedActual {
                    expected: "non-empty body".into(),
                    actual: "empty".into(),
                }),
                message: "fragment `body` is authored empty".into(),
                actions: vec![ChangieAction::InsertMissingField],
            });
            return;
        }
        Some(ChangieValue::String(text)) if text.trim().is_empty() => {
            diagnostics.push(ChangieDiagnostic {
                rule: ChangieRule::FragmentBodyMissing,
                result_class: ChangieResultClass::Finding,
                repo_path: entry.repo_path.clone(),
                field_path: Some(ChangieFieldPath(vec!["body".into()])),
                range: body.map(|n| n.range),
                related_config_ranges: Vec::new(),
                expected_actual: Some(ChangieExpectedActual {
                    expected: "non-whitespace body".into(),
                    actual: "whitespace-only".into(),
                }),
                message: "fragment `body` is authored whitespace-only".into(),
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
                related_config_ranges: Vec::new(),
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
    let length = text.len();
    if let Some(min) = contract.body.min_length
        && length < min
    {
        diagnostics.push(ChangieDiagnostic {
            rule: ChangieRule::FragmentBodyTooShort,
            result_class: ChangieResultClass::Finding,
            repo_path: entry.repo_path.clone(),
            field_path: Some(ChangieFieldPath(vec!["body".into()])),
            range: body.map(|n| n.range),
            related_config_ranges: Vec::new(),
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
    if let Some(max) = contract.body.max_length
        && length > max
    {
        diagnostics.push(ChangieDiagnostic {
            rule: ChangieRule::FragmentBodyTooLong,
            result_class: ChangieResultClass::Finding,
            repo_path: entry.repo_path.clone(),
            field_path: Some(ChangieFieldPath(vec!["body".into()])),
            range: body.map(|n| n.range),
            related_config_ranges: Vec::new(),
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

/// Time is expected only when the configuration declares `timeFormat`:
/// upstream authors the fragment `time` field in that layout. Without
/// it, persisted fragments carry no time and none is required — the
/// static contract follows the configuration, not a house rule.
fn validate_time(
    contract: &ChangieCompiledFragmentContractV1,
    entry: &ChangieCandidateEntry,
    root: &ChangieMapping,
    diagnostics: &mut Vec<ChangieDiagnostic>,
) {
    let Some(time_format) = &contract.time_format else {
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
                related_config_ranges: Vec::new(),
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
                related_config_ranges: Vec::new(),
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
            related_config_ranges: Vec::new(),
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
    if bytes.len() < 20 {
        return false;
    }
    let at = |index: usize| bytes.get(index).copied();
    let digits = |range: std::ops::Range<usize>| {
        bytes
            .get(range)
            .map(|slice| slice.iter().all(|b| b.is_ascii_digit()))
            .unwrap_or(false)
    };
    if !digits(0..4)
        || at(4) != Some(b'-')
        || !digits(5..7)
        || at(7) != Some(b'-')
        || !digits(8..10)
    {
        return false;
    }
    match at(10) {
        Some(b'T') | Some(b't') | Some(b' ') => {}
        _ => return false,
    }
    if !digits(11..13)
        || at(13) != Some(b':')
        || !digits(14..16)
        || at(16) != Some(b':')
        || !digits(17..19)
    {
        return false;
    }
    let number = |range: std::ops::Range<usize>| {
        bytes
            .get(range)
            .and_then(|slice| std::str::from_utf8(slice).ok())
            .and_then(|value| value.parse::<u32>().ok())
    };
    let (Some(month), Some(day), Some(hour), Some(minute), Some(second)) = (
        number(5..7),
        number(8..10),
        number(11..13),
        number(14..16),
        number(17..19),
    ) else {
        return false;
    };
    if !(1..=12).contains(&month) || day == 0 || day > 31 || hour > 23 || minute > 59 || second > 60
    {
        return false;
    }
    let mut cursor = 19;
    if at(cursor) == Some(b'.') {
        cursor += 1;
        while bytes.get(cursor).is_some_and(|b| b.is_ascii_digit()) {
            cursor += 1;
        }
    }
    match at(cursor) {
        Some(b'Z') | Some(b'z') => bytes.len() == cursor + 1,
        Some(b'+') | Some(b'-') => {
            bytes.len() == cursor + 6
                && at(cursor + 3) == Some(b':')
                && digits(cursor + 1..cursor + 3)
                && digits(cursor + 4..cursor + 6)
        }
        _ => false,
    }
}

fn validate_custom_values(
    contract: &ChangieCompiledFragmentContractV1,
    kind: Option<&CompiledKind>,
    entry: &ChangieCandidateEntry,
    root: &ChangieMapping,
    diagnostics: &mut Vec<ChangieDiagnostic>,
) {
    let choices: Vec<&CompiledChoice> = match kind.map(|kind| kind.label.as_str()) {
        Some(label) => active_choices(contract, label),
        None => contract
            .choices
            .iter()
            .filter(|choice| choice.scope.is_global())
            .collect(),
    };
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
                related_config_ranges: choices.iter().map(|c| c.declaration_range).collect(),
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
                    related_config_ranges: vec![choice.declaration_range],
                    expected_actual: None,
                    message: format!("required custom choice `{}` is absent or empty", choice.key),
                    actions: vec![ChangieAction::InsertMissingField],
                });
            }
            continue;
        }
        let value = value.unwrap_or_else(|| std::panic::panic_any("checked above"));
        match choice.choice_type {
            ChoiceType::Int => validate_int_choice(choice, value, entry, diagnostics),
            ChoiceType::Enum => validate_enum_choice(choice, value, entry, diagnostics),
            ChoiceType::String | ChoiceType::Block => {
                validate_string_choice(choice, value, entry, diagnostics)
            }
        }
    }
    // Unconfigured custom keys stay visible rather than disappearing.
    // Post-generated keys are, by construction, never required authored
    // input: only configured choices are required, and any other key is
    // reported here instead of being demanded.
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
                    related_config_ranges: choices.iter().map(|c| c.declaration_range).collect(),
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
    choice: &CompiledChoice,
    value: &ChangieNode,
    entry: &ChangieCandidateEntry,
    diagnostics: &mut Vec<ChangieDiagnostic>,
) {
    // Changie persists custom values as strings, including integer
    // choices: an authored YAML integer is accepted, and a string-valued
    // integer must parse as base-10 — neither shape is required over
    // the other.
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
            related_config_ranges: vec![choice.declaration_range],
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
    for (bound, which) in [(choice.min_int, "minimum"), (choice.max_int, "maximum")] {
        if let Some(bound) = bound {
            let violated = if which == "minimum" {
                number < bound
            } else {
                number > bound
            };
            if violated {
                let mut diagnostic = out_of_range(choice, value, number, bound, which);
                diagnostic.repo_path = entry.repo_path.clone();
                diagnostics.push(diagnostic);
            }
        }
    }
}

fn out_of_range(
    choice: &CompiledChoice,
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
        related_config_ranges: vec![choice.declaration_range],
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
    choice: &CompiledChoice,
    value: &ChangieNode,
    entry: &ChangieCandidateEntry,
    diagnostics: &mut Vec<ChangieDiagnostic>,
) {
    if let ChangieValue::String(text) = &value.value {
        // Enum comparison is exact in the modeled generation: no case
        // folding.
        if !choice.enum_options.contains(text) {
            diagnostics.push(ChangieDiagnostic {
                rule: ChangieRule::FragmentCustomUnknownValue,
                result_class: ChangieResultClass::Finding,
                repo_path: entry.repo_path.clone(),
                field_path: Some(ChangieFieldPath(vec!["custom".into(), choice.key.clone()])),
                range: Some(value.range),
                related_config_ranges: vec![choice.declaration_range],
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
        related_config_ranges: vec![choice.declaration_range],
        expected_actual: Some(ChangieExpectedActual {
            expected: "enum option string".into(),
            actual: shape(&value.value).into(),
        }),
        message: format!("custom choice `{}` must be a string option", choice.key),
        actions: vec![ChangieAction::ChooseConfiguredValue],
    });
}

fn validate_string_choice(
    choice: &CompiledChoice,
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
                related_config_ranges: vec![choice.declaration_range],
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
    for (bound, which) in [
        (choice.min_length, "minimum"),
        (choice.max_length, "maximum"),
    ] {
        if let Some(bound) = bound {
            let violated = if which == "minimum" {
                length < bound
            } else {
                length > bound
            };
            if violated {
                diagnostics.push(ChangieDiagnostic {
                    rule: ChangieRule::FragmentCustomOutOfRange,
                    result_class: ChangieResultClass::Finding,
                    repo_path: entry.repo_path.clone(),
                    field_path: Some(ChangieFieldPath(vec!["custom".into(), choice.key.clone()])),
                    range: Some(value.range),
                    related_config_ranges: vec![choice.declaration_range],
                    expected_actual: Some(ChangieExpectedActual {
                        expected: format!("{which} {bound} {BODY_LENGTH_SEMANTICS}"),
                        actual: format!("{length}"),
                    }),
                    message: format!(
                        "custom choice `{}` is {length} {BODY_LENGTH_SEMANTICS}; the configured {which} is {bound}",
                        choice.key
                    ),
                    actions: vec![ChangieAction::OpenRelatedConfigValue],
                });
            }
        }
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
