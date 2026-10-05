use super::prune_policy_entries;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn require_invalid_policy(input: &str, ids: &[&str]) -> TestResult {
    let Err(error) = prune_policy_entries(input, ids) else {
        return Err("policy refusal unexpectedly succeeded".into());
    };
    assert_eq!(error.kind(), allow_core::CargoAllowErrorKind::InvalidPolicy);
    Ok(())
}

fn entry(id: &str) -> String {
    format!(
        "[[allow]]\nid = '{id}'\nkind = 'panic'\npath = 'src/lib.rs'\nowner = 'core'\nclassification = 'reviewed'\nreason = 'fixture'\nexpires = '2027-01-01'\n[allow.selector]\nast_kind = 'method_call'\ncallee = 'unwrap'\n"
    )
}

#[test]
fn quoted_headers_multiline_strings_and_nested_values_preserve_survivors() -> TestResult {
    let prefix = "\u{feff}# historical\r\npolicy = 'cargo-allow'\r\n# next entry\n";
    let selected = entry("allow-selected")
        .replace("[[allow]]", "[[ 'allow' ]] # selected")
        .replace("[allow.selector]", "[ 'allow' . 'selector' ]")
        .replace(
            "reason = 'fixture'",
            "reason = '''fixture\n[[allow]]\nid = 'fake'\n[workspace]\nstill a string'''",
        )
        .replace("\n", "\r\n");
    let suffix = format!(
        "\n# 保持 unrelated comment\n{}# EOF",
        entry("allow-survivor")
    );
    let input = format!("{prefix}{selected}{suffix}");
    let result = prune_policy_entries(&input, &["allow-selected"])?;
    assert_eq!(result, format!("{prefix}{suffix}"));
    assert_eq!(prune_policy_entries(&result, &[])?, result);
    require_invalid_policy(&result, &["allow-selected"])?;
    Ok(())
}

#[test]
fn multiple_selected_entries_and_last_line_without_newline() -> TestResult {
    let prefix = "policy = 'cargo-allow'\n# retained\n";
    let first = entry("allow-first");
    let survivor = entry("allow-survivor");
    let last = entry("allow-last");
    let input = format!("{prefix}{first}{survivor}{}", last.trim_end());
    assert_eq!(
        prune_policy_entries(&input, &["allow-last", "allow-first"])?,
        format!("{prefix}{survivor}")
    );
    assert_eq!(
        prune_policy_entries(&input, &["allow-last", "allow-first", "allow-survivor"])?,
        prefix
    );
    Ok(())
}

#[test]
fn unrelated_interleaved_empty_table_refuses_instead_of_disappearing() -> TestResult {
    let selected =
        entry("allow-selected").replace("[allow.selector]", "[workspace]\n[allow.selector]");
    let input = format!("policy = 'cargo-allow'\n{selected}");
    crate::parse_policy(&input)?;
    require_invalid_policy(&input, &["allow-selected"])?;
    Ok(())
}

#[test]
fn inline_arrays_duplicate_unknown_and_oversized_input_refuse() -> TestResult {
    let inline = "policy = 'cargo-allow'\nallow = [{id='allow-one', kind='panic', path='src/lib.rs', owner='core', classification='reviewed', reason='fixture', expires='2027-01-01', selector={ast_kind='method_call', callee='unwrap'}}]\n";
    crate::parse_policy(inline)?;
    require_invalid_policy(inline, &["allow-one"])?;
    let input = format!("policy = 'cargo-allow'\n{}", entry("allow-one"));
    require_invalid_policy(&input, &["allow-one", "allow-one"])?;
    require_invalid_policy(&input, &["allow-missing"])?;
    let oversized = " ".repeat(allow_core::SOURCE_FILE_READ_MAX_BYTES as usize + 1);
    require_invalid_policy(&oversized, &[])?;
    Ok(())
}

#[test]
fn implicit_ids_refuse_survivor_identity_drift() -> TestResult {
    let implicit = entry("unused").replace("id = 'unused'\n", "");
    let prefix = "policy = 'cargo-allow'\n";
    let input = format!("{prefix}{implicit}{implicit}");
    require_invalid_policy(&input, &["allow-0001"])?;
    assert_eq!(
        prune_policy_entries(&input, &["allow-0002"])?,
        format!("{prefix}{implicit}")
    );
    Ok(())
}
