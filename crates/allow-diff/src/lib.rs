use allow_core::{
    AllowConfig, AllowEntry, CargoAllowError, CargoAllowResult, Finding, SimpleDate, glob_matches,
    normalize_path,
};
use allow_policy::parse_policy;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn changed_files(
    root: impl AsRef<Path>,
    base: &str,
    head: Option<&str>,
) -> CargoAllowResult<Vec<PathBuf>> {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(root.as_ref())
        .arg("diff")
        .arg("--name-only")
        .arg(base);
    if let Some(head) = head {
        cmd.arg(head);
    }
    let output = cmd
        .output()
        .map_err(|e| CargoAllowError::new(format!("failed to run git diff: {e}")))?;
    if !output.status.success() {
        return Err(CargoAllowError::new("git diff --name-only failed"));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(PathBuf::from)
        .collect())
}

pub fn git_tracked_files_at_revision(
    root: impl AsRef<Path>,
    revision: &str,
) -> CargoAllowResult<Vec<PathBuf>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root.as_ref())
        .arg("ls-tree")
        .arg("-r")
        .arg("--name-only")
        .arg(revision)
        .output()
        .map_err(|e| CargoAllowError::new(format!("failed to run git ls-tree: {e}")))?;
    if !output.status.success() {
        return Err(CargoAllowError::new(format!(
            "git ls-tree failed for {revision}"
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(PathBuf::from)
        .collect())
}

pub fn findings_at_revision(
    root: impl AsRef<Path>,
    revision: &str,
    cfg: &AllowConfig,
) -> CargoAllowResult<Vec<Finding>> {
    let root = root.as_ref();
    let mut files = git_tracked_files_at_revision(root, revision)?;
    files.retain(|path| !is_ignored(path, &cfg.workspace.ignored));
    let mut findings = Vec::new();
    for rel in files
        .iter()
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("rs"))
    {
        if let Some(text) = read_file_at_revision(root, revision, rel)? {
            findings.extend(allow_rust::scan_rust_source(rel, &text));
        }
    }
    findings.extend(allow_files::scan_files_with_options(
        &files,
        &allow_files::FileScanOptions {
            generated: cfg.workspace.generated.clone(),
        },
    ));
    Ok(findings)
}

fn is_ignored(path: &Path, patterns: &[String]) -> bool {
    let normalized = normalize_path(path);
    patterns.iter().any(|pattern| {
        glob_matches(pattern, path)
            || pattern
                .strip_suffix("/**")
                .map(|prefix| normalized == prefix || normalized.starts_with(&format!("{prefix}/")))
                .unwrap_or(false)
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindingPostureChange {
    pub kind: FindingPostureKind,
    pub key: String,
    pub finding_kind: String,
    pub family: Option<String>,
    pub path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindingPostureKind {
    New,
    Removed,
}

impl FindingPostureKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Removed => "removed",
        }
    }
}

pub fn finding_posture_changes(base: &[Finding], head: &[Finding]) -> Vec<FindingPostureChange> {
    let base_by_key = findings_by_key(base);
    let head_by_key = findings_by_key(head);
    let mut changes = Vec::new();
    for (key, counted) in &head_by_key {
        let base_count = base_by_key
            .get(key)
            .map(|counted| counted.count)
            .unwrap_or(0);
        if counted.count > base_count {
            for _ in 0..(counted.count - base_count) {
                changes.push(finding_posture_change(
                    FindingPostureKind::New,
                    key,
                    counted.finding,
                ));
            }
        }
    }
    for (key, counted) in &base_by_key {
        let head_count = head_by_key
            .get(key)
            .map(|counted| counted.count)
            .unwrap_or(0);
        if counted.count > head_count {
            for _ in 0..(counted.count - head_count) {
                changes.push(finding_posture_change(
                    FindingPostureKind::Removed,
                    key,
                    counted.finding,
                ));
            }
        }
    }
    changes
}

#[derive(Debug, Clone, Copy)]
struct CountedFinding<'a> {
    finding: &'a Finding,
    count: usize,
}

fn findings_by_key(findings: &[Finding]) -> BTreeMap<String, CountedFinding<'_>> {
    let mut by_key = BTreeMap::new();
    for finding in findings {
        by_key
            .entry(finding_identity_key(finding))
            .and_modify(|counted: &mut CountedFinding<'_>| counted.count += 1)
            .or_insert(CountedFinding { finding, count: 1 });
    }
    by_key
}

fn finding_posture_change(
    kind: FindingPostureKind,
    key: &str,
    finding: &Finding,
) -> FindingPostureChange {
    FindingPostureChange {
        kind,
        key: key.to_string(),
        finding_kind: finding.kind.as_str().to_string(),
        family: finding.family.clone(),
        path: normalize_path(&finding.path),
    }
}

pub fn finding_identity_key(finding: &Finding) -> String {
    [
        finding.kind.as_str().to_string(),
        finding.family.clone().unwrap_or_default(),
        normalize_path(&finding.path),
        finding.identity.ast_kind.clone(),
        finding.identity.module.clone().unwrap_or_default(),
        finding.identity.container.clone().unwrap_or_default(),
        finding.identity.callee.clone().unwrap_or_default(),
        finding.identity.macro_name.clone().unwrap_or_default(),
        finding.identity.lint.clone().unwrap_or_default(),
        finding.identity.symbol.clone().unwrap_or_default(),
        finding
            .identity
            .receiver_fingerprint
            .clone()
            .unwrap_or_default(),
        finding
            .identity
            .target_fingerprint
            .clone()
            .unwrap_or_default(),
        finding
            .identity
            .normalized_snippet_hash
            .clone()
            .unwrap_or_default(),
    ]
    .join("|")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyChange {
    pub allow_id: String,
    pub kind: PolicyChangeKind,
    pub severity: PolicyChangeSeverity,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyChangeKind {
    AddedAllow,
    BaselineDebtAdded,
    ScopeBroadened,
    SelectorPrecisionDecreased,
    ExpiryExtended,
    ReviewAfterExtended,
    EvidenceRemoved,
    OwnerRemoved,
    ReasonRemoved,
    ClassificationRemoved,
    OccurrenceLimitLoosened,
}

impl PolicyChangeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AddedAllow => "added_allow",
            Self::BaselineDebtAdded => "baseline_debt_added",
            Self::ScopeBroadened => "scope_broadened",
            Self::SelectorPrecisionDecreased => "selector_precision_decreased",
            Self::ExpiryExtended => "expiry_extended",
            Self::ReviewAfterExtended => "review_after_extended",
            Self::EvidenceRemoved => "evidence_removed",
            Self::OwnerRemoved => "owner_removed",
            Self::ReasonRemoved => "reason_removed",
            Self::ClassificationRemoved => "classification_removed",
            Self::OccurrenceLimitLoosened => "occurrence_limit_loosened",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyChangeSeverity {
    Review,
    Fail,
}

impl PolicyChangeSeverity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Review => "review",
            Self::Fail => "fail",
        }
    }

    pub fn fails(self) -> bool {
        matches!(self, Self::Fail)
    }
}

