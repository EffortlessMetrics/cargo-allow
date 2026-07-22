#[path = "snapshot_package/staged_index.rs"]
mod staged_index_impl;

pub use staged_index_impl::*;

const _: usize = core::mem::size_of::<staged_index_impl::StagedIndexSurface>();
const _STAGED_INDEX_SURFACE: &str = staged_index_impl::StagedIndexSurface::MODULE_ID;
