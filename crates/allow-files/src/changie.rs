//! Embeddable Changie 1.25 source sensor — parse slice (#3588, ruling #3587).
//!
//! `parse_config`/`parse_fragment` turn caller-supplied repository-relative
//! documents into a source-aware model: ordered mappings that retain
//! duplicate keys, per-node source ranges, presence/scalar distinctions
//! (missing, null, empty, typed), unknown and unsupported field records,
//! and parse diagnostics. A clean Rust result says **static contract
//! satisfied** — it never says `changie batch` ran, never claims
//! rendering, and never decides whether a change needs a release note.
//! The parser never discovers files, validates
//! config-derived relationships, executes templates, or starts a process.
//!
//! A clean parse says the document is syntactically representable under
//! the modeled YAML surface — nothing more. Validation (#3589) owns the
//! meaning; a pinned upstream binary owns rendering.

use std::fmt;

use yaml_rust2::scanner::TScalarStyle;

/// The named upstream compatibility generation this sensor models
/// (#3587). The static authoring/discovery contract is shared across
/// 1.25.x patches; behavior deltas between patches are render-oracle
/// concerns, not statically checkable authoring differences.
pub const CHANGIE_COMPATIBILITY_GENERATION: &str = "1.25";

/// Marker newtype for the modeled generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChangieCompatibilityGeneration;

impl ChangieCompatibilityGeneration {
    pub const fn current() -> Self {
        Self
    }

    pub const fn as_str(self) -> &'static str {
        CHANGIE_COMPATIBILITY_GENERATION
    }
}

/// Repository-relative, forward-slash normalized document path.
/// Absolute checkout paths never enter portable semantic values (#3588).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangieRepoPath(String);

impl ChangieRepoPath {
    pub fn from_repo_relative(path: impl Into<String>) -> Result<Self, String> {
        let raw = path.into();
        if raw.is_empty() {
            return Err("repository-relative path must be non-empty".into());
        }
        if raw.starts_with('/') || raw.starts_with('\\') || raw.contains(':') {
            return Err(format!(
                "path {raw:?} must be repository-relative (no absolute paths or drive letters)"
            ));
        }
        let mut parts = Vec::new();
        for segment in raw.replace('\\', "/").split('/') {
            match segment {
                "" | "." => {}
                ".." => {
                    if parts.is_empty() {
                        return Err(format!("path {raw:?} escapes the repository root"));
                    }
                    parts.pop();
                }
                other => parts.push(other.to_string()),
            }
        }
        if parts.is_empty() {
            return Err(format!("path {raw:?} has no effective segments"));
        }
        Ok(Self(parts.join("/")))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Non-crypto content identity for a source document: byte length plus a
/// 64-bit FNV-1a digest. This is an identity token for deduplication and
/// receipt correlation, deliberately not a security claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChangieContentIdentity {
    pub byte_len: u64,
    pub fnv1a64: u64,
}

impl ChangieContentIdentity {
    pub fn of(bytes: &[u8]) -> Self {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        Self {
            byte_len: bytes.len() as u64,
            fnv1a64: hash,
        }
    }
}

impl fmt::Display for ChangieContentIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "fnv1a64:{:016x}+{}", self.fnv1a64, self.byte_len)
    }
}

/// Line-ending classification retained alongside the raw bytes so
/// diagnostics can be stated against the author's actual file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangieLineEndingClass {
    Empty,
    Lf,
    Crlf,
    Mixed,
    NoFinalNewline,
}

fn classify_line_endings(bytes: &[u8]) -> ChangieLineEndingClass {
    if bytes.is_empty() {
        return ChangieLineEndingClass::Empty;
    }
    let mut lf = 0usize;
    let mut crlf = 0usize;
    let mut iter = bytes.iter().copied().peekable();
    while let Some(byte) = iter.next() {
        match byte {
            b'\r' if iter.peek() == Some(&b'\n') => {
                crlf += 1;
                iter.next();
            }
            b'\n' => lf += 1,
            _ => {}
        }
    }
    if lf == 0 && crlf == 0 {
        return ChangieLineEndingClass::NoFinalNewline;
    }
    if lf > 0 && crlf > 0 {
        return ChangieLineEndingClass::Mixed;
    }
    if crlf > 0 {
        return ChangieLineEndingClass::Crlf;
    }
    ChangieLineEndingClass::Lf
}

