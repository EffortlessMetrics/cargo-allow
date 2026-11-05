//! ReleaseQualificationV1 ledger checker (#2263).
//!
//! Validates the deterministic release blocker ledger at
//! `docs/release/evidence/next-release-qualification.json` against its closed
//! row contract, recomputes the derived readiness flags, cross-checks the
//! platform vocabulary against the `#3737` final-selection freeze, and
//! regenerates the human projection so JSON and Markdown cannot disagree.
//!
//! The checker is a repository-native invariant, not release authority: it
//! derives go/no-go inputs and never grants, creates, or validates release
//! authorization (#3760).

use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

/// The ledger and its projection, checked as one unit.
#[derive(Debug)]
struct Ledger {
    root: PathBuf,
    doc: Value,
}

fn main_guarded() -> Result<(), Box<dyn Error>> {
    let root = repository_root()?;
    if !root.join(".git").exists() {
        // Packaged or export checkouts do not carry the release evidence tree.
        return Ok(());
    }
    let ledger = Ledger::load(&root)?;
    ledger.check_binding()?;
    ledger.check_vocabularies()?;
    ledger.check_rows()?;
    ledger.check_derived()?;
    ledger.check_support_matrix()?;
    ledger.check_markdown()?;
    Ok(())
}

#[test]
fn release_qualification_ledger_satisfies_row_contract() -> Result<(), Box<dyn Error>> {
    main_guarded()
}

impl Ledger {
    fn load(root: &Path) -> Result<Self, Box<dyn Error>> {
        let _json_text = read(
            root,
            "docs/release/evidence/next-release-qualification.json",
        )?;
        let doc: Value = serde_json::from_str(&_json_text)
            .map_err(|error| io::Error::other(format!("ledger JSON is not valid: {error}")))?;
        Ok(Ledger {
            root: root.to_path_buf(),
            doc,
        })
    }

    fn check_binding(&self) -> Result<(), Box<dyn Error>> {
        let binding = obj(&self.doc, "binding")?;
        for key in [
            "repository",
            "basis_commit",
            "basis_tree",
            "workspace_version",
            "declared_msrv",
            "support_matrix_selection_digest",
            "issue_snapshot_utc",
        ] {
            let value = str_field(binding, key)?;
            require(
                !value.trim().is_empty(),
                &format!("binding.{key} must be a non-empty string"),
            )?;
        }
        require(
            str_field(binding, "repository")? == "EffortlessMetrics/cargo-allow",
            "binding.repository must name this repository",
        )?;
        for key in ["basis_commit", "basis_tree"] {
            let value = str_field(binding, key)?;
            require(
                value.len() == 40 && value.chars().all(|c| c.is_ascii_hexdigit()),
                &format!("binding.{key} must be a full 40-hex git object id, got {value}"),
            )?;
        }
        for key in [
            "qualification_umbrella",
            "ledger_issue",
            "release_controller",
            "release_train",
            "selection_controlling_issue",
        ] {
            let value = binding.get(key).and_then(Value::as_u64).ok_or_else(|| {
                io::Error::other(format!("binding.{key} must be an issue number"))
            })?;
            require(value > 0, &format!("binding.{key} must be positive"))?;
        }
        require(
            str_field(binding, "workspace_version")? == "0.2.0",
            "binding.workspace_version must record the prospective final version",
        )?;
        // The snapshot must be canonical UTC so refreshes order deterministically.
        // Access is via `str::get`; bracket indexing would panic on malformed
        // input instead of failing into the require below.
        let snapshot = str_field(binding, "issue_snapshot_utc")?;
        let canonical = snapshot.len() == 20
            && snapshot.is_ascii()
            && snapshot.get(4..5) == Some("-")
            && snapshot.get(7..8) == Some("-")
            && snapshot.get(10..11) == Some("T")
            && snapshot.get(13..14) == Some(":")
            && snapshot.get(16..17) == Some(":")
            && snapshot.get(19..20) == Some("Z")
            && snapshot.get(..19).is_some_and(|body| {
                body.chars()
                    .all(|c| c.is_ascii_digit() || c == '-' || c == ':' || c == 'T')
            });
        require(
            canonical,
            &format!(
                "binding.issue_snapshot_utc must be canonical UTC (YYYY-MM-DDTHH:MM:SSZ), got {snapshot}"
            ),
        )?;
        Ok(())
    }

