use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoAllowError {
    message: String,
}

impl CargoAllowError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for CargoAllowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CargoAllowError {}

pub type CargoAllowResult<T> = Result<T, CargoAllowError>;
