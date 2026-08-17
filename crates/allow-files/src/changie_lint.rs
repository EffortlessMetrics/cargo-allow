//! Changie static authoring contract — validation slice (#3589 PR B1).
//!
//! `lint` answers: does this configuration (and, in PR B2, fragment
//! population) satisfy the statically checkable Changie 1.25
//! authoring/discovery contract the configuration declares? The
//! validator is pure: caller-supplied parsed documents and entry states
//! only — no filesystem, Git, network, process, mutation, or release
//! decision. Where upstream behavior is operation-specific or
//! template-execution-dependent, the report records `Unsupported` or
//! `NotProven` completeness instead of guessing.

use crate::changie::{
    CHANGIE_COMPATIBILITY_GENERATION, ChangieConfigDocument, ChangieFieldPath,
    ChangieFragmentDocument, ChangieNode, ChangieSourceRange, ChangieValue,
};

// ---------------------------------------------------------------------------
// Diagnostic and report model
// ---------------------------------------------------------------------------

/// Stable, provider-neutral rule identities (#3589). Independent
/// falsifiers get independent IDs; tightening renames must keep identity
/// stable per generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangieRule {
    ConfigMalformed,
    ConfigDuplicateKey,
    ConfigUnknownField,
    ConfigUnsupportedSemantics,
    ConfigPathInvalid,
    ConfigDuplicateProject,
    ConfigDuplicateComponent,
    ConfigDuplicateKind,
    ConfigInvalidConstraint,
    FragmentPathNotDiscovered,
    FragmentEntryUnsupported,
    FragmentMalformed,
}

impl ChangieRule {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ConfigMalformed => "changie.config.malformed",
            Self::ConfigDuplicateKey => "changie.config.duplicate_key",
            Self::ConfigUnknownField => "changie.config.unknown_field",
            Self::ConfigUnsupportedSemantics => "changie.config.unsupported_semantics",
            Self::ConfigPathInvalid => "changie.config.path_invalid",
            Self::ConfigDuplicateProject => "changie.config.duplicate_project",
            Self::ConfigDuplicateComponent => "changie.config.duplicate_component",
            Self::ConfigDuplicateKind => "changie.config.duplicate_kind",
            Self::ConfigInvalidConstraint => "changie.config.invalid_constraint",
            Self::FragmentPathNotDiscovered => "changie.fragment.path_not_discovered",
            Self::FragmentEntryUnsupported => "changie.fragment.entry_unsupported",
            Self::FragmentMalformed => "changie.fragment.malformed",
        }
    }
}

/// Result class: what kind of answer this diagnostic is. Kept separate
/// from severity and from any consumer's blocking posture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangieResultClass {
    Finding,
    Malformed,
    Unsupported,
    Partial,
}

impl ChangieResultClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Finding => "finding",
            Self::Malformed => "malformed",
            Self::Unsupported => "unsupported",
            Self::Partial => "partial",
        }
    }
}

/// Deterministic, non-mutating action descriptors for later CLI/LSP
/// projection. Never an edit, never an invented judgment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangieAction {
    OpenRelatedConfigValue,
    ChooseConfiguredValue,
    InsertMissingField,
    ShowStaticVersusRenderLimitation,
}

impl ChangieAction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OpenRelatedConfigValue => "open related configured value",
            Self::ChooseConfiguredValue => "choose one configured value",
            Self::InsertMissingField => "insert the missing field after supplying a value",
            Self::ShowStaticVersusRenderLimitation => "show static-versus-render limitation",
        }
    }
}

/// A typed expected/actual pair where a constraint has a comparable
/// value side. Values are rendered strings so reports stay portable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangieExpectedActual {
    pub expected: String,
    pub actual: String,
}

/// One diagnostic with full source correlation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangieDiagnostic {
    pub rule: ChangieRule,
    pub result_class: ChangieResultClass,
    pub repo_path: String,
    pub field_path: Option<ChangieFieldPath>,
    pub range: Option<ChangieSourceRange>,
    /// Ranges in the config declaration this diagnostic relates to.
    pub related_config_ranges: Vec<ChangieSourceRange>,
    pub expected_actual: Option<ChangieExpectedActual>,
    pub message: String,
    pub actions: Vec<ChangieAction>,
}

/// What the report did and did not prove.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangieCompleteness {
    /// All modeled static rules ran to a decision.
    Complete,
    /// Some inputs were missing or partial (e.g. fragment parse states
    /// not supplied); findings stand but coverage is narrower.
    Partial,
    /// Operation-affecting configuration is present but not evaluated
    /// (templates, post, auto, mutation-only fields): no clean static
    /// claim beyond the modeled contract.
    NotProven,
}

/// Caller-supplied source entry state for discovery classification.
/// The distinct states stay distinct (#3589): a directory is not a
/// missing file, a symlink is not unsupported, and so on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangieEntryState {
    File,
    Directory,
    Symlink,
    Gitlink,
    UnsupportedMode,
    Missing,
    DeletedTracked,
    RenamedAway,
    TypeChanged,
    NotUtf8,
}

impl ChangieEntryState {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Directory => "directory",
            Self::Symlink => "symlink",
            Self::Gitlink => "gitlink",
            Self::UnsupportedMode => "unsupported_mode",
            Self::Missing => "missing",
            Self::DeletedTracked => "deleted_tracked",
            Self::RenamedAway => "renamed_away",
            Self::TypeChanged => "type_changed",
            Self::NotUtf8 => "not_utf8",
        }
    }
}