    fn check_vocabularies(&self) -> Result<(), Box<dyn Error>> {
        let vocab = obj(&self.doc, "vocabularies")?;
        for key in [
            "qualification_disposition",
            "reproduction_result",
            "timing_gate",
            "obligation",
            "severity",
            "capability",
        ] {
            let values = vocab
                .get(key)
                .and_then(Value::as_array)
                .ok_or_else(|| io::Error::other(format!("vocabularies.{key} must be a list")))?;
            require(
                !values.is_empty(),
                &format!("vocabularies.{key} must not be empty"),
            )?;
            for value in values {
                require(
                    value.is_string(),
                    &format!("vocabularies.{key} entries must be strings"),
                )?;
            }
        }
        let claim = str_field(&self.doc, "claim_boundary")?;
        require(
            claim.contains("does not fix defects") && claim.contains("#3760"),
            "claim_boundary must state the audit boundary and the authorization owner",
        )?;
        let staleness = str_field(&self.doc, "staleness_law")?;
        require(
            staleness.contains("stale against any head other than binding.basis_commit"),
            "staleness_law must bind staleness to the basis commit",
        )?;
        Ok(())
    }

    fn rows(&self) -> Result<Vec<Value>, Box<dyn Error>> {
        let rows = self
            .doc
            .get("rows")
            .and_then(Value::as_array)
            .ok_or_else(|| io::Error::other("rows must be a list"))?;
        require(!rows.is_empty(), "rows must not be empty")?;
        Ok(rows.clone())
    }

    fn check_rows(&self) -> Result<(), Box<dyn Error>> {
        let rows = self.rows()?;
        let mut seen_ids = std::collections::BTreeSet::new();
        for row in rows.iter() {
            let owner = format!(
                "row {}",
                row.get("id").and_then(Value::as_str).unwrap_or("?")
            );
            let id = str_field(row, "id")?;
            require(
                seen_ids.insert(id.clone()),
                &format!("duplicate row id {id}"),
            )?;
            let issues = row
                .get("issues")
                .and_then(Value::as_array)
                .ok_or_else(|| io::Error::other(format!("{owner}: issues must be a list")))?;
            for issue in issues {
                require(
                    issue.is_u64(),
                    &format!("{owner}: issues must be issue numbers"),
                )?;
            }
            let umbrella = row.get("umbrella").and_then(Value::as_u64).ok_or_else(|| {
                io::Error::other(format!("{owner}: umbrella must be an issue number"))
            })?;
            require(
                umbrella == 2262,
                &format!("{owner}: umbrella must be #2262"),
            )?;
            let title = str_field(row, "title")?;
            require(
                !title.trim().is_empty(),
                &format!("{owner}: title required"),
            )?;
            let capability = str_field(row, "capability")?;
            self.require_vocab("capability", &capability)?;
            if let Some(platform) = optional_str_field(row, "platform")? {
                require(
                    !platform.trim().is_empty(),
                    &format!("{owner}: platform must be null or a non-empty name"),
                )?;
            }
            let evidence = row
                .get("evidence_current_main")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    io::Error::other(format!("{owner}: evidence_current_main must be a list"))
                })?;
            for entry in evidence {
                let entry = entry.as_str().ok_or_else(|| {
                    io::Error::other(format!("{owner}: evidence entries must be paths"))
                })?;
                let path = self.root.join(entry);
                require(
                    path.is_file(),
                    &format!("{owner}: evidence path {entry} does not exist on this tree"),
                )?;
            }
            let query = str_field(row, "reproduction_command_or_query")?;
            require(
                !query.trim().is_empty(),
                &format!("{owner}: reproduction_command_or_query required"),
            )?;
            let result = str_field(row, "reproduction_result")?;
            self.require_vocab("reproduction_result", &result)?;
            let disposition = str_field(row, "qualification_disposition")?;
            self.require_vocab("qualification_disposition", &disposition)?;
            let timing = str_field(row, "timing_gate")?;
            self.require_vocab("timing_gate", &timing)?;
            let obligation = str_field(row, "obligation")?;
            self.require_vocab("obligation", &obligation)?;
            let severity = str_field(row, "severity")?;
            self.require_vocab("severity", &severity)?;
            let next_action = str_field(row, "next_action")?;
            let closure = str_field(row, "closure_proof_required")?;
            let claim = str_field(row, "claim_boundary")?;
            require(
                !claim.trim().is_empty(),
                &format!("{owner}: claim_boundary required"),
            )?;

