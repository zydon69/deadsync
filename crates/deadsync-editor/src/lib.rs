//! Editor domain independent of rendering, audio playback, and gameplay state.
//!
//! Simfiles retain their original bytes. Edits replace explicit value spans,
//! leaving unknown tags, comments, encodings, and unedited charts intact.
pub mod analysis;
pub mod chart_properties;
pub mod document;
pub mod generation;
pub mod history;
pub mod notes;
mod number;
pub mod pad;
pub mod playback;
pub mod runtime;
pub mod storage;
pub mod timing_edit;
pub mod transform;
pub mod workspace;

use std::fmt;

#[derive(Debug)]
pub enum Error {
    InvalidFormat(String),
    Unsupported(String),
    InvalidEdit(String),
    ExternalChange,
    Io(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFormat(message)
            | Self::Unsupported(message)
            | Self::InvalidEdit(message) => f.write_str(message),
            Self::ExternalChange => {
                f.write_str("The simfile changed on disk; reload or save a copy.")
            }
            Self::Io(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}
