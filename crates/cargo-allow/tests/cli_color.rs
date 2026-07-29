//! Black-box coverage for `--color` semantics and terminal safety (#2572).
//!
//! The flag was advertised and discarded, so these tests assert observable
//! bytes rather than internal state: either an ANSI escape is present in the
//! process output or it is not.
//!
//! Tests run with stdout captured, i.e. not a terminal, which is exactly the
//! `auto`-should-stay-plain case. `always` is therefore the only way to
//! observe styling here, and that is the point — a CI log must never be
//! styled by accident.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

const ESC: char = '\u{1b}';

fn run(root: &Path, envs: &[(&str, &str)], args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-allow"));
    command.current_dir(root);
    // Clear inherited colour variables so a developer's shell cannot change
    // the result of these assertions.
    for key in ["NO_COLOR", "CLICOLOR_FORCE", "CARGO_TERM_COLOR"] {
        command.env_remove(key);
    }
    for (key, value) in envs {
        command.env(key, value);
    }
    command
        .args(args)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow {args:?}: {err}")))
}

fn stdout_of(result: &Output) -> String {
    String::from_utf8_lossy(&result.stdout).into_owned()
}

fn has_ansi(text: &str) -> bool {
    text.contains(ESC)
}

fn git(root: &Path, args: &[&str]) {
    let out = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("git {args:?}: {err}")));
    assert!(out.status.success(), "git {args:?} should succeed");
}

/// A repository that passes, so the styled path renders a result line.
fn fixture(label: &str) -> std::path::PathBuf {
    let root =
        std::env::temp_dir().join(format!("cargo-allow-color-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create fixture: {err}")));
    fs::write(root.join("src/lib.rs"), "pub fn ok() -> u32 {\n    1\n}\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write source: {err}")));

    git(&root, &["init"]);
    git(&root, &["config", "user.email", "t@example.invalid"]);
    git(&root, &["config", "user.name", "cargo-allow test"]);

    assert!(run(&root, &[], &["init"]).status.success(), "init");
    git(&root, &["add", "."]);

    let receipt = run(
        &root,
        &[],
        &[
            "add",
            "--update",
            "--kind",
            "non-rust",
            "--glob",
            "policy/*.toml",
            "--owner",
            "core/test",
            "--reason",
            "fixture policy file for colour characterization",
        ],
    );
    assert!(receipt.status.success(), "receipting policy should succeed");
    git(&root, &["add", "."]);

    root
}