            // One closed disposition drives the timing mapping.
            let timing_ok = match disposition.as_str() {
                "BlockerBeforeCandidate" => {
                    timing == "candidate_bytes" || timing == "source_candidate_freeze"
                }
                "BlockerBeforeTag" => timing == "tag",
                "PostReleaseHardening" => timing == "post_publication",
                _ => timing == "none",
            };
            require(
                timing_ok,
                &format!("{owner}: disposition {disposition} forbids timing {timing}"),
            )?;

            // Blockers need an owner action and closure proof; stale rows need
            // current evidence; pending rows must not claim fixed results.
            if disposition == "BlockerBeforeCandidate" || disposition == "BlockerBeforeTag" {
                require(
                    !next_action.trim().is_empty() && !closure.trim().is_empty(),
                    &format!("{owner}: blocker rows need next_action and closure_proof_required"),
                )?;
                require(
                    result != "AlreadyFixed",
                    &format!("{owner}: a blocker cannot reproduce as AlreadyFixed"),
                )?;
            }
            if disposition == "StaleOrAlreadyFixed" {
                require(
                    !evidence.is_empty(),
                    &format!("{owner}: StaleOrAlreadyFixed rows need current evidence paths"),
                )?;
                require(
                    result == "AlreadyFixed" || result == "NotReproduced",
                    &format!(
                        "{owner}: StaleOrAlreadyFixed rows need a fixed/not-reproduced result"
                    ),
                )?;
            }
            if disposition == "PostReleaseHardening" {
                require(
                    result == "NeedsOperatorProof" || result == "Reproduced",
                    &format!(
                        "{owner}: PostReleaseHardening rows need an operator or reproduced result"
                    ),
                )?;
            }
            if result == "AmbiguousPremise" {
                require(
                    disposition == "NeedsRepositoryDecision",
                    &format!("{owner}: ambiguous premises must route to NeedsRepositoryDecision"),
                )?;
            }
            if result == "NeedsOperatorProof" {
                require(
                    disposition != "StaleOrAlreadyFixed",
                    &format!("{owner}: unexecuted operator evidence cannot count as already fixed"),
                )?;
            }
        }
        Ok(())
    }

    fn check_derived(&self) -> Result<(), Box<dyn Error>> {
        let rows = self.rows()?;
        let candidate = rows
            .iter()
            .filter(|r| {
                str_field(r, "qualification_disposition").ok().as_deref()
                    == Some("BlockerBeforeCandidate")
            })
            .count();
        let tag = rows
            .iter()
            .filter(|r| {
                str_field(r, "qualification_disposition").ok().as_deref()
                    == Some("BlockerBeforeTag")
            })
            .count();
        let derived = obj(&self.doc, "derived")?;
        let stored_candidate = derived
            .get("blocker_before_candidate_count")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                io::Error::other("derived.blocker_before_candidate_count must be a number")
            })? as usize;
        let stored_tag = derived
            .get("blocker_before_tag_count")
            .and_then(Value::as_u64)
            .ok_or_else(|| io::Error::other("derived.blocker_before_tag_count must be a number"))?
            as usize;
        let stored_candidate_ready = derived
            .get("candidate_ready")
            .and_then(Value::as_bool)
            .ok_or_else(|| io::Error::other("derived.candidate_ready must be a boolean"))?;
        let stored_tag_ready = derived
            .get("tag_ready")
            .and_then(Value::as_bool)
            .ok_or_else(|| io::Error::other("derived.tag_ready must be a boolean"))?;

        require(
            stored_candidate == candidate && stored_tag == tag,
            &format!(
                "derived blocker counts {stored_candidate}/{stored_tag} do not match the rows {candidate}/{tag}"
            ),
        )?;
        let candidate_ready = candidate == 0;
        let tag_ready = candidate_ready && tag == 0;
        require(
            stored_candidate_ready == candidate_ready,
            "candidate_ready must equal (blocker_before_candidate_count == 0)",
        )?;
        require(
            stored_tag_ready == tag_ready,
            "tag_ready must equal candidate_ready && (blocker_before_tag_count == 0)",
        )?;

        // Currentness law: an old basis can never carry a readiness claim. The
        // current ledger has blockers, so this only bites a future refresh
        // that claims readiness from a stale basis.
        if stored_candidate_ready || stored_tag_ready {
            let basis = str_field(obj(&self.doc, "binding")?, "basis_commit")?;
            let head = current_head().ok_or_else(|| {
                io::Error::other(
                    "the ledger claims readiness but the current HEAD cannot be resolved; \
                     a readiness claim requires a current-head basis",
                )
            })?;
            require(
                basis == head,
                &format!(
                    "the ledger claims readiness from basis {basis} but HEAD is {head}; \
                     refresh the ledger on the current head before claiming readiness"
                ),
            )?;
        }
        Ok(())
    }

    fn check_support_matrix(&self) -> Result<(), Box<dyn Error>> {
        let matrix = read(&self.root, "docs/support-matrix.toml")?;
        let section = final_selection_section(&matrix)?;
        let digest_line = section
            .lines()
            .find_map(|line| line.strip_prefix("selection_digest = \""))
            .and_then(|rest| rest.strip_suffix('"'))
            .ok_or_else(|| io::Error::other("final_selection section has no selection_digest"))?;
        let binding = obj(&self.doc, "binding")?;
        require(
            str_field(binding, "support_matrix_selection_digest")? == digest_line,
            "binding.support_matrix_selection_digest must equal the final_selection digest in docs/support-matrix.toml",
        )?;

        // Collect the closed dimension/subject vocabulary of the selection.
        let mut subjects: Vec<(String, String, String)> = Vec::new();
        let mut dimension = String::new();
        let mut subject = String::new();
        let mut disposition = String::new();
        let flush = |dimension: &mut String,
                     subject: &mut String,
                     disposition: &mut String,
                     out: &mut Vec<(String, String, String)>| {
            if !dimension.is_empty() && !subject.is_empty() {
                out.push((
                    std::mem::take(dimension),
                    std::mem::take(subject),
                    std::mem::take(disposition),
                ));
            }
        };
        for line in section.lines() {
            let line = line.trim();
            if line == "[[final_selection.rows]]" {
                flush(
                    &mut dimension,
                    &mut subject,
                    &mut disposition,
                    &mut subjects,
                );
                continue;
            }
            if let Some(rest) = line.strip_prefix("dimension = \"") {
                dimension = rest.strip_suffix('"').unwrap_or("").to_string();
            } else if let Some(rest) = line.strip_prefix("subject = \"") {
                subject = rest.strip_suffix('"').unwrap_or("").to_string();
            } else if let Some(rest) = line.strip_prefix("disposition = \"") {
                disposition = rest.strip_suffix('"').unwrap_or("").to_string();
            }
        }
        flush(
            &mut dimension,
            &mut subject,
            &mut disposition,
            &mut subjects,
        );
        require(
            !subjects.is_empty(),
            "final_selection rows must parse from docs/support-matrix.toml",
        )?;

        // Ledger platform names must exist in the closed selection vocabulary,
        // and a declined (not_included) platform claim can never carry a
        // release blocker.
        let rows = self.rows()?;
        for row in &rows {
            let platform = match optional_str_field(row, "platform")? {
                Some(platform) => platform,
                None => continue,
            };
            let id = str_field(row, "id")?;
            let matched = subjects.iter().any(|(dimension, subject, _disposition)| {
                subject.as_str() == platform
                    || (dimension == "rust" && platform == format!("rust-{subject}"))
            });
            require(
                matched,
                &format!(
                    "row {id}: platform {platform} is absent from the #3737 final-selection vocabulary"
                ),
            )?;
            let declined = subjects.iter().any(|(dimension, subject, disposition)| {
                (subject.as_str() == platform
                    || (dimension == "rust" && platform == format!("rust-{subject}")))
                    && disposition == "not_included"
            });
            if declined {
                let disposition = str_field(row, "qualification_disposition")?;
                require(
                    disposition != "BlockerBeforeCandidate" && disposition != "BlockerBeforeTag",
                    &format!(
                        "row {id}: platform {platform} is not_included in the support matrix and cannot carry a release blocker"
                    ),
                )?;
            }
        }
        Ok(())
    }

    fn check_markdown(&self) -> Result<(), Box<dyn Error>> {
        let committed = match fs::read_to_string(
            self.root
                .join("docs/release/evidence/next-release-qualification.md"),
        ) {
            Ok(text) => Some(text.replace("\r\n", "\n")),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => {
                return Err(io::Error::other(format!(
                    "failed to read the committed projection: {error}"
                ))
                .into());
            }
        };
        let rendered = self.render_markdown();
        if committed.as_deref() != Some(rendered.as_str()) {
            let fallback = std::env::temp_dir().join("next-release-qualification.rendered.md");
            fs::write(&fallback, rendered.as_bytes()).map_err(|error| {
                io::Error::other(format!(
                    "failed to write the rendered projection to {}: {error}",
                    fallback.display()
                ))
            })?;
            return Err(io::Error::other(format!(
                "committed markdown {} match the JSON projection; \
                 rendered bytes written to {} for reconciliation",
                if committed.is_some() {
                    "does not"
                } else {
                    "file does not exist, so it cannot"
                },
                fallback.display()
            ))
            .into());
        }
        Ok(())
    }

    fn render_markdown(&self) -> String {
        // Render is only reached after the structural checks pass, so the
        // fallbacks below are unreachable on any checked ledger; they keep the
        // renderer panic-free instead of indexing or unwrapping.
        let binding = self.doc.get("binding").cloned().unwrap_or(Value::Null);
        let derived = self.doc.get("derived").cloned().unwrap_or(Value::Null);
        let field = |name: &str| {
            binding
                .get(name)
                .and_then(Value::as_str)
                .unwrap_or_default()
        };
        let num = |name: &str| {
            derived
                .get(name)
                .and_then(Value::as_u64)
                .unwrap_or_default()
        };
        let flag = |name: &str| {
            derived
                .get(name)
                .and_then(Value::as_bool)
                .unwrap_or_default()
        };
        let rows = self
            .doc
            .get("rows")
            .cloned()
            .unwrap_or(Value::Array(Vec::new()));
        let empty_rows = Vec::new();
        let all_rows: Vec<&Value> = rows.as_array().unwrap_or(&empty_rows).iter().collect();
        let mut out = String::new();
        out.push_str("# ReleaseQualificationV1 — current-main blocker ledger\n\n");
        out.push_str("Dated, checked go/no-go input for the prospective `0.2.0` final release,\n");
        out.push_str("materializing #2263 against exact main basis. This projection derives\n");
        out.push_str("from `docs/release/evidence/next-release-qualification.json`; the\n");
        out.push_str("`release_qualification` checker regenerates it and fails on divergence.\n");
        out.push_str(
            "It classifies evidence and routes work; it is not release authorization.\n\n",
        );
        out.push_str("## Binding\n\n");
        out.push_str("| Field | Value |\n| --- | --- |\n");
        out.push_str(&format!("| repository | {} |\n", field("repository")));
        out.push_str(&format!("| basis commit | `{}` |\n", field("basis_commit")));
        out.push_str(&format!("| basis tree | `{}` |\n", field("basis_tree")));
        out.push_str(&format!(
            "| workspace version | {} |\n",
            field("workspace_version")
        ));
        out.push_str(&format!(
            "| declared MSRV | Rust {} |\n",
            field("declared_msrv")
        ));
        out.push_str(&format!(
            "| qualification umbrella | #{} |\n",
            binding
                .get("qualification_umbrella")
                .and_then(Value::as_u64)
                .unwrap_or_default()
        ));
        out.push_str(&format!(
            "| ledger issue | #{} |\n",
            binding
                .get("ledger_issue")
                .and_then(Value::as_u64)
                .unwrap_or_default()
        ));
        out.push_str(&format!(
            "| release controller | #{} |\n",
            binding
                .get("release_controller")
                .and_then(Value::as_u64)
                .unwrap_or_default()
        ));
        out.push_str(&format!(
            "| release train | #{} |\n",
            binding
                .get("release_train")
                .and_then(Value::as_u64)
                .unwrap_or_default()
        ));
        out.push_str(&format!(
            "| selection controlling issue | #{} |\n",
            binding
                .get("selection_controlling_issue")
                .and_then(Value::as_u64)
                .unwrap_or_default()
        ));
        out.push_str(&format!(
            "| support-matrix selection digest | `{}` |\n",
            field("support_matrix_selection_digest")
        ));
        out.push_str(&format!(
            "| issue snapshot (UTC) | {} |\n",
            field("issue_snapshot_utc")
        ));
        out.push_str(&format!(
            "| schema | {} generation {} |\n\n",
            self.doc
                .get("schema")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            self.doc
                .get("schema_version")
                .and_then(Value::as_u64)
                .unwrap_or_default()
        ));
        out.push_str("## Staleness law\n\n");
        out.push_str(&wrap_paragraph(
            self.doc
                .get("staleness_law")
                .and_then(Value::as_str)
                .unwrap_or_default(),
        ));
        out.push('\n');
        out.push_str("## Derived readiness\n\n");
        out.push_str(&format!(
            "- BlockerBeforeCandidate rows: {}\n",
            num("blocker_before_candidate_count")
        ));
        out.push_str(&format!(
            "- BlockerBeforeTag rows: {}\n",
            num("blocker_before_tag_count")
        ));
        out.push_str(&format!("- candidate_ready: {}\n", flag("candidate_ready")));
        out.push_str(&format!("- tag_ready: {}\n\n", flag("tag_ready")));
        out.push_str(&wrap_paragraph(
            "Both readiness flags are derived from the rows, never hand-written: \
             candidate_ready is true only when no BlockerBeforeCandidate row remains and the \
             basis equals the current head; tag_ready additionally requires no BlockerBeforeTag \
             row. The checker recomputes both and fails on any divergence.\n\n",
        ));
        let blockers: Vec<&Value> = all_rows
            .iter()
            .copied()
            .filter(|row| {
                matches!(
                    str_field(row, "qualification_disposition").ok().as_deref(),
                    Some("BlockerBeforeCandidate") | Some("BlockerBeforeTag")
                )
            })
            .collect();
        out.push_str(&format!("## Open blockers ({})\n\n", blockers.len()));
        out.push_str(
            "| Row | Issues | Capability | Gate | Next action |\n| --- | --- | --- | --- | --- |\n",
        );
        for row in &blockers {
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                str_field(row, "id").unwrap_or_default(),
                render_issues(row),
                str_field(row, "capability").unwrap_or_default(),
                str_field(row, "timing_gate").unwrap_or_default(),
                str_field(row, "next_action").unwrap_or_default(),
            ));
        }
        out.push('\n');
        out.push_str(&format!("## All rows ({})\n\n", all_rows.len()));
        out.push_str(
            "| Row | Issues | Capability | Platform | Result | Disposition | Gate | Severity |\n",
        );
        out.push_str("| --- | --- | --- | --- | --- | --- | --- | --- |\n");
        for row in &all_rows {
            let platform = optional_str_field(row, "platform")
                .ok()
                .flatten()
                .unwrap_or_else(|| "-".to_string());
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} |\n",
                str_field(row, "id").unwrap_or_default(),
                render_issues(row),
                str_field(row, "capability").unwrap_or_default(),
                platform,
                str_field(row, "reproduction_result").unwrap_or_default(),
                str_field(row, "qualification_disposition").unwrap_or_default(),
                str_field(row, "timing_gate").unwrap_or_default(),
                str_field(row, "severity").unwrap_or_default(),
            ));
        }
        out.push('\n');
        out.push_str("## Claim boundary\n\n");
        out.push_str(&wrap_paragraph(
            self.doc
                .get("claim_boundary")
                .and_then(Value::as_str)
                .unwrap_or_default(),
        ));
        out
    }

    fn require_vocab(&self, vocabulary: &str, value: &str) -> Result<(), Box<dyn Error>> {
        let vocab = obj(&self.doc, "vocabularies")?;
        let allowed = vocab
            .get(vocabulary)
            .and_then(Value::as_array)
            .ok_or_else(|| io::Error::other(format!("vocabularies.{vocabulary} missing")))?;
        require(
            allowed.iter().any(|v| v.as_str() == Some(value)),
            &format!("{vocabulary} value {value} is outside the closed vocabulary"),
        )
    }
}

