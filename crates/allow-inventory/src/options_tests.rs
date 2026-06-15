use super::InventorySource;

#[test]
fn inventory_source_as_str_match_arm_observer() {
    assert_eq!(InventorySource::GitTracked.as_str(), "git_tracked");
    assert_eq!(
        InventorySource::FilesystemFallback.as_str(),
        "filesystem_fallback"
    );
    assert_eq!(
        InventorySource::FilesystemIncludeUntracked.as_str(),
        "filesystem_include_untracked"
    );
}
