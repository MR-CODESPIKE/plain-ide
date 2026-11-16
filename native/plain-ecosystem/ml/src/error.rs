//! Errors returned by the ML library instead of panicking.
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum MlError {
    /// Tensor shapes do not fit the operation.
    Shape(String),
    /// An argument has an invalid value.
    Invalid(String),
    /// Reading or writing a file failed.
    Io(String),
    /// A checkpoint is malformed or does not match the model.
    Format(String),
}

impl fmt::Display for MlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MlError::Shape(m) => write!(f, "shape error: {}", m),
            MlError::Invalid(m) => write!(f, "invalid argument: {}", m),
            MlError::Io(m) => write!(f, "i/o error: {}", m),
            MlError::Format(m) => write!(f, "bad checkpoint: {}", m),
        }
    }
}

impl std::error::Error for MlError {}

pub type MlResult<T> = Result<T, MlError>;