/// A half-open source range. Lines and columns are 1-based; `index` is
/// the 0-based byte offset, retained for callers that need exact spans.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChangieSourcePos {
    pub line: u32,
    pub column: u32,
    pub index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChangieSourceRange {
    pub start: ChangieSourcePos,
    pub end: ChangieSourcePos,
}

/// Dotted field path used by unknown/unsupported/diagnostic records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangieFieldPath(pub Vec<String>);

impl fmt::Display for ChangieFieldPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for segment in &self.0 {
            if !first {
                write!(f, ".")?;
            }
            first = false;
            write!(f, "{segment}")?;
        }
        Ok(())
    }
}

/// Presence-aware field reference: `Missing` and authored `Null` stay
/// distinct, and neither is `EmptyString` (#3588 presence law).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangieFieldPresence<T> {
    Missing,
    Present(T),
}

impl<T> ChangieFieldPresence<T> {
    pub fn as_ref(&self) -> ChangieFieldPresence<&T> {
        match self {
            ChangieFieldPresence::Missing => ChangieFieldPresence::Missing,
            ChangieFieldPresence::Present(value) => ChangieFieldPresence::Present(value),
        }
    }
}

/// Scalar and structural value shapes retained without coercion: strings
/// stay strings, integers stay integers, booleans stay booleans.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangieValue {
    Null,
    EmptyString,
    String(String),
    Integer(i64),
    Boolean(bool),
    Sequence(Vec<ChangieNode>),
    Mapping(ChangieMapping),
    /// An alias/anchor construct the static model declines to resolve.
    /// Always accompanied by an `UnsupportedConstruct` diagnostic.
    UnsupportedAlias,
}

/// One mapping entry; duplicates are retained in authored order so
/// last-writer-wins cannot silently collapse them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangieMappingEntry {
    pub key: String,
    pub key_range: ChangieSourceRange,
    pub value: ChangieNode,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChangieMapping {
    pub entries: Vec<ChangieMappingEntry>,
}

impl ChangieMapping {
    /// First entry with this key (authored order), for callers that want
    /// a representative value while still seeing duplicates in `entries`.
    pub fn first(&self, key: &str) -> Option<&ChangieNode> {
        self.entries
            .iter()
            .find(|entry| entry.key == key)
            .map(|entry| &entry.value)
    }

    pub fn count(&self, key: &str) -> usize {
        self.entries.iter().filter(|entry| entry.key == key).count()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangieNode {
    pub value: ChangieValue,
    pub range: ChangieSourceRange,
}

/// A structurally unrecognized field under a recognized container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangieUnknownField {
    pub path: ChangieFieldPath,
    pub range: ChangieSourceRange,
}

/// A field whose value used a construct the static model does not
/// evaluate (aliases/anchors).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangieUnsupportedField {
    pub path: ChangieFieldPath,
    pub range: ChangieSourceRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangieParseDiagnosticKind {
    /// The YAML surface could not be parsed at all.
    Malformed,
    /// Syntactically valid YAML the static model declines to evaluate.
    UnsupportedConstruct,
    /// The bytes are not valid UTF-8; no tree is produced.
    NonUtf8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangieParseDiagnostic {
    pub kind: ChangieParseDiagnosticKind,
    pub path: Option<ChangieFieldPath>,
    pub range: Option<ChangieSourceRange>,
    pub message: String,
}

/// Caller-supplied source document. The sensor never reads the
/// filesystem; the caller owns discovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangieSourceDocument {
    repo_path: ChangieRepoPath,
    bytes: Vec<u8>,
    /// Subject token the caller uses to correlate this document with its
    /// own scan subjects (opaque to the sensor).
    subject: Option<String>,
}

impl ChangieSourceDocument {
    pub fn from_bytes(
        repo_path: ChangieRepoPath,
        bytes: Vec<u8>,
        subject: Option<String>,
    ) -> Result<Self, String> {
        let _ = &repo_path;
        Ok(Self {
            repo_path,
            bytes,
            subject,
        })
    }

