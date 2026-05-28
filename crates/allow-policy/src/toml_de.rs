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