/// auto on a non-TTY is plain; never is plain; always styles.
#[test]
fn the_three_choices_have_observable_behaviour() {
    let root = fixture("choices");

    let auto = stdout_of(&run(
        &root,
        &[],
        &["check", "--mode", "no-new", "--color", "auto"],
    ));
    let never = stdout_of(&run(
        &root,
        &[],
        &["check", "--mode", "no-new", "--color", "never"],
    ));
    let always = stdout_of(&run(
        &root,
        &[],
        &["check", "--mode", "no-new", "--color", "always"],
    ));

    assert!(!has_ansi(&auto), "auto on a pipe must stay plain");
    assert!(!has_ansi(&never), "never must stay plain");
    assert!(has_ansi(&always), "always must style even on a pipe");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn list_human_statuses_use_shared_style_but_files_stay_plain() {
    let root = fixture("list");
    let plain_result = run(&root, &[], &["list", "--color", "never"]);
    let styled_result = run(&root, &[], &["list", "--color", "always"]);

    assert!(plain_result.status.success(), "plain list should succeed");
    assert!(styled_result.status.success(), "styled list should succeed");
    assert!(!has_ansi(&stdout_of(&plain_result)));
    assert!(has_ansi(&stdout_of(&styled_result)));

    let json_result = run(
        &root,
        &[],
        &["list", "--color", "always", "--format", "json"],
    );
    assert!(json_result.status.success(), "JSON list should succeed");
    assert!(
        !has_ansi(&stdout_of(&json_result)),
        "JSON list must stay plain"
    );

    fs::create_dir_all(root.join("target/cargo-allow"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create list output dir: {err}")));
    let written = run(
        &root,
        &[],
        &[
            "list",
            "--color",
            "always",
            "--output",
            "target/cargo-allow/list.txt",
        ],
    );
    assert!(written.status.success(), "written list should succeed");
    let text = fs::read_to_string(root.join("target/cargo-allow/list.txt"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read list output: {err}")));
    assert!(!has_ansi(&text), "written list output must stay plain");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn explain_human_statuses_use_shared_style_but_files_stay_plain() {
    let root = fixture("explain");
    let plain_result = run(&root, &[], &["explain", "allow-0001", "--color", "never"]);
    let styled_result = run(&root, &[], &["explain", "allow-0001", "--color", "always"]);

    assert!(
        plain_result.status.success(),
        "plain explain should succeed"
    );
    assert!(
        styled_result.status.success(),
        "styled explain should succeed"
    );
    assert!(!has_ansi(&stdout_of(&plain_result)));
    assert!(has_ansi(&stdout_of(&styled_result)));

    let json_result = run(
        &root,
        &[],
        &[
            "explain",
            "allow-0001",
            "--color",
            "always",
            "--format",
            "json",
        ],
    );
    assert!(json_result.status.success(), "JSON explain should succeed");
    assert!(
        !has_ansi(&stdout_of(&json_result)),
        "JSON explain must stay plain"
    );

    fs::create_dir_all(root.join("target/cargo-allow"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create explain output dir: {err}")));
    let written = run(
        &root,
        &[],
        &[
            "explain",
            "allow-0001",
            "--color",
            "always",
            "--output",
            "target/cargo-allow/explain.txt",
        ],
    );
    assert!(written.status.success(), "written explain should succeed");
    let text = fs::read_to_string(root.join("target/cargo-allow/explain.txt"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read explain output: {err}")));
    assert!(!has_ansi(&text), "written explain output must stay plain");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn why_human_statuses_use_shared_style_but_files_stay_plain() {
    let root = fixture("why");
    fs::write(
        root.join("src/lib.rs"),
        "pub fn fail(value: Option<u8>) -> u8 {\n    value.unwrap()\n}\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write why source: {err}")));

    let plain_result = run(
        &root,
        &[],
        &[
            "why",
            "--kind",
            "panic",
            "--path",
            "src/lib.rs",
            "--line",
            "2",
            "--color",
            "never",
        ],
    );
    let styled_result = run(
        &root,
        &[],
        &[
            "why",
            "--kind",
            "panic",
            "--path",
            "src/lib.rs",
            "--line",
            "2",
            "--color",
            "always",
        ],
    );
    assert!(plain_result.status.success(), "plain why should succeed");
    assert!(styled_result.status.success(), "styled why should succeed");
    assert!(!has_ansi(&stdout_of(&plain_result)));
    assert!(has_ansi(&stdout_of(&styled_result)));

    let json_result = run(
        &root,
        &[],
        &[
            "why",
            "--kind",
            "panic",
            "--path",
            "src/lib.rs",
            "--line",
            "2",
            "--color",
            "always",
            "--format",
            "json",
        ],
    );
    assert!(json_result.status.success(), "JSON why should succeed");
    assert!(
        !has_ansi(&stdout_of(&json_result)),
        "JSON why must stay plain"
    );

    fs::create_dir_all(root.join("target/cargo-allow"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create why output dir: {err}")));
    let output_path = root.join("target/cargo-allow/why.txt");
    let written = run(
        &root,
        &[],
        &[
            "why",
            "--kind",
            "panic",
            "--path",
            "src/lib.rs",
            "--line",
            "2",
            "--color",
            "always",
            "--output",
            "target/cargo-allow/why.txt",
        ],
    );
    assert!(written.status.success(), "written why should succeed");
    let text = fs::read_to_string(output_path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read why output: {err}")));
    assert!(!has_ansi(&text), "written why output must stay plain");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn worklist_human_statuses_use_shared_style_but_files_stay_plain() {
    let root = fixture("worklist");
    let plain_result = run(&root, &[], &["worklist", "--color", "never"]);
    let styled_result = run(&root, &[], &["worklist", "--color", "always"]);

    assert!(
        plain_result.status.success(),
        "plain worklist should succeed"
    );
    assert!(
        styled_result.status.success(),
        "styled worklist should succeed"
    );
    assert!(!has_ansi(&stdout_of(&plain_result)));
    assert!(has_ansi(&stdout_of(&styled_result)));

    let json_result = run(
        &root,
        &[],
        &["worklist", "--color", "always", "--format", "json"],
    );
    assert!(json_result.status.success(), "JSON worklist should succeed");
    assert!(
        !has_ansi(&stdout_of(&json_result)),
        "JSON worklist must stay plain"
    );

    fs::create_dir_all(root.join("target/cargo-allow"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create worklist output dir: {err}")));
    let written = run(
        &root,
        &[],
        &[
            "worklist",
            "--color",
            "always",
            "--output",
            "target/cargo-allow/worklist.txt",
        ],
    );
    assert!(written.status.success(), "written worklist should succeed");
    let text = fs::read_to_string(root.join("target/cargo-allow/worklist.txt"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read worklist output: {err}")));
    assert!(!has_ansi(&text), "written worklist output must stay plain");

    let _ = fs::remove_dir_all(&root);
}

/// The documented precedence, asserted end to end rather than only in the
/// unit test of the resolver.
#[test]
fn environment_precedence_matches_the_documented_law() {
    let root = fixture("env");
    let check = ["check", "--mode", "no-new"];

    // NO_COLOR disables auto...
    let no_color = stdout_of(&run(&root, &[("NO_COLOR", "1")], &check));
    assert!(!has_ansi(&no_color), "NO_COLOR must disable auto");

    // ...but an explicit --color always is a direct instruction and wins.
    let forced = stdout_of(&run(
        &root,
        &[("NO_COLOR", "1")],
        &["check", "--mode", "no-new", "--color", "always"],
    ));
    assert!(
        has_ansi(&forced),
        "explicit --color always outranks NO_COLOR"
    );

    // CLICOLOR_FORCE turns styling on for a pipe...
    let clicolor = stdout_of(&run(&root, &[("CLICOLOR_FORCE", "1")], &check));
    assert!(has_ansi(&clicolor), "CLICOLOR_FORCE must force styling");

    // ...but NO_COLOR outranks it, which is this tool's documented deviation
    // from the clicolor convention.
    let both = stdout_of(&run(
        &root,
        &[("NO_COLOR", "1"), ("CLICOLOR_FORCE", "1")],
        &check,
    ));
    assert!(!has_ansi(&both), "NO_COLOR must outrank CLICOLOR_FORCE");

    // CARGO_TERM_COLOR can only disable. CI tooling exports
    // `CARGO_TERM_COLOR=always` for cargo's own logs; treating that as
    // permission to style our report put ANSI into a non-TTY CI stream and
    // broke a consumer grepping for a literal result line.
    let cargo_always = stdout_of(&run(&root, &[("CARGO_TERM_COLOR", "always")], &check));
    let cargo_never = stdout_of(&run(
        &root,
        &[("CARGO_TERM_COLOR", "never"), ("CLICOLOR_FORCE", "0")],
        &check,
    ));
    assert!(
        !has_ansi(&cargo_always),
        "CARGO_TERM_COLOR=always must not enable styling on a pipe"
    );
    assert!(
        !has_ansi(&cargo_never),
        "CARGO_TERM_COLOR=never stays plain"
    );

    // The exact regression: a consumer grepping the literal result line must
    // keep working under the environment CI actually runs with.
    assert!(
        cargo_always.contains("Result: passed (enforcing)"),
        "the literal result line must survive CI's CARGO_TERM_COLOR=always"
    );

    // An unrecognised value must not silently change output.
    let odd = stdout_of(&run(&root, &[("CARGO_TERM_COLOR", "chartreuse")], &check));
    assert!(
        !has_ansi(&odd),
        "an unknown value falls through to capability"
    );

    let _ = fs::remove_dir_all(&root);
}

/// The law that matters most: machine formats are byte-stable and ANSI-free
/// no matter what the user asked for.
#[test]
fn machine_formats_are_never_styled_even_with_color_always() {
    let root = fixture("machine");

    for format in ["json", "sarif", "markdown", "html"] {
        let out = stdout_of(&run(
            &root,
            &[("CLICOLOR_FORCE", "1")],
            &[
                "check", "--mode", "no-new", "--color", "always", "--format", format,
            ],
        ));
        assert!(!has_ansi(&out), "{format} output must never contain ANSI");
    }

    let _ = fs::remove_dir_all(&root);
}

/// Written artifacts stay portable: a report or receipt committed to a repo
/// or attached to CI must not carry escapes.
#[test]
fn written_files_and_receipts_are_never_styled() {
    let root = fixture("files");
    fs::create_dir_all(root.join("target/cargo-allow"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create artifact dir: {err}")));

    let human = run(
        &root,
        &[],
        &[
            "check",
            "--mode",
            "no-new",
            "--color",
            "always",
            "--output",
            "target/cargo-allow/report.txt",
        ],
    );
    assert!(
        human.status.success(),
        "writing a human report should succeed"
    );
    let written =
        fs::read_to_string(root.join("target/cargo-allow/report.txt")).unwrap_or_default();
    assert!(!written.is_empty(), "the report file should have content");
    assert!(!has_ansi(&written), "--output files must stay plain");

    let receipt_run = run(
        &root,
        &[],
        &[
            "check",
            "--mode",
            "no-new",
            "--color",
            "always",
            "--receipt",
            "target/cargo-allow/check.receipt.json",
        ],
    );
    assert!(
        receipt_run.status.success(),
        "writing a receipt should succeed"
    );
    let receipt =
        fs::read_to_string(root.join("target/cargo-allow/check.receipt.json")).unwrap_or_default();
    assert!(!has_ansi(&receipt), "receipts must stay plain");
    assert!(
        serde_json::from_str::<serde_json::Value>(&receipt).is_ok(),
        "the receipt must remain valid JSON"
    );

    let _ = fs::remove_dir_all(&root);
}

/// Styling must add escapes without changing a single word, so nothing is
/// conveyed by colour alone and plain output loses no meaning.
#[test]
fn styled_and_plain_output_carry_identical_text() {
    let root = fixture("equal");

    let plain = stdout_of(&run(
        &root,
        &[],
        &["check", "--mode", "no-new", "--color", "never"],
    ));
    let styled = stdout_of(&run(
        &root,
        &[],
        &["check", "--mode", "no-new", "--color", "always"],
    ));

    let stripped: String = strip_ansi(&styled);
    assert_eq!(
        stripped, plain,
        "with escapes removed, styled output must be byte-identical to plain"
    );

    let _ = fs::remove_dir_all(&root);
}

/// Repository-controlled text must not reach the terminal as control
/// sequences. A path containing an escape is the injection vector.
///
/// This is the end-to-end confirmation, not the guarantee. `allow-report`'s
/// `style` module unit-tests `sanitize_terminal_text` directly and those run
/// on every platform. This test additionally needs a filesystem that permits
/// control characters in filenames — Linux does, Windows does not — so on
/// Windows it exits early and the unit tests carry the coverage alone.
#[test]
fn repository_controlled_text_cannot_inject_escapes() {
    let root = fixture("injection");

    // A tracked file whose *name* carries an escape sequence. The scanner
    // reports the path, so an unsanitised renderer would replay it.
    let hostile = root.join("src/pwn\u{1b}[31m.rs");
    if fs::write(&hostile, "pub fn x() {}\n").is_err() {
        let _ = fs::remove_dir_all(&root);
        return;
    }
    git(&root, &["add", "."]);

    for color in ["never", "always"] {
        let out = stdout_of(&run(
            &root,
            &[],
            &["check", "--mode", "no-new", "--color", color],
        ));
        // With --color always the tool emits its own escapes; those are the
        // only ones allowed. Strip them, then assert nothing escape-like
        // survived from the path itself.
        let stripped = strip_ansi(&out);
        assert!(
            !stripped.contains(ESC),
            "--color {color}: repository text must not inject escapes"
        );
    }

    let _ = fs::remove_dir_all(&root);
}

/// Remove SGR sequences the tool itself emitted.
fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(character) = chars.next() {
        if character != ESC {
            out.push(character);
            continue;
        }
        // Skip `[` … `m`. Anything else is left intact for the caller to
        // notice — including the character we had to consume to find out,
        // which must not be swallowed or a lone ESC would eat the byte after
        // it and weaken the injection assertion.
        let next = chars.next();
        if next != Some('[') {
            out.push(character);
            if let Some(consumed) = next {
                out.push(consumed);
            }
            continue;
        }
        for inner in chars.by_ref() {
            if inner == 'm' {
                break;
            }
        }
    }
    out
}
