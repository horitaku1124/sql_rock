use std::error::Error;
use std::fmt;
use std::io;

pub type Result<T> = std::result::Result<T, SqlRockError>;

#[derive(Debug)]
pub struct SqlRockError {
    message: String,
}

impl SqlRockError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for SqlRockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for SqlRockError {}

impl From<io::Error> for SqlRockError {
    fn from(value: io::Error) -> Self {
        Self::new(value.to_string())
    }
}
