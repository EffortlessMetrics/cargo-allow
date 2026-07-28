use allow_core::AllowConfig;

use crate::render_entry::render_allow_entry;
use crate::render_sections::{
    render_lanes, render_policy_header, render_requirements, render_workspace,
};

pub fn render_policy(cfg: &AllowConfig) -> String {
    let mut out = String::new();
    render_policy_header(&mut out, cfg);
    render_workspace(&mut out, &cfg.workspace);
    render_requirements(&mut out, &cfg.requirements);
    render_lanes(&mut out, &cfg.lanes);
    for entry in &cfg.allow {
        render_allow_entry(&mut out, entry);
    }
    out
}
