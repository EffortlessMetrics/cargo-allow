use super::is_default_policy_path;

#[test]
fn default_policy_spellings_compare_equal_across_prefix_forms() {
    // #4558/#4573: the relative, absolute, and UNC-spelled forms of the
    // default policy location must all compare as the default (the
    // canonicalized UNC root collapses to one leading slash while the
    // user spelling keeps two), and a genuinely different path must
    // not.
    let root = std::path::Path::new("/repo-root");
    assert!(is_default_policy_path(
        std::path::Path::new("/repo-root/policy/allow.toml"),
        root
    ));
    let unc_root = std::path::Path::new("//server/share/repo");
    assert!(is_default_policy_path(
        std::path::Path::new("//server/share/repo/policy/allow.toml"),
        unc_root
    ));
    // #4574 review: parent segments must fold before the missing-file
    // walk, or the reconstruction lands on the wrong base.
    assert!(is_default_policy_path(
        std::path::Path::new("/repo-root/policy/../policy/allow.toml"),
        root
    ));
    assert!(!is_default_policy_path(
        std::path::Path::new("/repo-root/custom/other.toml"),
        root
    ));
}
