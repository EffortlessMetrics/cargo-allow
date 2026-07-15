use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use std::path::Path;

use super::{RequirementGraph, parse_requirement_blocks_at};

const SYNTHETIC_FRONT_MATTER_LINES: u32 = 3;

pub fn parse_requirement_blocks_for_document(
    document_id: &str,
    markdown: &str,
) -> CargoAllowResult<RequirementGraph> {
    parse_requirement_blocks_for_document_at(document_id, None, markdown)
}

pub fn parse_requirement_blocks_for_document_at(
    document_id: &str,
    path: Option<&Path>,
    markdown: &str,
) -> CargoAllowResult<RequirementGraph> {
    let document_id = document_id.trim();
    if document_id.is_empty()
        || document_id.contains('#')
        || document_id.contains('\n')
        || document_id.contains('\r')
    {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidPolicy,
            "requirement document id must be non-empty and contain no '#', newline, or carriage return",
        ));
    }

    let synthetic = format!("---\nid: {document_id}\n---\n{markdown}");
    let mut graph = parse_requirement_blocks_at(path, &synthetic)?;
    graph.source.start_line = graph
        .source
        .start_line
        .saturating_sub(SYNTHETIC_FRONT_MATTER_LINES);
    graph.source.end_line = graph
        .source
        .end_line
        .saturating_sub(SYNTHETIC_FRONT_MATTER_LINES);
    Ok(graph)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RIPR_SPEC: &str = r#"# RIPR-SPEC-0124: Runtime Promotion

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
    fn parses_requirement_fence_with_dialect_document_id() -> Result<(), String> {
        let graph = parse_requirement_blocks_for_document("RIPR-SPEC-0124", RIPR_SPEC)
            .map_err(|error| error.to_string())?;
        let requirement = graph
            .requirements
            .first()
            .ok_or_else(|| "expected one parsed requirement".to_string())?;

        assert_eq!(
            requirement.id.as_str(),
            "RIPR-SPEC-0124#spec-only-runtime-promotion"
        );
        assert_eq!(graph.source.start_line, 3);
        Ok(())
    }

    #[test]
    fn rejects_document_id_injection() {
        assert!(parse_requirement_blocks_for_document("bad\nid", RIPR_SPEC).is_err());
    }
}
