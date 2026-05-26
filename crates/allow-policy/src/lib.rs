use allow_core::{
    AllowConfig, AllowEntry, CargoAllowError, CargoAllowResult, FindingKind, LastSeen, Lifecycle,
    Requirements, Selector, WorkspaceConfig,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Value {
    String(String),
    Bool(bool),
    Array(Vec<String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Root,
    Workspace,
    Requirements,
    UnsafeRequirements,
    Allow(usize),
    AllowSelector(usize),
    AllowLastSeen(usize),
}

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
    let mut root: BTreeMap<String, Value> = BTreeMap::new();
    let mut workspace: BTreeMap<String, Value> = BTreeMap::new();
    let mut req: BTreeMap<String, Value> = BTreeMap::new();
    let mut unsafe_req: BTreeMap<String, Value> = BTreeMap::new();
    let mut allow_maps: Vec<BTreeMap<String, Value>> = Vec::new();
    let mut selectors: Vec<BTreeMap<String, Value>> = Vec::new();
    let mut last_seen: Vec<BTreeMap<String, Value>> = Vec::new();
    let mut section = Section::Root;

    for (idx, raw_line) in input.lines().enumerate() {
        let line = strip_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        if line == "[[allow]]" {
            allow_maps.push(BTreeMap::new());
            selectors.push(BTreeMap::new());
            last_seen.push(BTreeMap::new());
            section = Section::Allow(allow_maps.len() - 1);
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            let name = &line[1..line.len() - 1];
            section = match name {
                "workspace" => Section::Workspace,
                "requirements" => Section::Requirements,
                "requirements.unsafe" => Section::UnsafeRequirements,
                "allow.selector" => {
                    Section::AllowSelector(allow_maps.len().checked_sub(1).ok_or_else(|| {
                        CargoAllowError::new(format!(
                            "line {}: [allow.selector] before [[allow]]",
                            idx + 1
                        ))
                    })?)
                }
                "allow.last_seen" => {
                    Section::AllowLastSeen(allow_maps.len().checked_sub(1).ok_or_else(|| {
                        CargoAllowError::new(format!(
                            "line {}: [allow.last_seen] before [[allow]]",
                            idx + 1
                        ))
                    })?)
                }
                other => {
                    return Err(CargoAllowError::new(format!(
                        "line {}: unsupported section [{other}]",
                        idx + 1
                    )));
                }
            };
            continue;
        }
        let (key, value) = parse_assignment(line).ok_or_else(|| {
            CargoAllowError::new(format!("line {}: expected key = value", idx + 1))
        })?;
        match section {
            Section::Root => {
                root.insert(key, value);
            }
            Section::Workspace => {
                workspace.insert(key, value);
            }
            Section::Requirements => {
                req.insert(key, value);
            }
            Section::UnsafeRequirements => {
                unsafe_req.insert(key, value);
            }
            Section::Allow(i) => {
                allow_maps[i].insert(key, value);
            }
            Section::AllowSelector(i) => {
                selectors[i].insert(key, value);
            }
            Section::AllowLastSeen(i) => {
                last_seen[i].insert(key, value);
            }
        }
    }

    let mut cfg = AllowConfig {
        schema_version: get_string(&root, "schema_version").unwrap_or_else(|| "0.1".to_string()),
        policy: get_string(&root, "policy").unwrap_or_else(|| "cargo-allow".to_string()),
        owner: get_string(&root, "owner"),
        status: get_string(&root, "status"),
        workspace: parse_workspace(&workspace),
        requirements: parse_requirements(&req, &unsafe_req),
        allow: Vec::new(),
    };

    for i in 0..allow_maps.len() {
        cfg.allow.push(parse_allow_entry(
            &allow_maps[i],
            &selectors[i],
            &last_seen[i],
            i,
        )?);
    }
    validate_policy(&cfg)?;
    Ok(cfg)
}

pub fn validate_policy(cfg: &AllowConfig) -> CargoAllowResult<()> {
    if cfg.policy != "cargo-allow" {
        return Err(CargoAllowError::new(format!(
            "unsupported policy `{}`",
            cfg.policy
        )));
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
    }
    Ok(())
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
    out.push_str(&format!("owner_required = {}\nreason_required = {}\nclassification_required = {}\nexpires_or_review_after_required = {}\nallow_bare_allow_attributes = {}\nstale_entries_fail = {}\n\n", cfg.requirements.owner_required, cfg.requirements.reason_required, cfg.requirements.classification_required, cfg.requirements.expires_or_review_after_required, cfg.requirements.allow_bare_allow_attributes, cfg.requirements.stale_entries_fail));
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

fn parse_workspace(map: &BTreeMap<String, Value>) -> WorkspaceConfig {
    let mut w = WorkspaceConfig::default();
    if let Some(v) = get_string(map, "root") {
        w.root = v;
    }
    if let Some(v) = get_string(map, "inventory") {
        w.inventory = v;
    }
    if let Some(v) = get_string(map, "default_mode") {
        w.default_mode = v;
    }
    if let Some(v) = get_array(map, "ignored") {
        w.ignored = v;
    }
    if let Some(v) = get_array(map, "generated") {
        w.generated = v;
    }
    w
}

fn parse_requirements(
    map: &BTreeMap<String, Value>,
    unsafe_map: &BTreeMap<String, Value>,
) -> Requirements {
    let mut r = Requirements::default();
    if let Some(v) = get_bool(map, "owner_required") {
        r.owner_required = v;
    }
    if let Some(v) = get_bool(map, "reason_required") {
        r.reason_required = v;
    }
    if let Some(v) = get_bool(map, "classification_required") {
        r.classification_required = v;
    }
    if let Some(v) = get_bool(map, "expires_or_review_after_required") {
        r.expires_or_review_after_required = v;
    }
    if let Some(v) = get_bool(map, "allow_bare_allow_attributes") {
        r.allow_bare_allow_attributes = v;
    }
    if let Some(v) = get_bool(map, "stale_entries_fail") {
        r.stale_entries_fail = v;
    }
    if let Some(v) = get_bool(unsafe_map, "evidence_required") {
        r.unsafe_evidence_required = v;
    }
    if let Some(v) = get_bool(unsafe_map, "safety_comment_required") {
        r.unsafe_safety_comment_required = v;
    }
    r
}

fn parse_allow_entry(
    map: &BTreeMap<String, Value>,
    selector_map: &BTreeMap<String, Value>,
    last_map: &BTreeMap<String, Value>,
    index: usize,
) -> CargoAllowResult<AllowEntry> {
    let id = get_string(map, "id").unwrap_or_else(|| format!("allow-{:04}", index + 1));
    let kind_text = get_string(map, "kind")
        .ok_or_else(|| CargoAllowError::new(format!("{id} missing kind")))?;
    let kind = FindingKind::from_str(&kind_text)?;
    let path = get_string(map, "path").map(PathBuf::from);
    let glob = get_string(map, "glob");
    let selector = Selector {
        ast_kind: get_string(selector_map, "ast_kind").or_else(|| get_string(selector_map, "kind")),
        container: get_string(selector_map, "container"),
        callee: get_string(selector_map, "callee"),
        macro_name: get_string(selector_map, "macro_name")
            .or_else(|| get_string(selector_map, "macro")),
        lint: get_string(selector_map, "lint"),
        symbol: get_string(selector_map, "symbol"),
        receiver_fingerprint: get_string(selector_map, "receiver_fingerprint"),
        target_fingerprint: get_string(selector_map, "target_fingerprint"),
        normalized_snippet_hash: get_string(selector_map, "normalized_snippet_hash"),
        line_hint: get_u32(selector_map, "line_hint"),
        glob: get_string(selector_map, "glob"),
    };
    let last_seen = match (get_u32(last_map, "line"), get_u32(last_map, "column")) {
        (Some(line), Some(column)) => Some(LastSeen { line, column }),
        _ => None,
    };
    Ok(AllowEntry {
        id,
        kind,
        family: get_string(map, "family"),
        path,
        glob,
        owner: get_string(map, "owner").unwrap_or_default(),
        classification: get_string(map, "classification").unwrap_or_default(),
        reason: get_string(map, "reason")
            .or_else(|| get_string(map, "explanation"))
            .unwrap_or_default(),
        evidence: get_array(map, "evidence")
            .or_else(|| get_array(map, "covered_by"))
            .unwrap_or_default(),
        links: get_array(map, "links").unwrap_or_default(),
        lifecycle: Lifecycle {
            created: get_string(map, "created"),
            review_after: get_string(map, "review_after"),
            expires: get_string(map, "expires"),
        },
        selector,
        last_seen,
    })
}

fn parse_assignment(line: &str) -> Option<(String, Value)> {
    let mut parts = line.splitn(2, '=');
    let key = parts.next()?.trim().to_string();
    let value = parse_value(parts.next()?.trim())?;
    Some((key, value))
}

fn parse_value(input: &str) -> Option<Value> {
    let input = input.trim();
    if input.starts_with('"') && input.ends_with('"') && input.len() >= 2 {
        return Some(Value::String(
            input[1..input.len() - 1]
                .replace("\\\"", "\"")
                .replace("\\\\", "\\"),
        ));
    }
    if input == "true" {
        return Some(Value::Bool(true));
    }
    if input == "false" {
        return Some(Value::Bool(false));
    }
    if input.starts_with('[') && input.ends_with(']') {
        let inner = &input[1..input.len() - 1];
        let mut arr = Vec::new();
        for part in split_array(inner) {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            if part.starts_with('"') && part.ends_with('"') && part.len() >= 2 {
                arr.push(
                    part[1..part.len() - 1]
                        .replace("\\\"", "\"")
                        .replace("\\\\", "\\"),
                );
            } else {
                arr.push(part.to_string());
            }
        }
        return Some(Value::Array(arr));
    }
    Some(Value::String(input.to_string()))
}

fn split_array(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut escaped = false;
    for ch in input.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            current.push(ch);
            escaped = true;
            continue;
        }
        if ch == '"' {
            in_string = !in_string;
            current.push(ch);
            continue;
        }
        if ch == ',' && !in_string {
            out.push(current.trim().to_string());
            current.clear();
        } else {
            current.push(ch);
        }
    }
    if !current.trim().is_empty() {
        out.push(current.trim().to_string());
    }
    out
}

