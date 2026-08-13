use crate::{RustSourceCouplingKind, RustSourceCouplingPathBase, scan_rust_source_coupling};

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
fn extracts_compile_time_path_reads_and_marks_ambiguous_arguments() -> Result<(), String> {
    let scan = scan_rust_source_coupling(
        "include!(\"owned.rs\");\ninclude_str!(r#\"../../shared/src/lib.rs\"#);\ninclude_bytes!(concat!(\"unknown\", \".rs\"));\n",
    )
    .map_err(|error| format!("scan path reads: {error}"))?;
    let reads: Vec<_> = scan
        .facts
        .iter()
        .filter(|fact| fact.kind == RustSourceCouplingKind::PathRead)
        .collect();
    let paths: Vec<_> = reads.iter().map(|fact| fact.path.as_str()).collect();
    if paths != ["owned.rs", "../../shared/src/lib.rs", ""]
        || reads.iter().map(|fact| fact.start_line).collect::<Vec<_>>() != [1, 2, 3]
    {
        return Err(format!("unexpected path-read facts: {reads:?}"));
    }
    Ok(())
}

#[test]
fn extracts_manifest_directory_concat_path_reads() -> Result<(), String> {
    let scan = scan_rust_source_coupling(
        "include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/../shared/\", \"public.rs\"));\n",
    )
    .map_err(|error| format!("scan concat path read: {error}"))?;
    let fact = scan
        .facts
        .iter()
        .find(|fact| fact.kind == RustSourceCouplingKind::PathRead)
        .ok_or_else(|| "missing concat path-read fact".to_string())?;
    if fact.path != "/../shared/public.rs"
        || fact.path_base != RustSourceCouplingPathBase::ManifestDirectory
    {
        return Err(format!("unexpected concat path-read fact: {fact:?}"));
    }
    Ok(())
}

#[test]
fn manifest_concat_reconstructs_fragments_after_the_manifest_directory() -> Result<(), String> {
    let scan = scan_rust_source_coupling(
        "include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), r#\"/assets/\"#, \"schema.json\"));\n",
    )
    .map_err(|error| format!("scan fragment concat: {error}"))?;
    let fact = scan
        .facts
        .iter()
        .find(|fact| fact.kind == RustSourceCouplingKind::PathRead)
        .ok_or_else(|| "missing fragment concat fact".to_string())?;
    if fact.path != "/assets/schema.json"
        || fact.path_base != RustSourceCouplingPathBase::ManifestDirectory
    {
        return Err(format!("unexpected fragment concat fact: {fact:?}"));
    }
    Ok(())
}

#[test]
fn manifest_concat_rejects_other_environment_and_dynamic_fragments() -> Result<(), String> {
    let scan = scan_rust_source_coupling(
        "include_str!(concat!(env!(\"OTHER_DIR\"), \"/schema.json\"));\ninclude_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), path!()));\ninclude_str!(concat!(\"prefix/\", env!(\"CARGO_MANIFEST_DIR\"), \"/schema.json\"));\ninclude_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), env!(\"CARGO_MANIFEST_DIR\"), \"/schema.json\"));\n",
    )
    .map_err(|error| format!("scan invalid manifest concat: {error}"))?;
    let reads: Vec<_> = scan
        .facts
        .iter()
        .filter(|fact| fact.kind == RustSourceCouplingKind::PathRead)
        .collect();
    if reads.len() != 4 || reads.iter().any(|fact| !fact.path.is_empty()) {
        return Err(format!("invalid concat fragments resolved: {reads:?}"));
    }
    Ok(())
}

#[test]
fn direct_path_containing_manifest_name_stays_source_relative() -> Result<(), String> {
    let scan = scan_rust_source_coupling(
        "include_str!(\"CARGO_MANIFEST_DIR.txt\");\ninclude_str!(/* concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/decoy.rs\") */ \"owned.txt\");\n",
    )
    .map_err(|error| format!("scan direct manifest-named path: {error}"))?;
    let reads: Vec<_> = scan
        .facts
        .iter()
        .filter(|fact| fact.kind == RustSourceCouplingKind::PathRead)
        .collect();
    if reads.len() != 2
        || reads[0].path != "CARGO_MANIFEST_DIR.txt"
        || reads[1].path != "owned.txt"
        || reads
            .iter()
            .any(|fact| fact.path_base != RustSourceCouplingPathBase::SourceFile)
    {
        return Err(format!("unexpected direct manifest-named paths: {reads:?}"));
    }
    Ok(())
}

#[test]
fn manifest_concat_uses_the_structural_argument_and_all_delimiters() -> Result<(), String> {
    let scan = scan_rust_source_coupling(
        "include_str!(/* concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/decoy.rs\") */ concat!{env![\"CARGO_MANIFEST_DIR\"], \"/actual.rs\"});\ninclude_str!(concat![env!{\"CARGO_MANIFEST_DIR\"}, \"/bracket.rs\"]);\n",
    )
    .map_err(|error| format!("scan structural concat arguments: {error}"))?;
    let reads: Vec<_> = scan
        .facts
        .iter()
        .filter(|fact| fact.kind == RustSourceCouplingKind::PathRead)
        .collect();
    if reads.len() != 2
        || reads[0].path != "/actual.rs"
        || reads[1].path != "/bracket.rs"
        || reads
            .iter()
            .any(|fact| fact.path_base != RustSourceCouplingPathBase::ManifestDirectory)
    {
        return Err(format!("unexpected structural concat paths: {reads:?}"));
    }
    Ok(())
}

#[test]
fn manifest_concat_ignores_delimiters_inside_literals() -> Result<(), String> {
    let scan = scan_rust_source_coupling(
        "include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/fixtures/)comma,\", r#\"raw,)ok.txt\"#));\n",
    )
    .map_err(|error| format!("scan delimiter concat: {error}"))?;
    let fact = scan
        .facts
        .iter()
        .find(|fact| fact.kind == RustSourceCouplingKind::PathRead)
        .ok_or_else(|| "missing delimiter concat fact".to_string())?;
    if fact.path != "/fixtures/)comma,raw,)ok.txt"
        || fact.path_base != RustSourceCouplingPathBase::ManifestDirectory
    {
        return Err(format!("unexpected delimiter concat fact: {fact:?}"));
    }
    Ok(())
}

#[test]
fn path_reads_fail_closed_for_dynamic_and_escaped_literals() -> Result<(), String> {
    let scan = scan_rust_source_coupling(
        "include!(env!(\"OTHER_DIR\"));\ninclude!(\"dir\\\\file.rs\");\ninclude!(concat!(path!(), \"owned.rs\"));\n",
    )
    .map_err(|error| format!("scan ambiguous path reads: {error}"))?;
    let reads: Vec<_> = scan
        .facts
        .iter()
        .filter(|fact| fact.kind == RustSourceCouplingKind::PathRead)
        .collect();
    if reads.len() != 3 || reads.iter().any(|fact| !fact.path.is_empty()) {
        return Err(format!(
            "ambiguous path reads were not unresolved: {reads:?}"
        ));
    }
    Ok(())
}

#[test]
fn handles_empty_and_unscoped_use_lists() -> Result<(), String> {
    let scan = scan_rust_source_coupling("use broken::{,};\nuse {product_b::item};\n")
        .map_err(|error| format!("scan edge-case use lists: {error}"))?;
    let paths: Vec<_> = scan.facts.iter().map(|fact| fact.path.as_str()).collect();
    if scan.has_parse_error || paths != ["product_b::item"] {
        return Err(format!(
            "unexpected edge-case facts: parse_error={}, paths={paths:?}",
            scan.has_parse_error
        ));
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
