//! Support tier claim DTOs (#2584-B).

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupportTierRow {
    pub surface: String,
    pub tier: SupportTierLevel,
    pub claim: String,
    pub proof_command: String,
    pub notes: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportTierLevel {
    Stable,
    Stabilizing,
    Experimental,
    Compatibility,
    Advisory,
    NotIncluded,
}

impl SupportTierLevel {
    pub fn requires_proof_command(self) -> bool {
        matches!(self, Self::Stable | Self::Stabilizing)
    }
}

use allow_core::{CargoAllowError, CargoAllowResult};

pub fn validate_support_tier_claims(input: &str) -> CargoAllowResult<Vec<SupportTierRow>> {
    let rows = parse_support_tier_claims(input)?;
    for row in &rows {
        ensure_non_empty("support-tier surface", &row.surface)?;
        ensure_non_empty("support-tier claim", &row.claim)?;
        if row.tier.requires_proof_command() {
            ensure_non_empty(
                &format!("{} support-tier proof command", row.surface),
                &row.proof_command,
            )?;
        }
    }
    Ok(rows)
}

pub fn parse_support_tier_claims(input: &str) -> CargoAllowResult<Vec<SupportTierRow>> {
    let mut lines = input.lines();
    while let Some(line) = lines.next() {
        let Some(header) = table_cells(line) else {
            continue;
        };
        let Some(columns) = claims_columns(&header)? else {
            continue;
        };

        let Some(separator) = lines.next() else {
            return Err(CargoAllowError::new(
                "support-tier claims table missing separator row",
            ));
        };
        if !is_separator_row(separator) {
            return Err(CargoAllowError::new(
                "support-tier claims table separator row is invalid",
            ));
        }

        let mut rows = Vec::new();
        for row_line in lines {
            let Some(cells) = table_cells(row_line) else {
                break;
            };
            if is_separator_cells(&cells) {
                continue;
            }
            if cells.len() <= columns.max_required_index() {
                return Err(CargoAllowError::new(format!(
                    "support-tier claims row has {} cells; expected at least {}",
                    cells.len(),
                    columns.max_required_index() + 1
                )));
            }
            rows.push(parse_claims_row(&cells, columns)?);
        }

        if rows.is_empty() {
            return Err(CargoAllowError::new(
                "support-tier claims table must include at least one claim row",
            ));
        }

        return Ok(rows);
    }

    Err(CargoAllowError::new(
        "support-tier claims table with Surface, Tier, Claim, Proof command or Proof or evidence, and Notes or Limitations columns not found",
    ))
}

#[derive(Debug, Clone, Copy)]
struct ClaimsColumns {
    surface: usize,
    tier: usize,
    claim: usize,
    proof_command: usize,
    notes: Option<usize>,
}

impl ClaimsColumns {
    fn max_required_index(self) -> usize {
        [self.surface, self.tier, self.claim, self.proof_command]
            .into_iter()
            .fold(0, usize::max)
    }
}

fn parse_claims_row(cells: &[String], columns: ClaimsColumns) -> CargoAllowResult<SupportTierRow> {
    let surface = cell(cells, columns.surface, "surface")?;
    let tier = parse_support_tier_level(&cell(cells, columns.tier, "tier")?)?;
    let claim = cell(cells, columns.claim, "claim")?;
    let proof_command = cell(cells, columns.proof_command, "proof command or evidence")?;
    let notes = match columns.notes {
        Some(index) => cell(cells, index, "notes")?,
        None => String::new(),
    };

    Ok(SupportTierRow {
        surface,
        tier,
        claim,
        proof_command,
        notes,
    })
}

fn cell(cells: &[String], index: usize, label: &str) -> CargoAllowResult<String> {
    let Some(value) = cells.get(index) else {
        return Err(CargoAllowError::new(format!(
            "support-tier claims row missing {label} cell"
        )));
    };
    Ok(value.clone())
}

fn parse_support_tier_level(input: &str) -> CargoAllowResult<SupportTierLevel> {
    if input.trim().is_empty() {
        return Err(CargoAllowError::new("support-tier tier must not be empty"));
    }

    match normalize_cell(input).as_str() {
        "stable" => Ok(SupportTierLevel::Stable),
        "stabilizing" => Ok(SupportTierLevel::Stabilizing),
        "experimental" => Ok(SupportTierLevel::Experimental),
        "compatibility" => Ok(SupportTierLevel::Compatibility),
        "advisory" => Ok(SupportTierLevel::Advisory),
        "not included" => Ok(SupportTierLevel::NotIncluded),
        value => Err(CargoAllowError::new(format!(
            "unknown support-tier level {value}"
        ))),
    }
}

fn claims_columns(cells: &[String]) -> CargoAllowResult<Option<ClaimsColumns>> {
    let normalized = cells
        .iter()
        .map(|cell| normalize_cell(cell))
        .collect::<Vec<_>>();
    let surface = column_index(&normalized, "surface");
    let tier = column_index(&normalized, "tier");
    let claim = column_index(&normalized, "claim");
    let proof_command = column_index(&normalized, "proof command")
        .or_else(|| column_index(&normalized, "proof or evidence"));
    let notes =
        column_index(&normalized, "notes").or_else(|| column_index(&normalized, "limitations"));

    let has_claim_marker = surface.is_some() || claim.is_some() || proof_command.is_some();
    if !has_claim_marker {
        return Ok(None);
    }

    let missing = [
        ("Surface", surface),
        ("Tier", tier),
        ("Claim", claim),
        ("Proof command or Proof or evidence", proof_command),
    ]
    .into_iter()
    .filter_map(|(name, index)| if index.is_none() { Some(name) } else { None })
    .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(CargoAllowError::new(format!(
            "support-tier claims table missing required column {}",
            missing.join(", ")
        )));
    }

    let (Some(surface), Some(tier), Some(claim), Some(proof_command)) =
        (surface, tier, claim, proof_command)
    else {
        return Ok(None);
    };

    Ok(Some(ClaimsColumns {
        surface,
        tier,
        claim,
        proof_command,
        notes,
    }))
}