fn render_issues(row: &Value) -> String {
    let issues = row
        .get("issues")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(Value::as_u64)
                .map(|n| format!("#{n}"))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    if issues.is_empty() {
        "-".to_string()
    } else {
        issues
    }
}

fn wrap_paragraph(text: &str) -> String {
    let mut out = String::new();
    let mut line_len = 0usize;
    for word in text.split_whitespace() {
        if line_len == 0 {
            out.push_str(word);
            line_len = word.len();
        } else if line_len + 1 + word.len() <= 76 {
            out.push(' ');
            out.push_str(word);
            line_len += 1 + word.len();
        } else {
            out.push('\n');
            out.push_str(word);
            line_len = word.len();
        }
    }
    out.push_str("\n\n");
    out
}

fn final_selection_section(matrix: &str) -> Result<String, Box<dyn Error>> {
    let mut in_section = false;
    let mut body = String::new();
    for line in matrix.lines() {
        if line == "[final_selection]" {
            in_section = true;
            continue;
        }
        if !in_section {
            continue;
        }
        // A new section header that is not a final_selection row marker ends
        // the section.
        if line.starts_with('[') && line != "[[final_selection.rows]]" {
            break;
        }
        body.push_str(line);
        body.push('\n');
    }
    if !in_section {
        return Err(
            io::Error::other("docs/support-matrix.toml has no [final_selection] section").into(),
        );
    }
    Ok(body)
}