    pub fn repo_path(&self) -> &str {
        self.repo_path.as_str()
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn subject(&self) -> Option<&str> {
        self.subject.as_deref()
    }

    pub fn content_identity(&self) -> ChangieContentIdentity {
        ChangieContentIdentity::of(&self.bytes)
    }

    pub fn line_ending_class(&self) -> ChangieLineEndingClass {
        classify_line_endings(&self.bytes)
    }

    fn text(&self) -> Option<&str> {
        std::str::from_utf8(&self.bytes).ok()
    }
}

/// Parsed Changie configuration document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangieConfigDocument {
    pub generation: ChangieCompatibilityGeneration,
    pub source: ChangieSourceDocument,
    /// `None` when the document is malformed or not UTF-8.
    pub root: Option<ChangieNode>,
    pub unknown_fields: Vec<ChangieUnknownField>,
    pub unsupported_fields: Vec<ChangieUnsupportedField>,
    pub diagnostics: Vec<ChangieParseDiagnostic>,
}

/// Parsed Changie fragment document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangieFragmentDocument {
    pub generation: ChangieCompatibilityGeneration,
    pub source: ChangieSourceDocument,
    pub root: Option<ChangieNode>,
    pub unknown_fields: Vec<ChangieUnknownField>,
    pub unsupported_fields: Vec<ChangieUnsupportedField>,
    pub diagnostics: Vec<ChangieParseDiagnostic>,
}

/// The recognized top-level configuration field surface for the modeled
/// generation. Unlisted top-level keys are recorded as unknown fields;
/// recognition is structural, not validation.
pub const CHANGIE_CONFIG_FIELD_SURFACE: &[&str] = &[
    "changesDir",
    "unreleasedDir",
    "headerPath",
    "changelogPath",
    "versionExt",
    "versionHeaderPath",
    "versionFooterPath",
    "versionFileFormat",
    "fragmentFileFormat",
    "versionFormat",
    "componentFormat",
    "kindFormat",
    "changeFormat",
    "headerFormat",
    "footerFormat",
    "majorVersionKind",
    "minorVersionKind",
    "patchVersionKind",
    "newlines",
    "body",
    "signoffs",
    "executableEnv",
    "timeFormat",
    "fragmentTemplateDirs",
    "versionTemplateDirs",
    "post",
    "authors",
    "replacements",
    "projects",
    "projectsVersionFile",
    "components",
    "kinds",
    "custom",
    "choices",
];

/// The recognized fragment field surface. `custom`, `env`, and nested
/// choice maps contain configuration-dependent keys, so their nested
/// fields are not classified as unknown at parse time.
pub const CHANGIE_FRAGMENT_FIELD_SURFACE: &[&str] = &[
    "kind",
    "body",
    "time",
    "project",
    "component",
    "custom",
    "env",
    "author",
    "signoff",
];

pub fn parse_config(source: ChangieSourceDocument) -> ChangieConfigDocument {
    let parsed = parse_source(&source);
    ChangieConfigDocument {
        generation: ChangieCompatibilityGeneration::current(),
        unknown_fields: collect_unknown(&parsed, CHANGIE_CONFIG_FIELD_SURFACE),
        unsupported_fields: parsed.unsupported_fields,
        root: parsed.root,
        diagnostics: parsed.diagnostics,
        source,
    }
}

pub fn parse_fragment(source: ChangieSourceDocument) -> ChangieFragmentDocument {
    let parsed = parse_source(&source);
    ChangieFragmentDocument {
        generation: ChangieCompatibilityGeneration::current(),
        unknown_fields: collect_unknown(&parsed, CHANGIE_FRAGMENT_FIELD_SURFACE),
        unsupported_fields: parsed.unsupported_fields,
        root: parsed.root,
        diagnostics: parsed.diagnostics,
        source,
    }
}

/// Top-level field lookup honoring the presence law: authored `null` is
/// `Present(ChangieValue::Null)`, absence is `Missing`.
pub fn config_field<'a>(
    document: &'a ChangieConfigDocument,
    key: &str,
) -> ChangieFieldPresence<&'a ChangieNode> {
    mapping_field(document.root.as_ref(), key)
}

pub fn fragment_field<'a>(
    document: &'a ChangieFragmentDocument,
    key: &str,
) -> ChangieFieldPresence<&'a ChangieNode> {
    mapping_field(document.root.as_ref(), key)
}

fn mapping_field<'a>(
    root: Option<&'a ChangieNode>,
    key: &str,
) -> ChangieFieldPresence<&'a ChangieNode> {
    match root.and_then(|node| match &node.value {
        ChangieValue::Mapping(mapping) => mapping.first(key),
        _ => None,
    }) {
        Some(node) => ChangieFieldPresence::Present(node),
        None => ChangieFieldPresence::Missing,
    }
}

struct ParsedSource {
    root: Option<ChangieNode>,
    unsupported_fields: Vec<ChangieUnsupportedField>,
    diagnostics: Vec<ChangieParseDiagnostic>,
}

