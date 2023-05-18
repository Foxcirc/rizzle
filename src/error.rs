
use std::fmt;

pub enum RizzleError {

}

impl fmt::Debug for RizzleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("")
    }
}

impl fmt::Display for RizzleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("")
    }
}

impl std::error::Error for RizzleError {}

