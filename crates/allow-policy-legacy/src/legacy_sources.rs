use std::path::Path;

use crate::migration_lane_descriptors::descriptor_for_legacy_filename;

pub struct LegacyPolicySource {
    pub file_name: String,
    pub compat_kind: &'static str,
}

pub fn legacy_compat_kind(file_name: &str) -> Option<&'static str> {
    descriptor_for_legacy_filename(file_name).map(|descriptor| descriptor.compat_kind_id())
}

pub fn legacy_policy_source_for_path(path: &Path) -> Option<LegacyPolicySource> {
    let file_name = path.file_name()?.to_str()?;
    let compat_kind = legacy_compat_kind(file_name)?;
    Some(LegacyPolicySource {
        file_name: file_name.to_string(),
        compat_kind,
    })
}

pub fn list_legacy_policy_sources_in_dir(dir: &Path) -> Vec<LegacyPolicySource> {
    crate::migration_lane_descriptors::legacy_policy_filenames()
        .filter_map(|file_name| {
            let path = dir.join(file_name);
            if !path.is_file() {
                return None;
            }
            let compat_kind = legacy_compat_kind(file_name)?;
            Some(LegacyPolicySource {
                file_name: file_name.to_string(),
                compat_kind,
            })
        })
        .collect()
}
