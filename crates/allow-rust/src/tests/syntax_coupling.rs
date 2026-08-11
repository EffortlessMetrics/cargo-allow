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
    let use_facts: Vec<_> = scan
        .facts
        .iter()
        .filter(|fact| fact.kind == RustSourceCouplingKind::UseDeclaration)
        .collect();
    let use_paths: Vec<_> = use_facts.iter().map(|fact| fact.path.as_str()).collect();
    if use_paths != ["intent_protocol::Receipt", "intent_protocol::self"]
        || use_facts
            .iter()
            .any(|fact| fact.start_line != 1 || fact.start_column != 1)
    {
        return Err(format!("unexpected use facts: {use_facts:?}"));
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
fn extracts_each_top_level_path_from_grouped_use() -> Result<(), String> {
    let scan = scan_rust_source_coupling(
        "use {product_b::private_api, product_c::other_api};\nuse product_b::{a, b};\n",
    )
    .map_err(|error| format!("scan grouped use: {error}"))?;
    let paths: Vec<_> = scan.facts.iter().map(|fact| fact.path.as_str()).collect();
    if paths
        != [
            "product_b::private_api",
            "product_c::other_api",
            "product_b::a",
            "product_b::b",
        ]
    {
        return Err(format!("unexpected grouped paths: {paths:?}"));
    }
    Ok(())
}

#[test]
fn ignores_out_of_line_modules_when_extracting_inline_modules() -> Result<(), String> {
    let scan = scan_rust_source_coupling("mod external;\nmod inline {}\n")
        .map_err(|error| format!("scan modules: {error}"))?;
    let paths: Vec<_> = scan.facts.iter().map(|fact| fact.path.as_str()).collect();
    if paths != ["inline"] {
        return Err(format!("unexpected module paths: {paths:?}"));
    }
    Ok(())
}

#[test]
fn handles_empty_and_unscoped_use_lists() -> Result<(), String> {
    let scan = scan_rust_source_coupling("use broken::{,};\nuse {product_b::item};\n")
        .map_err(|error| format!("scan edge-case use lists: {error}"))?;
    if scan.facts.is_empty() {
        return Err("edge-case use lists produced no facts".to_string());
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
