pub fn chosen(value: Option<&str>) -> &str {
    value.expect("fixture pins an expect call")
}

pub fn guard(size: usize) -> &'static str {
    match size {
        0 => unreachable!("fixture pins an unreachable macro"),
        _ => "ok",
    }
}