/// A caller-supplied candidate entry: a repository-relative path plus
/// its state and, where applicable, the parsed fragment document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangieCandidateEntry {
    pub repo_path: String,
    pub state: ChangieEntryState,
    pub fragment: Option<ChangieFragmentDocument>,
}

/// The lint input: config plus candidate entries. Everything is
/// caller-supplied; the validator never invents inventory.
#[derive(Debug, Clone)]
pub struct ChangieLintCandidate {
    pub config: ChangieConfigDocument,
    pub entries: Vec<ChangieCandidateEntry>,
}

/// The lint output.
#[derive(Debug, Clone)]
pub struct ChangieLintReport {
    pub generation: &'static str,
    pub diagnostics: Vec<ChangieDiagnostic>,
    /// Entry paths classified as discovered ordinary fragments under
    /// `<changesDir>/<unreleasedDir>/<direct child>.yaml`.
    pub discovered: Vec<String>,
    /// Entry paths examined but not ordinary discovered fragments.
    pub not_discovered: Vec<String>,
    pub completeness: ChangieCompleteness,
    /// Dimensions this report explicitly does not prove.
    pub not_proven_dimensions: Vec<&'static str>,
}

// ---------------------------------------------------------------------------
// lint()
// ---------------------------------------------------------------------------

pub fn lint(candidate: ChangieLintCandidate) -> ChangieLintReport {
    let config = &candidate.config;
    let mut diagnostics = Vec::new();
    let mut completeness = ChangieCompleteness::Complete;
    let mut not_proven = Vec::new();

    // Parse-level config diagnostics flow through with rule identity.
    for diagnostic in &config.diagnostics {
        diagnostics.push(ChangieDiagnostic {
            rule: ChangieRule::ConfigMalformed,
            result_class: ChangieResultClass::Malformed,
            repo_path: config.source.repo_path().to_string(),
            field_path: diagnostic.path.clone(),
            range: diagnostic.range,
            related_config_ranges: Vec::new(),
            expected_actual: None,
            message: diagnostic.message.clone(),
            actions: vec![ChangieAction::InsertMissingField],
        });
    }
    // Duplicates preserved by the parser become rule-identified findings.
    if let Some(ChangieValue::Mapping(mapping)) = config.root.as_ref().map(|n| &n.value) {
        let mut seen = std::collections::BTreeSet::new();
        for entry in &mapping.entries {
            if !seen.insert(entry.key.clone()) {
                diagnostics.push(ChangieDiagnostic {
                        rule: ChangieRule::ConfigDuplicateKey,
                        result_class: ChangieResultClass::Finding,
                        repo_path: config.source.repo_path().to_string(),
                        field_path: Some(ChangieFieldPath(vec![entry.key.clone()])),
                        range: Some(entry.key_range),
                        related_config_ranges: Vec::new(),
                        expected_actual: Some(ChangieExpectedActual {
                            expected: "one authored key".into(),
                            actual: format!("duplicate key {}", entry.key),
                        }),
                        message: format!(
                            "duplicate mapping key `{}`; last-writer-wins would silently drop the earlier value",
                            entry.key
                        ),
                        actions: vec![ChangieAction::OpenRelatedConfigValue],
                    });
            }
        }
    }
    for unknown in &config.unknown_fields {
        diagnostics.push(ChangieDiagnostic {
            rule: ChangieRule::ConfigUnknownField,
            result_class: ChangieResultClass::Finding,
            repo_path: config.source.repo_path().to_string(),
            field_path: Some(unknown.path.clone()),
            range: Some(unknown.range),
            related_config_ranges: Vec::new(),
            expected_actual: None,
            message: format!("unknown field `{}` for the modeled surface", unknown.path),
            actions: vec![ChangieAction::OpenRelatedConfigValue],
        });
    }
    for unsupported in &config.unsupported_fields {
        diagnostics.push(ChangieDiagnostic {
            rule: ChangieRule::ConfigUnsupportedSemantics,
            result_class: ChangieResultClass::Unsupported,
            repo_path: config.source.repo_path().to_string(),
            field_path: Some(unsupported.path.clone()),
            range: Some(unsupported.range),
            related_config_ranges: Vec::new(),
            expected_actual: None,
            message: format!(
                "field `{}` uses an anchor/alias the static sensor does not evaluate",
                unsupported.path
            ),
            actions: vec![ChangieAction::ShowStaticVersusRenderLimitation],
        });
        completeness = ChangieCompleteness::NotProven;
    }

    // Path safety for the discovery-bearing roots.
    validate_config_paths(config, &mut diagnostics);

    // Components, kinds, body constraints, custom choices.
    validate_components(config, &mut diagnostics);
    validate_kinds(config, &mut diagnostics);
    validate_body_constraints(config, &mut diagnostics);
    validate_custom_choices(config, &mut diagnostics);

    // Operation-affecting fields the static contract deliberately does
    // not evaluate: completeness, not errors.
    for opaque in [
        "headerFormat",
        "footerFormat",
        "changeFormat",
        "versionFormat",
        "fragmentFileFormat",
        "versionFileFormat",
    ] {
        if field_is_present(config, opaque) {
            not_proven.push("template_render_semantics");
            break;
        }
    }
    if field_is_present(config, "post") {
        not_proven.push("post_execution");
        completeness = ChangieCompleteness::NotProven;
    }
    if field_is_present(config, "replacements") {
        not_proven.push("replacement_execution");
        completeness = ChangieCompleteness::NotProven;
    }
    if not_proven.contains(&"template_render_semantics") {
        completeness = ChangieCompleteness::NotProven;
    }

    // Candidate entry discovery classification.
    let mut discovered = Vec::new();
    let mut not_discovered = Vec::new();
    classify_entries(
        config,
        &candidate.entries,
        &mut diagnostics,
        &mut discovered,
        &mut not_discovered,
    );
    if candidate.entries.is_empty() {
        completeness = match completeness {
            ChangieCompleteness::NotProven => ChangieCompleteness::NotProven,
            _ => ChangieCompleteness::Partial,
        };
    }

    ChangieLintReport {
        generation: CHANGIE_COMPATIBILITY_GENERATION,
        diagnostics,
        discovered,
        not_discovered,
        completeness,
        not_proven_dimensions: not_proven,
    }
}

