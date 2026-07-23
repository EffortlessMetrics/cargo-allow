#[path = "snapshot_package/revision_identity.rs"]
mod revision_identity_impl;

pub use revision_identity_impl::*;

const _: usize = core::mem::size_of::<revision_identity_impl::RevisionIdentitySurface>();
const _REVISION_IDENTITY_SURFACE: &str = revision_identity_impl::RevisionIdentitySurface::MODULE_ID;
