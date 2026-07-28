//! One shared output-style decision (#2572).
//!
//! The `--color` flag was advertised and discarded. Making it truthful means
//! exactly one place decides whether ANSI is emitted, and the machine formats
//! never consult it.
//!
//! # Structural guarantee
//!
//! [`Style`] is reached only through `ReportContext::style`, and only the
//! human renderer reads that field. JSON, SARIF, markdown, and HTML renderers
//! contain no reference to it, so they cannot emit ANSI even if this module is
//! wrong. That is deliberate: "machine formats are never styled" is enforced
//! by what the code can reach, not by a runtime check someone could forget.
//!
//! # Terminal safety
//!
//! Styling is applied only to fixed, tool-authored words. Repository-controlled
//! text — paths, reasons, symbols, scan messages — is never wrapped in escape
//! sequences, and [`sanitize_terminal_text`] strips control characters from it
//! so a crafted source file cannot inject cursor moves or colors into a
//! terminal reading a report.

/// Whether ANSI styling is emitted, and the vocabulary for doing so.
///
/// Color is supplemental: every state remains fully legible with styling off,
/// because the words and counts are identical either way. Only the escape
/// sequences differ.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Style {
    enabled: bool,
}

impl Style {
    /// Styling off. The default, and what every machine format uses.
    pub const PLAIN: Self = Self { enabled: false };

    /// Styling on. Only constructed by the CLI after the precedence in
    /// [`resolve`] selects it.
    pub const ANSI: Self = Self { enabled: true };

    pub fn is_enabled(self) -> bool {
        self.enabled
    }

    fn paint(self, code: &str, text: &str) -> String {
        if self.enabled {
            format!("\u{1b}[{code}m{text}\u{1b}[0m")
        } else {
            text.to_string()
        }
    }

    /// A passing or matched state.
    pub fn ok(self, text: &str) -> String {
        self.paint("32", text)
    }

    /// A blocking state: the reason a run failed.
    pub fn blocking(self, text: &str) -> String {
        self.paint("31", text)
    }

    /// An advisory state: worth attention, not failing the run.
    pub fn advisory(self, text: &str) -> String {
        self.paint("33", text)
    }

    /// Structural emphasis for headings and the result line.
    pub fn strong(self, text: &str) -> String {
        self.paint("1", text)
    }
}

/// What the user asked for on the command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorChoice {
    Auto,
    Always,
    Never,
}

/// Where the decision came from, so the precedence is observable in tests
/// rather than inferred from the outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleReason {
    FlagAlways,
    FlagNever,
    NoColorEnv,
    ClicolorForceEnv,
    CargoTermColorEnv,
    Terminal,
    NotATerminal,
    /// Machine format, or output redirected to a file.
    NotHumanStdout,
}

/// Environment inputs, passed in rather than read here so the precedence is
/// testable without mutating process state.
#[derive(Debug, Clone, Copy, Default)]
pub struct StyleEnv<'a> {
    pub no_color: Option<&'a str>,
    pub clicolor_force: Option<&'a str>,
    pub cargo_term_color: Option<&'a str>,
    pub stdout_is_terminal: bool,
}

/// The documented precedence law.
///
/// ```text
/// explicit --color always|never
///   > NO_COLOR
///   > CLICOLOR_FORCE
///   > CARGO_TERM_COLOR
///   > terminal capability
///   > plain
/// ```
///
/// `NO_COLOR` deliberately outranks `CLICOLOR_FORCE`. The clicolor convention
/// puts force first; this tool does not, because a user who exported
/// `NO_COLOR` has stated a preference and should not be overridden by an
/// environment variable they may not know is set. An explicit `--color always`
/// still wins over both, since that is a direct instruction for this run.
///
/// An unrecognised value for any variable is ignored and falls through to the
/// next rule; it never silently changes output.
pub fn resolve(choice: ColorChoice, env: StyleEnv<'_>) -> (Style, StyleReason) {
    match choice {
        ColorChoice::Never => return (Style::PLAIN, StyleReason::FlagNever),
        ColorChoice::Always => return (Style::ANSI, StyleReason::FlagAlways),
        ColorChoice::Auto => {}
    }

    // NO_COLOR: set and non-empty disables. An empty value is treated as unset,
    // matching the common reading of the convention.
    if env.no_color.is_some_and(|value| !value.is_empty()) {
        return (Style::PLAIN, StyleReason::NoColorEnv);
    }

    // CLICOLOR_FORCE: set and not "0" forces styling on.
    if env
        .clicolor_force
        .is_some_and(|value| !value.is_empty() && value != "0")
    {
        return (Style::ANSI, StyleReason::ClicolorForceEnv);
    }

    match env.cargo_term_color {
        Some("never") => return (Style::PLAIN, StyleReason::CargoTermColorEnv),
        Some("always") => return (Style::ANSI, StyleReason::CargoTermColorEnv),
        // "auto" and any unrecognised value fall through to capability.
        _ => {}
    }

    if env.stdout_is_terminal {
        (Style::ANSI, StyleReason::Terminal)
    } else {
        (Style::PLAIN, StyleReason::NotATerminal)
    }
}

/// Strip terminal control characters from repository-controlled text.
///
/// Paths, reasons, symbols, and scanner messages are derived from files in the
/// scanned repository. Echoing them verbatim would let a crafted source file
/// emit escape sequences into an operator's terminal — recoloring output,
/// moving the cursor, or hiding text — regardless of whether `--color` is on.
///
/// Tab and newline are preserved because the human layout uses them. Every
/// other C0 control, plus DEL and the C1 range, is replaced with U+FFFD so the
/// text stays visible and its length stays honest.
pub fn sanitize_terminal_text(text: &str) -> String {
    if !text.chars().any(is_terminal_control) {
        return text.to_string();
    }
    text.chars()
        .map(|character| {
            if is_terminal_control(character) {
                '\u{fffd}'
            } else {
                character
            }
        })
        .collect()
}