fn parse_source(source: &ChangieSourceDocument) -> ParsedSource {
    let mut diagnostics = Vec::new();
    let text = match source.text() {
        Some(text) => text,
        None => {
            diagnostics.push(ChangieParseDiagnostic {
                kind: ChangieParseDiagnosticKind::NonUtf8,
                path: None,
                range: None,
                message: "document is not valid UTF-8; no tree was produced".into(),
            });
            return ParsedSource {
                root: None,
                unsupported_fields: Vec::new(),
                diagnostics,
            };
        }
    };
    let mut events = Vec::new();
    let mut receiver = CaptureReceiver {
        events: &mut events,
    };
    let mut parser = yaml_rust2::parser::Parser::new(text.chars());
    if let Err(error) = parser.load(&mut receiver, true) {
        diagnostics.push(ChangieParseDiagnostic {
            kind: ChangieParseDiagnosticKind::Malformed,
            path: None,
            range: None,
            message: format!("YAML syntax error: {error}"),
        });
        return ParsedSource {
            root: None,
            unsupported_fields: Vec::new(),
            diagnostics,
        };
    }
    let mut walker = Walker {
        events: events.as_slice(),
        cursor: 0,
        path: Vec::new(),
        unsupported_fields: Vec::new(),
        diagnostics: Vec::new(),
    };
    let root = walker.build_node();
    diagnostics.extend(walker.diagnostics);
    ParsedSource {
        root,
        unsupported_fields: walker.unsupported_fields,
        diagnostics,
    }
}

struct CaptureReceiver<'a> {
    events: &'a mut Vec<(yaml_rust2::parser::Event, yaml_rust2::scanner::Marker)>,
}

impl yaml_rust2::parser::MarkedEventReceiver for CaptureReceiver<'_> {
    fn on_event(&mut self, event: yaml_rust2::parser::Event, marker: yaml_rust2::scanner::Marker) {
        self.events.push((event, marker));
    }
}

fn pos(marker: &yaml_rust2::scanner::Marker) -> ChangieSourcePos {
    // yaml-rust2 markers are 1-indexed on both axes.
    ChangieSourcePos {
        line: marker.line() as u32,
        column: marker.col() as u32,
        index: marker.index(),
    }
}

fn range_for(
    start: &yaml_rust2::scanner::Marker,
    end: &yaml_rust2::scanner::Marker,
) -> ChangieSourceRange {
    ChangieSourceRange {
        start: pos(start),
        end: pos(end),
    }
}

struct Walker<'a> {
    events: &'a [(yaml_rust2::parser::Event, yaml_rust2::scanner::Marker)],
    cursor: usize,
    path: Vec<String>,
    unsupported_fields: Vec<ChangieUnsupportedField>,
    diagnostics: Vec<ChangieParseDiagnostic>,
}

