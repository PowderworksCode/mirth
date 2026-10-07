//! Error types, `?`, `From` conversions, `dyn Error`, `Result` chains.

use core::fmt;
use std::error::Error;
use std::num::ParseIntError;

#[derive(Debug)]
pub enum SinkError {
    Parse(ParseIntError),
    Range { value: i64, max: i64 },
    Other(Box<dyn Error + Send + Sync>),
}

impl fmt::Display for SinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SinkError::Parse(e) => write!(f, "parse: {e}"),
            SinkError::Range { value, max } => write!(f, "{value} is over {max}"),
            SinkError::Other(e) => write!(f, "other: {e}"),
        }
    }
}

impl Error for SinkError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            SinkError::Parse(e) => Some(e),
            _ => None,
        }
    }
}

impl From<ParseIntError> for SinkError {
    fn from(e: ParseIntError) -> Self {
        SinkError::Parse(e)
    }
}

pub fn parse_bounded(s: &str, max: i64) -> Result<i64, SinkError> {
    let value: i64 = s.trim().parse()?;
    if value > max {
        return Err(SinkError::Range { value, max });
    }
    Ok(value)
}

pub fn sum_all(items: &[&str]) -> Result<i64, Box<dyn Error>> {
    let mut total = 0;
    for item in items {
        total += parse_bounded(item, 1000)?;
    }
    Ok(total)
}

pub fn first_char_upper(s: &str) -> Option<char> {
    Some(s.chars().next()?.to_ascii_uppercase())
}