// ---------------------------------------------------------------------------
// Configuration consistency
// ---------------------------------------------------------------------------

fn mapping_of(config: &ChangieConfigDocument) -> Option<&crate::changie::ChangieMapping> {
    config.root.as_ref().and_then(|node| match &node.value {
        ChangieValue::Mapping(mapping) => Some(mapping),
        _ => None,
    })
}

fn field_node<'a>(
    config: &'a ChangieConfigDocument,
    key: &str,
) -> Option<&'a crate::changie::ChangieNode> {
    mapping_of(config).and_then(|mapping| mapping.first(key))
}

fn field_is_present(config: &ChangieConfigDocument, key: &str) -> bool {
    field_node(config, key).is_some()
}

/// Repository-relative safety for configured paths: non-empty strings,
/// no absolute paths, no parent traversal, no drive letters, and a
/// normalization-collision check across the path-bearing fields.
fn validate_config_paths(config: &ChangieConfigDocument, diagnostics: &mut Vec<ChangieDiagnostic>) {
    let path_fields = [
        "changesDir",
        "unreleasedDir",
        "headerPath",
        "changelogPath",
        "versionHeaderPath",
        "versionFooterPath",
    ];
    let mut normalized: Vec<(String, String)> = Vec::new();
    for field in path_fields {
        let Some(node) = field_node(config, field) else {
            continue;
        };
        let ChangieValue::String(raw) = &node.value else {
            if matches!(node.value, ChangieValue::Null) {
                continue;
            }
            diagnostics.push(ChangieDiagnostic {
                rule: ChangieRule::ConfigPathInvalid,
                result_class: ChangieResultClass::Finding,
                repo_path: config.source.repo_path().to_string(),
                field_path: Some(ChangieFieldPath(vec![field.to_string()])),
                range: Some(node.range),
                related_config_ranges: Vec::new(),
                expected_actual: Some(ChangieExpectedActual {
                    expected: "path string".into(),
                    actual: shape(&node.value).into(),
                }),
                message: format!("`{field}` must be a path string"),
                actions: vec![ChangieAction::OpenRelatedConfigValue],
            });
            continue;
        };
        if let Err(reason) = safe_repo_relative(raw) {
            diagnostics.push(ChangieDiagnostic {
                rule: ChangieRule::ConfigPathInvalid,
                result_class: ChangieResultClass::Finding,
                repo_path: config.source.repo_path().to_string(),
                field_path: Some(ChangieFieldPath(vec![field.to_string()])),
                range: Some(node.range),
                related_config_ranges: Vec::new(),
                expected_actual: Some(ChangieExpectedActual {
                    expected: "repository-relative path".into(),
                    actual: raw.clone(),
                }),
                message: format!("`{field}` is not a safe repository-relative path: {reason}"),
                actions: vec![ChangieAction::OpenRelatedConfigValue],
            });
            continue;
        }
        let canonical = normalize_repo_relative(raw);
        if let Some((prior_field, _)) = normalized
            .iter()
            .find(|(field_name, path)| field_name != field && path == &canonical)
            .map(|(a, b)| (a.to_string(), b.clone()))
        {
            let related = field_node(config, &prior_field).map(|n| n.range);
            diagnostics.push(ChangieDiagnostic {
                rule: ChangieRule::ConfigPathInvalid,
                result_class: ChangieResultClass::Finding,
                repo_path: config.source.repo_path().to_string(),
                field_path: Some(ChangieFieldPath(vec![field.to_string()])),
                range: Some(node.range),
                related_config_ranges: related.into_iter().collect(),
                expected_actual: Some(ChangieExpectedActual {
                    expected: "distinct normalized paths".into(),
                    actual: format!("both normalize to `{canonical}`"),
                }),
                message: format!(
                    "`{field}` normalizes onto the same path as `{prior_field}` ({canonical})"
                ),
                actions: vec![ChangieAction::OpenRelatedConfigValue],
            });
        } else {
            normalized.push((field.to_string(), canonical));
        }
    }
    // The discovery roots are load-bearing: their absence or non-string
    // shape is a finding when the config claims the default population.
    for required in ["changesDir", "unreleasedDir"] {
        match field_node(config, required) {
            None => diagnostics.push(ChangieDiagnostic {
                rule: ChangieRule::ConfigPathInvalid,
                result_class: ChangieResultClass::Finding,
                repo_path: config.source.repo_path().to_string(),
                field_path: Some(ChangieFieldPath(vec![required.to_string()])),
                range: None,
                related_config_ranges: Vec::new(),
                expected_actual: None,
                message: format!(
                    "`{required}` is absent; the default fragment population root is not derivable"
                ),
                actions: vec![ChangieAction::InsertMissingField],
            }),
            Some(node) if matches!(node.value, ChangieValue::Null) => {
                diagnostics.push(ChangieDiagnostic {
                    rule: ChangieRule::ConfigPathInvalid,
                    result_class: ChangieResultClass::Finding,
                    repo_path: config.source.repo_path().to_string(),
                    field_path: Some(ChangieFieldPath(vec![required.to_string()])),
                    range: Some(node.range),
                    related_config_ranges: Vec::new(),
                    expected_actual: Some(ChangieExpectedActual {
                        expected: "path string".into(),
                        actual: "null".into(),
                    }),
                    message: format!("`{required}` is authored null"),
                    actions: vec![ChangieAction::InsertMissingField],
                })
            }
            _ => {}
        }
    }
}