fn is_terminal_control(character: char) -> bool {
    match character {
        '\t' | '\n' => false,
        '\u{0}'..='\u{1f}' | '\u{7f}' => true,
        '\u{80}'..='\u{9f}' => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env() -> StyleEnv<'static> {
        StyleEnv::default()
    }

    #[test]
    fn an_explicit_flag_outranks_every_environment_variable() {
        let hostile = StyleEnv {
            no_color: Some("1"),
            clicolor_force: Some("1"),
            cargo_term_color: Some("never"),
            stdout_is_terminal: false,
        };
        assert_eq!(
            resolve(ColorChoice::Always, hostile),
            (Style::ANSI, StyleReason::FlagAlways)
        );

        let friendly = StyleEnv {
            no_color: None,
            clicolor_force: Some("1"),
            cargo_term_color: Some("always"),
            stdout_is_terminal: true,
        };
        assert_eq!(
            resolve(ColorChoice::Never, friendly),
            (Style::PLAIN, StyleReason::FlagNever)
        );
    }

    /// The deliberate deviation from the clicolor convention. A user who
    /// exported NO_COLOR is not overridden by CLICOLOR_FORCE.
    #[test]
    fn no_color_outranks_clicolor_force() {
        let both = StyleEnv {
            no_color: Some("1"),
            clicolor_force: Some("1"),
            ..env()
        };
        assert_eq!(
            resolve(ColorChoice::Auto, both),
            (Style::PLAIN, StyleReason::NoColorEnv)
        );
    }

    #[test]
    fn an_empty_no_color_is_treated_as_unset() {
        let empty = StyleEnv {
            no_color: Some(""),
            stdout_is_terminal: true,
            ..env()
        };
        assert_eq!(
            resolve(ColorChoice::Auto, empty),
            (Style::ANSI, StyleReason::Terminal)
        );
    }

    #[test]
    fn clicolor_force_zero_does_not_force() {
        let disabled = StyleEnv {
            clicolor_force: Some("0"),
            stdout_is_terminal: false,
            ..env()
        };
        assert_eq!(
            resolve(ColorChoice::Auto, disabled),
            (Style::PLAIN, StyleReason::NotATerminal)
        );
    }

    #[test]
    fn cargo_term_color_is_honoured_below_the_other_variables() {
        for (value, expected) in [("never", Style::PLAIN), ("always", Style::ANSI)] {
            let set = StyleEnv {
                cargo_term_color: Some(value),
                stdout_is_terminal: value == "never",
                ..env()
            };
            assert_eq!(
                resolve(ColorChoice::Auto, set),
                (expected, StyleReason::CargoTermColorEnv)
            );
        }
    }

    /// An unrecognised value must not silently change output; it falls through.
    #[test]
    fn an_unrecognised_cargo_term_color_falls_through_to_capability() {
        let odd = StyleEnv {
            cargo_term_color: Some("chartreuse"),
            stdout_is_terminal: false,
            ..env()
        };
        assert_eq!(
            resolve(ColorChoice::Auto, odd),
            (Style::PLAIN, StyleReason::NotATerminal)
        );
    }

    #[test]
    fn auto_follows_terminal_capability_when_nothing_else_applies() {
        let piped = StyleEnv {
            stdout_is_terminal: false,
            ..env()
        };
        let tty = StyleEnv {
            stdout_is_terminal: true,
            ..env()
        };
        assert_eq!(
            resolve(ColorChoice::Auto, piped),
            (Style::PLAIN, StyleReason::NotATerminal)
        );
        assert_eq!(
            resolve(ColorChoice::Auto, tty),
            (Style::ANSI, StyleReason::Terminal)
        );
    }

    /// Plain and styled output must carry identical words, so nothing is
    /// conveyed by color alone.
    #[test]
    fn styling_adds_escapes_without_changing_the_words() {
        for text in ["matched", "new", "stale", "Result: passed (enforcing)"] {
            assert_eq!(Style::PLAIN.ok(text), text);
            let styled = Style::ANSI.ok(text);
            assert!(styled.contains(text), "styled output must keep the word");
            assert!(styled.starts_with('\u{1b}'), "styled output carries ANSI");
        }
    }

    #[test]
    fn sanitize_strips_escape_sequences_from_repository_text() {
        let hostile = "src/\u{1b}[31mevil\u{1b}[0m.rs";
        let cleaned = sanitize_terminal_text(hostile);
        assert!(!cleaned.contains('\u{1b}'), "escape must not survive");
        assert!(cleaned.contains("evil"), "visible text is preserved");
    }

    #[test]
    fn sanitize_preserves_layout_whitespace_and_ordinary_text() {
        assert_eq!(sanitize_terminal_text("a\tb\nc"), "a\tb\nc");
        assert_eq!(
            sanitize_terminal_text("crates/foo/src/lib.rs"),
            "crates/foo/src/lib.rs"
        );
        assert_eq!(sanitize_terminal_text("réason — ok"), "réason — ok");
    }

    #[test]
    fn sanitize_strips_carriage_return_and_c1_controls() {
        assert!(!sanitize_terminal_text("a\rb").contains('\r'));
        assert!(!sanitize_terminal_text("a\u{9b}b").contains('\u{9b}'));
        assert!(!sanitize_terminal_text("a\u{7f}b").contains('\u{7f}'));
    }
}
