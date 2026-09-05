use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid syntax")]
    Syntax,
    #[error("leftover bytes")]
    Leftover,
    #[error("incompatible integer")]
    FromInt(#[from] std::num::TryFromIntError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Other(String),
}

impl From<Error> for io::Error {
    fn from(value: Error) -> Self {
        match value {
            Error::Syntax | Error::Leftover => io::Error::other(value),
            Error::FromInt(e) => io::Error::other(e),
            Error::Io(e) => e,
            Error::Other(e) => io::Error::other(e),
        }
    }
}

impl Error {
    pub const INVALID_UTF8: Self = Self::Io(io::const_error!(
        io::ErrorKind::InvalidData,
        "stream did not contain valid UTF-8",
    ));

    pub const READ_EXACT_EOF: Self = Self::Io(io::const_error!(
        io::ErrorKind::UnexpectedEof,
        "failed to fill whole buffer",
    ));
}

impl serde::ser::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: std::fmt::Display,
    {
        Self::Other(msg.to_string())
    }
}

impl serde::de::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: std::fmt::Display,
    {
        Self::Other(msg.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
