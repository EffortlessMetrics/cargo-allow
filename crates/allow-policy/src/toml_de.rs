use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum StringList {
    One(String),
    Many(Vec<String>),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Boolish {
    Bool(bool),
    String(String),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum U32ish {
    Number(u32),
    String(String),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum SchemaVersionish {
    Integer(i64),
    String(String),
}

pub(crate) fn string_or_vec<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match StringList::deserialize(deserializer)? {
        StringList::One(value) => Ok(vec![value]),
        StringList::Many(values) => Ok(values),
    }
}

pub(crate) fn option_bool_or_string<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match Option::<Boolish>::deserialize(deserializer)? {
        Some(Boolish::Bool(value)) => Ok(Some(value)),
        Some(Boolish::String(value)) => value
            .parse::<bool>()
            .map(Some)
            .map_err(serde::de::Error::custom),
        None => Ok(None),
    }
}

pub(crate) fn option_u32_or_string<'de, D>(deserializer: D) -> Result<Option<u32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match Option::<U32ish>::deserialize(deserializer)? {
        Some(U32ish::Number(value)) => Ok(Some(value)),
        Some(U32ish::String(value)) => value
            .parse::<u32>()
            .map(Some)
            .map_err(serde::de::Error::custom),
        None => Ok(None),
    }
}

pub(crate) fn option_schema_version<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match Option::<SchemaVersionish>::deserialize(deserializer)? {
        Some(SchemaVersionish::Integer(value)) => Ok(Some(value.to_string())),
        Some(SchemaVersionish::String(value)) => Ok(Some(value)),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Deserialize)]
    struct StringVecFixture {
        #[serde(default, deserialize_with = "string_or_vec")]
        values: Vec<String>,
    }

    #[derive(Debug, Deserialize)]
    struct BoolFixture {
        #[serde(default, deserialize_with = "option_bool_or_string")]
        enabled: Option<bool>,
    }

    #[derive(Debug, Deserialize)]
    struct U32Fixture {
        #[serde(default, deserialize_with = "option_u32_or_string")]
        count: Option<u32>,
    }

    #[derive(Debug, Deserialize)]
    struct SchemaVersionFixture {
        #[serde(default, deserialize_with = "option_schema_version")]
        schema_version: Option<String>,
    }

    fn parse<T: serde::de::DeserializeOwned>(input: &str) -> Result<T, toml::de::Error> {
        toml::from_str(input)
    }

    #[test]
    fn string_or_vec_accepts_scalar_array_and_missing_values() {
        let scalar = parse::<StringVecFixture>("values = \"one\"")
            .unwrap_or_else(|err| std::panic::panic_any(format!("scalar parses: {err}")));
        assert_eq!(scalar.values, vec!["one".to_string()]);

        let array = parse::<StringVecFixture>("values = [\"one\", \"two\"]")
            .unwrap_or_else(|err| std::panic::panic_any(format!("array parses: {err}")));
        assert_eq!(array.values, vec!["one".to_string(), "two".to_string()]);

        let missing = parse::<StringVecFixture>("")
            .unwrap_or_else(|err| std::panic::panic_any(format!("missing default parses: {err}")));
        assert!(missing.values.is_empty());
    }

    #[test]
    fn string_or_vec_rejects_non_string_shapes() {
        let number = parse::<StringVecFixture>("values = 7")
            .err()
            .unwrap_or_else(|| std::panic::panic_any("numeric scalar should fail"));
        assert!(
            number
                .to_string()
                .contains("data did not match any variant")
        );

        let mixed = parse::<StringVecFixture>("values = [\"one\", 7]")
            .err()
            .unwrap_or_else(|| std::panic::panic_any("mixed array should fail"));
        assert!(mixed.to_string().contains("data did not match any variant"));
    }

    #[test]
    fn option_bool_or_string_accepts_bool_string_and_missing_values() {
        let bool_value = parse::<BoolFixture>("enabled = true")
            .unwrap_or_else(|err| std::panic::panic_any(format!("bool parses: {err}")));
        assert_eq!(bool_value.enabled, Some(true));

        let string_true = parse::<BoolFixture>("enabled = \"true\"")
            .unwrap_or_else(|err| std::panic::panic_any(format!("string true parses: {err}")));
        assert_eq!(string_true.enabled, Some(true));

        let string_false = parse::<BoolFixture>("enabled = \"false\"")
            .unwrap_or_else(|err| std::panic::panic_any(format!("string false parses: {err}")));
        assert_eq!(string_false.enabled, Some(false));

        let missing = parse::<BoolFixture>("")
            .unwrap_or_else(|err| std::panic::panic_any(format!("missing bool parses: {err}")));
        assert_eq!(missing.enabled, None);
    }

    #[test]
    fn option_bool_or_string_rejects_invalid_strings_and_shapes() {
        let invalid = parse::<BoolFixture>("enabled = \"yes\"")
            .err()
            .unwrap_or_else(|| std::panic::panic_any("invalid bool string should fail"));
        assert!(invalid.to_string().contains("provided string was not"));

        let number = parse::<BoolFixture>("enabled = 1")
            .err()
            .unwrap_or_else(|| std::panic::panic_any("numeric bool should fail"));
        assert!(
            number
                .to_string()
                .contains("data did not match any variant")
        );
    }

    #[test]
    fn option_u32_or_string_accepts_number_string_and_missing_values() {
        let number = parse::<U32Fixture>("count = 7")
            .unwrap_or_else(|err| std::panic::panic_any(format!("number parses: {err}")));
        assert_eq!(number.count, Some(7));

        let string = parse::<U32Fixture>("count = \"8\"")
            .unwrap_or_else(|err| std::panic::panic_any(format!("string number parses: {err}")));
        assert_eq!(string.count, Some(8));

        let missing = parse::<U32Fixture>("")
            .unwrap_or_else(|err| std::panic::panic_any(format!("missing u32 parses: {err}")));
        assert_eq!(missing.count, None);
    }

    #[test]
    fn option_schema_version_accepts_integer_string_and_missing_values() {
        let integer = parse::<SchemaVersionFixture>("schema_version = 1")
            .unwrap_or_else(|err| std::panic::panic_any(format!("integer parses: {err}")));
        assert_eq!(integer.schema_version.as_deref(), Some("1"));

        let string = parse::<SchemaVersionFixture>("schema_version = \"0.1\"")
            .unwrap_or_else(|err| std::panic::panic_any(format!("string parses: {err}")));
        assert_eq!(string.schema_version.as_deref(), Some("0.1"));

        let missing = parse::<SchemaVersionFixture>("")
            .unwrap_or_else(|err| std::panic::panic_any(format!("missing schema parses: {err}")));
        assert_eq!(missing.schema_version, None);
    }

    #[test]
    fn option_u32_or_string_rejects_invalid_string_and_numeric_boundaries() {
        let invalid = parse::<U32Fixture>("count = \"many\"")
            .err()
            .unwrap_or_else(|| std::panic::panic_any("invalid u32 string should fail"));
        assert!(invalid.to_string().contains("invalid digit"));

        let negative = parse::<U32Fixture>("count = -1")
            .err()
            .unwrap_or_else(|| std::panic::panic_any("negative number should fail"));
        assert!(
            negative
                .to_string()
                .contains("data did not match any variant")
        );

        let too_large = parse::<U32Fixture>("count = 4294967296")
            .err()
            .unwrap_or_else(|| std::panic::panic_any("out-of-range number should fail"));
        assert!(
            too_large
                .to_string()
                .contains("data did not match any variant")
        );
    }
}
