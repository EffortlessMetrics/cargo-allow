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

    /// Style a fixed status label using the shared semantic palette.
    ///
    /// The caller supplies tool-authored text; repository-controlled values
    /// must remain outside this helper so they cannot receive terminal escapes.
    pub fn status(self, status: &str, text: &str) -> String {
        match status {
            "matched" | "healthy" | "complete" | "pass" | "passed" | "valid" | "preserved" => {
                self.ok(text)
            }
            "new"
            | "expired"
            | "ambiguous"
            | "invalid_selector"
            | "missing_required_field"
            | "evidence_missing"
            | "baseline_debt"
            | "blocking"
            | "failed"
            | "invalid" => self.blocking(text),
            "review_due" | "stale" => self.advisory(text),
            _ => self.advisory(text),
        }
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
///   > NO_COLOR                 (disables)
///   > CLICOLOR_FORCE           (enables)
///   > CARGO_TERM_COLOR=never   (disables only; never enables)
///   > terminal capability
///   > plain
/// ```
///
/// Two deliberate deviations, both in the conservative direction:
///
/// `NO_COLOR` outranks `CLICOLOR_FORCE`. The clicolor convention puts force
/// first; this tool does not, because a user who exported `NO_COLOR` has
/// stated a preference and should not be overridden by a variable they may
/// not know is set.
///
/// `CARGO_TERM_COLOR` can only *disable*. It is cargo's variable, and CI
/// tooling sets `CARGO_TERM_COLOR=always` so that **cargo's** build logs are
/// coloured — it is not a statement about this tool's report output. Honouring
/// it to enable styling put ANSI into a non-TTY CI stream and broke consumers
/// grepping for a literal result line, so another tool's variable may turn our
/// styling off but only `--color` or a real terminal may turn it on.
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

    // Disable-only: see the precedence note above. `always` deliberately does
    // not force styling on, because CI sets it for cargo, not for us.
    if env.cargo_term_color == Some("never") {
        return (Style::PLAIN, StyleReason::CargoTermColorEnv);
    }

    if env.stdout_is_terminal {
        (Style::ANSI, StyleReason::Terminal)
    } else {
        (Style::PLAIN, StyleReason::NotATerminal)
    }
}

/// Make repository-controlled text safe to print on one report line.
///
/// Paths, families, reasons, symbols, and scanner messages are derived from
/// files in the scanned repository. Two distinct hazards, both handled here:
///
/// 1. **Terminal control.** Echoing an escape sequence verbatim would let a
///    crafted source file recolour output, move the cursor, or hide text,
///    regardless of whether `--color` is on. Those characters become U+FFFD.
///
/// 2. **Line forging.** A newline would let repository text start what looks
///    like a fresh tool-authored line — a fake `new: unreceipted …` entry in a
///    report someone reads to make a governance decision. Every value passed
///    through here is a scalar that belongs on one line, so `\n`, `\r`, and
///    `\t` are rendered as visible two-character escapes rather than acted on.
///
/// The text stays legible and its line count stays honest.
pub fn sanitize_terminal_text(text: &str) -> String {
    if !text.chars().any(needs_sanitizing) {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other if is_terminal_control(other) => out.push('\u{fffd}'),
            other => out.push(other),
        }
    }
    out
}

fn needs_sanitizing(character: char) -> bool {
    matches!(character, '\n' | '\r' | '\t') || is_terminal_control(character)
}

fn is_terminal_control(character: char) -> bool {
    matches!(character, '\u{0}'..='\u{1f}' | '\u{7f}' | '\u{80}'..='\u{9f}')
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
    fn cargo_term_color_never_disables_styling() {
        let set = StyleEnv {
            cargo_term_color: Some("never"),
            stdout_is_terminal: true,
            ..env()
        };
        assert_eq!(
            resolve(ColorChoice::Auto, set),
            (Style::PLAIN, StyleReason::CargoTermColorEnv)
        );
    }

    /// The regression this rule exists for: CI tooling (Swatinem/rust-cache)
    /// exports `CARGO_TERM_COLOR=always` so cargo's own logs are coloured.
    /// Treating that as permission to style our report put ANSI into a
    /// non-TTY CI stream and broke a consumer grepping for a literal result
    /// line. It must not enable styling.
    #[test]
    fn cargo_term_color_always_does_not_enable_styling_on_a_pipe() {
        let ci = StyleEnv {
            cargo_term_color: Some("always"),
            stdout_is_terminal: false,
            ..env()
        };
        assert_eq!(
            resolve(ColorChoice::Auto, ci),
            (Style::PLAIN, StyleReason::NotATerminal)
        );
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
    fn sanitize_leaves_ordinary_text_untouched() {
        assert_eq!(
            sanitize_terminal_text("crates/foo/src/lib.rs"),
            "crates/foo/src/lib.rs"
        );
        assert_eq!(sanitize_terminal_text("réason — ok"), "réason — ok");
    }

    /// Repository text must not be able to start a line that looks like the
    /// tool wrote it. A path containing a newline could otherwise forge a
    /// `new: unreceipted …` entry in a report someone acts on.
    #[test]
    fn sanitize_prevents_repository_text_from_forging_a_report_line() {
        let forged = "docs/a.md\nnew: unreceipted panic at evil.rs:1:1";
        let cleaned = sanitize_terminal_text(forged);
        assert!(!cleaned.contains('\n'), "no real newline may survive");
        assert_eq!(cleaned.lines().count(), 1, "must stay one line");
        assert!(
            cleaned.contains("\\n"),
            "the newline is shown, not acted on"
        );
        assert!(cleaned.contains("evil.rs"), "text stays visible");
    }

    #[test]
    fn sanitize_escapes_tab_and_carriage_return_visibly() {
        assert_eq!(sanitize_terminal_text("a\tb"), "a\\tb");
        assert_eq!(sanitize_terminal_text("a\rb"), "a\\rb");
    }

    #[test]
    fn sanitize_strips_c1_and_delete_controls() {
        assert!(!sanitize_terminal_text("a\u{9b}b").contains('\u{9b}'));
        assert!(!sanitize_terminal_text("a\u{7f}b").contains('\u{7f}'));
    }
}
