use super::refresh_policy_entry;
use allow_core::{CargoAllowErrorKind, LastSeen, SOURCE_FILE_READ_MAX_BYTES};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn entry() -> &'static str {
    "[[allow]]\nid='selected'\nkind='panic'\npath='src/lib.rs'\nowner='team'\nclassification='reviewed'\nreason='fixture'\nevidence=['test:refresh']\nreview_after='2099-01-01'\nselector={ast_kind='method_call',callee='unwrap',line_hint='99'}\n"
}

#[test]
fn bounded_values_preserve_quoted_headers_fake_headers_comments_and_legacy_hints() -> TestResult {
    let prefix = format!(
        "\u{feff}policy='cargo-allow'\r\nowner='custom'\nstatus='advisory'\r\n{}",
        entry()
    );
    // A multiline reason containing apparent tables must never drive selection.
    let prefix = prefix.replace(
        "reason='fixture'",
        "reason='''fixture\n[allow.last_seen]\nline=99\ncolumn=1'''",
    );
    let prefix = format!(
        "{prefix}# inert legacy hint above\r\n[ 'allow' . 'last_seen' ] # selected\r\n'line' = "
    );
    for (line, column, new_line, new_column) in [
        ("99", "1", "3", "50"),
        ("'099'", "'01'", "'3'", "'50'"),
        ("'+99'", "'+1'", "'3'", "'50'"),
        ("\"99\"", "\"1\"", "\"3\"", "\"50\""),
        ("+99", "0x01", "3", "50"),
    ] {
        let input = format!("{prefix}{line} # line\ncolumn = {column} # EOF");
        crate::parse_policy(&input)?;
        let expected = format!("{prefix}{new_line} # line\ncolumn = {new_column} # EOF");
        let actual = refresh_policy_entry(
            &input,
            "selected",
            &LastSeen {
                line: 3,
                column: 50,
            },
        )?;
        assert_eq!(actual, expected);
        assert_eq!(
            refresh_policy_entry(
                &actual,
                "selected",
                &LastSeen {
                    line: 3,
                    column: 50
                }
            )?,
            actual
        );
    }
    Ok(())
}

#[test]
fn unchanged_coordinate_keeps_its_lexical_bytes() -> TestResult {
    let prefix = format!(
        "policy='cargo-allow'\n{}[allow.last_seen]\nline=99\ncolumn=",
        entry()
    );
    let input = format!("{prefix}'0001' # EOF");
    let expected = format!(
        "policy='cargo-allow'\n{}[allow.last_seen]\nline=3\ncolumn='0001' # EOF",
        entry()
    );
    assert_eq!(
        refresh_policy_entry(&input, "selected", &LastSeen { line: 3, column: 1 })?,
        expected
    );
    Ok(())
}

fn refuses(input: &str, id: &str) -> TestResult {
    let original = input.as_bytes().to_vec();
    let Err(error) = refresh_policy_entry(
        input,
        id,
        &LastSeen {
            line: 3,
            column: 50,
        },
    ) else {
        return Err("unsupported policy unexpectedly refreshed".into());
    };
    assert_eq!(error.kind(), CargoAllowErrorKind::InvalidPolicy);
    assert_eq!(input.as_bytes(), original);
    Ok(())
}

#[test]
fn unsupported_ambiguous_and_invalid_shapes_refuse() -> TestResult {
    let explicit = format!(
        "policy='cargo-allow'\n{}[allow.last_seen]\nline=99\ncolumn=1\n",
        entry()
    );
    for tail in [
        "[allow.last_seen]\nline=\"\"\"99\"\"\"\ncolumn=1",
        "[allow.last_seen]\nline=\"\\u0039\\u0039\"\ncolumn=1",
        "[allow.last_seen]\nline=99\ncolumn=1\nline=88",
        "[allow.last_seen]\nline=99",
        "[allow.last_seen]\nline=99\ncolumn=1\ncustom=42",
        "last_seen={line=99,column=1}",
        "last_seen.line=99\nlast_seen.column=1",
        "",
    ] {
        let input = format!("policy='cargo-allow'\n{}{tail}", entry());
        refuses(&input, "selected")?;
    }
    let inline = "policy='cargo-allow'\nallow=[{id='selected',kind='panic',path='src/lib.rs',owner='team',classification='reviewed',reason='fixture',review_after='2099-01-01',selector={ast_kind='method_call',callee='unwrap'},last_seen={line=99,column=1}}]";
    crate::parse_policy(inline)?;
    refuses(inline, "selected")?;
    refuses(&explicit, "missing")?;
    refuses(&format!("{explicit}\n{}", entry()), "selected")?;
    Ok(())
}

#[test]
fn loader_cap_applies_to_preimage_and_grown_result() -> TestResult {
    let prefix = format!(
        "policy='cargo-allow'\n{}[allow.last_seen]\nline=1\ncolumn=1\n#",
        entry()
    );
    let pad = SOURCE_FILE_READ_MAX_BYTES as usize - prefix.len();
    let input = format!("{prefix}{}", "x".repeat(pad));
    let original = input.as_bytes().to_vec();
    assert_eq!(
        refresh_policy_entry(&input, "selected", &LastSeen { line: 1, column: 1 })?,
        input
    );
    refuses(&input, "selected")?;
    assert_eq!(input.as_bytes(), original);
    refuses(&format!("{input}x"), "selected")?;
    let shorter = format!("{prefix}{}", "x".repeat(pad - 1));
    let result = refresh_policy_entry(
        &shorter,
        "selected",
        &LastSeen {
            line: 3,
            column: 50,
        },
    )?;
    assert_eq!(result.len() as u64, SOURCE_FILE_READ_MAX_BYTES);
    Ok(())
}
