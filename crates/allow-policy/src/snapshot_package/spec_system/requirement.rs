//! Requirement domain DTOs (#2584-B).

use serde::{Deserialize, Serialize};

pub const REQUIREMENT_BLOCK_SCHEMA_VERSION: &str = "1.0";
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RequirementId(pub String);

impl RequirementId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Normative status of a requirement.
///
/// This deliberately does not contain implementation states. A requirement may
/// remain accepted while different implementation claims are planned,
/// implemented, unsupported, or stale independently.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequirementStatus {
    Draft,
    Accepted,
    Deferred,
    Superseded,
    Rejected,
    RemovedWithReplacement,
}

impl RequirementStatus {
    pub fn allows_implementation_claim(self) -> bool {
        matches!(self, Self::Accepted)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequirementClaimClass {
    RuntimeBehavior,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpecRequirement {
    pub id: RequirementId,
    pub local_id: String,
    pub generation: u32,
    pub status: RequirementStatus,
    pub statement: String,
    pub claim_class: RequirementClaimClass,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequirementSource {
    #[serde(default)]
    pub path: Option<String>,
    pub start_line: u32,
    pub end_line: u32,
    pub content_identity: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequirementGraph {
    pub schema_version: String,
    pub document_id: String,
    pub source: RequirementSource,
    pub requirements: Vec<SpecRequirement>,
}

use allow_core::{
    CargoAllowError, CargoAllowErrorKind, CargoAllowResult, normalize_path, stable_hash_hex,
};
use std::collections::BTreeSet;
use std::path::Path;

const REQUIREMENT_FENCE: &str = "```toml cargo-allow-requirements";
const FENCE_END: &str = "```";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRequirementBlock {
    schema_version: String,
    requirement: Vec<RawRequirement>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRequirement {
    id: String,
    generation: u32,
    #[serde(alias = "lifecycle")]
    status: RequirementStatus,
    statement: String,
    claim_class: RequirementClaimClass,
}

pub fn parse_requirement_blocks(markdown: &str) -> CargoAllowResult<RequirementGraph> {
    parse_requirement_blocks_at(None, markdown)
}

pub fn parse_requirement_blocks_at(
    path: Option<&Path>,
    markdown: &str,
) -> CargoAllowResult<RequirementGraph> {
    let document_id = parse_document_id(path, markdown)?;
    let block = find_single_requirement_block(path, markdown)?;
    let raw = toml::from_str::<RawRequirementBlock>(&block.body).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidPolicy,
            format!("failed to parse cargo-allow requirement block TOML: {error}"),
        )
        .with_toml_span(path, &block.body, error.span())
    })?;

    if raw.schema_version != REQUIREMENT_BLOCK_SCHEMA_VERSION {
        return Err(invalid_requirement_source(
            path,
            format!(
                "requirement block schema_version must be {REQUIREMENT_BLOCK_SCHEMA_VERSION}, found {}",
                raw.schema_version
            ),
        ));
    }
    if raw.requirement.is_empty() {
        return Err(invalid_requirement_source(
            path,
            "requirement block must contain at least one [[requirement]] entry",
        ));
    }

    let mut seen = BTreeSet::new();
    let mut requirements = Vec::with_capacity(raw.requirement.len());
    for requirement in raw.requirement {
        let local_id = requirement.id.trim();
        if local_id.is_empty() || local_id.contains('#') {
            return Err(invalid_requirement_source(
                path,
                "requirement id must be non-empty and local to its document",
            ));
        }
        if requirement.generation == 0 {
            return Err(invalid_requirement_source(
                path,
                format!("requirement {local_id} generation must be greater than zero"),
            ));
        }
        if requirement.statement.trim().is_empty() {
            return Err(invalid_requirement_source(
                path,
                format!("requirement {local_id} statement must not be empty"),
            ));
        }

        let id = RequirementId(format!("{document_id}#{local_id}"));
        if !seen.insert(id.clone()) {
            return Err(invalid_requirement_source(
                path,
                format!("duplicate requirement id {}", id.as_str()),
            ));
        }
        requirements.push(SpecRequirement {
            id,
            local_id: local_id.to_string(),
            generation: requirement.generation,
            status: requirement.status,
            statement: requirement.statement,
            claim_class: requirement.claim_class,
        });
    }

    Ok(RequirementGraph {
        schema_version: raw.schema_version,
        document_id,
        source: RequirementSource {
            path: path.map(|value| normalize_path(value.display().to_string())),
            start_line: block.start_line,
            end_line: block.end_line,
            content_identity: stable_hash_hex(&block.body),
        },
        requirements,
    })
}

struct RequirementBlock {
    body: String,
    start_line: u32,
    end_line: u32,
}

fn parse_document_id(path: Option<&Path>, markdown: &str) -> CargoAllowResult<String> {
    let mut lines = markdown.lines();
    if lines.next().map(str::trim) != Some("---") {
        return Err(invalid_requirement_source(
            path,
            "spec document must start with YAML front matter",
        ));
    }

    for line in lines {
        let trimmed = line.trim();
        if trimmed == "---" {
            break;
        }
        if let Some(value) = trimmed.strip_prefix("id:") {
            let id = value.trim();
            if !id.is_empty() {
                return Ok(id.to_string());
            }
        }
    }

    Err(invalid_requirement_source(
        path,
        "spec document front matter must contain a non-empty id",
    ))
}

fn find_single_requirement_block(
    path: Option<&Path>,
    markdown: &str,
) -> CargoAllowResult<RequirementBlock> {
    let mut lines = markdown.lines().enumerate();
    let mut blocks = Vec::new();

    while let Some((line_index, line)) = lines.next() {
        if line.trim() != REQUIREMENT_FENCE {
            continue;
        }

        let opening_line = line_index + 1;
        let mut body = Vec::new();
        let mut closing_line = None;
        for (body_index, body_line) in lines.by_ref() {
            if body_line.trim() == FENCE_END {
                closing_line = Some(body_index + 1);
                break;
            }
            body.push(body_line);
        }

        let Some(end_line) = closing_line else {
            return Err(invalid_requirement_source(
                path,
                format!("requirement block opened on line {opening_line} is not closed"),
            ));
        };
        blocks.push(RequirementBlock {
            body: body.join("\n"),
            start_line: u32::try_from(opening_line).unwrap_or(u32::MAX),
            end_line: u32::try_from(end_line).unwrap_or(u32::MAX),
        });
    }

    match blocks.len() {
        1 => blocks.pop().ok_or_else(|| {
            invalid_requirement_source(path, "requirement block disappeared during parsing")
        }),
        0 => Err(invalid_requirement_source(
            path,
            format!("spec document must contain exactly one {REQUIREMENT_FENCE} block"),
        )),
        count => Err(invalid_requirement_source(
            path,
            format!("spec document contains {count} requirement blocks; exactly one is allowed"),
        )),
    }
}

fn invalid_requirement_source(path: Option<&Path>, message: impl Into<String>) -> CargoAllowError {
    let message = match path {
        Some(path) => format!("{}: {}", path.display(), message.into()),
        None => message.into(),
    };
    CargoAllowError::with_kind(CargoAllowErrorKind::InvalidPolicy, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC: &str = r#"---
id: CARGO-ALLOW-SPEC-0009
kind: spec
---

# Spec

```toml cargo-allow-requirements
schema_version = "1.0"

[[requirement]]
id = "spec-only-runtime-promotion"
generation = 1
status = "accepted"
statement = "A spec-only slice cannot promote runtime state without closure."
claim_class = "runtime_behavior"
```
"#;

    #[test]
    fn parses_exact_requirement_fence() -> Result<(), String> {
        let graph = parse_requirement_blocks_at(Some(Path::new("docs/spec.md")), SPEC)
            .map_err(|error| error.to_string())?;

        assert_eq!(graph.document_id, "CARGO-ALLOW-SPEC-0009");
        assert_eq!(graph.requirements.len(), 1);
        assert_eq!(
            graph.requirements[0].id.as_str(),
            "CARGO-ALLOW-SPEC-0009#spec-only-runtime-promotion"
        );
        assert_eq!(graph.requirements[0].generation, 1);
        assert_eq!(graph.requirements[0].status, RequirementStatus::Accepted);
        assert_eq!(graph.source.path.as_deref(), Some("docs/spec.md"));
        assert!(graph.source.start_line < graph.source.end_line);
        assert!(graph.source.content_identity.starts_with("fnv1a64:"));
        Ok(())
    }

    #[test]
    fn reads_accepted_legacy_field_without_allowing_implemented_status() -> Result<(), String> {
        let accepted = SPEC.replace("status = \"accepted\"", "lifecycle = \"accepted\"");
        let graph = parse_requirement_blocks(&accepted).map_err(|error| error.to_string())?;
        assert_eq!(graph.requirements[0].status, RequirementStatus::Accepted);

        let implemented = SPEC.replace("status = \"accepted\"", "lifecycle = \"implemented\"");
        assert!(parse_requirement_blocks(&implemented).is_err());
        Ok(())
    }

    #[test]
    fn rejects_missing_requirement_fence() {
        let result = parse_requirement_blocks("---\nid: CARGO-ALLOW-SPEC-0009\n---\n");
        assert!(result.is_err());
    }

    #[test]
    fn rejects_multiple_requirement_fences() {
        let result = parse_requirement_blocks(&format!("{SPEC}\n{SPEC}"));
        assert!(result.is_err());
    }

    #[test]
    fn rejects_unknown_requirement_generation() {
        let result = parse_requirement_blocks(
            &SPEC.replace("schema_version = \"1.0\"", "schema_version = \"2.0\""),
        );
        assert!(result.is_err());
    }
}
