use crate::{RustSourceCouplingKind, scan_rust_source_coupling};

#[test]
fn extracts_use_and_inline_module_paths_with_locations() -> Result<(), String> {
    let scan = scan_rust_source_coupling(
        "use intent_protocol::{Receipt, self as protocol};\nmod local {\n}\n",
    )
    .map_err(|error| format!("scan source coupling: {error}"))?;

    if scan.has_parse_error {
        return Err("valid coupling fixture parsed with errors".to_string());
    }
    let use_fact = scan
        .facts
        .iter()
        .find(|fact| fact.kind == RustSourceCouplingKind::UseDeclaration)
        .ok_or_else(|| "missing use coupling fact".to_string())?;
    if use_fact.path != "intent_protocol::{Receipt, self as protocol}"
        || use_fact.start_line != 1
        || use_fact.start_column != 1
    {
        return Err(format!("unexpected use fact: {use_fact:?}"));
    }
    let module_fact = scan
        .facts
        .iter()
        .find(|fact| fact.kind == RustSourceCouplingKind::InlineModule)
        .ok_or_else(|| "missing module coupling fact".to_string())?;
    if module_fact.path != "local" || module_fact.start_line != 2 {
        return Err(format!("unexpected module fact: {module_fact:?}"));
    }
    Ok(())
}

#[test]
fn preserves_parse_error_signal() -> Result<(), String> {
    let scan = scan_rust_source_coupling("use broken::{\n")
        .map_err(|error| format!("scan malformed source: {error}"))?;
    if !scan.has_parse_error {
        return Err("malformed source did not retain parse error signal".to_string());
    }
    Ok(())
}