pub fn policy_changes_from_git(
    root: impl AsRef<Path>,
    base: &str,
    policy_path: impl AsRef<Path>,
    head_cfg: &AllowConfig,
) -> CargoAllowResult<Vec<PolicyChange>> {
    let Some(base_cfg) = policy_config_at_revision(root, base, policy_path)? else {
        return Ok(Vec::new());
    };
    Ok(policy_changes(&base_cfg, head_cfg))
}

pub fn policy_config_at_revision(
    root: impl AsRef<Path>,
    revision: &str,
    policy_path: impl AsRef<Path>,
) -> CargoAllowResult<Option<AllowConfig>> {
    let Some(text) = read_file_at_revision(root, revision, policy_path)? else {
        return Ok(None);
    };
    parse_policy(&text).map(Some)
}

pub fn read_file_at_revision(
    root: impl AsRef<Path>,
    revision: &str,
    path: impl AsRef<Path>,
) -> CargoAllowResult<Option<String>> {
    let spec = format!(
        "{}:{}",
        revision,
        path.as_ref().to_string_lossy().replace('\\', "/")
    );
    let output = Command::new("git")
        .arg("-C")
        .arg(root.as_ref())
        .arg("show")
        .arg(&spec)
        .output()
        .map_err(|e| CargoAllowError::new(format!("failed to run git show: {e}")))?;
    if output.status.success() {
        return Ok(Some(String::from_utf8_lossy(&output.stdout).to_string()));
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("exists on disk, but not in")
        || stderr.contains("Path")
        || stderr.contains("does not exist")
    {
        return Ok(None);
    }
    Err(CargoAllowError::new(format!(
        "failed to read {} from {revision}",
        path.as_ref().display()
    )))
}

pub fn policy_changes(base: &AllowConfig, head: &AllowConfig) -> Vec<PolicyChange> {
    let base_by_id = base
        .allow
        .iter()
        .map(|entry| (entry.id.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let mut changes = Vec::new();
    for head_entry in &head.allow {
        let Some(base_entry) = base_by_id.get(head_entry.id.as_str()).copied() else {
            changes.push(added_allow_change(head_entry));
            continue;
        };
        changes.extend(entry_policy_changes(base_entry, head_entry));
    }
    changes
}

fn added_allow_change(entry: &AllowEntry) -> PolicyChange {
    let baseline = entry.classification == "baseline_debt";
    PolicyChange {
        allow_id: entry.id.clone(),
        kind: if baseline {
            PolicyChangeKind::BaselineDebtAdded
        } else {
            PolicyChangeKind::AddedAllow
        },
        severity: if baseline {
            PolicyChangeSeverity::Fail
        } else {
            PolicyChangeSeverity::Review
        },
        message: if baseline {
            format!("{} added generated baseline debt", entry.id)
        } else {
            format!("{} added a new allow entry", entry.id)
        },
    }
}

fn entry_policy_changes(base: &AllowEntry, head: &AllowEntry) -> Vec<PolicyChange> {
    let mut changes = Vec::new();
    if scope_broadened(base, head) {
        changes.push(change(
            head,
            PolicyChangeKind::ScopeBroadened,
            PolicyChangeSeverity::Fail,
            "scope broadened",
        ));
    }
    let base_precision = selector_precision_score(base);
    let head_precision = selector_precision_score(head);
    if head_precision < base_precision {
        changes.push(PolicyChange {
            allow_id: head.id.clone(),
            kind: PolicyChangeKind::SelectorPrecisionDecreased,
            severity: PolicyChangeSeverity::Fail,
            message: format!(
                "{} selector precision decreased: {} -> {}",
                head.id, base_precision, head_precision
            ),
        });
    }
    if date_extended(
        base.lifecycle.expires.as_deref(),
        head.lifecycle.expires.as_deref(),
    ) {
        changes.push(change(
            head,
            PolicyChangeKind::ExpiryExtended,
            PolicyChangeSeverity::Review,
            "expiry extended or removed",
        ));
    }
    if date_extended(
        base.lifecycle.review_after.as_deref(),
        head.lifecycle.review_after.as_deref(),
    ) {
        changes.push(change(
            head,
            PolicyChangeKind::ReviewAfterExtended,
            PolicyChangeSeverity::Review,
            "review_after extended or removed",
        ));
    }
    if removed_values(&base.evidence, &head.evidence) {
        changes.push(change(
            head,
            PolicyChangeKind::EvidenceRemoved,
            PolicyChangeSeverity::Fail,
            "evidence removed",
        ));
    }
    if removed_required_text(&base.owner, &head.owner) {
        changes.push(change(
            head,
            PolicyChangeKind::OwnerRemoved,
            PolicyChangeSeverity::Fail,
            "owner removed",
        ));
    }
    if removed_required_text(&base.reason, &head.reason) {
        changes.push(change(
            head,
            PolicyChangeKind::ReasonRemoved,
            PolicyChangeSeverity::Fail,
            "reason removed",
        ));
    }
    if removed_required_text(&base.classification, &head.classification) {
        changes.push(change(
            head,
            PolicyChangeKind::ClassificationRemoved,
            PolicyChangeSeverity::Fail,
            "classification removed",
        ));
    }
    if occurrence_limit_loosened(base.occurrence_limit, head.occurrence_limit) {
        changes.push(change(
            head,
            PolicyChangeKind::OccurrenceLimitLoosened,
            PolicyChangeSeverity::Fail,
            "occurrence_limit increased or removed",
        ));
    }
    changes
}

fn change(
    entry: &AllowEntry,
    kind: PolicyChangeKind,
    severity: PolicyChangeSeverity,
    message: &str,
) -> PolicyChange {
    PolicyChange {
        allow_id: entry.id.clone(),
        kind,
        severity,
        message: format!("{} {message}", entry.id),
    }
}

pub fn selector_precision_score(entry: &AllowEntry) -> u32 {
    let selector = &entry.selector;
    let mut score = 0;
    if entry.path.is_some() {
        score += 20;
    }
    if entry.glob.is_some() || selector.glob.is_some() {
        score += 5;
    }
    if entry.family.is_some() {
        score += 10;
    }
    if selector.ast_kind.is_some() {
        score += 15;
    }
    if selector.container.is_some() {
        score += 15;
    }
    if selector.callee.is_some() {
        score += 10;
    }
    if selector.macro_name.is_some() {
        score += 10;
    }
    if selector.lint.is_some() {
        score += 10;
    }
    if selector.symbol.is_some() {
        score += 8;
    }
    if selector.receiver_fingerprint.is_some() {
        score += 6;
    }
    if selector.target_fingerprint.is_some() {
        score += 6;
    }
    if selector.normalized_snippet_hash.is_some() {
        score += 20;
    }
    if entry.occurrence_limit.is_some() {
        score += 5;
    }
    score
}

fn scope_broadened(base: &AllowEntry, head: &AllowEntry) -> bool {
    let base_exact_path =
        base.path.is_some() && base.glob.is_none() && base.selector.glob.is_none();
    let head_uses_glob = head.glob.is_some() || head.selector.glob.is_some();
    if base_exact_path && head_uses_glob {
        return true;
    }
    if glob_scope_broadened(base.glob.as_deref(), head.glob.as_deref())
        || glob_scope_broadened(base.selector.glob.as_deref(), head.selector.glob.as_deref())
    {
        return true;
    }
    match (entry_scope_text(base), entry_scope_text(head)) {
        (Some(base_scope), Some(head_scope)) => {
            head_scope.contains('*')
                && !base_scope.contains('*')
                && wildcard_covers_path(head_scope, base_scope)
        }
        _ => false,
    }
}

fn glob_scope_broadened(base: Option<&str>, head: Option<&str>) -> bool {
    match (base, head) {
        (Some(base), Some(head)) => head != base && wildcard_covers_path(head, base),
        (None, Some(head)) => head.contains('*'),
        _ => false,
    }
}

fn entry_scope_text(entry: &AllowEntry) -> Option<&str> {
    entry
        .path
        .as_ref()
        .and_then(|path| path.to_str())
        .or(entry.glob.as_deref())
        .or(entry.selector.glob.as_deref())
}

fn wildcard_covers_path(pattern: &str, path: &str) -> bool {
    if pattern == "*" || pattern == "**" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix("/**") {
        return path.starts_with(prefix);
    }
    if let Some(prefix) = pattern.split('*').next() {
        return !prefix.is_empty() && path.starts_with(prefix);
    }
    false
}

fn date_extended(base: Option<&str>, head: Option<&str>) -> bool {
    match (base, head) {
        (Some(base), Some(head)) if base == head => false,
        (Some(base), Some("never")) if base != "never" => true,
        (Some(base), Some(head)) => match (SimpleDate::parse(base), SimpleDate::parse(head)) {
            (Some(base_date), Some(head_date)) => head_date > base_date,
            _ => false,
        },
        (Some(base), None) => base != "never",
        _ => false,
    }
}

fn removed_values(base: &[String], head: &[String]) -> bool {
    base.iter()
        .any(|item| !head.iter().any(|head| head == item))
}

fn removed_required_text(base: &str, head: &str) -> bool {
    !base.trim().is_empty() && head.trim().is_empty()
}

fn occurrence_limit_loosened(base: Option<u32>, head: Option<u32>) -> bool {
    match (base, head) {
        (Some(_), None) => true,
        (Some(base), Some(head)) => head > base,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{FindingKind, Lifecycle, Selector, Span, StructuralIdentity};
    use std::path::PathBuf;

    #[test]
    fn finding_posture_ignores_line_movement_for_same_identity() {
        let base = vec![finding("src/lib.rs", 10, "load")];
        let head = vec![finding("src/lib.rs", 99, "load")];

        let changes = finding_posture_changes(&base, &head);

        assert!(changes.is_empty());
    }

    #[test]
    fn finding_posture_reports_new_and_removed_findings() {
        let base = vec![finding("src/old.rs", 10, "old")];
        let head = vec![finding("src/new.rs", 10, "new")];

        let changes = finding_posture_changes(&base, &head);

        assert!(changes.iter().any(|change| {
            change.kind == FindingPostureKind::New && change.path == "src/new.rs"
        }));
        assert!(changes.iter().any(|change| {
            change.kind == FindingPostureKind::Removed && change.path == "src/old.rs"
        }));
    }

    #[test]
    fn finding_posture_reports_count_changes_for_same_identity() {
        let base = vec![finding("src/lib.rs", 10, "load")];
        let head = vec![
            finding("src/lib.rs", 10, "load"),
            finding("src/lib.rs", 20, "load"),
        ];

        let changes = finding_posture_changes(&base, &head);

        assert_eq!(changes.len(), 1);
        let change = changes
            .first()
            .unwrap_or_else(|| std::panic::panic_any("expected one posture change"));
        assert_eq!(change.kind, FindingPostureKind::New);
        assert_eq!(change.path, "src/lib.rs");
    }

    #[test]
    fn detects_scope_broadening_from_path_to_glob() {
        let base = config_with(entry("allow-1"));
        let mut widened = entry("allow-1");
        widened.path = None;
        widened.glob = Some("src/**".to_string());
        let head = config_with(widened);

        let changes = policy_changes(&base, &head);

        assert!(changes.iter().any(|change| {
            change.kind == PolicyChangeKind::ScopeBroadened
                && change.severity == PolicyChangeSeverity::Fail
        }));
    }

    #[test]
    fn detects_selector_glob_broadening_even_when_path_remains() {
        let mut base_entry = entry("allow-1");
        base_entry.selector.glob = Some("src/lib.rs".to_string());
        let base = config_with(base_entry);
        let mut widened = entry("allow-1");
        widened.selector.glob = Some("src/**".to_string());
        let head = config_with(widened);

        let changes = policy_changes(&base, &head);

        assert!(
            changes
                .iter()
                .any(|change| change.kind == PolicyChangeKind::ScopeBroadened)
        );
    }

    #[test]
    fn detects_selector_precision_decrease() {
        let base = config_with(entry("allow-1"));
        let mut weaker = entry("allow-1");
        weaker.selector.normalized_snippet_hash = None;
        weaker.selector.container = None;
        let head = config_with(weaker);

        let changes = policy_changes(&base, &head);

        assert!(changes.iter().any(|change| {
            change.kind == PolicyChangeKind::SelectorPrecisionDecreased
                && change.message.contains("decreased")
        }));
    }

    #[test]
    fn detects_evidence_removed_and_lifecycle_extended() {
        let base = config_with(entry("allow-1"));
        let mut weaker = entry("allow-1");
        weaker.evidence.clear();
        weaker.lifecycle.expires = Some("2026-12-01".to_string());
        weaker.lifecycle.review_after = Some("2026-10-01".to_string());
        let head = config_with(weaker);

        let changes = policy_changes(&base, &head);

        assert!(
            changes
                .iter()
                .any(|change| change.kind == PolicyChangeKind::EvidenceRemoved)
        );
        assert!(
            changes
                .iter()
                .any(|change| change.kind == PolicyChangeKind::ExpiryExtended)
        );
        assert!(
            changes
                .iter()
                .any(|change| change.kind == PolicyChangeKind::ReviewAfterExtended)
        );
    }

    #[test]
    fn detects_required_metadata_removed_and_limit_loosened() {
        let base = config_with(entry("allow-1"));
        let mut weaker = entry("allow-1");
        weaker.owner.clear();
        weaker.reason.clear();
        weaker.classification.clear();
        weaker.occurrence_limit = None;
        let head = config_with(weaker);

        let changes = policy_changes(&base, &head);

        assert!(
            changes
                .iter()
                .any(|change| change.kind == PolicyChangeKind::OwnerRemoved)
        );
        assert!(
            changes
                .iter()
                .any(|change| change.kind == PolicyChangeKind::ReasonRemoved)
        );
        assert!(
            changes
                .iter()
                .any(|change| change.kind == PolicyChangeKind::ClassificationRemoved)
        );
        assert!(
            changes
                .iter()
                .any(|change| change.kind == PolicyChangeKind::OccurrenceLimitLoosened)
        );
    }

    #[test]
    fn detects_added_baseline_debt_as_failure() {
        let base = config_with(entry("allow-1"));
        let mut added = entry("allow-2");
        added.classification = "baseline_debt".to_string();
        let mut head = base.clone();
        head.allow.push(added);

        let changes = policy_changes(&base, &head);

        assert!(changes.iter().any(|change| {
            change.kind == PolicyChangeKind::BaselineDebtAdded
                && change.severity == PolicyChangeSeverity::Fail
        }));
    }

    fn config_with(entry: AllowEntry) -> AllowConfig {
        let mut cfg = AllowConfig::empty();
        cfg.allow.push(entry);
        cfg
    }

    fn entry(id: &str) -> AllowEntry {
        AllowEntry {
            id: id.to_string(),
            kind: FindingKind::Panic,
            family: Some("unwrap".to_string()),
            path: Some(PathBuf::from("src/lib.rs")),
            glob: None,
            owner: "core".to_string(),
            classification: "reviewed_exception".to_string(),
            reason: "Range is validated before use.".to_string(),
            evidence: vec!["test:range_is_validated".to_string()],
            links: Vec::new(),
            occurrence_limit: Some(1),
            lifecycle: Lifecycle {
                created: Some("2026-05-26".to_string()),
                review_after: Some("2026-08-01".to_string()),
                expires: Some("2026-09-01".to_string()),
            },
            selector: Selector {
                ast_kind: Some("method_call".to_string()),
                container: Some("load".to_string()),
                callee: Some("unwrap".to_string()),
                normalized_snippet_hash: Some("fnv1a64:1234".to_string()),
                ..Selector::default()
            },
            last_seen: None,
        }
    }

    fn finding(path: &str, line: u32, container: &str) -> Finding {
        let mut identity = StructuralIdentity::new("rust", "unsafe_fn");
        identity.container = Some(container.to_string());
        identity.normalized_snippet_hash = Some(format!("fnv1a64:{container}"));
        Finding {
            kind: FindingKind::Unsafe,
            family: Some("unsafe_fn".to_string()),
            path: PathBuf::from(path),
            span: Some(Span { line, column: 1 }),
            identity,
            message: "test finding".to_string(),
        }
    }
}
