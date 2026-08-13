//! Scoped-text rule schema and literal/phrase/token matcher MVP (#2714 / #2684).
//!
//! Implements the smallest useful engine slice: a versioned rule schema with
//! deterministic literal, phrase, and token/word-boundary matching. No regex,
//! inline exceptions, structured scope, or RIPR parity (those are #2715,
//! #2718, #2719, #2720).

use serde::Deserialize;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MatcherKind {
    Literal,
    Phrase,
    Token,
    Regex,
}

impl MatcherKind {
    fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "literal" => Ok(MatcherKind::Literal),
            "phrase" => Ok(MatcherKind::Phrase),
            "token" => Ok(MatcherKind::Token),
            "regex" => Ok(MatcherKind::Regex),
            other => Err(format!("unknown matcher kind `{other}`")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Severity {
    Info,
    Warning,
    Error,
}

impl Severity {
    fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "info" => Ok(Severity::Info),
            "warning" => Ok(Severity::Warning),
            "error" => Ok(Severity::Error),
            other => Err(format!("unknown severity `{other}`")),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScopedTextRuleSchema {
    schema_id: String,
    schema_version: u32,
    controlling_issue: u32,
    generated_by: String,
    #[serde(rename = "rule")]
    rules: Vec<RuleEntry>,
    #[serde(default, rename = "exception")]
    exceptions: Vec<ExceptionEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuleEntry {
    id: String,
    kind: String,
    term: String,
    #[serde(default = "default_true")]
    case_sensitive: bool,
    severity: String,
    help: String,
    owner: String,
    rationale: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExceptionEntry {
    rule_id: String,
    reason: String,
    owner: String,
    #[serde(default)]
    max_occurrences: Option<u32>,
    #[serde(default)]
    expires: Option<String>,
    #[serde(default)]
    review_after: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone)]
struct ScopedTextRule {
    id: String,
    kind: MatcherKind,
    term: String,
    case_sensitive: bool,
    severity: Severity,
    help: String,
    owner: String,
    rationale: String,
}

/// A stable machine-readable finding emitted by the matcher.
#[derive(Debug, Clone, PartialEq, Eq)]
struct TextFinding {
    rule_id: String,
    line: usize,
    column: usize,
    matched_text: String,
    severity: String,
}

/// A central text exception that suppresses findings for a bounded reason (#2716).
#[derive(Debug, Clone)]
struct TextException {
    rule_id: String,
    reason: String,
    owner: String,
    max_occurrences: Option<u32>,
    expires: Option<String>,
    review_after: Option<String>,
}

/// Diagnostic kind for exception validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExceptionDiagnosticKind {
    Expired,
    ReviewDue,
    OrphanRule,
    OccurrenceOverflow,
}

/// Validate exceptions against findings and an as-of date. Returns
/// diagnostics for expired, review-due, orphan, and overflow exceptions.
fn validate_exceptions(
    exceptions: &[TextException],
    findings: &[TextFinding],
    known_rule_ids: &[&str],
    as_of: &str,
) -> Vec<(ExceptionDiagnosticKind, String)> {
    let mut diags = Vec::new();
    for exc in exceptions {
        // Orphan: exception references a rule that doesn't exist.
        if !known_rule_ids.contains(&exc.rule_id.as_str()) {
            diags.push((
                ExceptionDiagnosticKind::OrphanRule,
                format!("exception references unknown rule `{}`", exc.rule_id),
            ));
            continue;
        }
        // Expired: hard expiry date has passed.
        if let Some(ref expires) = exc.expires
            && expires.as_str() <= as_of
        {
            diags.push((
                ExceptionDiagnosticKind::Expired,
                format!("exception for `{}` expired on {expires}", exc.rule_id),
            ));
        }
        // Review due: soft review date has passed (warning only).
        if let Some(ref review) = exc.review_after
            && review.as_str() <= as_of
        {
            diags.push((
                ExceptionDiagnosticKind::ReviewDue,
                format!("exception for `{}` due for review on {review}", exc.rule_id),
            ));
        }
        // Occurrence overflow: more findings than max_occurrences allows.
        if let Some(max) = exc.max_occurrences {
            let count = findings.iter().filter(|f| f.rule_id == exc.rule_id).count() as u32;
            if count > max {
                diags.push((
                    ExceptionDiagnosticKind::OccurrenceOverflow,
                    format!(
                        "exception for `{}` allows {max} occurrences but found {count}",
                        exc.rule_id
                    ),
                ));
            }
        }
    }
    diags
}

fn load_rules() -> Result<Vec<ScopedTextRule>, String> {
    let root = workspace_root();
    let path = root.join("policy/scoped-text-rules.toml");
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("read policy/scoped-text-rules.toml: {e}"))?;
    let raw: ScopedTextRuleSchema =
        toml::from_str(&text).map_err(|e| format!("parse scoped-text-rules.toml: {e}"))?;
    if raw.schema_id != "cargo-allow.scoped-text-rules.v1" {
        return Err(format!(
            "unexpected schema_id `{}`; expected cargo-allow.scoped-text-rules.v1",
            raw.schema_id
        ));
    }
    if raw.schema_version != 1 {
        return Err(format!(
            "unexpected schema_version {}; expected 1",
            raw.schema_version
        ));
    }
    if raw.controlling_issue != 2684 {
        return Err(format!(
            "unexpected controlling_issue {}; expected 2684",
            raw.controlling_issue
        ));
    }
    if !raw.generated_by.contains("#2714") {
        return Err(format!(
            "generated_by must reference #2714; found `{}`",
            raw.generated_by
        ));
    }
    raw.rules
        .into_iter()
        .map(|r| {
            Ok(ScopedTextRule {
                kind: MatcherKind::parse(&r.kind)?,
                severity: Severity::parse(&r.severity)?,
                id: r.id,
                term: r.term,
                case_sensitive: r.case_sensitive,
                help: r.help,
                owner: r.owner,
                rationale: r.rationale,
            })
        })
        .collect()
}

fn load_exceptions() -> Result<Vec<TextException>, String> {
    let root = workspace_root();
    let path = root.join("policy/scoped-text-rules.toml");
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("read policy/scoped-text-rules.toml: {e}"))?;
    let raw: ScopedTextRuleSchema =
        toml::from_str(&text).map_err(|e| format!("parse scoped-text-rules.toml: {e}"))?;
    raw.exceptions
        .into_iter()
        .map(|e| {
            Ok(TextException {
                rule_id: e.rule_id,
                reason: e.reason,
                owner: e.owner,
                max_occurrences: e.max_occurrences,
                expires: e.expires,
                review_after: e.review_after,
            })
        })
        .collect()
}

/// Check whether a character is a word boundary character (non-alphanumeric
/// and non-underscore). Used for phrase and token matching.
fn is_word_boundary(ch: char) -> bool {
    !ch.is_alphanumeric() && ch != '_'
}

/// Match a single rule against a line of text. Returns all match positions
/// (column = byte offset from line start, 0-based).
fn match_rule(rule: &ScopedTextRule, line: &str) -> Vec<(usize, String)> {
    let (haystack, needle) = if rule.case_sensitive {
        (line.to_string(), rule.term.clone())
    } else {
        (line.to_lowercase(), rule.term.to_lowercase())
    };
    let haystack_bytes = haystack.as_bytes();

    match rule.kind {
        MatcherKind::Literal => {
            // Substring match: find all occurrences.
            let mut results = Vec::new();
            let mut start = 0;
            while let Some(relative) = haystack.get(start..).and_then(|s| s.find(&needle)) {
                let pos = start + relative;
                let matched = line.get(pos..pos + needle.len()).unwrap_or("").to_string();
                results.push((pos, matched));
                start = pos + needle.len();
            }
            results
        }
        MatcherKind::Phrase => {
            // Whole-phrase match: word boundary on both sides.
            let mut results = Vec::new();
            let mut start = 0;
            while let Some(relative) = haystack.get(start..).and_then(|s| s.find(&needle)) {
                let pos = start + relative;
                let end = pos + needle.len();
                let before_ok = pos == 0
                    || haystack_bytes
                        .get(pos.wrapping_sub(1))
                        .is_some_and(|&b| is_word_boundary(b as char));
                let after_ok = end >= haystack.len()
                    || haystack_bytes
                        .get(end)
                        .is_some_and(|&b| is_word_boundary(b as char));
                if before_ok && after_ok {
                    let matched = line.get(pos..end).unwrap_or("").to_string();
                    results.push((pos, matched));
                }
                start = pos + needle.len();
            }
            results
        }
        MatcherKind::Token => {
            // Single-token match: word boundary on both sides, no spaces in term.
            if rule.term.contains(' ') {
                return Vec::new();
            }
            let mut results = Vec::new();
            let mut start = 0;
            while let Some(relative) = haystack.get(start..).and_then(|s| s.find(&needle)) {
                let pos = start + relative;
                let end = pos + needle.len();
                let before_ok = pos == 0
                    || haystack_bytes
                        .get(pos.wrapping_sub(1))
                        .is_some_and(|&b| is_word_boundary(b as char));
                let after_ok = end >= haystack.len()
                    || haystack_bytes
                        .get(end)
                        .is_some_and(|&b| is_word_boundary(b as char));
                if before_ok && after_ok {
                    let matched = line.get(pos..end).unwrap_or("").to_string();
                    results.push((pos, matched));
                }
                start = pos + needle.len();
            }
            results
        }
        MatcherKind::Regex => {
            // Regex match: compile the pattern and find all matches.
            let Ok(re) = regex::Regex::new(&rule.term) else {
                return Vec::new();
            };
            let mut results = Vec::new();
            for mat in re.find_iter(line) {
                results.push((mat.start(), mat.as_str().to_string()));
            }
            results
        }
    }
}

/// Scan a multi-line text input with all rules. Returns findings sorted by
/// (line, column, rule_id) for deterministic ordering.
fn scan_text(rules: &[ScopedTextRule], text: &str) -> Vec<TextFinding> {
    let mut findings = Vec::new();
    for (line_num, line) in text.lines().enumerate() {
        for rule in rules {
            for (col, matched) in match_rule(rule, line) {
                findings.push(TextFinding {
                    rule_id: rule.id.clone(),
                    line: line_num + 1,
                    column: col,
                    matched_text: matched,
                    severity: match rule.severity {
                        Severity::Info => "info",
                        Severity::Warning => "warning",
                        Severity::Error => "error",
                    }
                    .to_string(),
                });
            }
        }
    }
    findings.sort_by(|a, b| (a.line, a.column, &a.rule_id).cmp(&(b.line, b.column, &b.rule_id)));
    findings
}

#[test]
fn manifest_loads_and_has_required_fields() -> Result<(), String> {
    let rules = load_rules()?;
    if rules.is_empty() {
        return Err("scoped-text-rules.toml has no rules".into());
    }
    for r in &rules {
        for (label, value) in [
            ("id", r.id.as_str()),
            ("term", r.term.as_str()),
            ("help", r.help.as_str()),
            ("owner", r.owner.as_str()),
            ("rationale", r.rationale.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(format!("rule `{}` has empty field `{}`", r.id, label));
            }
        }
    }
    Ok(())
}

#[test]
fn literal_matcher_finds_substring_occurrences() -> Result<(), String> {
    let rule = ScopedTextRule {
        id: "test-literal".to_string(),
        kind: MatcherKind::Literal,
        term: "TODO".to_string(),
        case_sensitive: true,
        severity: Severity::Info,
        help: String::new(),
        owner: String::new(),
        rationale: String::new(),
    };
    // Exact match
    let results = match_rule(&rule, "  TODO: fix this");
    if results.len() != 1 || results.first().map(|r| r.0) != Some(2) {
        return Err(format!("expected one match at col 2, got {results:?}"));
    }
    // Case-sensitive: lowercase should not match
    let results = match_rule(&rule, "  todo: fix this");
    if !results.is_empty() {
        return Err(format!(
            "case-sensitive literal should not match lowercase: {results:?}"
        ));
    }
    // Multiple occurrences on one line
    let results = match_rule(&rule, "TODO and TODO again");
    if results.len() != 2 {
        return Err(format!("expected 2 matches, got {results:?}"));
    }
    // Substring coincidence: "TODOIST" should match (literal = substring)
    let results = match_rule(&rule, "TODOIST");
    if results.is_empty() {
        return Err("literal matcher should find TODO inside TODOIST (substring semantics)".into());
    }
    Ok(())
}

#[test]
fn phrase_matcher_respects_word_boundaries() -> Result<(), String> {
    let rule = ScopedTextRule {
        id: "test-phrase".to_string(),
        kind: MatcherKind::Phrase,
        term: "deprecated API".to_string(),
        case_sensitive: false,
        severity: Severity::Warning,
        help: String::new(),
        owner: String::new(),
        rationale: String::new(),
    };
    // Exact phrase
    let results = match_rule(&rule, "This uses a deprecated API for backwards compat");
    if results.len() != 1 {
        return Err(format!("expected 1 phrase match, got {results:?}"));
    }
    // Case-insensitive
    let results = match_rule(&rule, "This uses a DEPRECATED API");
    if results.is_empty() {
        return Err("phrase matcher should be case-insensitive when configured".into());
    }
    // No match inside a larger word (e.g. "undeprecated API")
    let results = match_rule(&rule, "undeprecated API is fine");
    if !results.is_empty() {
        return Err(format!(
            "phrase matcher should not match inside larger word: {results:?}"
        ));
    }
    Ok(())
}

#[test]
fn token_matcher_respects_word_boundaries() -> Result<(), String> {
    let rule = ScopedTextRule {
        id: "test-token".to_string(),
        kind: MatcherKind::Token,
        term: "unsafe".to_string(),
        case_sensitive: true,
        severity: Severity::Info,
        help: String::new(),
        owner: String::new(),
        rationale: String::new(),
    };
    // Exact token
    let results = match_rule(&rule, "This is unsafe code");
    if results.len() != 1 {
        return Err(format!("expected 1 token match, got {results:?}"));
    }
    // Token preceded by non-word char
    let results = match_rule(&rule, "uses (unsafe) blocks");
    if results.is_empty() {
        return Err("token matcher should match within parentheses".into());
    }
    // No match inside a larger word: "unsafeLY" should NOT match
    let results = match_rule(&rule, "unsafely written");
    if !results.is_empty() {
        return Err(format!(
            "token matcher should not match inside larger word: {results:?}"
        ));
    }
    // No match as substring: "xunsafe" should NOT match
    let results = match_rule(&rule, "xunsafe");
    if !results.is_empty() {
        return Err(format!(
            "token matcher should not match as substring: {results:?}"
        ));
    }
    Ok(())
}

#[test]
fn scan_text_produces_deterministic_ordering() -> Result<(), String> {
    let rules = vec![
        ScopedTextRule {
            id: "rule-b".to_string(),
            kind: MatcherKind::Literal,
            term: "foo".to_string(),
            case_sensitive: true,
            severity: Severity::Info,
            help: String::new(),
            owner: String::new(),
            rationale: String::new(),
        },
        ScopedTextRule {
            id: "rule-a".to_string(),
            kind: MatcherKind::Literal,
            term: "bar".to_string(),
            case_sensitive: true,
            severity: Severity::Warning,
            help: String::new(),
            owner: String::new(),
            rationale: String::new(),
        },
    ];
    let text = "bar foo\nfoo bar";
    let findings = scan_text(&rules, text);
    // Deterministic sort: (line, column, rule_id)
    // Line 1: "bar foo" -> bar at col 0 (rule-a), foo at col 4 (rule-b)
    // Line 2: "foo bar" -> foo at col 0 (rule-b), bar at col 4 (rule-a)
    if findings.len() != 4 {
        return Err(format!("expected 4 findings, got {}", findings.len()));
    }
    // Verify ordering using adjacent pairs
    for window in findings.windows(2) {
        let Some(prev) = window.first() else { continue };
        let Some(curr) = window.get(1) else { continue };
        let prev_key = (&prev.line, &prev.column, &prev.rule_id);
        let curr_key = (&curr.line, &curr.column, &curr.rule_id);
        if prev_key > curr_key {
            return Err(format!(
                "findings not sorted: {:?} > {:?}",
                prev_key, curr_key
            ));
        }
    }
    Ok(())
}

#[test]
fn unknown_matcher_kind_fails() -> Result<(), String> {
    let result = MatcherKind::parse("glob");
    if result.is_ok() {
        return Err("unknown matcher kind 'glob' should fail".into());
    }
    let result = Severity::parse("critical");
    if result.is_ok() {
        return Err("unknown severity 'critical' should fail".into());
    }
    Ok(())
}

#[test]
fn case_insensitive_unicode_normalization_policy() -> Result<(), String> {
    // Case-insensitive matching should work with basic Unicode.
    let rule = ScopedTextRule {
        id: "test-unicode".to_string(),
        kind: MatcherKind::Token,
        term: "café".to_string(),
        case_sensitive: false,
        severity: Severity::Info,
        help: String::new(),
        owner: String::new(),
        rationale: String::new(),
    };
    // The MVP uses to_lowercase() which handles basic Unicode case folding.
    // Full Unicode normalization (NFC/NFD) is explicitly out of scope.
    let results = match_rule(&rule, "drinking café au lait");
    if results.is_empty() {
        return Err("case-insensitive token matcher should find 'café'".into());
    }
    Ok(())
}

#[test]
fn regex_matcher_finds_pattern_matches() -> Result<(), String> {
    let rule = ScopedTextRule {
        id: "test-regex-version".to_string(),
        kind: MatcherKind::Regex,
        term: r"\d+\.\d+\.\d+".to_string(),
        case_sensitive: true,
        severity: Severity::Info,
        help: String::new(),
        owner: String::new(),
        rationale: String::new(),
    };
    // Match version-like strings
    let results = match_rule(&rule, "version 1.2.3 released");
    if results.len() != 1 {
        return Err(format!("expected 1 regex match, got {results:?}"));
    }
    if results.first().map(|r| &r.1) != Some(&"1.2.3".to_string()) {
        return Err(format!("expected '1.2.3', got {results:?}"));
    }
    // Multiple matches on one line
    let results = match_rule(&rule, "upgrade from 0.1.0 to 0.2.0");
    if results.len() != 2 {
        return Err(format!("expected 2 regex matches, got {results:?}"));
    }
    Ok(())
}

#[test]
fn regex_matcher_handles_invalid_pattern_gracefully() -> Result<(), String> {
    let rule = ScopedTextRule {
        id: "test-regex-invalid".to_string(),
        kind: MatcherKind::Regex,
        term: "[invalid".to_string(), // unclosed character class
        case_sensitive: true,
        severity: Severity::Info,
        help: String::new(),
        owner: String::new(),
        rationale: String::new(),
    };
    // Invalid regex should return empty results, not panic.
    let results = match_rule(&rule, "some text");
    if !results.is_empty() {
        return Err(format!(
            "invalid regex should return empty, got {results:?}"
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// #2716: Central text exception tests
// ---------------------------------------------------------------------------

#[test]
fn exceptions_load_from_manifest() -> Result<(), String> {
    let excs = load_exceptions()?;
    if excs.is_empty() {
        return Err("scoped-text-rules.toml should have at least one exception".into());
    }
    for e in &excs {
        if e.rule_id.trim().is_empty() || e.reason.trim().is_empty() || e.owner.trim().is_empty() {
            return Err(format!(
                "exception for rule `{}` has empty required field",
                e.rule_id
            ));
        }
    }
    Ok(())
}

#[test]
fn exception_detects_expired_hard_expiry() -> Result<(), String> {
    let excs = vec![TextException {
        rule_id: "test-rule".to_string(),
        reason: "test".to_string(),
        owner: "test".to_string(),
        max_occurrences: None,
        expires: Some("2026-01-01".to_string()),
        review_after: None,
    }];
    let diags = validate_exceptions(&excs, &[], &["test-rule"], "2026-08-13");
    let expired = diags
        .iter()
        .any(|(k, _)| *k == ExceptionDiagnosticKind::Expired);
    if !expired {
        return Err("expired exception should be detected".into());
    }
    Ok(())
}

#[test]
fn exception_detects_review_due() -> Result<(), String> {
    let excs = vec![TextException {
        rule_id: "test-rule".to_string(),
        reason: "test".to_string(),
        owner: "test".to_string(),
        max_occurrences: None,
        expires: None,
        review_after: Some("2026-01-01".to_string()),
    }];
    let diags = validate_exceptions(&excs, &[], &["test-rule"], "2026-08-13");
    let due = diags
        .iter()
        .any(|(k, _)| *k == ExceptionDiagnosticKind::ReviewDue);
    if !due {
        return Err("review-due exception should be detected".into());
    }
    Ok(())
}

#[test]
fn exception_detects_orphan_rule_reference() -> Result<(), String> {
    let excs = vec![TextException {
        rule_id: "nonexistent-rule".to_string(),
        reason: "test".to_string(),
        owner: "test".to_string(),
        max_occurrences: None,
        expires: None,
        review_after: None,
    }];
    let diags = validate_exceptions(&excs, &[], &["real-rule"], "2026-08-13");
    let orphan = diags
        .iter()
        .any(|(k, _)| *k == ExceptionDiagnosticKind::OrphanRule);
    if !orphan {
        return Err("orphan rule reference should be detected".into());
    }
    Ok(())
}

#[test]
fn exception_detects_occurrence_overflow() -> Result<(), String> {
    let excs = vec![TextException {
        rule_id: "test-rule".to_string(),
        reason: "test".to_string(),
        owner: "test".to_string(),
        max_occurrences: Some(2),
        expires: None,
        review_after: None,
    }];
    let findings = vec![
        TextFinding {
            rule_id: "test-rule".to_string(),
            line: 1,
            column: 0,
            matched_text: "match".to_string(),
            severity: "info".to_string(),
        },
        TextFinding {
            rule_id: "test-rule".to_string(),
            line: 2,
            column: 0,
            matched_text: "match".to_string(),
            severity: "info".to_string(),
        },
        TextFinding {
            rule_id: "test-rule".to_string(),
            line: 3,
            column: 0,
            matched_text: "match".to_string(),
            severity: "info".to_string(),
        },
    ];
    let diags = validate_exceptions(&excs, &findings, &["test-rule"], "2026-08-13");
    let overflow = diags
        .iter()
        .any(|(k, _)| *k == ExceptionDiagnosticKind::OccurrenceOverflow);
    if !overflow {
        return Err("occurrence overflow (3 > 2) should be detected".into());
    }
    Ok(())
}

#[test]
fn exception_within_bounds_produces_no_diagnostics() -> Result<(), String> {
    let excs = vec![TextException {
        rule_id: "test-rule".to_string(),
        reason: "legitimate use".to_string(),
        owner: "test".to_string(),
        max_occurrences: Some(5),
        expires: Some("2027-01-01".to_string()),
        review_after: Some("2027-01-01".to_string()),
    }];
    let findings = vec![TextFinding {
        rule_id: "test-rule".to_string(),
        line: 1,
        column: 0,
        matched_text: "match".to_string(),
        severity: "info".to_string(),
    }];
    let diags = validate_exceptions(&excs, &findings, &["test-rule"], "2026-08-13");
    if !diags.is_empty() {
        return Err(format!(
            "valid exception should produce no diagnostics, got {diags:?}"
        ));
    }
    Ok(())
}
