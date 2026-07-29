use allow_core::FindingKind;
use std::path::Path;

#[derive(Debug, Clone)]
pub(super) struct PruneContext<'a> {
    pub(super) inventory: allow_report::InventoryContext<'a>,
    pub(super) mutation_receipt: allow_report::MutationReceipt<'a>,
}

pub(super) struct PruneRenderMode {
    explicit_dry_run: bool,
    write_requested: bool,
    written_path: Option<String>,
    total_entries: usize,
}

impl PruneRenderMode {
    pub(super) fn new(
        explicit_dry_run: bool,
        write_requested: bool,
        written_path: Option<&Path>,
        total_entries: usize,
    ) -> Self {
        Self {
            explicit_dry_run,
            write_requested,
            written_path: written_path.map(|path| path.display().to_string()),
            total_entries,
        }
    }

    pub(super) fn context(&self) -> allow_report::PruneModeContext<'_> {
        allow_report::PruneModeContext {
            explicit_dry_run: self.explicit_dry_run,
            write_requested: self.write_requested,
            written_path: self.written_path.as_deref(),
            total_entries: self.total_entries,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PruneCandidate {
    pub(super) id: String,
    pub(super) kind: FindingKind,
    pub(super) family: Option<String>,
    pub(super) owner: String,
    pub(super) classification: String,
    pub(super) scope: String,
    pub(super) reason: String,
}
