use allow_core::{
    AllowConfig, AllowEntry, CargoAllowError, CargoAllowResult, FindingKind, SimpleDate,
    WorkspaceConfig, normalize_path,
};
use std::collections::BTreeSet;
use std::path::Path;

const BASELINE_DEBT_MAX_DAYS: i64 = 120;
const SUPPORTED_SCHEMA_VERSION: &str = "0.1";

pub fn validate_policy(cfg: &AllowConfig) -> CargoAllowResult<()> {
    if cfg.schema_version.trim().is_empty() {
        return Err(CargoAllowError::new(
            "policy schema_version must not be empty",
        ));
    }
    if cfg.schema_version != SUPPORTED_SCHEMA_VERSION {
        return Err(CargoAllowError::new(format!(
            "unsupported policy schema_version `{}`",
            cfg.schema_version
        )));
    }
    if cfg.policy != "cargo-allow" {
        return Err(CargoAllowError::new(format!(
            "unsupported policy `{}`",
            cfg.policy
        )));
    }
    if cfg
        .owner
        .as_deref()
        .is_some_and(|owner| owner.trim().is_empty())
    {
        return Err(CargoAllowError::new("policy owner must not be empty"));
    }
    if cfg
        .status
        .as_deref()
        .is_some_and(|status| status.trim().is_empty())
    {
        return Err(CargoAllowError::new("policy status must not be empty"));
    }
    if let Some(status) = &cfg.status {
        if !matches!(status.as_str(), "active" | "advisory") {
            return Err(CargoAllowError::new(format!(
                "unsupported policy status `{status}`"
            )));
        }
    }
    validate_workspace(&cfg.workspace)?;
    for pattern in &cfg.workspace.ignored {
        validate_glob("source-tree ignored glob", pattern)?;
    }
    for pattern in &cfg.workspace.generated {
        validate_glob("source-tree generated glob", pattern)?;
    }
    let mut ids = BTreeSet::new();
    for entry in &cfg.allow {
        validate_allow_id(&entry.id)?;
        if entry
            .family
            .as_deref()
            .is_some_and(|family| family.trim().is_empty())
        {
            return Err(CargoAllowError::new(format!(
                "{} family must not be empty",
                entry.id
            )));
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
        validate_scope_consistency(entry)?;
        validate_selector(entry)?;
        validate_source_hints(entry)?;
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
        validate_non_empty_values(&entry.id, "evidence", &entry.evidence)?;
        validate_non_empty_values(&entry.id, "link", &entry.links)?;
        if cfg.requirements.expires_or_review_after_required && !has_real_lifecycle_review(entry) {
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
        if cfg.requirements.evidence_required && entry.evidence.is_empty() {
            return Err(CargoAllowError::new(format!(
                "{} missing evidence",
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

fn validate_allow_id(id: &str) -> CargoAllowResult<()> {
    if id.trim().is_empty() {
        return Err(CargoAllowError::new("allow entry has empty id"));
    }
    if id.trim() != id {
        return Err(CargoAllowError::new(format!(
            "allow id `{id}` must not have leading or trailing whitespace"
        )));
    }
    if !id
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
    {
        return Err(CargoAllowError::new(format!(
            "allow id `{id}` may contain only ASCII letters, digits, hyphen, or underscore"
        )));
    }
    Ok(())
}

fn validate_workspace(workspace: &WorkspaceConfig) -> CargoAllowResult<()> {
    validate_path_scope("workspace root", Path::new(&workspace.root))?;
    if workspace.inventory.trim().is_empty() {
        return Err(CargoAllowError::new(
            "workspace inventory must not be empty",
        ));
    }
    if workspace.inventory != "git-tracked" {
        return Err(CargoAllowError::new(format!(
            "unsupported workspace inventory `{}`",
            workspace.inventory
        )));
    }
    if workspace.default_mode.trim().is_empty() {
        return Err(CargoAllowError::new(
            "workspace default_mode must not be empty",
        ));
    }
    if !matches!(
        workspace.default_mode.as_str(),
        "audit" | "no-new" | "strict" | "release"
    ) {
        return Err(CargoAllowError::new(format!(
            "unsupported workspace default_mode `{}`",
            workspace.default_mode
        )));
    }
    Ok(())
}

fn validate_non_empty_values(id: &str, label: &str, values: &[String]) -> CargoAllowResult<()> {
    for (index, value) in values.iter().enumerate() {
        if value.trim().is_empty() {
            return Err(CargoAllowError::new(format!(
                "{id} {label} entry {} must not be empty",
                index + 1
            )));
        }
    }
    Ok(())
}

fn has_real_lifecycle_review(entry: &AllowEntry) -> bool {
    let has_review_after = entry
        .lifecycle
        .review_after
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    let has_expiry = entry
        .lifecycle
        .expires
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty() && value != "never");
    has_review_after || has_expiry
}

pub(crate) fn validate_path_scope(id: &str, path: &Path) -> CargoAllowResult<()> {
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

fn validate_scope_consistency(entry: &AllowEntry) -> CargoAllowResult<()> {
    if entry.path.is_some() && entry.glob.is_some() {
        return Err(CargoAllowError::new(format!(
            "{} must not define both path and glob",
            entry.id
        )));
    }
    if let (Some(path), Some(selector_glob)) = (&entry.path, &entry.selector.glob) {
        let path = normalize_path(path);
        if selector_glob != &path {
            return Err(CargoAllowError::new(format!(
                "{} selector glob `{selector_glob}` must match path `{path}` or omit one scope",
                entry.id
            )));
        }
    }
    if let (Some(glob), Some(selector_glob)) = (&entry.glob, &entry.selector.glob) {
        if selector_glob != glob {
            return Err(CargoAllowError::new(format!(
                "{} selector glob `{selector_glob}` must match glob `{glob}` or omit one scope",
                entry.id
            )));
        }
    }
    Ok(())
}

fn validate_selector(entry: &AllowEntry) -> CargoAllowResult<()> {
    let selector = &entry.selector;
    for (field, value) in [
        ("ast_kind", selector.ast_kind.as_deref()),
        ("container", selector.container.as_deref()),
        ("callee", selector.callee.as_deref()),
        ("macro_name", selector.macro_name.as_deref()),
        ("lint", selector.lint.as_deref()),
        ("symbol", selector.symbol.as_deref()),
        (
            "receiver_fingerprint",
            selector.receiver_fingerprint.as_deref(),
        ),
        ("target_fingerprint", selector.target_fingerprint.as_deref()),
        (
            "normalized_snippet_hash",
            selector.normalized_snippet_hash.as_deref(),
        ),
    ] {
        if value.is_some_and(|text| text.trim().is_empty()) {
            return Err(CargoAllowError::new(format!(
                "{} selector {field} must not be empty",
                entry.id
            )));
        }
    }
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

fn validate_source_hints(entry: &AllowEntry) -> CargoAllowResult<()> {
    if entry.selector.line_hint == Some(0) {
        return Err(CargoAllowError::new(format!(
            "{} line_hint must be greater than zero",
            entry.id
        )));
    }
    if let Some(last_seen) = &entry.last_seen {
        if last_seen.line == 0 {
            return Err(CargoAllowError::new(format!(
                "{} last_seen line must be greater than zero",
                entry.id
            )));
        }
        if last_seen.column == 0 {
            return Err(CargoAllowError::new(format!(
                "{} last_seen column must be greater than zero",
                entry.id
            )));
        }
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
