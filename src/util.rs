
use std::{error::Error, panic::Location, fmt};

pub(crate) struct OptionError<'a>(Location<'a>);

impl<'a> Error for OptionError<'a> {}

impl<'a> fmt::Debug for OptionError<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "Got None at `{}`", self.0)
    }
}

impl<'a> fmt::Display for OptionError<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, fmt)
    }
}

pub(crate) trait OptionToResult<T> {
    fn some<'d>(self) -> Result<T, OptionError<'d>>;
}

impl<T> OptionToResult<T> for Option<T> {
    #[track_caller]
    fn some<'d>(self) -> Result<T, OptionError<'d>> {
        match self {
            Some(t) => Ok(t),
            None => Err(OptionError(Location::caller().clone()))
        }
    }
}

