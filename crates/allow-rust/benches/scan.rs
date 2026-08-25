use allow_rust::scan_rust_source;
use std::hint::black_box;
use std::time::Instant;

fn source() -> String {
    (0..2_000)
        .map(|index| {
            format!(
                "fn item_{index}(value: Option<u8>) -> u8 {{ value.unwrap(); assert!(true); unsafe {{ 0 }} }}\n"
            )
        })
        .collect()
}

fn scan() {
    let source = source();
    let start = Instant::now();
    let mut findings = 0;
    for _ in 0..10 {
        findings += scan_rust_source("benches/fixture.rs", &source).len();
    }
    println!(
        "scan_rust_source_2000_lines: {:?} ({findings} findings)",
        start.elapsed()
    );
}

fn main() {
    black_box(scan());
}
