
use std::{fmt, io};

use crate::util::OptionError;

#[derive(Debug)]
pub enum Error {
    IoError(io::Error),
    NetworkError(ureq::Error),
    InvalidResponse(serde_json::Error),
    UnknownInvalidResponse,
    CannotDownload(ureq::Error),
    InvalidCredentials,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IoError(value) => write!(f, "IoError: {}", value),
            Self::NetworkError(value) => write!(f, "NetworkError: {}", value),
            Self::CannotDownload(value) => write!(f, "CannotDownload: {}", value),
            Self::InvalidResponse(value) => write!(f, "InvalidResponse: {}", value),
            Self::UnknownInvalidResponse => write!(f, "UnknownInvalidResponse"),
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

impl From<ureq::Error> for Error {
    fn from(value: ureq::Error) -> Self {
        Self::NetworkError(value)
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self::InvalidResponse(value)
    }
}

impl From<OptionError<'_>> for Error {
    fn from(_: OptionError) -> Self {
        Self::UnknownInvalidResponse
    }
}

