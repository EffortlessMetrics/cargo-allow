use std::fs;

use super::unique_test_dir;

#[test]
fn test_root_drop_call_presence_observer() {
    let path = {
        let root = unique_test_dir("drop-observer");
        let path = root.as_ref().to_path_buf();
        fs::create_dir_all(&path)
            .unwrap_or_else(|err| std::panic::panic_any(format!("fixture dir: {err}")));
        assert!(path.is_dir());
        path
    };

    assert!(!path.exists());
}
