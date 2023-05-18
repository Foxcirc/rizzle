
use std::{fmt::{self, Display}, io};

use crate::util::OptionError;

#[derive(Debug)]
pub enum Error {
    IoError(io::Error),
    NetworkError(ureq::Error),
    InvalidResponse,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IoError(value) => Display::fmt(value, f),
            Self::NetworkError(value) => Display::fmt(value, f),
            Self::InvalidResponse => f.write_str("Invalid response"),
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
    fn from(_: serde_json::Error) -> Self {
        Self::InvalidResponse
    }
}

impl From<OptionError<'_>> for Error {
    fn from(_: OptionError) -> Self {
        Self::InvalidResponse
    }
}