fn column_index(cells: &[String], name: &str) -> Option<usize> {
    cells.iter().position(|cell| cell == name)
}

fn is_separator_row(line: &str) -> bool {
    table_cells(line).is_some_and(|cells| is_separator_cells(&cells))
}

fn is_separator_cells(cells: &[String]) -> bool {
    !cells.is_empty()
        && cells
            .iter()
            .all(|cell| cell.contains('-') && cell.chars().all(is_separator_char))
}

fn is_separator_char(ch: char) -> bool {
    ch == '-' || ch == ':' || ch.is_whitespace()
}

fn table_cells(line: &str) -> Option<Vec<String>> {
    let line = line.trim();
    if !line.starts_with('|') {
        return None;
    }

    let cells = line
        .trim_matches('|')
        .split('|')
        .map(clean_cell)
        .collect::<Vec<_>>();
    if cells.is_empty() { None } else { Some(cells) }
}

fn clean_cell(cell: &str) -> String {
    cell.trim().trim_matches('`').trim().to_string()
}

fn normalize_cell(cell: &str) -> String {
    clean_cell(cell).to_ascii_lowercase()
}

fn ensure_non_empty(label: &str, value: &str) -> CargoAllowResult<()> {
    if value.trim().is_empty() {
        return Err(CargoAllowError::new(format!("{label} must not be empty")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_generation_two_support_vocabulary() -> Result<(), String> {
        let input = r#"| Surface | Tier | Claim | Proof or evidence | Limitations |
| --- | --- | --- | --- | --- |
| cargo-allow | Stable | Published behavior. | cargo-allow check --mode no-new | Published channel. |
| cargo-intent | Experimental | Landed walking skeleton. | cargo-intent identity | Not independently published. |
| Legacy profile | Compatibility | Delegates or fails explicitly. | receipt:compat | Historical surface. |
| Repository extraction | Not included | No extraction authorization. | spec:convergence | Separate decision. |
"#;
        let rows = validate_support_tier_claims(input).map_err(|error| error.to_string())?;
        let expected = [
            ("cargo-allow", SupportTierLevel::Stable),
            ("cargo-intent", SupportTierLevel::Experimental),
            ("Legacy profile", SupportTierLevel::Compatibility),
            ("Repository extraction", SupportTierLevel::NotIncluded),
        ];
        if rows.len() != expected.len() {
            return Err(format!(
                "expected {} support rows, got {}",
                expected.len(),
                rows.len()
            ));
        }
        for (surface, tier) in expected {
            let Some(row) = rows.iter().find(|row| row.surface == surface) else {
                return Err(format!("missing support-tier row {surface}"));
            };
            if row.tier != tier {
                return Err(format!(
                    "support-tier row {surface} expected {tier:?}, got {:?}",
                    row.tier
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn retains_proof_command_header_compatibility() -> Result<(), String> {
        let input = r#"| Surface | Tier | Claim | Proof command | Notes |
| --- | --- | --- | --- | --- |
| cargo-allow | Stabilizing | Source candidate. | cargo-allow check --mode no-new | Exact source candidate. |
"#;
        let rows = validate_support_tier_claims(input).map_err(|error| error.to_string())?;
        let Some(row) = rows.first() else {
            return Err("proof-command compatibility table produced no rows".to_string());
        };
        if row.tier != SupportTierLevel::Stabilizing {
            return Err(format!(
                "expected Stabilizing compatibility row, got {:?}",
                row.tier
            ));
        }
        Ok(())
    }
}
