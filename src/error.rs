
use std::{fmt, io};

#[derive(Debug)]
pub enum Error {
    IoError(io::Error),
    InvalidResponse(serde_json::Error),
    UnknownInvalidResponse,
    InvalidCredentials,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IoError(value) => write!(f, "IoError: {}", value),
            Self::InvalidResponse(value) => write!(f, "InvalidResponse: {}", value),
            Self::UnknownInvalidResponse => write!(f, "UnknownInvalidResponse"), // todo: what the fuck is this?
            Self::InvalidCredentials => write!(f, "InvalidCredentials"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Self::IoError(value)
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self::InvalidResponse(value)
    }
}
