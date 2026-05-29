use serde_json::Value;

pub fn assert_json_u64(value: &Value, pointer: &str, expected: u64, message: &str) {
    assert_eq!(
        value.pointer(pointer).and_then(Value::as_u64),
        Some(expected),
        "{message}"
    );
}

pub fn assert_json_str(value: &Value, pointer: &str, expected: &str, message: &str) {
    assert_eq!(
        value.pointer(pointer).and_then(Value::as_str),
        Some(expected),
        "{message}"
    );
}
