#![no_main]

use allow_policy::{parse_policy, render_policy, validate_policy};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let input = String::from_utf8_lossy(data);
    if let Ok(config) = parse_policy(&input) {
        // Exercise validation on successfully decoded policies, then ensure the
        // canonical renderer only emits policy text that the parser accepts.
        let _ = validate_policy(&config);
        let rendered = render_policy(&config);
        let reparsed = parse_policy(&rendered).expect("rendered policy must parse");
        validate_policy(&reparsed).expect("rendered policy must validate");
    }
});
