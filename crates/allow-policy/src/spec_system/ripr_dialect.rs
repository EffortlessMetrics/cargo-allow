use allow_core::{
    CargoAllowError, CargoAllowErrorKind, CargoAllowResult, normalize_path, stable_hash_hex,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

use super::{
    ImplementationSliceV1, RequirementGraph, parse_implementation_slice_at,
    parse_requirement_blocks_for_document_at,
};

pub const RIPR_SPEC_DIALECT_ID: &str = "cargo-allow.ripr-spec.v2";
const REQUIREMENT_FENCE: &str = "```toml cargo-allow-requirements";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiprSpecStatus {
    Draft,
    Planned,
    Proposed,
    Accepted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiprSpecSourceClass {
    LegacyDocumentLevel,
    V2Requirements,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RiprSpecLinks {
    pub proposals: Vec<String>,
    pub adrs: Vec<String>,
    pub plans: Vec<String>,
    pub issues: Vec<String>,
    pub prs: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RiprSpecSource {
    #[serde(default)]
    pub path: Option<String>,
    pub title_line: u32,
    pub status_line: u32,
    pub owner_line: u32,
    pub created_line: u32,
    pub content_identity: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RiprSpecDocument {
    pub dialect_id: String,
    pub id: String,
    pub title: String,
    pub status: RiprSpecStatus,
    pub owner: String,
    pub created: String,
    pub links: RiprSpecLinks,
    pub support_tier_impact: Vec<String>,
    pub policy_impact: Vec<String>,
    pub source_class: RiprSpecSourceClass,
    #[serde(default)]
    pub requirements: Option<RequirementGraph>,
    pub source: RiprSpecSource,
}

pub fn parse_ripr_spec(markdown: &str) -> CargoAllowResult<RiprSpecDocument> {
    parse_ripr_spec_at(None, markdown)
}

pub fn parse_ripr_spec_at(
    path: Option<&Path>,
    markdown: &str,
) -> CargoAllowResult<RiprSpecDocument> {
    let normalized = markdown.replace("\r\n", "\n");
    let lines = normalized.lines().collect::<Vec<_>>();
    let title_line = lines
        .first()
        .ok_or_else(|| invalid_ripr_spec(path, "RIPR spec document is empty"))?;
    let (id, title) = parse_title(path, title_line)?;
    let (status, status_line) = parse_named_value(path, &lines, "Status:")?;
    let (owner, owner_line) = parse_named_value(path, &lines, "Owner:")?;
    let (created, created_line) = parse_named_value(path, &lines, "Created:")?;
    let status = parse_status(path, &status)?;

    let requirement_block_count = lines
        .iter()
        .filter(|line| line.trim() == REQUIREMENT_FENCE)
        .count();
    let (source_class, requirements) = match requirement_block_count {
        0 => (RiprSpecSourceClass::LegacyDocumentLevel, None),
        1 => (
            RiprSpecSourceClass::V2Requirements,
            Some(parse_requirement_blocks_for_document_at(
                &id,
                path,
                &normalized,
            )?),
        ),
        count => {
            return Err(invalid_ripr_spec(
                path,
                format!("RIPR spec contains {count} requirement blocks; at most one is allowed"),
            ));
        }
    };

    Ok(RiprSpecDocument {
        dialect_id: RIPR_SPEC_DIALECT_ID.to_string(),
        id,
        title,
        status,
        owner,
        created,
        links: RiprSpecLinks {
            proposals: parse_list_section(&lines, "Linked proposal:"),
            adrs: parse_list_section(&lines, "Linked ADRs:"),
            plans: parse_list_section(&lines, "Linked plan:"),
            issues: parse_list_section(&lines, "Linked issues:"),
            prs: parse_list_section(&lines, "Linked PRs:"),
        },
        support_tier_impact: parse_list_section(&lines, "Support-tier impact:"),
        policy_impact: parse_list_section(&lines, "Policy impact:"),
        source_class,
        requirements,
        source: RiprSpecSource {
            path: path.map(|value| normalize_path(value.display().to_string())),
            title_line: 1,
            status_line,
            owner_line,
            created_line,
            content_identity: stable_hash_hex(&normalized),
        },
    })
}

pub fn parse_ripr_implementation_slice(input: &str) -> CargoAllowResult<ImplementationSliceV1> {
    parse_ripr_implementation_slice_at(None, input)
}

pub fn parse_ripr_implementation_slice_at(
    path: Option<&Path>,
    input: &str,
) -> CargoAllowResult<ImplementationSliceV1> {
    parse_implementation_slice_at(path, input)
}

fn parse_title(path: Option<&Path>, line: &str) -> CargoAllowResult<(String, String)> {
    let Some(rest) = line.trim().strip_prefix("# ") else {
        return Err(invalid_ripr_spec(
            path,
            "RIPR spec must start with '# RIPR-SPEC-NNNN: Title'",
        ));
    };
    let Some((id, title)) = rest.split_once(':') else {
        return Err(invalid_ripr_spec(
            path,
            "RIPR spec title must separate id and title with ':'",
        ));
    };
    let id = id.trim();
    let title = title.trim();
    if !valid_ripr_spec_id(id) || title.is_empty() {
        return Err(invalid_ripr_spec(
            path,
            "RIPR spec title must contain a valid RIPR-SPEC-NNNN id and non-empty title",
        ));
    }
    Ok((id.to_string(), title.to_string()))
}

fn valid_ripr_spec_id(value: &str) -> bool {
    let Some(number) = value.strip_prefix("RIPR-SPEC-") else {
        return false;
    };
    number.len() == 4 && number.bytes().all(|byte| byte.is_ascii_digit())
}

fn parse_named_value(
    path: Option<&Path>,
    lines: &[&str],
    label: &str,
) -> CargoAllowResult<(String, u32)> {
    for (index, line) in lines.iter().enumerate() {
        if let Some(value) = line.trim().strip_prefix(label) {
            let value = value.trim();
            if value.is_empty() {
                return Err(invalid_ripr_spec(
                    path,
                    format!("RIPR spec {label} value must not be empty"),
                ));
            }
            return Ok((
                value.to_string(),
                u32::try_from(index + 1).unwrap_or(u32::MAX),
            ));
        }
    }
    Err(invalid_ripr_spec(
        path,
        format!("RIPR spec must contain {label}"),
    ))
}

fn parse_status(path: Option<&Path>, value: &str) -> CargoAllowResult<RiprSpecStatus> {
    match value.to_ascii_lowercase().as_str() {
        "draft" => Ok(RiprSpecStatus::Draft),
        "planned" => Ok(RiprSpecStatus::Planned),
        "proposed" => Ok(RiprSpecStatus::Proposed),
        "accepted" => Ok(RiprSpecStatus::Accepted),
        other => Err(invalid_ripr_spec(
            path,
            format!("unsupported RIPR spec status {other}"),
        )),
    }
}

fn parse_list_section(lines: &[&str], label: &str) -> Vec<String> {
    let mut iter = lines.iter();
    if iter.find(|line| line.trim() == label).is_none() {
        return Vec::new();
    }

    let mut values: Vec<String> = Vec::new();
    let mut seen_item = false;
    for line in iter {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if seen_item {
                break;
            }
            continue;
        }
        if trimmed.starts_with("## ") || trimmed.ends_with(':') {
            break;
        }
        if let Some(item) = trimmed.strip_prefix("- ") {
            seen_item = true;
            if item != "None yet" && !item.is_empty() {
                values.push(item.to_string());
            }
            continue;
        }
        if seen_item {
            if let Some(last) = values.last_mut() {
                last.push(' ');
                last.push_str(trimmed);
            }
        } else {
            break;
        }
    }

    values
}

fn invalid_ripr_spec(path: Option<&Path>, message: impl Into<String>) -> CargoAllowError {
    let message = match path {
        Some(path) => format!("{}: {}", path.display(), message.into()),
        None => message.into(),
    };
    CargoAllowError::with_kind(CargoAllowErrorKind::InvalidPolicy, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEGACY: &str = r#"# RIPR-SPEC-0123: Targeted Rust Rerun

Status: accepted

Owner: product / swarm

Created: 2026-07-10

Linked proposal:

- None yet

Linked ADRs:

- None yet

Linked plan:

- [RIPR-PLAN-0062](../../plans/rust-one-shot-evidence-to-repair.md)

Linked issues:

- #1424

Linked PRs:

- #1531

Support-tier impact:

- No tier promotion.

Policy impact:

- Register this spec in policy/doc-artifacts.toml.

## Problem

Legacy document-level behavior.
"#;

    const V2: &str = r#"# RIPR-SPEC-0124: Runtime Promotion

Status: proposed

Owner: product / swarm

Created: 2026-07-15

Linked proposal:

- None yet

Linked ADRs:

- None yet

Linked plan:

- None yet

Linked issues:

- #1672

Linked PRs:

- None yet

Support-tier impact:

- None.

Policy impact:

- Advisory spec-system adoption.

## Normative Requirements

```toml cargo-allow-requirements
schema_version = "1.0"

[[requirement]]
id = "spec-only-runtime-promotion"
generation = 1
lifecycle = "accepted"
statement = "A spec-only slice cannot promote runtime state without closure."
claim_class = "runtime_behavior"
```
"#;

    #[test]
    fn parses_legacy_ripr_spec_without_fabricating_requirements() -> Result<(), String> {
        let document = parse_ripr_spec_at(Some(Path::new("docs/spec.md")), LEGACY)
            .map_err(|error| error.to_string())?;

        assert_eq!(document.id, "RIPR-SPEC-0123");
        assert_eq!(document.title, "Targeted Rust Rerun");
        assert_eq!(document.status, RiprSpecStatus::Accepted);
        assert_eq!(
            document.source_class,
            RiprSpecSourceClass::LegacyDocumentLevel
        );
        assert!(document.requirements.is_none());
        assert_eq!(document.links.issues, vec!["#1424"]);
        Ok(())
    }

    #[test]
    fn parses_v2_ripr_spec_into_shared_requirement_graph() -> Result<(), String> {
        let document = parse_ripr_spec(V2).map_err(|error| error.to_string())?;
        let graph = document
            .requirements
            .as_ref()
            .ok_or_else(|| "expected requirement graph".to_string())?;
        let requirement = graph
            .requirements
            .first()
            .ok_or_else(|| "expected one parsed requirement".to_string())?;

        assert_eq!(document.source_class, RiprSpecSourceClass::V2Requirements);
        assert_eq!(
            requirement.id.as_str(),
            "RIPR-SPEC-0124#spec-only-runtime-promotion"
        );
        assert_eq!(graph.document_id, document.id);
        Ok(())
    }

    #[test]
    fn normalizes_crlf_for_portable_identity() -> Result<(), String> {
        let unix = parse_ripr_spec_at(Some(Path::new("docs/spec.md")), V2)
            .map_err(|error| error.to_string())?;
        let windows_text = V2.replace('\n', "\r\n");
        let windows = parse_ripr_spec_at(Some(Path::new("docs\\spec.md")), &windows_text)
            .map_err(|error| error.to_string())?;

        assert_eq!(
            unix.source.content_identity,
            windows.source.content_identity
        );
        assert_eq!(unix.source.path, windows.source.path);
        assert_eq!(
            unix.requirements
                .as_ref()
                .map(|graph| &graph.source.content_identity),
            windows
                .requirements
                .as_ref()
                .map(|graph| &graph.source.content_identity)
        );
        Ok(())
    }

    #[test]
    fn rejects_multiple_v2_requirement_blocks() {
        let duplicated = format!("{V2}\n{V2}");
        assert!(parse_ripr_spec(&duplicated).is_err());
    }

    #[test]
    fn shared_slice_parser_rejects_mutable_execution_state() {
        let slice = r#"
schema_version = "2.0"
id = "ripr.slice.example.v1"
generation = 1
source_issue = "issue:1672"
design_reference = "RIPR-SPEC-0124#spec-only-runtime-promotion"
change_class = "spec_or_policy_change"
claim_boundary = "No runtime claim."
branch = "main"

[[requirement_delta]]
requirement_id = "RIPR-SPEC-0124#spec-only-runtime-promotion"
requirement_generation = 1
to = "accepted"

[implementation_claim]
status = "outstanding"

[evidence]
state = "outstanding"

[support_claim]
state = "unchanged"
"#;
        assert!(parse_ripr_implementation_slice(slice).is_err());
    }
}
