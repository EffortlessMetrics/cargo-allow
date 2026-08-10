//! Shell-safe rendering of `why` proof plans.
//!
//! Authority is always `program` + ordered `args`. Human text is a projection
//! for the current platform's common paste shell; JSON keeps structured argv.

/// One suggested follow-up invocation: program plus ordered arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ProofPlan {
    pub program: String,
    pub args: Vec<String>,
}

impl ProofPlan {
    pub(super) fn cargo_allow(args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            program: "cargo-allow".to_string(),
            args: args.into_iter().map(Into::into).collect(),
        }
    }
}

/// Render a plan for human "Proof commands" output.
///
/// When every argument can be represented as one pasteable shell token for the
/// current platform, returns a single shell line. Otherwise returns an explicit
/// non-copyable argv listing so operators do not paste a broken command.
pub(super) fn render_proof_command(plan: &ProofPlan) -> String {
    if plan_has_unsafe_shell_chars(plan) {
        return render_non_copyable_argv(plan);
    }
    let mut parts = Vec::with_capacity(plan.args.len() + 1);
    parts.push(quote_for_platform(&plan.program));
    for arg in &plan.args {
        parts.push(quote_for_platform(arg));
    }
    parts.join(" ")
}

fn plan_has_unsafe_shell_chars(plan: &ProofPlan) -> bool {
    std::iter::once(plan.program.as_str())
        .chain(plan.args.iter().map(String::as_str))
        .any(|part| part.chars().any(|character| character.is_control()))
}

fn render_non_copyable_argv(plan: &ProofPlan) -> String {
    let mut out =
        String::from("[not a pasteable shell command; use structured argv / proof_plans JSON]\n");
    out.push_str("program: ");
    out.push_str(&plan.program);
    out.push_str("\nargs:");
    for (index, arg) in plan.args.iter().enumerate() {
        out.push_str(&format!("\n  [{index}]={}", debug_arg(arg)));
    }
    out
}

fn debug_arg(arg: &str) -> String {
    // Debug quoting keeps newlines/tabs visible without implying shell paste safety.
    format!("{arg:?}")
}

fn quote_for_platform(arg: &str) -> String {
    if cfg!(windows) {
        quote_windows_cmd(arg)
    } else {
        quote_posix(arg)
    }
}

/// POSIX sh single-quote strategy (bash / dash / zsh paste-safe).
pub(super) fn quote_posix(arg: &str) -> String {
    if arg.is_empty() {
        return "''".to_string();
    }
    if is_posix_safe_unquoted(arg) {
        return arg.to_string();
    }
    let mut out = String::from("'");
    for ch in arg.chars() {
        if ch == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}

fn is_posix_safe_unquoted(arg: &str) -> bool {
    !arg.is_empty()
        && arg.bytes().all(|b| {
            matches!(
                b,
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
        })
}

/// Windows `cmd.exe` token quoting for pasteable one-line commands.
pub(super) fn quote_windows_cmd(arg: &str) -> String {
    if arg.is_empty() {
        return "\"\"".to_string();
    }
    if is_windows_safe_unquoted(arg) {
        return arg.to_string();
    }
    let mut out = String::from("\"");
    let mut backslashes = 0usize;
    for ch in arg.chars() {
        match ch {
            '\\' => {
                backslashes += 1;
            }
            '"' => {
                for _ in 0..(backslashes * 2 + 1) {
                    out.push('\\');
                }
                backslashes = 0;
                out.push('"');
            }
            _ => {
                for _ in 0..backslashes {
                    out.push('\\');
                }
                backslashes = 0;
                out.push(ch);
            }
        }
    }
    for _ in 0..(backslashes * 2) {
        out.push('\\');
    }
    out.push('"');
    out
}

fn is_windows_safe_unquoted(arg: &str) -> bool {
    !arg.is_empty()
        && !arg.chars().any(|ch| {
            matches!(
                ch,
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
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn posix_quotes_spaces_and_metacharacters_as_one_token() {
        assert_eq!(quote_posix("src/ordinary.rs"), "src/ordinary.rs");
        assert_eq!(quote_posix("src/has space.rs"), "'src/has space.rs'");
        assert_eq!(
            quote_posix("src/quote'and\"double.rs"),
            "'src/quote'\\''and\"double.rs'"
        );
        assert_eq!(
            quote_posix("src/$(touch pwned).rs"),
            "'src/$(touch pwned).rs'"
        );
        assert_eq!(
            quote_posix("src/a;echo injected.rs"),
            "'src/a;echo injected.rs'"
        );
        assert_eq!(quote_posix("-leading-dash.rs"), "-leading-dash.rs");
        assert_eq!(quote_posix("src/ユニコード.rs"), "'src/ユニコード.rs'");
    }

    #[test]
    fn windows_quotes_spaces_and_metacharacters_as_one_token() {
        assert_eq!(quote_windows_cmd("src/ordinary.rs"), "src/ordinary.rs");
        assert_eq!(
            quote_windows_cmd("src/has space.rs"),
            "\"src/has space.rs\""
        );
        assert_eq!(
            quote_windows_cmd("src/a;echo injected.rs"),
            "\"src/a;echo injected.rs\""
        );
        assert_eq!(
            quote_windows_cmd("src/quote\"double.rs"),
            "\"src/quote\\\"double.rs\""
        );
    }

    #[test]
    fn newline_path_uses_non_copyable_argv_listing() {
        let plan = ProofPlan::cargo_allow(["add", "--path", "src/line\nbreak.rs", "--line", "1"]);
        let rendered = render_proof_command(&plan);
        assert!(
            rendered.contains("not a pasteable shell command"),
            "newline paths must not look pasteable: {rendered}"
        );
        assert!(
            rendered.contains("src/line\\nbreak.rs") || rendered.contains("line\\nbreak"),
            "debug form should keep newline visible: {rendered}"
        );
        assert!(
            !rendered.contains("cargo-allow add --path src/line\n"),
            "must not emit a two-line pasteable shell command: {rendered}"
        );
    }

    #[test]
    fn ordinary_add_plan_round_trips_path_argument_identity() {
        let path = "src/has space.rs";
        let plan = ProofPlan::cargo_allow([
            "add",
            "--kind",
            "panic",
            "--path",
            path,
            "--line",
            "10",
            "--owner",
            "<owner>",
            "--reason",
            "...",
            "--evidence",
            "<ref>",
            "--write",
            "policy/allow.toml",
        ]);
        assert_eq!(
            plan.args.iter().find(|arg| arg.as_str() == path),
            Some(&path.to_string()),
            "plan must retain exact normalized path argument"
        );
        let rendered = render_proof_command(&plan);
        assert!(
            rendered.contains("cargo-allow"),
            "rendered command should name the program: {rendered}"
        );
        // Platform quote keeps the path as one token; strip quotes and compare.
        let path_arg = plan
            .args
            .iter()
            .position(|arg| arg == "--path")
            .and_then(|index| plan.args.get(index + 1))
            .map(String::as_str);
        assert_eq!(path_arg, Some(path));
    }
}