fn safe_repo_relative(raw: &str) -> Result<(), String> {
    if raw.is_empty() {
        return Err("empty path".into());
    }
    if raw.starts_with('/') || raw.starts_with('\\') {
        return Err("absolute path".into());
    }
    if raw.contains(':') {
        return Err("drive letter or scheme separator".into());
    }
    let mut depth: i64 = 0;
    for segment in raw.replace('\\', "/").split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                depth -= 1;
                if depth < 0 {
                    return Err("escapes the repository root".into());
                }
            }
            _ => depth += 1,
        }
    }
    Ok(())
}

fn normalize_repo_relative(raw: &str) -> String {
    let owned = raw.replace("\\", "/");
    let mut parts: Vec<&str> = Vec::new();
    for segment in owned.split('/') {
        match segment {
            "" | "." | ".." => {}
            other => parts.push(other),
        }
    }
    parts.join("/")
}

fn validate_components(config: &ChangieConfigDocument, diagnostics: &mut Vec<ChangieDiagnostic>) {
    let Some(node) = field_node(config, "components") else {
        return;
    };
    let ChangieValue::Sequence(items) = &node.value else {
        report_wrong_shape(config, "components", "sequence", &node.value, diagnostics);
        return;
    };
    let mut seen = std::collections::BTreeSet::new();
    for item in items {
        // A quoted empty string is an authored empty component, not a
        // wrong type; both empty shapes report as empty.
        if matches!(&item.value, ChangieValue::EmptyString) {
            diagnostics.push(ChangieDiagnostic {
                rule: ChangieRule::ConfigInvalidConstraint,
                result_class: ChangieResultClass::Finding,
                repo_path: config.source.repo_path().to_string(),
                field_path: Some(ChangieFieldPath(vec!["components".into()])),
                range: Some(item.range),
                related_config_ranges: Vec::new(),
                expected_actual: Some(ChangieExpectedActual {
                    expected: "non-empty component".into(),
                    actual: "empty".into(),
                }),
                message: "configured component is empty".into(),
                actions: vec![ChangieAction::OpenRelatedConfigValue],
            });
            continue;
        }
        match &item.value {
            ChangieValue::String(value) => {
                if value.trim().is_empty() || matches!(&item.value, ChangieValue::EmptyString) {
                    diagnostics.push(ChangieDiagnostic {
                        rule: ChangieRule::ConfigInvalidConstraint,
                        result_class: ChangieResultClass::Finding,
                        repo_path: config.source.repo_path().to_string(),
                        field_path: Some(ChangieFieldPath(vec!["components".into()])),
                        range: Some(item.range),
                        related_config_ranges: Vec::new(),
                        expected_actual: Some(ChangieExpectedActual {
                            expected: "non-empty component".into(),
                            actual: "empty".into(),
                        }),
                        message: "configured component is empty".into(),
                        actions: vec![ChangieAction::OpenRelatedConfigValue],
                    });
                }
                if !seen.insert(value.clone()) {
                    diagnostics.push(ChangieDiagnostic {
                        rule: ChangieRule::ConfigDuplicateComponent,
                        result_class: ChangieResultClass::Finding,
                        repo_path: config.source.repo_path().to_string(),
                        field_path: Some(ChangieFieldPath(vec!["components".into()])),
                        range: Some(item.range),
                        related_config_ranges: Vec::new(),
                        expected_actual: Some(ChangieExpectedActual {
                            expected: "unique component values".into(),
                            actual: value.clone(),
                        }),
                        message: format!("duplicate configured component `{value}`"),
                        actions: vec![ChangieAction::OpenRelatedConfigValue],
                    });
                }
            }
            other => diagnostics.push(ChangieDiagnostic {
                rule: ChangieRule::ConfigInvalidConstraint,
                result_class: ChangieResultClass::Finding,
                repo_path: config.source.repo_path().to_string(),
                field_path: Some(ChangieFieldPath(vec!["components".into()])),
                range: Some(item.range),
                related_config_ranges: Vec::new(),
                expected_actual: Some(ChangieExpectedActual {
                    expected: "component string".into(),
                    actual: shape(other).into(),
                }),
                message: "configured component must be a string".into(),
                actions: vec![ChangieAction::OpenRelatedConfigValue],
            }),
        }
    }
}