impl<'a> Walker<'a> {
    fn build_node(&mut self) -> Option<ChangieNode> {
        let (event, start) = {
            let (event, marker) = self.events.get(self.cursor)?;
            (event.clone(), *marker)
        };
        match event {
            yaml_rust2::parser::Event::Scalar(value, style, _, _) => {
                self.cursor += 1;
                let end = self
                    .events
                    .get(self.cursor)
                    .map(|(_, marker)| *marker)
                    .unwrap_or(start);
                let node_value = if style != TScalarStyle::Plain {
                    // Quoted/literal/folded scalars are always authored
                    // strings: "42" is a String, never an Integer.
                    if value.is_empty() {
                        ChangieValue::EmptyString
                    } else {
                        ChangieValue::String(value)
                    }
                } else if value.is_empty() {
                    // A plain empty scalar (`key:` with no value) resolves
                    // to null in the YAML 1.2 core schema.
                    ChangieValue::Null
                } else {
                    scalar_value(&value)
                };
                Some(ChangieNode {
                    value: node_value,
                    range: range_for(&start, &end),
                })
            }
            yaml_rust2::parser::Event::SequenceStart(..) => {
                self.cursor += 1;
                let mut items = Vec::new();
                loop {
                    match self.events.get(self.cursor) {
                        Some((yaml_rust2::parser::Event::SequenceEnd, end)) => {
                            let end = *end;
                            self.cursor += 1;
                            return Some(ChangieNode {
                                value: ChangieValue::Sequence(items),
                                range: range_for(&start, &end),
                            });
                        }
                        Some(_) => {
                            let before = self.cursor;
                            match self.build_node() {
                                Some(item) => items.push(item),
                                None if self.cursor == before => return None,
                                None => {}
                            }
                        }
                        None => return None,
                    }
                }
            }
            yaml_rust2::parser::Event::MappingStart(..) => {
                self.cursor += 1;
                let mut mapping = ChangieMapping::default();
                loop {
                    let (event, event_marker) = {
                        let (event, marker) = self.events.get(self.cursor)?;
                        (event.clone(), *marker)
                    };
                    if matches!(event, yaml_rust2::parser::Event::MappingEnd) {
                        let end = event_marker;
                        self.cursor += 1;
                        return Some(ChangieNode {
                            value: ChangieValue::Mapping(mapping),
                            range: range_for(&start, &end),
                        });
                    }
                    // Key: the next scalar (non-scalar keys are malformed
                    // for this surface and recorded instead of guessed).
                    let key = self.build_node()?;
                    let key_text = match &key.value {
                        ChangieValue::String(text) => text.clone(),
                        ChangieValue::EmptyString => String::new(),
                        other => {
                            self.diagnostics.push(ChangieParseDiagnostic {
                                kind: ChangieParseDiagnosticKind::Malformed,
                                path: Some(ChangieFieldPath(self.path.clone())),
                                range: Some(key.range),
                                message: format!(
                                    "mapping key is not a plain scalar ({})",
                                    shape_name(other)
                                ),
                            });
                            continue;
                        }
                    };
                    let key_range = key.range;
                    let value_before = self.cursor;
                    let value = match self.build_node() {
                        Some(node) => node,
                        None if self.cursor > value_before => continue,
                        None => return None,
                    };
                    mapping.entries.push(ChangieMappingEntry {
                        key: key_text.clone(),
                        key_range,
                        value,
                    });
                }
            }
            yaml_rust2::parser::Event::Alias(_) => {
                self.cursor += 1;
                let end = self
                    .events
                    .get(self.cursor)
                    .map(|(_, marker)| *marker)
                    .unwrap_or(start);
                self.unsupported_fields.push(ChangieUnsupportedField {
                    path: ChangieFieldPath(self.path.clone()),
                    range: range_for(&start, &end),
                });
                self.diagnostics.push(ChangieParseDiagnostic {
                    kind: ChangieParseDiagnosticKind::UnsupportedConstruct,
                    path: Some(ChangieFieldPath(self.path.clone())),
                    range: Some(range_for(&start, &end)),
                    message: "YAML alias/anchor: the static sensor does not evaluate aliases"
                        .into(),
                });
                Some(ChangieNode {
                    value: ChangieValue::UnsupportedAlias,
                    range: range_for(&start, &end),
                })
            }
            yaml_rust2::parser::Event::Nothing
            | yaml_rust2::parser::Event::DocumentStart
            | yaml_rust2::parser::Event::DocumentEnd
            | yaml_rust2::parser::Event::StreamStart
            | yaml_rust2::parser::Event::StreamEnd => {
                self.cursor += 1;
                self.build_node()
            }
            // Container ends are consumed by their own branches; reaching
            // one here means a caller walked past its container — return
            // nothing rather than silently skipping into a sibling.
            yaml_rust2::parser::Event::MappingEnd | yaml_rust2::parser::Event::SequenceEnd => None,
        }
    }
}

fn scalar_value(value: &str) -> ChangieValue {
    match value {
        "~" | "null" | "Null" | "NULL" => ChangieValue::Null,
        "true" | "True" | "TRUE" => ChangieValue::Boolean(true),
        "false" | "False" | "FALSE" => ChangieValue::Boolean(false),
        other => match other.parse::<i64>() {
            Ok(integer) => ChangieValue::Integer(integer),
            Err(_) => ChangieValue::String(other.to_string()),
        },
    }
}

fn shape_name(value: &ChangieValue) -> &'static str {
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

fn collect_unknown(parsed: &ParsedSource, surface: &[&str]) -> Vec<ChangieUnknownField> {
    let Some(root) = parsed.root.as_ref() else {
        return Vec::new();
    };
    let ChangieValue::Mapping(mapping) = &root.value else {
        return Vec::new();
    };
    let mut unknown = Vec::new();
    for entry in &mapping.entries {
        if !surface.contains(&entry.key.as_str()) {
            unknown.push(ChangieUnknownField {
                path: ChangieFieldPath(vec![entry.key.clone()]),
                range: entry.key_range,
            });
        }
    }
    unknown
}

#[cfg(test)]
#[path = "changie_tests.rs"]
mod tests;