fn strip_comment(input: &str) -> &str {
    let mut in_string = false;
    let mut escaped = false;
    for (idx, ch) in input.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' {
            in_string = !in_string;
            continue;
        }
        if ch == '#' && !in_string {
            return &input[..idx];
        }
    }
    input
}

fn get_string(map: &BTreeMap<String, Value>, key: &str) -> Option<String> {
    match map.get(key) {
        Some(Value::String(v)) => Some(v.clone()),
        Some(Value::Bool(v)) => Some(v.to_string()),
        Some(Value::Array(_)) | None => None,
    }
}

fn get_array(map: &BTreeMap<String, Value>, key: &str) -> Option<Vec<String>> {
    match map.get(key) {
        Some(Value::Array(v)) => Some(v.clone()),
        Some(Value::String(v)) => Some(vec![v.clone()]),
        _ => None,
    }
}

fn get_bool(map: &BTreeMap<String, Value>, key: &str) -> Option<bool> {
    match map.get(key) {
        Some(Value::Bool(v)) => Some(*v),
        Some(Value::String(v)) => v.parse().ok(),
        _ => None,
    }
}

fn get_u32(map: &BTreeMap<String, Value>, key: &str) -> Option<u32> {
    get_string(map, key).and_then(|v| v.parse().ok())
}

fn escape_toml(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
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
        assert_eq!(cfg.allow[0].selector.callee.as_deref(), Some("unwrap"));
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
}
