use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult, normalize_path, stable_hash_hex};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;

pub const REQUIREMENT_BLOCK_SCHEMA_VERSION: &str = "1.0";
const REQUIREMENT_FENCE: &str = "```toml cargo-allow-requirements";
const FENCE_END: &str = "```";

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RequirementId(pub String);

impl RequirementId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequirementLifecycle {
    Accepted,
    Implemented,
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
    pub lifecycle: RequirementLifecycle,
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
    lifecycle: RequirementLifecycle,
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
            lifecycle: requirement.lifecycle,
            statement: requirement.statement,
            claim_class: requirement.claim_class,
        });
    }

    Ok(RequirementGraph {
        schema_version: raw.schema_version,
        document_id,
        source: RequirementSource {
            path: path.map(|value| normalize_path(&value.display().to_string())),
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
    let lines = markdown.lines().collect::<Vec<_>>();
    let mut blocks = Vec::new();
    let mut index = 0usize;

    while index < lines.len() {
        if lines[index].trim() != REQUIREMENT_FENCE {
            index += 1;
            continue;
        }

        let opening_line = index + 1;
        let body_start = index + 1;
        let Some(relative_end) = lines[body_start..]
            .iter()
            .position(|line| line.trim() == FENCE_END)
        else {
            return Err(invalid_requirement_source(
                path,
                format!("requirement block opened on line {opening_line} is not closed"),
            ));
        };
        let body_end = body_start + relative_end;
        blocks.push(RequirementBlock {
            body: lines[body_start..body_end].join("\n"),
            start_line: u32::try_from(opening_line).unwrap_or(u32::MAX),
            end_line: u32::try_from(body_end + 1).unwrap_or(u32::MAX),
        });
        index = body_end + 1;
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
lifecycle = "accepted"
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
        assert_eq!(graph.requirements[0].lifecycle, RequirementLifecycle::Accepted);
        assert_eq!(graph.source.path.as_deref(), Some("docs/spec.md"));
        assert!(graph.source.start_line < graph.source.end_line);
        assert!(graph.source.content_identity.starts_with("fnv1a64:"));
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
        let result = parse_requirement_blocks(&SPEC.replace(
            "schema_version = \"1.0\"",
            "schema_version = \"2.0\"",
        ));
        assert!(result.is_err());
    }
}
