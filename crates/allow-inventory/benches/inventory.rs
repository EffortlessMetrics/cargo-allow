use allow_inventory::{InventoryOptions, inventory_files};
use std::fs;
use std::hint::black_box;
use std::path::PathBuf;
use std::time::Instant;

fn fixture() -> Option<PathBuf> {
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-inventory-bench-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    for index in 0..2_000 {
        let dir = root.join(format!("src/module_{}", index % 100));
        if fs::create_dir_all(&dir).is_err()
            || fs::write(dir.join(format!("file_{index}.rs")), "fn item() {}\n").is_err()
        {
            return None;
        }
    }
    Some(root)
}

fn inventory() -> usize {
    let Some(root) = fixture() else {
        eprintln!("unable to create inventory benchmark fixture");
        return 0;
    };
    let options = InventoryOptions {
        include_untracked: true,
        ..InventoryOptions::default()
    };
    let start = Instant::now();
    let mut files = 0;
    for _ in 0..10 {
        match inventory_files(&root, &options) {
            Ok(inventory) => files += inventory.len(),
            Err(error) => {
                eprintln!("inventory benchmark failed: {error}");
                return 0;
            }
        }
    }
    println!(
        "inventory_files_2000: {:?} ({files} files)",
        start.elapsed()
    );
    files
}

fn main() {
    black_box(inventory());
}
