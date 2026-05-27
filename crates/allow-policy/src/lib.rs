use allow_core::{
    AllowConfig, AllowEntry, CargoAllowError, CargoAllowResult, FindingKind, LastSeen, Lifecycle,
    Requirements, Selector, SimpleDate, WorkspaceConfig,
};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

#[derive(Debug, Default, Deserialize)]
struct PolicyToml {
    schema_version: Option<String>,
    policy: Option<String>,
    owner: Option<String>,
    status: Option<String>,
    #[serde(default)]
    workspace: WorkspaceToml,
    #[serde(default)]
    requirements: RequirementsToml,
    #[serde(default)]
    allow: Vec<AllowEntryToml>,
}

#[derive(Debug, Default, Deserialize)]
struct WorkspaceToml {
    root: Option<String>,
    inventory: Option<String>,
    default_mode: Option<String>,
    #[serde(default, deserialize_with = "string_or_vec")]
    ignored: Vec<String>,
    #[serde(default, deserialize_with = "string_or_vec")]
    generated: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RequirementsToml {
    #[serde(default, deserialize_with = "option_bool_or_string")]
    owner_required: Option<bool>,
    #[serde(default, deserialize_with = "option_bool_or_string")]
    reason_required: Option<bool>,
    #[serde(default, deserialize_with = "option_bool_or_string")]
    classification_required: Option<bool>,
    #[serde(default, deserialize_with = "option_bool_or_string")]
    expires_or_review_after_required: Option<bool>,
    #[serde(default, deserialize_with = "option_bool_or_string")]
    allow_bare_allow_attributes: Option<bool>,
    #[serde(default, deserialize_with = "option_bool_or_string")]
    lint_policy_id_required: Option<bool>,
    #[serde(default, deserialize_with = "option_bool_or_string")]
    stale_entries_fail: Option<bool>,
    #[serde(default, rename = "unsafe")]
    unsafe_requirements: UnsafeRequirementsToml,
}

#[derive(Debug, Default, Deserialize)]
struct UnsafeRequirementsToml {
    #[serde(default, deserialize_with = "option_bool_or_string")]
    evidence_required: Option<bool>,
    #[serde(default, deserialize_with = "option_bool_or_string")]
    safety_comment_required: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
struct AllowEntryToml {
    id: Option<String>,
    kind: Option<String>,
    family: Option<String>,
    path: Option<PathBuf>,
    glob: Option<String>,
    owner: Option<String>,
    classification: Option<String>,
    #[serde(alias = "explanation")]
    reason: Option<String>,
    #[serde(default, alias = "covered_by", deserialize_with = "string_or_vec")]
    evidence: Vec<String>,
    #[serde(default, deserialize_with = "string_or_vec")]
    links: Vec<String>,
    #[serde(alias = "count")]
    occurrence_limit: Option<u32>,
    created: Option<String>,
    review_after: Option<String>,
    expires: Option<String>,
    #[serde(default)]
    selector: SelectorToml,
    #[serde(default)]
    last_seen: LastSeenToml,
}

#[derive(Debug, Default, Deserialize)]
struct SelectorToml {
    #[serde(alias = "kind")]
    ast_kind: Option<String>,
    container: Option<String>,
    callee: Option<String>,
    #[serde(alias = "macro")]
    macro_name: Option<String>,
    lint: Option<String>,
    symbol: Option<String>,
    receiver_fingerprint: Option<String>,
    target_fingerprint: Option<String>,
    normalized_snippet_hash: Option<String>,
    #[serde(default, deserialize_with = "option_u32_or_string")]
    line_hint: Option<u32>,
    glob: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct LastSeenToml {
    #[serde(default, deserialize_with = "option_u32_or_string")]
    line: Option<u32>,
    #[serde(default, deserialize_with = "option_u32_or_string")]
    column: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum StringList {
    One(String),
    Many(Vec<String>),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Boolish {
    Bool(bool),
    String(String),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum U32ish {
    Number(u32),
    String(String),
}

const BASELINE_DEBT_MAX_DAYS: i64 = 120;

pub fn find_config(start: impl AsRef<Path>) -> Option<PathBuf> {
    let mut dir = start.as_ref().canonicalize().ok()?;
    loop {
        for rel in ["policy/allow.toml", ".cargo/allow.toml", "allow.toml"] {
            let candidate = dir.join(rel);
            if candidate.exists() {
                return Some(candidate);
            }
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

pub fn load_policy(path: impl AsRef<Path>) -> CargoAllowResult<AllowConfig> {
    let text = fs::read_to_string(path.as_ref()).map_err(|e| {
        CargoAllowError::new(format!("failed to read {}: {e}", path.as_ref().display()))
    })?;
    parse_policy(&text)
}

pub fn parse_policy(input: &str) -> CargoAllowResult<AllowConfig> {
    let raw = toml::from_str::<PolicyToml>(input)
        .map_err(|e| CargoAllowError::new(format!("failed to parse policy TOML: {e}")))?;
    let cfg = raw.into_config()?;
    validate_policy(&cfg)?;
    Ok(cfg)
}

impl PolicyToml {
    fn into_config(self) -> CargoAllowResult<AllowConfig> {
        let allow = self
            .allow
            .into_iter()
            .enumerate()
            .map(|(index, entry)| entry.into_allow_entry(index))
            .collect::<CargoAllowResult<Vec<_>>>()?;
        Ok(AllowConfig {
            schema_version: self.schema_version.unwrap_or_else(|| "0.1".to_string()),
            policy: self.policy.unwrap_or_else(|| "cargo-allow".to_string()),
            owner: self.owner,
            status: self.status,
            workspace: self.workspace.into_workspace_config(),
            requirements: self.requirements.into_requirements(),
            allow,
        })
    }
}

impl WorkspaceToml {
    fn into_workspace_config(self) -> WorkspaceConfig {
        let default = WorkspaceConfig::default();
        WorkspaceConfig {
            root: self.root.unwrap_or(default.root),
            inventory: self.inventory.unwrap_or(default.inventory),
            ignored: if self.ignored.is_empty() {
                default.ignored
            } else {
                self.ignored
            },
            generated: if self.generated.is_empty() {
                default.generated
            } else {
                self.generated
            },
            default_mode: self.default_mode.unwrap_or(default.default_mode),
        }
    }
}

impl RequirementsToml {
    fn into_requirements(self) -> Requirements {
        let default = Requirements::default();
        Requirements {
            owner_required: self.owner_required.unwrap_or(default.owner_required),
            reason_required: self.reason_required.unwrap_or(default.reason_required),
            classification_required: self
                .classification_required
                .unwrap_or(default.classification_required),
            expires_or_review_after_required: self
                .expires_or_review_after_required
                .unwrap_or(default.expires_or_review_after_required),
            allow_bare_allow_attributes: self
                .allow_bare_allow_attributes
                .unwrap_or(default.allow_bare_allow_attributes),
            lint_policy_id_required: self
                .lint_policy_id_required
                .unwrap_or(default.lint_policy_id_required),
            stale_entries_fail: self
                .stale_entries_fail
                .unwrap_or(default.stale_entries_fail),
            unsafe_evidence_required: self
                .unsafe_requirements
                .evidence_required
                .unwrap_or(default.unsafe_evidence_required),
            unsafe_safety_comment_required: self
                .unsafe_requirements
                .safety_comment_required
                .unwrap_or(default.unsafe_safety_comment_required),
        }
    }
}

impl AllowEntryToml {
    fn into_allow_entry(self, index: usize) -> CargoAllowResult<AllowEntry> {
        let id = self.id.unwrap_or_else(|| format!("allow-{:04}", index + 1));
        let kind_text = self
            .kind
            .ok_or_else(|| CargoAllowError::new(format!("{id} missing kind")))?;
        let kind = FindingKind::from_str(&kind_text)?;
        let last_seen = match (self.last_seen.line, self.last_seen.column) {
            (Some(line), Some(column)) => Some(LastSeen { line, column }),
            _ => None,
        };
        Ok(AllowEntry {
            id,
            kind,
            family: self.family,
            path: self.path,
            glob: self.glob,
            owner: self.owner.unwrap_or_default(),
            classification: self.classification.unwrap_or_default(),
            reason: self.reason.unwrap_or_default(),
            evidence: self.evidence,
            links: self.links,
            occurrence_limit: self.occurrence_limit,
            lifecycle: Lifecycle {
                created: self.created,
                review_after: self.review_after,
                expires: self.expires,
            },
            selector: Selector {
                ast_kind: self.selector.ast_kind,
                container: self.selector.container,
                callee: self.selector.callee,
                macro_name: self.selector.macro_name,
                lint: self.selector.lint,
                symbol: self.selector.symbol,
                receiver_fingerprint: self.selector.receiver_fingerprint,
                target_fingerprint: self.selector.target_fingerprint,
                normalized_snippet_hash: self.selector.normalized_snippet_hash,
                line_hint: self.selector.line_hint,
                glob: self.selector.glob,
            },
            last_seen,
        })
    }
}

pub fn validate_policy(cfg: &AllowConfig) -> CargoAllowResult<()> {
    if cfg.policy != "cargo-allow" {
        return Err(CargoAllowError::new(format!(
            "unsupported policy `{}`",
            cfg.policy
        )));
    }
    for pattern in &cfg.workspace.ignored {
        validate_glob("source-tree ignored glob", pattern)?;
    }
    for pattern in &cfg.workspace.generated {
        validate_glob("source-tree generated glob", pattern)?;
    }
    let mut ids = BTreeSet::new();
    for entry in &cfg.allow {
        if entry.id.trim().is_empty() {
            return Err(CargoAllowError::new("allow entry has empty id"));
        }
        if !ids.insert(entry.id.clone()) {
            return Err(CargoAllowError::new(format!(
                "duplicate allow id `{}`",
                entry.id
            )));
        }
        if entry.path.is_none() && entry.glob.is_none() && entry.selector.glob.is_none() {
            return Err(CargoAllowError::new(format!(
                "{} has no path or glob",
                entry.id
            )));
        }
        if let Some(path) = &entry.path {
            validate_path_scope(&entry.id, path)?;
        }
        if let Some(glob) = &entry.glob {
            validate_glob(&format!("{} glob", entry.id), glob)?;
        }
        if let Some(glob) = &entry.selector.glob {
            validate_glob(&format!("{} selector glob", entry.id), glob)?;
        }
        validate_selector(entry)?;
        validate_lifecycle(entry)?;
        if cfg.requirements.owner_required && entry.owner.trim().is_empty() {
            return Err(CargoAllowError::new(format!("{} missing owner", entry.id)));
        }
        if cfg.requirements.reason_required && entry.reason.trim().is_empty() {
            return Err(CargoAllowError::new(format!("{} missing reason", entry.id)));
        }
        if cfg.requirements.classification_required && entry.classification.trim().is_empty() {
            return Err(CargoAllowError::new(format!(
                "{} missing classification",
                entry.id
            )));
        }
        if cfg.requirements.expires_or_review_after_required
            && entry.lifecycle.expires.is_none()
            && entry.lifecycle.review_after.is_none()
        {
            return Err(CargoAllowError::new(format!(
                "{} missing expires or review_after",
                entry.id
            )));
        }
        if cfg.requirements.unsafe_evidence_required
            && entry.kind == FindingKind::Unsafe
            && entry.evidence.is_empty()
        {
            return Err(CargoAllowError::new(format!(
                "{} unsafe entry missing evidence",
                entry.id
            )));
        }
        if entry.occurrence_limit == Some(0) {
            return Err(CargoAllowError::new(format!(
                "{} occurrence_limit must be greater than zero",
                entry.id
            )));
        }
    }
    Ok(())
}

pub fn validate_local_evidence_references(
    root: impl AsRef<Path>,
    cfg: &AllowConfig,
) -> CargoAllowResult<()> {
    let root = root.as_ref();
    for entry in &cfg.allow {
        for evidence in &entry.evidence {
            let Some(reference) = EvidenceReference::parse(evidence) else {
                continue;
            };
            if reference.kind.is_local_file() {
                validate_path_scope(
                    &format!("{} evidence `{}`", entry.id, reference.raw),
                    reference.value.as_ref(),
                )?;
                let path = root.join(&reference.value);
                if !path.exists() {
                    return Err(CargoAllowError::new(format!(
                        "{} evidence `{}` references missing local file {}",
                        entry.id,
                        reference.raw,
                        reference.value.display()
                    )));
                }
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceReferenceStatus {
    LocalFilePresent,
    LocalFileMissing,
    InvalidLocalPath,
    TraceabilityOnly,
    Unstructured,
}

impl EvidenceReferenceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LocalFilePresent => "local_file_present",
            Self::LocalFileMissing => "local_file_missing",
            Self::InvalidLocalPath => "invalid_local_path",
            Self::TraceabilityOnly => "traceability_only",
            Self::Unstructured => "unstructured",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceReferenceDiagnostic {
    pub raw: String,
    pub prefix: Option<String>,
    pub target: Option<PathBuf>,
    pub status: EvidenceReferenceStatus,
    pub message: String,
}

pub fn evidence_reference_diagnostics(
    root: impl AsRef<Path>,
    entry: &AllowEntry,
) -> Vec<EvidenceReferenceDiagnostic> {
    let root = root.as_ref();
    entry
        .evidence
        .iter()
        .map(|evidence| evidence_reference_diagnostic(root, evidence))
        .collect()
}

fn evidence_reference_diagnostic(root: &Path, raw: &str) -> EvidenceReferenceDiagnostic {
    let Some(reference) = EvidenceReference::parse(raw) else {
        return EvidenceReferenceDiagnostic {
            raw: raw.to_string(),
            prefix: None,
            target: None,
            status: EvidenceReferenceStatus::Unstructured,
            message: "unstructured evidence string; not locally validated".to_string(),
        };
    };
    let prefix = Some(reference.prefix.to_string());
    if reference.kind == EvidenceKind::Unknown {
        return EvidenceReferenceDiagnostic {
            raw: raw.to_string(),
            prefix,
            target: Some(reference.value.clone()),
            status: EvidenceReferenceStatus::Unstructured,
            message: "unrecognized evidence prefix; not locally validated".to_string(),
        };
    }
    if !reference.kind.is_local_file() {
        return EvidenceReferenceDiagnostic {
            raw: raw.to_string(),
            prefix,
            target: Some(reference.value.clone()),
            status: EvidenceReferenceStatus::TraceabilityOnly,
            message: "traceability reference; not executed or resolved by cargo-allow".to_string(),
        };
    }
    if let Err(err) = validate_path_scope("evidence", reference.value.as_ref()) {
        return EvidenceReferenceDiagnostic {
            raw: raw.to_string(),
            prefix,
            target: Some(reference.value.clone()),
            status: EvidenceReferenceStatus::InvalidLocalPath,
            message: err.to_string(),
        };
    }
    if root.join(&reference.value).exists() {
        EvidenceReferenceDiagnostic {
            raw: raw.to_string(),
            prefix,
            target: Some(reference.value.clone()),
            status: EvidenceReferenceStatus::LocalFilePresent,
            message: "local evidence file exists".to_string(),
        }
    } else {
        EvidenceReferenceDiagnostic {
            raw: raw.to_string(),
            prefix,
            target: Some(reference.value.clone()),
            status: EvidenceReferenceStatus::LocalFileMissing,
            message: "local evidence file is missing".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EvidenceKind {
    Test,
    Cargo,
    Ripr,
    UnsafeReview,
    Coverage,
    Doc,
    Spec,
    Adr,
    Issue,
    Pr,
    Unknown,
}

impl EvidenceKind {
    fn parse(prefix: &str) -> Self {
        match prefix {
            "test" => Self::Test,
            "cargo" => Self::Cargo,
            "ripr" => Self::Ripr,
            "unsafe-review" | "unsafe_review" => Self::UnsafeReview,
            "coverage" => Self::Coverage,
            "doc" => Self::Doc,
            "spec" => Self::Spec,
            "adr" => Self::Adr,
            "issue" => Self::Issue,
            "pr" => Self::Pr,
            _ => Self::Unknown,
        }
    }

    fn is_local_file(self) -> bool {
        matches!(
            self,
            Self::Ripr | Self::UnsafeReview | Self::Coverage | Self::Doc | Self::Spec | Self::Adr
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EvidenceReference<'a> {
    raw: &'a str,
    prefix: &'a str,
    kind: EvidenceKind,
    value: PathBuf,
}

impl<'a> EvidenceReference<'a> {
    fn parse(raw: &'a str) -> Option<Self> {
        let (prefix, value) = raw.split_once(':')?;
        let value = value.trim();
        if value.is_empty() {
            return Some(Self {
                raw,
                prefix: prefix.trim(),
                kind: EvidenceKind::parse(prefix.trim()),
                value: PathBuf::new(),
            });
        }
        Some(Self {
            raw,
            prefix: prefix.trim(),
            kind: EvidenceKind::parse(prefix.trim()),
            value: PathBuf::from(value),
        })
    }
}

fn validate_path_scope(id: &str, path: &Path) -> CargoAllowResult<()> {
    let text = path.to_string_lossy().replace('\\', "/");
    if text.trim().is_empty() {
        return Err(CargoAllowError::new(format!("{id} has empty path")));
    }
    if text.starts_with('/') || text.contains(':') {
        return Err(CargoAllowError::new(format!(
            "{id} path must be source-tree-relative"
        )));
    }
    if text.split('/').any(|part| part == "..") {
        return Err(CargoAllowError::new(format!(
            "{id} path must not contain parent directory segments"
        )));
    }
    Ok(())
}

fn validate_glob(label: &str, glob: &str) -> CargoAllowResult<()> {
    let text = glob.replace('\\', "/");
    if text.trim().is_empty() {
        return Err(CargoAllowError::new(format!("{label} is empty")));
    }
    if text.starts_with('/') || text.contains(':') {
        return Err(CargoAllowError::new(format!(
            "{label} must be source-tree-relative"
        )));
    }
    if text.split('/').any(|part| part == "..") {
        return Err(CargoAllowError::new(format!(
            "{label} must not contain parent directory segments"
        )));
    }
    Ok(())
}

fn validate_selector(entry: &AllowEntry) -> CargoAllowResult<()> {
    let selector = &entry.selector;
    let has_identity = selector.ast_kind.is_some()
        || selector.container.is_some()
        || selector.callee.is_some()
        || selector.macro_name.is_some()
        || selector.lint.is_some()
        || selector.symbol.is_some()
        || selector.receiver_fingerprint.is_some()
        || selector.target_fingerprint.is_some()
        || selector.normalized_snippet_hash.is_some()
        || selector.glob.is_some();
    if !has_identity {
        return Err(CargoAllowError::new(format!(
            "{} selector must include structural identity beyond line hints",
            entry.id
        )));
    }
    Ok(())
}

fn validate_lifecycle(entry: &AllowEntry) -> CargoAllowResult<()> {
    let created = parse_lifecycle_date(&entry.id, "created", entry.lifecycle.created.as_deref())?;
    let review_after = parse_lifecycle_date(
        &entry.id,
        "review_after",
        entry.lifecycle.review_after.as_deref(),
    )?;
    let expires = parse_expires(&entry.id, entry.lifecycle.expires.as_deref())?;

    if let (Some(created), Some(review_after)) = (created, review_after) {
        if created.days_until(review_after) < 0 {
            return Err(CargoAllowError::new(format!(
                "{} review_after must not be before created",
                entry.id
            )));
        }
    }
    if let (Some(created), Some(expires)) = (created, expires) {
        if created.days_until(expires) < 0 {
            return Err(CargoAllowError::new(format!(
                "{} expires must not be before created",
                entry.id
            )));
        }
    }
    if entry.classification == "baseline_debt" {
        let expires = expires.ok_or_else(|| {
            CargoAllowError::new(format!("{} baseline_debt requires expires", entry.id))
        })?;
        let start = created.unwrap_or_else(SimpleDate::today_utc_approx);
        let days = start.days_until(expires);
        if !(0..=BASELINE_DEBT_MAX_DAYS).contains(&days) {
            return Err(CargoAllowError::new(format!(
                "{} baseline_debt expires must be within {BASELINE_DEBT_MAX_DAYS} days",
                entry.id
            )));
        }
    }
    Ok(())
}

fn parse_lifecycle_date(
    id: &str,
    field: &str,
    value: Option<&str>,
) -> CargoAllowResult<Option<SimpleDate>> {
    match value {
        Some(value) => SimpleDate::parse(value).map(Some).ok_or_else(|| {
            CargoAllowError::new(format!("{id} has invalid {field} date `{value}`"))
        }),
        None => Ok(None),
    }
}

fn parse_expires(id: &str, value: Option<&str>) -> CargoAllowResult<Option<SimpleDate>> {
    match value {
        Some("never") => Ok(None),
        Some(value) => SimpleDate::parse(value).map(Some).ok_or_else(|| {
            CargoAllowError::new(format!("{id} has invalid expires date `{value}`"))
        }),
        None => Ok(None),
    }
}

pub fn starter_policy(strict: bool) -> String {
    let stale = if strict { "true" } else { "false" };
    format!(
        r#"schema_version = "0.1"
policy = "cargo-allow"
owner = "core/policy"
status = "active"

[workspace]
root = "."
inventory = "git-tracked"
default_mode = "{}"
ignored = [".git/**", "target/**"]
generated = ["target/**", "vendor/**"]

[requirements]
owner_required = true
reason_required = true
classification_required = true
expires_or_review_after_required = true
allow_bare_allow_attributes = false
lint_policy_id_required = false
stale_entries_fail = {stale}

[requirements.unsafe]
evidence_required = true
safety_comment_required = false
"#,
        if strict { "strict" } else { "no-new" }
    )
}

pub fn render_policy(cfg: &AllowConfig) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "schema_version = \"{}\"\npolicy = \"{}\"\n",
        cfg.schema_version, cfg.policy
    ));
    if let Some(owner) = &cfg.owner {
        out.push_str(&format!("owner = \"{}\"\n", escape_toml(owner)));
    }
    if let Some(status) = &cfg.status {
        out.push_str(&format!("status = \"{}\"\n", escape_toml(status)));
    }
    out.push('\n');
    out.push_str("[workspace]\n");
    out.push_str(&format!(
        "root = \"{}\"\ninventory = \"{}\"\ndefault_mode = \"{}\"\n",
        escape_toml(&cfg.workspace.root),
        escape_toml(&cfg.workspace.inventory),
        escape_toml(&cfg.workspace.default_mode)
    ));
    out.push_str(&format!(
        "ignored = [{}]\n",
        render_array(&cfg.workspace.ignored)
    ));
    out.push_str(&format!(
        "generated = [{}]\n\n",
        render_array(&cfg.workspace.generated)
    ));
    out.push_str("[requirements]\n");
    out.push_str(&format!("owner_required = {}\nreason_required = {}\nclassification_required = {}\nexpires_or_review_after_required = {}\nallow_bare_allow_attributes = {}\nlint_policy_id_required = {}\nstale_entries_fail = {}\n\n", cfg.requirements.owner_required, cfg.requirements.reason_required, cfg.requirements.classification_required, cfg.requirements.expires_or_review_after_required, cfg.requirements.allow_bare_allow_attributes, cfg.requirements.lint_policy_id_required, cfg.requirements.stale_entries_fail));
    out.push_str("[requirements.unsafe]\n");
    out.push_str(&format!(
        "evidence_required = {}\nsafety_comment_required = {}\n",
        cfg.requirements.unsafe_evidence_required, cfg.requirements.unsafe_safety_comment_required
    ));
    for entry in &cfg.allow {
        out.push_str("\n[[allow]]\n");
        out.push_str(&format!(
            "id = \"{}\"\nkind = \"{}\"\n",
            escape_toml(&entry.id),
            entry.kind.as_str()
        ));
        if let Some(family) = &entry.family {
            out.push_str(&format!("family = \"{}\"\n", escape_toml(family)));
        }
        if let Some(path) = &entry.path {
            out.push_str(&format!(
                "path = \"{}\"\n",
                escape_toml(&path.to_string_lossy())
            ));
        }
        if let Some(glob) = &entry.glob {
            out.push_str(&format!("glob = \"{}\"\n", escape_toml(glob)));
        }
        out.push_str(&format!(
            "owner = \"{}\"\nclassification = \"{}\"\nreason = \"{}\"\n",
            escape_toml(&entry.owner),
            escape_toml(&entry.classification),
            escape_toml(&entry.reason)
        ));
        if !entry.evidence.is_empty() {
            out.push_str(&format!("evidence = [{}]\n", render_array(&entry.evidence)));
        }
        if !entry.links.is_empty() {
            out.push_str(&format!("links = [{}]\n", render_array(&entry.links)));
        }
        if let Some(limit) = entry.occurrence_limit {
            out.push_str(&format!("occurrence_limit = {limit}\n"));
        }
        if let Some(created) = &entry.lifecycle.created {
            out.push_str(&format!("created = \"{}\"\n", escape_toml(created)));
        }
        if let Some(review_after) = &entry.lifecycle.review_after {
            out.push_str(&format!(
                "review_after = \"{}\"\n",
                escape_toml(review_after)
            ));
        }
        if let Some(expires) = &entry.lifecycle.expires {
            out.push_str(&format!("expires = \"{}\"\n", escape_toml(expires)));
        }
        out.push_str("\n[allow.selector]\n");
        if let Some(v) = &entry.selector.ast_kind {
            out.push_str(&format!("ast_kind = \"{}\"\n", escape_toml(v)));
        }
        if let Some(v) = &entry.selector.container {
            out.push_str(&format!("container = \"{}\"\n", escape_toml(v)));
        }
        if let Some(v) = &entry.selector.callee {
            out.push_str(&format!("callee = \"{}\"\n", escape_toml(v)));
        }
        if let Some(v) = &entry.selector.macro_name {
            out.push_str(&format!("macro_name = \"{}\"\n", escape_toml(v)));
        }
        if let Some(v) = &entry.selector.lint {
            out.push_str(&format!("lint = \"{}\"\n", escape_toml(v)));
        }
        if let Some(v) = &entry.selector.symbol {
            out.push_str(&format!("symbol = \"{}\"\n", escape_toml(v)));
        }
        if let Some(v) = &entry.selector.receiver_fingerprint {
            out.push_str(&format!("receiver_fingerprint = \"{}\"\n", escape_toml(v)));
        }
        if let Some(v) = &entry.selector.target_fingerprint {
            out.push_str(&format!("target_fingerprint = \"{}\"\n", escape_toml(v)));
        }
        if let Some(v) = &entry.selector.normalized_snippet_hash {
            out.push_str(&format!(
                "normalized_snippet_hash = \"{}\"\n",
                escape_toml(v)
            ));
        }
        if let Some(v) = entry.selector.line_hint {
            out.push_str(&format!("line_hint = {}\n", v));
        }
        if let Some(v) = &entry.selector.glob {
            out.push_str(&format!("glob = \"{}\"\n", escape_toml(v)));
        }
        if let Some(last) = &entry.last_seen {
            out.push_str("\n[allow.last_seen]\n");
            out.push_str(&format!("line = {}\ncolumn = {}\n", last.line, last.column));
        }
    }
    out
}

fn escape_toml(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

fn string_or_vec<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match StringList::deserialize(deserializer)? {
        StringList::One(value) => Ok(vec![value]),
        StringList::Many(values) => Ok(values),
    }
}

fn option_bool_or_string<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match Option::<Boolish>::deserialize(deserializer)? {
        Some(Boolish::Bool(value)) => Ok(Some(value)),
        Some(Boolish::String(value)) => value
            .parse::<bool>()
            .map(Some)
            .map_err(serde::de::Error::custom),
        None => Ok(None),
    }
}

fn option_u32_or_string<'de, D>(deserializer: D) -> Result<Option<u32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match Option::<U32ish>::deserialize(deserializer)? {
        Some(U32ish::Number(value)) => Ok(Some(value)),
        Some(U32ish::String(value)) => value
            .parse::<u32>()
            .map(Some)
            .map_err(serde::de::Error::custom),
        None => Ok(None),
    }
}

fn render_array(values: &[String]) -> String {
    values
        .iter()
        .map(|v| format!("\"{}\"", escape_toml(v)))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_policy_with_allow() {
        let cfg = parse_policy(
            r#"
                schema_version = "0.1"
                policy = "cargo-allow"

                [requirements]
                expires_or_review_after_required = true
                lint_policy_id_required = true

                [[allow]]
                id = "allow-0001"
                kind = "panic"
                family = "unwrap"
                path = "src/lib.rs"
                owner = "core"
                classification = "test"
                reason = "fixture"
                expires = "2026-08-01"

                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
                container = "load"
            "#,
        )
        .expect("policy parses");
        assert_eq!(cfg.allow.len(), 1);
        assert!(cfg.requirements.lint_policy_id_required);
        assert_eq!(cfg.allow[0].selector.callee.as_deref(), Some("unwrap"));
    }

    #[test]
    fn parses_unsafe_safety_comment_requirement() {
        let cfg = parse_policy(
            r#"
                policy = "cargo-allow"

                [requirements.unsafe]
                safety_comment_required = true

                [[allow]]
                id = "allow-unsafe"
                kind = "unsafe"
                path = "src/lib.rs"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                evidence = ["test:unsafe_boundary"]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "unsafe_block"
                container = "load"
            "#,
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy should parse: {err}")));

        assert!(cfg.requirements.unsafe_safety_comment_required);
    }

    #[test]
    fn rejects_duplicate_ids() {
        let err = parse_policy(
            r#"
                policy = "cargo-allow"
                [[allow]]
                id = "x"
                kind = "non_rust_file"
                path = "a.py"
                owner = "o"
                classification = "c"
                reason = "r"
                expires = "2026-08-01"
                [allow.selector]
                glob = "a.py"
                [[allow]]
                id = "x"
                kind = "non_rust_file"
                path = "b.py"
                owner = "o"
                classification = "c"
                reason = "r"
                expires = "2026-08-01"
                [allow.selector]
                glob = "b.py"
            "#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("duplicate"));
    }

    #[test]
    fn parses_legacy_aliases_and_scalar_arrays() {
        let cfg = parse_policy(
            r#"
                policy = "cargo-allow"

                [workspace]
                ignored = ".git/**"

                [requirements]
                owner_required = "true"

                [[allow]]
                id = "allow-legacy"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "legacy"
                explanation = "legacy reason field"
                covered_by = "test:legacy"
                count = 2
                expires = "2026-08-01"

                [allow.selector]
                kind = "macro_call"
                macro = "panic"
                line_hint = "12"
            "#,
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("legacy aliases parse: {err}")));

        assert_eq!(cfg.workspace.ignored, vec![".git/**"]);
        let entry = cfg
            .allow
            .first()
            .unwrap_or_else(|| std::panic::panic_any("expected one allow entry"));
        assert_eq!(entry.reason, "legacy reason field");
        assert_eq!(entry.evidence, vec!["test:legacy"]);
        assert_eq!(entry.occurrence_limit, Some(2));
        assert_eq!(entry.selector.ast_kind.as_deref(), Some("macro_call"));
        assert_eq!(entry.selector.macro_name.as_deref(), Some("panic"));
        assert_eq!(entry.selector.line_hint, Some(12));
    }

    #[test]
    fn validates_existing_local_evidence_references() {
        let root = unique_test_dir("evidence-existing");
        fs::create_dir_all(root.join("docs"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("create docs dir: {err}")));
        fs::write(root.join("docs/safety.md"), "review notes")
            .unwrap_or_else(|err| std::panic::panic_any(format!("write evidence: {err}")));
        let cfg = parse_policy(
            r#"
                policy = "cargo-allow"
                [[allow]]
                id = "allow-doc"
                kind = "unsafe"
                path = "src/lib.rs"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                evidence = ["doc:docs/safety.md", "test:safety_fixture"]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "unsafe_block"
                container = "load"
            "#,
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy parses: {err}")));

        validate_local_evidence_references(&root, &cfg)
            .unwrap_or_else(|err| std::panic::panic_any(format!("evidence validates: {err}")));
        remove_test_dir(root);
    }

    #[test]
    fn rejects_missing_local_evidence_references() {
        let root = unique_test_dir("evidence-missing");
        fs::create_dir_all(&root)
            .unwrap_or_else(|err| std::panic::panic_any(format!("create root: {err}")));
        let cfg = parse_policy(
            r#"
                policy = "cargo-allow"
                [[allow]]
                id = "allow-doc"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                evidence = ["doc:docs/missing.md"]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy parses: {err}")));

        let err = validate_local_evidence_references(&root, &cfg).unwrap_err();
        assert!(err.to_string().contains("allow-doc evidence"));
        assert!(err.to_string().contains("missing local file"));
        remove_test_dir(root);
    }

    #[test]
    fn rejects_escaping_local_evidence_references() {
        let cfg = parse_policy(
            r#"
                policy = "cargo-allow"
                [[allow]]
                id = "allow-doc"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                evidence = ["doc:../outside.md"]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy parses: {err}")));

        let err = validate_local_evidence_references(".", &cfg).unwrap_err();
        assert!(
            err.to_string()
                .contains("must not contain parent directory segments")
        );
    }

    #[test]
    fn reports_evidence_reference_diagnostics() {
        let root = unique_test_dir("evidence-diagnostics");
        fs::create_dir_all(root.join("docs"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("create docs dir: {err}")));
        fs::write(root.join("docs/safety.md"), "review notes")
            .unwrap_or_else(|err| std::panic::panic_any(format!("write evidence: {err}")));
        let mut entry = AllowEntry {
            id: "allow-doc".to_string(),
            kind: FindingKind::Panic,
            family: Some("unwrap".to_string()),
            path: Some(PathBuf::from("src/lib.rs")),
            glob: None,
            owner: "core".to_string(),
            classification: "reviewed".to_string(),
            reason: "fixture".to_string(),
            evidence: vec![
                "doc:docs/safety.md".to_string(),
                "spec:docs/missing.md".to_string(),
                "test:parser_rejects_bad_range".to_string(),
                "TODO: add reviewed evidence".to_string(),
            ],
            links: Vec::new(),
            occurrence_limit: None,
            lifecycle: Lifecycle {
                created: None,
                review_after: None,
                expires: Some("2026-08-01".to_string()),
            },
            selector: Selector {
                ast_kind: Some("method_call".to_string()),
                callee: Some("unwrap".to_string()),
                ..Selector::default()
            },
            last_seen: None,
        };

        let diagnostics = evidence_reference_diagnostics(&root, &entry);
        assert_eq!(
            diagnostics
                .iter()
                .map(|diagnostic| diagnostic.status)
                .collect::<Vec<_>>(),
            vec![
                EvidenceReferenceStatus::LocalFilePresent,
                EvidenceReferenceStatus::LocalFileMissing,
                EvidenceReferenceStatus::TraceabilityOnly,
                EvidenceReferenceStatus::Unstructured
            ]
        );

        entry.evidence = vec!["doc:../outside.md".to_string()];
        let diagnostics = evidence_reference_diagnostics(&root, &entry);
        assert_eq!(
            diagnostics.first().map(|diagnostic| diagnostic.status),
            Some(EvidenceReferenceStatus::InvalidLocalPath)
        );
        remove_test_dir(root);
    }

    #[test]
    fn reports_toml_parse_errors() {
        let err = parse_policy("policy = [").unwrap_err();

        assert!(err.to_string().contains("failed to parse policy TOML"));
    }

    #[test]
    fn parses_current_repository_policy() {
        let cfg = parse_policy(include_str!("../../../policy/allow.toml"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("repo policy parses: {err}")));

        assert_eq!(cfg.policy, "cargo-allow");
        assert!(cfg.allow.iter().any(|entry| entry.id == "allow-0001"));
        assert!(cfg.allow.iter().any(|entry| entry.id == "allow-0088"));
        for removed in [
            "allow-0019",
            "allow-0020",
            "allow-0031",
            "allow-0032",
            "allow-0033",
            "allow-0039",
            "allow-0041",
            "allow-0042",
            "allow-0043",
            "allow-0044",
            "allow-0045",
            "allow-0046",
            "allow-0047",
            "allow-0048",
            "allow-0049",
            "allow-0050",
            "allow-0051",
            "allow-0054",
            "allow-0056",
            "allow-0057",
            "allow-0058",
            "allow-0059",
            "allow-0060",
            "allow-0061",
            "allow-0062",
            "allow-0063",
            "allow-0064",
            "allow-0065",
            "allow-0066",
        ] {
            assert!(
                !cfg.allow.iter().any(|entry| entry.id == removed),
                "{removed} should stay pruned from the repository policy"
            );
        }
    }

    #[test]
    fn rejects_invalid_lifecycle_dates() {
        let err = parse_err(
            r#"
                policy = "cargo-allow"
                [[allow]]
                id = "bad-date"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "test"
                reason = "fixture"
                expires = "2026-02-31"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
        );

        assert!(err.contains("invalid expires date"));
    }

    #[test]
    fn renders_and_parses_occurrence_limit() {
        let mut cfg = AllowConfig::empty();
        cfg.allow.push(AllowEntry {
            id: "allow-counted".to_string(),
            kind: FindingKind::Panic,
            family: Some("unwrap".to_string()),
            path: Some(PathBuf::from("src/lib.rs")),
            glob: None,
            owner: "core".to_string(),
            classification: "baseline_debt".to_string(),
            reason: "Generated baseline debt.".to_string(),
            evidence: Vec::new(),
            links: Vec::new(),
            occurrence_limit: Some(3),
            lifecycle: Lifecycle {
                created: Some("2026-05-26".to_string()),
                review_after: None,
                expires: Some("2026-08-01".to_string()),
            },
            selector: Selector {
                ast_kind: Some("method_call".to_string()),
                callee: Some("unwrap".to_string()),
                ..Selector::default()
            },
            last_seen: None,
        });

        let rendered = render_policy(&cfg);
        assert!(rendered.contains("occurrence_limit = 3"));
        let reparsed = parse_policy(&rendered)
            .unwrap_or_else(|err| std::panic::panic_any(format!("rendered policy parses: {err}")));
        assert_eq!(
            reparsed
                .allow
                .first()
                .and_then(|entry| entry.occurrence_limit),
            Some(3)
        );
    }

    #[test]
    fn rejects_zero_occurrence_limit() {
        let err = parse_err(
            r#"
                policy = "cargo-allow"
                [[allow]]
                id = "allow-zero"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "baseline_debt"
                reason = "Generated baseline debt."
                occurrence_limit = 0
                created = "2026-05-26"
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
        );

        assert!(
            err.to_string()
                .contains("occurrence_limit must be greater than zero")
        );
    }

    #[test]
    fn rejects_lifecycle_dates_before_created() {
        let err = parse_err(
            r#"
                policy = "cargo-allow"
                [[allow]]
                id = "bad-order"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "test"
                reason = "fixture"
                created = "2026-08-01"
                review_after = "2026-07-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
        );

        assert!(err.contains("review_after must not be before created"));
    }

    #[test]
    fn rejects_invalid_glob_scope() {
        let err = parse_err(
            r#"
                policy = "cargo-allow"
                [[allow]]
                id = "bad-glob"
                kind = "non_rust_file"
                glob = "../scripts/**"
                owner = "core"
                classification = "test"
                reason = "fixture"
                expires = "2026-08-01"
                [allow.selector]
                glob = "../scripts/**"
            "#,
        );

        assert!(err.contains("parent directory"));
    }

    #[test]
    fn rejects_line_only_selector() {
        let err = parse_err(
            r#"
                policy = "cargo-allow"
                [[allow]]
                id = "line-only"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "test"
                reason = "fixture"
                expires = "2026-08-01"
                [allow.selector]
                line_hint = 12
            "#,
        );

        assert!(err.contains("selector must include structural identity"));
    }

    #[test]
    fn rejects_baseline_debt_without_short_expiry() {
        let err = parse_err(
            r#"
                policy = "cargo-allow"
                [[allow]]
                id = "baseline-too-long"
                kind = "panic"
                path = "src/lib.rs"
                owner = "unowned"
                classification = "baseline_debt"
                reason = "Generated by cargo-allow propose; requires human review."
                created = "2026-05-26"
                expires = "2027-05-26"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
        );

        assert!(err.contains("baseline_debt expires must be within"));
    }

    fn unique_test_dir(slug: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("cargo-allow-policy-{slug}-{}", std::process::id()));
        remove_test_dir(path.clone());
        path
    }

    fn remove_test_dir(path: PathBuf) {
        match fs::remove_dir_all(&path) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => std::panic::panic_any(format!(
                "failed to remove test dir {}: {err}",
                path.display()
            )),
        }
    }

    fn parse_err(input: &str) -> String {
        match parse_policy(input) {
            Ok(_) => std::panic::panic_any("expected policy parse failure"),
            Err(err) => err.to_string(),
        }
    }
}
