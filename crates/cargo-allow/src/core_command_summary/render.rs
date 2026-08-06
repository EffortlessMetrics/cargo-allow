use repo_protocol::ResultClassV1;

use super::{CoreCommandPostureV1, CoreCommandSummaryV1, validate_core_command_summary};

pub fn render_core_command_summary_json(summary: &CoreCommandSummaryV1) -> Result<String, String> {
    validate_core_command_summary(summary)?;
    serde_json::to_string_pretty(summary).map_err(|error| error.to_string())
}

pub fn render_core_command_summary_human(summary: &CoreCommandSummaryV1) -> String {
    let mut output = String::new();
    output.push_str("Result: ");
    output.push_str(&result_label(summary));
    output.push('\n');
    output.push_str("Why: ");
    output.push_str(&allow_report::sanitize_terminal_text(
        &summary.reason.message,
    ));
    output.push('\n');
    output.push_str("Subject: ");
    output.push_str(summary.subject.kind.as_str());
    output.push(' ');
    output.push_str(&allow_report::sanitize_terminal_text(
        &summary.subject.portable_identity,
    ));
    output.push('\n');
    output.push_str("Coverage: ");
    output.push_str(summary.completeness.as_str());
    output.push_str(" / ");
    output.push_str(summary.currentness.as_str());
    if let Some(limitation) = summary.subject.limitations.first() {
        output.push_str(" — ");
        output.push_str(&allow_report::sanitize_terminal_text(limitation));
    }
    output.push('\n');
    output.push_str("Next: ");
    match summary.primary_action.as_ref() {
        Some(action) => output.push_str(&allow_report::sanitize_terminal_text(&action.display)),
        None if summary.posture == CoreCommandPostureV1::DecisionRequired => {
            output.push_str("repository decision required")
        }
        None => output.push_str("no deterministic safe action selected"),
    }
    output.push('\n');
    output.push_str("Writes: ");
    render_writes(summary, &mut output);
    output.push('\n');
    output.push_str("Then: ");
    match summary.next_proof.as_ref() {
        Some(action) => output.push_str(&allow_report::sanitize_terminal_text(&action.display)),
        None => output.push_str("no follow-up proof command selected"),
    }
    output.push('\n');
    output.push_str("Not proven: ");
    output.push_str(&allow_report::sanitize_terminal_text(
        &summary.claim_boundary.statement,
    ));
    if let Some(limitation) = summary.claim_boundary.limitations.first() {
        output.push_str(" — ");
        output.push_str(&allow_report::sanitize_terminal_text(limitation));
    }
    output.push('\n');
    output
}

fn result_label(summary: &CoreCommandSummaryV1) -> String {
    match (summary.result_class, summary.posture) {
        (ResultClassV1::Completed, CoreCommandPostureV1::Satisfied) => "satisfied".to_string(),
        (ResultClassV1::Findings, CoreCommandPostureV1::Advisory) => {
            "findings (advisory)".to_string()
        }
        (ResultClassV1::Findings, CoreCommandPostureV1::Blocking) => {
            "findings (blocking)".to_string()
        }
        _ => format!(
            "{} ({})",
            summary.result_class.as_str(),
            summary.posture.as_str()
        ),
    }
}

fn render_writes(summary: &CoreCommandSummaryV1, output: &mut String) {
    if summary.operation_effects.writes_repository {
        output.push_str(&join_paths(&summary.operation_effects.write_paths));
        return;
    }
    output.push_str("nothing in this operation");
    if let Some(action) = summary.primary_action.as_ref()
        && !action.may_write_paths.is_empty()
    {
        output.push_str("; selected next action may write ");
        output.push_str(&join_paths(&action.may_write_paths));
    }
}

fn join_paths(paths: &[String]) -> String {
    paths
        .iter()
        .map(|path| allow_report::sanitize_terminal_text(path))
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) fn render_argv_for_display(program: &str, args: &[String]) -> String {
    if std::iter::once(program)
        .chain(args.iter().map(String::as_str))
        .any(|part| part.contains('\0') || part.contains('\n') || part.contains('\r'))
    {
        return "[use structured argv; command contains non-pasteable control text]".to_string();
    }
    std::iter::once(program)
        .chain(args.iter().map(String::as_str))
        .map(quote_for_platform)
        .collect::<Vec<_>>()
        .join(" ")
}

fn quote_for_platform(argument: &str) -> String {
    if cfg!(windows) {
        quote_windows_cmd(argument)
    } else {
        quote_posix(argument)
    }
}

fn quote_posix(argument: &str) -> String {
    if argument.is_empty() {
        return "''".to_string();
    }
    if argument.bytes().all(|byte| {
        matches!(
            byte,
            b'a'..=b'z'
                | b'A'..=b'Z'
                | b'0'..=b'9'
                | b'_'
                | b'-'
                | b'.'
                | b'/'
                | b':'
                | b'@'
                | b'+'
                | b'='
                | b','
                | b'%'
        )
    }) {
        return argument.to_string();
    }
    let mut output = String::from("'");
    for character in argument.chars() {
        if character == '\'' {
            output.push_str("'\\''");
        } else {
            output.push(character);
        }
    }
    output.push('\'');
    output
}

fn quote_windows_cmd(argument: &str) -> String {
    if argument.is_empty() {
        return "\"\"".to_string();
    }
    if !argument.chars().any(|character| {
        matches!(
            character,
            ' ' | '\t'
                | '"'
                | '&'
                | '|'
                | '<'
                | '>'
                | '^'
                | '%'
                | '!'
                | '('
                | ')'
                | ','
                | ';'
                | '='
        )
    }) {
        return argument.to_string();
    }
    let mut output = String::from("\"");
    let mut backslashes = 0usize;
    for character in argument.chars() {
        match character {
            '\\' => backslashes += 1,
            '"' => {
                for _ in 0..(backslashes * 2 + 1) {
                    output.push('\\');
                }
                backslashes = 0;
                output.push('"');
            }
            _ => {
                for _ in 0..backslashes {
                    output.push('\\');
                }
                backslashes = 0;
                output.push(character);
            }
        }
    }
    for _ in 0..(backslashes * 2) {
        output.push('\\');
    }
    output.push('"');
    output
}
