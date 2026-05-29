#![no_main]

use allow_policy::{parse_policy, render_policy, starter_policy, validate_policy};
use libfuzzer_sys::fuzz_target;

const MAX_INPUT: usize = 64 * 1024;

fuzz_target!(|data: &[u8]| {
    let input = String::from_utf8_lossy(&data[..data.len().min(MAX_INPUT)]);

    if let Ok(config) = parse_policy(&input) {
        let _ = validate_policy(&config);
        let rendered = render_policy(&config);
        let reparsed = parse_policy(&rendered).expect("rendered policy must parse");
        let rerendered = render_policy(&reparsed);
        assert_eq!(rendered, rerendered, "policy rendering should be stable");
    }

    let starter = starter_policy(false);
    let _ = parse_policy(&starter).expect("starter policy must parse");
    let strict_starter = starter_policy(true);
    let _ = parse_policy(&strict_starter).expect("strict starter policy must parse");
});
