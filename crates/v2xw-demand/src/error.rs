//! The crate's error type.

use std::path::Path;

/// Everything that can go wrong building demand.
#[derive(Debug, thiserror::Error)]
pub enum DemandError {
    /// A file could not be read or written.
    #[error("{path}: {message}")]
    Io {
        /// The file.
        path: String,
        /// What happened.
        message: String,
    },
    /// A file's content does not parse.
    #[error("{path}: {message}")]
    Parse {
        /// The file (or `<inline>`).
        path: String,
        /// What is wrong, with the line when known.
        message: String,
    },
    /// A route or trip file was made for another world.
    #[error(
        "made for world {recorded}, but this world is {actual}: re-run the assignment or \
         the trip generation on this world"
    )]
    WorldMismatch {
        /// The hash the file records.
        recorded: String,
        /// This world's hash.
        actual: String,
    },
    /// The parameters do not make sense.
    #[error("{0}")]
    Invalid(String),
}

impl DemandError {
    pub(crate) fn io(path: &Path, e: std::io::Error) -> Self {
        DemandError::Io {
            path: path.display().to_string(),
            message: e.to_string(),
        }
    }

    pub(crate) fn parse(path: &Path, message: impl Into<String>) -> Self {
        DemandError::Parse {
            path: path.display().to_string(),
            message: message.into(),
        }
    }

    pub(crate) fn inline(message: impl Into<String>) -> Self {
        DemandError::Parse {
            path: "<inline>".to_string(),
            message: message.into(),
        }
    }
}

/// The crate's result type.
pub type Result<T> = std::result::Result<T, DemandError>;