fn validate_kinds(config: &ChangieConfigDocument, diagnostics: &mut Vec<ChangieDiagnostic>) {
    let Some(node) = field_node(config, "kinds") else {
        return;
    };
    let ChangieValue::Sequence(items) = &node.value else {
        report_wrong_shape(config, "kinds", "sequence", &node.value, diagnostics);
        return;
    };
    let mut seen = std::collections::BTreeSet::new();
    for item in items {
        let ChangieValue::Mapping(mapping) = &item.value else {
            diagnostics.push(ChangieDiagnostic {
                rule: ChangieRule::ConfigInvalidConstraint,
                result_class: ChangieResultClass::Finding,
                repo_path: config.source.repo_path().to_string(),
                field_path: Some(ChangieFieldPath(vec!["kinds".into()])),
                range: Some(item.range),
                related_config_ranges: Vec::new(),
                expected_actual: Some(ChangieExpectedActual {
                    expected: "kind mapping with `label`".into(),
                    actual: shape(&item.value).into(),
                }),
                message: "each kind must be a mapping with a `label`".into(),
                actions: vec![ChangieAction::OpenRelatedConfigValue],
            });
            continue;
        };
        let Some(label_node) = mapping.first("label") else {
            diagnostics.push(ChangieDiagnostic {
                rule: ChangieRule::ConfigInvalidConstraint,
                result_class: ChangieResultClass::Finding,
                repo_path: config.source.repo_path().to_string(),
                field_path: Some(ChangieFieldPath(vec!["kinds".into()])),
                range: Some(item.range),
                related_config_ranges: Vec::new(),
                expected_actual: None,
                message: "kind is missing its `label`".into(),
                actions: vec![ChangieAction::InsertMissingField],
            });
            continue;
        };
        let ChangieValue::String(label) = &label_node.value else {
            diagnostics.push(ChangieDiagnostic {
                rule: ChangieRule::ConfigInvalidConstraint,
                result_class: ChangieResultClass::Finding,
                repo_path: config.source.repo_path().to_string(),
                field_path: Some(ChangieFieldPath(vec!["kinds".into()])),
                range: Some(label_node.range),
                related_config_ranges: Vec::new(),
                expected_actual: Some(ChangieExpectedActual {
                    expected: "label string".into(),
                    actual: shape(&label_node.value).into(),
                }),
                message: "kind `label` must be a non-empty string".into(),
                actions: vec![ChangieAction::OpenRelatedConfigValue],
            });
            continue;
        };
        if !seen.insert(label.clone()) {
            diagnostics.push(ChangieDiagnostic {
                rule: ChangieRule::ConfigDuplicateKind,
                result_class: ChangieResultClass::Finding,
                repo_path: config.source.repo_path().to_string(),
                field_path: Some(ChangieFieldPath(vec!["kinds".into()])),
                range: Some(label_node.range),
                related_config_ranges: Vec::new(),
                expected_actual: Some(ChangieExpectedActual {
                    expected: "unique kind labels".into(),
                    actual: label.clone(),
                }),
                message: format!("duplicate kind label `{label}`"),
                actions: vec![ChangieAction::OpenRelatedConfigValue],
            });
        }
    }
}

fn report_wrong_shape(
    config: &ChangieConfigDocument,
    field: &str,
    expected: &str,
    actual: &ChangieValue,
    diagnostics: &mut Vec<ChangieDiagnostic>,
) {
    let range = field_node(config, field).map(|n| n.range);
    diagnostics.push(ChangieDiagnostic {
        rule: ChangieRule::ConfigInvalidConstraint,
        result_class: ChangieResultClass::Finding,
        repo_path: config.source.repo_path().to_string(),
        field_path: Some(ChangieFieldPath(vec![field.to_string()])),
        range,
        related_config_ranges: Vec::new(),
        expected_actual: Some(ChangieExpectedActual {
            expected: expected.into(),
            actual: shape(actual).into(),
        }),
        message: format!("`{field}` must be a {expected}"),
        actions: vec![ChangieAction::OpenRelatedConfigValue],
    });
}

