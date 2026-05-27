use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

pub const STRUCTURAL_IDENTITY_SCHEMA_ID: &str = "cargo-allow.structural-identity.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoAllowError {
    message: String,
}

impl CargoAllowError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for CargoAllowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CargoAllowError {}

pub type CargoAllowResult<T> = Result<T, CargoAllowError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SimpleDate {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

impl SimpleDate {
    pub fn parse(input: &str) -> Option<Self> {
        let mut parts = input.trim().split('-');
        let year = parts.next()?.parse().ok()?;
        let month = parts.next()?.parse().ok()?;
        let day = parts.next()?.parse().ok()?;
        if parts.next().is_some() || !valid_ymd(year, month, day) {
            return None;
        }
        Some(Self { year, month, day })
    }

    pub fn days_until(self, other: Self) -> i64 {
        other.days_since_unix_epoch() - self.days_since_unix_epoch()
    }

    pub fn add_days(self, days: i64) -> Self {
        Self::from_days_since_unix_epoch(self.days_since_unix_epoch() + days)
    }

    fn days_since_unix_epoch(self) -> i64 {
        // Howard Hinnant's civil date algorithm. This keeps lifecycle validation
        // deterministic without adding a date dependency to the core crate.
        let mut year = i64::from(self.year);
        let month = i64::from(self.month);
        let day = i64::from(self.day);
        if month <= 2 {
            year -= 1;
        }
        let era = if year >= 0 { year } else { year - 399 } / 400;
        let year_of_era = year - era * 400;
        let month_prime = month + if month > 2 { -3 } else { 9 };
        let day_of_year = (153 * month_prime + 2) / 5 + day - 1;
        let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
        era * 146_097 + day_of_era - 719_468
    }

    fn from_days_since_unix_epoch(days: i64) -> Self {
        // Inverse of the civil date algorithm above.
        let z = days + 719_468;
        let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
        let day_of_era = z - era * 146_097;
        let year_of_era =
            (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
        let mut year = year_of_era + era * 400;
        let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let month_prime = (5 * day_of_year + 2) / 153;
        let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
        let month = month_prime + if month_prime < 10 { 3 } else { -9 };
        if month <= 2 {
            year += 1;
        }
        Self {
            year: year as i32,
            month: month as u32,
            day: day as u32,
        }
    }

    pub fn today_utc_approx() -> Self {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        Self::from_days_since_unix_epoch((seconds / 86_400) as i64)
    }
}

fn valid_ymd(year: i32, month: u32, day: u32) -> bool {
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap_year(year) => 29,
        2 => 28,
        _ => return false,
    };
    day > 0 && day <= max_day
}

fn leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

impl fmt::Display for SimpleDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FindingKind {
    Panic,
    Unsafe,
    LintException,
    NonRustFile,
    GeneratedCode,
    PolicyException,
}

impl FindingKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Panic => "panic",
            Self::Unsafe => "unsafe",
            Self::LintException => "lint_exception",
            Self::NonRustFile => "non_rust_file",
            Self::GeneratedCode => "generated_code",
            Self::PolicyException => "policy_exception",
        }
    }
}

