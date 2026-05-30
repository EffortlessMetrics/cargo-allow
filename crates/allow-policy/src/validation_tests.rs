use super::*;

mod entry;
mod lifecycle;
mod policy_header;
mod selector;
mod workspace;

fn parse_err(input: &str) -> String {
    match parse_policy(input) {
        Ok(_) => std::panic::panic_any("expected policy parse failure"),
        Err(err) => err.to_string(),
    }
}