/// `body` constraints: integer bounds, non-negative, min <= max.
/// Values are compared as authored; no string coercion.
fn validate_body_constraints(
    config: &ChangieConfigDocument,
    diagnostics: &mut Vec<ChangieDiagnostic>,
) {
    let Some(node) = field_node(config, "body") else {
        return;
    };
    let ChangieValue::Mapping(mapping) = &node.value else {
        report_wrong_shape(config, "body", "mapping", &node.value, diagnostics);
        return;
    };
    let mut minimum: Option<i64> = None;
    let mut maximum: Option<i64> = None;
    for (key, expected) in [("minLength", &mut minimum), ("maxLength", &mut maximum)] {
        if let Some(bound) = mapping.first(key) {
            match &bound.value {
                ChangieValue::Integer(value) if *value >= 0 => {
                    *expected = Some(*value);
                }
                ChangieValue::Integer(value) => diagnostics.push(ChangieDiagnostic {
                    rule: ChangieRule::ConfigInvalidConstraint,
                    result_class: ChangieResultClass::Finding,
                    repo_path: config.source.repo_path().to_string(),
                    field_path: Some(ChangieFieldPath(vec!["body".into(), key.into()])),
                    range: Some(bound.range),
                    related_config_ranges: Vec::new(),
                    expected_actual: Some(ChangieExpectedActual {
                        expected: "non-negative integer".into(),
                        actual: value.to_string(),
                    }),
                    message: format!("`body.{key}` must be non-negative"),
                    actions: vec![ChangieAction::OpenRelatedConfigValue],
                }),
                other => diagnostics.push(ChangieDiagnostic {
                    rule: ChangieRule::ConfigInvalidConstraint,
                    result_class: ChangieResultClass::Finding,
                    repo_path: config.source.repo_path().to_string(),
                    field_path: Some(ChangieFieldPath(vec!["body".into(), key.into()])),
                    range: Some(bound.range),
                    related_config_ranges: Vec::new(),
                    expected_actual: Some(ChangieExpectedActual {
                        expected: "integer".into(),
                        actual: shape(other).into(),
                    }),
                    message: format!("`body.{key}` must be an integer"),
                    actions: vec![ChangieAction::OpenRelatedConfigValue],
                }),
            }
        }
    }
    if let (Some(min), Some(max)) = (minimum, maximum)
        && min > max
    {
        let ranges: Vec<ChangieSourceRange> = ["minLength", "maxLength"]
            .iter()
            .filter_map(|key| mapping.first(key).map(|n| n.range))
            .collect();
        diagnostics.push(ChangieDiagnostic {
            rule: ChangieRule::ConfigInvalidConstraint,
            result_class: ChangieResultClass::Finding,
            repo_path: config.source.repo_path().to_string(),
            field_path: Some(ChangieFieldPath(vec!["body".into(), "minLength".into()])),
            range: mapping.first("minLength").map(|n| n.range),
            related_config_ranges: ranges,
            expected_actual: Some(ChangieExpectedActual {
                expected: format!("minLength {min} <= maxLength {max}"),
                actual: format!("{min} > {max}"),
            }),
            message: "`body.minLength` exceeds `body.maxLength`".into(),
            actions: vec![ChangieAction::OpenRelatedConfigValue],
        });
    }
}

