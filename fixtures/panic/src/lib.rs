pub fn load_fixture(path: &str) -> String {
    std::fs::read_to_string(path).unwrap()
}

pub fn token_at(tokens: &[String], idx: usize) -> &String {
    &tokens[idx]
}