impl fmt::Display for FindingKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for FindingKind {
    type Err = CargoAllowError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "panic" | "panic_family" | "panic-family" | "indexing" => Ok(Self::Panic),
            "unsafe" => Ok(Self::Unsafe),
            "lint_exception" | "lint-exception" | "clippy" | "allow_attribute"
            | "allow-attribute" | "expect_attribute" | "expect-attribute" => {
                Ok(Self::LintException)
            }
            "non_rust_file" | "non-rust-file" | "non_rust" | "non-rust" | "file" => {
                Ok(Self::NonRustFile)
            }
            "generated_code" | "generated-code" | "generated" => Ok(Self::GeneratedCode),
            "policy_exception" | "policy-exception" | "policy" => Ok(Self::PolicyException),
            other => Err(CargoAllowError::new(format!(
                "unsupported finding kind `{other}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralIdentity {
    pub language: String,
    pub crate_name: Option<String>,
    pub module: Option<String>,
    pub container: Option<String>,
    pub ast_kind: String,
    pub symbol: Option<String>,
    pub callee: Option<String>,
    pub macro_name: Option<String>,
    pub lint: Option<String>,
    pub receiver_fingerprint: Option<String>,
    pub target_fingerprint: Option<String>,
    pub normalized_snippet_hash: Option<String>,
    pub line_hint: Option<u32>,
    pub column_hint: Option<u32>,
}

impl StructuralIdentity {
    pub fn schema_id() -> &'static str {
        STRUCTURAL_IDENTITY_SCHEMA_ID
    }

    pub fn new(language: impl Into<String>, ast_kind: impl Into<String>) -> Self {
        Self {
            language: language.into(),
            crate_name: None,
            module: None,
            container: None,
            ast_kind: ast_kind.into(),
            symbol: None,
            callee: None,
            macro_name: None,
            lint: None,
            receiver_fingerprint: None,
            target_fingerprint: None,
            normalized_snippet_hash: None,
            line_hint: None,
            column_hint: None,
        }
    }

    pub fn stable_key(&self) -> String {
        stable_identity_key_from_parts(self.stable_key_parts())
    }

    pub fn stable_key_parts(&self) -> Vec<(&'static str, String)> {
        vec![
            ("language", self.language.clone()),
            ("crate_name", self.crate_name.clone().unwrap_or_default()),
            ("module", self.module.clone().unwrap_or_default()),
            ("container", self.container.clone().unwrap_or_default()),
            ("ast_kind", self.ast_kind.clone()),
            ("symbol", self.symbol.clone().unwrap_or_default()),
            ("callee", self.callee.clone().unwrap_or_default()),
            ("macro_name", self.macro_name.clone().unwrap_or_default()),
            ("lint", self.lint.clone().unwrap_or_default()),
            (
                "receiver_fingerprint",
                self.receiver_fingerprint.clone().unwrap_or_default(),
            ),
            (
                "target_fingerprint",
                self.target_fingerprint.clone().unwrap_or_default(),
            ),
            (
                "normalized_snippet_hash",
                self.normalized_snippet_hash.clone().unwrap_or_default(),
            ),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub kind: FindingKind,
    pub family: Option<String>,
    pub path: PathBuf,
    pub span: Option<Span>,
    pub identity: StructuralIdentity,
    pub message: String,
}

pub fn finding_identity_key(finding: &Finding) -> String {
    let mut parts = vec![
        ("kind", finding.kind.as_str().to_string()),
        ("family", finding.family.clone().unwrap_or_default()),
        ("path", normalize_path(&finding.path)),
    ];
    parts.extend(finding.identity.stable_key_parts());
    stable_identity_key_from_parts(parts)
}

fn stable_identity_key_from_parts(parts: Vec<(&'static str, String)>) -> String {
    parts
        .into_iter()
        .map(|(name, value)| format!("{name}:{}:{value}", value.len()))
        .collect::<Vec<_>>()
        .join("|")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastSeen {
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Selector {
    pub ast_kind: Option<String>,
    pub container: Option<String>,
    pub callee: Option<String>,
    pub macro_name: Option<String>,
    pub lint: Option<String>,
    pub symbol: Option<String>,
    pub receiver_fingerprint: Option<String>,
    pub target_fingerprint: Option<String>,
    pub normalized_snippet_hash: Option<String>,
    pub line_hint: Option<u32>,
    pub glob: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lifecycle {
    pub created: Option<String>,
    pub review_after: Option<String>,
    pub expires: Option<String>,
}

impl Lifecycle {
    pub fn empty() -> Self {
        Self {
            created: None,
            review_after: None,
            expires: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllowEntry {
    pub id: String,
    pub kind: FindingKind,
    pub family: Option<String>,
    pub path: Option<PathBuf>,
    pub glob: Option<String>,
    pub owner: String,
    pub classification: String,
    pub reason: String,
    pub evidence: Vec<String>,
    pub links: Vec<String>,
    pub occurrence_limit: Option<u32>,
    pub lifecycle: Lifecycle,
    pub selector: Selector,
    pub last_seen: Option<LastSeen>,
}

impl AllowEntry {
    pub fn path_or_glob(&self) -> String {
        if let Some(path) = &self.path {
            normalize_path(path)
        } else if let Some(glob) = &self.glob {
            glob.clone()
        } else if let Some(glob) = &self.selector.glob {
            glob.clone()
        } else {
            String::new()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requirements {
    pub owner_required: bool,
    pub reason_required: bool,
    pub classification_required: bool,
    pub expires_or_review_after_required: bool,
    pub allow_bare_allow_attributes: bool,
    pub lint_policy_id_required: bool,
    pub stale_entries_fail: bool,
    pub unsafe_evidence_required: bool,
    pub unsafe_safety_comment_required: bool,
}

impl Default for Requirements {
    fn default() -> Self {
        Self {
            owner_required: true,
            reason_required: true,
            classification_required: true,
            expires_or_review_after_required: true,
            allow_bare_allow_attributes: false,
            lint_policy_id_required: false,
            stale_entries_fail: false,
            unsafe_evidence_required: true,
            unsafe_safety_comment_required: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceConfig {
    pub root: String,
    pub inventory: String,
    pub ignored: Vec<String>,
    pub generated: Vec<String>,
    pub default_mode: String,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            root: ".".to_string(),
            inventory: "git-tracked".to_string(),
            ignored: vec![".git/**".to_string(), "target/**".to_string()],
            generated: vec!["target/**".to_string(), "vendor/**".to_string()],
            default_mode: "no-new".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllowConfig {
    pub schema_version: String,
    pub policy: String,
    pub owner: Option<String>,
    pub status: Option<String>,
    pub workspace: WorkspaceConfig,
    pub requirements: Requirements,
    pub allow: Vec<AllowEntry>,
}

impl AllowConfig {
    pub fn empty() -> Self {
        Self {
            schema_version: "0.1".to_string(),
            policy: "cargo-allow".to_string(),
            owner: None,
            status: Some("active".to_string()),
            workspace: WorkspaceConfig::default(),
            requirements: Requirements::default(),
            allow: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MatchStatus {
    Matched,
    New,
    Stale,
    Expired,
    ReviewDue,
    Ambiguous,
    InvalidSelector,
    MissingRequiredField,
    EvidenceMissing,
    BaselineDebt,
}

impl MatchStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Matched => "matched",
            Self::New => "new",
            Self::Stale => "stale",
            Self::Expired => "expired",
            Self::ReviewDue => "review_due",
            Self::Ambiguous => "ambiguous",
            Self::InvalidSelector => "invalid_selector",
            Self::MissingRequiredField => "missing_required_field",
            Self::EvidenceMissing => "evidence_missing",
            Self::BaselineDebt => "baseline_debt",
        }
    }

    pub fn is_failure_in_strict(self) -> bool {
        !matches!(self, Self::Matched | Self::ReviewDue)
    }

    pub fn is_failure_in_no_new(self) -> bool {
        matches!(
            self,
            Self::New
                | Self::Expired
                | Self::Ambiguous
                | Self::InvalidSelector
                | Self::MissingRequiredField
                | Self::EvidenceMissing
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchOutcome {
    pub status: MatchStatus,
    pub allow_id: Option<String>,
    pub finding_index: Option<usize>,
    pub message: String,
    pub score: u32,
}

pub fn normalize_path(path: impl AsRef<Path>) -> String {
    let text = path.as_ref().to_string_lossy().replace('\\', "/");
    let absolute = text.starts_with('/');
    let mut parts = Vec::new();
    for part in text.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.last().is_some_and(|part| *part != "..") {
                    parts.pop();
                } else if !absolute {
                    parts.push(part);
                }
            }
            other => parts.push(other),
        }
    }
    let normalized = parts.join("/");
    if absolute {
        format!("/{normalized}")
    } else {
        normalized
    }
}

pub fn normalize_snippet(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn stable_hash_hex(input: &str) -> String {
    // FNV-1a 64-bit. Not cryptographic; stable across platforms and enough for drift hints.
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}

pub fn maybe_line_distance_score(hint: Option<u32>, actual: Option<u32>) -> u32 {
    match (hint, actual) {
        (Some(h), Some(a)) => {
            let diff = h.abs_diff(a);
            if diff == 0 {
                15
            } else if diff <= 3 {
                12
            } else if diff <= 10 {
                8
            } else if diff <= 25 {
                3
            } else {
                0
            }
        }
        _ => 0,
    }
}

pub fn glob_matches(pattern: &str, path: &Path) -> bool {
    let path = normalize_path(path);
    glob_matches_str(pattern, &path)
}

pub fn glob_matches_str(pattern: &str, path: &str) -> bool {
    let p = pattern.replace('\\', "/");
    glob_match_tokens(&split_glob(&p), &split_glob(path))
}

fn split_glob(s: &str) -> Vec<&str> {
    s.split('/').filter(|part| !part.is_empty()).collect()
}

fn glob_match_tokens(pattern: &[&str], path: &[&str]) -> bool {
    if pattern.is_empty() {
        return path.is_empty();
    }
    if pattern[0] == "**" {
        if glob_match_tokens(&pattern[1..], path) {
            return true;
        }
        return !path.is_empty() && glob_match_tokens(pattern, &path[1..]);
    }
    if path.is_empty() {
        return false;
    }
    segment_matches(pattern[0], path[0]) && glob_match_tokens(&pattern[1..], &path[1..])
}

fn segment_matches(pattern: &str, text: &str) -> bool {
    segment_match_bytes(pattern.as_bytes(), text.as_bytes())
}

fn segment_match_bytes(pattern: &[u8], text: &[u8]) -> bool {
    if pattern.is_empty() {
        return text.is_empty();
    }
    match pattern[0] {
        b'*' => {
            segment_match_bytes(&pattern[1..], text)
                || (!text.is_empty() && segment_match_bytes(pattern, &text[1..]))
        }
        b'?' => !text.is_empty() && segment_match_bytes(&pattern[1..], &text[1..]),
        byte => {
            !text.is_empty() && byte == text[0] && segment_match_bytes(&pattern[1..], &text[1..])
        }
    }
}

pub fn json_escape(input: &str) -> String {
    let mut out = String::new();
    for ch in input.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_supports_double_star() {
        assert!(glob_matches_str("crates/**/*.rs", "crates/foo/src/lib.rs"));
        assert!(glob_matches_str(
            ".github/workflows/*.yml",
            ".github/workflows/ci.yml"
        ));
        assert!(!glob_matches_str(
            "scripts/*.sh",
            "scripts/release/build.sh"
        ));
    }

    #[test]
    fn finding_kind_accepts_hyphenated_cli_aliases() {
        assert_eq!(
            FindingKind::from_str("non-rust"),
            Ok(FindingKind::NonRustFile)
        );
        assert_eq!(
            FindingKind::from_str("lint-exception"),
            Ok(FindingKind::LintException)
        );
        assert_eq!(
            FindingKind::from_str("generated-code"),
            Ok(FindingKind::GeneratedCode)
        );
    }

    #[test]
    fn normalize_path_preserves_leading_parent_segments() {
        assert_eq!(normalize_path("../src/lib.rs"), "../src/lib.rs");
        assert_eq!(normalize_path("../../src/../README.md"), "../../README.md");
        assert_eq!(normalize_path("src/../README.md"), "README.md");
        assert_eq!(normalize_path(r"..\src\lib.rs"), "../src/lib.rs");
    }

    #[test]
    fn normalize_path_preserves_absolute_unix_root() {
        assert_eq!(normalize_path("/a/../b"), "/b");
        assert_eq!(normalize_path("/../b"), "/b");
        assert_eq!(normalize_path("/"), "/");
        assert_eq!(normalize_path("/a//./b/"), "/a/b");
    }

    #[test]
    fn hash_is_stable() {
        assert_eq!(stable_hash_hex("abc"), stable_hash_hex("abc"));
        assert_ne!(stable_hash_hex("abc"), stable_hash_hex("abd"));
    }

    #[test]
    fn structural_identity_key_excludes_line_and_column_hints() {
        let mut first = StructuralIdentity::new("rust", "method_call");
        first.module = Some("parser::span".to_string());
        first.container = Some("parse_span".to_string());
        first.callee = Some("unwrap".to_string());
        first.normalized_snippet_hash = Some("fnv1a64:1234".to_string());
        first.line_hint = Some(12);
        first.column_hint = Some(8);

        let mut moved = first.clone();
        moved.line_hint = Some(99);
        moved.column_hint = Some(42);

        assert_eq!(first.stable_key(), moved.stable_key());

        moved.container = Some("parse_other_span".to_string());

        assert_ne!(first.stable_key(), moved.stable_key());
    }

    #[test]
    fn structural_identity_has_v1_schema_id() {
        assert_eq!(
            StructuralIdentity::schema_id(),
            "cargo-allow.structural-identity.v1"
        );
    }

    #[test]
    fn structural_identity_key_uses_length_prefixed_parts() {
        let mut first = StructuralIdentity::new("rust", "method_call");
        first.container = Some("load".to_string());
        first.callee = Some("unwrap".to_string());
        first.normalized_snippet_hash = Some("fnv1a64:abcd".to_string());

        let key = first.stable_key();

        assert!(key.contains("language:4:rust"));
        assert!(key.contains("container:4:load"));
        assert!(key.contains("callee:6:unwrap"));
        assert!(key.contains("normalized_snippet_hash:12:fnv1a64:abcd"));
    }

    #[test]
    fn structural_identity_v1_fields_affect_stable_key_except_hints() {
        let mut base = StructuralIdentity::new("rust", "method_call");
        base.crate_name = Some("parser".to_string());
        base.module = Some("parser::span".to_string());
        base.container = Some("slice_checked_text_range".to_string());
        base.symbol = Some("source[range]".to_string());
        base.callee = Some("expect".to_string());
        base.macro_name = Some("panic".to_string());
        base.lint = Some("clippy::indexing_slicing".to_string());
        base.receiver_fingerprint = Some("source".to_string());
        base.target_fingerprint = Some("range".to_string());
        base.normalized_snippet_hash = Some("fnv1a64:abcd".to_string());
        base.line_hint = Some(10);
        base.column_hint = Some(4);

        let mut moved = base.clone();
        moved.line_hint = Some(100);
        moved.column_hint = Some(40);
        assert_eq!(base.stable_key(), moved.stable_key());

        let cases: &[fn(&mut StructuralIdentity)] = &[
            |id| id.language = "file".to_string(),
            |id| id.crate_name = Some("runtime".to_string()),
            |id| id.module = Some("runtime::ffi".to_string()),
            |id| id.container = Some("read_buffer".to_string()),
            |id| id.ast_kind = "macro_call".to_string(),
            |id| id.symbol = Some("buffer[index]".to_string()),
            |id| id.callee = Some("unwrap".to_string()),
            |id| id.macro_name = Some("todo".to_string()),
            |id| id.lint = Some("dead_code".to_string()),
            |id| id.receiver_fingerprint = Some("buffer".to_string()),
            |id| id.target_fingerprint = Some("index".to_string()),
            |id| id.normalized_snippet_hash = Some("fnv1a64:dcba".to_string()),
        ];

        for mutate in cases {
            let mut changed = base.clone();
            mutate(&mut changed);
            assert_ne!(base.stable_key(), changed.stable_key());
        }
    }

    #[test]
    fn finding_identity_key_excludes_span_but_includes_structural_scope() {
        let mut identity = StructuralIdentity::new("rust", "method_call");
        identity.container = Some("load".to_string());
        identity.callee = Some("unwrap".to_string());
        identity.normalized_snippet_hash = Some("fnv1a64:abcd".to_string());

        let mut first = Finding {
            kind: FindingKind::Panic,
            family: Some("unwrap".to_string()),
            path: PathBuf::from("src/lib.rs"),
            span: Some(Span {
                line: 10,
                column: 4,
            }),
            identity,
            message: "test finding".to_string(),
        };
        let mut moved = first.clone();
        moved.span = Some(Span {
            line: 200,
            column: 40,
        });

        assert_eq!(finding_identity_key(&first), finding_identity_key(&moved));

        moved.path = PathBuf::from("src/other.rs");
        assert_ne!(finding_identity_key(&first), finding_identity_key(&moved));

        moved.path = first.path.clone();
        first.family = Some("expect".to_string());
        assert_ne!(finding_identity_key(&first), finding_identity_key(&moved));
    }

    #[test]
    fn simple_date_rejects_invalid_calendar_dates() {
        assert!(SimpleDate::parse("2026-02-29").is_none());
        assert!(SimpleDate::parse("2024-02-29").is_some());
        assert!(SimpleDate::parse("2026-04-31").is_none());
        assert!(SimpleDate::parse("2026-13-01").is_none());
    }

    #[test]
    fn simple_date_counts_days_between_dates() {
        let start = SimpleDate::parse("2026-05-26")
            .unwrap_or_else(|| std::panic::panic_any("valid start date"));
        let end = SimpleDate::parse("2026-08-01")
            .unwrap_or_else(|| std::panic::panic_any("valid end date"));

        assert_eq!(start.days_until(end), 67);
    }

    #[test]
    fn simple_date_adds_days_across_months() {
        let start = SimpleDate::parse("2026-05-26")
            .unwrap_or_else(|| std::panic::panic_any("valid start date"));

        assert_eq!(start.add_days(67).to_string(), "2026-08-01");
    }

    #[test]
    fn simple_date_converts_unix_epoch_days() {
        assert_eq!(
            SimpleDate::from_days_since_unix_epoch(0).to_string(),
            "1970-01-01"
        );
        assert_eq!(
            SimpleDate::from_days_since_unix_epoch(
                SimpleDate::parse("2026-05-27")
                    .unwrap_or_else(|| std::panic::panic_any("valid date"))
                    .days_since_unix_epoch()
            )
            .to_string(),
            "2026-05-27"
        );
    }

    #[test]
    fn today_utc_approx_uses_system_clock_day() {
        let before = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs() / 86_400)
            .unwrap_or(0);
        let today = SimpleDate::today_utc_approx();
        let after = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs() / 86_400)
            .unwrap_or(0);

        let today_days = today.days_since_unix_epoch() as u64;
        assert!(
            (before..=after).contains(&today_days),
            "today_utc_approx should use the current UTC day"
        );
    }
}
