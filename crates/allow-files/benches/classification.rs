use allow_files::classify_path;
use std::hint::black_box;
use std::path::PathBuf;
use std::time::Instant;

fn classification() {
    let paths: Vec<_> = (0..2_000)
        .map(|index| {
            let path = match index % 5 {
                0 => format!(".github/workflows/workflow_{index}.yml"),
                1 => format!("scripts/script_{index}.sh"),
                2 => format!("docs/doc_{index}.md"),
                3 => format!("Cargo_{index}.toml"),
                _ => format!("src/module_{}/file_{index}.rs", index % 100),
            };
            PathBuf::from(path)
        })
        .collect();
    let start = Instant::now();
    let mut classified = 0;
    for _ in 0..10 {
        classified += paths.iter().filter_map(|path| classify_path(path)).count();
    }
    println!("classify_path_2000: {:?} ({classified} classified)", start.elapsed());
}

fn main() {
    black_box(classification());
}
