#[derive(Debug, Clone, Copy)]
pub(super) struct DoctorFacts<'a> {
    pub(super) source_tree_root: &'a str,
    pub(super) root_discovery: &'a str,
    pub(super) config_path: Option<&'a str>,
    pub(super) inventory_source: &'a str,
    pub(super) files_scanned: usize,
}

pub(super) fn render_doctor_human(facts: DoctorFacts<'_>) -> String {
    allow_report::render_doctor_human(doctor_report(facts))
}

pub(super) fn render_doctor_json(facts: DoctorFacts<'_>) -> String {
    allow_report::render_doctor_json(doctor_report(facts))
}

fn doctor_report(facts: DoctorFacts<'_>) -> allow_report::DoctorReport<'_> {
    allow_report::DoctorReport {
        source_tree_root: facts.source_tree_root,
        root_discovery: facts.root_discovery,
        config_path: facts.config_path,
        inventory_source: facts.inventory_source,
        files_scanned: facts.files_scanned,
    }
}
