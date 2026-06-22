use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrainWindow {
    pub mirror_ledger: String,
    pub drain_owner: String,
    pub drain_reason: String,
    pub review_after: String,
    pub expiry: Option<String>,
    pub linked_closeout: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DrainWindowToml {
    mirror_ledger: String,
    drain_owner: String,
    drain_reason: String,
    review_after: String,
    expiry: Option<String>,
    linked_closeout: String,
}

impl DrainWindowToml {
    pub(super) fn into_drain_window(self) -> DrainWindow {
        DrainWindow {
            mirror_ledger: self.mirror_ledger,
            drain_owner: self.drain_owner,
            drain_reason: self.drain_reason,
            review_after: self.review_after,
            expiry: self.expiry,
            linked_closeout: self.linked_closeout,
        }
    }
}

pub(super) fn parse_drain_windows(raw: &[DrainWindowToml]) -> Vec<DrainWindow> {
    raw.iter()
        .cloned()
        .map(DrainWindowToml::into_drain_window)
        .collect()
}
