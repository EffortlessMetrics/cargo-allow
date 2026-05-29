use allow_core::AllowConfig;

use crate::render::render_policy;

pub fn starter_policy(strict: bool) -> String {
    let mut cfg = AllowConfig::empty();
    cfg.owner = Some("core/policy".to_string());
    if strict {
        cfg.workspace.default_mode = "strict".to_string();
        cfg.requirements.stale_entries_fail = true;
    }
    render_policy(&cfg)
}
