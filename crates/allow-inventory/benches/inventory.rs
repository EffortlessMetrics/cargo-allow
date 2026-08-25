use allow_inventory::{InventoryOptions, inventory_files};
use std::fs;
use std::hint::black_box;
use std::path::PathBuf;
use std::time::Instant;

fn fixture() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-inventory-bench-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    for index in 0..2_000 {
        let dir = root.join(format!("src/module_{}", index % 100));
        fs::create_dir_all(&dir).expect("create benchmark fixture");
        fs::write(
            dir.join(format!("file_{index}.rs")),
            "fn item() {}\n",
        )
        .expect("write benchmark fixture");
    }
    root
}

fn inventory() {
    let root = fixture();
    let options = InventoryOptions {
        include_untracked: true,
        ..InventoryOptions::default()
    };
    let start = Instant::now();
    let mut files = 0;
    for _ in 0..10 {
        files += inventory_files(&root, &options)
            .expect("inventory benchmark")
            .len();
    }
    println!("inventory_files_2000: {:?} ({files} files)", start.elapsed());
}

fn main() {
    black_box(inventory());
}