fn current_head() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|text| text.trim().to_string())
}

fn repository_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let crates_dir = manifest_dir
        .parent()
        .ok_or_else(|| io::Error::other("cargo-allow manifest has no crates parent"))?;
    let root = crates_dir
        .parent()
        .ok_or_else(|| io::Error::other("cargo-allow crates directory has no repository parent"))?;
    Ok(root.to_path_buf())
}

fn read(root: &Path, relative: &str) -> Result<String, Box<dyn Error>> {
    let path = root.join(relative);
    fs::read_to_string(&path)
        .map(|text| text.replace("\r\n", "\n"))
        .map_err(|error| {
            io::Error::other(format!("failed to read {}: {error}", path.display())).into()
        })
}

fn require(condition: bool, message: &str) -> Result<(), Box<dyn Error>> {
    if condition {
        Ok(())
    } else {
        Err(io::Error::other(message.to_string()).into())
    }
}

fn obj<'a>(value: &'a Value, name: &str) -> Result<&'a Value, Box<dyn Error>> {
    value
        .get(name)
        .filter(|v| v.is_object())
        .ok_or_else(|| io::Error::other(format!("section {name} must be an object")).into())
}

fn str_field(value: &Value, key: &str) -> Result<String, Box<dyn Error>> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| io::Error::other(format!("field {key} must be a string")).into())
}

fn optional_str_field(value: &Value, key: &str) -> Result<Option<String>, Box<dyn Error>> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(_) => Err(io::Error::other(format!("field {key} must be null or a string")).into()),
    }
}