/// The custom-choice contract: unique non-empty keys, supported types,
/// bounds compatible with the type and each other, enum option
/// non-emptiness and uniqueness.
fn validate_custom_choices(
    config: &ChangieConfigDocument,
    diagnostics: &mut Vec<ChangieDiagnostic>,
) {
    let Some(node) = field_node(config, "custom") else {
        return;
    };
    let ChangieValue::Sequence(items) = &node.value else {
        report_wrong_shape(config, "custom", "sequence", &node.value, diagnostics);
        return;
    };
    let mut seen = std::collections::BTreeSet::new();
    for item in items {
        let ChangieValue::Mapping(mapping) = &item.value else {
            diagnostics.push(ChangieDiagnostic {
                rule: ChangieRule::ConfigInvalidConstraint,
                result_class: ChangieResultClass::Finding,
                repo_path: config.source.repo_path().to_string(),
                field_path: Some(ChangieFieldPath(vec!["custom".into()])),
                range: Some(item.range),
                related_config_ranges: Vec::new(),
                expected_actual: Some(ChangieExpectedActual {
                    expected: "choice mapping with `key` and `type`".into(),
                    actual: shape(&item.value).into(),
                }),
                message: "each custom choice must be a mapping with `key` and `type`".into(),
                actions: vec![ChangieAction::OpenRelatedConfigValue],
            });
            continue;
        };
        let Some(key_node) = mapping.first("key") else {
            diagnostics.push(ChangieDiagnostic {
                rule: ChangieRule::ConfigInvalidConstraint,
                result_class: ChangieResultClass::Finding,
                repo_path: config.source.repo_path().to_string(),
                field_path: Some(ChangieFieldPath(vec!["custom".into()])),
                range: Some(item.range),
                related_config_ranges: Vec::new(),
                expected_actual: None,
                message: "custom choice is missing its `key`".into(),
                actions: vec![ChangieAction::InsertMissingField],
            });
            continue;
        };
        let ChangieValue::String(key) = &key_node.value else {
            diagnostics.push(ChangieDiagnostic {
                rule: ChangieRule::ConfigInvalidConstraint,
                result_class: ChangieResultClass::Finding,
                repo_path: config.source.repo_path().to_string(),
                field_path: Some(ChangieFieldPath(vec!["custom".into()])),
                range: Some(key_node.range),
                related_config_ranges: Vec::new(),
                expected_actual: Some(ChangieExpectedActual {
                    expected: "key string".into(),
                    actual: shape(&key_node.value).into(),
                }),
                message: "custom choice `key` must be a non-empty string".into(),
                actions: vec![ChangieAction::OpenRelatedConfigValue],
            });
            continue;
        };
        if !seen.insert(key.clone()) {
            diagnostics.push(ChangieDiagnostic {
                rule: ChangieRule::ConfigInvalidConstraint,
                result_class: ChangieResultClass::Finding,
                repo_path: config.source.repo_path().to_string(),
                field_path: Some(ChangieFieldPath(vec!["custom".into()])),
                range: Some(key_node.range),
                related_config_ranges: Vec::new(),
                expected_actual: Some(ChangieExpectedActual {
                    expected: "unique choice keys".into(),
                    actual: key.clone(),
                }),
                message: format!("duplicate custom choice key `{key}`"),
                actions: vec![ChangieAction::OpenRelatedConfigValue],
            });
        }
        let Some(type_node) = mapping.first("type") else {
            diagnostics.push(ChangieDiagnostic {
                rule: ChangieRule::ConfigInvalidConstraint,
                result_class: ChangieResultClass::Finding,
                repo_path: config.source.repo_path().to_string(),
                field_path: Some(ChangieFieldPath(vec!["custom".into(), key.clone()])),
                range: Some(key_node.range),
                related_config_ranges: Vec::new(),
                expected_actual: None,
                message: format!("custom choice `{key}` is missing its `type`"),
                actions: vec![ChangieAction::InsertMissingField],
            });
            continue;
        };
        let ChangieValue::String(choice_type) = &type_node.value else {
            diagnostics.push(ChangieDiagnostic {
                rule: ChangieRule::ConfigInvalidConstraint,
                result_class: ChangieResultClass::Finding,
                repo_path: config.source.repo_path().to_string(),
                field_path: Some(ChangieFieldPath(vec!["custom".into(), key.clone()])),
                range: Some(type_node.range),
                related_config_ranges: Vec::new(),
                expected_actual: Some(ChangieExpectedActual {
                    expected: "type string".into(),
                    actual: shape(&type_node.value).into(),
                }),
                message: format!("custom choice `{key}` type must be a string"),
                actions: vec![ChangieAction::OpenRelatedConfigValue],
            });
            continue;
        };
        match choice_type.as_str() {
            "string" | "block" => {}
            "int" => {}
            "enum" => {
                let enum_node = mapping.first("enum");
                match enum_node {
                    Some(ChangieNode {
                        value: ChangieValue::Sequence(options),
                        range,
                    }) => {
                        if options.is_empty() {
                            diagnostics.push(ChangieDiagnostic {
                                rule: ChangieRule::ConfigInvalidConstraint,
                                result_class: ChangieResultClass::Finding,
                                repo_path: config.source.repo_path().to_string(),
                                field_path: Some(ChangieFieldPath(vec![
                                    "custom".into(),
                                    key.clone(),
                                    "enum".into(),
                                ])),
                                range: Some(*range),
                                related_config_ranges: Vec::new(),
                                expected_actual: Some(ChangieExpectedActual {
                                    expected: "non-empty enum option list".into(),
                                    actual: "empty".into(),
                                }),
                                message: format!("enum choice `{key}` has no options"),
                                actions: vec![ChangieAction::ChooseConfiguredValue],
                            });
                        }
                        let mut option_seen = std::collections::BTreeSet::new();
                        for option in options {
                            if let ChangieValue::String(option_value) = &option.value
                                && !option_seen.insert(option_value.clone())
                            {
                                diagnostics.push(ChangieDiagnostic {
                                    rule: ChangieRule::ConfigInvalidConstraint,
                                    result_class: ChangieResultClass::Finding,
                                    repo_path: config.source.repo_path().to_string(),
                                    field_path: Some(ChangieFieldPath(vec![
                                        "custom".into(),
                                        key.clone(),
                                        "enum".into(),
                                    ])),
                                    range: Some(option.range),
                                    related_config_ranges: Vec::new(),
                                    expected_actual: Some(ChangieExpectedActual {
                                        expected: "unique enum options".into(),
                                        actual: option_value.clone(),
                                    }),
                                    message: format!(
                                        "duplicate enum option `{option_value}` in choice `{key}`"
                                    ),
                                    actions: vec![ChangieAction::OpenRelatedConfigValue],
                                });
                            }
                        }
                    }
                    Some(other) => diagnostics.push(ChangieDiagnostic {
                        rule: ChangieRule::ConfigInvalidConstraint,
                        result_class: ChangieResultClass::Finding,
                        repo_path: config.source.repo_path().to_string(),
                        field_path: Some(ChangieFieldPath(vec![
                            "custom".into(),
                            key.clone(),
                            "enum".into(),
                        ])),
                        range: Some(other.range),
                        related_config_ranges: Vec::new(),
                        expected_actual: Some(ChangieExpectedActual {
                            expected: "enum option sequence".into(),
                            actual: shape(&other.value).into(),
                        }),
                        message: format!("enum choice `{key}` must list options as a sequence"),
                        actions: vec![ChangieAction::OpenRelatedConfigValue],
                    }),
                    None => diagnostics.push(ChangieDiagnostic {
                        rule: ChangieRule::ConfigInvalidConstraint,
                        result_class: ChangieResultClass::Finding,
                        repo_path: config.source.repo_path().to_string(),
                        field_path: Some(ChangieFieldPath(vec!["custom".into(), key.clone()])),
                        range: Some(type_node.range),
                        related_config_ranges: Vec::new(),
                        expected_actual: None,
                        message: format!("enum choice `{key}` is missing its `enum` options"),
                        actions: vec![ChangieAction::InsertMissingField],
                    }),
                }
            }
            other => diagnostics.push(ChangieDiagnostic {
                rule: ChangieRule::ConfigUnsupportedSemantics,
                result_class: ChangieResultClass::Unsupported,
                repo_path: config.source.repo_path().to_string(),
                field_path: Some(ChangieFieldPath(vec!["custom".into(), key.clone()])),
                range: Some(type_node.range),
                related_config_ranges: Vec::new(),
                expected_actual: Some(ChangieExpectedActual {
                    expected: "string | block | int | enum".into(),
                    actual: other.to_string(),
                }),
                message: format!("custom choice `{key}` has unsupported type `{other}`"),
                actions: vec![ChangieAction::ShowStaticVersusRenderLimitation],
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Candidate entry discovery classification
// ---------------------------------------------------------------------------

/// Classify caller-supplied entries against
/// `<changesDir>/<unreleasedDir>/<direct child>.yaml`: exact `.yaml`
/// extension (case-sensitive per the modeled generation), direct
/// children only, and distinct handling for every non-file state.
fn classify_entries(
    config: &ChangieConfigDocument,
    entries: &[ChangieCandidateEntry],
    diagnostics: &mut Vec<ChangieDiagnostic>,
    discovered: &mut Vec<String>,
    not_discovered: &mut Vec<String>,
) {
    let changes_dir = string_field(config, "changesDir").unwrap_or_default();
    let unreleased_dir = string_field(config, "unreleasedDir").unwrap_or_default();
    let root = normalize_repo_relative(&format!("{changes_dir}/{unreleased_dir}"));
    for entry in entries {
        let normalized = normalize_repo_relative(&entry.repo_path);
        let in_root = normalized != root && normalized.starts_with(&format!("{root}/"));
        let remainder = normalized
            .strip_prefix(&format!("{root}/"))
            .unwrap_or_default();
        let direct_child = !remainder.contains('/');
        let exact_extension = remainder.ends_with(".yaml");
        let under_discovery_root = in_root && !remainder.is_empty();
        match (
            &entry.state,
            under_discovery_root,
            direct_child,
            exact_extension,
        ) {
            (ChangieEntryState::File, true, true, true) => discovered.push(entry.repo_path.clone()),
            (ChangieEntryState::File, true, true, false) => {
                not_discovered.push(entry.repo_path.clone());
                diagnostics.push(not_discovered_diagnostic(
                    entry,
                    format!(
                        "extension is `{}`, not the exact `.yaml` the discovery contract matches",
                        extension_of(remainder)
                    ),
                ));
            }
            (ChangieEntryState::File, true, false, _) => {
                not_discovered.push(entry.repo_path.clone());
                diagnostics.push(not_discovered_diagnostic(
                    entry,
                    "nested under the unreleased root; discovery matches direct children only"
                        .into(),
                ));
            }
            (ChangieEntryState::File, false, _, _) => {
                not_discovered.push(entry.repo_path.clone());
            }
            (state, _, _, _) => {
                not_discovered.push(entry.repo_path.clone());
                if under_discovery_root {
                    diagnostics.push(ChangieDiagnostic {
                        rule: ChangieRule::FragmentEntryUnsupported,
                        result_class: ChangieResultClass::Partial,
                        repo_path: entry.repo_path.clone(),
                        field_path: None,
                        range: None,
                        related_config_ranges: Vec::new(),
                        expected_actual: Some(ChangieExpectedActual {
                            expected: "regular file".into(),
                            actual: state.as_str().into(),
                        }),
                        message: format!(
                            "entry under the fragment root is a {}, not a regular fragment file",
                            state.as_str()
                        ),
                        actions: vec![ChangieAction::ShowStaticVersusRenderLimitation],
                    });
                }
            }
        }
        // A malformed fragment stays in the population report.
        if let Some(fragment) = entry.fragment.as_ref()
            && !fragment.diagnostics.is_empty()
        {
            for diagnostic in &fragment.diagnostics {
                diagnostics.push(ChangieDiagnostic {
                    rule: ChangieRule::FragmentMalformed,
                    result_class: ChangieResultClass::Malformed,
                    repo_path: entry.repo_path.clone(),
                    field_path: diagnostic.path.clone(),
                    range: diagnostic.range,
                    related_config_ranges: Vec::new(),
                    expected_actual: None,
                    message: diagnostic.message.clone(),
                    actions: vec![ChangieAction::InsertMissingField],
                });
            }
        }
    }
}

fn not_discovered_diagnostic(entry: &ChangieCandidateEntry, reason: String) -> ChangieDiagnostic {
    ChangieDiagnostic {
        rule: ChangieRule::FragmentPathNotDiscovered,
        result_class: ChangieResultClass::Finding,
        repo_path: entry.repo_path.clone(),
        field_path: None,
        range: None,
        related_config_ranges: Vec::new(),
        expected_actual: None,
        message: format!(
            "candidate `{}` is not an ordinary discovered fragment: {reason}",
            entry.repo_path
        ),
        actions: vec![ChangieAction::ShowStaticVersusRenderLimitation],
    }
}

fn string_field(config: &ChangieConfigDocument, key: &str) -> Option<String> {
    match field_node(config, key).map(|node| &node.value) {
        Some(ChangieValue::String(value)) => Some(value.clone()),
        _ => None,
    }
}

fn extension_of(name: &str) -> &str {
    match name.rfind('.') {
        Some(index) => &name[index..],
        None => "",
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
#[path = "changie_lint_tests.rs"]
mod tests;
