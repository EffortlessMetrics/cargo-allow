use allow_core::AllowConfig;

use crate::ledger_self_receipt::ledger_self_receipt;
use crate::render::render_policy;

/// Owner recorded on the starter ledger and on its self-receipt.
const STARTER_OWNER: &str = "core/policy";

/// Render a starter policy for a ledger that will live at `policy_rel_path`.
///
/// The ledger receipts itself: once committed, the policy file is a tracked
/// non-Rust file and appears in its own inventory, so without this entry the
/// first `check --mode no-new` after adoption fails on `policy/allow.toml`
/// rather than on anything the adopter wrote (#3032).
pub fn starter_policy(strict: bool, policy_rel_path: &str) -> String {
    let mut cfg = AllowConfig::empty();
    cfg.owner = Some(STARTER_OWNER.to_string());
    if strict {
        cfg.workspace.default_mode = "strict".to_string();
        cfg.requirements.stale_entries_fail = true;
    }
    cfg.allow.push(ledger_self_receipt(
        "allow-0001",
        policy_rel_path,
        STARTER_OWNER,
    ));
    render_policy(&cfg)
}
