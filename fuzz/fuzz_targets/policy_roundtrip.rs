#![no_main]

use allow_policy::{parse_policy, render_policy};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };

    if let Ok(policy) = parse_policy(input) {
        let rendered = render_policy(&policy);
        let reparsed = parse_policy(&rendered).expect("rendered policy should parse");
        assert_eq!(policy, reparsed);
    }
});
