#[derive(Debug, Clone, Copy)]
pub(super) struct DoctorFacts<'a> {
    pub(super) source_tree_root: &'a str,
    pub(super) root_discovery: &'a str,
    pub(super) config_path: Option<&'a str>,
    pub(super) inventory_source: &'a str,
    pub(super) files_scanned: usize,
}

pub(super) fn render_doctor_human(facts: DoctorFacts<'_>) -> String {
    let mut out = String::new();
    out.push_str(&format!("source tree root: {}\n", facts.source_tree_root));
    out.push_str(&format!("root discovery: {}\n", facts.root_discovery));
    match facts.config_path {
        Some(path) => out.push_str(&format!("config: {path}\n")),
        None => out.push_str("config: not found; run `cargo-allow init`\n"),
    }
    out.push_str(&format!(
        "inventory: source_tree/source_syntax via {}; files scanned: {}\n",
        facts.inventory_source, facts.files_scanned
    ));
    out.push_str(allow_report::CLAIM_BOUNDARY_TEXT);
    out
}

pub(super) fn render_doctor_json(facts: DoctorFacts<'_>) -> String {
    allow_report::render_doctor_json(allow_report::DoctorReport {
        source_tree_root: facts.source_tree_root,
        root_discovery: facts.root_discovery,
        config_path: facts.config_path,
        inventory_source: facts.inventory_source,
        files_scanned: facts.files_scanned,
    })
}
